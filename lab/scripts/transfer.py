"""Transfer test: everything is fitted on DS2 (author-written data) and scored on DS3 (independent mistakes written by another agent).

Inputs (all produced in the sandbox):
  out/stage1_mac_ds2.json, out/stage1_mac_ds3.json   from stage1_mac.py
  out/nli_ds2.json, out/nli_ds3.json                 from nli.py
Writes out/transfer.json and prints markdown tables. Run: lab/run.sh scripts/transfer.py
"""
import collections, json, sys, warnings

sys.path.insert(0, "/work/bench")
sys.path.insert(0, "/work/scripts")
import numpy as np
import ekm, textfeat
from sklearn.ensemble import HistGradientBoostingClassifier
from sklearn.linear_model import LogisticRegression
from sklearn.pipeline import make_pipeline
from sklearn.preprocessing import StandardScaler

warnings.filterwarnings("ignore")
CLASSES = ["covered", "missing", "contradicted"]


def load(stage1, nli_path):
    pts = ekm.load(stage1)
    nli = {(r["cid"], r["kid"]): r for r in json.load(open(nli_path))["scores"]}
    return pts, nli


ds2, nli2 = load("/work/out/stage1_mac_ds2.json", "/work/out/nli_ds2.json")
ds3, nli3 = load("/work/out/stage1_mac_ds3.json", "/work/out/nli_ds3.json")
G = {"base": ["s", "hits"], "wc": ["sw", "gap"], "num": ["kp_has_num", "num_hit", "num_mismatch"], "neg": ["neg_flip"], "ant": ["antonym"],
     "nli12": ["n1_ent", "n1_con", "n2_ent", "n2_con"], "nli3": ["n3_ent"]}
CHEAP = G["base"] + G["num"] + G["neg"] + G["ant"]          # no wrong-claim information at all


def row(p, nli):
    n = nli[(p["cid"], p["kid"])]
    return {"s": p["s"], "hits": min(p["hits"], 2), "sw": p["sw"], "gap": p["sw"] - p["s"], **textfeat.number_info(p["kp"], p["sent"]),
            "neg_flip": textfeat.neg_flip(p["kp"], p["sent"]), "antonym": textfeat.antonym_hit(p["kp"], p["sent"]),
            "n1_ent": n["p1"]["ent"], "n1_con": n["p1"]["con"], "n2_ent": n["p2"]["ent"], "n2_con": n["p2"]["con"], "n3_ent": n["p3"]["ent"]}


R2, R3 = [row(p, nli2) for p in ds2], [row(p, nli3) for p in ds3]
y2, y3 = np.array([CLASSES.index(p["lab"]) for p in ds2]), np.array([CLASSES.index(p["lab"]) for p in ds3])
X = lambda R, cols: np.array([[r[c] for c in cols] for r in R], dtype=float)


def lr(balanced=True, C=1.0):
    return lambda: make_pipeline(StandardScaler(), LogisticRegression(C=C, max_iter=3000, class_weight="balanced" if balanced else None))


def gbm():
    return lambda: HistGradientBoostingClassifier(max_depth=3, max_iter=120, learning_rate=0.08, class_weight="balanced", random_state=0)


def fit_predict(cols, make, Rtr, ytr, Rte):
    m = make().fit(X(Rtr, cols), ytr)
    out = np.zeros((len(Rte), 3))
    out[:, m.classes_] = m.predict_proba(X(Rte, cols))
    return out


def pairs(y, pred):
    return [(CLASSES[a], CLASSES[b]) for a, b in zip(y, pred)]


HEAD = ["| Model | Exact | Flag P | Flag R | Flag F1 | Contradicted flagged / named | False 'contradicted' |", "|---|---|---|---|---|---|---|"]
line = lambda n, m: f"| {n} | {m['exact']} | {m['flag_p']} | {m['flag_r']} | {m['flag_f1']} | {m['contradicted_flagged']} / {m['contradicted_named']} | {m['false_contradicted']} |"
out, preds = {}, {}


def report(name, pred):
    m = ekm.metrics(pairs(y3, pred))
    out[name] = m
    preds[name] = pred
    return line(name, m)


print(f"DS2: {len(ds2)} points ({collections.Counter(p['lab'] for p in ds2)}); DS3: {len(ds3)} points ({collections.Counter(p['lab'] for p in ds3)})")
rows = []
rows.append(report("keywords (at least 1 hit)", np.array([0 if p["hits"] >= 1 else 1 for p in ds3])))
T, a, Tw, m_ = ekm.tune(ds2)
print(f"\nThresholds tuned on DS2 only: T={T}, keyword boost={a}, wrong-claim T={Tw}, margin={m_}")


