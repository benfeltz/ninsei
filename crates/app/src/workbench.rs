//! The hub desktop: a Workbench 1.3-style screen with pixel-art icons, draggable windows,
//! a Shell, the RIG / STASH / HexEd tools (see `hubtools`), and "insert volume" requesters
//! for tools that aren't installed yet.
//! All drawn by hand into one area so the whole screen can be dragged off a run.

use egui::{
    Align2, Color32, FontFamily, FontId, Id, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2, pos2,
    vec2,
};
use ninsei_core::Data;
use ninsei_core::data::ThemeDef;
use std::cell::Cell;

use crate::theme::Palette;
use ninsei_core::binary::Bin;
use ninsei_core::hexedit::HexBuffer;
use ninsei_core::hub::{self, IconDef, Save, ScanResult, ShellEffect, Tool, Upgrade};
use ninsei_core::modem::Handshake;

use crate::hubtools;

/// The desktop's colours. Like the Amiga's palette registers: set once a frame from the desktop
/// theme ([`set_desk`]) and read by everything that draws the hub.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Desk {
    /// The screen behind the windows.
    pub screen: Color32,
    /// Window frames, title bars, text.
    pub ink: Color32,
    /// Darkest tone: shadows, gadget outlines.
    pub shadow: Color32,
    /// Selections and highlights.
    pub accent: Color32,
    /// Secondary text.
    pub muted: Color32,
    /// Fill inside icons and unlit cells.
    pub fill: Color32,
}

impl Desk {
    /// Workbench 1.3, used until the first theme is set.
    pub const WORKBENCH: Desk = Desk {
        screen: Color32::from_rgb(0x00, 0x55, 0xAA),
        ink: Color32::from_rgb(0xFF, 0xFF, 0xFF),
        shadow: Color32::from_rgb(0x00, 0x00, 0x22),
        accent: Color32::from_rgb(0xFF, 0x88, 0x00),
        muted: Color32::from_rgb(0xAA, 0xCC, 0xFF),
        fill: Color32::from_rgb(0x00, 0x55, 0xAA),
    };

    /// A theme's six colours, mapped onto the desktop's roles.
    pub fn from_def(t: &ThemeDef) -> Desk {
        let p = Palette::from_def(t);
        Desk {
            screen: p.panel,
            ink: p.fg,
            shadow: p.bg,
            accent: p.bright,
            muted: p.dim,
            fill: p.faint,
        }
    }
}

thread_local! {
    static DESK: Cell<Desk> = const { Cell::new(Desk::WORKBENCH) };
}

/// Loads the desktop's palette registers.
pub fn set_desk(d: Desk) {
    DESK.with(|c| c.set(d));
}

fn desk() -> Desk {
    DESK.with(Cell::get)
}

pub fn backdrop() -> Color32 {
    desk().screen
}

pub fn ink() -> Color32 {
    desk().ink
}

pub fn shadow() -> Color32 {
    desk().shadow
}

pub fn accent() -> Color32 {
    desk().accent
}

pub fn muted() -> Color32 {
    desk().muted
}

pub fn fill() -> Color32 {
    desk().fill
}

const FONT: f32 = 14.0;
const TITLE_H: f32 = 20.0;
const SCREEN_BAR_H: f32 = 22.0;
/// Screen pixels per icon pixel.
const ICON_SCALE: f32 = 3.0;
const ICON_PITCH: f32 = 84.0;
const BORDER: f32 = 2.0;
/// Seconds between clicks that still count as a double-click.
const DOUBLE_CLICK_SECS: f64 = 0.45;
const SHELL_KEEP: usize = 200;

fn font() -> FontId {
    FontId::new(FONT, FontFamily::Monospace)
}

/// What the desktop asks the app to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WbOut {
    /// Dial a phonebook entry (and jack in), with an optional seed.
    Dial(u32, Option<u64>),
    /// Wardial a range (by id).
    Scan(String),
    /// Call a BBS from home (board id): it opens over the desktop.
    Call(String),
    Theme(String),
    /// Set the CRT display preset (by id).
    Display(String),
    Crt,
    /// Modem sounds on/off.
    Sound,
    Buy(Upgrade),
    ToggleStarter(String),
    ToggleStash(u32),
    Scrap(u32),
    Patch(u32, Bin),
    Optimize(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Shell,
    /// CarrierTerm: the phonebook.
    Terminal,
    Prefs,
    Rig,
    Stash,
    HexEd,
    /// "Please insert volume X in any drive".
    Requester(Tool),
}

