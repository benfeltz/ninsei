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
    /// No frame drawn yet: the frame it opens on, the keys and clicks are the ones that opened
    /// it (a command's Enter, a node's click), never the board's.
    fresh: bool,
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
            fresh: true,
        }
    }

    /// Runs one frame: clock, keys, drawing. Returns what the game should do.
    pub fn show(&mut self, ctx: &Context, game: &dyn Line, p: &Palette) -> Vec<Outcome> {
        let mut out = Vec::new();
        let dt = ctx.input(|i| i.stable_dt).min(0.1);
        out.push(self.session.tick(game, dt));

        // Every keystroke goes down the phone line while the window is up, from the frame after
        // it opens.
        ctx.memory_mut(|m| m.stop_text_input());
        let fresh = std::mem::take(&mut self.fresh);
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
        let keys = if fresh { Vec::new() } else { keys };
        for k in keys {
            let o = self.session.key(game, k);
            // A key that hurried the line is the frame's last: the splash waits for its own.
            let cut = o == Outcome::Cut;
            out.push(o);
            if cut {
                break;
            }
        }
        // While it connects, a click anywhere on the terminal hurries it too.
        let click = ctx
            .input(|i| {
                i.pointer
                    .primary_clicked()
                    .then(|| i.pointer.interact_pos())
                    .flatten()
            })
            .filter(|_| !fresh);

        let blink = (ctx.input(|i| i.time) * 2.0) as i64 % 2 == 0;
        let mut clicked: Option<char> = None;

        let connecting = self.session.connecting();
        let shown = egui::Window::new(Session::title(game))
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

                let keys = self.session.keys(game);
                if let Some(k) = key_bar(ui, &keys, &mut self.color, p) {
                    clicked = Some(k);
                }
            });
        let on_window =
            |at: egui::Pos2| shown.as_ref().is_some_and(|w| w.response.rect.contains(at));
        if connecting && click.is_some_and(on_window) {
            out.push(self.session.skip(game));
        }
        if let Some(k) = clicked {
            out.push(self.session.key(game, k));
        }
        ctx.request_repaint();
        out.retain(|o| *o != Outcome::None);
        out
    }
}

/// The hotkeys as buttons, for mice and thumbs, after the display switch has its place on the
/// right: as many as fit whole, in order (every key still types). Returns the key clicked.
fn key_bar(
    ui: &mut egui::Ui,
    keys: &[(char, String)],
    color: &mut bool,
    p: &Palette,
) -> Option<char> {
    let mut clicked = None;
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let mode = if *color { "ANSI color" } else { "phosphor" };
            if ui
                .button(RichText::new(format!("display: {mode}")).color(p.dim))
                .clicked()
            {
                *color = !*color;
            }
            ui.add_space(ui.spacing().item_spacing.x * 2.0);
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                for (k, label) in keys {
                    let cap = if *k == '\n' {
                        "ENTER".to_string()
                    } else {
                        k.to_string()
                    };
                    let text = format!("[{cap}] {label}").trim_end().to_string();
                    if button_width(ui, &text) > ui.available_width() {
                        break;
                    }
                    if ui.button(RichText::new(text).color(p.fg)).clicked() {
                        clicked = Some(*k);
                    }
                }
            });
        });
    });
    clicked
}