def rules(pts):
    out_ = []
    for p in pts:
        sc = ekm.score(p, a)
        xc = p["sw"] >= Tw and p["sw"] - p["s"] >= m_
        out_.append(2 if xc else (0 if sc >= T else 1))
    return np.array(out_)


rows.append(report("EKM rules: keyword boost + wrong claims (B3-04 method, tuned on DS2)", rules(ds3)))
ds2_rules = ekm.metrics(pairs(y2, rules(ds2)))
for name, cols, mk in [
        ("LR, no wrong-claim info", CHEAP, lr()),
        ("LR, + wrong-claim embedding match", CHEAP + G["wc"], lr()),
        ("LR, no wrong-claim info + NLI key point (P1, P2)", CHEAP + G["nli12"], lr()),
        ("LR, + wrong-claim match + NLI key point (P1, P2)", CHEAP + G["wc"] + G["nli12"], lr()),
        ("LR, everything incl. NLI vs wrong claim (P3)", CHEAP + G["wc"] + G["nli12"] + G["nli3"], lr()),
        ("GBM, no wrong-claim info + NLI (P1, P2)", CHEAP + G["nli12"], gbm()),
        ("GBM, + wrong-claim match + NLI (P1, P2)", CHEAP + G["wc"] + G["nli12"], gbm()),
        ("GBM, everything incl. P3", CHEAP + G["wc"] + G["nli12"] + G["nli3"], gbm()),
        ("LR, no class weights, + wrong claim + NLI (P1, P2)", CHEAP + G["wc"] + G["nli12"], lr(False))]:
    pr = fit_predict(cols, mk, R2, y2, R3)
    rows.append(report(name, pr.argmax(1)))
    preds[name + "|proba"] = pr
VERD = {"ent": 0, "con": 2, "neu": 1}
for nm, key in [("zero-shot NLI alone, sentence premise (P1)", "p1"), ("zero-shot NLI alone, whole-answer premise (P2)", "p2")]:
    rows.append(report(nm, np.array([VERD[max(("ent", "con", "neu"), key=nli3[(p["cid"], p["kid"])][key].get)] for p in ds3])))
print("\n## Train on DS2, test on DS3 (independent mistakes written by another agent)")
print("\n".join(HEAD + rows))
print(f"\nFor reference, the same rules scored on DS2 itself (in-sample): exact {ds2_rules['exact']}, F1 {ds2_rules['flag_f1']}, named {ds2_rules['contradicted_named']}")

# ---- how well do the authored wrong claims cover real mistakes?
print("\n## Do the authored wrong claims cover the independent mistakes?")
con2 = np.array([p["sw"] for p in ds2 if p["lab"] == "contradicted"])
con3 = np.array([p["sw"] for p in ds3 if p["lab"] == "contradicted"])
oth3 = np.array([p["sw"] for p in ds3 if p["lab"] != "contradicted"])
p3_con3 = np.array([nli3[(p["cid"], p["kid"])]["p3"]["ent"] for p in ds3 if p["lab"] == "contradicted"])
p3_con2 = np.array([nli2[(p["cid"], p["kid"])]["p3"]["ent"] for p in ds2 if p["lab"] == "contradicted"])
print("| Contradicted points | n | mean similarity to nearest authored wrong claim | share with similarity >= 0.75 | share where NLI says the sentence entails a wrong claim (>= 0.5) |")
print("|---|---|---|---|---|")
print(f"| DS2 (built from the wrong claims) | {len(con2)} | {con2.mean():.2f} | {(con2 >= .75).mean():.0%} | {(p3_con2 >= .5).mean():.0%} |")
print(f"| DS3 (independent) | {len(con3)} | {con3.mean():.2f} | {(con3 >= .75).mean():.0%} | {(p3_con3 >= .5).mean():.0%} |")
print(f"| DS3 other points (covered/missing) | {len(oth3)} | {oth3.mean():.2f} | {(oth3 >= .75).mean():.0%} | - |")

# ---- by mistake type
cmp_models = ["EKM rules: keyword boost + wrong claims (B3-04 method, tuned on DS2)", "LR, no wrong-claim info + NLI key point (P1, P2)",
              "LR, + wrong-claim match + NLI key point (P1, P2)", "LR, everything incl. NLI vs wrong claim (P3)", "GBM, no wrong-claim info + NLI (P1, P2)"]
