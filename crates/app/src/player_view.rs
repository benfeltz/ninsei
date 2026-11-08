//! TRAX, the music player, as each desktop dresses it. On AnarchyOS it's a TUI player after
//! cmus 2.x (its playlist view and status line) with Winamp 2's transport and spectrum: three
//! boxes on the kit's cells, whole cells only, every colour the desk's. What it shows is
//! core's (`ninsei_core::player`); this only draws and turns keys and clicks into
//! [`WbOut::Player`] acts. The other desktops show a plain listing until they dress it.

use egui::{Color32, Key, Rect, Sense, Ui};
use ninsei_core::Data;
use ninsei_core::hub::Save;
use ninsei_core::player::{self, Act, Transport};

use crate::home::{WbOut, accent, ink, muted, shadow};
use crate::kit::cells::Unlit;
use crate::kit::{Cells, List};

/// The player's view between frames: the row the cursor is on and the list's window.
#[derive(Default)]
pub struct PlayerUi {
    pub sel: Option<usize>,
    pub(crate) list: List,
    /// The pass it was last drawn on: the app hands it the speaker's samples only while
    /// it's on screen.
    pub(crate) drawn: Option<u64>,
}

/// What the speaker is playing, for the spectrum: the last samples and their rate. Filled by
/// the app (`Speaker::listen`, as the vis pane is) only while the player is on screen.
#[derive(Default)]
pub struct Heard {
    pub samples: Vec<f32>,
    pub rate: u32,
}

/// The grid the tile works in: a playlist of three rows, the spectrum and the status box.
pub const NEED: crate::kit::fit::Fit = crate::kit::fit::Fit { cols: 40, rows: 13 };

/// Rows the spectrum box takes, its border included.
const VIS_H: usize = 5;
/// Rows the status box takes, its border included.
const NOW_H: usize = 4;
/// Columns a playlist row needs to show its author too; narrower, the author is dropped.
const WIDE: usize = 2 + ninsei_core::data::TUNE_TITLE_MAX + 2 + 20 + 2 + 5 + 2;

/// The gradient the spectrum climbs, darkest first: never down at the empty cells' colour, so
/// a lit cell never disappears on a mono tube.
fn ramp() -> [Color32; 3] {
    [muted(), ink(), accent()]
}

/// This frame's acts and cursor moves from the keyboard, while the tile has it.
fn keys(ui: &Ui, rows: usize, sel: &mut Option<usize>) -> Vec<Act> {
    let mut acts = Vec::new();
    ui.input(|i| {
        for e in &i.events {
            let egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = e
            else {
                continue;
            };
            if modifiers.ctrl || modifiers.alt || modifiers.command {
                continue;
            }
            let last = rows.saturating_sub(1);
            match key {
                Key::Space => acts.push(Act::Toggle),
                Key::N => acts.push(Act::Next),
                Key::P => acts.push(Act::Prev),
                Key::Plus | Key::Equals => acts.push(Act::Louder),
                Key::Minus => acts.push(Act::Softer),
                Key::R => acts.push(Act::Repeat),
                Key::S => acts.push(Act::Shuffle),
                Key::J | Key::ArrowDown if rows > 0 => {
                    *sel = Some(sel.map_or(0, |s| (s + 1).min(last)));
                }
                Key::K | Key::ArrowUp if rows > 0 => {
                    *sel = Some(sel.map_or(0, |s| s.saturating_sub(1)));
                }
                Key::Enter => {
                    if let Some(s) = *sel {
                        acts.push(Act::Pick(s));
                    }
                }
                _ => {}
            }
        }
    });
    acts
}

