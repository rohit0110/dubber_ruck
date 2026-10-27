"""EKM stage 1: threshold tuning and prediction from exported scores (keywords + embeddings + wrong claims).

Points come from `bench.py stage1` (one dict per key point per answer). Thresholds are tuned
leave-one-chapter-out (LOCO): never on the chapter they are applied to.
"""
import json

TS = [t / 100 for t in range(30, 82, 2)]            # score threshold
AS = [0, .05, .1, .15, .2, .3]                      # keyword boost per hit (max 2 hits count)
TWS = [t / 100 for t in range(40, 95, 5)]           # wrong-claim similarity threshold
MS = [-.1, -.05, 0, .05, .1, .15, .2]               # wrong claim must beat the key point by this margin


def load(path):
    data = json.load(open(path))
    return (data[0] if isinstance(data, list) else data)["points"]


def f1(tp, fp, fn):
    p = tp / (tp + fp) if tp + fp else 0
    r = tp / (tp + fn) if tp + fn else 0
    return (2 * p * r / (p + r) if p + r else 0), p, r


def score(x, a):
    return x["s"] + a * min(x["hits"], 2)


def flag_f1(rows, T, a):
    tp = fp = fn = 0
    for x in rows:
        cov, pos = score(x, a) >= T, x["lab"] != "covered"
        tp += pos and not cov
        fp += (not pos) and not cov
        fn += pos and cov
    return f1(tp, fp, fn)


def x_f1(rows, Tw, m):
    tp = fp = fn = 0
    for x in rows:
        pred, pos = x["sw"] >= Tw and x["sw"] - x["s"] >= m, x["lab"] == "contradicted"
        tp += pos and pred
        fp += (not pos) and pred
        fn += pos and not pred
    return f1(tp, fp, fn)


def tune(rows):
    T, a = max(((flag_f1(rows, T, a)[0], (T, a)) for T in TS for a in AS))[1]
    Tw, m = max(((x_f1(rows, tw, m)[0], (tw, m)) for tw in TWS for m in MS))[1]
    return T, a, Tw, m


def loco(points):
    """Stage-1 verdicts with thresholds tuned on the other chapters. Adds stage1, margin, T, a, Tw, m."""
    out = []
    for held in sorted({p["ch"] for p in points}):
        T, a, Tw, m = tune([p for p in points if p["ch"] != held])
        for x in points:
            if x["ch"] != held:
                continue
            sc = score(x, a)
            xc = x["sw"] >= Tw and x["sw"] - x["s"] >= m
            out.append(dict(x, T=T, a=a, Tw=Tw, m=m, margin=round(sc - T, 4),
                            stage1="contradicted" if xc else ("covered" if sc >= T else "missing")))
    return out


def metrics(pairs):
    """pairs: (label, verdict). Returns exact, flag precision/recall/F1, contradiction stats."""
    n = len(pairs)
    exact = sum(1 for l, v in pairs if l == v) / n
    tp = sum(1 for l, v in pairs if l != "covered" and v != "covered")
    fp = sum(1 for l, v in pairs if l == "covered" and v != "covered")
    fn = sum(1 for l, v in pairs if l != "covered" and v == "covered")
    f, p, r = f1(tp, fp, fn)
    con = [(l, v) for l, v in pairs if l == "contradicted"]
    return {"points": n, "exact": round(exact, 3), "flag_p": round(p, 3), "flag_r": round(r, 3), "flag_f1": round(f, 3),
            "contradicted_flagged": f"{sum(1 for l, v in con if v != 'covered')}/{len(con)}",
            "contradicted_named": f"{sum(1 for l, v in con if v == 'contradicted')}/{len(con)}",
            "false_contradicted": sum(1 for l, v in pairs if l != "contradicted" and v == "contradicted")}
