# Tiny on-device answer checker: research report

Date: 2026-10-28. Nothing below was run on the phone; performance numbers are cited or marked as estimates. Several items are only partly researched (see the end).

## TL;DR: top 3 next experiments

1. **Run an NLI cross-encoder only on points the cheap checkers flag.**
   - Pair each flagged key point with the closest transcript sentence and use `cross-encoder/nli-MiniLM2-L6-H768` (Apache-2.0, 82M params). It outputs contradiction/entailment/neutral directly.
   - Expected gain: the only candidate that targets contradictions, the weakest result so far (at most 1 of 5 named correctly). Unmeasured.
   - Cost: about a day. Risk: medium. It was trained on MNLI-style text and spoken fragments are out of domain, so fine-tune if zero-shot is weak (experiment 3).
2. **Add rule-based number and negation checks, plus matching against `wrong_claims`.**
   - Convert spoken numbers to digits ("four twenty nine" to 429), then compare numbers between key point and matched sentence. This fixes "403 vs 429" exactly. No embedding-numeracy benchmark was found, so this is reasoning, not evidence.
   - Cost: hours, no model. Risk: low.
3. **Fine-tune the cross-encoder on synthetic pairs, not a 270M generative model.**
   - Have a strong LLM generate a few thousand (key point, spoken answer, label) triples with fillers and number words. Fine-tune the 82M encoder on the Mac (minutes to an hour).
   - Why: about 3x smaller than Gemma 270M, no generation needed (about 5.6 tok/s measured), calibrated 3-way label. No source gave example counts or expected gains; "a few thousand" is a guess.
   - Cost: 1-2 days. Risk: medium (synthetic data must match real STT output).

## Candidates