struct Win {
    kind: Kind,
    /// Top-left, relative to the screen.
    pos: Pos2,
    size: Vec2,
}

impl Win {
    fn title(&self) -> String {
        match self.kind {
            Kind::Shell => "NinShell".into(),
            Kind::Prefs => "Prefs".into(),
            Kind::Terminal => "CarrierTerm 1.0: Dialing Directory".into(),
            Kind::Rig => "RIG: deck shop".into(),
            Kind::Stash => "STASH".into(),
            Kind::HexEd => "HexEd".into(),
            Kind::Requester(_) => "System Request".into(),
        }
    }
}

pub struct Workbench {
    /// Back to front: the last window is on top.
    wins: Vec<Win>,
    selected: Option<Tool>,
    last_click: Option<(Tool, f64)>,
    shell_lines: Vec<String>,
    shell_input: String,
    focus_shell: bool,
    /// The stash program open in HexEd: its id and the working buffer.
    pub(crate) hex: Option<(u32, HexBuffer)>,
    /// Phonebook entry highlighted in CarrierTerm.
    pub(crate) term_sel: Option<u32>,
    pub(crate) term_tab: TermTab,
    /// Range highlighted in WarDial.
    pub(crate) range_sel: Option<String>,
    /// The last scan and how long it's been filling in.
    pub(crate) scan: Option<(ScanResult, f32)>,
    /// A dial in progress: the modem handshake before jacking in.
    pub(crate) dialing: Option<Dialing>,
    /// A dial that just started (phonebook entry id), for the app to play its handshake.
    pub(crate) dial_started: Option<u32>,
}

/// CarrierTerm's two tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TermTab {
    Phonebook,
    WarDial,
}

/// The modem handshake that plays before a dial jacks you in.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Dialing {
    pub entry: u32,
    pub seed: Option<u64>,
    pub t: f32,
}

impl Workbench {
    pub fn new(boot: Vec<String>) -> Self {
        let mut wb = Self {
            wins: Vec::new(),
            selected: None,
            last_click: None,
            shell_lines: boot,
            shell_input: String::new(),
            focus_shell: true,
            hex: None,
            term_sel: None,
            term_tab: TermTab::Phonebook,
            range_sel: None,
            scan: None,
            dialing: None,
            dial_started: None,
        };
        wb.open(Tool::Shell);
        wb
    }

    /// Gives the Shell the keyboard next time it's drawn.
    pub fn focus(&mut self) {
        self.focus_shell = true;
    }

    pub fn say(&mut self, line: impl Into<String>) {
        self.shell_lines.push(line.into());
        if self.shell_lines.len() > SHELL_KEEP {
            let extra = self.shell_lines.len() - SHELL_KEEP;
            self.shell_lines.drain(..extra);
        }
    }

    /// Opens (or raises) a tool's window. Tools not installed yet get a requester.
    fn open(&mut self, tool: Tool) {
        let kind = match tool {
            t if !t.installed() => Kind::Requester(t),
            Tool::Terminal => Kind::Terminal,
            Tool::Prefs => Kind::Prefs,
            Tool::Shell => Kind::Shell,
            Tool::Rig => Kind::Rig,
            Tool::Stash => Kind::Stash,
            Tool::HexEd => Kind::HexEd,
        };
        if let Some(i) = self.wins.iter().position(|w| w.kind == kind) {
            let w = self.wins.remove(i);
            self.wins.push(w);
        } else {
            let (pos, size) = match kind {
                Kind::Shell => (pos2(150.0, 300.0), vec2(600.0, 280.0)),
                Kind::Terminal => (pos2(170.0, 40.0), vec2(720.0, 440.0)),
                Kind::Prefs => (pos2(260.0, 40.0), vec2(560.0, 600.0)),
                Kind::Rig => (pos2(170.0, 50.0), vec2(620.0, 330.0)),
                Kind::Stash => (pos2(210.0, 80.0), vec2(640.0, 380.0)),
                Kind::HexEd => (pos2(130.0, 36.0), vec2(800.0, 500.0)),
                Kind::Requester(_) => (pos2(260.0, 150.0), vec2(330.0, 150.0)),
            };
            self.wins.push(Win { kind, pos, size });
        }
        if kind == Kind::Shell {
            self.focus_shell = true;
        }
    }