/// Draws the AnarchyOS tile in the room `ui` has (at least [`NEED`]). `keys`: the tile has
/// the keyboard.
#[allow(clippy::too_many_arguments)]
pub fn tile(
    ui: &mut Ui,
    data: &Data,
    save: &Save,
    t: &Transport,
    pui: &mut PlayerUi,
    heard: &Heard,
    keys_on: bool,
    out: &mut Vec<WbOut>,
) {
    pui.drawn = Some(ui.ctx().cumulative_pass_nr());
    let rows = player::rows(save, data);
    if pui.sel.is_none_or(|s| s >= rows.len()) {
        pui.sel = player::current(save, data).or((!rows.is_empty()).then_some(0));
    }
    if keys_on {
        // Its keys are the player's: nothing behind it types them.
        ui.ctx().memory_mut(|m| m.stop_text_input());
        for a in keys(ui, rows.len(), &mut pui.sel) {
            out.push(WbOut::Player(a));
        }
    }

    let room = ui.available_rect_before_wrap();
    let (rect, _) = ui.allocate_exact_size(room.size(), Sense::hover());
    let g = Cells::fit(ui, rect, muted(), shadow(), muted());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 0.0, shadow());
    let list_h = g.rows.saturating_sub(VIS_H + NOW_H).max(3);

    // The playlist: cmus's view, one tune a row, the cursor in inverse, the playing one lit.
    g.boxed(&p, 0, 0, g.cols, list_h, "playlist", accent());
    let inner_w = g.cols.saturating_sub(4);
    let shown = list_h.saturating_sub(2);
    let range = pui.list.view(rows.len(), pui.sel, shown);
    let wide = g.cols >= WIDE;
    for i in range {
        let row = &rows[i];
        let r = 1 + pui.list.row(i);
        let (c, w) = (2, inner_w);
        let len_col = c + w - 5;
        let title_w = if wide {
            ninsei_core::data::TUNE_TITLE_MAX
        } else {
            len_col.saturating_sub(c + 3)
        };
        let mark = if row.current { "▶ " } else { "  " };
        let sel = pui.sel == Some(i);
        let (fg, bg) = if sel {
            (shadow(), if keys_on { accent() } else { muted() })
        } else if row.current {
            (accent(), shadow())
        } else {
            (ink(), shadow())
        };
        if sel {
            g.fill(&p, c, r, w, 1, bg);
        }
        g.text(&p, c, r, mark, fg);
        g.text_max(&p, c + 2, r, row.title, title_w, fg);
        if wide {
            let by = c + 2 + title_w + 2;
            g.text_max(&p, by, r, row.by, len_col.saturating_sub(by + 1), fg);
        }
        g.text_right(&p, len_col + 5, r, &row.length, fg);
        let hit = ui.interact(
            g.at(c, r, w, 1),
            ui.id().with(("trax-row", i)),
            Sense::click(),
        );
        if hit.double_clicked() {
            out.push(WbOut::Player(Act::Pick(i)));
        } else if hit.clicked() {
            pui.sel = Some(i);
        }
    }
    pui.list.mark(&g, &p, 0, 0, g.cols, 12, muted());
    let wheel = ui.input(|i| i.smooth_scroll_delta.y);
    if wheel != 0.0 && ui.rect_contains_pointer(g.at(0, 0, g.cols, list_h)) {
        pui.list.wheel(-wheel / g.ch);
    }

    // The spectrum: Winamp's bars as columns of whole cells, from the speaker as the vis pane
    // hears it.
    let top = list_h;
    g.boxed(&p, 0, top, g.cols, VIS_H, "spectrum", muted());
    let h = VIS_H - 2;
    let n = (g.cols.saturating_sub(4) / 2).max(1);
    let samples: &[f32] = if heard.samples.is_empty() {
        &[0.0; 64]
    } else {
        &heard.samples
    };
    let bars = crate::vispane::bars(ui, "trax", samples, heard.rate.max(1), n);
    let lit = ninsei_core::scope::cells_lit(&bars, h);
    let colours: Vec<Color32> = (0..h)
        .map(|k| ramp()[(k * (ramp().len() - 1)) / (h - 1).max(1)])
        .collect();
    for (i, on) in lit.into_iter().enumerate() {
        g.meter_up(&p, 2 + i * 2, top + h, h, on, &colours);
    }

    // The status box: what's playing, cmus's `artist - title`, and Winamp's position bar;
    // the transport and the modes in its top edge, the level and the keys in the bottom one.
    let top = top + VIS_H;
    g.boxed(
        &p,
        0,
        top,
        g.cols,
        NOW_H,
        &player::status(save, t),
        accent(),
    );
    let vol = player::volume(save);
    g.note(
        &p,
        g.cols - vol.chars().count() - 4,
        top + NOW_H - 1,
        &vol,
        ink(),
    );
    let mut c = 2;
    for (k, what) in player::KEYS {
        let s = format!("{k} {what}");
        let room = g.cols.saturating_sub(vol.chars().count() + 6);
        if c + s.chars().count() + 2 > room {
            break;
        }
        c += g.note(&p, c, top + NOW_H - 1, &s, muted()) + 1;
    }
    match player::now(save, data, t) {
        Some((head, at, total)) => {
            g.text_max(&p, 2, top + 1, &head, inner_w, ink());
            let (a, b) = (player::clock(at), player::clock(total));
            let bar_c = 2 + a.chars().count() + 1;
            let bar_w = inner_w.saturating_sub(a.chars().count() + b.chars().count() + 2);
            g.text(&p, 2, top + 2, &a, ink());
            let frac = if total > 0.0 { at / total } else { 0.0 };
            g.gauge(
                &p,
                bar_c,
                top + 2,
                bar_w,
                frac * bar_w as f32,
                |_| accent(),
                Unlit::Dot(muted()),
            );
            g.text_right(&p, 2 + inner_w, top + 2, &b, ink());
            // A click on the bar seeks there.
            let bar = g.at(bar_c, top + 2, bar_w, 1);
            let hit = ui.interact(bar, ui.id().with("trax-seek"), Sense::click());
            if hit.clicked()
                && let Some(pos) = hit.interact_pointer_pos()
            {
                out.push(WbOut::Player(Act::Seek(seek_at(bar, pos.x, total))));
            }
        }
        None => {
            g.text(&p, 2, top + 1, "-", muted());
        }
    }
}

