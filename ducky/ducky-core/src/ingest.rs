use crate::provider::{Provider, Reply, Request};
use crate::quiz::{KeyPoint, Question, Quiz};
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;

/// Text of every page, in order (index 0 is page 1).
pub fn extract_pages(pdf: &Path) -> Result<Vec<String>> {
    pdf_extract::extract_text_by_pages(pdf).with_context(|| format!("cannot read {}", pdf.display()))
}

/// Groups consecutive pages (1-based, inclusive range) into chunks of at most `max_chars`.
/// Returns (first_page, last_page, text). A single page longer than the limit stays whole.
pub fn chunk_pages(pages: &[String], first: usize, last: usize, max_chars: usize) -> Vec<(usize, usize, String)> {
    let mut chunks: Vec<(usize, usize, String)> = Vec::new();
    for n in first.max(1)..=last.min(pages.len()) {
        let text = pages[n - 1].trim();
        if text.is_empty() {
            continue;
        }
        match chunks.last_mut() {
            Some((_, end, buf)) if buf.len() + text.len() <= max_chars => {
                buf.push_str("\n\n");
                buf.push_str(text);
                *end = n;
            }
            _ => chunks.push((n, n, text.to_string())),
        }
    }
    chunks
}

const SYSTEM: &str = "You turn study material into spoken-quiz questions. \
Write each question and model answer in your OWN words; never copy sentences from the text. \
Ask open questions that a person can answer out loud in 2 to 5 sentences (explain, compare, why, how, trade-offs). \
Skip trivia, page references, figure descriptions and questions that need a diagram. \
Each question has 2 to 5 key points: single self-contained sentences that together make a complete answer. \
Per key point give 3 to 6 lowercase keywords a speaker would likely say, and 1 to 2 wrong_claims: \
plausible mistakes that directly contradict that key point. \
Give each question 2 to 4 short lowercase tags (broad topic first, then concept). Return JSON only.";

fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {"questions": {"type": "array", "items": {
            "type": "object",
            "properties": {
                "question": {"type": "string"},
                "answer": {"type": "string"},
                "tags": {"type": "array", "items": {"type": "string"}},
                "key_points": {"type": "array", "items": {
                    "type": "object",
                    "properties": {
                        "text": {"type": "string"},
                        "keywords": {"type": "array", "items": {"type": "string"}},
                        "wrong_claims": {"type": "array", "items": {"type": "string"}}
                    },
                    "required": ["text", "keywords", "wrong_claims"]
                }}
            },
            "required": ["question", "answer", "tags", "key_points"]
        }}},
        "required": ["questions"]
    })
}

#[derive(Deserialize)]
struct RawOut {
    questions: Vec<RawQuestion>,
}
#[derive(Deserialize)]
struct RawQuestion {
    question: String,
    answer: String,
    #[serde(default)]
    tags: Vec<String>,
    key_points: Vec<RawPoint>,
}
#[derive(Deserialize)]
struct RawPoint {
    text: String,
    #[serde(default)]
    keywords: Vec<String>,
    #[serde(default)]
    wrong_claims: Vec<String>,
}

/// Parses one chunk's reply into questions. `prefix` makes ids unique across chunks ("p51q1").
pub fn parse_questions(text: &str, prefix: &str, page: usize) -> Result<Vec<Question>> {
    let start = text.find('{').context("no JSON object in reply")?;
    let end = text.rfind('}').context("no JSON object in reply")?;
    let raw: RawOut = serde_json::from_str(&text[start..=end]).context("reply does not match the schema")?;
    let mut out = Vec::new();
    for (i, q) in raw.questions.into_iter().enumerate() {
        // Drop malformed questions instead of failing the whole chunk.
        if q.question.trim().is_empty() || q.answer.trim().is_empty() || q.key_points.is_empty() {
            continue;
        }
        out.push(Question {
            id: format!("{prefix}q{}", i + 1),
            question: q.question,
            answer: q.answer,
            source_page: Some(page.to_string()),
            tags: q.tags.into_iter().map(|t| t.trim().to_lowercase()).filter(|t| !t.is_empty()).collect(),
            key_points: q
                .key_points
                .into_iter()
                .enumerate()
                .map(|(j, k)| KeyPoint {
                    id: format!("k{}", j + 1),
                    text: k.text,
                    keywords: k.keywords.into_iter().map(|w| w.to_lowercase()).collect(),
                    wrong_claims: k.wrong_claims,
                })
                .collect(),
        });
    }
    if out.is_empty() {
        bail!("reply contained no usable questions");
    }
    Ok(out)
}

