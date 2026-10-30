//! The Settings screen. Edits a draft; SAVE validates and writes the config.
use crate::theme;
use chrono::Local;
use ducky_core::budget;
use ducky_core::config::{Config, Window};
use eframe::egui;

pub struct Settings {
    draft: Option<Config>,
    message: Option<(bool, String)>,
}

/// True when the config was saved and the app should pick up the new values.
pub struct Saved(pub bool);

impl Settings {
    pub fn new() -> Settings {
        Settings { draft: None, message: None }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, live: &mut Config) -> Saved {
        let mut saved = false;
        let draft = self.draft.get_or_insert_with(|| live.clone());

        let used = budget::used_on(Local::now().date_naive());
        ui.label(theme::heading("TODAY"));
        let frac = (used.tokens as f32 / live.daily_token_cap.max(1) as f32).min(1.0);
        ui.add(egui::ProgressBar::new(frac).text(format!("{} / {} tokens   ${:.4} / ${:.2}", used.tokens, live.daily_token_cap, used.cost_usd, live.daily_cost_cap_usd)));
        if frac >= 1.0 || used.cost_usd >= live.daily_cost_cap_usd {
            ui.colored_label(theme::BAD, "Daily cap reached: ducky will not ask more questions today.");
        }
        ui.separator();

        ui.label(theme::heading("WHEN"));
        window_row(ui, "Active hours", &mut draft.active);
        ui.label("Never speak during:");
        let mut remove = None;
        for (i, w) in draft.quiet.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                time_edit(ui, &mut w.from);
                ui.label("to");
                time_edit(ui, &mut w.to);
                if theme::button(ui, "X").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            draft.quiet.remove(i);
        }
        if theme::button(ui, "ADD QUIET TIME").clicked() {
            draft.quiet.push(Window { from: "12:00".into(), to: "13:00".into() });
        }
        ui.horizontal(|ui| {
            ui.label("Questions per hour");
            ui.add(egui::DragValue::new(&mut draft.questions_per_hour).range(0.0..=20.0).speed(0.1));
        });
        ui.horizontal(|ui| {
            ui.label("At least");
            ui.add(egui::DragValue::new(&mut draft.min_gap_minutes).range(0..=600));
            ui.label("minutes apart");
        });
        ui.horizontal(|ui| {
            ui.label("Check every");
            ui.add(egui::DragValue::new(&mut draft.tick_minutes).range(1..=120));
            ui.label("minutes");
        });
        if draft.tick_minutes != live.tick_minutes {
            ui.label(egui::RichText::new("To apply a new check interval, run `ducky-tick install` again.").small());
        }
        ui.separator();

        ui.label(theme::heading("LIMITS"));
        ui.horizontal(|ui| {
            ui.label("Daily tokens");
            ui.add(egui::DragValue::new(&mut draft.daily_token_cap).range(0..=50_000_000).speed(1000));
        });
        ui.horizontal(|ui| {
            ui.label("Daily cost $");
            ui.add(egui::DragValue::new(&mut draft.daily_cost_cap_usd).range(0.0..=1000.0).speed(0.05));
        });
        ui.horizontal(|ui| {
            ui.label("Model");
            ui.text_edit_singleline(&mut draft.model);
        });
        ui.separator();

        ui.label(theme::heading("VOICE"));
        ui.checkbox(&mut draft.voice, "Speak questions and feedback, and show the mic");
        ui.horizontal(|ui| {
            ui.label("Voice");
            ui.add(egui::TextEdit::singleline(&mut draft.tts_voice).hint_text("system default, e.g. Zoe (Premium)"));
        });
        ui.label(egui::RichText::new("See the names with `say -v '?'` in a terminal.").small());
        ui.separator();

        if let Some((ok, m)) = &self.message {
            ui.label(egui::RichText::new(m).color(if *ok { theme::GOOD } else { theme::BAD }));
        }
        let mut reset = false;
        ui.horizontal(|ui| {
            if theme::button(ui, "SAVE").clicked() {
                match draft.validate() {
                    Ok(()) => {
                        // Topic on/off lives on the Topics tab, so keep whatever is live.
                        draft.disabled_topics = live.disabled_topics.clone();
                        *live = draft.clone();
                        match live.save() {
                            Ok(()) => {
                                self.message = Some((true, "Saved.".into()));
                                saved = true;
                            }
                            Err(e) => self.message = Some((false, format!("Could not save: {e}"))),
                        }
                    }
                    Err(e) => self.message = Some((false, e)),
                }
            }
            reset = theme::button(ui, "RESET").clicked();
        });
        if reset {
            self.draft = Some(live.clone());
            self.message = None;
        }
        Saved(saved)
    }
}

fn time_edit(ui: &mut egui::Ui, t: &mut String) {
    let bad = !ducky_core::config::valid_time(t);
    let mut e = egui::TextEdit::singleline(t).desired_width(56.0);
    if bad {
        e = e.text_color(theme::BAD);
    }
    ui.add(e);
}

fn window_row(ui: &mut egui::Ui, label: &str, w: &mut Window) {
    ui.horizontal(|ui| {
        ui.label(label);
        time_edit(ui, &mut w.from);
        ui.label("to");
        time_edit(ui, &mut w.to);
    });
}
