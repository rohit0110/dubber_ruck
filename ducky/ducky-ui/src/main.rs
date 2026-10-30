//! The ducky app. `ducky-ui` opens on Review; `ducky-ui --ask` (what ducky-tick launches) opens on a question.
mod review;
mod settings;
mod theme;
mod topics;

use anyhow::Result;
use chrono::Local;
use eframe::egui;
use ducky_core::budget::Metered;
use ducky_core::config::{home_dir, Config};
use ducky_core::history;
use ducky_core::judge::{self, Judgement, Verdict};
use ducky_core::provider::ClaudeCli;
use ducky_core::quiz::{Question, Quiz};
use ducky_voice::{Recorder, Speaker, Stt};
use rand::seq::IndexedRandom;
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

/// The speech model loads once, in the background, so the window opens at once.
type SharedStt = Arc<OnceLock<Result<Stt, String>>>;

fn stt_get(cell: &SharedStt) -> &Result<Stt, String> {
    cell.get_or_init(|| {
        let model = home_dir().join("models").join("ggml-base.en.bin");
        Stt::load(&model).map_err(|e| format!("{e:#}"))
    })
}

fn now_ts() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Quiz,
    Review,
    Topics,
    Settings,
}

enum Stage {
    Asking,
    Judging(Receiver<Result<Judgement, String>>),
    Done(Judgement),
    Failed(String),
}

enum Mic {
    Idle,
    Recording(Recorder),
    Transcribing(Receiver<Result<String, String>>),
}

/// One question being worked on.
struct QuizView {
    topic: String,
    question: Question,
    answer: String,
    stage: Stage,
    started: Instant,
    saved: bool,
    mic: Mic,
    mic_error: Option<String>,
}

impl QuizView {
    fn new(topic: String, question: Question) -> QuizView {
        QuizView { topic, question, answer: String::new(), stage: Stage::Asking, started: Instant::now(), saved: false, mic: Mic::Idle, mic_error: None }
    }

    fn log_answer(&mut self, j: &Judgement) {
        if !std::mem::replace(&mut self.saved, true) {
            let r = history::answered(now_ts(), &self.topic, &self.question, &self.answer, &j.points, self.started.elapsed().as_secs());
            let _ = history::append(&r);
        }
    }

    fn log_skip(&mut self) {
        if !std::mem::replace(&mut self.saved, true) {
            let _ = history::append(&history::skipped(now_ts(), &self.topic, &self.question, self.started.elapsed().as_secs()));
        }
    }
}

enum Action {
    Ask,
    Retry(String, String),
    Tab(Tab),
    Close,
    Submit,
    Skip,
    Mic,
    Again,
}

struct App {
    tab: Tab,
    /// Close the window after a skip (the scheduler opened it just for one question).
    ask_mode: bool,
    quiz: Option<QuizView>,
    library: review::Library,
    review: review::Review,
    topics: topics::Topics,
    settings: settings::Settings,
    cfg: Config,
    voice: bool,
    speaker: Speaker,
    stt: SharedStt,
    model: String,
    lock_touched: Instant,
}

fn load_library() -> review::Library {
    Quiz::load_dir(&home_dir().join("topics"))
        .into_iter()
        .flat_map(|z| z.questions.into_iter().map(move |q| ((z.topic.clone(), q.id.clone()), q)))
        .collect()
}

impl App {
    fn start_quiz(&mut self, topic: String, question: Question) {
        if self.voice {
            self.speaker.say(&question.question);
        }
        self.quiz = Some(QuizView::new(topic, question));
        self.tab = Tab::Quiz;
    }

    fn start_random(&mut self) {
        let pick = {
            let all: Vec<_> = self.library.iter().filter(|((t, _), _)| !self.cfg.disabled_topics.contains(t)).collect();
            all.choose(&mut rand::rng()).map(|((t, _), q)| (t.clone(), (*q).clone()))
        };
        if let Some((t, q)) = pick {
            self.start_quiz(t, q);
        }
    }

