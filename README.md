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

## Product design: two modes, privacy first (decided 2026-10-29)

One duck, two ways to judge answers. The user picks; nothing is forced.

| | **Local mode** (default) | **Cloud mode** (optional, user brings an API key) |
|---|---|---|
| Purpose | Works on a throwaway phone, with no network and nothing leaving the device | Better judging, explanations and real conversation |
| Speech to text | On the device | On the device (audio never leaves it) |
| Judge | EKM: keywords + sentence embeddings, plus an "unsure" answer; optionally the NLI model if it fits the phone | A small hosted model reads the key point and what the user said |
| What leaves the device | Nothing. Wi-fi and Bluetooth can stay off | Text only: the transcript sentence(s) and the key point. Never audio |
| Measured quality | Flag F1 about 0.7 to 0.8 on independent mistakes, contradictions named about half the time with many false alarms (B11). Advisory, not a grader | Claude Haiku as judge on the same independent set: F1 0.993, 93% of contradictions named, 96% of 'contradicted' calls right (B15). Judged by sub-agents, so latency and price are not yet measured |
| Speed | About 0.5 s for the first verdict on the phone | About 1 to 3 s per answer (estimate) |
| Cost | Free | Pennies per day at a few questions a day (estimate); user sets a spending cap |
| Hardware | An old Android phone with about 3 GB RAM (the Moto G 5S works) | Anything with a mic, speaker and wi-fi |

- **Local mode is honest about its limits.** It speaks the verdict immediately, says "I'm not sure, check the answer" when its confidence is low, and always reads out the model answer so
  the user can judge for themselves.
- **Hybrid option:** the local check answers first; the API is called only when the local check is unsure (saves cost and keeps most answers off the network).
- **Privacy rules for both modes:** the microphone is open only during an answer window after the duck asks a question (push-to-talk or a tap also works). **No always-on listening
  and no wake phrase by default.** The API key is supplied by the user, stored only on the device, never committed to git, and should carry a spending cap.
- The "hey ducky" wake-word idea is therefore demoted to an opt-in extra that is off by default.

## Hardware decision

| Option | Verdict |
|---|---|
| Raspberry Pi 5 (4/8GB) + USB speakerphone + RTC | Best result, but costs more. Fallback if the phone fails. |
| Old phone | Chosen first: free, has mic, speaker, battery and a USB port. |

### The phone: Moto G (5S) (`montana`)

- Android 8.1.0, **32-bit userland (`armv7l`, ABI `armeabi-v7a`)** even though the SoC
  (Snapdragon 430, MSM8937, 8 x Cortex-A53) is 64-bit capable. Measured max clocks: cpu0 1.40 GHz, cpu4 and cpu7 1.09 GHz
  (probably two clusters of four).
- **Erratum (2026-10-29):** earlier versions of this README, `prompts/research-agent.md` and the two agent research
  reports in `research/` called the phone a Moto G5 Plus (`potter`) with a Snapdragon 625 at about 2 GHz. That was my assumption
  from "moto G5"; the phone itself reports Moto G (5S), `montana`, MSM8937. Every measurement in the benchmark log was taken on
  this real phone, so the numbers stand and only the label was wrong. The research reports still carry the wrong label; their
  armv7 findings hold, but any speed expectation based on a Snapdragon 625 should be dropped.
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
| DS3 | `ds3-independent` (`tests/ds3-independent.transcripts.json`, uses the DS2 quiz) | Written by a different agent from `data/ds3_input.json` (questions, model answers and key points only; no keywords, wrong claims or DS2 answers). 39 questions, 272 answers (6 to 8 per question), 811 labeled points: 520 covered, 142 missing, 149 contradicted (125 answers contain a mistake). Kinds: correct in own words, correct with speech-to-text noise, partial, one subtle mistake, mixed, confident wrong, self-correction (30), hedged wrong (8). 11 mistake types, 5% numeric. Same questions as DS2, so it tests new answers, not new topics. |

**Definitions.** "Flag" = the checker reports a point as missing or wrong. P/R/F1 are for flagging
points whose label is not `covered`. "Contradicted flagged" = contradicted points reported as
not covered; "named" = reported specifically as contradicted. Exact = 3-class accuracy.
**EKM** = Embeddings + Keyword Method: keywords and sentence embeddings combined. Two variants are logged: "either flags" (report a point if either test reports it, the first version) and a single score where each keyword hit adds a boost to the embedding similarity (`cosine + 0.2 to 0.3 x min(hits, 2) >= threshold`, the better version). "Held-out" = threshold fixed from DS1 or tuned on four chapters and tested on the fifth (leave-one-chapter-out, LOCO).

**Device for all runs:** Moto G (5S) (montana), Snapdragon 430 (MSM8937, 8 x Cortex-A53 at 1.4 / 1.1 GHz), 32-bit ARM (armv7l), Android 8.1, Termux,
llama.cpp b1-6c44591 (patched), 4 threads.

