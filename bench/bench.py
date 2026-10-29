#!/usr/bin/env python3
"""Benchmark answer checkers for the duck on labeled transcripts.

Modes:
  fuzzy    keyword hits, no model
  embed    sentence embeddings (llama-server --embeddings), cosine vs key points
  llm      small LLM returns covered/missing/contradicted per key point (JSON schema)
  feedback small LLM writes short spoken feedback (qualitative, a few cases)

Runs on the phone inside Termux (needs ~/llama.cpp/build/bin/llama-server).
"""
import argparse, json, os, re, subprocess, sys, time, urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
BIN = os.path.expanduser("~/llama.cpp/build/bin")
FILLERS = re.compile(r"\b(?:um+|uh+|uhm|erm?|hmm+|you know|i mean)\b[,.]?", re.I)
VERDICTS = ["covered", "missing", "contradicted"]


def find(name):
    for d in (HERE, os.path.join(HERE, "..", "data"), os.path.join(HERE, "..", "tests")):
        p = os.path.join(d, name)
        if os.path.exists(p):
            return p
    sys.exit(f"missing {name}")


def load(dataset):
    quiz = json.load(open(find(f"{dataset}.quiz.json")))
    cases = json.load(open(find(f"{dataset}.transcripts.json")))["cases"]
    return {q["id"]: q for q in quiz["questions"]}, cases


def read_jsonl(path):
    """Read finished records for resuming; skip a truncated last line left by a killed run."""
    out = []
    for line in open(path):
        try:
            out.append(json.loads(line))
        except ValueError:
            pass
    return out


def post(port, path, body, timeout=600):
    req = urllib.request.Request(f"http://127.0.0.1:{port}{path}", json.dumps(body).encode(),
                                 {"Content-Type": "application/json"})
    t = time.time()
    with urllib.request.urlopen(req, timeout=timeout) as r:
        data = json.load(r)
    return data, time.time() - t


class Server:
    def __init__(self, model, port, extra, threads):
        env = dict(os.environ, LD_LIBRARY_PATH=BIN)
        self.port = port
        self.log = open(os.path.expanduser(f"~/server_{port}.log"), "w")
        t = time.time()
        self.p = subprocess.Popen([f"{BIN}/llama-server", "-m", os.path.expanduser(model), "--port", str(port),
                                   "-t", str(threads), "--host", "127.0.0.1", *extra],
                                  env=env, stdout=self.log, stderr=subprocess.STDOUT)
        for _ in range(300):
            try:
                urllib.request.urlopen(f"http://127.0.0.1:{port}/health", timeout=2)
                break
            except Exception:
                if self.p.poll() is not None:
                    raise RuntimeError(f"server exited, see ~/server_{port}.log")
                time.sleep(1)
        else:
            raise RuntimeError("server start timeout")
        self.load_s = time.time() - t

    def rss_mb(self):
        try:
            for line in open(f"/proc/{self.p.pid}/status"):
                if line.startswith("VmRSS"):
                    return int(line.split()[1]) / 1024
        except OSError:
            pass

    def stop(self):
        self.p.terminate()
        try:
            self.p.wait(10)
        except subprocess.TimeoutExpired:
            self.p.kill()


def prf(rows):
    """rows: (label, pred). Positive class = point needs feedback (not covered)."""
    tp = sum(1 for l, p in rows if l != "covered" and p != "covered")
    fp = sum(1 for l, p in rows if l == "covered" and p != "covered")
    fn = sum(1 for l, p in rows if l != "covered" and p == "covered")
    pr = tp / (tp + fp) if tp + fp else 0.0
    rc = tp / (tp + fn) if tp + fn else 0.0
    f1 = 2 * pr * rc / (pr + rc) if pr + rc else 0.0
    return pr, rc, f1


def summarize(name, rows, extra=None):
    exact = sum(1 for l, p in rows if l == p) / len(rows)
    pr, rc, f1 = prf(rows)
    contra = [(l, p) for l, p in rows if l == "contradicted"]
    c_flag = sum(1 for l, p in contra if p != "covered") / len(contra) if contra else 0
    c_exact = sum(1 for l, p in contra if p == "contradicted") / len(contra) if contra else 0
    out = {"name": name, "points": len(rows), "exact_3class": round(exact, 3),
           "flag_precision": round(pr, 3), "flag_recall": round(rc, 3), "flag_f1": round(f1, 3),
           "contradicted_flagged": round(c_flag, 3), "contradicted_named": round(c_exact, 3)}
    out.update(extra or {})
    return out


