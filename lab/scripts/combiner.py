"""Learned combiner over the cheap scores (steps: numbers/negation rules, logistic regression, abstention).

One row per (answer, key point). Leave-one-chapter-out: the model never sees the chapter it predicts.
Reads bench/results/stage1_sdi.json (scores and closest sentences exported from the phone harness) and, if present,
out/nli_scores.json (from nli.py). Writes out/combiner.json and prints markdown tables.
Run inside the sandbox: lab/run.sh scripts/combiner.py
"""
import json, os, sys, collections, warnings

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
pts = ekm.load("/work/bench/results/stage1_sdi.json")
cases = {c["id"]: c for c in json.load(open("/work/tests/sdi-ch4-8.transcripts.json"))["cases"]}
nli = {}
if os.path.exists("/work/out/nli_scores.json"):
    nli = {(r["cid"], r["kid"]): r for r in json.load(open("/work/out/nli_scores.json"))["scores"]}

GROUPS = {
    "base": ["s", "hits"],
    "wrong_claims": ["sw", "gap"],
    "numbers": ["kp_has_num", "num_hit", "num_mismatch"],
    "negation": ["neg_flip"],
    "antonyms": ["antonym"],
    "nli": ["n1_ent", "n1_con", "n2_ent", "n2_con", "n3_ent"],
    "nli12": ["n1_ent", "n1_con", "n2_ent", "n2_con"],   # key point vs sentence / whole answer: no authored wrong claims involved
    "nli3": ["n3_ent"],                                    # sentence vs authored wrong claim: mirrors how the test answers were built
}


def row(p):
    ni = textfeat.number_info(p["kp"], p["sent"])
    f = {"s": p["s"], "hits": min(p["hits"], 2), "sw": p["sw"], "gap": p["sw"] - p["s"], **ni,
         "neg_flip": textfeat.neg_flip(p["kp"], p["sent"]), "antonym": textfeat.antonym_hit(p["kp"], p["sent"])}
    n = nli.get((p["cid"], p["kid"]))
    if n:
        f.update(n1_ent=n["p1"]["ent"], n1_con=n["p1"]["con"], n2_ent=n["p2"]["ent"], n2_con=n["p2"]["con"], n3_ent=n["p3"]["ent"])
    return f


rows = [row(p) for p in pts]
y = np.array([CLASSES.index(p["lab"]) for p in pts])
chs = np.array([p["ch"] for p in pts])


def loco(cols, make):
    X = np.array([[r[c] for c in cols] for r in rows], dtype=float)
    proba = np.zeros((len(pts), 3))
    for held in sorted(set(chs)):
        tr, te = chs != held, chs == held
        m = make().fit(X[tr], y[tr])
        pr = m.predict_proba(X[te])
        full = np.zeros((te.sum(), 3))
        full[:, m.classes_] = pr
        proba[te] = full
    return proba


def evaluate(proba):
    pred = proba.argmax(1)
    pairs = [(CLASSES[a], CLASSES[b]) for a, b in zip(y, pred)]
    return ekm.metrics(pairs)


def lr(C=1.0, balanced=True):
    return lambda: make_pipeline(StandardScaler(), LogisticRegression(C=C, max_iter=3000, class_weight="balanced" if balanced else None))


def gbm():
    return lambda: HistGradientBoostingClassifier(max_depth=3, max_iter=120, learning_rate=0.08, class_weight="balanced", random_state=0)


def line(name, m):
    return (f"| {name} | {m['exact']} | {m['flag_p']} | {m['flag_r']} | {m['flag_f1']} | "
            f"{m['contradicted_flagged']} / {m['contradicted_named']} | {m['false_contradicted']} |")


HEAD = ["| Model | Exact | Flag P | Flag R | Flag F1 | Contradicted flagged / named | False 'contradicted' |", "|---|---|---|---|---|---|---|"]
have_nli = bool(nli)
all_cols = [c for g, cs in GROUPS.items() if g != "nli" or have_nli for c in cs]
out = {}

print("## Reference: rule-based stage 1 (B3-04), same folds")
ref = ekm.loco(pts)
print("\n".join(HEAD + [line("EKM keyword boost + wrong_claims (rules)", ekm.metrics([(p["lab"], p["stage1"]) for p in ref]))]))

print("\n## Logistic regression, leave-one-chapter-out")
res = []
for name, cols in [("base only (similarity + keyword hits)", GROUPS["base"]),
                   ("+ wrong_claims", GROUPS["base"] + GROUPS["wrong_claims"]),
                   ("+ wrong_claims + numbers", GROUPS["base"] + GROUPS["wrong_claims"] + GROUPS["numbers"]),
                   ("+ wrong_claims + numbers + negation", GROUPS["base"] + GROUPS["wrong_claims"] + GROUPS["numbers"] + GROUPS["negation"]),
                   ("all cheap features (+ antonyms)", [c for g in ("base", "wrong_claims", "numbers", "negation", "antonyms") for c in GROUPS[g]])]:
    m = evaluate(loco(cols, lr()))
    res.append(line(name, m))
    out["lr: " + name] = m