**Testing limitations (hardware).**
- **Heat and power.** The runs are long: a full LLM verdict run is about 20 s per answer, so about an hour per model, and the whole chain about
  4 hours. During the first 50 minutes of continuous 4-thread load the battery rose from 36.2 to 39.0 °C (CPU sensors 55 to 65 °C) and was
  flattening, and the charge slowly drifted from 100% to 98% while on a computer's USB port, which supplies less power than the phone draws at full
  load. The battery is from 2017. No throttling was seen, but it was not measured, so timings late in a long run could be affected by heat.
- **Safeguards.** `bench/watchdog.py` runs on the phone, logs battery and CPU temperature every minute (`temp.log`), and kills the chain if the
  battery reaches 45 °C, its health stops reporting GOOD, or the CPU reaches 85 °C (alert level 42 °C). These limits come from general lithium-ion
  practice, not from measurements on this phone. Its own cost is negligible (about 5 CPU-seconds per hour against about 14,000 for the benchmark).
- **Duty cycle.** Run in blocks of about an hour or two and let the phone cool to about 35 °C between blocks (suggestion, not tested). Write the
  battery temperature range into the notes of each run (B5 LFM2 so far: 38.2 to 39.0 °C).
- **Pausing and resuming.** The runs were paused on purpose, not by failure. All steps save every finished answer and resume automatically.
- **Operational pitfall.** `pkill -f <name>` run over SSH also matches the SSH shell's own command line and kills the session; use the
  `[n]ame` pattern trick instead.

### Latest runs

**Cloud-ceiling runs (2026-10-29).** What does a good small hosted model score on the independent set (DS3), given the same information the phone gets? Eight fresh Claude Haiku
agents (no access to this project's context) judged all 272 answers blind: each read one input file of 34 answers (question, key points, transcript; no labels, kinds, personas, mistake types, keywords or wrong
claims) and wrote one output file. I audited their tool calls: each made exactly one Read of its own input and one Write of its own output, nothing else. Scored in the Mac sandbox (`lab/scripts/judge_eval.py`).
The judges are sub-agents, not API calls, so there are no real latency or price figures.

**B15: hosted small model as the judge**

| Run | Date | Dataset | Method | Model / config | Time per answer | RAM | Tokens in / out | Exact | Flag P | Flag R | Flag F1 | Contradicted flagged / named | Notes |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| B15-01 | 2026-10-29 | DS3, blind | Hosted small model as judge | Claude Haiku, 8 sub-agents x 34 answers; sees only question, key points and transcript (what the phone sees); writes a short note on each contradicted point | about 40 s per 34-answer batch (agent overhead included, not a real API latency) | - | about 200 case tokens + a short instruction in, about 40 out per answer (estimate from file sizes) | 0.975 | 0.997 | 0.990 | 0.993 | 148/149 / 138/149 | 6 false 'contradicted': 138 of its 144 'contradicted' calls are right (96%) |

| Same 811 points, same labels | Exact | Flag F1 | Contradictions named | False 'contradicted' |
|---|---|---|---|---|
| Keywords (B11-01) | 0.744 | 0.623 | 0 / 149 | 0 |
| EKM rules, tuned on DS2 (B11-02) | 0.751 | 0.676 | 7 / 149 | 8 |
| Best local: learned combiner + zero-shot NLI (B11-05) | 0.805 | 0.805 | 88 / 149 | 83 |
| Cautious local: gradient boosting + NLI (B11-08) | 0.806 | 0.775 | 57 / 149 | 39 |
| **Hosted small model, Claude Haiku (B15-01)** | **0.975** | **0.993** | **138 / 149** | **6** |

**B16: hybrid routing (phone judges first; the whole answer goes to the API only when the local judge is unsure or flags a problem)**

| Run | Policy (local judge = learned combiner + NLI; hosted judge = B15-01) | Answers sent to the API | Exact | Flag P | Flag R | Flag F1 | Contradicted flagged / named | False 'contradicted' |
|---|---|---|---|---|---|---|---|---|
| B16-01 | local only (never calls the API) | 0% | 0.805 | 0.776 | 0.835 | 0.805 | 105/149 / 88/149 | 83 |
| B16-02 | send when any point's confidence < 0.6 | 36% | 0.901 | 0.912 | 0.924 | 0.918 | 129/149 / 113/149 | 37 |
| B16-03 | send when any point's confidence < 0.7 | 56% | 0.928 | 0.945 | 0.942 | 0.943 | 134/149 / 123/149 | 26 |
| B16-04 | send when any point's confidence < 0.8 | 70% | 0.949 | 0.962 | 0.969 | 0.966 | 142/149 / 130/149 | 16 |
| B16-05 | send when any point's confidence < 0.9 | 90% | 0.969 | 0.993 | 0.983 | 0.988 | 146/149 / 135/149 | 7 |
| B16-06 | send when the local check flags any problem | 70% | 0.954 | 1.000 | 0.928 | 0.963 | 131/149 / 121/149 | 6 |
| B16-07 | send when it flags a problem OR any confidence < 0.8 | 88% | 0.973 | 0.997 | 0.983 | 0.990 | 146/149 / 136/149 | 6 |
| B16-08 | always send to the API | 100% | 0.975 | 0.997 | 0.990 | 0.993 | 148/149 / 138/149 | 6 |

