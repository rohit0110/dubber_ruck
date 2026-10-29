"""Zero-shot NLI test: cross-encoder/nli-MiniLM2-L6-H768 on our (closest sentence, key point) pairs.

Premise/hypothesis variants:
  P1: premise = closest transcript sentence,   hypothesis = key point   (entailment -> covered, contradiction -> contradicted)
  P2: premise = the whole spoken answer,       hypothesis = key point
  P3: premise = sentence closest to the typical mistake, hypothesis = that wrong claim (entailment -> speaker made that mistake)
Usage: nli.py [stage1.json transcripts.json out_scores.json]. Writes the scores (used by combiner.py / transfer.py) and prints markdown tables.
Run inside the sandbox: lab/run.sh scripts/nli.py
"""
import json, sys, time, collections

sys.path.insert(0, "/work/bench")
import torch
import ekm
from transformers import AutoModelForSequenceClassification, AutoTokenizer

torch.set_num_threads(4)
MODEL = "/models/nli-minilm2"
tok = AutoTokenizer.from_pretrained(MODEL)
model = AutoModelForSequenceClassification.from_pretrained(MODEL, use_safetensors=True).eval()
labels = {int(k): v.lower() for k, v in model.config.id2label.items()}
idx = {v: k for k, v in labels.items()}
print("model labels:", labels, "| parameters:", sum(p.numel() for p in model.parameters()) // 1_000_000, "M")
CON, ENT, NEU = idx["contradiction"], idx["entailment"], idx["neutral"]

ARGS = sys.argv[1:] + [None] * 3
STAGE1 = ARGS[0] or "/work/bench/results/stage1_sdi.json"
TRANSCRIPTS = ARGS[1] or "/work/tests/sdi-ch4-8.transcripts.json"
OUT = ARGS[2] or "/work/out/nli_scores.json"
pts = ekm.load(STAGE1)
cases = {c["id"]: c for c in json.load(open(TRANSCRIPTS))["cases"]}


def score(pairs, bs=32):
    out = []
    for i in range(0, len(pairs), bs):
        b = pairs[i:i + bs]
        enc = tok([a for a, _ in b], [h for _, h in b], padding=True, truncation=True, max_length=256, return_tensors="pt")
        with torch.no_grad():
            out += torch.softmax(model(**enc).logits, -1).tolist()
    return out


def timed(name, pairs):
    t = time.time()
    r = score(pairs)
    dt = time.time() - t
    print(f"{name}: {len(pairs)} pairs in {dt:.1f} s ({dt / len(pairs) * 1000:.0f} ms per pair on this Mac container, 4 threads; the phone will be far slower)")
    return r


p1 = timed("P1 sentence vs key point", [(p["sent"], p["kp"]) for p in pts])
p2 = timed("P2 whole answer vs key point", [(cases[p["cid"]]["transcript"], p["kp"]) for p in pts])
p3 = timed("P3 sentence vs wrong claim", [(p["wsent"] or p["sent"], p["wclaim"] or p["kp"]) for p in pts])
mk = lambda v: {"con": v[CON], "ent": v[ENT], "neu": v[NEU]}
scores = [{"cid": p["cid"], "kid": p["kid"], "p1": mk(a), "p2": mk(b), "p3": mk(c)} for p, a, b, c in zip(pts, p1, p2, p3)]
json.dump({"labels": labels, "scores": scores}, open(OUT, "w"))

VERD = {ENT: "covered", CON: "contradicted", NEU: "missing"}
HEAD = ["| Premise | Exact | Flag P | Flag R | Flag F1 | Contradicted flagged / named | False 'contradicted' |", "|---|---|---|---|---|---|---|"]


def line(name, pairs):
    m = ekm.metrics(pairs)
    return f"| {name} | {m['exact']} | {m['flag_p']} | {m['flag_r']} | {m['flag_f1']} | {m['contradicted_flagged']} / {m['contradicted_named']} | {m['false_contradicted']} |"


print("\n## Zero-shot NLI alone (argmax: entailment = covered, contradiction = contradicted, neutral = missing), 580 points")
rows = []
for name, pr in [("P1 closest sentence", p1), ("P2 whole answer", p2)]:
    rows.append(line(name, [(p["lab"], VERD[max(range(3), key=v.__getitem__)]) for p, v in zip(pts, pr)]))
print("\n".join(HEAD + rows))

print("\nConfusion, P1 (label -> NLI verdict):", {f"{a[:3]}->{b[:3]}": c for (a, b), c in sorted(collections.Counter(
    (p["lab"], VERD[max(range(3), key=v.__getitem__)]) for p, v in zip(pts, p1)).items())})
print("Confusion, P2 (label -> NLI verdict):", {f"{a[:3]}->{b[:3]}": c for (a, b), c in sorted(collections.Counter(
    (p["lab"], VERD[max(range(3), key=v.__getitem__)]) for p, v in zip(pts, p2)).items())})

print("\n## P3: does the speaker's sentence entail the typical mistake? (entailment >= 0.5 -> 'made that mistake')")
tp = sum(1 for p, v in zip(pts, p3) if p["lab"] == "contradicted" and v[ENT] >= 0.5)
fp = sum(1 for p, v in zip(pts, p3) if p["lab"] != "contradicted" and v[ENT] >= 0.5)
nc = sum(1 for p in pts if p["lab"] == "contradicted")
print(f"named {tp} of {nc} contradictions, {fp} false alarms among the other {len(pts) - nc} points")

print("\n## NLI as second opinion on EKM stage 1 (rule-based, leave-one-chapter-out), routed points only (stage 1 flags it or it is borderline)")
st = ekm.loco(pts)
by = {(p["cid"], p["kid"]): (a, b) for p, a, b in zip(pts, p1, p2)}
HEAD2 = ["| Policy | Exact | Flag P | Flag R | Flag F1 | Contradicted flagged / named | False 'contradicted' |", "|---|---|---|---|---|---|---|"]
rows = []
for prem, k in [("P1 sentence", 0), ("P2 whole answer", 1)]:
    def verdict(p):
        v = by[(p["cid"], p["kid"])][k]
        return VERD[max(range(3), key=v.__getitem__)]
    for name, pol in [("EKM stage 1 only", None), ("NLI vetoes flags", "veto"), ("NLI vetoes flags, escalates borderline", "esc"),
                      ("NLI names contradictions", "name"), ("NLI decides on routed points", "decide")]:
        if pol is None and k == 1:
            continue
        pairs = []
        for p in st:
            s1, f = p["stage1"], p["stage1"]
            routed = s1 != "covered" or p["margin"] < 0.08
            if routed and pol:
                v = verdict(p)
                if pol == "veto" and s1 != "covered" and v == "covered":
                    f = "covered"
                elif pol == "esc":
                    f = "covered" if (s1 != "covered" and v == "covered") else (v if (s1 == "covered" and v != "covered") else s1)
                elif pol == "name" and v == "contradicted":
                    f = "contradicted"
                elif pol == "decide":
                    f = v
            pairs.append((p["lab"], f))
        rows.append(line(f"{prem}: {name}" if pol else name, pairs))
print("\n".join(HEAD2 + rows))