    /// Draws the whole screen into `ui`, dragged down by `offset` pixels.
    /// Only takes input when `live`.
    /// Opens a stash program in HexEd.
    pub fn edit(&mut self, id: u32, save: &Save) {
        if let Some(bin) = save.item(id).and_then(|i| i.bin()) {
            self.hex = Some((id, HexBuffer::open(0, bin)));
            self.open(Tool::HexEd);
        }
    }

    /// Keeps the HexEd buffer in step with the stash: a successful write or an optimize
    /// becomes the baseline, and a scrapped program closes the editor.
    fn sync_hex(&mut self, save: &Save) {
        let Some((id, h)) = &mut self.hex else { return };
        match save.item(*id).and_then(|i| i.bin()) {
            None => self.hex = None,
            Some(bin) if h.buf == bin || !h.dirty() => h.committed(bin),
            Some(_) => {}
        }
    }

    /// Draws the whole screen into `ui`, dragged down by `offset` pixels.
    /// Only takes input when `live`.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        data: &Data,
        save: &Save,
        offset: f32,
        live: bool,
    ) -> Vec<WbOut> {
        self.sync_hex(save);
        let icons = &data.icons;
        let mut out = Vec::new();
        // The handshake runs on its own clock, then hands over to the jack-in drag.
        if live && let Some(d) = &mut self.dialing {
            d.t += ui.input(|i| i.stable_dt).min(0.1);
            let secs = save
                .entry(d.entry)
                .map_or(0.0, |e| Handshake::new(&e.number).end);
            if d.t >= secs {
                out.push(WbOut::Dial(d.entry, d.seed));
                self.dialing = None;
            }
            ui.ctx().request_repaint();
        }
        if let Some((_, t)) = &mut self.scan
            && *t < hubtools::SCAN_FILL_SECS
        {
            *t += ui.input(|i| i.stable_dt).min(0.1);
            ui.ctx().request_repaint();
        }
        let screen = ui.ctx().content_rect().translate(vec2(0.0, offset));
        let painter = ui.painter_at(screen);
        let at = |p: Pos2| screen.min + p.to_vec2();

        // Screen and its title bar.
        painter.rect_filled(screen, 0.0, backdrop());
        let bar = Rect::from_min_size(screen.min, vec2(screen.width(), SCREEN_BAR_H));
        painter.rect_filled(bar, 0.0, ink());
        painter.line_segment(
            [bar.left_bottom(), bar.right_bottom()],
            Stroke::new(1.0, shadow()),
        );
        let title = format!(
            "ninsei Workbench 1.3   runs {}  exits {}  flatlines {}   best {}¢",
            save.runs, save.exits, save.flatlines, save.best_haul
        );
        painter.text(
            bar.left_center() + vec2(8.0, 0.0),
            Align2::LEFT_CENTER,
            title,
            font(),
            backdrop(),
        );
        depth_gadget(
            &painter,
            Rect::from_min_size(bar.right_top() + vec2(-26.0, 3.0), vec2(22.0, 16.0)),
        );

        // Background click deselects icons.
        let bg = ui.interact(
            screen,
            Id::new("wb-bg"),
            if live { Sense::click() } else { Sense::hover() },
        );
        if bg.clicked() {
            self.selected = None;
        }

        // Icons down the left, like disks on a desk.
        let now = ui.input(|i| i.time);
        for (n, icon) in icons.iter().enumerate() {
            let w = icon.width() as f32 * ICON_SCALE;
            let h = icon.pixels.len() as f32 * ICON_SCALE;
            let cell = Rect::from_min_size(
                at(pos2(18.0, SCREEN_BAR_H + 16.0 + n as f32 * ICON_PITCH)),
                vec2(84.0, ICON_PITCH - 6.0),
            );
            let art = Rect::from_min_size(pos2(cell.center().x - w / 2.0, cell.top()), vec2(w, h));
            let selected = self.selected == Some(icon.tool);
            draw_icon(&painter, icon, art.min, selected);
            let label = Rect::from_center_size(
                pos2(cell.center().x, art.bottom() + 10.0),
                vec2(cell.width(), 16.0),
            );
            if selected {
                // Highlight exactly as wide as the name, however long it is.
                let w = painter
                    .layout_no_wrap(icon.label.clone(), font(), backdrop())
                    .size()
                    .x;
                painter.rect_filled(
                    Rect::from_center_size(label.center(), vec2(w + 8.0, label.height())),
                    0.0,
                    ink(),
                );
            }
            painter.text(
                label.center(),
                Align2::CENTER_CENTER,
                &icon.label,
                font(),
                if selected { backdrop() } else { ink() },
            );
            let hit = art.union(label);
            let r = ui.interact(
                hit,
                Id::new(("wb-icon", n)),
                if live { Sense::click() } else { Sense::hover() },
            );
            if r.clicked() {
                // Our own double-click: two clicks on the same icon in quick succession.
                let double = matches!(self.last_click, Some((t, at)) if t == icon.tool && now - at < DOUBLE_CLICK_SECS);
                self.selected = Some(icon.tool);
                self.last_click = Some((icon.tool, now));
                if double {
                    self.last_click = None;
                    self.activate(icon.tool);
                }
            }
        }

        // Windows, back to front.
        let mut raise = None;
        let mut close = None;
        let n = self.wins.len();
        let mut pending_edit = None;
        let mut pending_dial = None;
        for i in 0..n {
            let top = i + 1 == n;
            let (kind, pos, size) = (self.wins[i].kind, self.wins[i].pos, self.wins[i].size);
            let rect = Rect::from_min_size(at(pos), size);
            let title = self.wins[i].title();
            let wid = Id::new(("wb-win", format!("{kind:?}")));
            // The whole window swallows clicks so icons under it don't fire.
            let body = ui.interact(
                rect,
                wid.with("body"),
                if live { Sense::click() } else { Sense::hover() },
            );
            if body.clicked() || body.drag_started() {
                raise = Some(i);
            }
            window_frame(&painter, rect, &title, top);
            let bar = Rect::from_min_size(rect.min, vec2(rect.width(), TITLE_H));
            let close_r = Rect::from_min_size(bar.min + vec2(4.0, 3.0), vec2(16.0, 14.0));
            let depth_r = Rect::from_min_size(bar.right_top() + vec2(-26.0, 2.0), vec2(22.0, 16.0));
            let sense = if live {
                Sense::click_and_drag()
            } else {
                Sense::hover()
            };
            let drag = ui.interact(bar, wid.with("bar"), sense);
            if drag.dragged() {
                let d = drag.drag_delta();
                let p = &mut self.wins[i].pos;
                *p += d;
                p.x = p.x.clamp(-size.x + 60.0, screen.width() - 60.0);
                p.y = p.y.clamp(SCREEN_BAR_H, screen.height() - TITLE_H);
            }
            if drag.drag_started() || drag.clicked() {
                raise = Some(i);
            }
            if ui
                .interact(
                    close_r,
                    wid.with("close"),
                    if live { Sense::click() } else { Sense::hover() },
                )
                .clicked()
            {
                close = Some(i);
            }
            if ui
                .interact(
                    depth_r,
                    wid.with("depth"),
                    if live { Sense::click() } else { Sense::hover() },
                )
                .clicked()
            {
                // Depth gadget: top window goes to the back, anything else comes to the front.
                raise = if top { None } else { Some(i) };
                if top {
                    let w = self.wins.remove(i);
                    self.wins.insert(0, w);
                    break;
                }
            }
            let content = Rect::from_min_max(
                bar.left_bottom() + vec2(BORDER + 4.0, 4.0),
                rect.max - vec2(BORDER + 4.0, BORDER + 4.0),
            );
            match kind {
                Kind::Shell => self.shell(ui, content, data, save, live && top, &mut out),
                Kind::Rig | Kind::Stash | Kind::HexEd | Kind::Terminal | Kind::Prefs => {
                    let mut child = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(content)
                            .layout(egui::Layout::top_down(egui::Align::Min)),
                    );
                    child.set_clip_rect(content);
                    hubtools::wb_visuals(child.visuals_mut());
                    if !live {
                        child.disable();
                    }
                    let mut edit = None;
                    let mut dial = None;
                    match kind {
                        Kind::Terminal => {
                            if self.dialing.is_none() {
                                child.horizontal(|ui| {
                                    for (tab, label) in [
                                        (TermTab::Phonebook, " Phonebook "),
                                        (TermTab::WarDial, " WarDial "),
                                    ] {
                                        if ui
                                            .selectable_label(self.term_tab == tab, label)
                                            .clicked()
                                        {
                                            self.term_tab = tab;
                                        }
                                    }
                                });
                                child.separator();
                            }
                            if self.term_tab == TermTab::WarDial && self.dialing.is_none() {
                                hubtools::wardial(
                                    &mut child,
                                    data,
                                    save,
                                    &mut self.range_sel,
                                    self.scan.as_ref(),
                                    &mut out,
                                );
                            } else {
                                hubtools::carrierterm(
                                    &mut child,
                                    data,
                                    save,
                                    &mut self.term_sel,
                                    self.dialing.as_ref(),
                                    &mut dial,
                                    &mut out,
                                );
                            }
                        }
                        Kind::Rig => hubtools::rig(&mut child, data, save, &mut out),
                        Kind::Prefs => hubtools::prefs(&mut child, data, save, &mut out),
                        Kind::Stash => hubtools::stash(&mut child, data, save, &mut out, &mut edit),
                        _ => hubtools::hexed(&mut child, save, &mut self.hex, &mut out, &mut edit),
                    }
                    if edit.is_some() {
                        pending_edit = edit;
                    }
                    if dial.is_some() {
                        pending_dial = dial;
                    }
                }
                Kind::Requester(tool) => {
                    if let Some(action) = requester(ui, &painter, content, tool, live, wid) {
                        match action {
                            Req::Retry => {
                                self.say(format!("{}: still not installed.", tool.volume()))
                            }
                            Req::Cancel => close = Some(i),
                        }
                    }
                }
            }
        }
        if let Some(id) = pending_dial {
            self.start_dial(id, None);
        } else if let Some(id) = pending_edit {
            self.edit(id, save);
        } else if let Some(i) = close {
            if i < self.wins.len() {
                self.wins.remove(i);
            }
        } else if let Some(i) = raise
            && i < self.wins.len()
            && i + 1 != self.wins.len()
        {
            let w = self.wins.remove(i);
            self.wins.push(w);
        }
        out
    }

    fn activate(&mut self, tool: Tool) {
        self.open(tool);
    }

    /// Shows a finished scan filling in on the WarDial tab.
    pub fn show_scan(&mut self, result: ScanResult) {
        self.range_sel = Some(result.range.clone());
        self.scan = Some((result, 0.0));
        self.term_tab = TermTab::WarDial;
    }

    /// Starts the modem handshake for a phonebook entry, in the CarrierTerm window.
    pub fn start_dial(&mut self, entry: u32, seed: Option<u64>) {
        self.term_sel = Some(entry);
        self.term_tab = TermTab::Phonebook;
        self.dialing = Some(Dialing {
            entry,
            seed,
            t: 0.0,
        });
        self.dial_started = Some(entry);
        self.open(Tool::Terminal);
    }

    fn shell(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        data: &Data,
        save: &Save,
        focused: bool,
        out: &mut Vec<WbOut>,
    ) {
        let painter = ui.painter_at(rect);
        let row = FONT + 3.0;
        let rows = ((rect.height() - row) / row).floor().max(0.0) as usize;
        let start = self.shell_lines.len().saturating_sub(rows);
        for (k, line) in self.shell_lines[start..].iter().enumerate() {
            painter.text(
                rect.min + vec2(0.0, k as f32 * row),
                Align2::LEFT_TOP,
                line,
                font(),
                ink(),
            );
        }
        let prompt = "1.Workbench:> ";
        let py = rect.min.y + (self.shell_lines.len() - start) as f32 * row;
        let pw = painter
            .text(
                pos2(rect.min.x, py),
                Align2::LEFT_TOP,
                prompt,
                font(),
                accent(),
            )
            .width();
        let input_rect =
            Rect::from_min_max(pos2(rect.min.x + pw, py - 1.0), pos2(rect.max.x, py + row));
        let edit = egui::TextEdit::singleline(&mut self.shell_input)
            .frame(egui::Frame::NONE)
            .font(font())
            .text_color(ink())
            .desired_width(input_rect.width())
            .id(Id::new("wb-shell-input"));
        let resp = ui.put(input_rect, edit);
        if focused && self.focus_shell {
            resp.request_focus();
            self.focus_shell = false;
        }
        if resp.clicked() {
            self.focus_shell = true;
        }
        if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            let line = std::mem::take(&mut self.shell_input);
            self.say(format!("{prompt}{line}"));
            let (lines, effect) = hub::shell(&line, save, data);
            for l in lines {
                self.say(l);
            }
            match effect {
                ShellEffect::None => {}
                ShellEffect::Dial(entry, seed) => self.start_dial(entry, seed),
                ShellEffect::Scan(range) => {
                    self.term_tab = TermTab::WarDial;
                    self.range_sel = Some(range.clone());
                    self.open(Tool::Terminal);
                    out.push(WbOut::Scan(range));
                }
                ShellEffect::Call(id) => out.push(WbOut::Call(id)),
                ShellEffect::Open(t) => self.activate(t),
                ShellEffect::Theme(t) => out.push(WbOut::Theme(t)),
                ShellEffect::Crt => out.push(WbOut::Crt),
                ShellEffect::Sound => out.push(WbOut::Sound),
                ShellEffect::Display(id) => out.push(WbOut::Display(id)),
                ShellEffect::Clear => self.shell_lines.clear(),
            }
            self.focus_shell = true;
        }
    }
}

