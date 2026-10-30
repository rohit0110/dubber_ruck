use crate::config::home_dir;
use crate::provider::Reply;
use anyhow::Result;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub date: String,
    pub task: String,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub cost_usd: f64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Used {
    pub tokens: u64,
    pub cost_usd: f64,
}

fn ledger_path() -> PathBuf {
    home_dir().join("usage.jsonl")
}

/// Appends one provider call to the ledger.
pub fn record(date: NaiveDate, task: &str, reply: &Reply) -> Result<()> {
    std::fs::create_dir_all(home_dir())?;
    let e = Entry {
        date: date.to_string(),
        task: task.into(),
        tokens_in: reply.tokens_in,
        tokens_out: reply.tokens_out,
        cost_usd: reply.cost_usd.unwrap_or(0.0),
    };
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(ledger_path())?;
    writeln!(f, "{}", serde_json::to_string(&e)?)?;
    Ok(())
}

/// Sums one day. Unreadable lines are skipped, so a torn write never blocks the app.
pub fn used_on(date: NaiveDate) -> Used {
    let text = std::fs::read_to_string(ledger_path()).unwrap_or_default();
    sum(&text, date)
}

pub fn sum(ledger: &str, date: NaiveDate) -> Used {
    let day = date.to_string();
    ledger
        .lines()
        .filter_map(|l| serde_json::from_str::<Entry>(l).ok())
        .filter(|e| e.date == day)
        .fold(Used::default(), |u, e| Used { tokens: u.tokens + e.tokens_in + e.tokens_out, cost_usd: u.cost_usd + e.cost_usd })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sums_only_the_given_day_and_skips_bad_lines() {
        let l = r#"{"date":"2026-10-30","task":"judge","tokens_in":100,"tokens_out":20,"cost_usd":0.001}
not json
{"date":"2026-10-29","task":"judge","tokens_in":5,"tokens_out":5,"cost_usd":1.0}
{"date":"2026-10-30","task":"ingest","tokens_in":10,"tokens_out":0,"cost_usd":0.002}"#;
        let u = sum(l, NaiveDate::from_ymd_opt(2026, 10, 30).unwrap());
        assert_eq!(u.tokens, 130);
        assert!((u.cost_usd - 0.003).abs() < 1e-9);
    }
}

/// Wraps a provider so every call lands in the usage ledger and the daily cap sees it.
pub struct Metered<'a> {
    pub inner: &'a dyn crate::provider::Provider,
    pub task: &'a str,
}

impl crate::provider::Provider for Metered<'_> {
    fn complete(&self, req: &crate::provider::Request) -> Result<Reply> {
        let reply = self.inner.complete(req)?;
        // A failed write must not lose the answer the user already paid for.
        let _ = record(chrono::Local::now().date_naive(), self.task, &reply);
        Ok(reply)
    }
}