    fn apply(&mut self, a: Action, ctx: &egui::Context) {
        match a {
            Action::Ask => self.start_random(),
            Action::Retry(t, id) => {
                if let Some(q) = self.library.get(&(t.clone(), id)).cloned() {
                    self.start_quiz(t, q);
                }
            }
            Action::Tab(t) => self.tab = t,
            Action::Close => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Action::Skip => {
                self.speaker.stop();
                if let Some(q) = &mut self.quiz {
                    q.log_skip();
                }
                self.review.dirty = true;
                self.quiz = None;
                if self.ask_mode {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            Action::Again => {
                if let Some(q) = &self.quiz {
                    self.speaker.say(&q.question.question);
                }
            }
            Action::Submit => {
                self.speaker.stop();
                let Some(q) = &mut self.quiz else { return };
                q.mic = Mic::Idle; // closes the microphone if it was still open
                let (tx, rx) = channel();
                let (model, question, transcript, ctx) = (self.model.clone(), q.question.clone(), q.answer.clone(), ctx.clone());
                std::thread::spawn(move || {
                    let p = Metered { inner: &ClaudeCli, task: "judge" };
                    let _ = tx.send(judge::judge(&p, &model, &question, &transcript).map_err(|e| format!("{e:#}")));
                    ctx.request_repaint();
                });
                q.stage = Stage::Judging(rx);
            }
            Action::Mic => {
                self.speaker.stop();
                let Some(q) = &mut self.quiz else { return };
                q.mic_error = None;
                match std::mem::replace(&mut q.mic, Mic::Idle) {
                    Mic::Idle => match Recorder::start() {
                        Ok(r) => q.mic = Mic::Recording(r),
                        Err(e) => q.mic_error = Some(format!("{e:#}")),
                    },
                    Mic::Recording(r) => {
                        let audio = r.stop();
                        let (tx, rx) = channel();
                        let (stt, ctx) = (self.stt.clone(), ctx.clone());
                        std::thread::spawn(move || {
                            let res = match stt_get(&stt) {
                                Ok(s) => s.transcribe(&audio).map_err(|e| format!("{e:#}")),
                                Err(e) => Err(e.clone()),
                            };
                            let _ = tx.send(res);
                            ctx.request_repaint();
                        });
                        q.mic = Mic::Transcribing(rx);
                    }
                    busy @ Mic::Transcribing(_) => q.mic = busy,
                }
            }
        }
    }

    /// Picks up finished background work (judging, transcription).
    fn poll(&mut self) {
        let Some(q) = &mut self.quiz else { return };
        if let Stage::Judging(rx) = &q.stage {
            if let Ok(res) = rx.try_recv() {
                q.stage = match res {
                    Ok(j) => {
                        q.log_answer(&j);
                        self.review.dirty = true;
                        if self.voice {
                            self.speaker.say(&judge::spoken_feedback(&q.question, &j.points));
                        }
                        Stage::Done(j)
                    }
                    Err(e) => Stage::Failed(e),
                };
            }
        }
        if let Mic::Transcribing(rx) = &q.mic {
            if let Ok(res) = rx.try_recv() {
                match res {
                    Ok(t) if t.is_empty() => q.mic_error = Some("Did not catch anything. Try again.".into()),
                    Ok(t) => {
                        if !q.answer.trim().is_empty() {
                            q.answer.push(' ');
                        }
                        q.answer.push_str(&t);
                    }
                    Err(e) => q.mic_error = Some(e),
                }
                q.mic = Mic::Idle;
            }
        }
    }

    fn quiz_ui(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        let Some(q) = &mut self.quiz else {
            ui.label(theme::heading("READY?"));
            if self.library.is_empty() {
                ui.label("No quizzes yet. Add a PDF on the TOPICS tab.");
            } else if self.library.keys().all(|(t, _)| self.cfg.disabled_topics.contains(t)) {
                ui.label("All topics are switched off. Turn one on in TOPICS.");
            } else if theme::button(ui, "ASK ME ONE").clicked() {
                acts.push(Action::Ask);
            }
            return;
        };
        ui.label(theme::heading("QUESTION"));
        ui.label(egui::RichText::new(&q.question.question).size(17.0));
        ui.add_space(6.0);
        let ctx = ui.ctx().clone();
        match &q.stage {
            Stage::Asking => {
                ui.add(egui::TextEdit::multiline(&mut q.answer).desired_width(f32::INFINITY).desired_rows(6).hint_text("Type your answer..."));
                if self.voice {
                    ui.horizontal(|ui| {
                        let label = match &q.mic {
                            Mic::Idle => "MIC".to_string(),
                            Mic::Recording(r) => format!("STOP {:.0}s", r.seconds()),
                            Mic::Transcribing(_) => "...".to_string(),
                        };
                        let busy = matches!(q.mic, Mic::Transcribing(_));
                        if ui.add_enabled(!busy, egui::Button::new(egui::RichText::new(label).family(theme::pixel_font()).size(16.0))).clicked() {
                            acts.push(Action::Mic);
                        }
                        if theme::button(ui, "AGAIN").clicked() {
                            acts.push(Action::Again);
                        }
                        match &q.mic {
                            Mic::Recording(_) => {
                                ui.colored_label(theme::BAD, "listening - press STOP when done");
                                ctx.request_repaint_after(Duration::from_millis(250));
                            }
                            Mic::Transcribing(_) => {
                                ui.spinner();
                            }
                            Mic::Idle => {}
                        }
                    });
                    if let Some(e) = &q.mic_error {
                        ui.label(egui::RichText::new(e).small().color(theme::BAD));
                    }
                }
                ui.horizontal(|ui| {
                    let can = !q.answer.trim().is_empty();
                    if ui.add_enabled(can, egui::Button::new(egui::RichText::new("ANSWER").family(theme::pixel_font()).size(16.0))).clicked() {
                        acts.push(Action::Submit);
                    }
                    if theme::button(ui, "SKIP").clicked() {
                        acts.push(Action::Skip);
                    }
                });
            }
            Stage::Judging(_) => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Checking your answer...");
                });
            }
            Stage::Failed(e) => {
                ui.colored_label(theme::BAD, "Could not check the answer (nothing was logged).");
                ui.label(egui::RichText::new(e).small());
                if theme::button(ui, "TRY AGAIN").clicked() {
                    acts.push(Action::Submit);
                }
            }
            Stage::Done(j) => {
                ui.label(theme::heading("HOW YOU DID"));
                for k in &q.question.key_points {
                    let Some(r) = j.points.get(&k.id) else { continue };
                    let (tag, colour) = match r.verdict {
                        Verdict::Covered => ("GOT IT", theme::GOOD),
                        Verdict::Missing => ("MISSED", theme::WARN),
                        Verdict::Contradicted => ("WRONG", theme::BAD),
                    };
                    ui.horizontal_wrapped(|ui| {
                        ui.label(egui::RichText::new(tag).family(theme::pixel_font()).color(colour));
                        ui.label(&k.text);
                    });
                    if !r.note.is_empty() {
                        ui.label(egui::RichText::new(format!("   You said something off: {}", r.note)).color(theme::BAD));
                    }
                }
                ui.add_space(4.0);
                ui.label(theme::heading("MODEL ANSWER"));
                ui.label(&q.question.answer);
                ui.horizontal(|ui| {
                    if theme::button(ui, "NEXT").clicked() {
                        acts.push(Action::Ask);
                    }
                    if theme::button(ui, "REVIEW").clicked() {
                        acts.push(Action::Tab(Tab::Review));
                    }
                    if theme::button(ui, "CLOSE").clicked() {
                        acts.push(Action::Close);
                    }
                });
            }
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll();
        // Keep the lock fresh so ducky-tick does not treat a long-open window as abandoned.
        if self.lock_touched.elapsed() > Duration::from_secs(300) {
            Lock::touch();
            self.lock_touched = Instant::now();
        }
        let ctx = ui.ctx().clone();
        let mut acts: Vec<Action> = Vec::new();
        let mut reload = false;
        let mut apply_settings = false;
        if theme::title_bar(ui, "DUCKY") {
            acts.push(Action::Close);
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::PAPER).stroke(egui::Stroke::new(theme::BORDER, theme::INK)).inner_margin(16.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (t, label) in [(Tab::Quiz, "QUIZ"), (Tab::Review, "REVIEW"), (Tab::Topics, "TOPICS"), (Tab::Settings, "SETTINGS")] {
                        if theme::tab(ui, self.tab == t, label).clicked() {
                            acts.push(Action::Tab(t));
                        }
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match self.tab {
                    Tab::Quiz => self.quiz_ui(ui, &mut acts),
                    Tab::Review => {
                        if let Some((t, id)) = self.review.ui(ui, &self.library) {
                            acts.push(Action::Retry(t, id));
                        }
                    }
                    Tab::Settings => {
                        if self.settings.ui(ui, &mut self.cfg).0 {
                            apply_settings = true;
                        }
                    }
                    Tab::Topics => match self.topics.ui(ui, &mut self.cfg) {
                        topics::Event::ConfigChanged => {
                            let _ = self.cfg.save();
                        }
                        topics::Event::Reload => reload = true,
                        topics::Event::None => {}
                    },
                });
            });
        if apply_settings {
            self.voice = self.cfg.voice;
            self.model = self.cfg.model.clone();
            self.speaker = Speaker::new(&self.cfg.tts_voice);
        }
        if reload {
            self.library = load_library();
        }
        for a in acts {
            self.apply(a, &ctx);
        }
    }
}

