//! Opens the mic for 2 seconds and prints how much audio arrived and how loud it was. Saves nothing.
use ducky_voice::Recorder;

fn main() -> anyhow::Result<()> {
    let r = Recorder::start()?;
    std::thread::sleep(std::time::Duration::from_secs(2));
    let audio = r.stop();
    let rms = (audio.iter().map(|x| x * x).sum::<f32>() / audio.len().max(1) as f32).sqrt();
    println!("got {:.1}s of 16 kHz audio, level {:.4}", audio.len() as f32 / 16000.0, rms);
    Ok(())
}