# ---------------------------------------------------------------- fuzzy
def run_fuzzy(args, qs, cases):
    res = []
    for need in (1, 2):
        rows = []
        t = time.time()
        for c in cases:
            text = c["transcript"].lower()
            for kp in qs[c["question_id"]]["key_points"]:
                hits = sum(1 for k in kp["keywords"] if k.lower() in text)
                rows.append((c["labels"][kp["id"]], "covered" if hits >= need else "missing"))
        res.append(summarize(f"fuzzy keywords (>= {need} hit)", rows,
                             {"ms_per_case": round((time.time() - t) / len(cases) * 1000, 2), "tokens": 0}))
    return res


# ---------------------------------------------------------------- embed
def chunks(text):
    t = re.sub(r"\s+", " ", FILLERS.sub(" ", text)).strip()
    sents = [s.strip(" ,") for s in re.split(r"(?<=[.?!])\s+", t) if len(s.strip(" ,.?!")) > 2]
    out = list(sents) + [a + " " + b for a, b in zip(sents, sents[1:])]
    return out or [t]


def cos(a, b):
    d = sum(x * y for x, y in zip(a, b))
    return d / ((sum(x * x for x in a) ** 0.5) * (sum(y * y for y in b) ** 0.5))


def embed(port, texts):
    data, dt = post(port, "/v1/embeddings", {"input": texts, "model": "x"})
    vecs = [d["embedding"] for d in sorted(data["data"], key=lambda d: d["index"])]
    return vecs, dt, data.get("usage", {}).get("prompt_tokens", 0)


def run_embed(args, qs, cases):
    srv = Server(args.model, args.port, ["--embeddings", "-c", "2048", "-b", "512", "-ub", "512"], args.threads)
    try:
        embed(args.port, ["warm up"])
        kp_vecs, kp_time, kp_tok = {}, 0.0, 0
        for q in qs.values():
            v, dt, tok = embed(args.port, [k["text"] for k in q["key_points"]])
            kp_vecs[q["id"]] = v
            kp_time += dt
            kp_tok += tok
        # wrong claims (typical mistakes) embedded once at ingest: (key point index, vector)
        w_vecs = {}
        for q in qs.values():
            items = [(i, w) for i, k in enumerate(q["key_points"]) for w in k.get("wrong_claims", [])]
            if items:
                v, dt, tok = embed(args.port, [w for _, w in items])
                w_vecs[q["id"]] = [(i, vec) for (i, _), vec in zip(items, v)]
                kp_time += dt
                kp_tok += tok
        scored, wscored, times, toks = [], [], [], []
        for c in cases:
            ch = chunks(c["transcript"])
            v, dt, tok = embed(args.port, ch)
            times.append(dt)
            toks.append(tok)
            for i, kp in enumerate(qs[c["question_id"]]["key_points"]):
                s = max(cos(kp_vecs[c["question_id"]][i], x) for x in v)
                scored.append((c["labels"][kp["id"]], s, c["id"], kp["id"]))
                sw = max((cos(vec, x) for j, vec in w_vecs.get(c["question_id"], []) if j == i for x in v), default=0.0)
                wscored.append((c["id"], kp["id"], round(sw, 4)))
        # keyword verdict per point (at least one keyword present), for the EKM combination
        kw = {}
        for c in cases:
            low = c["transcript"].lower()
            for kp in qs[c["question_id"]]["key_points"]:
                kw[(c["id"], kp["id"])] = any(k.lower() in low for k in kp["keywords"])

        def rows_at(T, mode):
            out = []
            for l, s, c, k in scored:
                e, w = s >= T, kw[(c, k)]
                cov = {"embed": e, "keywords": w, "ekm": e and w, "ekm_both": e or w}[mode]  # ekm: flag if either flags
                out.append((l, "covered" if cov else "missing"))
            return out

        sweep = []
        for T in [x / 100 for x in range(20, 86, 2)]:
            sweep.append((prf(rows_at(T, "embed"))[2], T))
        f1, T = max(sweep)
        info = {"best_threshold": T, "ms_per_case": round(sum(times) / len(times) * 1000),
                "tokens_per_case": round(sum(toks) / len(toks), 1),
                "keypoints_precompute_ms_total": round(kp_time * 1000), "keypoint_tokens_total": kp_tok,
                "load_s": round(srv.load_s, 1), "rss_mb": round(srv.rss_mb() or 0)}
        name = os.path.basename(args.model)
        res = [summarize(f"embed {name} (best T={T}, tuned on same data)", rows_at(T, "embed"), info),
               summarize(f"EKM = keywords OR embeddings, flag if either flags (T={T}, tuned)", rows_at(T, "ekm"), info),
               summarize(f"EKM-both = flag only if both flag (T={T}, tuned)", rows_at(T, "ekm_both"), info)]
        if args.threshold is not None:
            T = args.threshold
            fixed = {**info, "best_threshold": T}
            res += [summarize(f"embed {name} (fixed T={T}, held-out)", rows_at(T, "embed"), fixed),
                    summarize(f"EKM flag if either flags (fixed T={T}, held-out)", rows_at(T, "ekm"), fixed),
                    summarize(f"EKM-both flag only if both flag (fixed T={T}, held-out)", rows_at(T, "ekm_both"), fixed)]
        # also show the score distribution by label for reading the threshold
        dist = {}
        for l, s, _, _ in scored:
            dist.setdefault(l, []).append(s)
        info2 = {l: {"n": len(v), "min": round(min(v), 2), "mean": round(sum(v) / len(v), 2), "max": round(max(v), 2)}
                 for l, v in dist.items()}
        res.append({"name": "score distribution by label", **info2})
        res.append({"name": "raw", "scored": [(l, round(s, 3), c, k) for l, s, c, k in scored], "wscored": wscored})
        return res
    finally:
        srv.stop()


