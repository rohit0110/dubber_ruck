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
5. **Check (fast path):** fuzzy keyword match of the transcript against `key_points`,
   then speak the missed or wrong points immediately. No LLM needed.
6. **Check (slow path, optional):** a small LLM (Qwen2.5-0.5B, Q4) via llama.cpp for
   clarification questions or answers that match no key point.
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
      in `#if 0 ... #endif`. Rebuild in progress with `-j2`.
- [ ] Download Qwen2.5-0.5B GGUF and benchmark tokens/second on the phone.
- [ ] Choose speech-to-text for 32-bit ARM.
- [ ] Define quiz JSON schema and an agent prompt for generating it.
- [ ] Write the scheduler + quiz loop.
- [ ] Debloat Android via `adb shell pm disable-user --user 0 <package>` (keep the
      TTS engine and system UI).
- [ ] Autostart on boot, power-loss behaviour, case/enclosure.

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
