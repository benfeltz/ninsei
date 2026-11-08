//! The BBS window: an 80x24 ANSI terminal floating over the playfield.
//! Draws whatever [`Session::render`] returns and feeds it keypresses.

use egui::{
    Align2, Color32, Context, Event, FontFamily, FontId, Key, RichText, Sense, Stroke, vec2,
};
use ninsei_core::ansi;
use ninsei_core::bbs::{COLS, Line, Outcome, ROWS, Session};

use crate::kit::Cells;
use crate::theme::Palette;

const FONT_SIZE: f32 = 13.0;
/// Rows packed tight so block art joins up.
const ROW_HEIGHT: f32 = 15.0;
const PAD: f32 = 10.0;

/// The 16 CGA colours every ANSI artist painted with.
const CGA: [u32; 16] = [
    0x000000, 0x0000AA, 0x00AA00, 0x00AAAA, 0xAA0000, 0xAA00AA, 0xAA5500, 0xAAAAAA, 0x555555,
    0x5555FF, 0x55FF55, 0x55FFFF, 0xFF5555, 0xFF55FF, 0xFFFF55, 0xFFFFFF,
];

pub struct BbsView {
    pub session: Session,
    /// The window's layer: over the playfield in a run, over the desktop at home.
    pub order: egui::Order,
    /// Real ANSI colours, or the phosphor in four intensities.
    pub color: bool,
}

fn cga(i: u8) -> Color32 {
    let c = CGA[(i & 15) as usize];
    Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

/// Maps a CGA colour onto the phosphor by brightness.
pub(crate) fn mono(i: u8, p: &Palette) -> Color32 {
    let c = cga(i);
    let luma = 0.3 * c.r() as f32 + 0.59 * c.g() as f32 + 0.11 * c.b() as f32;
    match luma as u32 {
        0 => Color32::TRANSPARENT,
        1..=60 => p.faint,
        61..=130 => p.dim,
        131..=200 => p.fg,
        _ => p.bright,
    }
}

impl BbsView {
    pub fn new() -> Self {
        Self {
            session: Session::new(),
            order: egui::Order::Middle,
            color: true,
        }
    }

    /// Runs one frame: clock, keys, drawing. Returns what the game should do.
    pub fn show(&mut self, ctx: &Context, game: &dyn Line, p: &Palette) -> Vec<Outcome> {
        let mut out = Vec::new();
        let dt = ctx.input(|i| i.stable_dt).min(0.1);
        out.push(self.session.tick(game, dt));

        // Every keystroke goes down the phone line while the window is up.
        ctx.memory_mut(|m| m.stop_text_input());
        let keys: Vec<char> = ctx.input(|i| {
            i.events
                .iter()
                .flat_map(|e| match e {
                    Event::Text(t) => t.chars().collect(),
                    Event::Key {
                        key: Key::Enter,
                        pressed: true,
                        ..
                    } => vec!['\n'],
                    Event::Key {
                        key: Key::Escape,
                        pressed: true,
                        ..
                    } => vec!['\x1b'],
                    _ => Vec::new(),
                })
                .collect()
        });
        for k in keys {
            out.push(self.session.key(game, k));
        }

        let b = game.board();
        let blink = (ctx.input(|i| i.time) * 2.0) as i64 % 2 == 0;
        let mut clicked: Option<char> = None;

        egui::Window::new(format!("TELIX 3.51 — {}  {} 8N1  ANSI-BBS", b.name, b.baud))
            .id(egui::Id::new("bbs"))
            .order(self.order)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, -20.0))
            .collapsible(false)
            .resizable(false)
            .title_bar(true)
            .frame(
                egui::Frame::new()
                    .fill(Color32::BLACK)
                    .stroke(Stroke::new(1.0, p.fg))
                    .inner_margin(4.0),
            )
            .show(ctx, |ui| {
                let lines: Vec<String> = self
                    .session
                    .render(game)
                    .into_iter()
                    .map(|l| if blink { l } else { l.replace("|15_", "|15 ") })
                    .collect();
                paint_screen(ui, &lines, self.color, p);

                // Hotkeys as buttons, for mice and thumbs.
                ui.horizontal(|ui| {
                    for (k, label) in self.session.keys(game) {
                        let cap = if k == '\n' {
                            "ENTER".to_string()
                        } else {
                            k.to_string()
                        };
                        let txt = RichText::new(format!("[{cap}] {label}").trim_end().to_string())
                            .color(p.fg);
                        if ui.button(txt).clicked() {
                            clicked = Some(k);
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mode = if self.color { "ANSI color" } else { "phosphor" };
                        if ui
                            .button(RichText::new(format!("display: {mode}")).color(p.dim))
                            .clicked()
                        {
                            self.color = !self.color;
                        }
                    });
                });
            });
        if let Some(k) = clicked {
            out.push(self.session.key(game, k));
        }
        ctx.request_repaint();
        out.retain(|o| *o != Outcome::None);
        out
    }
}

/// Paints pipe-coded lines as an 80x24 ANSI terminal: true CGA colours, or the phosphor (`p`)
/// in four intensities. The BBS window and the QWK reader both draw through it.
pub fn paint_screen(ui: &mut egui::Ui, lines: &[String], color: bool, p: &Palette) {
    paint_screen_at(ui, lines, color, p, 1.0);
}

/// The room the 80x24 terminal takes at its own size, padding and all.
pub fn screen_size(ui: &egui::Ui) -> egui::Vec2 {
    let font = FontId::new(FONT_SIZE, FontFamily::Monospace);
    Cells::sized(ui, egui::Pos2::ZERO, font, Some(ROW_HEIGHT), (COLS, ROWS))
        .rect
        .size()
        + vec2(PAD, PAD) * 2.0
}

