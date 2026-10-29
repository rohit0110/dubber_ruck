"""Score hosted-model judges against the hidden labels (cloud-ceiling test).

Judge files: out/judge_<name>/out_batch_*.json   ({"verdicts": {case: {kid: verdict}}, "notes": {...}})
Labels:     tests/ds3-independent.transcripts.json
Usage: lab/run.sh scripts/judge_eval.py <name>      e.g. haiku_ds3
"""
import collections, glob, json, sys

sys.path.insert(0, "/work/bench")
import ekm

name = sys.argv[1]
cases = {c["id"]: c for c in json.load(open("/work/tests/ds3-independent.transcripts.json"))["cases"]}
verd, notes, bad_files = {}, {}, []
for f in sorted(glob.glob(f"/work/out/judge_{name}/out_batch_*.json")):
    try:
        d = json.load(open(f))
        verd.update(d["verdicts"])
        for cid, n in d.get("notes", {}).items():
            notes[cid] = n
    except Exception as e:
        bad_files.append((f, repr(e)[:80]))
print(f"judge '{name}': {len(verd)} of {len(cases)} cases returned; unreadable files: {bad_files or 'none'}")
VALID = {"covered", "missing", "contradicted"}
pairs, rows, invalid, missing_case = [], [], 0, 0
for cid, c in cases.items():
    v = verd.get(cid)
    if v is None:
        missing_case += 1
    for kid, lab in c["labels"].items():
        p = (v or {}).get(kid)
        if p not in VALID:
            invalid += 1
            p = "covered"                     # an unanswered point gives no feedback: counted as 'covered' (the harmful default)
        pairs.append((lab, p))
        rows.append((cid, kid, lab, p, c["kind"], c.get("mistake_types", {}).get(kid, "")))
print(f"points scored: {len(pairs)} | cases not returned: {missing_case} | invalid or absent verdicts (counted as 'covered'): {invalid}")
m = ekm.metrics(pairs)
print("\n| Model | Exact | Flag P | Flag R | Flag F1 | Contradicted flagged / named | False 'contradicted' |\n|---|---|---|---|---|---|---|")
print(f"| {name} | {m['exact']} | {m['flag_p']} | {m['flag_r']} | {m['flag_f1']} | {m['contradicted_flagged']} / {m['contradicted_named']} | {m['false_contradicted']} |")
tp = sum(1 for l, p in pairs if l == "contradicted" and p == "contradicted")
fp = sum(1 for l, p in pairs if l != "contradicted" and p == "contradicted")
print(f"\nprecision of its 'contradicted' calls: {tp} right of {tp + fp} ({tp / max(1, tp + fp):.0%})")
print("confusion (label -> verdict):", {f"{a[:3]}->{b[:3]}": c for (a, b), c in sorted(collections.Counter(pairs).items())})

print("\n## By kind of answer")
print("| Kind | Covered points falsely flagged | Not-covered points called covered | Contradicted points named |\n|---|---|---|---|")
for k in sorted({r[4] for r in rows}):
    rk = [r for r in rows if r[4] == k]
    cv = [r for r in rk if r[2] == "covered"]
    nc = [r for r in rk if r[2] != "covered"]
    cn = [r for r in rk if r[2] == "contradicted"]
    print(f"| {k} | {sum(1 for r in cv if r[3] != 'covered')} / {len(cv)} | {sum(1 for r in nc if r[3] == 'covered')} / {len(nc)} | "
          f"{sum(1 for r in cn if r[3] == 'contradicted')} / {len(cn)} |")

print("\n## Contradicted points by mistake type: flagged as a problem / named 'contradicted'")
print("| Mistake type | n | flagged / named |\n|---|---|---|")
by = collections.defaultdict(list)
for r in rows:
    if r[2] == "contradicted":
        by[r[5]].append(r)
for t, rs in sorted(by.items(), key=lambda kv: -len(kv[1])):
    print(f"| {t} | {len(rs)} | {sum(1 for r in rs if r[3] != 'covered')} / {sum(1 for r in rs if r[3] == 'contradicted')} |")

print("\n## Sample of its notes on true contradictions (is the explanation right?)")
shown = 0
for cid, kid, lab, p, kind, mt in rows:
    if lab == "contradicted" and p == "contradicted" and notes.get(cid, {}).get(kid) and shown < 8:
        print(f"- [{cid} {kid} / {mt}] {notes[cid][kid]}")
        shown += 1
json.dump({"metrics": m, "contradicted_precision": [tp, tp + fp], "invalid": invalid, "missing_cases": missing_case},
          open(f"/work/out/judge_{name}_metrics.json", "w"), indent=1)
