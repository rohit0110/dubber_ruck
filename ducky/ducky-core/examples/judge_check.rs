//! Judge a sample of DS3 cases through the Claude CLI and score against the labels.
//! Usage: cargo run --example judge_check -- [n_cases=10] [model=haiku]
use ducky_core::{budget::Metered, judge::{judge, Verdict}, provider::ClaudeCli, quiz::Quiz};
use serde_json::Value;
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let n: usize = args.next().map(|s| s.parse()).transpose()?.unwrap_or(10);
    let model = args.next().unwrap_or_else(|| "haiku".into());
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let quiz = Quiz::load(&root.join("data/sdi-ch4-8.quiz.json"))?;
    let cases: Value = serde_json::from_str(&std::fs::read_to_string(root.join("tests/ds3-independent.transcripts.json"))?)?;
    let (mut right, mut total, mut tokens, mut cost) = (0, 0, 0, 0.0);
    // Spread the sample across the file rather than taking the first n.
    let all = cases["cases"].as_array().unwrap();
    let step = (all.len() / n.max(1)).max(1);
    for c in all.iter().step_by(step).take(n) {
        let qid = c["question_id"].as_str().unwrap();
        let q = quiz.questions.iter().find(|q| q.id == qid).unwrap();
        let j = judge(&Metered { inner: &ClaudeCli, task: "judge" }, &model, q, c["transcript"].as_str().unwrap())?;
        tokens += j.reply.tokens_in + j.reply.tokens_out;
        cost += j.reply.cost_usd.unwrap_or(0.0);
        for (kid, label) in c["labels"].as_object().unwrap() {
            let want = match label.as_str().unwrap() {
                "covered" => Verdict::Covered,
                "missing" => Verdict::Missing,
                _ => Verdict::Contradicted,
            };
            total += 1;
            right += (j.points[kid].verdict == want) as u32;
        }
        println!("{}: done", c["id"]);
    }
    println!("exact {right}/{total} ({:.3}); tokens {tokens}; reported cost ${cost:.4}", right as f64 / total as f64);
    Ok(())
}
