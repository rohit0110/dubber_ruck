//! Speaking (macOS `say`), microphone capture (cpal) and speech-to-text (whisper.cpp).
//! The microphone is only open between `Recorder::start` and `Recorder::stop`; nothing listens in the background.
use anyhow::{bail, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::path::Path;
use std::process::{Child, Command};
use std::sync::{Arc, Mutex};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub const WHISPER_RATE: u32 = 16_000;

/// Reads text aloud with the system voice. A new line cuts off the previous one.
pub struct Speaker {
    child: Option<Child>,
    voice: String,
}

impl Speaker {
    /// `voice` is a `say -v` voice name; empty means the system default.
    pub fn new(voice: &str) -> Speaker {
        Speaker { child: None, voice: voice.trim().to_string() }
    }

    pub fn say(&mut self, text: &str) {
        self.stop();
        let mut cmd = Command::new("say");
        if !self.voice.is_empty() {
            cmd.args(["-v", &self.voice]);
        }
        // Failing to speak (no `say`, no audio device, unknown voice) must never break the quiz.
        self.child = cmd.arg("--").arg(text).spawn().ok();
    }

    pub fn stop(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

impl Drop for Speaker {
    fn drop(&mut self) {
        self.stop();
    }
}

/// An open microphone. Dropping it closes the mic.
pub struct Recorder {
    stream: cpal::Stream,
    samples: Arc<Mutex<Vec<f32>>>,
    rate: u32,
}

/// Mixes interleaved frames down to mono.
pub fn downmix(data: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return data.to_vec();
    }
    data.chunks_exact(channels).map(|f| f.iter().sum::<f32>() / channels as f32).collect()
}

/// Linear resampling to 16 kHz, which is plenty for speech.
pub fn to_16k(samples: &[f32], rate: u32) -> Vec<f32> {
    if rate == WHISPER_RATE || samples.is_empty() {
        return samples.to_vec();
    }
    let ratio = rate as f64 / WHISPER_RATE as f64;
    let n = (samples.len() as f64 / ratio) as usize;
    (0..n)
        .map(|i| {
            let pos = i as f64 * ratio;
            let j = pos as usize;
            let frac = (pos - j as f64) as f32;
            let a = samples[j];
            let b = *samples.get(j + 1).unwrap_or(&a);
            a + (b - a) * frac
        })
        .collect()
}

impl Recorder {
    pub fn start() -> Result<Recorder> {
        let device = cpal::default_host().default_input_device().context("no microphone found")?;
        let config = device.default_input_config().context("cannot read microphone settings")?;
        let (rate, channels) = (config.sample_rate(), config.channels() as usize);
        let samples = Arc::new(Mutex::new(Vec::new()));
        let sink = samples.clone();
        let err_fn = |e: cpal::Error| eprintln!("microphone error: {e}");
        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => device.build_input_stream(
                config.into(),
                move |d: &[f32], _: &_| sink.lock().unwrap().extend(downmix(d, channels)),
                err_fn,
                None,
            )?,
            cpal::SampleFormat::I16 => device.build_input_stream(
                config.into(),
                move |d: &[i16], _: &_| {
                    let f: Vec<f32> = d.iter().map(|&s| s as f32 / 32768.0).collect();
                    sink.lock().unwrap().extend(downmix(&f, channels))
                },
                err_fn,
                None,
            )?,
            other => bail!("unsupported microphone sample format {other:?}"),
        };
        stream.play()?;
        Ok(Recorder { stream, samples, rate })
    }

    /// Seconds recorded so far.
    pub fn seconds(&self) -> f32 {
        self.samples.lock().unwrap().len() as f32 / self.rate as f32
    }

    /// Closes the mic and returns 16 kHz mono audio.
    pub fn stop(self) -> Vec<f32> {
        drop(self.stream);
        let raw = std::mem::take(&mut *self.samples.lock().unwrap());
        to_16k(&raw, self.rate)
    }
}

/// Whisper speech-to-text. Loading the model takes about a second, so keep one around.
pub struct Stt {
    ctx: WhisperContext,
}

impl Stt {
    pub fn load(model: &Path) -> Result<Stt> {
        if !model.exists() {
            bail!(
                "speech model missing at {}.\nDownload it with: curl -L -o '{}' https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin",
                model.display(),
                model.display()
            );
        }
        let ctx = WhisperContext::new_with_params(model, WhisperContextParameters::default())
            .map_err(|e| anyhow::anyhow!("cannot load speech model: {e}"))?;
        Ok(Stt { ctx })
    }

    /// Audio must be 16 kHz mono. Returns an empty string for silence or clips under half a second.
    pub fn transcribe(&self, audio: &[f32]) -> Result<String> {
        if audio.len() < WHISPER_RATE as usize / 2 || rms(audio) < 0.002 {
            return Ok(String::new());
        }
        let mut state = self.ctx.create_state().map_err(|e| anyhow::anyhow!("{e}"))?;
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some("en"));
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        state.full(params, audio).map_err(|e| anyhow::anyhow!("transcription failed: {e}"))?;
        let text: String = state.as_iter().map(|s| format!("{s}")).collect::<Vec<_>>().join(" ");
        Ok(clean_transcript(&text))
    }
}

fn rms(a: &[f32]) -> f32 {
    (a.iter().map(|x| x * x).sum::<f32>() / a.len().max(1) as f32).sqrt()
}

/// Removes whisper's non-speech markers like [BLANK_AUDIO] or (music) and tidies spaces.
pub fn clean_transcript(t: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for c in t.chars() {
        match c {
            '[' | '(' => depth += 1,
            ']' | ')' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downmix_averages_channels() {
        assert_eq!(downmix(&[1.0, 3.0, 2.0, 4.0], 2), vec![2.0, 3.0]);
        assert_eq!(downmix(&[1.0, 2.0], 1), vec![1.0, 2.0]);
    }

    #[test]
    fn resamples_to_16k_length() {
        let one_second_48k = vec![0.5f32; 48_000];
        let out = to_16k(&one_second_48k, 48_000);
        assert!((out.len() as i64 - 16_000).abs() <= 1);
        assert!(out.iter().all(|&x| (x - 0.5).abs() < 1e-6));
        assert_eq!(to_16k(&[0.1, 0.2], 16_000), vec![0.1, 0.2]);
    }

    #[test]
    fn strips_non_speech_markers() {
        assert_eq!(clean_transcript(" [BLANK_AUDIO] "), "");
        assert_eq!(clean_transcript("It is a  bucket (music) of tokens [ding]"), "It is a bucket of tokens");
    }
}