| Name | Params | Size | Format | License | armv7 / Termux | Evidence |
|---|---|---|---|---|---|---|
| [nli-MiniLM2-L6-H768](https://huggingface.co/cross-encoder/nli-MiniLM2-L6-H768) | 82.1M (6 layers, hidden 768, vocab 50k) | fp32 ONNX plus a `qint8_arm64` ONNX in repo (size not read) | safetensors, ONNX | Apache-2.0 | Likely, via llama.cpp (RoBERTa classifier convertible) or ONNX Runtime | MNLI-mm 86.89 per [sbert table](https://sbert.net/docs/cross_encoder/pretrained_models.html) (via search snippet) |
| [nli-deberta-v3-xsmall](https://huggingface.co/cross-encoder/nli-deberta-v3-xsmall) | 70.8M | ONNX incl. `qint8_arm64` | safetensors, ONNX | Apache-2.0 | llama.cpp: no (no DeBERTa support found). ONNX: likely | MNLI-mm 87.77, same table |
| [MoritzLaurer DeBERTa-v3-xsmall binary](https://huggingface.co/MoritzLaurer/DeBERTa-v3-xsmall-mnli-fever-anli-ling-binary) | 70.8M | ONNX plus quantized ONNX | ONNX | MIT | ONNX likely | Binary entailment/not-entailment; 782k training pairs from 4 datasets; no accuracy retrieved |
| [potion-base-8M](https://huggingface.co/minishlab/potion-base-8M) (also 4M, 2M) | 7.56M | about 30 MB | Model2Vec static | MIT | Likely (numpy plus tokenizer) | MTEB avg 51.08 vs 55.93 for all-MiniLM-L6-v2 ([results](https://minish.ai/packages/model2vec/results/)) |
| bge-small-en-v1.5, gte-small, e5-small-v2 | 33.4M each | not retrieved | HF; community GGUFs exist | MIT | Likely (BERT-class) | Params and license from HF API; no head-to-head scores found |
| [EmbeddingGemma-300M](https://arxiv.org/abs/2509.20354) | 303M | not retrieved | HF | Gemma, manual gate | Unknown | Paper: under 1% loss at int4/int8 (snippet). Too big for budget |
| SmolLM2-135M / 360M | 135M / 362M | not retrieved | HF | Apache-2.0 | Likely (llama.cpp) | Params and license confirmed; no task evidence |
| Gemma 3 270M | 268M | not retrieved | HF | Gemma, manual gate | Works (measured) | Measured: flags nearly everything |
| LFM2-350M | 354M | not retrieved | HF | "other" (LFM license) | Works (measured) | Measured: best LLM, 0.74 F1 |

## Confirmed facts (primary sources)

**llama.cpp and classifier heads**
- The converter registers `BertForSequenceClassification`, `RobertaForSequenceClassification`, `DistilBertForSequenceClassification`, `XLMRobertaForSequenceClassification`, `ModernBertForSequenceClassification`, `NeoBERTForSequenceClassification` ([bert.py](https://github.com/ggml-org/llama.cpp/blob/master/conversion/bert.py)). It writes `classifier.output_labels` into the GGUF.
- Server `/rerank` needs `--pooling rank` or `--reranking`; scores are probabilities in [0,1] ([server README](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md)). The README says nothing about multi-class classification. Not confirmed that 3-way NLI logits are exposed via CLI or server; test this first.

**Runtimes on armv7**
- sherpa-onnx v2.0.1 (2026-10-26) ships `android-armeabi-v7a-termux-shared` (16 MB) and `-static` (148 MB) tarballs, plus `linux-arm-gnueabihf` builds. PyPI has `android_24_armeabi_v7a` and `manylinux_2_35_armv7l` wheels for 2.0.1. The manylinux wheels target glibc and probably won't work on Termux (bionic); the Android wheel is the Termux candidate. cp310-313 tags seen; cp314 not checked. ([releases](https://github.com/k2-fsa/sherpa-onnx/releases), [PyPI JSON](https://pypi.org/pypi/sherpa-onnx/json))
- ONNX Runtime has no armv7 wheel on PyPI, but Termux packages it. The [build script](https://github.com/termux/termux-packages/blob/master/packages/onnxruntime/build.sh) (v1.31.1) enables XNNPACK and Python bindings, and its comments mention 32-bit target fixes. Exact install command not verified: run `pkg search onnxruntime` on the phone.
- Vosk 0.3.45 ships a `linux_armv7l` wheel, a `vosk-android` zip and a `vosk-linux-armv7l` zip ([releases](https://github.com/alphacep/vosk-api/releases)). Whether the wheel works in Termux is unverified. A Hugging Face mirror says small models are about 50 MB and need about 300 MB RAM.

**Data and terms**
- SemEval-2013 Task 7: Beetle has 56 questions and about 3,000 answers; SciEntsBank has 197 questions and about 10,000 answers. 5-way labels, with 3-way and 2-way derived. Mirrors: [nkazi/SciEntsBank](https://huggingface.co/datasets/nkazi/SciEntsBank), [nkazi/Beetle](https://huggingface.co/datasets/nkazi/Beetle). License not verified.
- Gemini free tier: only third-party blogs found, which disagree on limits. They agree free-tier content may be used to improve Google products. Check ai.google.dev before relying on it.

## Inferences, not verified

- A 6-layer, 768-hidden encoder on about 64 tokens should take well under a second on the A53s (estimate, not measured).
- int8 should suit a small encoder better than Q4. No evidence found on what Q4 does to NLI heads, so use Q8_0 or ONNX int8 and compare logits against PyTorch on about 200 pairs before trusting it.
- The `qint8_arm64` ONNX files were built for 64-bit ARM and may not run well on 32-bit; quantize locally.

## Steps

On the Mac (not yet run):
```bash
python3 -m venv .venv && source .venv/bin/activate
pip install sentence-transformers onnx onnxruntime
# 1. zero-shot check: pair = (best transcript sentence, key point)
python - <<'E'
from sentence_transformers import CrossEncoder
m = CrossEncoder("cross-encoder/nli-MiniLM2-L6-H768")
print(m.predict([("returns four twenty nine","returns 403")]))  # order: contradiction, entailment, neutral
E
# 2. GGUF route
git clone https://github.com/ggml-org/llama.cpp
python llama.cpp/convert_hf_to_gguf.py cross-encoder/nli-MiniLM2-L6-H768 --outtype f16 --outfile nli.f16.gguf
llama.cpp/build/bin/llama-quantize nli.f16.gguf nli.q8.gguf Q8_0
```
Reuse `bench/pipeline.py` to add the NLI stage. For a fast comparison, `onnxruntime.quantization.quantize_dynamic` gives an int8 ONNX to test on the phone.

On the phone: `pkg install onnxruntime` (verify name), then time 100 pairs at 4 and 8 threads. In parallel, check the sherpa-onnx Android wheel with `pip install sherpa-onnx`.

## What not to bother with

- Zero-shot Qwen 0.5B or Gemma 270M as judges: measured failures, and about 9 tok/s prompt processing makes long prompts slow.
- EmbeddingGemma: 303M params and gated, blows the budget.
- A DeBERTa GGUF: llama.cpp doesn't appear to support it. Use ONNX for DeBERTa or take MiniLM2.
- Tuning Q4_K vs Q4_0 flags: no armv7/A53 data found, and the encoder route avoids the problem.
- whisper.cpp for speech-to-text: no armv7 or Snapdragon 625 benchmarks found. Start with Vosk or sherpa-onnx, which have real armv7 builds.

## Not researched or not found

- Item 6 (hallucination rates of sub-1B models as judge vs narrator): no evidence gathered. The narrator role (rephrasing the known key point) is probably safer, but that is reasoning only.
- Item 2: no published recipe, sample counts or expected gains for tiny-model fine-tuning. GGUF export of Gemma 3 via MLX unconfirmed.
- Item 3: no evidence that any embedding model reliably handles negation or numbers. The CANNOT negation fine-tune exists but wasn't evaluated.
- Item 5: no non-llama.cpp runtime benchmarks; no data on constrained or speculative decoding on this CPU.
- Item 7: no data on how Vosk or sherpa-onnx handle spoken numbers or technical terms.
- Item 10: skipped; nothing above depends on it.
