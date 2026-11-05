//! Home: what every desktop shares. The desktop palette, what home asks the app to do
//! ([`WbOut`]), the Shell, CarrierTerm's dialing and scans, the HexEd buffer, and the code that
//! draws each tool's contents. A desktop (Workbench, tiling, ...) is only a front-end on this:
//! it implements [`DesktopView`] and lives in the registry ([`Desktops`]).

use egui::{Align2, Color32, FontFamily, FontId, Id, Rect, Ui, pos2, vec2};
use ninsei_core::Data;
use ninsei_core::binary::Bin;
use ninsei_core::data::ThemeDef;
use ninsei_core::hardware::Desktop;
use ninsei_core::hexedit::HexBuffer;
use ninsei_core::hub::{self, Save, ScanResult, ShellEffect, Tool, Upgrade};
use ninsei_core::modem::Handshake;
use std::cell::Cell;

use crate::hubtools;
use crate::theme::Palette;

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

pub(crate) fn desk() -> Desk {
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

pub(crate) const FONT: f32 = 14.0;
const SHELL_KEEP: usize = 200;
/// Probe runs played each frame: enough to finish a default probe in seconds, few enough to
/// keep the desktop smooth.
const PROBE_RUNS_PER_FRAME: u32 = 4;

pub(crate) fn font() -> FontId {
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
    /// Plug in a monitor you own (by id).
    Monitor(String),
    /// Run with a deck you own (by id).
    Deck(String),
    /// Switch the deck's firmware (by id).
    Firmware(String),
    /// Damage glitches on, reduced or off.
    Glitches(ninsei_core::glitch::Glitches),
    /// The Shell ran a command: keep its history in the save.
    ShellHistory(Vec<String>),
    /// Buy a secondary pane for the run screen in RIG (id).
    BuyPane(String),
    /// Buy a desktop add-on in RIG (id).
    BuyAddon(String),
    /// The tiling desktop's first-boot key help was dismissed.
    HelpSeen,
    /// The tiling desktop's tiles changed: keep them.
    Tiling(ninsei_core::desktop::Tiling),
    /// Buy a monitor or deck in RIG.
    BuyHardware(ninsei_core::hardware::Hardware),
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

/// A desktop: draws the whole home screen (dragged down by `offset` while jacking in or out)
/// and routes input when `live`. Everything a desktop shows comes from [`Home`]; tools it's
/// asked to open arrive through [`Home::take_opens`].
pub trait DesktopView {
    fn show(&mut self, ui: &mut Ui, home: &mut Home, cx: &Cx, out: &mut Vec<WbOut>);
}

/// What a desktop draws from this frame.
pub struct Cx<'a> {
    pub data: &'a Data,
    pub save: &'a Save,
    /// How far the screen is dragged down (jacking in or out).
    pub offset: f32,
    /// Whether it takes input.
    pub live: bool,
}

/// Every desktop, by name. A new desktop is a `Desktop` variant in core, a type implementing
/// [`DesktopView`], and one arm in [`Desktops::get`].
pub struct Desktops {
    workbench: crate::workbench::Workbench,
    tiling: crate::tiling::TilingDesk,
    liza: crate::liza::Liza,
}

impl Desktops {
    pub fn new() -> Self {
        Self {
            workbench: crate::workbench::Workbench::new(),
            tiling: crate::tiling::TilingDesk::new(),
            liza: crate::liza::Liza::new(),
        }
    }

    pub fn get(&mut self, d: Desktop) -> &mut dyn DesktopView {
        match d {
            Desktop::Workbench => &mut self.workbench,
            Desktop::Tiling => &mut self.tiling,
            Desktop::Liza => &mut self.liza,
        }
    }
}

/// Home's shared state.
pub struct Home {
    shell_lines: Vec<String>,
    /// The balance probe, while `probe` runs it a few runs a frame, and the last progress
    /// step printed (in tenths).
    probe: Option<(ninsei_core::probe::Probe, u32)>,
    /// The Shell's command line: readline keys, history, completion.
    shell: ninsei_term::LineEditor,
    focus_shell: bool,
    /// The Shell's prompt (each desktop sets its own).
    pub(crate) prompt: &'static str,
    /// Tools asked to open (by the Shell, a dial, a scan); the desktop opens them its way.
    opens: Vec<Tool>,
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
    /// The QWK reader: which packet or message is open, and true ANSI colours or the desk's.
    pub(crate) qwk: ninsei_core::qwk::Reader,
    pub(crate) qwk_color: bool,
}

impl Home {
    pub fn new(boot: Vec<String>) -> Self {
        Self {
            shell_lines: boot,
            shell: ninsei_term::LineEditor::new(ninsei_core::command::HISTORY_KEEP),
            probe: None,
            focus_shell: true,
            prompt: "1.Workbench:> ",
            opens: vec![Tool::Shell],
            hex: None,
            term_sel: None,
            term_tab: TermTab::Phonebook,
            range_sel: None,
            scan: None,
            dialing: None,
            dial_started: None,
            qwk: ninsei_core::qwk::Reader::new(),
            qwk_color: false,
        }
    }

    /// The QWK reader's window body: the screen, its hotkeys as buttons, and keys while the
    /// pointer is over it (the Shell keeps the keyboard otherwise).
    fn qwk_reader(&mut self, ui: &mut Ui, data: &Data, save: &Save) {
        // The desk's own colours when it isn't showing true ANSI.
        let p = Palette {
            bg: backdrop(),
            panel: backdrop(),
            faint: muted(),
            dim: muted(),
            fg: ink(),
            bright: accent(),
        };
        let r = &mut self.qwk;
        // The body hasn't grown yet, so test the window's visible area, not the ui's rect.
        let over = ui
            .ctx()
            .pointer_hover_pos()
            .is_some_and(|at| ui.clip_rect().contains(at));
        if over {
            let keys: Vec<char> = ui.input(|i| {
                i.events
                    .iter()
                    .flat_map(|e| match e {
                        egui::Event::Text(t) => t.chars().collect(),
                        egui::Event::Key {
                            key: egui::Key::Enter,
                            pressed: true,
                            ..
                        } => vec!['\n'],
                        egui::Event::Key {
                            key: egui::Key::Escape,
                            pressed: true,
                            ..
                        } => vec!['\x1b'],
                        _ => Vec::new(),
                    })
                    .collect()
            });
            for k in keys {
                r.key(save, data, k);
            }
        }
        let lines = r.render(save, data);
        egui::ScrollArea::both().show(ui, |ui| {
            crate::bbs_view::paint_screen(ui, &lines, self.qwk_color, &p);
        });
        let mut pressed = None;
        ui.horizontal_wrapped(|ui| {
            for (k, label) in self.qwk.keys(save, data) {
                let txt = format!("[{k}] {label}");
                if ui.button(txt.trim_end()).clicked() {
                    pressed = Some(k);
                }
            }
            let mode = if self.qwk_color { "ANSI color" } else { "desk" };
            if ui.button(format!("display: {mode}")).clicked() {
                self.qwk_color = !self.qwk_color;
            }
        });
        if let Some(k) = pressed {
            self.qwk.key(save, data, k);
        }
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

    /// Asks the desktop to open (or bring forward) a tool.
    pub fn open(&mut self, tool: Tool) {
        if tool == Tool::Shell {
            self.focus_shell = true;
        }
        if !self.opens.contains(&tool) {
            self.opens.push(tool);
        }
    }

    /// The tools waiting to be opened, for the desktop to open its way.
    pub fn take_opens(&mut self) -> Vec<Tool> {
        std::mem::take(&mut self.opens)
    }

    /// Opens a stash program in HexEd.
    pub fn edit(&mut self, id: u32, save: &Save) {
        if !save.opens(Tool::HexEd) {
            return;
        }
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

    /// Shows a finished scan filling in on the WarDial tab.
    pub fn show_scan(&mut self, result: ScanResult) {
        self.range_sel = Some(result.range.clone());
        self.scan = Some((result, 0.0));
        self.term_tab = TermTab::WarDial;
        self.open(Tool::Terminal);
    }

    /// Starts the modem handshake for a phonebook entry, in CarrierTerm.
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

    /// Draws home with the desktop `view`: runs the dial and scan clocks, then the desktop.
    pub fn show(
        &mut self,
        view: &mut dyn DesktopView,
        ui: &mut Ui,
        data: &Data,
        save: &Save,
        offset: f32,
        live: bool,
    ) -> Vec<WbOut> {
        self.sync_hex(save);
        self.step_probe(ui);
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
        let cx = Cx {
            data,
            save,
            offset,
            live,
        };
        view.show(ui, self, &cx, &mut out);
        out
    }

    /// Draws a tool's contents (everything but the Shell) into `child`. Desktops call this
    /// inside whatever frame they give a tool: a window, a tile.
    pub fn tool_body(
        &mut self,
        child: &mut Ui,
        tool: Tool,
        data: &Data,
        save: &Save,
        out: &mut Vec<WbOut>,
    ) {
        if tool == Tool::Shell {
            return;
        }
        let mut edit = None;
        let mut dial = None;
        match tool {
            Tool::Terminal => {
                // No dialer, no WarDial tab: CarrierTerm is just the phonebook.
                if !save.has_tool("wardial") {
                    self.term_tab = TermTab::Phonebook;
                }
                if self.dialing.is_none() && save.has_tool("wardial") {
                    child.horizontal(|ui| {
                        for (tab, label) in [
                            (TermTab::Phonebook, " Phonebook "),
                            (TermTab::WarDial, " WarDial "),
                        ] {
                            if ui.selectable_label(self.term_tab == tab, label).clicked() {
                                self.term_tab = tab;
                            }
                        }
                    });
                    child.separator();
                }
                if self.term_tab == TermTab::WarDial && self.dialing.is_none() {
                    hubtools::wardial(
                        child,
                        data,
                        save,
                        &mut self.range_sel,
                        self.scan.as_ref(),
                        out,
                    );
                } else {
                    hubtools::carrierterm(
                        child,
                        data,
                        save,
                        &mut self.term_sel,
                        self.dialing.as_ref(),
                        &mut dial,
                        out,
                    );
                }
            }
            Tool::Rig => hubtools::rig(child, data, save, out),
            Tool::Prefs => hubtools::prefs(child, data, save, out),
            Tool::Stash => hubtools::stash(child, data, save, out, &mut edit),
            Tool::HexEd => hubtools::hexed(child, save, &mut self.hex, out, &mut edit),
            Tool::Qwk => self.qwk_reader(child, data, save),
            Tool::Shell => {}
        }
        if let Some(id) = dial {
            self.start_dial(id, None);
        } else if let Some(id) = edit {
            self.edit(id, save);
        }
    }

    /// The Shell: its scrollback and prompt in `rect`, typed into when `focused`.
    pub fn shell(
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
        let prompt = self.prompt;
        let py = rect.min.y + (self.shell_lines.len() - start) as f32 * row;
        let input_rect =
            Rect::from_min_max(pos2(rect.min.x, py - 2.0), pos2(rect.max.x, py + row + 2.0));
        let id = Id::new("wb-shell-input");
        // Keep the request until the Shell can take it: a disabled frame (the desktop still
        // settling) would hand the focus straight back.
        if focused && self.focus_shell && ui.is_enabled() {
            ui.memory_mut(|m| m.request_focus(id));
            self.focus_shell = false;
        }
        // Only the Shell on top (or the focused tile) types.
        if !focused && ui.memory(|m| m.has_focus(id)) {
            ui.memory_mut(|m| m.surrender_focus(id));
        }
        let style = ninsei_term::widget::Style {
            font: font(),
            prompt: accent(),
            text: ink(),
            cursor: ink(),
            under_cursor: backdrop(),
            search: accent(),
        };
        let complete = |before: &str| hub::shell_complete(before, save, data);
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(input_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        let r = ninsei_term::widget::line(
            &mut child,
            id,
            &mut self.shell,
            prompt.trim_end(),
            &style,
            &complete,
        );
        if r.response.clicked() {
            self.focus_shell = true;
        }
        for o in r.outcomes {
            match o {
                ninsei_term::Outcome::Submit(line) => {
                    self.say(format!("{prompt}{line}"));
                    self.run(&line, data, save, out);
                    out.push(WbOut::ShellHistory(self.shell.history().to_vec()));
                }
                ninsei_term::Outcome::Cancelled(was) => self.say(format!("{prompt}{was}^C")),
                ninsei_term::Outcome::ClearScreen => self.shell_lines.clear(),
                ninsei_term::Outcome::List(words) => self.say(words.join("   ")),
                ninsei_term::Outcome::Nothing => {}
            }
        }
    }

    /// What's typed in the Shell's command line.
    #[cfg(test)]
    pub fn shell_text(&self) -> String {
        self.shell.text()
    }

    /// Plays a few of the probe's runs (there are no threads in the browser), prints progress
    /// every tenth, then the report.
    fn step_probe(&mut self, ui: &Ui) {
        let Some((p, shown)) = &mut self.probe else {
            return;
        };
        let finished = p.step(PROBE_RUNS_PER_FRAME);
        let (done, total) = p.progress();
        let tenth = done * 10 / total.max(1);
        let mut lines = Vec::new();
        if tenth > *shown && !finished {
            *shown = tenth;
            lines.push(format!("probe: {}% ({done}/{total} runs)", tenth * 10));
        }
        if finished {
            lines.extend(p.report());
            self.probe = None;
        } else {
            ui.ctx().request_repaint();
        }
        for l in lines {
            self.say(l);
        }
    }

    /// Whether the Shell is in a `Ctrl+R` search.
    pub fn shell_searching(&self) -> bool {
        self.shell.searching()
    }

    /// Restores the Shell's history (from the save), oldest first.
    pub fn set_shell_history(&mut self, history: Vec<String>) {
        self.shell.set_history(history);
    }

    /// Runs a Shell command line: prints what it says and does what it asks.
    pub fn run(&mut self, line: &str, data: &Data, save: &Save, out: &mut Vec<WbOut>) {
        let (lines, effect) = hub::shell(line, save, data);
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
            ShellEffect::Open(t) => self.open(t),
            ShellEffect::Theme(t) => out.push(WbOut::Theme(t)),
            ShellEffect::Crt => out.push(WbOut::Crt),
            ShellEffect::Sound => out.push(WbOut::Sound),
            ShellEffect::Monitor(id) => out.push(WbOut::Monitor(id)),
            ShellEffect::Deck(id) => out.push(WbOut::Deck(id)),
            ShellEffect::Clear => self.shell_lines.clear(),
            ShellEffect::Probe(n) => {
                let p =
                    ninsei_core::probe::Probe::new(data, ninsei_core::probe::Config::careers(n));
                let (_, total) = p.progress();
                self.say(format!(
                    "probe: bots play {n} careers ({total} runs). `probe stop` to stop."
                ));
                self.probe = Some((p, 0));
            }
            ShellEffect::ProbeStop => {
                if self.probe.take().is_some() {
                    self.say("probe: stopped.");
                } else {
                    self.say("probe: nothing running.");
                }
            }
        }
    }
}

/// A dim line for an empty tile or slot.
/// A tool's body in a window or tile that may be shorter than it: it scrolls, with a solid bar
/// drawn in the text colour so every theme shows it.
pub(crate) fn scrolled(
    ui: &mut Ui,
    salt: impl std::hash::Hash + std::fmt::Debug,
    body: impl FnOnce(&mut Ui),
) {
    ui.spacing_mut().scroll = crate::theme::visible_scroll();
    egui::ScrollArea::vertical()
        .id_salt(salt)
        .auto_shrink([false, false])
        .show(ui, body);
}

pub(crate) fn hint(painter: &egui::Painter, rect: Rect, text: &str) {
    painter.text(
        pos2(rect.center().x, rect.center().y),
        Align2::CENTER_CENTER,
        text,
        font(),
        muted(),
    );
}