**What B15 and B16 show.**
- **A good small hosted model is in a different league:** exact 0.975, flag F1 0.993, 93% of contradictions named, and 96% of its "contradicted" calls are right. It makes almost no false alarms on correct answers
  (1 of 520 covered points), including paraphrased (0 of 116), speech-to-text-noisy (0 of 116) and self-corrected answers (0 of 89): exactly where the local judge struggled.
- **It also explains.** Its short notes on contradictions read like usable spoken feedback ("Says the load balancer does it, not the API gateway"; "Says requests wait in a queue instead of being dropped").
  Weakest mistake types: wrong_cause (5 of 10 named), invented_detail (16 of 19).
- **Hybrid routing saves less than hoped on this set.** Sending an answer to the API only when the local check flags a problem uses the API for 70% of answers and keeps F1 at 0.963. DS3 is deliberately mistake-heavy
  (46% of answers contain a wrong point), so a real learner who answers mostly correctly would route far fewer answers; that is unmeasured. A confidence-only bar saves little (0.8 sends 70% for F1 0.966).
- **Caveats, read these.** (1) The labels were written by another Claude agent, so label author and judge are the same model family and likely agree more than they would with human-labeled real speech.
  (2) The labels carry roughly 5% noise (2 of 41 sampled answers disagreed on re-labelling), so 0.975 exact is probably close to the ceiling of these labels, not proof of 0.975 on real learners.
  (3) Each judge saw 34 answers per call; a real call judges one. (4) The judges' own summaries contained small errors (one said 38 cases, one said duplicate ids); their output files were checked and are correct.
  (5) No real latency, cost or rate-limit measurement, and no offline behaviour; that needs a real API test.

**Resumed** on 2026-10-29 at 22:24 after the overnight rest (paused 2026-10-28 05:10 for the hardware, see "Testing limitations" below). In progress, results are
filled in per run when each finishes. Planned: B5 small LLM verdicts on all of DS2 (LFM2-350M, Qwen2.5-0.5B, Gemma 3 270M), B6 the pipeline (EKM first, then a small model as
second opinion on routed points only: LFM2 and Qwen), B7 narrator sample (one spoken sentence from the known key point).
Status at 22:50: **B5 LFM2 finished** on all 195 answers (raw file `bench/results/b5_llm_LFM2.json.partial.jsonl`; first numbers: exact 0.617, flag F1 0.579,
contradictions named 7.7%, 22.3 s per answer, 302 MB; the DS1 run scored F1 0.74, so the smaller dataset flattered the LLM too). B6 LFM2 is running. The battery peaked at 39.7 °C;
the charge drifted from 34% to 32% while charging, so a wall charger would help (the watchdog stops the run at 10%).

**Resume:** on the phone start Termux and `sshd`; on the Mac `adb forward tcp:8022 tcp:8022`; copy `bench/chain.sh`, `bench/watchdog.py`
and the `bench/*.py` files to `~/bench` on the phone; then run `python watchdog.py &` and `./chain.sh` there. Every step resumes from its
`*.partial.jsonl` file, and a truncated last line from a killed run is ignored. The chain needs about 4 hours in total.

### Older runs (click to expand)

<details>
<summary>B11 to B14 (2026-10-29): independent mistakes (DS3), authored wrong claims, abstention, effect of adding independent training data</summary>

**Independent-data runs (2026-10-29).** DS3 is a second set of answers for the same 39 questions and 116 key points, written by a different agent that never saw the
wrong-claim lists, keywords or DS2 answers: 272 answers, 811 labeled points (520 covered, 142 missing, 149 contradicted), with persona, kind of answer and mistake type recorded.
Everything below is fitted on DS2 and scored on DS3 (or leave-one-chapter-out where stated), in the Mac sandbox, so there are no phone speed or memory figures. The embedding step was
recomputed in the sandbox (PyTorch, full precision) and reproduces the phone's B3-04 result on DS2 (exact 0.886, F1 0.866 vs 0.864).

**B11: models fitted on DS2, scored on DS3**

