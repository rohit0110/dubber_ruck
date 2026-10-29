"""Stage-1 export computed in the sandbox (PyTorch fp32 MiniLM) for any transcripts file, same fields as `bench.py stage1` on the phone.
Usage: lab/run.sh scripts/stage1_mac.py <transcripts.json> <out.json>
The quiz (key points, keywords, wrong claims) is always data/sdi-ch4-8.quiz.json.
"""
import json, sys

sys.path.insert(0, "/work/bench")
import torch
import bench as B
from transformers import AutoModel, AutoTokenizer

torch.set_num_threads(4)
tok = AutoTokenizer.from_pretrained("/models/minilm")
model = AutoModel.from_pretrained("/models/minilm", use_safetensors=True).eval()


def embed(texts):
    out = []
    for i in range(0, len(texts), 64):
        enc = tok(texts[i:i + 64], padding=True, truncation=True, max_length=256, return_tensors="pt")
        with torch.no_grad():
            h = model(**enc).last_hidden_state
        m = enc["attention_mask"].unsqueeze(-1).float()
        v = torch.nn.functional.normalize((h * m).sum(1) / m.sum(1).clamp(min=1e-9), dim=-1)
        out += v.tolist()
    return out


dot = lambda a, b: sum(x * y for x, y in zip(a, b))
qs = {q["id"]: q for q in json.load(open("/work/data/sdi-ch4-8.quiz.json"))["questions"]}
cases = json.load(open(sys.argv[1]))["cases"]
kp_vecs, w_vecs = {}, {}
for q in qs.values():
    kp_vecs[q["id"]] = embed([k["text"] for k in q["key_points"]])
    items = [(i, w) for i, k in enumerate(q["key_points"]) for w in k.get("wrong_claims", [])]
    w_vecs[q["id"]] = [(i, w, v) for (i, w), v in zip(items, embed([w for _, w in items]))] if items else []

points = []
for c in cases:
    q = qs[c["question_id"]]
    ch = B.chunks(c["transcript"])
    v = embed(ch)
    low = c["transcript"].lower()
    for i, kp in enumerate(q["key_points"]):
        sims = [dot(kp_vecs[q["id"]][i], x) for x in v]
        bi = max(range(len(sims)), key=sims.__getitem__)
        bw = (0.0, "", "")
        for j, w, vec in w_vecs[q["id"]]:
            if j != i:
                continue
            ws = [dot(vec, x) for x in v]
            wi = max(range(len(ws)), key=ws.__getitem__)
            if ws[wi] > bw[0]:
                bw = (ws[wi], w, ch[wi])
        points.append({"cid": c["id"], "kid": kp["id"], "qid": q["id"], "ch": c["id"].split("q")[0] or "all",
                       "var": c.get("kind") or c.get("variant"), "persona": c.get("persona", ""),
                       "mtype": c.get("mistake_types", {}).get(kp["id"], ""), "tags": c.get("tags", []),
                       "lab": c["labels"][kp["id"]], "question": q["question"], "kp": kp["text"],
                       "s": round(sims[bi], 4), "sent": ch[bi], "sw": round(bw[0], 4), "wclaim": bw[1], "wsent": bw[2],
                       "hits": sum(1 for k in kp["keywords"] if k.lower() in low)})
json.dump({"points": points}, open(sys.argv[2], "w"))
print(f"{len(cases)} answers, {len(points)} points -> {sys.argv[2]}")