enum Req {
    Retry,
    Cancel,
}

fn requester(
    ui: &mut Ui,
    painter: &egui::Painter,
    rect: Rect,
    tool: Tool,
    live: bool,
    wid: Id,
) -> Option<Req> {
    let lines = ["Please insert volume", tool.volume(), "in any drive"];
    for (k, l) in lines.iter().enumerate() {
        let col = if k == 1 { accent() } else { ink() };
        painter.text(
            pos2(rect.center().x, rect.top() + 6.0 + k as f32 * 18.0),
            Align2::CENTER_TOP,
            *l,
            font(),
            col,
        );
    }
    let bw = vec2(84.0, 22.0);
    let retry = Rect::from_min_size(pos2(rect.left() + 6.0, rect.bottom() - bw.y), bw);
    let cancel = Rect::from_min_size(pos2(rect.right() - 6.0 - bw.x, rect.bottom() - bw.y), bw);
    let mut out = None;
    for (r, label, what) in [(retry, "Retry", 0), (cancel, "Cancel", 1)] {
        let resp = ui.interact(
            r,
            wid.with(label),
            if live { Sense::click() } else { Sense::hover() },
        );
        let pressed = resp.is_pointer_button_down_on();
        button(painter, r, label, pressed);
        if resp.clicked() {
            out = Some(if what == 0 { Req::Retry } else { Req::Cancel });
        }
    }
    out
}

