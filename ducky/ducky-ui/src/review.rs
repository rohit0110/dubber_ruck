//! The Review screen: what you miss, get wrong, paraphrase, and get right. Counting only, no model.
use crate::theme;
use ducky_core::quiz::Question;
use ducky_core::stats::{self, Counts, QuestionStat, Stats};
use eframe::egui;
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum View {
    Missed,
    Wrong,
    Paraphrased,
    Strong,
    Skipped,
    Tags,
    Topics,
}

const VIEWS: [(View, &str); 7] = [
    (View::Missed, "MISSED"),
    (View::Wrong, "WRONG"),
    (View::Paraphrased, "PARAPHRASED"),
    (View::Strong, "STRONG"),
    (View::Skipped, "SKIPPED"),
    (View::Tags, "TAGS"),
    (View::Topics, "TOPICS"),
];

pub struct Review {
    pub stats: Stats,
    pub dirty: bool,
    view: View,
}

pub type Library = HashMap<(String, String), Question>;

impl Review {
    pub fn new() -> Review {
        Review { stats: Stats::default(), dirty: true, view: View::Missed }
    }

    /// Draws the screen. Returns (topic, question id) when the user clicks RETRY.
    pub fn ui(&mut self, ui: &mut egui::Ui, library: &Library) -> Option<(String, String)> {
        if self.dirty {
            self.stats = stats::compute(&stats::load_history());
            self.dirty = false;
        }
        let mut retry = None;
        let total = self.stats.by_topic.iter().fold(Counts::default(), |mut a, (_, c)| {
            a.answered += c.answered;
            a.skipped += c.skipped;
            a
        });
        if total.answered + total.skipped == 0 {
            ui.label(theme::heading("NOTHING YET"));
            ui.label("Answer a few questions and your weak spots will show up here.");
            return None;
        }
        ui.label(format!("{} answered, {} skipped", total.answered, total.skipped));
        ui.horizontal_wrapped(|ui| {
            for (v, label) in VIEWS {
                if theme::tab(ui, self.view == v, label).clicked() {
                    self.view = v;
                }
            }
        });
        ui.separator();
        let list: Vec<&QuestionStat> = match self.view {
            View::Missed => self.stats.frequently_missed(),
            View::Wrong => self.stats.often_contradicted(),
            View::Paraphrased => self.stats.paraphrased(),
            View::Strong => self.stats.strong(),
            View::Skipped => self.stats.often_skipped(),
            View::Tags => {
                group_rows(ui, &self.stats.by_tag);
                return None;
            }
            View::Topics => {
                group_rows(ui, &self.stats.by_topic);
                return None;
            }
        };
        if list.is_empty() {
            ui.label(match self.view {
                View::Strong => "Nothing here yet. A question counts as strong after 2 answers with 85% of points right.",
                View::Skipped => "No question skipped twice or more.",
                _ => "Nothing here. Good.",
            });
        }
        for s in list {
            if let Some(r) = question_row(ui, s, library.get(&(s.topic.clone(), s.question_id.clone())), self.view) {
                retry = Some(r);
            }
        }
        retry
    }
}

fn summary(c: &Counts, view: View) -> String {
    match view {
        View::Wrong => format!("wrong {}/{}", c.contradicted, c.points()),
        View::Paraphrased => format!("paraphrased {}/{}", c.paraphrased, c.covered),
        View::Strong => format!("got {}/{}", c.covered, c.points()),
        View::Skipped => format!("skipped {}x", c.skipped),
        _ => format!("missed {}/{}", c.missing + c.contradicted, c.points()),
    }
}

fn question_row(ui: &mut egui::Ui, s: &QuestionStat, q: Option<&Question>, view: View) -> Option<(String, String)> {
    let mut retry = None;
    let title = q.map_or("(question no longer in your topics)".to_string(), |q| truncate(&q.question, 70));
    egui::CollapsingHeader::new(format!("{title}   [{}]", summary(&s.counts, view)))
        .id_salt((&s.topic, &s.question_id))
        .show(ui, |ui| {
            theme::bar(ui, s.counts.covered, s.counts.missing, s.counts.contradicted, 260.0);
            ui.label(egui::RichText::new(format!("{}  |  {} attempts  |  last {}", s.tags.join(", "), s.counts.answered, s.last_ts.replace('T', " "))).small());
            if let Some(q) = q {
                for k in &q.key_points {
                    let Some(c) = s.per_point.get(&k.id) else { continue };
                    let (tag, colour) = if c.contradicted > 0 {
                        ("WRONG", theme::BAD)
                    } else if c.missing > 0 {
                        ("MISSED", theme::WARN)
                    } else {
                        ("GOT IT", theme::GOOD)
                    };
                    ui.horizontal_wrapped(|ui| {
                        ui.label(egui::RichText::new(tag).family(theme::pixel_font()).color(colour));
                        ui.label(format!("{} ({}/{})", k.text, c.missing + c.contradicted, c.covered + c.missing + c.contradicted));
                    });
                }
                if !s.last_transcript.is_empty() {
                    ui.label(theme::heading("YOUR LAST ANSWER"));
                    ui.label(&s.last_transcript);
                }
                ui.label(theme::heading("MODEL ANSWER"));
                ui.label(&q.answer);
                if theme::button(ui, "RETRY").clicked() {
                    retry = Some((s.topic.clone(), s.question_id.clone()));
                }
            }
        });
    retry
}

fn group_rows(ui: &mut egui::Ui, rows: &[(String, Counts)]) {
    for (name, c) in rows {
        ui.horizontal(|ui| {
            theme::bar(ui, c.covered, c.missing, c.contradicted, 140.0);
            ui.label(format!("{}  -  missed {:.0}%, {} answered", truncate(name, 40), c.miss_rate() * 100.0, c.answered));
        });
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}...", s.chars().take(n).collect::<String>())
    }
}
