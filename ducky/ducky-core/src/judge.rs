use crate::provider::{Provider, Reply, Request};
use crate::quiz::Question;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Covered,
    Missing,
    Contradicted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointResult {
    pub verdict: Verdict,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone)]
pub struct Judgement {
    pub points: BTreeMap<String, PointResult>,
    pub reply: Reply,
}

const SYSTEM: &str = "You grade a spoken answer against a list of key points. \
For each key point decide: \"covered\" (the speaker conveyed it, in any wording), \
\"missing\" (not mentioned), or \"contradicted\" (the speaker stated something that conflicts with it). \
Judge meaning, not wording; ignore filler words and speech-to-text noise; honour self-corrections. \
For a contradicted point give a one-sentence note saying what the speaker said wrong. Otherwise leave the note empty. \
Use only the information given. Return JSON only.";

fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "points": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "string"},
                        "verdict": {"type": "string", "enum": ["covered", "missing", "contradicted"]},
                        "note": {"type": "string"}
                    },
                    "required": ["id", "verdict", "note"]
                }
            }
        },
        "required": ["points"]
    })
}

fn user_prompt(q: &Question, transcript: &str) -> String {
    let points: Vec<Value> = q.key_points.iter().map(|k| json!({"id": k.id, "text": k.text})).collect();
    format!(
        "Question: {}\nKey points: {}\nTranscript: {}",
        q.question,
        Value::Array(points),
        transcript
    )
}

pub fn judge(p: &dyn Provider, model: &str, q: &Question, transcript: &str) -> Result<Judgement> {
    let schema = schema();
    let user = user_prompt(q, transcript);
    let reply = p.complete(&Request { model, system: SYSTEM, user: &user, json_schema: Some(&schema) })?;
    let points = parse_points(&reply.text, q)?;
    Ok(Judgement { points, reply })
}

/// Parses the model output. Errors if the reply is malformed or skips a key point; callers should
/// treat that as "not sure" rather than guessing a verdict.
pub fn parse_points(text: &str, q: &Question) -> Result<BTreeMap<String, PointResult>> {
    #[derive(Deserialize)]
    struct Row {
        id: String,
        verdict: Verdict,
        #[serde(default)]
        note: String,
    }
    #[derive(Deserialize)]
    struct Out {
        points: Vec<Row>,
    }
    let start = text.find('{').context("no JSON object in reply")?;
    let end = text.rfind('}').context("no JSON object in reply")?;
    let out: Out = serde_json::from_str(&text[start..=end]).context("reply does not match the schema")?;
    let mut map: BTreeMap<String, PointResult> =
        out.points.into_iter().map(|r| (r.id, PointResult { verdict: r.verdict, note: r.note })).collect();
    map.retain(|id, _| q.key_points.iter().any(|k| &k.id == id));
    for k in &q.key_points {
        anyhow::ensure!(map.contains_key(&k.id), "reply has no verdict for key point {}", k.id);
    }
    Ok(map)
}

/// Short text to read aloud: what was missed and what was wrong. The full model answer stays on screen.
pub fn spoken_feedback(q: &Question, points: &BTreeMap<String, PointResult>) -> String {
    let of = |v: Verdict| q.key_points.iter().filter(move |k| points.get(&k.id).is_some_and(|r| r.verdict == v));
    let wrong: Vec<String> = of(Verdict::Contradicted)
        .map(|k| match points[&k.id].note.trim() {
            "" => format!("You got this wrong: {}", k.text),
            n => format!("{n} Actually: {}", k.text),
        })
        .collect();
    let missed: Vec<&str> = of(Verdict::Missing).map(|k| k.text.as_str()).collect();
    let mut out = Vec::new();
    if wrong.is_empty() && missed.is_empty() {
        return "Nice, you covered everything.".into();
    }
    out.extend(wrong);
    if !missed.is_empty() {
        out.push(format!("You missed: {}", missed.join(" Also: ")));
    }
    out.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quiz::KeyPoint;

    fn question() -> Question {
        let kp = |id: &str| KeyPoint { id: id.into(), text: "t".into(), keywords: vec![], wrong_claims: vec![] };
        Question {
            id: "q".into(),
            question: "?".into(),
            answer: "a".into(),
            source_page: None,
            tags: vec![],
            key_points: vec![kp("k1"), kp("k2")],
        }
    }

    #[test]
    fn parses_verdicts() {
        let t = r#"{"points":[{"id":"k1","verdict":"covered","note":""},{"id":"k2","verdict":"contradicted","note":"said X"}]}"#;
        let m = parse_points(t, &question()).unwrap();
        assert_eq!(m["k2"].verdict, Verdict::Contradicted);
    }

    #[test]
    fn rejects_missing_point() {
        let t = r#"{"points":[{"id":"k1","verdict":"covered","note":""}]}"#;
        assert!(parse_points(t, &question()).is_err());
    }

    #[test]
    fn spoken_feedback_lists_wrong_then_missed() {
        let mut q = question();
        q.key_points[0].text = "It caps requests.".into();
        q.key_points[1].text = "It returns 429.".into();
        let mk = |a, b: &str| -> BTreeMap<String, PointResult> {
            [("k1".to_string(), PointResult { verdict: a, note: String::new() }),
             ("k2".to_string(), PointResult { verdict: Verdict::Contradicted, note: b.into() })].into_iter().collect()
        };
        let f = spoken_feedback(&q, &mk(Verdict::Missing, "Said 403."));
        assert_eq!(f, "Said 403. Actually: It returns 429. You missed: It caps requests.");
        let all: BTreeMap<_, _> = q.key_points.iter().map(|k| (k.id.clone(), PointResult { verdict: Verdict::Covered, note: String::new() })).collect();
        assert_eq!(spoken_feedback(&q, &all), "Nice, you covered everything.");
    }
}