/// A 1.3 push button: white box, black shadow edges, inverted while pressed.
fn button(painter: &egui::Painter, r: Rect, label: &str, pressed: bool) {
    let (face, text) = if pressed {
        (accent(), shadow())
    } else {
        (backdrop(), ink())
    };
    painter.rect_filled(r, 0.0, face);
    painter.line_segment(
        [r.left_top(), r.right_top()],
        Stroke::new(2.0, if pressed { shadow() } else { ink() }),
    );
    painter.line_segment(
        [r.left_top(), r.left_bottom()],
        Stroke::new(2.0, if pressed { shadow() } else { ink() }),
    );
    painter.line_segment(
        [r.left_bottom(), r.right_bottom()],
        Stroke::new(2.0, if pressed { ink() } else { shadow() }),
    );
    painter.line_segment(
        [r.right_top(), r.right_bottom()],
        Stroke::new(2.0, if pressed { ink() } else { shadow() }),
    );
    painter.text(r.center(), Align2::CENTER_CENTER, label, font(), text);
}

/// Window chrome: white border, a title bar (striped when active), close and depth gadgets.
fn window_frame(painter: &egui::Painter, rect: Rect, title: &str, active: bool) {
    painter.rect_filled(rect, 0.0, backdrop());
    painter.rect_stroke(rect, 0.0, Stroke::new(BORDER, ink()), StrokeKind::Inside);
    let bar = Rect::from_min_size(rect.min, vec2(rect.width(), TITLE_H));
    painter.rect_filled(bar, 0.0, if active { ink() } else { backdrop() });
    if active {
        // Horizontal pinstripes behind the title, the active-window tell.
        let mut y = bar.top() + 4.0;
        while y < bar.bottom() - 3.0 {
            painter.line_segment(
                [pos2(bar.left() + 24.0, y), pos2(bar.right() - 30.0, y)],
                Stroke::new(1.0, backdrop()),
            );
            y += 3.0;
        }
    }
    painter.rect_stroke(bar, 0.0, Stroke::new(BORDER, ink()), StrokeKind::Inside);
    let galley = painter.layout_no_wrap(
        title.to_string(),
        font(),
        if active { backdrop() } else { ink() },
    );
    let tw = galley.size().x + 10.0;
    let tbox = Rect::from_min_size(bar.min + vec2(26.0, 2.0), vec2(tw, TITLE_H - 4.0));
    painter.rect_filled(tbox, 0.0, if active { ink() } else { backdrop() });
    painter.galley(tbox.min + vec2(5.0, 1.0), galley, ink());
    // Close gadget: a box with a dot.
    let c = Rect::from_min_size(bar.min + vec2(4.0, 3.0), vec2(16.0, 14.0));
    painter.rect_filled(c, 0.0, ink());
    painter.rect_stroke(c, 0.0, Stroke::new(1.0, shadow()), StrokeKind::Inside);
    painter.rect_filled(
        Rect::from_center_size(c.center(), vec2(4.0, 4.0)),
        0.0,
        shadow(),
    );
    depth_gadget(
        painter,
        Rect::from_min_size(bar.right_top() + vec2(-26.0, 2.0), vec2(22.0, 16.0)),
    );
}