# ---------------------------------------------------------------- llm
def verdict_prompt(q, transcript):
    pts = "\n".join(f"{i + 1}. {k['text']}" for i, k in enumerate(q["key_points"]))
    return ("You grade a spoken quiz answer. For each numbered key point decide: "
            "covered (the answer states it, even in different words), "
            "missing (the answer does not mention it), or "
            "contradicted (the answer states the opposite or something wrong about it). "
            "Reply with a JSON array with one word per key point, in order.\n\n"
            f"Question: {q['question']}\nKey points:\n{pts}\n\nAnswer: {transcript}")


def run_llm(args, qs, cases):
    srv = Server(args.model, args.port, ["-c", "2048"], args.threads)
    try:
        post(args.port, "/v1/chat/completions", {"messages": [{"role": "user", "content": "hi"}], "max_tokens": 1})
        rows, per, fails = [], [], 0
        part = args.out + ".partial.jsonl" if args.out else None  # one finished answer per line, for resuming
        done = {}
        if part and os.path.exists(part):
            done = {r["case"]: r for r in read_jsonl(part)}
        for c in cases:
            q = qs[c["question_id"]]
            n = len(q["key_points"])
            if c["id"] in done:
                per.append(done[c["id"]])
                fails += done[c["id"]].get("fail", 0)
                rows += list(zip(done[c["id"]]["labels"], done[c["id"]]["preds"]))
                continue
            schema = {"type": "array", "items": {"enum": VERDICTS}, "minItems": n, "maxItems": n}
            body = {"messages": [{"role": "user", "content": verdict_prompt(q, c["transcript"])}],
                    "temperature": 0, "max_tokens": 60,
                    "response_format": {"type": "json_schema", "json_schema": {"name": "v", "strict": True, "schema": schema}}}
            data, dt = post(args.port, "/v1/chat/completions", body)
            text = data["choices"][0]["message"]["content"]
            fail = 0
            try:
                preds = json.loads(text)
                assert len(preds) == n
            except Exception:
                preds, fails, fail = ["covered"] * n, fails + 1, 1
            tm = data.get("timings", {})
            rec = {"case": c["id"], "s": round(dt, 1), "prompt_tokens": data["usage"]["prompt_tokens"],
                   "prompt_processed": tm.get("prompt_n"), "cached": tm.get("cache_n"),
                   "gen_tokens": data["usage"]["completion_tokens"], "gen_tps": round(tm.get("predicted_per_second", 0), 1),
                   "preds": preds, "labels": [c["labels"][k["id"]] for k in q["key_points"]], "fail": fail}
            per.append(rec)
            if part:
                with open(part, "a") as f:
                    f.write(json.dumps(rec) + "\n")
            for k, p in zip(q["key_points"], preds):
                rows.append((c["labels"][k["id"]], p))
        avg = lambda key: round(sum((x[key] or 0) for x in per) / len(per), 1)
        info = {"load_s": round(srv.load_s, 1), "rss_mb": round(srv.rss_mb() or 0), "parse_failures": fails,
                "s_per_case": avg("s"), "prompt_tokens_per_case": avg("prompt_tokens"),
                "prompt_processed_per_case": avg("prompt_processed"), "gen_tokens_per_case": avg("gen_tokens")}
        return [summarize(f"llm {os.path.basename(args.model)}", rows, info), {"name": "raw", "cases": per}]
    finally:
        srv.stop()


