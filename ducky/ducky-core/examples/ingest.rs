//! Usage: cargo run --release --example ingest -- <pdf> <first_page> <last_page> <topic> <out.json> [model=haiku]
use ducky_core::{budget::Metered, ingest, provider::ClaudeCli};
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    anyhow::ensure!(a.len() >= 5, "usage: ingest <pdf> <first> <last> <topic> <out.json> [model]");
    let model = a.get(5).map(String::as_str).unwrap_or("haiku");
    let pages = ingest::extract_pages(Path::new(&a[0]))?;
    let r = ingest::ingest(&Metered { inner: &ClaudeCli, task: "ingest" }, model, &a[3], &pages, a[1].parse()?, a[2].parse()?, 6000, &mut |d, t| {
        eprintln!("chunk {}/{t}", d + 1);
        true
    })?;
    std::fs::write(&a[4], serde_json::to_string_pretty(&r.quiz)?)?;
    println!(
        "{} questions; tokens {} in / {} out; cost ${:.4}; failed chunks: {:?}",
        r.quiz.questions.len(), r.tokens_in, r.tokens_out, r.cost_usd, r.failed_chunks
    );
    Ok(())
}
