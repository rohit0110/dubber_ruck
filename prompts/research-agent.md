# Research brief: tiny on-device answer checker for a "rubber duck" quiz buddy

You are a research agent with web search. Your job is to find, and where possible verify, ways to make a very small, very slow device do one narrow language task well. Do not write code for the whole project. Produce a ranked, evidence-backed report.

## The project
A desk "duck" quizzes its owner out loud on topics they chose (for example system design), at random times inside an active window. The owner answers by voice, speech-to-text turns it into a transcript, and the duck tells them which key points of the model answer they **missed** or **got wrong**. It should also be able to converse "a tiny bit" (a short clarification or explanation). Real open-ended conversation is out of scope for the on-device model and will use an optional API key and a big model later.

Quiz content is prepared offline by strong AI agents as JSON: `question`, `answer`, `key_points[]` (each with a text and keywords) and, optionally, `wrong_claims[]` (typical mistakes). The device never needs world knowledge. It only compares a spoken answer against known key points.

## The task, stated narrowly
Input: a key point (one sentence), and a transcript of what the user said (spoken, with fillers like "um", "you know", restarts, numbers said as words like "four twenty nine").
Output, per key point: `covered`, `missing`, or `contradicted` (the user stated the opposite or something wrong about it). Then a short spoken explanation built from the key point text.
It is a pairwise entailment / short-answer-grading problem, not a chat problem.

## Hard constraints (the target device)
- Moto G (5S) (2017, codename montana), Snapdragon 430 (MSM8937): 8 x Cortex-A53 at 1.4 GHz (cpu0) and 1.09 GHz (cpu4-7), **32-bit userland (armv7l / armeabi-v7a)** even though the SoC is 64-bit capable.
- Android 8.1, running Linux tools in Termux. About 3.6 GB RAM, about 2.4 GB available. 20 GB free storage.
- No GPU/NPU usable. NEON yes. **No dotprod, no fp16 vector arithmetic, no i8mm.**
- llama.cpp builds and runs (build b1-6c44591, needed a patch for a duplicate `vcvtnq_s32_f32` with clang 21). Python 3.14 in Termux. Check the availability of any other runtime (ONNX Runtime, ncnn, MNN, TFLite, CTranslate2, sherpa-onnx) for **armv7 / Termux** explicitly before recommending it.
- Offline by default. Anything needing the internet is an optional add-on.
- Budget thinking: prefer under 150 MB of model and under 500 MB of RAM total for the checker; one answer should be checked in a few seconds, with a first useful spoken reply in under 2 seconds.

## What we measured (on the device, 4 threads)
Dataset: 20 simulated spoken answers, 68 labeled key points (30 need feedback, 5 are contradictions). "Flag" = point reported as missing or wrong.

| Checker | Time/answer | RAM | Tokens | Flag P / R / F1 |
|---|---|---|---|---|
| Keywords, >= 1 hit | under 1 ms | none | 0 | 0.93 / 0.87 / 0.90 |
| all-MiniLM-L6-v2 GGUF Q4_0, max cosine of key point vs transcript sentences | 0.5 s | 38 MB | 128 in | 0.84 / 0.87 / 0.85 |
| Keywords OR embeddings (flag if either flags) | 0.5 s | 38 MB | 128 in | 0.83 / 0.97 / 0.89 |
| LFM2-350M Q4_0, JSON-schema verdicts, whole answer in one prompt | 21 s | 286 MB | 205 in / 25 out | 0.72 / 0.77 / 0.74 |
| Qwen2.5-0.5B Q4, same | 16 s | 556 MB | 219 in / 17 out | 0.57 / 0.57 / 0.57 |
| Gemma 3 270M Q4_0, same | 13 s | 340 MB | 206 in / 31 out | 0.45 / 1.00 / 0.62 (flags almost everything) |