/// How wide a button labelled `text` lays out in `ui`, padding and all.
fn button_width(ui: &egui::Ui, text: &str) -> f32 {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let w = ui.fonts_mut(|f| {
        f.layout_no_wrap(text.to_string(), font, Color32::WHITE)
            .size()
            .x
    });
    w + ui.spacing().button_padding.x * 2.0
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

    use crate::uitest::{click, text};
    use ninsei_core::bbs::Screen;

    /// One frame of the BBS window on `line`, with `events`; what it returned.
    fn show(h: &mut Harness, v: &mut BbsView, line: &dyn Line, events: Vec<Event>) -> Vec<Outcome> {
        let data = ninsei_core::Data::builtin().unwrap();
        let p = Palette::from_def(&data.themes[0]);
        let mut out = Vec::new();
        h.frame(events, |ui| out = v.show(&ui.ctx().clone(), line, &p));
        out
    }

    fn run() -> ninsei_core::Game {
        ninsei_core::Game::new(ninsei_core::Data::builtin().unwrap(), 3)
    }

    #[test]
    fn a_key_skips_a_run_s_connect_that_frame_and_only_that() {
        let g = run();
        let mut h = Harness::new(1280.0, 800.0);
        let mut v = BbsView::new();
        show(&mut h, &mut v, &g, Vec::new());
        assert!(v.session.connecting());
        // Two keys in one frame: the first hurries the line, the second isn't the splash's.
        let mut ev = text("x");
        ev.extend(text("\n"));
        assert_eq!(show(&mut h, &mut v, &g, ev), [Outcome::Cut]);
        assert_eq!(v.session.screen, Screen::Splash);
        show(&mut h, &mut v, &g, Vec::new());
        assert_eq!(v.session.screen, Screen::Splash, "it waits for its own key");
        show(&mut h, &mut v, &g, text("\n"));
        assert_eq!(v.session.screen, Screen::Main);
    }

    #[test]
    fn the_keys_and_clicks_that_opened_it_aren_t_the_board_s() {
        let g = run();
        let mut h = Harness::new(1280.0, 800.0);
        let mut v = BbsView::new();
        let mut ev = text("\n");
        ev.extend(crate::uitest::key(Key::Enter));
        ev.extend(click(egui::pos2(640.0, 400.0)));
        assert!(show(&mut h, &mut v, &g, ev).is_empty());
        assert!(v.session.connecting(), "the connect plays");
        assert_eq!(show(&mut h, &mut v, &g, text("x")), [Outcome::Cut]);
    }

    #[test]
    fn a_click_on_the_terminal_skips_a_run_s_connect() {
        let g = run();
        let mut h = Harness::new(1280.0, 800.0);
        let mut v = BbsView::new();
        show(&mut h, &mut v, &g, Vec::new());
        show(&mut h, &mut v, &g, Vec::new());
        // On the terminal, under the title bar.
        let middle = h.find(&Session::title(&g)).expect("the window").center() + vec2(0.0, 80.0);
        // A click off the window does nothing.
        show(&mut h, &mut v, &g, click(egui::pos2(5.0, 795.0)));
        assert!(v.session.connecting());
        assert_eq!(show(&mut h, &mut v, &g, click(middle)), [Outcome::Cut]);
        assert_eq!(v.session.screen, Screen::Splash);
        // Past the connect a click is only a click.
        assert!(show(&mut h, &mut v, &g, click(middle)).is_empty());
        assert_eq!(v.session.screen, Screen::Splash);
    }

    #[test]
    fn the_run_s_window_names_its_address_and_the_screen_hints_nothing() {
        let g = run();
        let mut h = Harness::new(1280.0, 800.0);
        let mut v = BbsView::new();
        show(&mut h, &mut v, &g, Vec::new());
        let title = Session::title(&g);
        assert!(h.find(&title).is_some(), "{title}");
        assert!(title.contains(&g.bbs_address()) && !title.contains("8N1"));
        show(&mut h, &mut v, &g, Vec::new());
        let words: Vec<String> = h.painted_text().into_iter().map(|(t, _)| t).collect();
        assert!(
            words.iter().any(|t| t.contains(&g.bbs_address())),
            "the PAD's call is on screen: {words:?}"
        );
        for w in ["press", "Press", "key", "skip", "click", "ENTER"] {
            assert!(!words.iter().any(|t| t.contains(w)), "{w} in {words:?}");
        }
    }

    #[test]
    fn a_home_call_can_t_be_hurried() {
        let d = ninsei_core::Data::builtin().unwrap();
        let mut save = ninsei_core::hub::Save::default();
        save.ensure_key(1);
        let call = save.home_call(&d, &d.rig, 0);
        let mut h = Harness::new(1280.0, 800.0);
        let mut v = BbsView::new();
        show(&mut h, &mut v, &call, Vec::new());
        assert!(show(&mut h, &mut v, &call, text("x")).is_empty());
        let middle = h.find(&Session::title(&call)).unwrap().center() + egui::vec2(0.0, 80.0);
        assert!(show(&mut h, &mut v, &call, click(middle)).is_empty());
        assert!(v.session.connecting(), "the full handshake");
    }

    #[test]
    fn the_key_bar_never_runs_into_the_display_switch() {
        // Ten areas (the Gutter's, its source area the tenth) and the way back, in a bar
        // narrower than they need.
        let data = ninsei_core::Data::builtin().unwrap();
        let p = Palette::from_def(&data.themes[0]);
        let mut keys: Vec<(char, String)> =
            "1234567890".chars().map(|c| (c, String::new())).collect();
        keys.push(('Q', "back".into()));
        let mut h = Harness::new(1280.0, 800.0);
        let mut color = true;
        for _ in 0..2 {
            h.frame(Vec::new(), |ui| {
                let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(420.0, 40.0));
                let mut bar = ui.new_child(egui::UiBuilder::new().max_rect(rect));
                key_bar(&mut bar, &keys, &mut color, &p);
            });
        }
        let switch = h.find("display: ANSI color").expect("the display switch");
        let drawn: Vec<(String, egui::Rect)> = h
            .painted_text()
            .into_iter()
            .filter(|(t, _)| t.starts_with('['))
            .collect();
        assert!(!drawn.is_empty() && drawn.len() < keys.len(), "{drawn:?}");
        for (t, r) in &drawn {
            assert!(r.right() < switch.left(), "{t} {r:?} runs into {switch:?}");
        }
        // The ones drawn are the first ones, in order.
        assert!(drawn[0].0.starts_with("[1]"), "{drawn:?}");
    }

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
