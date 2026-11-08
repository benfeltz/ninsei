//! TRAX, the music player, as each desktop dresses it. On AnarchyOS it's a TUI player after
//! cmus 2.x (its playlist view and status line) with Winamp 2's transport and spectrum: three
//! boxes on the kit's cells, whole cells only, every colour the desk's. What it shows is
//! core's (`ninsei_core::player`); this only draws and turns keys and clicks into
//! [`WbOut::Player`] acts. The other desktops show a plain listing until they dress it.

use egui::{Color32, Key, Rect, Sense, Ui};
use ninsei_core::Data;
use ninsei_core::hub::Save;
use ninsei_core::player::{self, Act, Transport};

use crate::home::{WbOut, accent, font, ink, muted, shadow};
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
pub const NEED: crate::kit::fit::Fit = crate::kit::fit::Fit { cols: 40, rows: 14 };

/// Rows the spectrum box takes at least, and at most, its border included.
const VIS_H: usize = 5;
const VIS_MAX: usize = 14;
/// Rows the status box takes, its border included.
const NOW_H: usize = 4;
/// Columns a playlist row needs to show its author too; narrower, the author is dropped.
const WIDE: usize = 2 + ninsei_core::data::TUNE_TITLE_MAX + 2 + 20 + 2 + 5 + 2;

/// The gradient the spectrum climbs, darkest first: never down at the empty cells' colour, so
/// a lit cell never disappears on a mono tube.
fn ramp() -> [Color32; 3] {
    [muted(), ink(), accent()]
}

/// This frame's acts and cursor moves from the keyboard, while the player has it.
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
    // The playlist takes the rows its tunes need, the spectrum what's left (up to a point),
    // and any more goes back to the playlist.
    let spare = g.rows.saturating_sub(NOW_H);
    let list_h = (rows.len() + 2).clamp(5, spare.saturating_sub(VIS_H).max(5));
    let vis_h = spare.saturating_sub(list_h).clamp(VIS_H, VIS_MAX);
    let list_h = spare.saturating_sub(vis_h).max(5);

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
    // Where you are in a long list, in the playlist's bottom edge.
    pui.list.mark(&g, &p, 0, list_h - 1, g.cols, 2, muted());
    let wheel = ui.input(|i| i.smooth_scroll_delta.y);
    if wheel != 0.0 && ui.rect_contains_pointer(g.at(0, 0, g.cols, list_h)) {
        pui.list.wheel(-wheel / g.ch);
    }

    // The spectrum: Winamp's bars as columns of whole cells, from the speaker as the vis pane
    // hears it.
    let top = list_h;
    g.boxed(&p, 0, top, g.cols, vis_h, "spectrum", muted());
    let h = vis_h - 2;
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
    let top = top + vis_h;
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
    // Each key in the legend is a button too: a click does what the key does (the second
    // button what its pair does: prev, softer).
    for (n, (k, what)) in player::KEYS.iter().enumerate() {
        let s = format!("{k} {what}");
        let room = g.cols.saturating_sub(vol.chars().count() + 6);
        if c + s.chars().count() + 2 > room {
            break;
        }
        let used = g.note(&p, c, top + NOW_H - 1, &s, muted());
        let hit = ui.interact(
            g.at(c, top + NOW_H - 1, used, 1),
            ui.id().with(("trax-key", n)),
            Sense::click(),
        );
        let (first, second) = legend_acts(n, pui.sel);
        if hit.clicked()
            && let Some(a) = first
        {
            out.push(WbOut::Player(a));
        } else if hit.secondary_clicked()
            && let Some(a) = second
        {
            out.push(WbOut::Player(a));
        }
        c += used + 1;
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

/// The Sense/Net's TRAX: its face's size, a fixed desk accessory.
pub const LIZA_SIZE: egui::Vec2 = egui::vec2(440.0, 404.0);

/// The spectrum box's height on the desk accessory, and a bar's width and gap.
const LIZA_VIS_H: f32 = 64.0;
const LIZA_BAR: f32 = 6.0;
const LIZA_GAP: f32 = 2.0;
/// A playlist cell's height.
const LIZA_CELL_H: f32 = 20.0;

/// The Liza look in the desk's colours, with the 50% gray.
fn liza_look(data: &Data) -> crate::kit::liza::Look<'_> {
    crate::kit::liza::Look {
        ink: ink(),
        paper: shadow(),
        font: font(),
        stripes: None,
        gray: data.pattern("p50"),
    }
}