| Run | Date | Dataset | Method | Model / config | Time per answer | RAM | Tokens in / out | Exact | Flag P | Flag R | Flag F1 | Contradicted flagged / named | Notes |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| B11-01 | 2026-10-29 | DS3, fitted on DS2 | Keywords | at least 1 hit | Mac only | - | - | 0.744 | 0.797 | 0.512 | 0.623 | 28/149 / 0/149 | Finds only half of the problems (recall 0.51) |
| B11-02 | 2026-10-29 | DS3, fitted on DS2 | EKM rules (B3-04 method) | keyword boost + wrong claims, thresholds tuned on DS2 | Mac only | - | - | 0.751 | 0.777 | 0.598 | 0.676 | 41/149 / 7/149 | 8 false 'contradicted'. On DS2 the same method named 51% of contradictions, here 5% |
| B11-03 | 2026-10-29 | DS3, fitted on DS2 | Learned combiner (LR) | cheap features, no wrong-claim info | Mac only | - | - | 0.679 | 0.601 | 0.787 | 0.682 | 90/149 / 72/149 | 162 false 'contradicted' |
| B11-04 | 2026-10-29 | DS3, fitted on DS2 | Learned combiner (LR) | + wrong-claim embedding match | Mac only | - | - | 0.74 | 0.683 | 0.687 | 0.685 | 65/149 / 48/149 | 77 false |
| B11-05 | 2026-10-29 | DS3, fitted on DS2 | Learned combiner (LR) | no wrong-claim info + zero-shot NLI key point (P1, P2) | Mac only | - | - | 0.805 | 0.776 | 0.835 | 0.805 | 105/149 / 88/149 | **Best F1.** 83 false 'contradicted': 88 of its 171 'contradicted' calls are right |
| B11-06 | 2026-10-29 | DS3, fitted on DS2 | Learned combiner (LR) | + wrong-claim match + NLI (P1, P2) | Mac only | - | - | 0.798 | 0.779 | 0.777 | 0.778 | 88/149 / 66/149 | 64 false |
| B11-07 | 2026-10-29 | DS3, fitted on DS2 | Learned combiner (LR) | everything incl. NLI sentence vs wrong claim (P3) | Mac only | - | - | 0.813 | 0.88 | 0.653 | 0.75 | 62/149 / 40/149 | 15 false. Best exact; lower recall |
| B11-08 | 2026-10-29 | DS3, fitted on DS2 | Learned combiner (GBM) | no wrong-claim info + NLI (P1, P2) | Mac only | - | - | 0.806 | 0.852 | 0.711 | 0.775 | 81/149 / 57/149 | 39 false. More conservative than the LR |
| B11-09 | 2026-10-29 | DS3, fitted on DS2 | Learned combiner (GBM) | + wrong-claim match + NLI (P1, P2) | Mac only | - | - | 0.806 | 0.89 | 0.667 | 0.762 | 65/149 / 35/149 | 18 false |
| B11-10 | 2026-10-29 | DS3, fitted on DS2 | Learned combiner (GBM) | everything incl. P3 | Mac only | - | - | 0.809 | 0.894 | 0.663 | 0.761 | 68/149 / 38/149 | 16 false |
| B11-11 | 2026-10-29 | DS3, fitted on DS2 | Learned combiner (LR) | no class weights, + wrong claims + NLI (P1, P2) | Mac only | - | - | 0.805 | 0.899 | 0.615 | 0.731 | 58/149 / 36/149 | 11 false. Highest precision, lowest recall |
| B11-12 | 2026-10-29 | DS3, fitted on DS2 | Zero-shot NLI alone | sentence premise (P1) | Mac only | - | - | 0.576 | 0.551 | 0.918 | 0.688 | 130/149 / 69/149 | 75 false 'contradicted' |
| B11-13 | 2026-10-29 | DS3, fitted on DS2 | Zero-shot NLI alone | whole-answer premise (P2) | Mac only | - | - | 0.55 | 0.52 | 0.907 | 0.661 | 129/149 / 68/149 | 82 false |

**B12: do the authored wrong claims cover real mistakes?**

| Contradicted points | n | Mean similarity to the nearest authored wrong claim | Share with similarity >= 0.75 | Share where NLI says the sentence entails a wrong claim |
|---|---|---|---|---|
| DS2 (built from the wrong claims) | 39 | 0.80 | 64% | 87% |
| DS3 (independent) | 149 | 0.58 | 15% | 15% |
| DS3 points that are not contradictions | 662 | 0.52 | 8% | - |

**B13: abstention on DS3 (bar chosen on DS2 data, applied to DS3)**

| Run | Model (fitted on DS2, scored on DS3) | Confidence bar | Answered | Accuracy on answered | Needed-feedback points still called 'covered' (of 291) | Flag recall (unsure = flagged) | Flag precision |
|---|---|---|---|---|---|---|---|
| B13-01 | LR, no wrong-claim info + NLI (P1, P2) | 0.0 | 100% | 0.805 | 48 | 0.835 | 0.776 |
| B13-02 | LR, no wrong-claim info + NLI (P1, P2) | 0.6 | 86% | 0.854 | 32 | 0.890 | 0.713 |
| B13-03 | LR, no wrong-claim info + NLI (P1, P2) | 0.7 | 74% | 0.876 | 25 | 0.914 | 0.639 |
| B13-04 | LR, no wrong-claim info + NLI (P1, P2) | 0.8 | 62% | 0.889 | 20 | 0.931 | 0.578 |
| B13-05 | LR, no wrong-claim info + NLI (P1, P2) | 0.9 | 40% | 0.925 | 9 | 0.969 | 0.475 |
| B13-06 | LR, + wrong-claim match + NLI (P1, P2) | 0.0 | 100% | 0.798 | 65 | 0.777 | 0.779 |
| B13-07 | LR, + wrong-claim match + NLI (P1, P2) | 0.6 | 90% | 0.833 | 50 | 0.828 | 0.742 |
| B13-08 | LR, + wrong-claim match + NLI (P1, P2) | 0.7 | 82% | 0.854 | 41 | 0.859 | 0.696 |
| B13-09 | LR, + wrong-claim match + NLI (P1, P2) | 0.8 | 71% | 0.891 | 28 | 0.904 | 0.651 |
| B13-10 | LR, + wrong-claim match + NLI (P1, P2) | 0.9 | 55% | 0.922 | 18 | 0.938 | 0.563 |

