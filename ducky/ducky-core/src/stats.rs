//! Struggle statistics from the history log. Plain counting over tags; no model involved.
use crate::config::home_dir;
use crate::history::Record;
use crate::judge::Verdict;
use std::collections::BTreeMap;

/// Point-level counts: every key point of every answered question counts once.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Counts {
    pub answered: u32,
    pub skipped: u32,
    pub covered: u32,
    pub missing: u32,
    pub contradicted: u32,
    /// Subset of `covered`: right idea, none of the expected words.
    pub paraphrased: u32,
}

impl Counts {
    pub fn points(&self) -> u32 {
        self.covered + self.missing + self.contradicted
    }
    fn rate(n: u32, d: u32) -> f32 {
        if d == 0 { 0.0 } else { n as f32 / d as f32 }
    }
    pub fn miss_rate(&self) -> f32 {
        Self::rate(self.missing + self.contradicted, self.points())
    }
    pub fn wrong_rate(&self) -> f32 {
        Self::rate(self.contradicted, self.points())
    }
    pub fn covered_rate(&self) -> f32 {
        Self::rate(self.covered, self.points())
    }
    /// Share of the points you got right that you said in other words.
    pub fn paraphrase_rate(&self) -> f32 {
        Self::rate(self.paraphrased, self.covered)
    }
    fn add(&mut self, o: &Counts) {
        self.answered += o.answered;
        self.skipped += o.skipped;
        self.covered += o.covered;
        self.missing += o.missing;
        self.contradicted += o.contradicted;
        self.paraphrased += o.paraphrased;
    }
}

#[derive(Debug, Clone, Default)]
pub struct QuestionStat {
    pub topic: String,
    pub question_id: String,
    pub tags: Vec<String>,
    pub counts: Counts,
    pub last_ts: String,
    pub last_transcript: String,
    /// Per key point id.
    pub per_point: BTreeMap<String, Counts>,
}

#[derive(Debug, Clone, Default)]
pub struct Stats {
    pub questions: Vec<QuestionStat>,
    pub by_tag: Vec<(String, Counts)>,
    pub by_topic: Vec<(String, Counts)>,
}

/// "Rate-Limiters " and "rate limiter" count as one tag.
pub fn normalize_tag(t: &str) -> String {
    t.to_lowercase()
        .split(|c: char| c.is_whitespace() || c == '-' || c == '_')
        .filter(|w| !w.is_empty())
        .map(|w| if w.len() > 3 && w.ends_with('s') && !w.ends_with("ss") { &w[..w.len() - 1] } else { w })
        .collect::<Vec<_>>()
        .join(" ")
}

fn counts_of(r: &Record) -> Counts {
    let mut c = Counts::default();
    if r.skipped {
        c.skipped = 1;
        return c;
    }
    c.answered = 1;
    for p in &r.points {
        match p.verdict {
            Verdict::Covered => {
                c.covered += 1;
                c.paraphrased += p.paraphrased as u32;
            }
            Verdict::Missing => c.missing += 1,
            Verdict::Contradicted => c.contradicted += 1,
        }
    }
    c
}

pub fn compute(records: &[Record]) -> Stats {
    let mut q: BTreeMap<(String, String), QuestionStat> = BTreeMap::new();
    let mut tags: BTreeMap<String, Counts> = BTreeMap::new();
    let mut topics: BTreeMap<String, Counts> = BTreeMap::new();
    for r in records {
        let c = counts_of(r);
        let s = q.entry((r.topic.clone(), r.question_id.clone())).or_insert_with(|| QuestionStat {
            topic: r.topic.clone(),
            question_id: r.question_id.clone(),
            ..Default::default()
        });
        s.counts.add(&c);
        s.tags = r.tags.iter().map(|t| normalize_tag(t)).filter(|t| !t.is_empty()).collect();
        if r.ts >= s.last_ts {
            s.last_ts = r.ts.clone();
            if !r.skipped {
                s.last_transcript = r.transcript.clone();
            }
        }
        for p in r.points.iter().filter(|_| !r.skipped) {
            let mut pc = Counts::default();
            match p.verdict {
                Verdict::Covered => {
                    pc.covered = 1;
                    pc.paraphrased = p.paraphrased as u32;
                }
                Verdict::Missing => pc.missing = 1,
                Verdict::Contradicted => pc.contradicted = 1,
            }
            s.per_point.entry(p.id.clone()).or_default().add(&pc);
        }
        topics.entry(r.topic.clone()).or_default().add(&c);
        let mut seen = std::collections::BTreeSet::new();
        for t in r.tags.iter().map(|t| normalize_tag(t)).filter(|t| !t.is_empty()) {
            if seen.insert(t.clone()) {
                tags.entry(t).or_default().add(&c);
            }
        }
    }
    let by_miss = |a: &(String, Counts), b: &(String, Counts)| b.1.miss_rate().total_cmp(&a.1.miss_rate()).then(a.0.cmp(&b.0));
    let mut by_tag: Vec<_> = tags.into_iter().collect();
    let mut by_topic: Vec<_> = topics.into_iter().collect();
    by_tag.sort_by(by_miss);
    by_topic.sort_by(by_miss);
    Stats { questions: q.into_values().collect(), by_tag, by_topic }
}

