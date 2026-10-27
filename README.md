# Rubber Duck Quizzer

A physical "duck" that quizzes you out loud on topics you choose, at random times
inside an active window you configure. You answer by voice. It tells you which key
points of the model answer you missed or contradicted.

Offline by design: no wifi or Bluetooth needed (can be added later for uploads).

## Goals

- Speaker + mic: the duck speaks questions and listens to answers.
- Q&A content is prepared ahead of time by AI agents (on a laptop) as structured files.
- A small on-device LLM (optional) clarifies where an answer deviated from the key points.
- A USB-C connection (or a USB stick) to drop in new quiz files and config.
- Config: active hours (from/to), selected topics, question frequency.
- When active, it asks a random question from the selected topics at random intervals.

## Hardware decision

| Option | Verdict |
|---|---|
| Raspberry Pi 5 (4/8GB) + USB speakerphone + RTC | Best result, but costs more. Fallback if the phone fails. |
| Old phone | Chosen first: free, has mic, speaker, battery and a USB port. |

### The phone: Moto G5 Plus (`potter`)

- Android 8.1.0, **32-bit userland (`armv7l`, ABI `armeabi-v7a`)** even though the SoC
  (Snapdragon 625) is 64-bit.
- 3.6GB RAM (about 2.4GB available), 25GB data partition.
- Display: right ~20% dead, touch works. Controlled via `adb` and `scrcpy`.
- Uses micro-USB (data cable needed, not charge-only).
- Previous Google account belonged to a deceased person: remove the account in
  Settings **before** any factory reset, otherwise Factory Reset Protection locks it.

## Planned software design

1. **Prepare content (laptop, AI agents):** produce quiz JSON per topic:
   `{ question, answer, key_points[], synonyms? }`. Plain JSON is more reliable than
   parsing PDFs on the device.
2. **Scheduler (on device):** active window, random interval, random question from
   selected topics.
3. **Speak:** Android TTS via `termux-tts-speak` (Termux:API).
4. **Listen:** record with Termux:API mic, then speech-to-text. Vosk may not have a
   32-bit ARM wheel, so alternatives are Android's recognizer or pocketsphinx.
5. **Check (fast path, EKM):** keywords plus sentence embeddings (MiniLM) against `key_points`, plus matching
   against authored `wrong_claims` to name contradictions, then speak the missed points immediately from templates
   built from the key point text. About 0.5 s, 38 MB, no LLM (see Benchmark log).
6. **Check (slow path, optional):** a small model for a short clarification of the worst mismatch,
   and an optional API key for real conversation. Zero-shot sub-1B LLMs were not good enough
   (see Benchmark log); a fine-tuned small model is on the TODO list.
7. **File drop:** MTP over USB, or a USB stick via OTG. Later: wifi upload page.
8. **Autostart:** Termux:Boot + `termux-wake-lock`, battery optimization off for Termux.

## Progress log

- [x] Hardware options compared; phone chosen as first attempt.
- [x] Phone unlocked (PIN guess), USB debugging on, `adb`/`scrcpy` mirroring works
      (Mac and Windows).
- [x] Termux, Termux:API and Termux:Boot installed (GitHub builds, same signing key).
- [x] TTS and mic recording tested in Termux.
- [x] Fixed broken `cmake` (package mismatch), `cmake --version` = 4.4.4.
- [x] llama.cpp build on the phone hit `redefinition of 'vcvtnq_s32_f32'` (32-bit ARM
      + clang 21). Fixed by wrapping that function in `ggml/src/ggml-cpu/ggml-cpu-impl.h`
      in `#if 0 ... #endif`. Rebuilt with `-j2` (about 1 hour), 0 errors.
- [x] Qwen2.5-0.5B Q4_K_M (469MB) runs on the phone with 4 threads:
      prompt processing about 9 tokens/s, generation about 5.6 tokens/s.
      Run with `LD_LIBRARY_PATH=~/llama.cpp/build/bin` and `--single-turn --simple-io`.