**B14: does adding independent data to training help? (leave-one-chapter-out, scored on DS3)**

| Run | Date | Dataset | Method | Model / config | Time per answer | RAM | Tokens in / out | Exact | Flag P | Flag R | Flag F1 | Contradicted flagged / named | Notes |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| B14-01 | 2026-10-29 | DS3, fitted on DS2 | Does independent data help? | LR + wrong-claim match + NLI (P1, P2); trained on DS2 only (leave-one-chapter-out) | Mac only | - | - | 0.787 | 0.763 | 0.773 | 0.768 | 93/149 / 68/149 | 66 false |
| B14-02 | 2026-10-29 | DS3, fitted on DS2 | Does independent data help? | same, trained on DS2 + DS3 from the other chapters | Mac only | - | - | 0.811 | 0.754 | 0.876 | 0.811 | 116/149 / 100/149 | 84 false. +0.04 F1 and 32 more contradictions named from a small amount of diverse data |

**What B11 to B14 show (read this before trusting any DS2 number).**
- **The DS2 results were optimistic.** On independent mistakes the best DS2 method (EKM keyword boost + wrong claims, B3-04) drops from exact 0.886 / F1 0.864 / 51% of contradictions named to
  exact 0.751 / F1 0.676 / 5% named. Keywords alone find only half of the problems (recall 0.51).
- **The authored wrong claims are a weak detector of real mistakes.** Only 15% of the independent mistakes are close to any authored wrong claim (64% on DS2), and NLI says the speaker asserted
  a wrong claim for 15% (87% on DS2). The earlier "36 of 39 named" (B10-12) came from how my test answers were built.
- **The best honest model so far is a learned combiner with zero-shot NLI scores as features:** exact 0.805, flag F1 0.805, 59% of contradictions named. But about half of its "contradicted" calls are
  wrong (88 right, 83 false), so it is good at saying "this does not match the answer" and not yet at saying "you said the opposite". The gradient-boosting version is more cautious (57 right, 39 false).
- **Abstention is miscalibrated under a shift.** On DS2 a bar of 0.7 left 4 of 219 problem points called "covered"; on DS3 it still leaves 25 of 291. It helps, but it cannot be trusted to be as safe as B9 suggested.
- **More diverse training data helps a lot:** adding DS3 answers from the other chapters raises F1 from 0.768 to 0.811 and named contradictions from 68 to 100 of 149. That supports training the combiner or a
  cross-encoder on many kinds of mistakes, across more topics.
- **Where it still fails** (best LR, full tables in `lab/out/transfer_report.txt`): correct answers in the speaker's own words are falsely flagged 25 of 116 times and after a self-correction 14 of 89;
  confident wrong answers get 20 of 101 problem points called "covered"; swapped terms are the hardest mistake type (10 of 22 flagged).
- **Caveats:** same questions and key points as DS2 (new answers, not new topics); DS3 is the work of one agent (its blind re-label disagreed on 2 of 41 sampled cases, so roughly 5% label noise); no phone timings.

</details>

<details>
<summary>B8 to B10 (2026-10-29): learned combiner, abstention and zero-shot NLI on DS2 (author-written data; optimistic, see B11 to B14)</summary>

**Mac sandbox runs (2026-10-29).** Accuracy experiments on the saved phone scores, run in the Docker sandbox described below, so no speed or
memory figures apply to the phone. All use leave-one-chapter-out on DS2 (580 points). Reference: EKM stage 1 with keyword boost and wrong claims
(B3-04) has exact 0.886, flag F1 0.864, 20 of 39 contradictions named, 7 false 'contradicted'.

**B8: learned combiner (logistic regression and gradient boosting over the cheap scores, plus ablations)**

| Run | Date | Dataset | Method | Model / config | Time per answer | RAM | Tokens in / out | Exact | Flag P | Flag R | Flag F1 | Contradicted flagged / named | Notes |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| B8-01 | 2026-10-29 | DS2, LOCO | Learned combiner | logistic regression, similarity + keyword hits only | Mac only | - | - | 0.767 | 0.685 | 0.954 | 0.798 | 29/39 / 22/39 | 97 false 'contradicted' |
| B8-02 | 2026-10-29 | DS2, LOCO | Learned combiner | + wrong-claim embedding match | Mac only | - | - | 0.84 | 0.757 | 0.941 | 0.839 | 31/39 / 29/39 | 55 false 'contradicted'. The wrong-claim group matters most |
| B8-03 | 2026-10-29 | DS2, LOCO | Learned combiner | + spoken-number mismatch rule | Mac only | - | - | 0.836 | 0.751 | 0.936 | 0.833 | 30/39 / 28/39 | 57 false. Numbers added nothing on DS2 |
| B8-04 | 2026-10-29 | DS2, LOCO | Learned combiner | + negation flip | Mac only | - | - | 0.841 | 0.765 | 0.936 | 0.842 | 30/39 / 27/39 | 55 false |
| B8-05 | 2026-10-29 | DS2, LOCO | Learned combiner | all cheap features (+ antonym pairs), C=1, balanced classes | Mac only | - | - | 0.845 | 0.769 | 0.941 | 0.846 | 31/39 / 28/39 | 53 false. Dropping wrong claims: exact .809, 20/39 named; dropping numbers: exact .855 |
| B8-06 | 2026-10-29 | DS2, LOCO | Learned combiner | same, C=0.1 | Mac only | - | - | 0.857 | 0.78 | 0.954 | 0.858 | 32/39 / 30/39 | 48 false |
| B8-07 | 2026-10-29 | DS2, LOCO | Learned combiner | same, no class weights | Mac only | - | - | 0.888 | 0.871 | 0.863 | 0.867 | 22/39 / 18/39 | 11 false. Equal to the hand-tuned rules (B3-04: .886 / .864) |
| B8-08 | 2026-10-29 | DS2, LOCO | Learned combiner | gradient boosting, depth 3 | Mac only | - | - | 0.897 | 0.857 | 0.904 | 0.88 | 27/39 / 24/39 | 17 false. About +0.01 over the hand-tuned rules |

