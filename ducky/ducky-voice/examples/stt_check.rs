//! Transcribes a 16-bit PCM mono 16 kHz WAV (e.g. from `say -o x.wav --data-format=LEI16@16000`).
//! Usage: cargo run --release -p ducky-voice --example stt_check -- <model.bin> <file.wav>
use ducky_voice::Stt;
use std::time::Instant;

fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let bytes = std::fs::read(&a[1])?;
    // Skip the 44-byte canonical WAV header (the files this check uses are plain PCM).
    let audio: Vec<f32> = bytes[44..].chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0).collect();
    let t = Instant::now();
    let stt = Stt::load(std::path::Path::new(&a[0]))?;
    println!("model load {:.2}s, audio {:.1}s", t.elapsed().as_secs_f32(), audio.len() as f32 / 16000.0);
    let t = Instant::now();
    println!("text: {:?} ({:.2}s)", stt.transcribe(&audio)?, t.elapsed().as_secs_f32());
    Ok(())
}
