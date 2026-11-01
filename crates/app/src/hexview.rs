//! The hex editor: a byte grid with an ASCII column, a field inspector and the linker's verdict.
//! [`editor`] works on any [`HexBuffer`] and reports what you asked for; [`show_run`] wires it to a
//! run's HDD (the `hexedit` pane), and the hub's HexEd window wires it to the stash.

use egui::{
    Align2, Color32, Event, EventFilter, FontFamily, FontId, Id, Key, Rect, RichText, Sense,
    Stroke, Ui, pos2, vec2,
};
use ninsei_core::binary::{self, Bin, Field, LEN};
use ninsei_core::hexedit::{HexBuffer, ROW};
use ninsei_core::{Action, Command, Effect, Game, tuning};

use crate::panes::Request;
use crate::theme::Palette;

const FONT: f32 = 15.0;
const CELL_H: f32 = 22.0;

/// The grid's keyboard-focus id; the app checks it to mute game hotkeys while you type hex.
pub fn grid_id() -> Id {
    Id::new("hexedit-grid")
}

fn field_color(f: Field, p: &Palette) -> Color32 {
    match f {
        Field::Magic | Field::Reserved => p.faint.gamma_multiply(2.0),
        Field::Signature | Field::Opt => p.dim,
        Field::Name => p.fg,
        Field::Opcode | Field::Power | Field::Vs | Field::Cpu | Field::Ram => p.bright,
        Field::Checksum => p.bright,
    }
}

/// What the editor is working on.
pub struct HexCtx {
    /// File name shown in the header.
    pub file: String,
    /// Key the binary is signed with.
    pub key: u64,
    /// Fragments available for optimizing.
    pub fragments: u32,
    /// The program's current opt level.
    pub opt: u32,
    /// Why writing is off right now, if it is.
    pub locked: Option<String>,
}

/// What the editor asks for.
pub enum HexOut {
    Write(Bin),
    Optimize,
    Close,
}

/// The run's `hexedit` pane: edits a program on the HDD, compiling only at a BBS.
pub fn show_run(
    ui: &mut Ui,
    game: &Game,
    hex: &mut Option<HexBuffer>,
    p: &Palette,
    out: &mut Vec<Request>,
) {
    let Some(h) = hex.as_mut() else {
        picker(ui, game, p, out);
        return;
    };
    let Some(slot) = game.hdd.get(h.slot) else {
        *hex = None;
        return;
    };
    let cx = HexCtx {
        file: format!("{}  [slot {}]", slot.spec.file(), h.slot),
        key: game.key(),
        fragments: game.deck.fragments,
        opt: slot.spec.opt,
        locked: game.compile_blocker().map(str::to_string),
    };
    let slot_ix = h.slot;
    for o in editor(ui, h, &cx, p) {
        out.push(match o {
            HexOut::Write(bin) => Request::Cmd(Command::Game(Action::Patch { slot: slot_ix, bin })),
            HexOut::Optimize => Request::Cmd(Command::Game(Action::Optimize { slot: slot_ix })),
            HexOut::Close => Request::CloseHex,
        });
    }
}

/// The editor itself: header, grid, inspector, verdict and buttons.
pub fn editor(ui: &mut Ui, h: &mut HexBuffer, cx: &HexCtx, p: &Palette) -> Vec<HexOut> {
    let mut out = Vec::new();
    let locked = cx.locked.is_some();
    ui.horizontal(|ui| {
        ui.label(RichText::new(&cx.file).color(p.bright));
        if h.dirty() {
            ui.label(RichText::new("● modified").color(p.bright));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("close").clicked() {
                out.push(HexOut::Close);
            }
            let mode = if h.ascii { "ASCII" } else { "HEX" };
            ui.label(
                RichText::new(format!("mode {mode} [tab]  frags {}", cx.fragments)).color(p.dim),
            );
        });
    });
    ui.add_space(4.0);

    grid(ui, h, p);
    ui.add_space(6.0);
    inspector(ui, cx.key, h, p);
    ui.add_space(4.0);

    let verdict = binary::check(&h.buf, cx.key);
    match &verdict {
        Ok(spec) => {
            let st = if h.dirty() {
                "LINK OK, ready to write"
            } else {
                "LINK OK"
            };
            ui.label(
                RichText::new(format!(
                    "{st}: {}  cpu {}  ram {}",
                    spec.effect.describe(),
                    spec.cpu,
                    spec.ram
                ))
                .color(p.fg),
            );
        }
        Err(faults) => {
            for f in faults {
                ui.label(RichText::new(format!("!! {f}")).color(p.bright));
            }
        }
    }
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let can_write = h.dirty() && verdict.is_ok() && !locked;
        if ui
            .add_enabled(can_write, egui::Button::new(" write [ctrl+s] "))
            .clicked()
        {
            out.push(HexOut::Write(h.buf));
        }
        if ui
            .add_enabled(h.dirty(), egui::Button::new(" revert "))
            .clicked()
        {
            h.revert();
        }
        let can_opt = !h.dirty()
            && !locked
            && cx.fragments >= tuning::FRAGMENTS_PER_OPT
            && cx.opt < tuning::MAX_OPT;
        let opt = ui
            .add_enabled(
                can_opt,
                egui::Button::new(format!(" optimize (-{} frag) ", tuning::FRAGMENTS_PER_OPT)),
            )
            .on_disabled_hover_text("needs a fragment and an unmodified buffer");
        if opt.clicked() {
            out.push(HexOut::Optimize);
        }
    });
    if let Some(why) = &cx.locked {
        ui.label(RichText::new(format!("read-only: {why}")).color(p.dim));
    }

    if ui.memory(|m| m.has_focus(grid_id())) {
        keys(ui, h, &mut out, verdict.is_ok() && !locked);
    }
    out
}