**B9: abstention (say "unsure" below a confidence bar)**

| Run | Model (leave-one-chapter-out, DS2, 580 points) | Confidence bar | Answered | Accuracy on answered | Needed-feedback points still called 'covered' (of 219) | Flag recall (unsure counts as flagged) | Flag precision |
|---|---|---|---|---|---|---|---|
| B9-01 | cheap features incl. wrong-claim embedding match | 0.0 | 100% | 0.845 | 13 | 0.941 | 0.769 |
| B9-02 | cheap features incl. wrong-claim embedding match | 0.5 | 98% | 0.860 | 11 | 0.950 | 0.765 |
| B9-03 | cheap features incl. wrong-claim embedding match | 0.6 | 91% | 0.892 | 9 | 0.959 | 0.727 |
| B9-04 | cheap features incl. wrong-claim embedding match | 0.7 | 84% | 0.909 | 4 | 0.982 | 0.689 |
| B9-05 | cheap features incl. wrong-claim embedding match | 0.8 | 73% | 0.924 | 3 | 0.986 | 0.619 |
| B9-06 | cheap features incl. wrong-claim embedding match | 0.9 | 57% | 0.948 | 1 | 0.995 | 0.545 |
| B9-07 | no wrong-claim info: cheap + NLI key point (P1, P2) | 0.0 | 100% | 0.843 | 13 | 0.941 | 0.777 |
| B9-08 | no wrong-claim info: cheap + NLI key point (P1, P2) | 0.5 | 97% | 0.861 | 11 | 0.950 | 0.762 |
| B9-09 | no wrong-claim info: cheap + NLI key point (P1, P2) | 0.6 | 90% | 0.883 | 8 | 0.963 | 0.730 |
| B9-10 | no wrong-claim info: cheap + NLI key point (P1, P2) | 0.7 | 81% | 0.901 | 6 | 0.973 | 0.672 |
| B9-11 | no wrong-claim info: cheap + NLI key point (P1, P2) | 0.8 | 70% | 0.933 | 5 | 0.977 | 0.617 |
| B9-12 | no wrong-claim info: cheap + NLI key point (P1, P2) | 0.9 | 48% | 0.946 | 3 | 0.986 | 0.507 |
| B9-13 | everything incl. NLI vs wrong claim (P3), optimistic | 0.0 | 100% | 0.940 | 6 | 0.973 | 0.887 |
| B9-14 | everything incl. NLI vs wrong claim (P3), optimistic | 0.5 | 99% | 0.939 | 6 | 0.973 | 0.877 |
| B9-15 | everything incl. NLI vs wrong claim (P3), optimistic | 0.6 | 96% | 0.953 | 6 | 0.973 | 0.866 |
| B9-16 | everything incl. NLI vs wrong claim (P3), optimistic | 0.7 | 93% | 0.970 | 1 | 0.995 | 0.858 |
| B9-17 | everything incl. NLI vs wrong claim (P3), optimistic | 0.8 | 89% | 0.977 | 1 | 0.995 | 0.826 |
| B9-18 | everything incl. NLI vs wrong claim (P3), optimistic | 0.9 | 84% | 0.982 | 0 | 1.000 | 0.779 |

**B10: zero-shot NLI cross-encoder, as judge, as second opinion, and as extra features**