pub struct Ingested {
    pub quiz: Quiz,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub cost_usd: f64,
    /// Chunks (first page) that failed twice and were skipped.
    pub failed_chunks: Vec<usize>,
}

/// Rough cost of one chunk, measured on the book (about 5.2k tokens in, 3.6k out for 2 chunks).
pub const TOKENS_PER_CHUNK: u64 = 4_400;

/// Turns pages `first..=last` of a PDF into a quiz. Each chunk is retried once on a bad reply.
/// `progress(done, total)` is called before each chunk; returning false cancels with an error.
pub fn ingest(
    p: &dyn Provider,
    model: &str,
    topic: &str,
    pages: &[String],
    first: usize,
    last: usize,
    max_chars: usize,
    progress: &mut dyn FnMut(usize, usize) -> bool,
) -> Result<Ingested> {
    let schema = schema();
    let mut res = Ingested {
        quiz: Quiz { topic: topic.to_string(), source_note: format!("Generated from pages {first}-{last}."), questions: vec![] },
        tokens_in: 0,
        tokens_out: 0,
        cost_usd: 0.0,
        failed_chunks: vec![],
    };
    let chunks = chunk_pages(pages, first, last, max_chars);
    let total = chunks.len();
    for (done, (start, _end, text)) in chunks.into_iter().enumerate() {
        if !progress(done, total) {
            bail!("cancelled");
        }
        let user = format!("Topic: {topic}\n\nMaterial:\n{text}");
        let mut parsed = None;
        for _ in 0..2 {
            let reply: Reply = p.complete(&Request { model, system: SYSTEM, user: &user, json_schema: Some(&schema) })?;
            res.tokens_in += reply.tokens_in;
            res.tokens_out += reply.tokens_out;
            res.cost_usd += reply.cost_usd.unwrap_or(0.0);
            if let Ok(qs) = parse_questions(&reply.text, &format!("p{start}"), start) {
                parsed = Some(qs);
                break;
            }
        }
        match parsed {
            Some(qs) => res.quiz.questions.extend(qs),
            None => res.failed_chunks.push(start),
        }
    }
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_by_size_and_skips_blank_pages() {
        let pages: Vec<String> = ["aaaa", "  ", "bbbb", "cccccccc"].iter().map(|s| s.to_string()).collect();
        let c = chunk_pages(&pages, 1, 4, 10);
        assert_eq!(c.len(), 2);
        assert_eq!((c[0].0, c[0].1), (1, 3));
        assert_eq!((c[1].0, c[1].1), (4, 4));
    }

    #[test]
    fn parses_and_normalises_questions() {
        let t = r#"{"questions":[
          {"question":"Why?","answer":"Because.","tags":[" Rate Limiter ","API"],"key_points":[{"text":"x","keywords":["Foo"],"wrong_claims":["y"]}]},
          {"question":"","answer":"a","tags":[],"key_points":[]}]}"#;
        let qs = parse_questions(t, "p51", 51).unwrap();
        assert_eq!(qs.len(), 1);
        assert_eq!(qs[0].id, "p51q1");
        assert_eq!(qs[0].tags, vec!["rate limiter", "api"]);
        assert_eq!(qs[0].key_points[0].keywords, vec!["foo"]);
    }

    #[test]
    fn rejects_empty_reply() {
        assert!(parse_questions(r#"{"questions":[]}"#, "p1", 1).is_err());
    }
}