fn grid(ui: &mut Ui, h: &mut HexBuffer, p: &Palette) {
    let font = FontId::new(FONT, FontFamily::Monospace);
    let cw = ui.ctx().fonts_mut(|f| f.glyph_width(&font, '0'));
    let off_w = cw * 8.0;
    let hex_w = cw * 3.0;
    let gap = cw;
    let ascii_x = off_w + hex_w * ROW as f32 + gap * 2.0;
    let rows = LEN / ROW;
    let size = vec2(
        ascii_x + cw * (ROW as f32 + 1.0),
        CELL_H * (rows as f32 + 1.0) + 4.0,
    );
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let focused = ui.memory(|m| m.has_focus(grid_id()));
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, p.bg);
    painter.rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0, if focused { p.fg } else { p.faint }),
        egui::StrokeKind::Inside,
    );

    let hex_x =
        |col: usize| rect.left() + off_w + hex_w * col as f32 + if col >= 8 { gap } else { 0.0 };
    let row_y = |row: usize| rect.top() + 2.0 + CELL_H * (row as f32 + 1.0);
    painter.text(
        pos2(rect.left() + cw, rect.top() + 4.0),
        Align2::LEFT_TOP,
        "OFFSET",
        font.clone(),
        p.dim,
    );
    for col in 0..ROW {
        painter.text(
            pos2(hex_x(col), rect.top() + 4.0),
            Align2::LEFT_TOP,
            format!("{col:02X}"),
            font.clone(),
            p.dim,
        );
    }
    painter.text(
        pos2(rect.left() + ascii_x, rect.top() + 4.0),
        Align2::LEFT_TOP,
        "ASCII",
        font.clone(),
        p.dim,
    );

    for i in 0..LEN {
        let (row, col) = (i / ROW, i % ROW);
        let y = row_y(row);
        if col == 0 {
            painter.text(
                pos2(rect.left() + cw, y),
                Align2::LEFT_TOP,
                format!("{:04X}", row * ROW),
                font.clone(),
                p.dim,
            );
        }
        let b = h.buf[i];
        let mut col_c = field_color(Field::at(i), p);
        if h.changed(i) {
            col_c = p.bright;
            let ux = hex_x(col);
            painter.line_segment(
                [
                    pos2(ux, y + FONT + 1.0),
                    pos2(ux + cw * 2.0, y + FONT + 1.0),
                ],
                Stroke::new(1.0, p.bright),
            );
        }
        let cell = Rect::from_min_size(
            pos2(hex_x(col) - 2.0, y - 2.0),
            vec2(cw * 2.0 + 4.0, CELL_H - 2.0),
        );
        let acell = Rect::from_min_size(
            pos2(rect.left() + ascii_x + cw * col as f32, y - 2.0),
            vec2(cw, CELL_H - 2.0),
        );
        let text = format!("{b:02X}");
        let ch = if b.is_ascii_graphic() { b as char } else { '.' };
        if i == h.cursor {
            // The active column gets the solid cursor; the other column gets an outline.
            let (solid, ghost) = if h.ascii {
                (acell, cell)
            } else {
                (cell, acell)
            };
            painter.rect_filled(solid, 0.0, if focused { p.fg } else { p.dim });
            painter.rect_stroke(
                ghost,
                0.0,
                Stroke::new(1.0, p.dim),
                egui::StrokeKind::Inside,
            );
            let ink = p.bg;
            if h.ascii {
                painter.text(
                    pos2(hex_x(col), y),
                    Align2::LEFT_TOP,
                    text,
                    font.clone(),
                    col_c,
                );
                painter.text(
                    acell.left_top() + vec2(0.0, 2.0),
                    Align2::LEFT_TOP,
                    ch,
                    font.clone(),
                    ink,
                );
            } else {
                painter.text(
                    pos2(hex_x(col), y),
                    Align2::LEFT_TOP,
                    text,
                    font.clone(),
                    ink,
                );
                if h.low_nibble {
                    let nx = hex_x(col) + cw;
                    painter.line_segment(
                        [pos2(nx, y + FONT + 2.0), pos2(nx + cw, y + FONT + 2.0)],
                        Stroke::new(2.0, ink),
                    );
                }
                painter.text(
                    acell.left_top() + vec2(0.0, 2.0),
                    Align2::LEFT_TOP,
                    ch,
                    font.clone(),
                    col_c,
                );
            }
        } else {
            painter.text(
                pos2(hex_x(col), y),
                Align2::LEFT_TOP,
                text,
                font.clone(),
                col_c,
            );
            painter.text(
                acell.left_top() + vec2(0.0, 2.0),
                Align2::LEFT_TOP,
                ch,
                font.clone(),
                col_c.gamma_multiply(0.8),
            );
        }
    }

    // Mouse: click a byte to put the cursor there and take keyboard focus.
    let pick = |pos: egui::Pos2| -> Option<(usize, bool)> {
        let row = ((pos.y - rect.top() - 2.0) / CELL_H).floor() as isize - 1;
        if !(0..rows as isize).contains(&row) {
            return None;
        }
        let row = row as usize;
        let ax = pos.x - rect.left() - ascii_x;
        if ax >= 0.0 {
            let col = (ax / cw) as usize;
            return (col < ROW).then_some((row * ROW + col, true));
        }
        (0..ROW)
            .find(|&c| pos.x >= hex_x(c) - 2.0 && pos.x < hex_x(c) + hex_w)
            .map(|c| (row * ROW + c, false))
    };
    if let Some(pos) = resp.hover_pos()
        && let Some((i, _)) = pick(pos)
    {
        let f = Field::at(i);
        resp.clone()
            .on_hover_text(format!("{:02X}: {}", i, f.label()));
    }
    if resp.clicked() {
        ui.memory_mut(|m| {
            m.request_focus(grid_id());
            m.set_focus_lock_filter(
                grid_id(),
                EventFilter {
                    tab: true,
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    escape: true,
                },
            );
        });
        if let Some(pos) = resp.interact_pointer_pos()
            && let Some((i, ascii)) = pick(pos)
        {
            h.set_cursor(i);
            h.ascii = ascii;
        }
    }
}

