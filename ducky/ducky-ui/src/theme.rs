//! All colours, fonts and chrome live here so the look can be reworked later without touching the app logic.
use eframe::egui::{self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, Stroke};
use std::sync::Arc;

pub const INK: Color32 = Color32::from_rgb(0x2b, 0x2b, 0x2b);
pub const TITLE_BAR: Color32 = Color32::from_rgb(0x92, 0x7f, 0x97);
pub const PAPER: Color32 = Color32::from_rgb(0xff, 0xf9, 0xdb);
pub const GOOD: Color32 = Color32::from_rgb(0x2e, 0x7d, 0x32);
pub const WARN: Color32 = Color32::from_rgb(0xb2, 0x6a, 0x00);
pub const BAD: Color32 = Color32::from_rgb(0xb7, 0x1c, 0x1c);
pub const BORDER: f32 = 3.0;

pub fn pixel_font() -> FontFamily {
    FontFamily::Name("pixel".into())
}

pub fn apply(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "silkscreen".into(),
        Arc::new(FontData::from_static(include_bytes!("../assets/Silkscreen-Regular.ttf"))),
    );
    fonts.families.insert(pixel_font(), vec!["silkscreen".into()]);
    ctx.set_fonts(fonts);

    ctx.all_styles_mut(|style| {
        let v = &mut style.visuals;
        v.override_text_color = Some(INK);
        v.panel_fill = PAPER;
        v.extreme_bg_color = Color32::WHITE;
        v.selection.bg_fill = TITLE_BAR;
        v.selection.stroke = Stroke::new(1.0, INK);
        for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
            w.corner_radius = CornerRadius::ZERO;
            w.bg_stroke = Stroke::new(2.0, INK);
            w.fg_stroke = Stroke::new(1.5, INK);
        }
        v.widgets.inactive.weak_bg_fill = Color32::from_rgb(0xea, 0xe0, 0xf0);
        v.widgets.hovered.weak_bg_fill = TITLE_BAR;
        v.widgets.active.weak_bg_fill = Color32::from_rgb(0x7a, 0x68, 0x80);
        style.spacing.item_spacing = egui::vec2(8.0, 10.0);
        style.spacing.button_padding = egui::vec2(12.0, 6.0);
    });
}

/// A chunky button in the pixel font.
pub fn button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(egui::Button::new(egui::RichText::new(label).family(pixel_font()).size(16.0)))
}

pub fn heading(text: &str) -> egui::RichText {
    egui::RichText::new(text).family(pixel_font()).size(20.0)
}

/// Draws the title bar with fake "- o x" controls. Returns true when the close box is clicked.
pub fn title_bar(ui: &mut egui::Ui, title: &str) -> bool {
    let mut close = false;
    egui::Frame::new().fill(TITLE_BAR).stroke(Stroke::new(BORDER, INK)).inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
        ui.horizontal(|ui| {
            let drag = ui.label(egui::RichText::new(title).family(pixel_font()).size(18.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Right to left: close first.
                close = ui.add(egui::Button::new(egui::RichText::new("x").family(pixel_font()).size(16.0))).clicked();
                if ui.add(egui::Button::new(egui::RichText::new("o").family(pixel_font()).size(16.0))).clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false))));
                }
                if ui.add(egui::Button::new(egui::RichText::new("-").family(pixel_font()).size(16.0))).clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }
            });
            let _ = drag;
        });
    });
    // Dragging anywhere on the bar moves the window.
    let bar = ui.min_rect();
    let r = ui.interact(egui::Rect::from_min_max(bar.min, egui::pos2(bar.max.x - 110.0, bar.max.y)), ui.make_persistent_id("title_drag"), egui::Sense::drag());
    if r.drag_started() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    close
}

/// A tab button; the selected one is filled like the title bar.
pub fn tab(ui: &mut egui::Ui, selected: bool, label: &str) -> egui::Response {
    let text = egui::RichText::new(label).family(pixel_font()).size(15.0);
    let b = egui::Button::new(text).fill(if selected { TITLE_BAR } else { PAPER });
    ui.add(b)
}

/// Segmented bar: covered (green), missing (amber), wrong (red). Empty outline when there is no data.
pub fn bar(ui: &mut egui::Ui, covered: u32, missing: u32, wrong: u32, width: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 14.0), egui::Sense::hover());
    let total = (covered + missing + wrong).max(1) as f32;
    let mut x = rect.min.x;
    for (n, colour) in [(covered, GOOD), (missing, WARN), (wrong, BAD)] {
        let w = rect.width() * n as f32 / total;
        if n > 0 {
            ui.painter().rect_filled(egui::Rect::from_min_size(egui::pos2(x, rect.min.y), egui::vec2(w, rect.height())), 0.0, colour);
        }
        x += w;
    }
    ui.painter().rect_stroke(rect, 0.0, Stroke::new(2.0, INK), egui::StrokeKind::Inside);
}