- [x] SSH into Termux over USB for remote commands (`adb forward tcp:8022 tcp:8022`).
- [x] Quiz JSON schema defined (`data/rate-limiter.quiz.json`), baseline dataset DS1 and
      benchmark harness written, B1 benchmark run (see Benchmark log).
- [x] Agent prompts written: `prompts/research-agent.md` and the dataset prompt below.
- [ ] Choose speech-to-text for 32-bit ARM.
- [ ] Write the scheduler + quiz loop.
- [ ] Debloat Android via `adb shell pm disable-user --user 0 <package>` (keep the
      TTS engine and system UI).
- [ ] Autostart on boot, power-loss behaviour, case/enclosure.

## Research: making the checker efficient (2026-10-28)

The checker is a **diff task**, not a chat task: given `key_points[]` and the
speech-to-text transcript, say which points are missing or contradicted. It needs no
world knowledge and no question context, so the design is tiered:

| Tier | Method | Cost | Catches |
|---|---|---|---|
| 0 | Fuzzy/keyword match (rapidfuzz, synonyms from JSON) | ms | obviously missing points |
| 1 | Sentence embeddings: all-MiniLM-L6-v2 (22.6M params), GGUF Q4_0, via the llama.cpp already built | under 1 s (to verify) | missing points, paraphrases |
| 2 | Contradiction check: tiny LLM with a short fixed prompt (about 60 tokens), constrained output (GBNF/JSON schema) | about 10 s | negations, wrong claims |
| 3 | Optional API key: Gemini API free tier (or another provider) | network | hard cases |

Candidates for tier 2 (all unverified on this phone): Gemma 3 270M, LFM2-350M, SmolLM2,
Qwen2.5-0.5B (works, 5.6 tok/s). Embeddings are weak at negation, which is why tier 2 exists.
A tiny model fine-tuned on synthetic "answer vs transcript" pairs generated by the agents is
the likely best result for the size. Cache the fixed prompt prefix to avoid re-processing it.
Do not int8-quantize DeBERTa NLI models (reported to collapse); MiniLM-family is safer.
No ready-made ONNX/GGUF NLI model found for armv7, so ONNX Runtime on 32-bit ARM is unproven.

### Potential solution: free Gemini for quality
- **The Gemini consumer app can't be driven by this project.** No public API for the app,
  it needs Android 10+ (this phone is 8.1), and automating its UI is fragile and likely against
  its terms. Not recommended.
- **The Gemini API with a free AI Studio key is the legitimate route.** Free tier is Flash /
  Flash-Lite only, with limits that vary by project (check the AI Studio console). About 20
  calls a day is far below any limit. Free-tier content may be used to improve Google
  products, which is acceptable for quiz text but worth knowing. Needs internet, so it
  conflicts with the "no wifi" goal and must stay optional.

### Hardware
- Pi 5 prices have risen in 2026 (a 4GB board around 140 EUR in one source). Other boards
  are not cheaper because of memory costs.
- If tiers 0-1 carry the work, compute needs are tiny, so a Pi Zero 2 W class board or a
  spare 64-bit phone is enough. No Pi 5 required.
- Walnut Pi 2B (Allwinner T527, 2 TOPS NPU, 1-4GB) is a cheap NPU board, but there are no
  LLM benchmarks for it.

## Benchmark log

Every run has its own table with its own ID prefix. The latest runs are shown first; older runs are in the dropdowns at the end
and are kept unchanged. To add a run: give it the next ID prefix, add a table with the same columns, and put raw output in
`bench/results/`. Harness: `bench/bench.py` (`fuzzy | embed | llm | feedback | stage1 | pipeline | narrate`, with `--dataset <name>`);
EKM tuning is in `bench/ekm.py` and the leave-one-chapter-out analysis in `bench/analyze_loco.py`.

**Datasets**