fn keys(ui: &mut Ui, h: &mut HexBuffer, out: &mut Vec<HexOut>, can_write: bool) {
    let events = ui.input(|i| i.events.clone());
    for e in events {
        match e {
            Event::Text(t) => {
                for ch in t.chars() {
                    h.input(ch);
                }
            }
            Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => match key {
                Key::S if modifiers.command => {
                    if h.dirty() && can_write {
                        out.push(HexOut::Write(h.buf));
                    }
                }
                Key::ArrowLeft | Key::Backspace => h.step(-1),
                Key::ArrowRight => h.step(1),
                Key::ArrowUp => h.step(-(ROW as isize)),
                Key::ArrowDown => h.step(ROW as isize),
                Key::Home => h.set_cursor(h.cursor - h.cursor % ROW),
                Key::End => h.set_cursor(h.cursor - h.cursor % ROW + ROW - 1),
                Key::Tab => {
                    h.ascii = !h.ascii;
                    h.low_nibble = false;
                }
                Key::Escape => ui.memory_mut(|m| m.surrender_focus(grid_id())),
                _ => {}
            },
            _ => {}
        }
    }
}

/// Decodes the buffer field by field, valid or not, so you can see what your bytes mean.
fn inspector(ui: &mut Ui, key: u64, h: &mut HexBuffer, p: &Palette) {
    let b = h.buf;
    let name: String = b[binary::OFF_NAME..binary::OFF_NAME + binary::NAME_LEN]
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| if c.is_ascii_graphic() { c as char } else { '?' })
        .collect();
    let sig_ok = binary::read_sig(&b) == binary::signature(key, b[binary::OFF_OPT]);
    let sum = binary::checksum(&b);
    let rows: Vec<(usize, &str, String)> = vec![
        (
            binary::OFF_OPCODE,
            "opcode",
            binary::opcode_name(b[binary::OFF_OPCODE]).to_string(),
        ),
        (
            binary::OFF_POWER,
            "power",
            format!("{}", b[binary::OFF_POWER]),
        ),
        (
            binary::OFF_VS,
            "vs class",
            binary::vs_name(b[binary::OFF_VS]).to_string(),
        ),
        (
            binary::OFF_CPU,
            "cpu cost",
            format!("{}", b[binary::OFF_CPU]),
        ),
        (
            binary::OFF_RAM,
            "ram cost",
            format!("{}", b[binary::OFF_RAM]),
        ),
        (
            binary::OFF_OPT,
            "opt level",
            format!(
                "O{}  sig {}",
                b[binary::OFF_OPT],
                if sig_ok { "ok" } else { "INVALID" }
            ),
        ),
        (binary::OFF_NAME, "name", format!("{name}.exe")),
        (
            binary::OFF_CHECKSUM,
            "checksum",
            format!(
                "{:02X}  computed {sum:02X}  {}",
                b[binary::OFF_CHECKSUM],
                if b[binary::OFF_CHECKSUM] == sum {
                    "ok"
                } else {
                    "MISMATCH"
                }
            ),
        ),
    ];
    let mut jump = None;
    egui::Grid::new("hex-inspector")
        .num_columns(3)
        .spacing([14.0, 2.0])
        .show(ui, |ui| {
            for (off, label, value) in rows {
                if ui
                    .add(
                        egui::Label::new(RichText::new(format!("{off:02X}")).color(p.dim))
                            .sense(Sense::click()),
                    )
                    .clicked()
                {
                    jump = Some(off);
                }
                ui.label(RichText::new(label).color(p.dim));
                ui.label(RichText::new(value).color(field_color(Field::at(off), p)));
                ui.end_row();
            }
        });
    if let Some(off) = jump {
        h.set_cursor(off);
        h.ascii = Field::at(off) == Field::Name;
    }

    // Budget: what the effect costs against what cpu/ram/opt allow.
    let power = u32::from(b[binary::OFF_POWER]);
    let effect = match b[binary::OFF_OPCODE] {
        1 => Some(Effect::Break { power, vs: None }),
        2 => Some(Effect::Shield(power)),
        3 => Some(Effect::Repair(power)),
        4 => Some(Effect::Scrub(power)),
        5 => Some(Effect::Stun),
        _ => None,
    };
    let budget = binary::budget(
        u32::from(b[binary::OFF_CPU]),
        u32::from(b[binary::OFF_RAM]),
        u32::from(b[binary::OFF_OPT]),
    );
    if let Some(e) = effect {
        let cost = binary::cost(e);
        let cells = 20usize;
        let scale = cost.max(budget).max(1) as usize;
        let on = (cost as usize * cells).div_ceil(scale).min(cells);
        let cap = (budget as usize * cells / scale).min(cells);
        let over = cost > budget;
        let mut bar = String::new();
        for i in 0..cells {
            bar.push(if i < on {
                '█'
            } else if i < cap {
                '░'
            } else {
                ' '
            });
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.label(RichText::new("budget ").color(p.dim));
            ui.label(RichText::new(format!("[{bar}]")).color(if over { p.bright } else { p.fg }));
            let note = if over { "  OVER" } else { "" };
            ui.label(
                RichText::new(format!(" cost {cost} / {budget}{note}")).color(if over {
                    p.bright
                } else {
                    p.fg
                }),
            );
        });
        ui.label(
            RichText::new(format!(
                "budget = {}×cpu + ram/{} + {}×opt",
                tuning::BUDGET_PER_CPU,
                tuning::BUDGET_RAM_DIV,
                tuning::BUDGET_PER_OPT
            ))
            .small()
            .color(p.dim),
        );
    }
}

/// No file open: list the warez to pick from.
fn picker(ui: &mut Ui, game: &Game, p: &Palette, out: &mut Vec<Request>) {
    ui.label(RichText::new("hexedit: no file open").color(p.bright));
    ui.label(
        RichText::new("open a program to patch its bytes. stats are whatever the binary says.")
            .color(p.dim),
    );
    ui.add_space(6.0);
    for (i, s) in game.hdd.iter().enumerate() {
        if ui.button(format!("[{i}] {}", s.spec.file())).clicked() {
            out.push(Request::OpenHex(i));
        }
    }
    ui.add_space(8.0);
    for l in [
        "00-02 magic 57 5A 01      03 opcode  01 BRK 02 SHD 03 REP 04 SCR 05 STN",
        "04 power   05 vs class   06 cpu cost   07 ram cost",
        "08 opt level, 09-0A its signature (fragments re-sign it)",
        "10-1E name   1F checksum = sum of 00-1E mod 256",
    ] {
        ui.label(RichText::new(l).small().color(p.dim));
    }
}
