//! The Topics screen: add a PDF (turned into a quiz in the background), switch topics on or off, delete them.
use crate::theme;
use ducky_core::budget::Metered;
use ducky_core::config::{home_dir, Config};
use ducky_core::ingest::{self, TOKENS_PER_CHUNK};
use ducky_core::provider::ClaudeCli;
use ducky_core::quiz::Quiz;
use eframe::egui;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

pub enum Event {
    None,
    /// `disabled_topics` changed; the caller saves the config.
    ConfigChanged,
    /// Files in the topics folder changed; the caller reloads the library.
    Reload,
}

struct TopicFile {
    path: PathBuf,
    topic: String,
    questions: usize,
}

struct AddForm {
    pdf: PathBuf,
    name: String,
    from: String,
    to: String,
}

enum Msg {
    Plan { chunks: usize },
    Progress { done: usize, total: usize },
    Finished(Result<String, String>),
}

struct Job {
    rx: Receiver<Msg>,
    cancel: Arc<AtomicBool>,
    name: String,
    done: usize,
    total: usize,
}

pub struct Topics {
    files: Vec<TopicFile>,
    dirty: bool,
    form: Option<AddForm>,
    job: Option<Job>,
    message: Option<(bool, String)>,
    confirm_delete: Option<PathBuf>,
}

fn topics_dir() -> PathBuf {
    home_dir().join("topics")
}

fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

fn load_files() -> Vec<TopicFile> {
    let mut v: Vec<TopicFile> = std::fs::read_dir(topics_dir())
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(".quiz.json"))
        .filter_map(|p| Quiz::load(&p).ok().map(|q| TopicFile { path: p, topic: q.topic, questions: q.questions.len() }))
        .collect();
    v.sort_by(|a, b| a.topic.cmp(&b.topic));
    v
}

impl Topics {
    pub fn new() -> Topics {
        Topics { files: vec![], dirty: true, form: None, job: None, message: None, confirm_delete: None }
    }

    fn start(&mut self, ctx: &egui::Context, model: &str) {
        let Some(f) = self.form.take() else { return };
        // Never let two topics share a name: Review and the library are keyed by it.
        let mut name = f.name.trim().to_string();
        let mut n = 2;
        while self.files.iter().any(|t| t.topic == name) {
            name = format!("{} ({n})", f.name.trim());
            n += 1;
        }
        let (tx, rx) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let (flag, model, ctx2, topic) = (cancel.clone(), model.to_string(), ctx.clone(), name.clone());
        let (from, to) = (f.from.trim().parse::<usize>().unwrap_or(1), f.to.trim().parse::<usize>().unwrap_or(usize::MAX));
        std::thread::spawn(move || {
            let res = run_ingest(&f.pdf, &topic, from, to, &model, &flag, &tx, &ctx2);
            let _ = tx.send(Msg::Finished(res));
            ctx2.request_repaint();
        });
        self.job = Some(Job { rx, cancel, name, done: 0, total: 0 });
    }

    fn poll(&mut self) -> bool {
        let mut reload = false;
        let Some(job) = &mut self.job else { return false };
        while let Ok(m) = job.rx.try_recv() {
            match m {
                Msg::Plan { chunks } => job.total = chunks,
                Msg::Progress { done, total } => (job.done, job.total) = (done, total),
                Msg::Finished(r) => {
                    self.message = Some(match r {
                        Ok(s) => (true, s),
                        Err(e) => (false, e),
                    });
                    reload = true;
                    self.job = None;
                    self.dirty = true;
                    break;
                }
            }
        }
        reload
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, cfg: &mut Config) -> Event {
        let mut event = Event::None;
        if self.poll() {
            event = Event::Reload;
        }
        if self.dirty {
            self.files = load_files();
            self.dirty = false;
        }
        ui.label(theme::heading("TOPICS"));
        if let Some((ok, m)) = &self.message {
            ui.label(egui::RichText::new(m).color(if *ok { theme::GOOD } else { theme::BAD }));
        }

        if let Some(job) = &self.job {
            ui.label(format!("Making questions for \"{}\"...", job.name));
            let frac = if job.total == 0 { 0.0 } else { job.done as f32 / job.total as f32 };
            ui.add(egui::ProgressBar::new(frac).text(format!("{} / {} sections", job.done, job.total)));
            if job.total > 0 {
                ui.label(egui::RichText::new(format!("About {} tokens in total. It counts toward your daily cap.", job.total as u64 * TOKENS_PER_CHUNK)).small());
            }
            if theme::button(ui, "CANCEL").clicked() {
                job.cancel.store(true, Ordering::Relaxed);
            }
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(300));
            return event;
        }