| ID | Name | Content |
|---|---|---|
| DS1 | `rate-limiter` | 5 questions, 17 key points; 20 simulated spoken answers (A full, B partial, C one point contradicted, D don't know). 68 labeled points: 38 covered, 25 missing, 5 contradicted. Written from general knowledge of the chapter "Design a Rate Limiter". Keywords and transcripts have the same author, so keyword results are optimistic. |
| DS2 | `sdi-ch4-8` (also split per chapter: `ch4-rate-limiter`, `ch5-consistent-hashing`, `ch6-key-value-store`, `ch7-unique-id-generator`, `ch8-url-shortener`) | Built from chapters 4 to 8 of the book (pages 51-130), in our own words. 39 questions, 116 key points, each with keywords and `wrong_claims`. 195 simulated spoken answers: A full (39), B partial (39), C one point contradicted (39), D clueless (39), plus E paraphrase-heavy (9), F speech-to-text noisy (14), G rambling (6), H hedged (10). 580 labeled points: 361 covered, 180 missing, 39 contradicted. All written by one author (Claude), and each C answer is built from one of that point's `wrong_claims`, so contradiction results are optimistic. |

**Definitions.** "Flag" = the checker reports a point as missing or wrong. P/R/F1 are for flagging
points whose label is not `covered`. "Contradicted flagged" = contradicted points reported as
not covered; "named" = reported specifically as contradicted. Exact = 3-class accuracy.
**EKM** = Embeddings + Keyword Method: keywords and sentence embeddings combined. Two variants are logged: "either flags" (report a point if either test reports it, the first version) and a single score where each keyword hit adds a boost to the embedding similarity (`cosine + 0.2 to 0.3 x min(hits, 2) >= threshold`, the better version). "Held-out" = threshold fixed from DS1 or tuned on four chapters and tested on the fifth (leave-one-chapter-out, LOCO).

**Device for all runs:** Moto G5 Plus, Snapdragon 625, 32-bit ARM (armv7l), Android 8.1, Termux,
llama.cpp b1-6c44591 (patched), 4 threads.

### Latest runs

**In progress** (started 2026-10-28, about 4 hours on the phone; results are filled in when each run finishes):
B5 small LLM verdicts on all of DS2 (LFM2-350M, Qwen2.5-0.5B, Gemma 3 270M), B6 the pipeline (EKM first, then a small model as
second opinion on routed points only: LFM2 and Qwen), B7 narrator sample (one spoken sentence from the known key point).

### Older runs (click to expand)

<details>
<summary>B3 (2026-10-28): EKM keyword boost and wrong_claims on DS2, leave-one-chapter-out</summary>

| Run | Date | Dataset | Method | Model / config | Time per answer | RAM | Tokens in / out | Exact | Flag P | Flag R | Flag F1 | Contradicted flagged / named | Notes |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| B3-01 | 2026-10-28 | DS2, LOCO | Embeddings only | Q4_0, threshold tuned on 4 chapters, tested on the 5th | about 0.5 s | 38 MB | 126.5 / 0 | 0.817 | 0.790 | 0.758 | 0.774 | 23% (9/39) / 0% | Offline analysis of B2-03 scores (`bench/analyze_loco.py`) |
| B3-02 | 2026-10-28 | DS2, LOCO | EKM, keyword boost | Q4_0 cosine + 0.2-0.3 x min(hits, 2) >= T (T 0.74-0.80) | about 0.5 s | 38 MB | 126.5 / 0 | 0.862 | 0.844 | 0.840 | 0.842 | 28% (11/39) / 0% | One combined score beats "either flags" and keywords alone |
| B3-03 | 2026-10-28 | DS2, LOCO | Embeddings + wrong_claims | Q4_0; contradicted if a sentence is closer to a wrong claim than to the key point (cosine >= 0.75-0.85 and margin 0.05) | about 0.5 s | 38 MB | 126.5 / 0 | 0.841 | 0.785 | 0.836 | 0.810 | 67% (26/39) / 51% (20/39) | 7 false "contradicted" among 541 other points. Wrong claims embedded once at ingest |
| B3-04 | 2026-10-28 | DS2, LOCO | EKM boost + wrong_claims | B3-02 plus B3-03 | about 0.5 s | 38 MB | 126.5 / 0 | **0.886** | 0.831 | **0.900** | **0.864** | 62% (24/39) / 51% (20/39) | Best so far. C answers were built from the same wrong_claims, so contradiction numbers are optimistic |

**Findings after B2 and B3 (DS2, 195 answers, 580 points).**
- The larger dataset is harder: keywords alone fell from F1 0.90 (DS1) to 0.83, so DS1 flattered them.
- Mean cosine to the key point is 0.76 for covered, 0.37 for missing and 0.66 for contradicted points: a contradicted statement
  looks almost as "covered" as a correct one. Embeddings and keywords alone name 0 of 39 contradictions and flag only 11 of 39.
- "Either flags" EKM is not better than keywords in F1 (0.825 vs 0.827): it only trades precision for recall. A single score where keyword
  hits boost the embedding similarity is better (F1 0.842, held out).
- Matching sentences against authored `wrong_claims` is the first thing that names contradictions: 20 of 39 named, with 7 false
  alarms among the other 541 points (held out). Combined with the boost it gives the best result so far (F1 0.864, exact 0.886).
  Caveat: the C answers were built from the same wrong claims, so real mistakes outside the list will be caught less often.
- Where it still fails (EKM boost + wrong_claims): paraphrase-heavy answers (variant E) get 20 false flags out of 26 covered points,
  noisy answers (F) 8 of 38, and 15 of 39 contradictions are missed. All other variants are at or near 0 to 6 false flags.
- Clean speech (A) and rambling answers (G) were never falsely flagged by any method.

</details>

<details>
<summary>B2 (2026-10-28): keywords, embeddings and EKM variants on DS2 (195 answers)</summary>

| Run | Date | Dataset | Method | Model / config | Time per answer | RAM | Tokens in / out | Exact | Flag P | Flag R | Flag F1 | Contradicted flagged / named | Notes |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| B2-01 | 2026-10-28 | DS2 | Keywords | at least 1 keyword hit | under 1 ms | none | 0 | 0.855 | 0.861 | 0.795 | 0.827 | 28% (11/39) / 0% | F1 down from 0.897 on DS1: DS1 flattered keywords |
| B2-02 | 2026-10-28 | DS2 | Keywords | at least 2 keyword hits | under 1 ms | none | 0 | 0.755 | 0.669 | 0.950 | 0.785 | 72% / 0% | Too strict |
| B2-03 | 2026-10-28 | DS2 | Embeddings | all-MiniLM-L6-v2 Q4_0, T=0.56 fixed from DS1 | 493-506 ms | 38 MB | 126.5 / 0 | 0.824 | 0.802 | 0.776 | 0.789 | 28% / 0% | Best T on DS2 is also 0.56, so the threshold transferred. Key point + wrong-claim precompute 18 s, 4525 tokens, once at ingest |
| B2-04 | 2026-10-28 | DS2 | Embeddings | all-MiniLM-L6-v2 Q8_0, T=0.54 fixed from DS1 | 519 ms | 43 MB | 126.5 / 0 | 0.831 | 0.831 | 0.740 | 0.783 | 21% / 0% | Precompute 11 s, 2724 tokens (run before wrong claims were embedded) |
| B2-05 | 2026-10-28 | DS2 | EKM, either flags | Q4_0 embeddings OR keywords, T=0.56 | about 0.5 s | 38 MB | 126.5 / 0 | 0.831 | 0.775 | 0.881 | 0.825 | 41% / 0% | No better F1 than keywords alone: recall up, precision down |
| B2-06 | 2026-10-28 | DS2 | EKM, both must flag | Q4_0 embeddings AND keywords, T=0.56 | about 0.5 s | 38 MB | 126.5 / 0 | 0.848 | 0.915 | 0.689 | 0.786 | 15% / 0% | Fewest false alarms, misses 31% of real problems |
| B2-07 | 2026-10-28 | DS2 | EKM, either flags | Q8_0 embeddings OR keywords, T=0.54 | about 0.5 s | 43 MB | 126.5 / 0 | 0.840 | 0.793 | 0.877 | 0.833 | 41% / 0% | |
| B2-08 | 2026-10-28 | DS2 | EKM, both must flag | Q8_0 embeddings AND keywords, T=0.54 | about 0.5 s | 43 MB | 126.5 / 0 | 0.847 | 0.929 | 0.658 | 0.770 | 8% / 0% | |

</details>

<details>
<summary>B1 (2026-10-28): first benchmark on DS1 (20 answers): keywords, embeddings, three small LLMs</summary>

| Run | Date | Dataset | Method | Model / config | Time per answer | RAM | Tokens in / out | Exact | Flag P | Flag R | Flag F1 | Contradicted flagged / named | Notes |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| B1-01 | 2026-10-28 | DS1 | Keywords | at least 1 keyword hit | 0.05 ms | none | 0 | 0.853 | 0.929 | 0.867 | 0.897 | 80% / 0% | Optimistic: keywords written by the data author |
| B1-02 | 2026-10-28 | DS1 | Keywords | at least 2 keyword hits | 0.05 ms | none | 0 | 0.706 | 0.674 | 0.967 | 0.795 | 100% / 0% | Too strict, many false flags |
| B1-03 | 2026-10-28 | DS1 | Embeddings | all-MiniLM-L6-v2 Q4_0, max cosine over transcript sentences, T=0.56 | 504 ms | 38 MB | 128.5 / 0 | 0.809 | 0.839 | 0.867 | 0.852 | 80% / 0% | Threshold tuned on same data. Key points precomputed: 1.1 s for 17 points. Load 1.3 s |
| B1-04 | 2026-10-28 | DS1 | Embeddings | all-MiniLM-L6-v2 Q8_0, T=0.54 | 526 ms | 43 MB | 128.5 / 0 | 0.824 | 0.889 | 0.800 | 0.842 | 60% / 0% | Q4_0 vs Q8_0: no meaningful difference |
| B1-05 | 2026-10-28 | DS1 | Keywords OR embeddings | flag if either B1-01 or B1-03 flags | about 0.5 s | 38 MB | 128.5 / 0 | - | 0.829 | 0.967 | 0.892 | - | Computed offline from B1-01 and B1-03 results. Best recall |
| B1-06 | 2026-10-28 | DS1 | Keywords AND embeddings | flag only if both flag | about 0.5 s | 38 MB | 128.5 / 0 | - | 0.957 | 0.733 | 0.830 | - | Computed offline. Fewest false alarms |
| B1-07 | 2026-10-28 | DS1 | LLM verdicts | LFM2-350M Q4_0, JSON-schema output, whole answer in one prompt | 20.9 s | 286 MB | 205.4 / 24.6 | 0.676 | 0.719 | 0.767 | 0.742 | 40% / 20% | Best of the small LLMs. No prefix caching observed |
| B1-08 | 2026-10-28 | DS1 | LLM verdicts | Qwen2.5-0.5B Q4, same prompt | 16.4 s | 556 MB | 219.1 (79.9 processed after cache) / 16.7 | 0.500 | 0.567 | 0.567 | 0.567 | 40% / 20% | Prefix caching worked |
| B1-09 | 2026-10-28 | DS1 | LLM verdicts | Gemma 3 270M Q4_0, same prompt | 12.7 s | 340 MB | 206.2 / 30.8 | 0.382 | 0.448 | 1.000 | 0.619 | 100% / 20% | Flags almost every point, so recall is meaningless |
| B1-10 | 2026-10-28 | DS1 (8 B and C cases) | LLM free-text feedback | Qwen2.5-0.5B Q4, max 70 tokens | 32.1 s | 556 MB | about 220 / 69.9 | - | - | - | - | - | Qualitative: called most flawed answers correct, invented that the student said things they did not |
| B1-11 | 2026-10-28 | DS1 (8 B and C cases) | LLM free-text feedback | LFM2-350M Q4_0, max 70 tokens | 27.1 s | 286 MB | about 205 / 69.8 | - | - | - | - | - | Qualitative: found some real misses but also invented errors |

**Findings so far.** Keywords plus embeddings carry the checker (0.5 s, 38 MB, no generated tokens).
Sub-1B LLMs are slow and unreliable here. No checker names contradictions reliably (at most 1 of 5).
Caveats: 20 cases, 5 contradictions, text transcripts (no speech-to-text errors yet), thresholds
tuned on the same data. Embeddings score "returns 403" as close to "returns 429".

</details>

<details>
<summary>B0 (2026-10-27): raw speed of Qwen2.5-0.5B on the phone</summary>

| Run | Date | Dataset | Method | Model / config | Time per answer | RAM | Tokens in / out | Exact | Flag P | Flag R | Flag F1 | Contradicted flagged / named | Notes |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| B0-1 | 2026-10-27 | none | LLM raw speed | Qwen2.5-0.5B Q4_K_M, llama-cli | 5.6 tok/s generation, 9.1 tok/s prompt | - | - | - | - | - | - | - | Single prompt about HTTP 404, answer correct |

</details>

## Ideas and next experiments

- **Pipeline (planned B4, not yet run):** keywords and embeddings first, then give only the mismatched
  points plus the closest transcript sentence to a small model. Judge role versus narrator role
  (rephrase the known-correct key point instead of judging). See `prompts/research-agent.md`.
- **`wrong_claims[]` per key point**: tested in B3-03/B3-04, works (20 of 39 contradictions named). Next: test with
  mistakes written by a different author than the wrong claims.
- **Bigger dataset** from the book: done (DS2). Next: have a different agent generate a dataset from the same chapters with the prompt below, to remove same-author bias.
- Compare embedding models (bge-small, gte-small, EmbeddingGemma, Model2Vec).

## TODO

- [x] Build a larger dataset from several book chapters and re-run the benchmark on it (DS2, B2/B3).
- [x] Test the `wrong_claims` idea (B3-03, B3-04): works on DS2, with the same-author caveat.
- [ ] **Layered feedback flow (design idea, untested).** After the answer: (1) EKM verdict in about 0.5 s, spoken right
      away from templates ("You missed X"); (2) while that and a recap of the correct answer are being spoken (about
      10-20 s of speech), a small model in parallel elaborates on the points where EKM is uncertain or where wrong-claim
      confusions are likely (for example 4xx codes); (3) its sentence is spoken afterwards if ready, dropped or logged
      if not. Questions to settle by measurement:
      - Short feedback may not cover the 7-14 s the model needs: pre-warm the cached prompt prefix (instruction,
        question, key points) while the question is being asked and the user answers, so only the transcript sentence
        remains to process; fall back to the recap of the full model answer as filler; or run the model only when EKM
        is in a borderline band (cosine close to the threshold) or flags a contradiction, and skip it otherwise.
      - Half duplex: mic off while the duck speaks (phone speaker and mic are close, no echo cancellation), and never
        queue model output that would be spoken while the user is talking.
      - CPU contention on 8 small cores (text-to-speech, speech-to-text, model together), thermal throttling and
        RAM when all three run at once.
      - Role of the small model: narrator (rephrase the known-correct key point, no new facts) rather than judge.
- [ ] **Run B4: the pipeline** (EKM first, then only the mismatches plus the closest transcript sentence to a small model, narrator
      role), measuring hallucination, usefulness and time against templates only.
- [ ] **Always-on microphone with a wake phrase** ("hey ducky"). Idea: a tiny always-listening keyword spotter wakes the
      duck, then speech-to-text captures a command or a question. Routing without the slow model: match the command
      against a short intent list and match topic names or question text with the same embeddings (about 0.5 s), so
      "ask me something about caches" picks a topic without an LLM. Only free-form chat goes to the small model
      (slow, about 10 s or more) or to the API tier. Things to check: a wake-word engine that really runs on 32-bit ARM
      in Termux (Vosk with a restricted grammar, pocketsphinx keyword spotting, others); CPU use while idle;
      false triggers and self-triggering from the duck's own speech; whether background microphone access works on
      Android 8.1 (it is restricted from Android 9, so this old phone may help); privacy (everything stays local).
- [ ] **Research fine-tuning and model choice further** (use `prompts/research-agent.md`, refresh it with B2-B7 results first). Open questions:
      - Would a 1.5B model (for example Qwen2.5-1.5B) zero-shot beat the 0.5B models enough to matter? Roughly 3x slower on this phone
        (about 30 s for a pairwise prompt); test on a sample of the routed points after B6.
      - Two jobs, two models: (1) fine-tune the 22M embedding model (MiniLM-L6) with contrastive or triplet loss, using wrong
        claims as hard negatives, plus a 3-way head, to separate covered / missing / contradicted (exports to GGUF, still about
        0.5 s); (2) LoRA-fine-tune a 270M-500M model (Gemma 3 270M, LFM2-350M, Qwen2.5-0.5B) as a narrator that restates the
        known key fact in one sentence. Compare against cross-encoder NLI models (DeBERTa-v3-xsmall, MiniLM NLI) and check
        whether any of them runs on 32-bit ARM.
      - Training data: how many examples (a few thousand?), how to generate them across many topics with strong agents, how to hold
        out whole topics, how to avoid same-author bias, and how real speech-to-text errors should be simulated.
      - Tooling on the M3 (16 GB, nothing installed yet): `mlx-lm` LoRA versus PyTorch + PEFT/TRL, sentence-transformers for the
        encoder, and the export path (merge adapter, convert to GGUF, quantize to Q4_0, test on the phone).
      - Realistic expected gains and how to measure them fairly (held-out topics, B2-B7 as the baseline).
- [ ] Train or fine-tune a small model (270M to 350M) for the narrow check: key point + transcript
      sentence -> covered / missing / contradicted, plus a one-line spoken explanation. Training
      data comes from the dataset prompt below plus synthetic examples from a strong model.
- [ ] Re-generate the dataset with a different agent (same chapters, prompt below) so keywords, wrong claims and
      transcripts no longer share an author, then re-run B2/B3.
- [ ] Paraphrase-heavy and noisy answers (E, F) are the weakest: try a larger embedding model (bge-small, gte-small,
      EmbeddingGemma) and shorter key point texts. Use `prompts/research-agent.md`.
- [ ] Run the small LLM verdict benchmark (B1-07 to B1-09) on DS2 (about an hour per model on the phone).
- [ ] Speech-to-text on 32-bit ARM; re-run with real transcripts.
- [ ] Optional API tier for hard cases (Gemini free key or other).
- [ ] Quiz-loop scheduler, file drop (USB), autostart, debloat.

## Prompt: build a benchmark dataset from a PDF

Give this to a capable AI agent that can read files, together with your PDF (or a chapter range).
It produces the two JSON files the harness reads. Replace the bracketed parts.
Use only PDFs you are allowed to use; this repo ignores `*.pdf` so books are not committed.

<details>
<summary>Show the prompt</summary>

````text
You are building a benchmark dataset for a spoken quiz buddy. The buddy asks a question aloud, the
user answers by voice (speech-to-text transcript), and a checker must decide for each key point of
the model answer whether the user COVERED it, MISSED it, or CONTRADICTED it. Build the data that
tests that checker.

INPUT: the attached PDF [FILE NAME]. Use chapter(s): [CHAPTER NAMES OR PAGE RANGES]. Topic slug: [slug, e.g. consistent-hashing].
Extract the text with a tool (pdftotext, pypdf, or OCR if scanned). Use the PDF outline to find page ranges.

STEP 1. Pick [10] concepts from the chapter that are good to quiz aloud. Skip anything that needs a
diagram, code, a table or a long calculation to answer, and skip trivia (dates, names) unless central.

STEP 2. For each concept write a quiz item:
- question: one sentence that is natural to hear spoken.
- answer: 2 to 4 sentences that are natural to speak, in YOUR OWN WORDS. Never copy sentences from the
  source; do not quote more than 5 consecutive words.
- key_points: 2 to 5 atomic points. Each states ONE fact, at most 20 words, and can be judged
  independently from the others.
- For each key point: keywords (words and phrases a speaker would naturally say, including numbers
  both as digits and as words, and common synonyms) and wrong_claims (1 to 3 plausible mistakes a
  student might state, each a complete statement that contradicts the key point).
- source_page: the page number(s) it came from.

STEP 3. For each quiz item write spoken-style test transcripts, with ground-truth labels per key
point (covered | missing | contradicted). Transcripts must sound like real speech: fillers ("um",
"uh", "you know", "like", "I mean"), false starts, self-corrections, trailing off, numbers spoken as
words ("four twenty nine"). Variants:
- A full: covers every key point, in the speaker's own words.
- B partial: omits one or two key points (label missing).
- C contradicted: states exactly ONE key point wrongly, using one of its wrong_claims, covers the rest.
- D clueless: does not know, or answers a different question. All missing.
- E paraphrase-heavy: covers every key point but shares as few words with the key points and keywords as possible.
- F noisy: covers every key point but with speech-to-text style errors (homophones, split or merged
  words, numbers as words).
- G rambling: covers the points but buried in tangents.
- H hedged: correct but uncertain ("I think it's 429?"). Label covered.
Definitions: covered = the transcript conveys the key point's meaning, even if reworded or hedged;
missing = not stated; contradicted = states the opposite or a specific wrong claim.
Labels must be true by construction. If a transcript is ambiguous, rewrite it until it is not.

STEP 4. Validate before you answer: ids unique; every key point id appears in every case's labels;
each variant C has exactly one contradicted label; variant E averages at most one keyword hit per key
point; no answer or transcript copies source text; counts reported at the end.

OUTPUT: exactly two JSON files with this shape (no extra fields, valid JSON):

data/[slug].quiz.json
{
  "topic": "...",
  "source_note": "book, chapter, pages; written in own words",
  "questions": [
    {"id": "q1", "question": "...", "answer": "...", "source_page": "52-54",
     "key_points": [
       {"id": "k1", "text": "...", "keywords": ["...", "..."], "wrong_claims": ["...", "..."]}
     ]}
  ]
}

tests/[slug].transcripts.json
{
  "note": "Simulated speech-to-text transcripts. Labels: covered | missing | contradicted.",
  "cases": [
    {"id": "q1-A", "question_id": "q1", "variant": "A",
     "transcript": "Um, so, basically ...",
     "labels": {"k1": "covered", "k2": "covered"}}
  ]
}

Finish with a short summary: number of questions, key points, cases, and label counts per class.
Do not include the source PDF text in your output.
````

Run the benchmark on the new data: `python bench/bench.py fuzzy --dataset <slug>` (also `embed` and `llm`
with a `--model`), then add rows to the Benchmark log above.

</details>

## Useful commands

```sh
# Mirror the phone
scrcpy --stay-awake --turn-screen-off

# Keep screen on while charging
adb shell svc power stayon true

# Remote shell into Termux over USB (start `sshd` in Termux first)
adb forward tcp:8022 tcp:8022
ssh -p 8022 -i <key> localhost
```

## Notes

- Never commit SSH keys. Keys used for the phone live outside this repo.
- Termux is not debuggable, so `adb run-as` does not work; use sshd as above.
