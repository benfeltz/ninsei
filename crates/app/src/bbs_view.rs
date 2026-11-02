//! The BBS window: an 80x24 ANSI terminal floating over the playfield.
//! Draws whatever [`Session::render`] returns and feeds it keypresses.

use egui::{
    Align2, Color32, Context, Event, FontFamily, FontId, Key, Painter, Rect, RichText, Sense,
    Stroke, pos2, vec2,
};
use ninsei_core::ansi;
use ninsei_core::bbs::{COLS, Line, Outcome, ROWS, Session};

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

    fn fg(&self, i: u8, p: &Palette) -> Color32 {
        if self.color { cga(i) } else { mono(i, p) }
    }

    fn bg(&self, i: u8, p: &Palette) -> Color32 {
        if i == 0 {
            Color32::TRANSPARENT
        } else if self.color {
            cga(i)
        } else {
            mono(i, p).gamma_multiply(0.5)
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
        let font = FontId::new(FONT_SIZE, FontFamily::Monospace);
        let cw = ctx.fonts_mut(|f| f.glyph_width(&font, 'M'));
        let size = vec2(
            cw * COLS as f32 + PAD * 2.0,
            ROW_HEIGHT * ROWS as f32 + PAD * 2.0,
        );
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
                let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                let painter = ui.painter_at(rect);
                let bg = if self.color { Color32::BLACK } else { p.bg };
                painter.rect_filled(rect, 0.0, bg);
                let (mut fg_c, mut bg_c) = (ansi::DEFAULT_FG, ansi::DEFAULT_BG);
                for (row, line) in self.session.render(game).iter().enumerate() {
                    let line = if blink {
                        line.clone()
                    } else {
                        line.replace("|15_", "|15 ")
                    };
                    let top = rect.top() + PAD + row as f32 * ROW_HEIGHT;
                    let mut col = 0usize;
                    for span in ansi::parse_line(&line, &mut fg_c, &mut bg_c) {
                        let fg = self.fg(span.fg, p);
                        let bgc = self.bg(span.bg, p);
                        let x0 = rect.left() + PAD + col as f32 * cw;
                        let n = span.text.chars().count();
                        if bgc != Color32::TRANSPARENT {
                            let r =
                                Rect::from_min_size(pos2(x0, top), vec2(cw * n as f32, ROW_HEIGHT));
                            painter.rect_filled(r, 0.0, bgc);
                        }
                        // Runs of ordinary glyphs go out as text; block elements are painted as
                        // solid cell rects so art joins up with no font gaps.
                        let mut run = String::new();
                        let mut run_col = col;
                        for ch in span.text.chars() {
                            if let Some(shape) = block(ch) {
                                flush(
                                    &painter,
                                    &mut run,
                                    pos2(rect.left() + PAD + run_col as f32 * cw, top),
                                    &font,
                                    fg,
                                );
                                let cell = Rect::from_min_size(
                                    pos2(rect.left() + PAD + col as f32 * cw, top),
                                    vec2(cw, ROW_HEIGHT),
                                );
                                paint_block(&painter, cell, shape, fg);
                                col += 1;
                                run_col = col;
                            } else {
                                run.push(ch);
                                col += 1;
                            }
                        }
                        flush(
                            &painter,
                            &mut run,
                            pos2(rect.left() + PAD + run_col as f32 * cw, top),
                            &font,
                            fg,
                        );
                    }
                }

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

#[derive(Clone, Copy)]
enum Block {
    /// A filled part of the cell: (x0, y0, x1, y1) as fractions.
    Part(f32, f32, f32, f32),
    /// The whole cell in the foreground colour at this opacity (░▒▓).
    Shade(f32),
}

fn block(ch: char) -> Option<Block> {
    Some(match ch {
        '█' => Block::Part(0.0, 0.0, 1.0, 1.0),
        '▀' => Block::Part(0.0, 0.0, 1.0, 0.5),
        '▄' => Block::Part(0.0, 0.5, 1.0, 1.0),
        '▌' => Block::Part(0.0, 0.0, 0.5, 1.0),
        '▐' => Block::Part(0.5, 0.0, 1.0, 1.0),
        '▪' => Block::Part(0.25, 0.3, 0.75, 0.7),
        '░' => Block::Shade(0.25),
        '▒' => Block::Shade(0.5),
        '▓' => Block::Shade(0.75),
        _ => return None,
    })
}

fn paint_block(painter: &Painter, cell: Rect, b: Block, fg: Color32) {
    match b {
        Block::Part(x0, y0, x1, y1) => {
            let r = Rect::from_min_max(
                pos2(
                    cell.left() + cell.width() * x0,
                    cell.top() + cell.height() * y0,
                ),
                pos2(
                    cell.left() + cell.width() * x1,
                    cell.top() + cell.height() * y1,
                ),
            );
            // Snap outward a hair so neighbouring cells meet with no seam.
            painter.rect_filled(r.expand(0.25), 0.0, fg);
        }
        Block::Shade(a) => {
            painter.rect_filled(cell, 0.0, fg.gamma_multiply(a));
        }
    }
}

/// Draws a run of ordinary glyphs starting at cell `at` (the cell's top-left corner).
fn flush(painter: &Painter, run: &mut String, at: egui::Pos2, font: &FontId, fg: Color32) {
    if run.is_empty() {
        return;
    }
    let pos = pos2(at.x, at.y + (ROW_HEIGHT - FONT_SIZE) / 2.0 - 1.0);
    painter.text(pos, Align2::LEFT_TOP, std::mem::take(run), font.clone(), fg);
}