/// [`paint_screen`], in a room smaller than the terminal: the whole screen, its type made
/// smaller to fit the width and the height left in view (never cut at the room's edges, nor
/// pushed past a scroll fold). At its own size when it fits.
pub fn paint_screen_fit(ui: &mut egui::Ui, lines: &[String], color: bool, p: &Palette) {
    let size = screen_size(ui);
    let tall = ui.clip_rect().bottom() - ui.cursor().top();
    let scale = (ui.available_width() / size.x).min(tall / size.y).min(1.0);
    paint_screen_at(ui, lines, color, p, scale);
}

fn paint_screen_at(ui: &mut egui::Ui, lines: &[String], color: bool, p: &Palette, scale: f32) {
    let font = FontId::new(FONT_SIZE * scale, FontFamily::Monospace);
    let fgc = |i: u8| if color { cga(i) } else { mono(i, p) };
    let bgc_of = |i: u8| {
        if i == 0 {
            Color32::TRANSPARENT
        } else if color {
            cga(i)
        } else {
            mono(i, p).gamma_multiply(0.5)
        }
    };
    // The terminal's own cells: its font, rows packed tight, inside the padding.
    let (resp, g) = Cells::allocate(
        ui,
        font,
        Some(ROW_HEIGHT * scale),
        (COLS, ROWS),
        vec2(PAD, PAD) * scale,
        Sense::hover(),
    );
    let rect = resp.rect;
    let painter = ui.painter_at(rect);
    let bg = if color { Color32::BLACK } else { p.bg };
    painter.rect_filled(rect, 0.0, bg);
    let (mut fg_c, mut bg_c) = (ansi::DEFAULT_FG, ansi::DEFAULT_BG);
    for (row, line) in lines.iter().enumerate().take(ROWS) {
        let mut col = 0usize;
        for span in ansi::parse_line(line, &mut fg_c, &mut bg_c) {
            let fg = fgc(span.fg);
            let bgc = bgc_of(span.bg);
            let n = span.text.chars().count();
            if bgc != Color32::TRANSPARENT {
                g.fill(&painter, col, row, n.min(COLS.saturating_sub(col)), 1, bgc);
            }
            // Block elements are painted as solid parts of their cells so art joins up with
            // no font gaps; runs of other glyphs go out as one piece of text from their first
            // cell (the screen repaints every frame). Nothing past the 80th column.
            let mut run = String::new();
            let mut from = col;
            for ch in span.text.chars() {
                if col >= COLS {
                    break;
                }
                if g.block(&painter, col, row, ch, fg) {
                    g.run(&painter, from, row, &std::mem::take(&mut run), fg);
                    from = col + 1;
                } else {
                    run.push(ch);
                }
                col += 1;
            }
            g.run(&painter, from, row, &run, fg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::uitest::Harness;

    #[test]
    fn a_screen_paints_in_its_cells_with_block_art_solid() {
        let data = ninsei_core::Data::builtin().unwrap();
        let p = Palette::from_def(&data.themes[0]);
        let mut lines = vec!["|15█|07A".to_string()];
        lines.extend((1..ROWS).map(|_| "x".repeat(COLS)));
        let mut h = Harness::new(1280.0, 800.0);
        let mut screen = egui::Rect::NOTHING;
        h.frame(Vec::new(), |ui| {
            paint_screen(ui, &lines, true, &p);
            screen = ui.min_rect();
        });
        let shapes = h.painted();
        // Everything inside the 80x24 screen it allocated.
        for s in shapes {
            let r = s.visual_bounding_rect();
            assert!(
                screen.expand(1.0).contains_rect(r),
                "{r:?} outside {screen:?}"
            );
        }
        // The block is a solid white rect, not a glyph; the A is a glyph one cell over.
        let white = cga(15);
        let block = shapes
            .iter()
            .find_map(|s| match s {
                egui::Shape::Rect(r) if r.fill == white => Some(r.rect),
                _ => None,
            })
            .expect("the █ as a rect");
        let a = shapes
            .iter()
            .find_map(|s| match s {
                egui::Shape::Text(t) if t.galley.text() == "A" => Some(t.visual_bounding_rect()),
                _ => None,
            })
            .expect("the A");
        assert!(
            a.left() >= block.right() - 1.0,
            "the A sits in the next cell"
        );
        assert!(
            !shapes
                .iter()
                .any(|s| matches!(s, egui::Shape::Text(t) if t.galley.text().contains('█')))
        );
    }

    #[test]
    fn nothing_past_the_80th_column_or_24th_row_in_either_mode() {
        let data = ninsei_core::Data::builtin().unwrap();
        let p = Palette::from_def(&data.themes[0]);
        // A row 90 wide on a blue background with block art past the edge, and 26 rows.
        let mut lines = vec![format!("|17|15{}{}", "x".repeat(78), "█".repeat(12))];
        lines.extend((1..ROWS + 2).map(|_| "|20|14".to_string() + &"y".repeat(COLS)));
        for color in [true, false] {
            let mut h = Harness::new(1280.0, 800.0);
            let mut screen = egui::Rect::NOTHING;
            h.frame(Vec::new(), |ui| {
                paint_screen(ui, &lines, color, &p);
                screen = ui.min_rect();
            });
            for s in h.painted() {
                let r = s.visual_bounding_rect();
                assert!(
                    screen.expand(1.0).contains_rect(r),
                    "color {color}: {r:?} outside {screen:?}"
                );
            }
        }
    }
}
