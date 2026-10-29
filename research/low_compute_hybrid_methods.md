# Low-compute hybrid methods: math + small models for answer checking

Date: 2026-10-29. Sources are mostly abstracts and search snippets, not full papers; numbers are the authors' own claims unless stated. Compute costs in the ladder are order-of-magnitude reasoning, not measurements. Companion files: `answer-checker-report.md`, `research_on_alternatives.md`.

## Design principle

Treat the small model as one component in a pipeline of cheap, verifiable math, not as the judge. Each stage has a narrow job, a stated cost and a confidence score. Whatever a rung leaves undecided moves to the next, costlier rung. Your own benchmark already points this way: keywords (F1 0.90) and a 38 MB embedding model (0.85) beat every generative LLM (0.57-0.74).

## Short answer: is an LLM the right judge?

No. The task is pairwise entailment (classification). A generative LLM is at most a narrator that rephrases the known key point. No hallucination-rate evidence for sub-1B models in either role was found; this is reasoning.

- Short-answer grading on SemEval-2013 Task 7: fine-tuned RoBERTa-class encoders gave the strongest gains (one study claims up to 13% absolute macro-F1; verify before quoting). Frozen embeddings alone scored below average on the 5-way task. ([survey](https://arxiv.org/abs/2204.03503), [2026 LLM-grading paper](https://arxiv.org/html/2605.00238v1))
- Embedding similarity cannot separate opposites ("cancel" vs "keep"), and best thresholds vary widely by task (0.334-0.867 in one study). ([threshold study](https://arxiv.org/pdf/1805.11611))

## The compute ladder

| Rung | Method | Cost | Evidence |
|---|---|---|---|
| 0. Exact math | Normalize spoken numbers/units, compare as numbers, detect negation and antonym pairs | microseconds | Inference. No source covers numeric mismatch for answer grading |
| 1. Lexical | BM25 / TF-IDF over key points plus authored keywords | microseconds | TF-IDF cosine best on short news texts ([comparison](https://arxiv.org/pdf/1910.09129)). Hybrid lexical+dense is a [common pattern](https://dev.to/gabrielanhaia/bm25-dense-fusion-when-keyword-search-saves-your-rag-359i), but no controlled study with small models found |
| 2. Static soft matching | Soft cosine or relaxed Word Mover's Distance on static word vectors (potion/Model2Vec) | under 1 ms | Soft cosine [scores paraphrases with no shared words](https://notebook.community/gojomo/gensim/docs/notebooks/soft_cosine_tutorial). Relaxed WMD [up to 1000x faster than WMD](https://arxiv.org/abs/1912.00509v1). No head-to-head against modern sentence embeddings found |
| 3. Small transformer embedding | Max cosine vs precomputed multi-paraphrase key-point vectors | tens of ms | Existing MiniLM stage. [Binary quantization](https://blog.vespa.ai/combining-matryoshka-with-binary-quantization-using-embedder/): 32x smaller for about 90% of accuracy (Vespa's claim), with full-precision rescoring of top hits |
| 4. Cross-encoder | NLI model on (best sentence, key point) for undecided cases | hundreds of ms | Cross-encoders best on negation per [NevIR](https://arxiv.org/pdf/2305.07614), though only slightly above random out of the box; contrastive fine-tuning helped |
| 5. Generative LM | Narrator, or judge of last resort | seconds | Only rung with unmeasured hallucination risk |

## Literature that changes how rungs 4 and 5 are built

1. **Cast the task as entailment.** [EFL](https://arxiv.org/abs/2104.14690) reformulates tasks as entailment and fine-tunes small models on as few as 8 examples; reports few-shot results competitive with models about 500x larger. Our task is already entailment.
2. **Skip the decoder.** A [linear probe on frozen hidden states](https://arxiv.org/html/2408.03414v1) beat in-context learning on 12 classification tasks (GPT-J, GPT-2). [Calibrated binary QA](https://arxiv.org/pdf/2407.01122) reads Yes/No token logits in one forward pass; [another paper](https://export.arxiv.org/pdf/2312.09300) finds token-level true/false scoring better calibrated than free-form output. Raw logits still need calibration, and no test on sub-1B models was found. Could turn a 21 s generate call into one prefill.
3. **Precompute the known side.** [PreTTR](https://arxiv.org/pdf/2004.14255v1) precomputes lower layers for the known side (reports 42x speedup on document reranking). [Windowed sparse attention](https://arxiv.org/abs/2312.17649v2) and [layer-wise token compression](https://arxiv.org/pdf/2605.20683) cut cross-encoder cost further. [ColBERT](https://arxiv.org/pdf/2004.12832) and [Poly-encoders](https://arxiv.org/pdf/1905.01969) sit between bi- and cross-encoders. Key points are known in advance, so this transfers, but it is research code needing custom work.
4. **Early exit.** [DeeBERT](https://ar5iv.labs.arxiv.org/html/2004.12993) saves about 40% inference time; [PABEE](https://liner.com/review/bert-loses-patience-fast-and-robust-inference-with-early-exit) uses patience-based exits. Single-paper claims; none covered cheap-features-first cascades.
5. **Cascade with abstention.** [FrugalGPT-style cascades](https://arxiv.org/pdf/2404.10136) and [early abstention](https://arxiv.org/html/2502.09054v2): answer confident cases cheaply, abstain or defer otherwise. Mostly LLM-serving papers; transfer to a tiny classifier is untested.
6. **Distill and train with rationales.** [Distilling Step-by-Step](https://ar5iv.labs.arxiv.org/html/2305.02301): a 770M T5 beat 540B PaLM on a benchmark with LLM-written rationales as extra supervision. [Stacey & Rei 2024](https://preview.aclanthology.org/new-sigs/2024.findings-acl.132/): generated in-domain data helped distilled NLI students. [Borchers et al.](https://arxiv.org/abs/2501.09126): 80% synthetic / 20% human best for a BERT classifier (different task). Rationale distillation was shown on generative T5; benefit for a pure classifier is unproven.
7. **Compress.** [TinyBERT](https://arxiv.org/abs/1909.10351): 4-layer keeps 96.8% of BERT-base GLUE, 7.5x smaller, 9.4x faster; 6-layer matches BERT-base. [MiniLM](https://arxiv.org/pdf/2002.10957): over 99% on some tasks at half the parameters.
8. **Trim the vocabulary.** [Vocabulary trimming](https://arxiv.org/pdf/2305.15020v1): embeddings can exceed 80% of parameters in some multilingual LMs. [VocabTailor](https://arxiv.org/html/2508.15229v2) reports up to 99% less vocabulary memory with minimal loss. [COLING 2025](https://aclanthology.org/2025.coling-industry.64.pdf): BPE tokenizers need rebuilding. For nli-MiniLM2-L6-H768, about 50k vocab x 768 is roughly 38M of 82M parameters (my arithmetic). Measure accuracy after trimming.
9. **Stack rungs with a learned combiner.** Hybrid rules-plus-neural designs help in low-resource text classification ([NAACL 2024 Findings](https://aclanthology.org/2024.findings-naacl.165.pdf), [EMNLP 2020](https://preview.aclanthology.org/setup/2020.emnlp-main.258)). No source tested logistic-regression stacking specifically.
10. **Skip:**
    - gzip+kNN ([paper](https://preview.aclanthology.org/setup/2023.findings-acl.426)): [rebuttal](https://arxiv.org/pdf/2307.15002v1.pdf) argues bag-of-words matches or beats it, and its accuracy figures were questioned.
    - BitNet ([bitnet.cpp](https://arxiv.org/pdf/2410.16144)): 1.37-5.07x ARM speedups, authors' numbers, against fp16 llama.cpp; needs ternary-trained models and none exist for NLI.

## Where accuracy bumps should come from

Items 1-3 are inference from the evidence above, not published results for this task.

1. **Multi-reference key points.** At quiz-build time, have the strong agent write 5-10 spoken paraphrases per key point, including sloppy ones, and score against the best match. Zero device cost.
2. **Contrastive negatives.** Train with negated and wrong-number pairs (NevIR shows this helps).
3. **Learned combiner and per-key-point thresholds.** Logistic regression or small tree over all rung scores, trained offline, calibrated so "unsure" is a real output.
4. **Abstention.** Report "unsure" below a confidence bar instead of guessing.
5. **Pre-encode the key side** in the quiz JSON (paraphrase vectors, expected number set, negated forms).
6. **Prefix KV caching** for any narrator prompt. Cache-reuse sources found were blogs and design notes; saving unmeasured.

## Gaps

- No source tested a full mixed math-plus-model grader like this. The pieces are well supported separately.
- No sub-1B probing-vs-generation comparison found; no study of multi-paraphrase expansion for short-answer grading.
- No head-to-head of soft cosine / WMD against modern sentence embeddings on compute.
- Surveys ([SLM survey](https://arxiv.org/html/2501.05465v2), [SLM measurements](https://www.alphaxiv.org/abs/2409.15790)) support task-specific small models in general; individual papers not checked.

## Next step

Build the ladder in `bench/` as a feature table: one row per (answer, key point), one column per rung's score. Fit the combiner, then ablate rungs to see which earn their cost.