        if let Some(f) = &mut self.form {
            let mut go = false;
            let mut cancel = false;
            ui.label(egui::RichText::new(f.pdf.display().to_string()).small());
            ui.horizontal(|ui| {
                ui.label("Name");
                ui.text_edit_singleline(&mut f.name);
            });
            ui.horizontal(|ui| {
                ui.label("Pages");
                ui.add(egui::TextEdit::singleline(&mut f.from).desired_width(50.0));
                ui.label("to");
                ui.add(egui::TextEdit::singleline(&mut f.to).desired_width(50.0).hint_text("end"));
            });
            ui.label(egui::RichText::new("Start with one chapter: a whole book can use well over a day's token cap.").small());
            ui.horizontal(|ui| {
                go = ui.add_enabled(!f.name.trim().is_empty(), egui::Button::new(egui::RichText::new("MAKE QUESTIONS").family(theme::pixel_font()).size(16.0))).clicked();
                cancel = theme::button(ui, "CANCEL").clicked();
            });
            if go {
                self.start(ui.ctx(), &cfg.model.clone());
            } else if cancel {
                self.form = None;
            }
        } else if theme::button(ui, "ADD PDF").clicked() {
            if let Some(p) = rfd::FileDialog::new().add_filter("PDF", &["pdf"]).pick_file() {
                let name = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                self.form = Some(AddForm { pdf: p, name, from: "1".into(), to: String::new() });
                self.message = None;
            }
        }

        ui.separator();
        if self.files.is_empty() {
            ui.label("No topics yet. Add a PDF to make your first quiz.");
        }
        let mut delete: Option<PathBuf> = None;
        for t in &self.files {
            ui.horizontal(|ui| {
                let mut on = !cfg.disabled_topics.contains(&t.topic);
                if ui.checkbox(&mut on, "").changed() {
                    cfg.disabled_topics.retain(|d| d != &t.topic);
                    if !on {
                        cfg.disabled_topics.push(t.topic.clone());
                    }
                    event = Event::ConfigChanged;
                }
                ui.label(format!("{}  ({} questions)", t.topic, t.questions));
                if self.confirm_delete.as_deref() == Some(t.path.as_path()) {
                    if theme::button(ui, "SURE?").clicked() {
                        delete = Some(t.path.clone());
                    }
                } else if theme::button(ui, "DELETE").clicked() {
                    self.confirm_delete = Some(t.path.clone());
                }
            });
        }
        if let Some(p) = delete {
            let _ = std::fs::remove_file(&p);
            self.confirm_delete = None;
            self.dirty = true;
            event = Event::Reload;
        }
        event
    }
}

#[allow(clippy::too_many_arguments)]
fn run_ingest(
    pdf: &Path,
    topic: &str,
    from: usize,
    to: usize,
    model: &str,
    cancel: &AtomicBool,
    tx: &std::sync::mpsc::Sender<Msg>,
    ctx: &egui::Context,
) -> Result<String, String> {
    let pages = ingest::extract_pages(pdf).map_err(|e| format!("{e:#}"))?;
    let (first, last) = (from.max(1), to.min(pages.len()));
    if first > last {
        return Err(format!("The PDF has {} pages; pages {from} to {to} is empty.", pages.len()));
    }
    let planned = ingest::chunk_pages(&pages, first, last, 6000).len();
    let _ = tx.send(Msg::Plan { chunks: planned });
    let provider = Metered { inner: &ClaudeCli, task: "ingest" };
    let res = ingest::ingest(&provider, model, topic, &pages, first, last, 6000, &mut |done, total| {
        let _ = tx.send(Msg::Progress { done, total });
        ctx.request_repaint();
        !cancel.load(Ordering::Relaxed)
    })
    .map_err(|e| format!("{e:#}"))?;
    if res.quiz.questions.is_empty() {
        return Err("No questions came back. Try a different page range.".into());
    }
    let mut path = topics_dir().join(format!("{}.quiz.json", slug(topic)));
    let mut n = 2;
    while path.exists() {
        path = topics_dir().join(format!("{}-{n}.quiz.json", slug(topic)));
        n += 1;
    }
    std::fs::create_dir_all(topics_dir()).map_err(|e| e.to_string())?;
    std::fs::write(&path, serde_json::to_string_pretty(&res.quiz).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let skipped = if res.failed_chunks.is_empty() { String::new() } else { format!(" {} sections failed and were skipped.", res.failed_chunks.len()) };
    Ok(format!("Made {} questions for \"{topic}\" from pages {first}-{last}.{skipped}", res.quiz.questions.len()))
}

#[cfg(test)]
mod tests {
    use super::slug;

    #[test]
    fn slugs_are_filename_safe() {
        assert_eq!(slug("System Design: Rate Limiter!"), "system-design-rate-limiter");
        assert_eq!(slug("  --a  b-- "), "a-b");
    }
}