| Run | Date | Dataset | Method | Model / config | Time per answer | RAM | Tokens in / out | Exact | Flag P | Flag R | Flag F1 | Contradicted flagged / named | Notes |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| B10-01 | 2026-10-29 | DS2, LOCO | Zero-shot NLI alone | nli-MiniLM2-L6-H768 (82M), premise = closest sentence, hypothesis = key point | 31 ms/pair (Mac) | - | - | 0.586 | 0.536 | 0.977 | 0.693 | 34/39 / 22/39 | 61 false 'contradicted'. Calls most correct answers 'neutral' (162 of 361). 31 ms per pair on the Mac, not measured on the phone |
| B10-02 | 2026-10-29 | DS2, LOCO | Zero-shot NLI alone | premise = whole spoken answer | 49 ms/pair (Mac) | - | - | 0.581 | 0.537 | 0.982 | 0.695 | 35/39 / 25/39 | 85 false 'contradicted'. 49 ms per pair on the Mac |
| B10-03 | 2026-10-29 | DS2, LOCO | NLI: asserted the typical mistake? | premise = sentence closest to the wrong claim, hypothesis = wrong claim, entailment >= 0.5 | 30 ms/pair (Mac) | - | - | - | - | - | - | - / 34/39 | 6 false alarms among the other 541 points. Optimistic: the wrong answers were built from these wrong claims |
| B10-04 | 2026-10-29 | DS2, LOCO | EKM stage 1 + NLI second opinion | NLI (sentence) vetoes flags | Mac only | - | - | 0.907 | 0.882 | 0.886 | 0.884 | 21/39 / 18/39 | 4 false 'contradicted'. +0.02 exact over EKM alone (B3-04) |
| B10-05 | 2026-10-29 | DS2, LOCO | EKM stage 1 + NLI second opinion | vetoes flags and escalates borderline points | Mac only | - | - | 0.903 | 0.852 | 0.918 | 0.884 | 23/39 / 20/39 | 6 false |
| B10-06 | 2026-10-29 | DS2, LOCO | EKM stage 1 + NLI second opinion | whole-answer premise, vetoes flags | Mac only | - | - | 0.905 | 0.874 | 0.89 | 0.882 | 22/39 / 19/39 | 3 false |
| B10-07 | 2026-10-29 | DS2, LOCO | EKM stage 1 + NLI second opinion | NLI names contradictions | Mac only | - | - | 0.824 | 0.826 | 0.909 | 0.865 | 26/39 / 24/39 | 53 false: far too many |
| B10-08 | 2026-10-29 | DS2, LOCO | EKM stage 1 + NLI second opinion | NLI decides on routed points | Mac only | - | - | 0.831 | 0.852 | 0.918 | 0.884 | 23/39 / 16/39 | 47 false |
| B10-09 | 2026-10-29 | DS2, LOCO | Stacking (LR) | similarity + keyword hits + NLI key point (P1, P2), no wrong-claim info | Mac only | - | - | 0.834 | 0.765 | 0.95 | 0.847 | 29/39 / 24/39 | 63 false |
| B10-10 | 2026-10-29 | DS2, LOCO | Stacking (gradient boosting) | cheap features + NLI (P1, P2), no wrong-claim info | Mac only | - | - | 0.893 | 0.886 | 0.886 | 0.886 | 24/39 / 18/39 | 20 false. Honest comparison to B3-04: about the same |
| B10-11 | 2026-10-29 | DS2, LOCO | Stacking (LR) | wrong-claim embedding match + NLI (P1, P2) | Mac only | - | - | 0.881 | 0.81 | 0.954 | 0.876 | 33/39 / 30/39 | 39 false |
| B10-12 | 2026-10-29 | DS2, LOCO | Stacking (LR) | everything incl. NLI sentence vs wrong claim (P3) | Mac only | - | - | 0.94 | 0.887 | 0.973 | 0.928 | 37/39 / 36/39 | 10 false. OPTIMISTIC: P3 mirrors how the test answers were built |
| B10-13 | 2026-10-29 | DS2, LOCO | Stacking (LR) | similarity + keyword hits + NLI incl. P3 | Mac only | - | - | 0.943 | 0.895 | 0.977 | 0.934 | 38/39 / 37/39 | 10 false. OPTIMISTIC, same reason |

**What B8 to B10 show.**
- The learned combiner is no better than the hand-tuned rules (best honest result: exact 0.897 vs 0.886). Its value is a probability, which makes abstention
  possible. The spoken-number rule added nothing on DS2 (only a few numeric contradictions); it passes its own 20-case self-test and costs nothing, so it stays optional.
- Abstention works. At a bar of 0.7 the duck answers 84% of points with accuracy 0.909, and wrong "covered" answers on points that needed feedback fall from 13 to 4
  of 219 (recall 0.982), at the price of more "please double-check" prompts (precision 0.769 to 0.689).
- Zero-shot NLI is a poor judge on its own (exact 0.58) because our key points are compound sentences and it calls correct answers "neutral". As a second opinion that
  can only veto false alarms it helps a little (exact +0.02, fewer false contradictions).
- The large gain (exact 0.94, 36 of 39 named) comes from asking NLI whether the speaker's sentence entails the authored wrong claim (P3). That matches how the
  test answers were constructed, so treat it as an upper bound. Without any wrong-claim information the best result is about equal to the existing rules.
- The open question is now clear: how many real mistakes fall outside the authored `wrong_claims`. It needs a dataset of mistakes written by someone who has not seen the lists.

</details>

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

## Lab: sandboxed Mac experiments

Experiments that need Python packages or model weights from the internet (scikit-learn, PyTorch, the NLI model) run in a Docker container so nothing
untrusted touches the Mac. Everything is in `lab/`.

- **Build** (`docker build -t duck-lab:1 lab/`, the only step with network access): Python 3.12, packages installed as pre-built wheels only (no package can run
  an install script), model weights downloaded as `safetensors` only (no pickle files that could run code). All of it executes inside Docker's Linux VM.
- **Run** (`lab/run.sh scripts/<name>.py`): no network, read-only root filesystem, no Linux capabilities, no privilege escalation, non-root user, 4 GB and 4 CPUs max.
  Only `data/`, `tests/`, `bench/` and `lab/scripts/` are visible, read-only; the only writable host folder is `lab/out/`. The PDF and the rest of the home folder are not visible.