short = ["EKM rules", "LR no-WC + NLI", "LR + WC + NLI", "LR everything (P3)", "GBM no-WC + NLI"]
bytype = collections.defaultdict(list)
for i, p in enumerate(ds3):
    if p["lab"] == "contradicted":
        bytype[p["mtype"]].append(i)
print("\n## Contradicted points: flagged as a problem / named 'contradicted', by mistake type (independent data)")
print("| Mistake type | n | " + " | ".join(short) + " |")
print("|---|---|" + "---|" * len(short))
for t, idx in sorted(bytype.items(), key=lambda kv: -len(kv[1])):
    cells = [f"{sum(1 for i in idx if preds[m][i] != 0)} / {sum(1 for i in idx if preds[m][i] == 2)}" for m in cmp_models]
    print(f"| {t} | {len(idx)} | " + " | ".join(cells) + " |")
tot = [f"{sum(1 for i in sum(bytype.values(), []) if preds[m][i] != 0)} / {sum(1 for i in sum(bytype.values(), []) if preds[m][i] == 2)}" for m in cmp_models]
print(f"| all | {sum(len(v) for v in bytype.values())} | " + " | ".join(tot) + " |")

# ---- by kind
best = "LR, no wrong-claim info + NLI key point (P1, P2)"
pr_best = preds[best]
print(f"\n## Failures by kind of answer ({best})")
print("| Kind | Covered points falsely flagged | Not-covered points called covered | Contradicted points named |")
print("|---|---|---|---|")
kinds = np.array([p["var"] for p in ds3])
for k in sorted(set(kinds)):
    cv, nc, cn = (kinds == k) & (y3 == 0), (kinds == k) & (y3 != 0), (kinds == k) & (y3 == 2)
    print(f"| {k} | {int(((pr_best != 0) & cv).sum())} / {int(cv.sum())} | {int(((pr_best == 0) & nc).sum())} / {int(nc.sum())} | "
          f"{int(((pr_best == 2) & cn).sum())} / {int(cn.sum())} |")

# ---- abstention with the DS2-trained model
print("\n## Abstention on DS3 (confidence bar applied to the model trained on DS2)")
for name in ["LR, no wrong-claim info + NLI key point (P1, P2)", "LR, + wrong-claim match + NLI key point (P1, P2)"]:
    pr = preds[name + "|proba"]
    conf, pred = pr.max(1), pr.argmax(1)
    need = y3 != 0
    print(f"\n{name}")
    print("| Bar | Answered | Accuracy on answered | Needed-feedback points still called 'covered' | Flag recall (unsure = flagged) | Flag precision |")
    print("|---|---|---|---|---|---|")
    for tau in (0.0, 0.6, 0.7, 0.8, 0.9):
        ans = conf >= tau
        flagged = (~ans) | (pred != 0)
        tp, fp = int((flagged & need).sum()), int((flagged & ~need).sum())
        print(f"| {tau:.1f} | {ans.mean():.0%} | {(pred[ans] == y3[ans]).mean():.3f} | {int(((pred == 0) & need & ans).sum())} of {int(need.sum())} | "
              f"{tp / max(1, need.sum()):.3f} | {tp / max(1, tp + fp):.3f} |")

# ---- does adding independent data to training help? (leave-one-chapter-out)
print("\n## Adding some independent data to the training set (leave-one-chapter-out, scored on DS3 of the held-out chapter)")
ch2, ch3 = np.array([p["ch"] for p in ds2]), np.array([p["ch"] for p in ds3])
cols = CHEAP + G["wc"] + G["nli12"]
res = {"train on DS2 only": np.zeros(len(ds3), int), "train on DS2 + DS3 (other chapters)": np.zeros(len(ds3), int)}
for held in sorted(set(ch3)):
    tr2, te3, tr3 = ch2 != held, ch3 == held, ch3 != held
    R_a = [r for r, k in zip(R2, tr2) if k]
    p1 = fit_predict(cols, lr(), R_a, y2[tr2], [r for r, k in zip(R3, te3) if k])
    R_b = R_a + [r for r, k in zip(R3, tr3) if k]
    y_b = np.concatenate([y2[tr2], y3[tr3]])
    p2 = fit_predict(cols, lr(), R_b, y_b, [r for r, k in zip(R3, te3) if k])
    res["train on DS2 only"][te3], res["train on DS2 + DS3 (other chapters)"][te3] = p1.argmax(1), p2.argmax(1)
print("\n".join(HEAD + [line(n, ekm.metrics(pairs(y3, v))) for n, v in res.items()]))
json.dump(out, open("/work/out/transfer.json", "w"), indent=1)