Other findings: prompt processing is about 9 tokens/s and generation about 5.6 tokens/s for a 0.5B Q4 model on this phone. Naming a contradiction correctly worked in at most 1 of 5 cases for every checker. Embeddings score "returns 403" as close to "returns 429". Free-text feedback from the 0.5B models took 27-32 s for about 70 tokens, called flawed answers "correct", and invented content. Caveats: tiny dataset, the keyword list was authored by the same person who wrote the transcripts, and thresholds were tuned on the same data.

## What to research, in priority order
1. **Contradiction / entailment on a budget.** Small NLI or cross-encoder models (MiniLM / DeBERTa-xsmall / TinyBERT / distilled variants, rerankers) that could run on this phone. Are there GGUF versions llama.cpp can load (classification or reranking heads)? What are their sizes, licenses and measured accuracy on MNLI/SNLI-style data? What breaks under int8/Q4 quantization?
2. **Fine-tuning a sub-500M model for exactly this task** (candidates: Gemma 3 270M, SmolLM2 135M/360M, LFM2-350M, Qwen3-0.6B, others you find). Best recipes for synthetic training data from a strong LLM, LoRA vs full fine-tune, how many examples are needed, how to export to GGUF and quantize, expected accuracy gains over zero-shot. Include a laptop-friendly training route (Apple Silicon Mac).
3. **Better or smaller embedding models** than all-MiniLM-L6-v2 for short-answer matching: bge-small, gte-small, e5-small, EmbeddingGemma, static-embedding approaches such as Model2Vec / potion (no transformer at runtime). Any evidence on negation and numeric sensitivity? Any trick that makes embeddings notice "403 vs 429"?
4. **Cheap non-neural contradiction signals:** matching against authored `wrong_claims`, number and entity mismatch, negation detection. What does the literature say about combining them with embeddings?
5. **Inference efficiency on armv7 with NEON only:** best llama.cpp quant types and flags for this CPU (Q4_0 vs Q4_K vs Q8_0, repacking, thread count 4 vs 8, prompt caching, constrained decoding, speculative decoding), and non-llama.cpp runtimes that genuinely support armv7 (verify, do not assume).
6. **Pipeline design:** if cheap checkers (keywords + embeddings) run first and only the mismatched points plus the closest transcript sentence are given to a small model, what should that model's job be (judge vs narrator that rephrases the known-correct key point)? Find evidence on hallucination rates for sub-1B models in each role.
7. **Speech-to-text on armv7 (secondary):** Vosk, whisper.cpp tiny/base, sherpa-onnx, pocketsphinx, Android's offline recognizer. Which actually run on 32-bit ARM in Termux and how do they handle spoken numbers and technical terms?
8. **Evaluation:** public datasets for short-answer grading or entailment we can use to sanity-check beyond our own data (for example SemEval-2013 Task 7, ASAP-SAS, Mohler, SNLI/MNLI/ANLI) and how to build a fair benchmark from them.
9. **Optional free API tier** for the hard cases (current free limits, data-use terms, alternatives to Gemini). Keep this short.
10. **Hardware (low priority):** cheap boards or phones that would remove the 32-bit limitation, only if it changes the answer above.

## How to report
- Start with a TL;DR: the top 3 recommended next experiments, each with expected gain, cost and risk.
- Then a table of candidates: name, parameters, file size, format, license, **armv7 / Termux viability (confirmed, likely, unknown)**, evidence, link.
- For each recommended experiment give exact commands or steps we can run on the phone or a Mac.
- Separate **confirmed** facts (cite the source) from **inferences** and from **things you could not verify**. Never invent benchmark numbers. If a source gives numbers for different hardware, say so.
- Say explicitly what you would NOT bother with, and why.
- Do not recommend anything needing a GPU, more than 1.5 GB of RAM, or a 64-bit-only runtime unless you also say how to get it running on armv7.
- Cite every URL you used. Prefer primary sources (model cards, papers, repos, release notes) over blog summaries, and note the date of anything that may be stale.
