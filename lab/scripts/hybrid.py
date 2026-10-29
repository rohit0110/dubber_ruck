"""Hybrid routing: a local judge decides; the answer goes to the hosted judge only when the local one is unsure (or flags a problem).

Local judge = logistic regression on (similarity, keyword hits, NLI key point P1/P2, numbers, negation, antonyms), fitted on DS2, scored on DS3.
Hosted judge = Haiku verdicts from out/judge_haiku_ds3/. Routing is per ANSWER (an API call judges a whole answer).
Run: lab/run.sh scripts/hybrid.py
"""
import glob, json, sys, warnings

sys.path.insert(0, "/work/bench")
sys.path.insert(0, "/work/scripts")
import numpy as np
import ekm, textfeat
from sklearn.linear_model import LogisticRegression
from sklearn.pipeline import make_pipeline
from sklearn.preprocessing import StandardScaler

warnings.filterwarnings("ignore")
CL = ["covered", "missing", "contradicted"]
COLS = ["s", "hits", "kp_has_num", "num_hit", "num_mismatch", "neg_flip", "antonym", "n1_ent", "n1_con", "n2_ent", "n2_con"]


def load(stage1, nli_path):
    pts = ekm.load(stage1)
    nli = {(r["cid"], r["kid"]): r for r in json.load(open(nli_path))["scores"]}
    rows = []
    for p in pts:
        n = nli[(p["cid"], p["kid"])]
        rows.append({"s": p["s"], "hits": min(p["hits"], 2), **textfeat.number_info(p["kp"], p["sent"]),
                     "neg_flip": textfeat.neg_flip(p["kp"], p["sent"]), "antonym": textfeat.antonym_hit(p["kp"], p["sent"]),
                     "n1_ent": n["p1"]["ent"], "n1_con": n["p1"]["con"], "n2_ent": n["p2"]["ent"], "n2_con": n["p2"]["con"]})
    return pts, np.array([[r[c] for c in COLS] for r in rows], float), np.array([CL.index(p["lab"]) for p in pts])


ds2, X2, y2 = load("/work/out/stage1_mac_ds2.json", "/work/out/nli_ds2.json")
ds3, X3, y3 = load("/work/out/stage1_mac_ds3.json", "/work/out/nli_ds3.json")
m = make_pipeline(StandardScaler(), LogisticRegression(C=1.0, max_iter=3000, class_weight="balanced")).fit(X2, y2)
proba = np.zeros((len(ds3), 3))
proba[:, m.classes_] = m.predict_proba(X3)
loc, conf = proba.argmax(1), proba.max(1)

hv = {}
for f in glob.glob("/work/out/judge_haiku_ds3/out_batch_*.json"):
    hv.update(json.load(open(f))["verdicts"])
host = np.array([CL.index(hv[p["cid"]][p["kid"]]) for p in ds3])
cids = np.array([p["cid"] for p in ds3])
answers = sorted(set(cids))


def evaluate(final):
    return ekm.metrics([(CL[a], CL[b]) for a, b in zip(y3, final)])


HEAD = "| Policy | Answers sent to the API | Exact | Flag P | Flag R | Flag F1 | Contradicted flagged / named | False 'contradicted' |\n|---|---|---|---|---|---|---|---|"
print(HEAD)
line = lambda n, sent, mm: print(f"| {n} | {sent} | {mm['exact']} | {mm['flag_p']} | {mm['flag_r']} | {mm['flag_f1']} | {mm['contradicted_flagged']} / {mm['contradicted_named']} | {mm['false_contradicted']} |")
line("local only (never calls the API)", "0%", evaluate(loc))
for tau in (0.6, 0.7, 0.8, 0.9, 0.95):
    send = {c for c in answers if conf[cids == c].min() < tau}
    final = np.where(np.isin(cids, list(send)), host, loc)
    line(f"send when any point's confidence < {tau}", f"{len(send) / len(answers):.0%}", evaluate(final))
send = {c for c in answers if (loc[cids == c] != 0).any()}
line("send when the local check flags any problem", f"{len(send) / len(answers):.0%}", evaluate(np.where(np.isin(cids, list(send)), host, loc)))
send = {c for c in answers if (loc[cids == c] != 0).any() or conf[cids == c].min() < 0.8}
line("send when it flags a problem OR any confidence < 0.8", f"{len(send) / len(answers):.0%}", evaluate(np.where(np.isin(cids, list(send)), host, loc)))
line("always send to the API", "100%", evaluate(host))
