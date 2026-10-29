# Research on alternatives: paraphrase-robust answer checking without a generative LLM

## Context
The duck must tell, per key point, covered / missing / contradicted from a spoken, paraphrased transcript on a Moto G5 Plus (armv7, NEON only, ~2.4 GB RAM). Measured so far (bench/): keywords F1 0.90, MiniLM embeddings 0.85, keywords OR embeddings 0.89 (recall 0.97), best generative LLM (LFM2-350M) 0.74 at 21 s per answer. Contradiction naming works at most 1 of 5 for every checker. Question: is an LLM the right tool, and what handles paraphrase?

## Answer
**No. A generative LLM is the wrong tool for the judging step.** The task is pairwise entailment, which is a classification problem. The evidence in hand:
- Your own data: every generative model is slower and worse than a 38 MB embedding model.
- The short-answer-grading literature (SemEval-2013 Task 7, 5-way/3-way entailment labels) reports its strongest gains from fine-tuned RoBERTa-class encoders, with one study claiming up to 13% absolute macro-F1 over earlier state of the art, and says frozen embeddings alone score below average on the 5-way task. A 2026 LLM-grading paper finds LLM errors cluster on "partially correct" answers. Sources: arxiv.org/abs/2204.03503 (survey), the SemEval-2013 Task 7 overview, arxiv.org/html/2605.00238v1. I read search snippets only, not the papers, so verify the 13% figure before quoting it.
- SetFit (arxiv.org/abs/2209.11055) shows a small fine-tuned sentence transformer can beat a 175B prompted model on few-shot classification. Not tested on-device.
- Embedding similarity cannot separate opposites ("cancel" vs "keep"); practitioner sources say to add an explicit contradiction check. Optimal similarity thresholds vary widely by task (0.33-0.87 in one study), so tune per key point or learn them.

Role for a small LLM, if any: **narrator only** (rephrase the known key point into a spoken sentence), never judge. No hallucination-rate evidence found for sub-1B models in either role; this is reasoning.

## What I could not find
No paper that runs NLI or short-answer grading on Snapdragon 625-class or armv7 hardware. Closest: MobileBERT quantized on a Raspberry Pi (arxiv.org/abs/2310.03971, about 1 tweet/s, 4.1% accuracy drop, a different task) and BERT/DistilBERT/TinyBERT on embedded boards (arxiv.org/abs/2304.11520, intent/NER, notes distilled models can underperform off the shelf). On-device latency for our case has to be measured, not cited.

## Recommended architecture (no generation in the judging path)
1. Number/negation normalisation of the transcript (words to digits, "not"/"n't"). Rules, ~0 ms.
2. Keyword hits plus embedding max-cosine (existing; recall 0.97 when OR-ed).
3. Fine-tuned small cross-encoder (NLI-MiniLM2-L6-H768, 82M, Apache-2.0) on (best transcript sentence, key point) for anything not clearly covered. This gives the contradiction label and handles paraphrase better than cosine.
4. Number/entity mismatch and `wrong_claims` matching as a veto layer for contradictions.
5. Spoken feedback from templates built from the key point text; optional tiny LLM narrator or API for "converse a bit".

## Experiments, in order (all reuse existing code)
1. **Zero-shot NLI stage.** Add a mode to `bench/pipeline.py` next to `run_stage1` / the judge prompt: score (`sent`, `kp`) with the cross-encoder, using the same stage-1 export (`kp`, `sent`, `wclaim`, `wsent`, labels). Compare P/R/F1 and contradiction accuracy against `bench/results/` baselines via `analyze_pipeline.py`.
2. **Rule layer.** Add number-word normalisation and numeric/negation mismatch to `bench/ekm.py`; re-score the 5 contradiction cases (403 vs 429 type).
3. **Fine-tune the cross-encoder** on LLM-generated (key point, spoken paraphrase, label) triples, held out by quiz file (`analyze_loco.py` already does leave-one-chapter-out), on the Mac. Include fillers, restarts, number words, paraphrase-only positives, and `wrong_claims` negatives.
4. **Phone latency.** Convert to GGUF Q8_0 (llama.cpp converter supports Roberta classifiers; whether 3-way logits are exposed via CLI/server is unconfirmed) or run int8 ONNX via Termux `onnxruntime`; time 100 pairs at 4 and 8 threads.
5. **External sanity check** on SemEval-2013 Task 7 (HF mirrors nkazi/Beetle, nkazi/SciEntsBank) so thresholds are not tuned only on our own transcripts. Larger and independent of the author's keyword lists, addressing the biggest caveat in the current benchmark.

## Verification
- Success criteria: flag F1 above 0.90 at under 2 s per answer, and at least 3 of 5 contradictions correctly named, on held-out chapters.
- Confirm llama.cpp can output NLI logits for the converted model (or fall back to ONNX).
- Report RAM and time on the device, not extrapolated.

## Not recommended
Bigger generative judges, EmbeddingGemma (303M, gated), DeBERTa via llama.cpp (unsupported), further prompt tuning of 0.3-0.5B models.