impl Stats {
    fn ranked(&self, keep: impl Fn(&QuestionStat) -> bool, key: impl Fn(&QuestionStat) -> f32) -> Vec<&QuestionStat> {
        let mut v: Vec<_> = self.questions.iter().filter(|s| keep(s)).collect();
        v.sort_by(|a, b| key(b).total_cmp(&key(a)).then(b.counts.answered.cmp(&a.counts.answered)));
        v
    }
    /// Questions where key points go missing or wrong, worst first.
    pub fn frequently_missed(&self) -> Vec<&QuestionStat> {
        self.ranked(|s| s.counts.missing + s.counts.contradicted > 0, |s| s.counts.miss_rate())
    }
    /// Questions where you stated something that conflicts with the answer.
    pub fn often_contradicted(&self) -> Vec<&QuestionStat> {
        self.ranked(|s| s.counts.contradicted > 0, |s| s.counts.wrong_rate())
    }
    /// Questions you get right but in your own words rather than the expected terms.
    pub fn paraphrased(&self) -> Vec<&QuestionStat> {
        self.ranked(|s| s.counts.paraphrased > 0, |s| s.counts.paraphrase_rate())
    }
    /// Answered at least twice, at least 85% of points covered.
    pub fn strong(&self) -> Vec<&QuestionStat> {
        self.ranked(|s| s.counts.answered >= 2 && s.counts.covered_rate() >= 0.85, |s| s.counts.covered_rate())
    }
    /// Skipped two or more times.
    pub fn often_skipped(&self) -> Vec<&QuestionStat> {
        self.ranked(|s| s.counts.skipped >= 2, |s| s.counts.skipped as f32)
    }
}

/// Reads `history.jsonl`; unreadable lines are skipped.
pub fn load_history() -> Vec<Record> {
    let text = std::fs::read_to_string(home_dir().join("history.jsonl")).unwrap_or_default();
    text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::PointRecord;

    fn rec(ts: &str, qid: &str, tags: &[&str], pts: &[(&str, Verdict, bool)]) -> Record {
        Record {
            ts: ts.into(),
            topic: "T".into(),
            question_id: qid.into(),
            tags: tags.iter().map(|s| s.to_string()).collect(),
            transcript: format!("said {ts}"),
            points: pts
                .iter()
                .map(|(id, v, p)| PointRecord { id: id.to_string(), verdict: *v, keyword_hits: 0, paraphrased: *p })
                .collect(),
            seconds: 5,
            skipped: false,
        }
    }

    fn history() -> Vec<Record> {
        use Verdict::*;
        let mut skip = rec("2026-10-30T10:03:00", "q3", &["Caching"], &[]);
        skip.skipped = true;
        vec![
            rec("2026-10-30T10:00:00", "q1", &["Rate Limiter", "api"], &[("k1", Covered, false), ("k2", Missing, false), ("k3", Contradicted, false)]),
            rec("2026-10-30T10:01:00", "q1", &["rate-limiters", "api"], &[("k1", Covered, false), ("k2", Missing, false), ("k3", Covered, false)]),
            rec("2026-10-30T10:02:00", "q2", &["caching"], &[("k1", Covered, true), ("k2", Covered, false)]),
            rec("2026-10-30T10:02:30", "q2", &["caching"], &[("k1", Covered, true), ("k2", Covered, false)]),
            skip.clone(),
            { let mut s = skip; s.ts = "2026-10-30T10:04:00".into(); s },
        ]
    }

    #[test]
    fn normalises_tags() {
        assert_eq!(normalize_tag(" Rate-Limiters "), "rate limiter");
        assert_eq!(normalize_tag("caching"), "caching");
        assert_eq!(normalize_tag("class"), "class");
    }

    #[test]
    fn counts_points_per_question_and_tag() {
        let s = compute(&history());
        let q1 = s.questions.iter().find(|q| q.question_id == "q1").unwrap();
        assert_eq!((q1.counts.answered, q1.counts.covered, q1.counts.missing, q1.counts.contradicted), (2, 3, 2, 1));
        assert_eq!(q1.per_point["k2"].missing, 2);
        assert_eq!(q1.last_transcript, "said 2026-10-30T10:01:00");
        let tag = |n: &str| s.by_tag.iter().find(|t| t.0 == n).unwrap().1;
        assert_eq!(tag("rate limiter").answered, 2); // both spellings merged
        assert_eq!(tag("api").missing, 2);
        assert_eq!(s.by_topic.len(), 1);
    }

    #[test]
    fn rankings() {
        let s = compute(&history());
        assert_eq!(s.frequently_missed().iter().map(|q| q.question_id.as_str()).collect::<Vec<_>>(), vec!["q1"]);
        assert_eq!(s.often_contradicted().len(), 1);
        let p = s.paraphrased();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].question_id, "q2");
        assert_eq!(s.strong().iter().map(|q| q.question_id.as_str()).collect::<Vec<_>>(), vec!["q2"]);
        assert_eq!(s.often_skipped()[0].question_id, "q3");
        assert_eq!(s.by_tag[0].0, "api"); // worst tag first
    }

    #[test]
    fn empty_history_is_fine() {
        let s = compute(&[]);
        assert!(s.questions.is_empty() && s.by_tag.is_empty());
    }
}