- **Verified** with `lab/scripts/isolation_test.py`: internet and DNS blocked, writes to the root filesystem and to every mounted folder except `lab/out` refused, the Mac's
  `/Users` and the PDF not visible, no capabilities, `setuid(0)` refused.
- **Scripts:** `textfeat.py` (spoken numbers, number mismatch, negation, antonyms; pure Python, runs on the phone unchanged), `combiner.py` (B8, B9), `nli.py` (B10).
  Reports are in `lab/out/*_report.txt`.
- **Caveats:** accuracy only. Timings from the container are not phone timings (the 82M NLI model takes 31 ms per pair on the Mac; the phone will be much slower and must be measured).
  The first image build stalled on the model download because the old pinned Hugging Face client mishandles its newer storage; a newer client with timeouts fixed it.
  The NLI model is `cross-encoder/nli-MiniLM2-L6-H768` (Apache-2.0), taken from the `main` branch at build time and not pinned to a commit.

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
- [ ] **(Demoted: opt-in only, off by default, for privacy. Default is mic open only during an answer window.)** Always-on microphone with a wake phrase ("hey ducky"). Idea: a tiny always-listening keyword spotter wakes the
      duck, then speech-to-text captures a command or a question. Routing without the slow model: match the command
      against a short intent list and match topic names or question text with the same embeddings (about 0.5 s), so
      "ask me something about caches" picks a topic without an LLM. Only free-form chat goes to the small model
      (slow, about 10 s or more) or to the API tier. Things to check: a wake-word engine that really runs on 32-bit ARM
      in Termux (Vosk with a restricted grammar, pocketsphinx keyword spotting, others); CPU use while idle;
      false triggers and self-triggering from the duck's own speech; whether background microphone access works on
      Android 8.1 (it is restricted from Android 9, so this old phone may help); privacy (everything stays local).
- [x] Mac sandbox set up and verified (`lab/`); learned combiner, abstention and zero-shot NLI tested (B8 to B10).
- [x] **Independent mistakes dataset (DS3).** DONE: it showed the DS2 results were optimistic (B11 to B14). Prompt: `prompts/make-independent-mistakes.md` (input: `data/ds3_input.json`, which has no wrong claims or keywords; expected output `tests/ds3-independent.transcripts.json`). Have a different agent write wrong answers for the DS2 key points WITHOUT seeing `wrong_claims`
      (plus paraphrased and noisy correct answers), then re-run B8 to B10. This decides how much of the wrong-claim and NLI gain is real.
- [ ] **Cloud mode.** (1) DONE for Claude Haiku via sub-agents (B15, B16): F1 0.993 on DS3. Still to do: a real API test for latency and price, and a second small model (for example Gemini Flash-Lite on the free tier) for comparison. Original plan: run a small hosted model (for example Claude Haiku, or Gemini Flash-Lite on the free tier) as the judge on DS3 with the same labels and compare F1,
      contradictions named, false 'contradicted' calls, cost and latency against B11. (2) Design the text-only request (key point + transcript sentence, never audio). (3) Hybrid routing: call the
      API only when the local check is unsure. (4) Key handling: user supplied, on-device only, spending cap, not in git. (5) Offline fallback to local mode.
- [ ] **Real API test.** Call a hosted small model one answer at a time with a real key (spending cap): measure latency, tokens, price, rate limits and failures; compare 2 or 3 models; test human-written or real spoken answers
      (labels from a person) so the judge and the label author are not the same model family.
- [ ] **Decide the hybrid policy** with a realistic mix of answers (mostly correct): how many answers actually need the API when the local check flags a problem.
- [ ] **Local mode UX for an advisory judge:** speak the verdict at once, say "unsure" below a confidence bar, always read the model answer, offer self-grading.
- [ ] **Decide what to do with the long phone LLM benchmark chain** (B5 to B7): the small LLMs are already clearly weak; the remaining Qwen and Gemma runs mostly confirm that.
- [ ] **New topics, not just new answers.** DS3 reuses the DS2 questions. Build a dataset from different chapters or another book (different agent, same independence rules) and test the combiner on it.
- [ ] **Train on diverse data.** B14 shows a small amount of independent data helps a lot. Generate many more mistakes per key point and train the combiner, then fine-tune the cross-encoder, leave-one-topic-out.
- [ ] **Spoken feedback policy from B11:** say "that does not match the answer" when unsure and only "you said the opposite" when the model is confident; use abstention but do not trust its bar blindly.
- [ ] Refresh `prompts/research-agent.md` and the research reports with B2 to B14 (they quote the early DS1 numbers).
- [ ] **Fine-tune the cross-encoder** (nli-MiniLM2 82M, and a 22M MiniLM variant) on DS3-style data, evaluate leave-one-topic-out; then convert to GGUF and time it on the phone
      (check that llama.cpp returns the 3 logits). Zero-shot is not enough alone (B10-01/02).
- [ ] Add multi-reference paraphrases per key point and an "unsure" spoken response (B9) to the design; measure paraphrase (variant E) false flags.
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
