"""Evaluate the pipeline: EKM stage 1 (keywords + embeddings + wrong claims, LOCO thresholds), then a small model as
second opinion on routed points only. Policies are compared offline on the same model answers.
Usage (from the repo root): python bench/analyze_pipeline.py bench/results/stage1_sdi.json bench/results/b6_pipe_LFM2.json
(also accepts the *.partial.jsonl file of an unfinished run)
"""
import json, os, sys, collections

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import ekm


def load_calls(path):
    if path.endswith(".jsonl"):
        return [json.loads(l) for l in open(path)]
    return json.load(open(path))[0]["calls"]


def main(stage1, calls_path, answers=195):
    pts = ekm.loco(ekm.load(stage1))
    llm = {(c["cid"], c["kid"]): c for c in load_calls(calls_path)}
    have = [p for p in pts if (p["cid"], p["kid"]) in llm]
    print(f"{os.path.basename(calls_path)}: {len(llm)} model calls on {len(have)} routed points")
    ok = [p for p in have if llm[(p["cid"], p["kid"])]["verdict"]]
    print("  unparsable verdicts:", len(have) - len(ok))
    acc_llm = sum(1 for p in ok if llm[(p["cid"], p["kid"])]["verdict"] == p["lab"]) / len(ok)
    acc_s1 = sum(1 for p in ok if p["stage1"] == p["lab"]) / len(ok)
    print(f"  on the routed points: model alone {acc_llm:.3f} exact, stage 1 alone {acc_s1:.3f} exact")
    conf = collections.Counter((p["lab"], llm[(p["cid"], p["kid"])]["verdict"]) for p in ok)
    print("  model confusion on routed points (label -> verdict):", {f"{a[:3]}->{str(b)[:3]}": c for (a, b), c in sorted(conf.items())})
    t = [llm[(p["cid"], p["kid"])]["s"] for p in have]
    pt = [llm[(p["cid"], p["kid"])]["prompt_tokens"] for p in have]
    gt = [llm[(p["cid"], p["kid"])]["gen_tokens"] for p in have]
    print(f"  per call: {sum(t) / len(t):.1f} s, {sum(pt) / len(pt):.0f} prompt tokens, {sum(gt) / len(gt):.1f} generated;"
          f" {len(have) / answers:.2f} calls per answer, so about {sum(t) / answers:.1f} s of model time per answer")

    def verdict(p):
        v = llm.get((p["cid"], p["kid"]), {}).get("verdict")
        return v

    def apply(policy):
        out = []
        for p in pts:
            s1, v = p["stage1"], verdict(p)
            f = s1
            if v:
                if policy == "veto" and s1 != "covered" and v == "covered":
                    f = "covered"
                elif policy == "veto+escalate":
                    if s1 != "covered" and v == "covered":
                        f = "covered"
                    elif s1 == "covered" and v in ("missing", "contradicted"):
                        f = v
                elif policy == "name contradictions" and v == "contradicted":
                    f = "contradicted"
                elif policy == "model decides":
                    f = v
            out.append((p["lab"], f))
        return out

    print("\n| Policy | Exact | Flag P | Flag R | Flag F1 | Contradicted flagged / named | False 'contradicted' |")
    print("|---|---|---|---|---|---|---|")
    for name, pol in [("EKM stage 1 only", None), ("model vetoes flags", "veto"), ("model vetoes flags and escalates borderline",
                      "veto+escalate"), ("model names contradictions", "name contradictions"), ("model decides on routed points", "model decides")]:
        pairs = [(p["lab"], p["stage1"]) for p in pts] if pol is None else apply(pol)
        m = ekm.metrics(pairs)
        print(f"| {name} | {m['exact']} | {m['flag_p']} | {m['flag_r']} | {m['flag_f1']} | {m['contradicted_flagged']} / {m['contradicted_named']} | {m['false_contradicted']} |")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