/// Draws the Sense/Net's TRAX: a System 6 desk accessory with Winamp 2's transport. The tune
/// and its crew, a position slider, ◀◀ ▶ ■ ▶▶, a volume thermometer, Repeat and Shuffle, the
/// spectrum in 1-bit bars, and the playlist in a list box. `keys`: the window is in front
/// with the keyboard.
#[allow(clippy::too_many_arguments)]
pub fn liza_face(
    ui: &mut Ui,
    data: &Data,
    save: &Save,
    t: &Transport,
    pui: &mut PlayerUi,
    heard: &Heard,
    keys_on: bool,
    out: &mut Vec<WbOut>,
) {
    use crate::kit::liza as kit;
    use egui::{Align2, Id, UiBuilder, pos2, vec2};
    pui.drawn = Some(ui.ctx().cumulative_pass_nr());
    let look = liza_look(data);
    let rows = player::rows(save, data);
    if pui.sel.is_none_or(|s| s >= rows.len()) {
        pui.sel = player::current(save, data).or((!rows.is_empty()).then_some(0));
    }
    let list_id = Id::new("trax-liza-list");
    if keys_on {
        // Its keys are the player's: nothing behind it types them, and ↑↓ move the cursor
        // here (the list box never keeps the focus that would walk it itself).
        ui.ctx().memory_mut(|m| m.stop_text_input());
        for a in keys(ui, rows.len(), &mut pui.sel) {
            out.push(WbOut::Player(a));
        }
    }
    // The face, centred across the window's room.
    let room = ui.available_width().max(LIZA_SIZE.x);
    let (row, _) = ui.allocate_exact_size(vec2(room, LIZA_SIZE.y), Sense::hover());
    let left = (row.left() + (row.width() - LIZA_SIZE.x) / 2.0).round();
    let rect = Rect::from_min_size(pos2(left, row.top().round()), LIZA_SIZE);
    let p = ui.painter_at(rect.expand(1.0));
    let (x0, w) = (rect.left(), rect.width());
    let line = 20.0;
    let mut y = rect.top();
    // Every column is measured in the face's type: an advance, and the widths of what sits
    // beside a control.
    let ctx = ui.ctx().clone();
    let measure = |s: &str| kit::text_width(&ctx, s, &look.font).ceil();
    let adv = measure("0");
    let chars = |px: f32| (px / adv).floor().max(1.0) as usize;
    let cut = crate::kit::cells::cut;

    // The tune in bold (struck twice a pixel apart, as a bitmap face bolds) and its crew.
    let tune = player::current_tune(save, data);
    let (title, by) = tune.map_or(("No tune", ""), |t| (t.title.as_str(), t.by.as_str()));
    let (title, by) = (cut(title, chars(w - 1.0)), cut(by, chars(w)));
    for dx in [0.0, 1.0] {
        p.text(
            pos2(x0 + dx, y),
            Align2::LEFT_TOP,
            &title,
            look.font.clone(),
            look.ink,
        );
    }
    p.text(
        pos2(x0, y + line),
        Align2::LEFT_TOP,
        &by,
        look.font.clone(),
        look.ink,
    );
    y += 2.0 * line + 6.0;

    // Where it is: elapsed, the slider, the length.
    let (at, total) = player::now(save, data, t).map_or((0.0, 0.0), |(_, a, b)| (a, b));
    let clock_w = measure("00:00") + 8.0;
    p.text(
        pos2(x0, y + 8.0),
        Align2::LEFT_CENTER,
        player::clock(at),
        look.font.clone(),
        look.ink,
    );
    p.text(
        pos2(x0 + w, y + 8.0),
        Align2::RIGHT_CENTER,
        player::clock(total),
        look.font.clone(),
        look.ink,
    );
    let bar = Rect::from_min_size(
        pos2(x0 + clock_w, y),
        vec2(w - 2.0 * clock_w, kit::size::SCROLL_W),
    );
    let place = if total > 0.0 { at / total } else { 0.0 };
    if let Some(v) = kit::hslider(ui, bar, Id::new("trax-liza-seek"), place, &look)
        && total > 0.0
    {
        out.push(WbOut::Player(Act::Seek((v * total * 1000.0) as u32)));
    }
    y += kit::size::SCROLL_W + 10.0;

    // The transport: Winamp's four buttons, ▶ reading ‖ while it plays.
    let play = if t.play == player::Play::Playing {
        "‖"
    } else {
        "▶"
    };
    let row_h = kit::size::BUTTON_H + 2.0 * kit::size::RING_OUT;
    let mut buttons = ui.new_child(
        UiBuilder::new()
            .max_rect(Rect::from_min_size(pos2(x0, y), vec2(w, row_h)))
            .layout(egui::Layout::left_to_right(egui::Align::Min)),
    );
    buttons.spacing_mut().item_spacing.x = 4.0;
    for (label, act) in [
        ("◀◀", Act::Prev),
        (play, Act::Toggle),
        ("■", Act::Stop),
        ("▶▶", Act::Next),
    ] {
        if kit::push_button(&mut buttons, label, false, &look).clicked() {
            out.push(WbOut::Player(act));
        }
    }
    y += row_h + 6.0;

    // The level as a thermometer, then the modes as check boxes.
    p.text(
        pos2(x0, y + 8.0),
        Align2::LEFT_CENTER,
        player::level_label(save),
        look.font.clone(),
        look.ink,
    );
    let label_w = measure("Volume").max(measure("Muted"));
    let therm = Rect::from_min_size(pos2(x0 + label_w + 10.0, y + 2.0), vec2(120.0, 12.0));
    let level = player::level(save);
    if let Some(v) = kit::thermometer(ui, therm, Id::new("trax-liza-volume"), level, &look) {
        out.push(WbOut::Player(Act::Volume((v * 100.0).round() as u8)));
    }
    let mut modes = ui.new_child(
        UiBuilder::new()
            .max_rect(Rect::from_min_max(
                pos2(therm.right() + 20.0, y),
                pos2(x0 + w, y + line),
            ))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    modes.spacing_mut().item_spacing.x = 14.0;
    let (mut repeat, mut shuffle) = (save.player.repeat, save.player.shuffle);
    if kit::check_box(&mut modes, &mut repeat, "Repeat", &look).changed() {
        out.push(WbOut::Player(Act::Repeat));
    }
    if kit::check_box(&mut modes, &mut shuffle, "Shuffle", &look).changed() {
        out.push(WbOut::Player(Act::Shuffle));
    }
    y += line + 10.0;

    // The spectrum: 1-bit bars, ink on paper, in a sunken box (a frame with a second rule
    // inside its top and left edges).
    let vis = Rect::from_min_size(pos2(x0, y), vec2(w, LIZA_VIS_H));
    p.rect_filled(vis, 0.0, look.paper);
    p.rect_stroke(
        vis,
        0.0,
        egui::Stroke::new(1.0, look.ink),
        egui::StrokeKind::Inside,
    );
    let inner = vis.shrink(4.0);
    p.line_segment(
        [
            vis.left_top() + vec2(1.5, 1.5),
            vis.right_top() + vec2(-1.5, 1.5),
        ],
        egui::Stroke::new(1.0, look.ink),
    );
    p.line_segment(
        [
            vis.left_top() + vec2(1.5, 1.5),
            vis.left_bottom() + vec2(1.5, -1.5),
        ],
        egui::Stroke::new(1.0, look.ink),
    );
    let n = ((inner.width() + LIZA_GAP) / (LIZA_BAR + LIZA_GAP))
        .floor()
        .max(1.0) as usize;
    let samples: &[f32] = if heard.samples.is_empty() {
        &[0.0; 64]
    } else {
        &heard.samples
    };
    let bars = crate::vispane::bars(ui, "trax-liza", samples, heard.rate.max(1), n);
    let left = inner.left()
        + ((inner.width() - (n as f32 * (LIZA_BAR + LIZA_GAP) - LIZA_GAP)) / 2.0).round();
    for (i, b) in bars.iter().enumerate() {
        let h = (b.clamp(0.0, 1.0) * inner.height()).round();
        if h >= 1.0 {
            let x = left + i as f32 * (LIZA_BAR + LIZA_GAP);
            p.rect_filled(
                Rect::from_min_max(
                    pos2(x, inner.bottom() - h),
                    pos2(x + LIZA_BAR, inner.bottom()),
                ),
                0.0,
                look.ink,
            );
        }
    }
    y += LIZA_VIS_H + 8.0;

    // The playlist: a list box, the cursor inverted, the playing tune marked; a double-click
    // (or Enter) plays.
    let mut list =
        ui.new_child(UiBuilder::new().max_rect(Rect::from_min_max(pos2(x0, y), rect.max)));
    let sel = pui.sel.unwrap_or(0);
    let font = look.font.clone();
    // The columns: the mark, the title (the longest a title may be), the crew in what's left,
    // and the length at the right.
    let title_x = 4.0 + 2.0 * adv;
    let by_x = title_x + adv * (ninsei_core::data::TUNE_TITLE_MAX + 2) as f32;
    let len_w = measure("00:00");
    let picked = kit::list_box(
        &mut list,
        vec2(w, rect.bottom() - y),
        list_id,
        rows.len(),
        LIZA_CELL_H,
        0.0,
        sel,
        &look,
        |p, cell, i, on| {
            let row = &rows[i];
            let (fg, bg) = if on {
                (look.paper, look.ink)
            } else {
                (look.ink, look.paper)
            };
            p.rect_filled(cell, 0.0, bg);
            let mid = cell.center().y;
            let mark = if row.current { "▶" } else { "" };
            p.text(
                pos2(cell.left() + 4.0, mid),
                Align2::LEFT_CENTER,
                mark,
                font.clone(),
                fg,
            );
            p.text(
                pos2(cell.left() + title_x, mid),
                Align2::LEFT_CENTER,
                cut(row.title, ninsei_core::data::TUNE_TITLE_MAX),
                font.clone(),
                fg,
            );
            let by_room = cell.width() - 6.0 - len_w - adv - by_x;
            p.text(
                pos2(cell.left() + by_x, mid),
                Align2::LEFT_CENTER,
                cut(row.by, chars(by_room)),
                font.clone(),
                fg,
            );
            p.text(
                pos2(cell.right() - 6.0, mid),
                Align2::RIGHT_CENTER,
                &row.length,
                font.clone(),
                fg,
            );
            row.title.to_string()
        },
    );
    if let Some(i) = picked {
        pui.sel = Some(i);
        if ui.input(|i| {
            i.pointer
                .button_double_clicked(egui::PointerButton::Primary)
        }) {
            out.push(WbOut::Player(Act::Pick(i)));
        }
    }
}

/// What a click on legend entry `n` ([`player::KEYS`]) does, and a second-button click.
fn legend_acts(n: usize, sel: Option<usize>) -> (Option<Act>, Option<Act>) {
    match n {
        0 => (Some(Act::Toggle), Some(Act::Stop)),
        1 => (Some(Act::Next), Some(Act::Prev)),
        2 => (Some(Act::Louder), Some(Act::Softer)),
        3 => (Some(Act::Repeat), None),
        4 => (Some(Act::Shuffle), None),
        _ => (sel.map(Act::Pick), None),
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

    #[test]
    fn the_legend_is_clickable() {
        let data = Data::builtin().unwrap();
        let save = ono(&data);
        let mut h = Harness::new(800.0, 600.0);
        let mut home = Home::new(Vec::new());
        frame(&mut h, &mut home, &data, &save, false, Vec::new());
        // The legend's first glyph: the bottom row's leftmost `s`.
        let ss: Vec<egui::Pos2> = h
            .painted_text()
            .into_iter()
            .filter(|(t, _)| t == "s")
            .map(|(_, r)| r.center())
            .collect();
        let bottom = ss.iter().map(|p| p.y).fold(0.0, f32::max);
        let spot = ss
            .into_iter()
            .filter(|p| (p.y - bottom).abs() < 1.0)
            .min_by(|a, b| a.x.total_cmp(&b.x))
            .unwrap();
        let acts = frame(
            &mut h,
            &mut home,
            &data,
            &save,
            false,
            crate::uitest::click(spot),
        );
        assert_eq!(acts, [Act::Toggle], "spc play: the first legend key");
    }

    #[test]
    fn a_long_playlist_says_where_you_are_under_the_list() {
        let data = Data::builtin().unwrap();
        let mut save = ono(&data);
        save.tunes = data.tunes.iter().map(|t| t.id.clone()).collect();
        let mut h = Harness::new(1400.0, 900.0);
        let mut home = Home::new(Vec::new());
        h.frame(Vec::new(), |_| {});
        let cell = crate::kit::Cells::cell_size(&h.ctx);
        let size = vec2(NEED.cols as f32 * cell.x, NEED.rows as f32 * cell.y);
        frame_keys(
            &mut h,
            &mut home,
            &data,
            &save,
            Tool::Player,
            rect(size.x, size.y),
            true,
            key(Key::J),
        );
        let rows = rows_painted(&h);
        let title = rows
            .iter()
            .position(|(r, _)| r.contains("playlist"))
            .unwrap();
        let mark = rows
            .iter()
            .position(|(r, _)| r.contains(&format!("2/{}", data.tunes.len())))
            .unwrap_or_else(|| panic!("{rows:?}"));
        assert_eq!(mark, title + 4, "on the bottom edge, under three rows");
    }

    mod liza {
        use super::*;
        use crate::uitest::click;

        /// A Sense/Net owning TRAX and three tunes.
        fn sensenet(data: &Data) -> Save {
            let mut s = deck(data);
            s.learn_tool("player");
            s.tunes = vec!["chroma".into(), "welcome".into(), "trap-line".into()];
            s
        }

        /// One frame of the desk accessory, with the keyboard or without; what it sent.
        fn da(
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
                rect(600.0, 600.0),
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

        /// Clicks the widget labelled `label` (two frames, as a click takes); what it sent.
        fn press(
            h: &mut Harness,
            home: &mut Home,
            data: &Data,
            save: &Save,
            label: &str,
        ) -> Vec<Act> {
            let at = h
                .find(label)
                .unwrap_or_else(|| panic!("no {label:?}: {:?}", h.texts()))
                .center();
            let mut acts = da(h, home, data, save, false, click(at));
            acts.extend(da(h, home, data, save, false, Vec::new()));
            acts
        }

        #[test]
        fn its_buttons_and_boxes_drive_the_player() {
            let data = Data::builtin().unwrap();
            let save = sensenet(&data);
            let mut h = Harness::new(800.0, 700.0);
            let mut home = Home::new(Vec::new());
            da(&mut h, &mut home, &data, &save, false, Vec::new());
            for (label, act) in [
                ("◀◀", Act::Prev),
                ("▶", Act::Toggle),
                ("■", Act::Stop),
                ("▶▶", Act::Next),
                ("Repeat", Act::Repeat),
                ("Shuffle", Act::Shuffle),
            ] {
                assert_eq!(
                    press(&mut h, &mut home, &data, &save, label),
                    [act],
                    "{label}"
                );
            }
        }

        #[test]
        fn play_reads_pause_while_it_plays() {
            let data = Data::builtin().unwrap();
            let mut save = sensenet(&data);
            let mut h = Harness::new(800.0, 700.0);
            let mut home = Home::new(Vec::new());
            player::apply(&mut save, &data, &mut home.player, Act::Pick(0));
            da(&mut h, &mut home, &data, &save, false, Vec::new());
            assert!(h.find("‖").is_some(), "{:?}", h.texts());
            assert_eq!(press(&mut h, &mut home, &data, &save, "‖"), [Act::Toggle]);
        }

        #[test]
        fn the_slider_seeks_and_the_thermometer_sets_the_level() {
            let data = Data::builtin().unwrap();
            let mut save = sensenet(&data);
            let mut h = Harness::new(800.0, 700.0);
            let mut home = Home::new(Vec::new());
            player::apply(&mut save, &data, &mut home.player, Act::Pick(0));
            da(&mut h, &mut home, &data, &save, false, Vec::new());
            let total = data.tune("chroma").unwrap().secs;
            let rects = h.rects();
            // The slider is the one rect 16 tall framed in ink that spans most of the face.
            let bar = rects
                .iter()
                .map(|(r, _, _)| *r)
                .find(|r| r.height() == 16.0 && r.width() > 300.0)
                .expect("the slider");
            let acts = da(&mut h, &mut home, &data, &save, false, click(bar.center()));
            match acts.as_slice() {
                [Act::Seek(ms)] => {
                    let half = total * 500;
                    assert!(ms.abs_diff(half) < total * 20, "{ms} vs {half}");
                }
                other => panic!("{other:?}"),
            }
            // The thermometer, 120 wide (116 inside), beside "Volume": a click three
            // quarters along its inside.
            let therm = rects
                .iter()
                .map(|(r, _, _)| *r)
                .find(|r| r.width() == 120.0 && r.height() == 12.0)
                .expect("the thermometer");
            let at = egui::pos2(therm.left() + 2.0 + 87.0, therm.center().y);
            let acts = da(&mut h, &mut home, &data, &save, false, click(at));
            assert_eq!(acts, [Act::Volume(75)]);
        }

        #[test]
        fn with_sound_off_the_level_still_shows_and_says_muted() {
            let data = Data::builtin().unwrap();
            let mut save = sensenet(&data);
            save.sound = false;
            save.volume = 60;
            let mut h = Harness::new(800.0, 700.0);
            let mut home = Home::new(Vec::new());
            da(&mut h, &mut home, &data, &save, false, Vec::new());
            assert!(
                h.painted_text().iter().any(|(t, _)| t == "Muted"),
                "{:?}",
                h.painted_text()
            );
            assert!(!h.painted_text().iter().any(|(t, _)| t == "Volume"));
            // The fill: the 50% gray over three fifths of the inside, not an empty bar.
            let therm = h
                .rects()
                .into_iter()
                .map(|(r, _, _)| r)
                .find(|r| r.width() == 120.0 && r.height() == 12.0)
                .unwrap();
            // Something painted across more than half the inside: the level, not an empty bar.
            let lit = h
                .painted()
                .iter()
                .map(|sh| sh.visual_bounding_rect())
                .filter(|r| therm.expand(0.5).contains_rect(*r) && r.width() < therm.width() - 3.0)
                .any(|r| r.right() > therm.left() + 60.0);
            assert!(lit, "the level is drawn");
        }

        #[test]
        fn the_longest_title_never_runs_into_its_crew() {
            let mut data = Data::builtin().unwrap();
            let longest = "W".repeat(ninsei_core::data::TUNE_TITLE_MAX);
            data.tunes[0].title = longest.clone();
            let mut save = sensenet(&data);
            save.tunes.insert(0, data.tunes[0].id.clone());
            let by = data.tunes[0].by.clone();
            let mut h = Harness::new(800.0, 700.0);
            let mut home = Home::new(Vec::new());
            da(&mut h, &mut home, &data, &save, false, Vec::new());
            let texts = h.painted_text();
            let at = |t: &str| {
                texts
                    .iter()
                    .filter(|(s, _)| s.starts_with(t))
                    .map(|(_, r)| *r)
                    .max_by(|a, b| a.top().total_cmp(&b.top()))
                    .unwrap_or_else(|| panic!("no {t:?}"))
            };
            // The crew may be cut short with …: found by how it starts.
            let (title, crew) = (at(&longest), at(&by[..6]));
            assert!((title.center().y - crew.center().y).abs() < 1.0, "one row");
            assert!(title.right() < crew.left(), "{title:?} {crew:?}");
        }

        #[test]
        fn its_keys_work_only_with_the_keyboard() {
            let data = Data::builtin().unwrap();
            let save = sensenet(&data);
            let mut h = Harness::new(800.0, 700.0);
            let mut home = Home::new(Vec::new());
            assert!(da(&mut h, &mut home, &data, &save, false, key(Key::Space)).is_empty());
            assert_eq!(
                da(&mut h, &mut home, &data, &save, true, key(Key::Space)),
                [Act::Toggle]
            );
            assert!(!h.ctx.egui_wants_keyboard_input());
            da(&mut h, &mut home, &data, &save, true, key(Key::ArrowDown));
            assert_eq!(home.player_ui.sel, Some(1));
            assert_eq!(
                da(&mut h, &mut home, &data, &save, true, key(Key::Enter)),
                [Act::Pick(1)]
            );
        }

        #[test]
        fn it_shows_whole_in_its_window_and_desktop_keeps_the_plain_listing() {
            let data = Data::builtin().unwrap();
            let save = sensenet(&data);
            let mut h = Harness::new(800.0, 700.0);
            let mut home = Home::new(Vec::new());
            da(&mut h, &mut home, &data, &save, false, Vec::new());
            assert!(h.cut_text().is_empty(), "{:?}", h.cut_text());
            assert!(h.find("Play/Pause").is_none(), "not the plain listing");
            // On Desktop (the Ono-Sendai's classic firmware), still the plain listing.
            let mut wb = save.clone();
            wb.decks.push("onosendai".into());
            wb.deck = "onosendai".into();
            wb.firmware = "6.2".into();
            assert_eq!(
                wb.firmware_def(&data).desktop,
                ninsei_core::hardware::Desktop::Workbench
            );
            da(&mut h, &mut home, &data, &wb, false, Vec::new());
            assert!(h.find("Play/Pause").is_some());
        }
    }
}