/// Milliseconds into a tune `total` seconds long for a click at `x` on a position bar.
fn seek_at(bar: Rect, x: f32, total: f32) -> u32 {
    (((x - bar.left()) / bar.width().max(1.0)).clamp(0.0, 1.0) * total * 1000.0) as u32
}

/// The plain listing the other desktops show until they dress the player: the playlist as
/// rows to click, the status, and the transport as buttons.
pub fn plain(ui: &mut Ui, data: &Data, save: &Save, t: &Transport, out: &mut Vec<WbOut>) {
    head(ui, save, data, t);
    ui.separator();
    for (i, row) in player::rows(save, data).iter().enumerate() {
        let text = format!(
            "{} {}  {}  {}",
            if row.current { "▶" } else { " " },
            row.title,
            row.by,
            row.length
        );
        if ui.selectable_label(row.current, text).clicked() {
            out.push(WbOut::Player(Act::Pick(i)));
        }
    }
    ui.separator();
    ui.horizontal(|ui| {
        for (label, act) in [
            ("Play/Pause", Act::Toggle),
            ("Stop", Act::Stop),
            ("Prev", Act::Prev),
            ("Next", Act::Next),
            ("Repeat", Act::Repeat),
            ("Shuffle", Act::Shuffle),
        ] {
            if ui.button(label).clicked() {
                out.push(WbOut::Player(act));
            }
        }
    });
}