/// Two overlapping squares: back one outlined, front one filled.
fn depth_gadget(painter: &egui::Painter, r: Rect) {
    painter.rect_filled(r, 0.0, ink());
    let back = Rect::from_min_size(r.min + vec2(3.0, 2.0), vec2(11.0, 8.0));
    let front = Rect::from_min_size(r.min + vec2(8.0, 6.0), vec2(11.0, 8.0));
    painter.rect_stroke(back, 0.0, Stroke::new(1.0, shadow()), StrokeKind::Inside);
    painter.rect_filled(front, 0.0, accent());
    painter.rect_stroke(front, 0.0, Stroke::new(1.0, shadow()), StrokeKind::Inside);
}

/// Paints an icon's pixels. Selected icons draw in complement colours, as on the real thing.
fn draw_icon(painter: &egui::Painter, icon: &IconDef, at: Pos2, selected: bool) {
    for (y, row) in icon.pixels.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            let col = match (ch, selected) {
                ('w', false) | ('k', true) => ink(),
                ('k', false) | ('w', true) => shadow(),
                ('o', false) | ('b', true) => accent(),
                ('b', false) | ('o', true) => fill(),
                _ => continue,
            };
            let p = at + vec2(x as f32, y as f32) * ICON_SCALE;
            painter.rect_filled(Rect::from_min_size(p, Vec2::splat(ICON_SCALE)), 0.0, col);
        }
    }
}
