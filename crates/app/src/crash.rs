//! The flatline crash screen: black glass, a blinking red box, our own words.

use egui::{Align2, Color32, FontFamily, FontId, Rect, Stroke, StrokeKind, Ui, pos2, vec2};
use ninsei_core::hub::RunReport;

const RED: Color32 = Color32::from_rgb(0xFF, 0x22, 0x11);
const BOX_H: f32 = 74.0;

/// Paints the crash screen over the whole window. Returns true when clicked.
pub fn show(ui: &mut Ui, report: Option<&RunReport>) -> bool {
    let screen = ui.ctx().content_rect();
    let resp = ui.interact(screen, egui::Id::new("crash"), egui::Sense::click());
    let painter = ui.painter_at(screen);
    painter.rect_filled(screen, 0.0, Color32::BLACK);
    let blink = (ui.input(|i| i.time) * 1.5) as i64 % 2 == 0;
    let r = Rect::from_min_size(
        screen.min + vec2(8.0, 8.0),
        vec2(screen.width() - 16.0, BOX_H),
    );
    if blink {
        painter.rect_stroke(r, 0.0, Stroke::new(6.0, RED), StrokeKind::Inside);
    }
    let font = FontId::new(17.0, FontFamily::Monospace);
    painter.text(
        pos2(r.center().x, r.top() + 20.0),
        Align2::CENTER_CENTER,
        "Deck Failure.   Click anywhere to go home.",
        font.clone(),
        RED,
    );
    let code = report.map_or(0, |r| r.seed as u32);
    painter.text(
        pos2(r.center().x, r.top() + 48.0),
        Align2::CENTER_CENTER,
        format!("Flatline  #00000004.{code:08X}"),
        font,
        RED,
    );
    if let Some(rep) = report {
        let small = FontId::new(13.0, FontFamily::Monospace);
        painter.text(
            pos2(r.center().x, r.bottom() + 24.0),
            Align2::CENTER_TOP,
            format!("lost {}¢ after {} hops", rep.credits, rep.hops),
            small,
            Color32::from_gray(120),
        );
    }
    ui.ctx().request_repaint();
    resp.clicked()
}
