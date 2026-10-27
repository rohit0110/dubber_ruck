import json, itertools, collections
qs = {q["id"]: q for q in json.load(open("data/sdi-ch4-8.quiz.json"))["questions"]}
cases = {c["id"]: c for c in json.load(open("tests/sdi-ch4-8.transcripts.json"))["cases"]}
emb = [r for r in json.load(open("bench/results/embed_sdi_Q4_0.json")) if r["name"] == "raw"][0]
W = {(c, k): sw for c, k, sw in emb["wscored"]}
P = []
for lab, s, cid, kid in emb["scored"]:
    c = cases[cid]; kp = next(k for k in qs[c["question_id"]]["key_points"] if k["id"] == kid)
    low = c["transcript"].lower()
    hits = sum(1 for k in kp["keywords"] if k.lower() in low)
    P.append(dict(lab=lab, s=s, sw=W[(cid, kid)], hits=hits, ch=cid[:2], var=c["variant"], cid=cid))
def f1(tp, fp, fn):
    p = tp / (tp + fp) if tp + fp else 0; r = tp / (tp + fn) if tp + fn else 0
    return (2 * p * r / (p + r) if p + r else 0), p, r
def flag_f1(rows, T, a):
    tp = fp = fn = 0
    for x in rows:
        cov = x["s"] + a * min(x["hits"], 2) >= T
        pos = x["lab"] != "covered"
        tp += pos and not cov; fp += (not pos) and not cov; fn += pos and cov
    return f1(tp, fp, fn)
def x_f1(rows, Tw, m):
    tp = fp = fn = 0
    for x in rows:
        pred = x["sw"] >= Tw and x["sw"] - x["s"] >= m
        pos = x["lab"] == "contradicted"
        tp += pos and pred; fp += (not pos) and pred; fn += pos and not pred
    return f1(tp, fp, fn)
Ts = [t / 100 for t in range(30, 82, 2)]; As = [0, .05, .1, .15, .2, .3]
TWs = [t / 100 for t in range(40, 95, 5)]; Ms = [-.1, -.05, 0, .05, .1, .15, .2]
chs = sorted({x["ch"] for x in P})
pred_rows = []
for held in chs:
    tr = [x for x in P if x["ch"] != held]; te = [x for x in P if x["ch"] == held]
    bT = max(((flag_f1(tr, T, 0)[0], T) for T in Ts))[1]
    bT2, bA = max(((flag_f1(tr, T, a)[0], (T, a)) for T in Ts for a in As))[1]
    bTw, bM = max(((x_f1(tr, tw, m)[0], (tw, m)) for tw in TWs for m in Ms))[1]
    for x in te:
        base_cov = x["s"] >= bT
        boost_cov = x["s"] + bA * min(x["hits"], 2) >= bT2
        xc = x["sw"] >= bTw and x["sw"] - x["s"] >= bM
        pred_rows.append(dict(x, base_cov=base_cov, boost_cov=boost_cov, xc=xc, kw_cov=x["hits"] >= 1))
    print(f"held-out {held}: T={bT}  boosted T,a={bT2},{bA}  wrong-claim Tw,m={bTw},{bM}")
def rep(name, cov_key, use_x=False):
    tp = fp = fn = 0; exact = 0
    for x in pred_rows:
        pos = x["lab"] != "covered"
        flagged = (not x[cov_key]) or (use_x and x["xc"])
        tp += pos and flagged; fp += (not pos) and flagged; fn += pos and not flagged
        pred = "contradicted" if (use_x and x["xc"]) else ("covered" if x[cov_key] else "missing")
        exact += pred == x["lab"]
    f, p, r = f1(tp, fp, fn)
    c = [x for x in pred_rows if x["lab"] == "contradicted"]
    named = sum(1 for x in c if use_x and x["xc"]); flagged_c = sum(1 for x in c if (not x[cov_key]) or (use_x and x["xc"]))
    fx = sum(1 for x in pred_rows if x["lab"] != "contradicted" and use_x and x["xc"])
    print(f"{name:52} exact {exact/len(pred_rows):.3f}  flag P {p:.3f} R {r:.3f} F1 {f:.3f}  contradicted flagged {flagged_c}/{len(c)} named {named}/{len(c)}  false 'contradicted' {fx}")
print("\nLeave-one-chapter-out (thresholds never see the chapter they are scored on), 580 points:")
rep("keywords >=1 hit", "kw_cov")
rep("embeddings only", "base_cov")
rep("embeddings + keyword boost (single score)", "boost_cov")
rep("embeddings + wrong_claims", "base_cov", True)
rep("embeddings + keyword boost + wrong_claims", "boost_cov", True)
print("\nBy variant, embeddings + keyword boost + wrong_claims, false flags on covered / missed on not-covered:")
for v in "ABCDEFGH":
    cov = [x for x in pred_rows if x["var"] == v and x["lab"] == "covered"]; nc = [x for x in pred_rows if x["var"] == v and x["lab"] != "covered"]
    ff = sum(1 for x in cov if (not x["boost_cov"]) or x["xc"]); mm = sum(1 for x in nc if x["boost_cov"] and not x["xc"])
    print(f"  {v}: covered points {len(cov):3} false flags {ff:3};  not-covered points {len(nc):3} missed {mm:3}")
print("\nSimilarity of wrong claims: mean sw by label:", {l: round(sum(x['sw'] for x in P if x['lab']==l)/sum(1 for x in P if x['lab']==l), 2) for l in ('covered','missing','contradicted')})