print("\n".join(HEAD + res))

print("\n## Ablation: all cheap features minus one group")
cheap = ["base", "wrong_claims", "numbers", "negation", "antonyms"]
res = []
for drop in cheap:
    cols = [c for g in cheap if g != drop for c in GROUPS[g]]
    m = evaluate(loco(cols, lr()))
    res.append(line(f"without {drop}", m))
    out["ablation without " + drop] = m
print("\n".join(HEAD + res))

print("\n## Regularization and a non-linear model (all cheap features)")
cols = [c for g in cheap for c in GROUPS[g]]
res = []
for name, mk in [("LR C=0.1", lr(0.1)), ("LR C=1", lr(1)), ("LR C=10", lr(10)), ("LR C=1, no class weights", lr(1, False)), ("gradient boosting, depth 3", gbm())]:
    m = evaluate(loco(cols, mk))
    res.append(line(name, m))
    out["model: " + name] = m
print("\n".join(HEAD + res))

if have_nli:
    print("\n## With zero-shot NLI scores as extra features (separating what depends on the authored wrong claims)")
    nowc = GROUPS["base"] + GROUPS["numbers"] + GROUPS["negation"] + GROUPS["antonyms"]          # no wrong-claim information at all
    withwc = nowc + GROUPS["wrong_claims"]                                                          # + embedding match to wrong claims
    res = []
    for name, cs, mk in [
            ("cheap features, no wrong claims", nowc, lr()),
            ("cheap features + wrong-claim embedding match", withwc, lr()),
            ("NO wrong-claim info: base + NLI key point (P1, P2)", GROUPS["base"] + GROUPS["nli12"], lr()),
            ("NO wrong-claim info: cheap + NLI key point (P1, P2)", nowc + GROUPS["nli12"], lr()),
            ("NO wrong-claim info: cheap + NLI (P1, P2), gradient boosting", nowc + GROUPS["nli12"], gbm()),
            ("wrong-claim embedding match + NLI key point (P1, P2)", withwc + GROUPS["nli12"], lr()),
            ("everything incl. NLI sentence vs wrong claim (P3)", withwc + GROUPS["nli"], lr()),
            ("base + NLI incl. P3", GROUPS["base"] + GROUPS["nli"], lr())]:
        m = evaluate(loco(cs, mk))
        res.append(line(name, m))
        out["nli: " + name] = m
    print("\n".join(HEAD + res))

def abstention(title, cs, mk):
    print(f"\n## Abstention: say 'unsure' below a confidence bar ({title})")
    pr = loco(cs, mk)
    conf, pred = pr.max(1), pr.argmax(1)
    print("| Confidence bar | Answered | Accuracy on answered | Points that needed feedback but were called 'covered' | Flag recall (unsure counts as flagged) | Flag precision |")
    print("|---|---|---|---|---|---|")
    need, ab = y != 0, []
    for tau in (0.0, 0.5, 0.6, 0.7, 0.8, 0.9):
        ans = conf >= tau
        acc = (pred[ans] == y[ans]).mean()
        fr = int(((pred == 0) & need & ans).sum())
        flagged = (~ans) | (pred != 0)
        tp, fp = int((flagged & need).sum()), int((flagged & ~need).sum())
        print(f"| {tau:.1f} | {ans.mean():.0%} | {acc:.3f} | {fr} of {need.sum()} | {tp / max(1, need.sum()):.3f} | {tp / max(1, tp + fp):.3f} |")
        ab.append({"tau": tau, "answered": float(ans.mean()), "accuracy": float(acc), "false_reassurance": fr,
                   "flag_recall": tp / max(1, need.sum()), "flag_precision": tp / max(1, tp + fp)})
    out["abstention: " + title] = ab
    return pred


def by_variant(title, pred):
    var = np.array([p["var"] for p in pts])
    print(f"\n## Failures by answer variant ({title})")
    print("| Variant | Covered points falsely flagged | Not-covered points called covered |")
    print("|---|---|---|")
    for v in sorted(set(var)):
        cv, nc = (var == v) & (y == 0), (var == v) & (y != 0)
        print(f"| {v} | {int(((pred != 0) & cv).sum())} / {int(cv.sum())} | {int(((pred == 0) & nc).sum())} / {int(nc.sum())} |")


pred = abstention("cheap features incl. wrong-claim embedding match, no NLI", cols, lr())
by_variant("same model", pred)
if have_nli:
    nowc = GROUPS["base"] + GROUPS["numbers"] + GROUPS["negation"] + GROUPS["antonyms"] + GROUPS["nli12"]
    pred = abstention("NO wrong-claim info at all: cheap + NLI key point (P1, P2)", nowc, lr())
    by_variant("same model", pred)
    pred = abstention("everything incl. NLI vs wrong claim (P3)", [c for g in cheap + ["nli"] for c in GROUPS[g]], lr())

json.dump(out, open("/work/out/combiner.json", "w"), indent=1)
