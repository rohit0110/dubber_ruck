"""Summarize `bench.py llm` results (full-answer verdicts) with a per-variant breakdown.
Usage (from the repo root): python bench/analyze_llm.py results.json [more.json ...]   (also reads *.partial.jsonl)
"""
import json, os, sys, collections

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import ekm

cases = {c["id"]: c for c in json.load(open(os.path.join(HERE, "..", "tests", "sdi-ch4-8.transcripts.json")))["cases"]}


def load(path):
    if path.endswith(".jsonl"):
        return [json.loads(l) for l in open(path)]
    return [r for r in json.load(open(path)) if r["name"] == "raw"][0]["cases"]


def main(paths):
    for path in paths:
        per = load(path)
        pairs, by_var = [], collections.defaultdict(list)
        for r in per:
            for l, p in zip(r["labels"], r["preds"]):
                pairs.append((l, p))
                by_var[cases[r["case"]]["variant"]].append((l, p))
        m = ekm.metrics(pairs)
        n = len(per)
        print(f"\n{os.path.basename(path)}: {n} answers")
        print("  ", m)
        print(f"   time/answer {sum(r['s'] for r in per) / n:.1f} s, prompt {sum(r['prompt_tokens'] for r in per) / n:.0f} tokens "
              f"(processed {sum((r['prompt_processed'] or 0) for r in per) / n:.0f}), generated {sum(r['gen_tokens'] for r in per) / n:.1f}, "
              f"parse failures {sum(r.get('fail', 0) for r in per)}")
        conf = collections.Counter(pairs)
        print("   confusion (label -> verdict):", {f"{a[:3]}->{b[:3]}": c for (a, b), c in sorted(conf.items())})
        for v in sorted(by_var):
            cov = [(l, p) for l, p in by_var[v] if l == "covered"]
            nc = [(l, p) for l, p in by_var[v] if l != "covered"]
            print(f"   variant {v}: false flags on covered {sum(1 for l, p in cov if p != 'covered')}/{len(cov)}, "
                  f"missed on not-covered {sum(1 for l, p in nc if p == 'covered')}/{len(nc)}")


if __name__ == "__main__":
    main(sys.argv[1:])