/// Holds ~/.ducky/quiz.lock for as long as the window lives, so ducky-tick never opens a second one.
struct Lock;
impl Lock {
    fn touch() {
        let _ = std::fs::create_dir_all(home_dir());
        let _ = std::fs::write(home_dir().join("quiz.lock"), std::process::id().to_string());
    }
    fn take() -> Lock {
        Lock::touch();
        Lock
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(home_dir().join("quiz.lock"));
    }
}

fn main() -> Result<()> {
    let ask_mode = std::env::args().any(|a| a == "--ask");
    let cfg = Config::load()?;
    let library = load_library();
    if ask_mode && library.keys().all(|(t, _)| cfg.disabled_topics.contains(t)) {
        eprintln!("no active quizzes in {}", home_dir().join("topics").display());
        return Ok(());
    }
    let _lock = Lock::take();
    let stt: SharedStt = Arc::new(OnceLock::new());
    if cfg.voice {
        let warm = stt.clone();
        std::thread::spawn(move || {
            let _ = stt_get(&warm);
        });
    }
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([640.0, 640.0])
            .with_min_inner_size([460.0, 400.0])
            .with_decorations(false)
            .with_title("Ducky")
            .with_always_on_top(),
        ..Default::default()
    };
    eframe::run_native(
        "Ducky",
        opts,
        Box::new(move |cc| {
            theme::apply(&cc.egui_ctx);
            let mut app = App {
                tab: if ask_mode { Tab::Quiz } else { Tab::Review },
                ask_mode,
                quiz: None,
                library,
                review: review::Review::new(),
                topics: topics::Topics::new(),
                settings: settings::Settings::new(),
                voice: cfg.voice,
                speaker: Speaker::new(&cfg.tts_voice),
                stt,
                model: cfg.model.clone(),
                cfg,
                lock_touched: Instant::now(),
            };
            if ask_mode {
                app.start_random();
            }
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))
}
