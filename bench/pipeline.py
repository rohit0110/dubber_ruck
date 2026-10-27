"""Pipeline modes for bench.py: stage1 (export EKM scores + closest sentences), pipeline (small model as
second opinion on routed points only), narrate (small model writes one spoken sentence for a flagged point)."""
import json, os, re, sys, time

import bench as B
import ekm

VERDICTS = ["covered", "missing", "contradicted"]


# ---------------------------------------------------------------- stage 1
def run_stage1(args, qs, cases):
    srv = B.Server(args.model, args.port, ["--embeddings", "-c", "2048", "-b", "512", "-ub", "512"], args.threads)
    try:
        B.embed(args.port, ["warm up"])
        kp_vecs, w_vecs = {}, {}
        t0 = time.time()
        for q in qs.values():
            kp_vecs[q["id"]] = B.embed(args.port, [k["text"] for k in q["key_points"]])[0]
            items = [(i, w) for i, k in enumerate(q["key_points"]) for w in k.get("wrong_claims", [])]
            if items:
                vs = B.embed(args.port, [w for _, w in items])[0]
                w_vecs[q["id"]] = [(i, w, v) for (i, w), v in zip(items, vs)]
        ingest_s = time.time() - t0
        points, times = [], []
        for c in cases:
            q = qs[c["question_id"]]
            ch = B.chunks(c["transcript"])
            v, dt, _ = B.embed(args.port, ch)
            times.append(dt)
            low = c["transcript"].lower()
            for i, kp in enumerate(q["key_points"]):
                sims = [B.cos(kp_vecs[q["id"]][i], x) for x in v]
                bi = max(range(len(sims)), key=sims.__getitem__)
                bw = (0.0, "", "")
                for j, w, vec in w_vecs.get(q["id"], []):
                    if j != i:
                        continue
                    ws = [B.cos(vec, x) for x in v]
                    wi = max(range(len(ws)), key=ws.__getitem__)
                    if ws[wi] > bw[0]:
                        bw = (ws[wi], w, ch[wi])
                points.append({"cid": c["id"], "kid": kp["id"], "qid": q["id"], "ch": c["id"].split("q")[0] or "all",
                               "var": c["variant"], "lab": c["labels"][kp["id"]], "question": q["question"], "kp": kp["text"],
                               "s": round(sims[bi], 4), "sent": ch[bi], "sw": round(bw[0], 4), "wclaim": bw[1], "wsent": bw[2],
                               "hits": sum(1 for k in kp["keywords"] if k.lower() in low)})
        meta = {"model": os.path.basename(args.model), "ms_per_answer": round(sum(times) / len(times) * 1000),
                "ingest_s": round(ingest_s, 1), "rss_mb": round(srv.rss_mb() or 0)}
        return [{"name": "stage1", "meta": meta, "points": points}]
    finally:
        srv.stop()


# ---------------------------------------------------------------- pipeline
def judge_prompt(p):
    return ("You check one spoken answer against one key fact.\n"
            f"Question: {p['question']}\nKey fact: {p['kp']}\n"
            + (f"A common mistake: {p['wclaim']}\n" if p["wclaim"] else "")
            + f"What the speaker said (closest sentence): {p['sent']}\n"
            "Does the speaker state the key fact (covered), state something that contradicts it (contradicted), "
            "or not mention it (missing)? Reply with one word.")


def routed_points(args):
    pts = ekm.loco(ekm.load(args.stage1))
    return pts, [p for p in pts if p["stage1"] != "covered" or p["margin"] < args.band]


def run_pipeline(args, qs, cases):
    pts, routed = routed_points(args)
    part = args.out + ".partial.jsonl"
    done = {}
    if os.path.exists(part):
        done = {(r["cid"], r["kid"]): r for r in map(json.loads, open(part))}
    srv = B.Server(args.model, args.port, ["-c", "2048"], args.threads)
    try:
        B.post(args.port, "/v1/chat/completions", {"messages": [{"role": "user", "content": "hi"}], "max_tokens": 1})
        out, fails = [], 0
        for p in routed:
            key = (p["cid"], p["kid"])
            if key in done:
                out.append(done[key])
                continue
            body = {"messages": [{"role": "user", "content": judge_prompt(p)}], "temperature": 0, "max_tokens": 12,
                    "response_format": {"type": "json_schema", "json_schema": {"name": "v", "strict": True, "schema": {"enum": VERDICTS}}}}
            data, dt = B.post(args.port, "/v1/chat/completions", body)
            try:
                verdict = json.loads(data["choices"][0]["message"]["content"])
                assert verdict in VERDICTS
            except Exception:
                verdict = None
            tm = data.get("timings", {})
            rec = {"cid": p["cid"], "kid": p["kid"], "verdict": verdict, "s": round(dt, 2),
                   "prompt_tokens": data["usage"]["prompt_tokens"], "prompt_processed": tm.get("prompt_n"),
                   "gen_tokens": data["usage"]["completion_tokens"]}
            out.append(rec)
            with open(part, "a") as f:
                f.write(json.dumps(rec) + "\n")
        meta = {"model": os.path.basename(args.model), "band": args.band, "routed": len(routed), "points": len(pts),
                "load_s": round(srv.load_s, 1), "rss_mb": round(srv.rss_mb() or 0)}
        return [{"name": "pipeline", "meta": meta, "calls": out}]
    finally:
        srv.stop()


# ---------------------------------------------------------------- narrator
def narrate_prompt(p):
    kind = ("The speaker said something that contradicts it." if p["stage1"] == "contradicted"
            else "The speaker did not mention it.")
    return ("You are a friendly study buddy speaking out loud. Tell the speaker, in one short sentence, "
            "what the correct fact is. Use only the key fact below; do not add anything else.\n"
            f"Question: {p['question']}\nKey fact: {p['kp']}\n{kind}")


def run_narrate(args, qs, cases):
    pts, _ = routed_points(args)
    flagged = [p for p in pts if p["stage1"] != "covered"]
    sample = ([p for p in flagged if p["lab"] == "contradicted"][::max(1, args.sample // 2)][:args.sample // 2]
              + [p for p in flagged if p["lab"] == "missing"][::7][:args.sample // 2])
    srv = B.Server(args.model, args.port, ["-c", "2048"], args.threads)
    try:
        B.post(args.port, "/v1/chat/completions", {"messages": [{"role": "user", "content": "hi"}], "max_tokens": 1})
        out = []
        for p in sample:
            data, dt = B.post(args.port, "/v1/chat/completions",
                              {"messages": [{"role": "user", "content": narrate_prompt(p)}], "temperature": 0, "max_tokens": 45})
            text = data["choices"][0]["message"]["content"].strip()
            out.append({"cid": p["cid"], "kid": p["kid"], "label": p["lab"], "stage1": p["stage1"], "kp": p["kp"],
                        "said": p["sent"], "narration": text, "s": round(dt, 1),
                        "prompt_tokens": data["usage"]["prompt_tokens"], "gen_tokens": data["usage"]["completion_tokens"]})
        return [{"name": "narrate", "meta": {"model": os.path.basename(args.model)}, "samples": out}]
    finally:
        srv.stop()
