use crate::config::home_dir;
use crate::judge::{PointResult, Verdict};
use crate::quiz::{KeyPoint, Question};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Write;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointRecord {
    pub id: String,
    pub verdict: Verdict,
    pub keyword_hits: usize,
    /// Judged covered although none of the expected keywords were said: right idea, different words.
    pub paraphrased: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub ts: String,
    pub topic: String,
    pub question_id: String,
    pub tags: Vec<String>,
    pub transcript: String,
    pub points: Vec<PointRecord>,
    pub seconds: u64,
    pub skipped: bool,
}

pub fn keyword_hits(transcript: &str, kp: &KeyPoint) -> usize {
    let t = transcript.to_lowercase();
    kp.keywords.iter().filter(|w| !w.is_empty() && t.contains(&w.to_lowercase())).count()
}

/// Builds the record for an answered question. `judged` maps key point id to its verdict.
pub fn answered(
    ts: String,
    topic: &str,
    q: &Question,
    transcript: &str,
    judged: &BTreeMap<String, PointResult>,
    seconds: u64,
) -> Record {
    let points = q
        .key_points
        .iter()
        .filter_map(|k| {
            let v = judged.get(&k.id)?.verdict;
            let hits = keyword_hits(transcript, k);
            Some(PointRecord {
                id: k.id.clone(),
                verdict: v,
                keyword_hits: hits,
                paraphrased: v == Verdict::Covered && !k.keywords.is_empty() && hits == 0,
            })
        })
        .collect();
    Record {
        ts,
        topic: topic.into(),
        question_id: q.id.clone(),
        tags: q.tags.clone(),
        transcript: transcript.into(),
        points,
        seconds,
        skipped: false,
    }
}

pub fn skipped(ts: String, topic: &str, q: &Question, seconds: u64) -> Record {
    Record {
        ts,
        topic: topic.into(),
        question_id: q.id.clone(),
        tags: q.tags.clone(),
        transcript: String::new(),
        points: vec![],
        seconds,
        skipped: true,
    }
}

pub fn append(r: &Record) -> Result<()> {
    std::fs::create_dir_all(home_dir())?;
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(home_dir().join("history.jsonl"))?;
    writeln!(f, "{}", serde_json::to_string(r)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn question() -> Question {
        let kp = |id: &str, kw: &[&str]| KeyPoint {
            id: id.into(),
            text: "t".into(),
            keywords: kw.iter().map(|s| s.to_string()).collect(),
            wrong_claims: vec![],
        };
        Question {
            id: "q1".into(),
            question: "?".into(),
            answer: "a".into(),
            source_page: None,
            tags: vec!["x".into()],
            key_points: vec![kp("k1", &["bucket", "tokens"]), kp("k2", &["refill"]), kp("k3", &[])],
        }
    }

    fn pr(v: Verdict) -> PointResult {
        PointResult { verdict: v, note: String::new() }
    }

    #[test]
    fn flags_paraphrase_only_for_covered_points_without_keywords() {
        let j: BTreeMap<_, _> = [("k1", Verdict::Covered), ("k2", Verdict::Covered), ("k3", Verdict::Covered)]
            .into_iter()
            .map(|(k, v)| (k.to_string(), pr(v)))
            .collect();
        let r = answered("t".into(), "topic", &question(), "It keeps a Bucket and tops it up over time", &j, 5);
        assert_eq!(r.points[0].keyword_hits, 1);
        assert!(!r.points[0].paraphrased); // said "bucket"
        assert!(r.points[1].paraphrased); // covered, but never said "refill"
        assert!(!r.points[2].paraphrased); // no keywords to compare against
    }

    #[test]
    fn missing_points_are_not_paraphrases() {
        let j: BTreeMap<_, _> = [("k1", Verdict::Missing), ("k2", Verdict::Contradicted), ("k3", Verdict::Missing)]
            .into_iter()
            .map(|(k, v)| (k.to_string(), pr(v)))
            .collect();
        let r = answered("t".into(), "topic", &question(), "nothing relevant", &j, 5);
        assert!(r.points.iter().all(|p| !p.paraphrased));
    }
}