def run_feedback(args, qs, cases):
    srv = Server(args.model, args.port, ["-c", "2048"], args.threads)
    try:
        post(args.port, "/v1/chat/completions", {"messages": [{"role": "user", "content": "hi"}], "max_tokens": 1})
        out = []
        for c in cases:
            if c["variant"] not in ("B", "C"):
                continue
            q = qs[c["question_id"]]
            pts = "\n".join(f"- {k['text']}" for k in q["key_points"])
            prompt = ("A student answered a quiz question aloud. In at most two short spoken sentences, say which key points "
                      "the answer missed or got wrong. If nothing is wrong, say so.\n\n"
                      f"Question: {q['question']}\nKey points:\n{pts}\n\nStudent answer: {c['transcript']}")
            data, dt = post(args.port, "/v1/chat/completions",
                            {"messages": [{"role": "user", "content": prompt}], "temperature": 0, "max_tokens": 70})
            miss = [k["text"] for k in q["key_points"] if c["labels"][k["id"]] != "covered"]
            out.append({"case": c["id"], "s": round(dt, 1), "gen_tokens": data["usage"]["completion_tokens"],
                        "truth_needs_feedback_on": miss, "feedback": data["choices"][0]["message"]["content"].strip()})
        return [{"name": f"feedback {os.path.basename(args.model)}", "cases": out}]
    finally:
        srv.stop()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("mode", choices=["fuzzy", "embed", "llm", "feedback", "stage1", "pipeline", "narrate"])
    ap.add_argument("--model")
    ap.add_argument("--port", type=int, default=8081)
    ap.add_argument("--threads", type=int, default=4)
    ap.add_argument("--out")
    ap.add_argument("--threshold", type=float, help="embed mode: also report at this fixed cosine threshold (held-out test)")
    ap.add_argument("--dataset", default="rate-limiter",
                    help="name N for N.quiz.json and N.transcripts.json (in this dir, ../data and ../tests)")
    ap.add_argument("--stage1", help="pipeline/narrate: stage1 export from `bench.py stage1`")
    ap.add_argument("--band", type=float, default=0.08, help="pipeline: also route covered points within this score margin of the threshold")
    ap.add_argument("--sample", type=int, default=16, help="narrate: number of flagged points to narrate")
    args = ap.parse_args()
    qs, cases = load(args.dataset)
    if args.mode in ("stage1", "pipeline", "narrate"):
        import pipeline
        run = getattr(pipeline, "run_" + args.mode)
    else:
        run = {"fuzzy": run_fuzzy, "embed": run_embed, "llm": run_llm, "feedback": run_feedback}[args.mode]
    res = run(args, qs, cases)
    if args.out:
        json.dump(res, open(args.out, "w"), indent=1)
    for r in res:
        if r["name"] != "raw":
            print(json.dumps(r)[:600])


if __name__ == "__main__":
    main()