/// The plain listing's head: the status and what's playing.
fn head(ui: &mut Ui, save: &Save, data: &Data, t: &Transport) {
    ui.label(player::status(save, t));
    if let Some((head, at, total)) = player::now(save, data, t) {
        ui.label(format!(
            "{head}  {} / {}",
            player::clock(at),
            player::clock(total)
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::home::Home;
    use crate::home::tests::{deck, frame_keys};
    use crate::uitest::{Harness, key};
    use egui::{Pos2, vec2};
    use ninsei_core::hub::Tool;
    use ninsei_core::player::Play;

    /// An Ono-Sendai owning TRAX and three tunes.
    fn ono(data: &Data) -> Save {
        let mut s = deck(data);
        s.decks.push("onosendai".into());
        s.deck = "onosendai".into();
        s.firmware.clear();
        s.ensure_hardware(data);
        s.learn_tool("player");
        s.tunes = vec!["chroma".into(), "welcome".into(), "trap-line".into()];
        s
    }

    fn rect(w: f32, h: f32) -> Rect {
        Rect::from_min_size(Pos2::ZERO, vec2(w, h))
    }

    /// One frame of the tile, with the keyboard or without; what it sent.
    fn frame(
        h: &mut Harness,
        home: &mut Home,
        data: &Data,
        save: &Save,
        keys: bool,
        events: Vec<egui::Event>,
    ) -> Vec<Act> {
        frame_keys(
            h,
            home,
            data,
            save,
            Tool::Player,
            rect(640.0, 400.0),
            keys,
            events,
        )
        .into_iter()
        .filter_map(|o| match o {
            WbOut::Player(a) => Some(a),
            _ => None,
        })
        .collect()
    }

    /// Each row of glyphs the last frame painted, left to right, with each glyph's colour.
    fn rows_painted(h: &Harness) -> Vec<(String, Vec<Color32>)> {
        let mut glyphs: Vec<(f32, f32, String, Color32)> = h
            .painted()
            .iter()
            .filter_map(|s| match s {
                egui::Shape::Text(t) => {
                    let r = Rect::from_min_size(t.pos, t.galley.size());
                    Some((
                        r.center().y.round(),
                        r.center().x,
                        t.galley.text().to_string(),
                        t.fallback_color,
                    ))
                }
                _ => None,
            })
            .collect();
        glyphs.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        let mut out: Vec<(f32, String, Vec<Color32>)> = Vec::new();
        for (y, _, t, c) in glyphs {
            match out.last_mut() {
                Some((ly, s, cs)) if (*ly - y).abs() < 2.0 => {
                    s.push_str(&t);
                    cs.push(c);
                }
                _ => out.push((y, t, vec![c])),
            }
        }
        out.into_iter().map(|(_, s, c)| (s, c)).collect()
    }

    #[test]
    fn it_is_not_there_until_it_is_downloaded() {
        let data = Data::builtin().unwrap();
        let mut s = deck(&data);
        assert!(!s.opens(Tool::Player));
        assert!(!s.icons(&data).iter().any(|i| i.tool == Tool::Player));
        s.learn_tool("player");
        assert!(s.opens(Tool::Player));
        assert!(s.icons(&data).iter().any(|i| i.tool == Tool::Player));
    }

    #[test]
    fn its_keys_drive_the_player_only_while_it_has_the_keyboard() {
        let data = Data::builtin().unwrap();
        let save = ono(&data);
        let mut h = Harness::new(800.0, 600.0);
        let mut home = Home::new(Vec::new());
        for (k, act) in [
            (Key::Space, Act::Toggle),
            (Key::N, Act::Next),
            (Key::P, Act::Prev),
            (Key::Plus, Act::Louder),
            (Key::Minus, Act::Softer),
            (Key::R, Act::Repeat),
            (Key::S, Act::Shuffle),
        ] {
            assert_eq!(frame(&mut h, &mut home, &data, &save, true, key(k)), [act]);
            assert!(
                frame(&mut h, &mut home, &data, &save, false, key(k)).is_empty(),
                "{k:?} without the keyboard"
            );
        }
        // Its keys are its own: nothing behind it types them.
        frame(&mut h, &mut home, &data, &save, true, key(Key::Space));
        assert!(!h.ctx.egui_wants_keyboard_input());
    }

    #[test]
    fn j_k_and_enter_pick_a_tune() {
        let data = Data::builtin().unwrap();
        let save = ono(&data);
        let mut h = Harness::new(800.0, 600.0);
        let mut home = Home::new(Vec::new());
        frame(&mut h, &mut home, &data, &save, true, Vec::new());
        assert_eq!(home.player_ui.sel, Some(0), "the cursor starts at the top");
        frame(&mut h, &mut home, &data, &save, true, key(Key::J));
        frame(&mut h, &mut home, &data, &save, true, key(Key::J));
        frame(&mut h, &mut home, &data, &save, true, key(Key::J));
        assert_eq!(home.player_ui.sel, Some(2), "it stops at the bottom");
        frame(&mut h, &mut home, &data, &save, true, key(Key::K));
        assert_eq!(home.player_ui.sel, Some(1));
        let acts = frame(&mut h, &mut home, &data, &save, true, key(Key::Enter));
        assert_eq!(acts, [Act::Pick(1)]);
    }

    #[test]
    fn the_playing_row_is_lit_and_the_status_says_so() {
        let data = Data::builtin().unwrap();
        let mut save = ono(&data);
        let mut h = Harness::new(800.0, 600.0);
        let mut home = Home::new(Vec::new());
        player::apply(&mut save, &data, &mut home.player, Act::Pick(1));
        // The cursor on another row, so the lit one is the playing one.
        home.player_ui.sel = Some(0);
        frame(&mut h, &mut home, &data, &save, false, Vec::new());
        let rows = rows_painted(&h);
        let row = |s: &str| rows.iter().find(|(r, _)| r.contains(s)).cloned().unwrap();
        let (text, colours) = row("Welcome");
        assert!(text.starts_with("▶"), "{text}");
        assert!(
            colours.iter().all(|c| *c == accent()),
            "the playing row is lit"
        );
        let (_, colours) = row("trapline");
        assert!(colours.iter().all(|c| *c == ink()), "the others aren't");
        assert!(rows.iter().any(|(r, _)| r.contains("▶playing")), "{rows:?}");
        assert!(
            rows.iter()
                .any(|(r, _)| r.contains("Sense/Net-WelcometotheNet")),
            "the now line, cmus's artist - title: {rows:?}"
        );
        assert_eq!(home.player.play, Play::Playing);
    }

    #[test]
    fn it_fits_whole_at_three_sizes() {
        let data = Data::builtin().unwrap();
        let mut save = ono(&data);
        let mut h = Harness::new(1400.0, 900.0);
        let mut home = Home::new(Vec::new());
        player::apply(&mut save, &data, &mut home.player, Act::Pick(0));
        h.frame(Vec::new(), |_| {});
        let cell = crate::kit::Cells::cell_size(&h.ctx);
        let min = vec2(NEED.cols as f32 * cell.x, NEED.rows as f32 * cell.y);
        for size in [min, vec2(640.0, 400.0), vec2(1260.0, 760.0)] {
            frame_keys(
                &mut h,
                &mut home,
                &data,
                &save,
                Tool::Player,
                rect(size.x, size.y),
                true,
                Vec::new(),
            );
            assert!(home.notices.is_empty(), "drawn at {size:?}");
            assert!(h.cut_text().is_empty(), "{:?}", h.cut_text());
            let rows = rows_painted(&h);
            assert!(rows.iter().any(|(r, _)| r.contains("CHROMA")), "{size:?}");
            assert!(rows.iter().any(|(r, _)| r.contains("1:20")), "{size:?}");
        }
        // Wide enough, each row names the crew; narrow, the crew goes and the title stays.
        let rows = rows_painted(&h);
        assert!(
            rows.iter()
                .any(|(r, _)| r.contains("CHROMA") && r.contains("PHOSPHOR"))
        );
    }
}
