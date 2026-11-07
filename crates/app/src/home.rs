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
use ninsei_core::hub::{self, Save, ScanResult, ShellEffect, Tool};
use ninsei_core::modem::Handshake;
use std::cell::Cell;

use crate::hubtools;
use crate::kit::{Fit, fit};
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
    /// Wardial a range (by id), from this tool's wardialer (none: the Shell's `wardial`).
    /// The result shows where it was asked for.
    Scan(String, Option<Tool>),
    /// A line typed at a modem terminal (NiGHTCOMM's TERM or Tiger Team Pro's Console), for
    /// the modem (core's `at::run`).
    At(Tool, String),
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
    /// Cracktros on or off.
    Intros(bool),
    /// The Welcome to the Net stack opened on first boot.
    GuideSeen,
    /// The Shell ran a command: keep its history in the save.
    ShellHistory(Vec<String>),
    /// The tiling desktop's first-boot key help was dismissed.
    HelpSeen,
    /// The tiling desktop's tiles changed: keep them.
    Tiling(ninsei_core::desktop::Tiling),
    /// Order anything from CPU Shopper (RIG).
    Order(ninsei_core::shopper::Item),
    Crt,
    /// Modem sounds on/off.
    Sound,
    /// Install a program on the disk (System's Disk panel): it's in the next run.
    Install(ninsei_core::disk::Disk),
    /// Take a program off the disk, back into the box.
    Remove(ninsei_core::disk::Disk),
    /// Defragment finished: lay the disk out end to end.
    Defragment,
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
    /// NiGHTCOMM's page, order, filter and picks.
    pub(crate) nightcomm: crate::nightcomm_view::NightComm,
    /// Tiger Team Pro's section, picks and its own terminal.
    pub(crate) tigerteam: crate::tigerteam_view::TigerTeam,
    /// The tools a desktop drew the "too small" notice for instead (tests).
    #[cfg(test)]
    pub(crate) notices: Vec<Tool>,
    pub(crate) term_tab: TermTab,
    /// Range highlighted in WarDial.
    pub(crate) range_sel: Option<String>,
    /// The last scan and how long it's been filling in.
    pub(crate) scan: Option<(ScanResult, f32)>,
    /// A dial in progress: the modem handshake before jacking in.
    pub(crate) dialing: Option<Dialing>,
    /// A dial that just started (phonebook entry id), for the app to play its handshake.
    pub(crate) dial_started: Option<u32>,
    /// Set when a call is hung up mid-handshake, so the app can cut the modem.
    pub(crate) dial_cut: bool,
    /// CarrierTerm's directory: search, filter and the line picked.
    pub(crate) directory: hubtools::Directory,
    /// The QWK reader: which packet or message is open, and true ANSI colours or the desk's.
    pub(crate) qwk: ninsei_core::qwk::Reader,
    pub(crate) qwk_color: bool,
    /// The ICE database's picked entry.
    pub(crate) icedb_sel: Option<usize>,
    /// Cracktros: the pass each tool's body was last drawn on (so a fresh open replays its
    /// intro), and the intros playing now with the time each started.
    intro_drawn: std::collections::HashMap<&'static str, u64>,
    intro_playing: std::collections::HashMap<&'static str, f64>,
    /// For the app's speaker: an intro whose tune should start (its id), or stop.
    pub(crate) tune: Option<TuneCue>,
    /// Where the Welcome to the Net stack is open.
    pub(crate) guide_at: ninsei_core::cards::Place,
    /// System's open panel, and its Disk panel's pick.
    pub(crate) system: crate::system::SystemUi,
}

/// What the speaker should do with the cracktro's tune this frame.
#[derive(Clone, Debug, PartialEq)]
pub enum TuneCue {
    /// Start the tune of this intro (an id in `data/intros/`).
    Play(String),
    Stop,
}

/// The desk's colours as a run palette, for screens drawn like a BBS (QWK, cracktros).
fn desk_palette() -> Palette {
    Palette {
        bg: backdrop(),
        panel: backdrop(),
        faint: muted(),
        dim: muted(),
        fg: ink(),
        bright: accent(),
    }
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
            nightcomm: Default::default(),
            tigerteam: Default::default(),
            #[cfg(test)]
            notices: Vec::new(),
            term_tab: TermTab::Phonebook,
            range_sel: None,
            scan: None,
            dialing: None,
            dial_started: None,
            dial_cut: false,
            directory: hubtools::Directory::default(),
            qwk: ninsei_core::qwk::Reader::new(),
            qwk_color: false,
            icedb_sel: None,
            intro_drawn: Default::default(),
            intro_playing: Default::default(),
            tune: None,
            guide_at: ninsei_core::cards::Place::Home,
            system: crate::system::SystemUi::default(),
        }
    }

    /// The QWK reader's window body: the screen, its hotkeys as buttons, and keys while the
    /// pointer is over it (the Shell keeps the keyboard otherwise).
    fn qwk_reader(&mut self, ui: &mut Ui, data: &Data, save: &Save) {
        // The desk's own colours when it isn't showing true ANSI.
        let p = desk_palette();
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

    /// Plays tool `id`'s cracktro in its window, every time the window opens, until it ends or
    /// a click or key skips it. True while it's showing (draw nothing else). A tool with no
    /// intro, or intros switched off in System, opens straight away.
    fn intro_gate(&mut self, ui: &mut Ui, data: &Data, save: &Save, id: &'static str) -> bool {
        match data.tool(id).and_then(|t| t.intro.clone()) {
            Some(intro) => self.splash_gate(ui, data, save, id, &intro),
            None => false,
        }
    }

    /// Plays intro `intro` (an id in `data/intros/`) under the key `id`, as [`Home::intro_gate`]
    /// does for a tool's cracktro: CarrierTerm's own splash goes through here too.
    fn splash_gate(
        &mut self,
        ui: &mut Ui,
        data: &Data,
        save: &Save,
        id: &'static str,
        intro: &str,
    ) -> bool {
        let Some(def) = data.intro(intro) else {
            return false;
        };
        let pass = ui.ctx().cumulative_pass_nr();
        let now = ui.input(|i| i.time);
        // Not drawn last pass: the window (or tab) just opened.
        let fresh = self
            .intro_drawn
            .insert(id, pass)
            .is_none_or(|last| last + 1 < pass);
        if fresh && save.intros {
            self.intro_playing.insert(id, now);
            self.tune = Some(TuneCue::Play(def.id.clone()));
        }
        let Some(&start) = self.intro_playing.get(id) else {
            return false;
        };
        let over = |p: egui::Pos2| ui.clip_rect().contains(p);
        let skip = ui.input(|i| {
            i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(over)
                || i.pointer.hover_pos().is_some_and(over)
                    && i.events.iter().any(|e| {
                        matches!(
                            e,
                            egui::Event::Key { pressed: true, .. } | egui::Event::Text(_)
                        )
                    })
        });
        let t = (now - start) as f32;
        if skip || def.done(t) {
            self.intro_playing.remove(id);
            if skip {
                self.tune = Some(TuneCue::Stop);
            }
            return false;
        }
        let calm = save.glitches != ninsei_core::glitch::Glitches::On;
        let lines = def.screen(data, t, calm);
        crate::bbs_view::paint_screen(ui, &lines, false, &desk_palette());
        ui.ctx().request_repaint();
        true
    }

    /// Forgets intros whose windows closed while they played (not drawn since the last pass),
    /// and asks for their tune to stop.
    pub fn drop_closed_intros(&mut self, pass: u64) {
        let drawn = &self.intro_drawn;
        let before = self.intro_playing.len();
        self.intro_playing
            .retain(|id, _| drawn.get(id).is_some_and(|&p| p + 1 >= pass));
        if self.intro_playing.len() < before {
            self.tune = Some(TuneCue::Stop);
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

    /// Shows a finished scan filling in, in the wardialer it came `from`. From the Shell, in
    /// the best one you have: Tiger Team Pro, NiGHTCOMM, or CarrierTerm's WarDial tab.
    pub fn show_scan(&mut self, result: ScanResult, save: &Save, from: Option<Tool>) {
        self.range_sel = Some(result.range.clone());
        self.nightcomm.range = Some(result.range.clone());
        self.tigerteam.nc.range = Some(result.range.clone());
        self.scan = Some((result, 0.0));
        let to = from.unwrap_or(if save.has_tool("tigerteam") {
            Tool::TigerTeam
        } else if save.has_tool("nightcomm") {
            Tool::NightComm
        } else {
            Tool::Terminal
        });
        if to == Tool::TigerTeam {
            self.tigerteam.page = ninsei_core::tigerteam::Page::Recon;
            self.open(Tool::TigerTeam);
        } else if to == Tool::NightComm {
            self.nightcomm.tab = ninsei_core::nightcomm::Tab::Wardial;
            self.open(Tool::NightComm);
        } else {
            self.term_tab = TermTab::WarDial;
            self.open(Tool::Terminal);
        }
    }

    /// Starts the handshake from NiGHTCOMM's TERM, which prints it itself: CarrierTerm
    /// stays where it is.
    pub fn start_dial_here(&mut self, entry: u32) {
        self.term_sel = Some(entry);
        self.dialing = Some(Dialing {
            entry,
            seed: None,
            t: 0.0,
        });
        self.dial_started = Some(entry);
    }

    /// The modem terminal a tool types at: Tiger Team Pro's Console, else NiGHTCOMM's TERM.
    pub fn term_for(&mut self, from: Tool) -> &mut crate::nightcomm_view::NightComm {
        if from == Tool::TigerTeam {
            &mut self.tigerteam.nc
        } else {
            &mut self.nightcomm
        }
    }

    /// A dial asked for in `tool`'s window. A program with its own modem terminal plays the
    /// call there (NiGHTCOMM on TERM, Tiger Team Pro on its Console); anything else hands it
    /// to CarrierTerm.
    fn dial_from(&mut self, tool: Tool, entry: u32, save: &Save) {
        let number = save.entry(entry).map(|e| e.number.clone());
        match (tool, number) {
            (Tool::NightComm | Tool::TigerTeam, Some(n)) => {
                if tool == Tool::TigerTeam {
                    self.tigerteam.page = ninsei_core::tigerteam::Page::Console;
                } else {
                    self.nightcomm.tab = ninsei_core::nightcomm::Tab::Term;
                }
                self.term_for(tool)
                    .term_say(&[format!("> ATDT {n}"), format!("DIALING {n}")]);
                self.start_dial_here(entry);
            }
            _ => self.start_dial(entry, None),
        }
    }

    /// `ATH`: drops a call under way, and the modem sound with it.
    pub fn hang_up(&mut self) {
        if self.dialing.take().is_some() {
            self.dial_cut = true;
        }
    }

    /// Starts the modem handshake for a phonebook entry, in CarrierTerm.
    pub fn start_dial(&mut self, entry: u32, seed: Option<u64>) {
        self.term_sel = Some(entry);
        self.directory.sel = Some(ninsei_core::directory::Listed::Entry(entry));
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

    #[cfg(test)]
    /// The text-mode tools: the ones laid out in cells, which the fit test draws at every size
    /// the tiler gives.
    pub const TEXT_TOOLS: [Tool; 3] = [Tool::NightComm, Tool::TigerTeam, Tool::System];

    /// The smallest grids of cells each tool works in (it fits if any one does): the one
    /// place a tool says so. A tool that says nothing gets [`Fit::DEFAULT`].
    pub fn fit(tool: Tool) -> &'static [Fit] {
        match tool {
            // The list with its header and a few rows, and every page's columns.
            Tool::NightComm | Tool::TigerTeam => &[Fit { cols: 56, rows: 12 }],
            // The text face's About panel: the logo beside the specs fills an 80x24 terminal;
            // narrower, the logo goes above them and it needs the height. Every other panel
            // fits inside either.
            Tool::System => &[Fit { cols: 80, rows: 24 }, Fit { cols: 56, rows: 34 }],
            _ => &[Fit::DEFAULT],
        }
    }

    /// Draws `tool` in `ui` (a desktop's window or tile, dressed by `hubtools::wb_visuals`) if
    /// the room it has is at least the tool's [`Home::fit`]; below it, the one notice in its
    /// place, with the desktop's `hint`. Returns whether the tool was drawn.
    #[allow(clippy::too_many_arguments)]
    pub fn tool_in(
        &mut self,
        ui: &mut Ui,
        tool: Tool,
        data: &Data,
        save: &Save,
        out: &mut Vec<WbOut>,
        hint: Option<&str>,
        salt: impl std::hash::Hash + std::fmt::Debug,
    ) -> bool {
        let rect = ui.max_rect();
        let need = Self::fit(tool);
        let (have, fits) = fit::check(ui, rect, need);
        if !fits {
            #[cfg(test)]
            self.notices.push(tool);
            fit::notice(
                ui,
                rect,
                tool.volume(),
                need,
                have,
                hint,
                (backdrop(), ink(), muted()),
            );
            return false;
        }
        scrolled(ui, salt, |ui| self.tool_body(ui, tool, data, save, out));
        true
    }

    /// Draws a tool's contents (everything but the Shell) into `child`. Desktops call it
    /// through [`Home::tool_in`], inside whatever frame they give a tool: a window, a tile.
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
            // CarrierTerm's splash, unless a dial is already under way.
            Tool::Terminal
                if self.dialing.is_none()
                    && self.splash_gate(child, data, save, "carrierterm", "carrierterm") =>
            {
                return;
            }
            Tool::Terminal => {
                // Drawn this pass, splash or not, so hanging up a dial doesn't replay it.
                self.intro_drawn
                    .insert("carrierterm", child.ctx().cumulative_pass_nr());
                // No dialer, no WarDial tab: CarrierTerm is just the phonebook. Once
                // NiGHTCOMM is here, wardialing is its job.
                let wardial_tab = save.has_tool("wardial") && !save.has_dialer();
                if !wardial_tab {
                    self.term_tab = TermTab::Phonebook;
                }
                if self.dialing.is_none() && wardial_tab {
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
                    if self.intro_gate(child, data, save, "wardial") {
                        return;
                    }
                    hubtools::wardial(
                        child,
                        data,
                        save,
                        &mut self.range_sel,
                        self.scan.as_ref(),
                        out,
                    );
                } else {
                    let mut hang_up = false;
                    hubtools::carrierterm(
                        child,
                        data,
                        save,
                        &mut self.directory,
                        self.dialing.as_ref(),
                        &mut dial,
                        &mut hang_up,
                        out,
                    );
                    if hang_up {
                        self.dialing = None;
                        self.dial_cut = true;
                    }
                }
            }
            Tool::Rig => crate::shopper::show(child, data, save, out),
            Tool::System => crate::system::show(child, data, save, &mut self.system, out),
            Tool::HexEd if self.intro_gate(child, data, save, "hexed") => return,
            Tool::Qwk if self.intro_gate(child, data, save, "qwkread") => return,
            Tool::IceDb if self.intro_gate(child, data, save, "icedb") => return,
            Tool::NightComm if self.intro_gate(child, data, save, "nightcomm") => return,
            Tool::NightComm => {
                crate::nightcomm_view::show(
                    child,
                    data,
                    save,
                    &mut self.nightcomm,
                    self.scan.as_ref(),
                    self.dialing.as_ref(),
                    &mut dial,
                    out,
                );
            }
            Tool::TigerTeam if self.splash_gate(child, data, save, "tigerteam", "tigerteam") => {
                return;
            }
            Tool::TigerTeam => {
                crate::tigerteam_view::show(
                    child,
                    data,
                    save,
                    &mut self.tigerteam,
                    self.scan.as_ref(),
                    self.dialing.as_ref(),
                    &mut dial,
                    out,
                );
            }
            Tool::HexEd => hubtools::hexed(child, save, &mut self.hex, out, &mut edit),
            Tool::Qwk => self.qwk_reader(child, data, save),
            Tool::IceDb => hubtools::icedb(child, data, save, &mut self.icedb_sel),
            Tool::Guide => hubtools::guide(child, data, &mut self.guide_at),
            Tool::Shell => {}
        }
        if let Some(id) = dial {
            self.dial_from(tool, id, save);
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

    /// Whether the Shell's output shows a line containing `needle` (tests).
    #[cfg(test)]
    pub(crate) fn shell_has(&self, needle: &str) -> bool {
        self.shell_lines.iter().any(|l| l.contains(needle))
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
            // The scan's result opens where it's shown (`show_scan`).
            ShellEffect::Scan(range) => {
                self.range_sel = Some(range.clone());
                out.push(WbOut::Scan(range, None));
            }
            ShellEffect::Call(id) => out.push(WbOut::Call(id)),
            ShellEffect::Open(t) => self.open(t),
            ShellEffect::System(p) => {
                self.system.panel = p;
                self.open(Tool::System);
            }
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

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::uitest::{Harness, key, text};
    use egui::{Key, Pos2, UiBuilder};
    use ninsei_core::directory::Listed;

    /// A started deck with a phonebook, no splashes to wait through.
    pub(crate) fn deck(data: &Data) -> Save {
        let mut s = Save {
            intros: false,
            guide_seen: true,
            ..Save::default()
        };
        s.ensure_key(0x7E57);
        s.ensure_phonebook(data, 1);
        s
    }

    /// One frame of `tool`'s window, 880 × 500, with these events. Returns what it sent.
    pub(crate) fn frame(
        h: &mut Harness,
        home: &mut Home,
        data: &Data,
        save: &Save,
        tool: Tool,
        events: Vec<egui::Event>,
    ) -> Vec<WbOut> {
        let rect = Rect::from_min_size(Pos2::ZERO, vec2(880.0, 500.0));
        frame_at(h, home, data, save, tool, rect, events)
    }

    /// One frame of `tool` drawn in `rect`, with these events. Returns what it sent.
    pub(crate) fn frame_at(
        h: &mut Harness,
        home: &mut Home,
        data: &Data,
        save: &Save,
        tool: Tool,
        rect: Rect,
        events: Vec<egui::Event>,
    ) -> Vec<WbOut> {
        let mut out = Vec::new();
        h.frame(events, |ui| {
            let mut child = ui.new_child(UiBuilder::new().max_rect(rect));
            home.tool_body(&mut child, tool, data, save, &mut out);
        });
        out
    }

    /// Clicks the widget in `tool`'s window whose text starts with `label`.
    pub(crate) fn click(
        h: &mut Harness,
        home: &mut Home,
        data: &Data,
        save: &Save,
        tool: Tool,
        label: &str,
    ) -> Vec<WbOut> {
        let at = h
            .find(label)
            .unwrap_or_else(|| panic!("no {label:?} on screen: {:?}", h.texts()))
            .center();
        let mut out = frame(h, home, data, save, tool, crate::uitest::click(at));
        out.extend(frame(h, home, data, save, tool, Vec::new()));
        out
    }

    impl Home {
        /// How many pages `tool` has, for a test to draw each.
        fn pages(tool: Tool) -> usize {
            match tool {
                Tool::NightComm => ninsei_core::nightcomm::Tab::ALL.len(),
                Tool::TigerTeam => ninsei_core::tigerteam::Page::ALL.len(),
                Tool::System => ninsei_core::system::Panel::ALL.len(),
                _ => 1,
            }
        }

        /// Turns `tool` to its `page`th page.
        fn turn_to(&mut self, tool: Tool, page: usize) {
            match tool {
                Tool::NightComm => self.nightcomm.tab = ninsei_core::nightcomm::Tab::ALL[page],
                Tool::TigerTeam => self.tigerteam.page = ninsei_core::tigerteam::Page::ALL[page],
                Tool::System => self.system.panel = ninsei_core::system::Panel::ALL[page],
                _ => {}
            }
        }
    }

    /// The rects a tile on AnarchyOS at 1280x800 gives a tool to draw in: alone, as the
    /// master and the stack beside one to three others, with the master at its default,
    /// narrowest and widest share. Each with what it is, for a failing test to say.
    fn tile_sizes() -> Vec<(String, Rect)> {
        use ninsei_core::desktop::Tiling;
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(1280.0, 800.0));
        let area = crate::tiling::work_area(screen);
        let others = [Tool::Terminal, Tool::Qwk, Tool::IceDb];
        let mut out: Vec<(String, Rect)> = Vec::new();
        for n in 0..=others.len() {
            for steps in [0, -100, 100] {
                let mut t = Tiling::default();
                for o in &others[..n] {
                    t.open(*o);
                }
                t.resize_master(steps);
                let tiles = t.layout(
                    [area.left(), area.top(), area.width(), area.height()],
                    crate::tiling::size::GAP,
                );
                for (i, (_, [x, y, w, h])) in tiles.into_iter().enumerate() {
                    let r = crate::tiling::content_of(Rect::from_min_size(pos2(x, y), vec2(w, h)));
                    if !out.iter().any(|(_, o)| o.size() == r.size()) {
                        let what = format!(
                            "tile {i} of {} with the master at {}%",
                            n + 1,
                            t.shape().master
                        );
                        out.push((what, r));
                    }
                }
            }
        }
        out
    }

    /// A deck on AnarchyOS that owns every tool, so each draws its text-mode face.
    fn anarchy_deck(data: &Data) -> Save {
        let mut s = deck(data);
        s.tools = ninsei_core::data::TOOLS
            .iter()
            .map(|t| t.to_string())
            .collect();
        s.decks = vec!["onosendai".into()];
        s.deck = "onosendai".into();
        s.ensure_hardware(data);
        s
    }

    /// Draws `tool`, turned to its `page`th page, through the desktops' one door at `rect` on
    /// a themed screen, a few frames so it settles. Returns the cells it had and whether the
    /// tool (not the notice) was drawn.
    fn draw_at(
        data: &Data,
        save: &Save,
        tool: Tool,
        rect: Rect,
        page: usize,
    ) -> ((usize, usize), bool, Harness) {
        let mut h = Harness::new(1280.0, 800.0);
        crate::uitest::themed(&h.ctx, data);
        let mut home = Home::new(Vec::new());
        home.turn_to(tool, page);
        let (mut have, mut drawn) = ((0, 0), false);
        for _ in 0..3 {
            home.notices.clear();
            h.frame(Vec::new(), |ui| {
                let mut child = ui.new_child(UiBuilder::new().max_rect(rect));
                have = crate::kit::cells::cells_in(&child, rect);
                drawn = home.tool_in(&mut child, tool, data, save, &mut Vec::new(), None, "t");
            });
        }
        assert_eq!(
            home.notices.is_empty(),
            drawn,
            "a notice exactly when not drawn"
        );
        (have, drawn, h)
    }

    #[test]
    fn carrierterm_in_a_narrow_tile_shows_whole_rows() {
        let data = Data::builtin().unwrap();
        let mut save = anarchy_deck(&data);
        save.add_starters(&data);
        let mut h = Harness::new(1280.0, 800.0);
        crate::uitest::themed(&h.ctx, &data);
        let mut home = Home::new(Vec::new());
        // An AnarchyOS stack tile: the card goes under the list.
        let rect = Rect::from_min_size(pos2(10.0, 10.0), vec2(508.0, 700.0));
        for _ in 0..3 {
            frame_at(
                &mut h,
                &mut home,
                &data,
                &save,
                Tool::Terminal,
                rect,
                Vec::new(),
            );
        }
        let footer = h
            .find("7 of 7 numbers")
            .unwrap_or_else(|| panic!("{:?}", h.texts()))
            .top();
        // The box ends above its footer: the item spacing, then the frame's margin.
        let bottom = footer - 4.0 - 2.0;
        let rows = h.all("555-");
        let shown: Vec<_> = rows.iter().filter(|r| r.top() < bottom).collect();
        assert!(shown.len() >= 5, "a list: {:?}", h.texts());
        assert!(
            rows.iter().any(|r| r.top() >= bottom),
            "and more below it: {rows:?}"
        );
        for r in shown {
            assert!(
                r.bottom() <= bottom,
                "a row cut by the box: {r:?}, box ends {bottom}"
            );
        }
    }

    #[test]
    fn every_text_tool_fits_its_tiles_or_says_so() {
        let data = Data::builtin().unwrap();
        let save = anarchy_deck(&data);
        let sizes = tile_sizes();
        assert!(sizes.len() > 8, "a spread of tiles: {sizes:?}");
        for tool in Home::TEXT_TOOLS {
            for page in 0..Home::pages(tool) {
                for (what, rect) in &sizes {
                    let (have, drawn, h) = draw_at(&data, &save, tool, *rect, page);
                    assert_eq!(
                        drawn,
                        fit::fits(Home::fit(tool), have),
                        "{tool:?} page {page} as {what}: {have:?} cells, needs {:?}",
                        Home::fit(tool)
                    );
                    if drawn {
                        assert!(
                            h.outside(*rect).is_empty(),
                            "{tool:?} page {page} as {what} reaches outside: {:?}",
                            h.outside(*rect)
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_notice_shows_exactly_below_each_tools_minimum() {
        let data = Data::builtin().unwrap();
        let save = anarchy_deck(&data);
        let mut cell = vec2(0.0, 0.0);
        let mut h = Harness::new(100.0, 100.0);
        crate::uitest::themed(&h.ctx, &data);
        h.frame(Vec::new(), |ui| cell = crate::kit::cells::cell_size(ui));
        let at = |cols: usize, rows: usize| {
            Rect::from_min_size(
                pos2(10.0, 10.0),
                vec2(cols as f32 * cell.x + 0.5, rows as f32 * cell.y + 0.5),
            )
        };
        for tool in Home::TEXT_TOOLS {
            let needs = Home::fit(tool);
            for (page, need) in
                (0..Home::pages(tool)).flat_map(|p| needs.iter().map(move |n| (p, n)))
            {
                let (have, drawn, _) = draw_at(&data, &save, tool, at(need.cols, need.rows), page);
                assert_eq!(have, (need.cols, need.rows));
                assert!(drawn, "{tool:?} page {page} at exactly {need:?} is drawn");
                // A cell short either way: the notice, unless another shape it declared fits.
                for short in [(need.cols - 1, need.rows), (need.cols, need.rows - 1)] {
                    let drawn = draw_at(&data, &save, tool, at(short.0, short.1), page).1;
                    assert_eq!(drawn, fit::fits(needs, short), "{tool:?} at {short:?}");
                }
            }
        }
    }

    #[test]
    fn carrierterm_takes_the_keys_when_it_opens() {
        let data = Data::builtin().unwrap();
        let save = deck(&data);
        let (mut h, mut home) = (Harness::new(1280.0, 800.0), Home::new(Vec::new()));
        for _ in 0..2 {
            frame(&mut h, &mut home, &data, &save, Tool::Terminal, Vec::new());
        }
        frame(&mut h, &mut home, &data, &save, Tool::Terminal, text("gut"));
        assert_eq!(
            home.directory.query, "gut",
            "typing goes straight into Find"
        );
    }

    #[test]
    fn typing_a_number_picks_it_and_enter_dials_it() {
        let data = Data::builtin().unwrap();
        let save = deck(&data);
        let e = save.phonebook.last().unwrap();
        let (mut h, mut home) = (Harness::new(1280.0, 800.0), Home::new(Vec::new()));
        for events in [Vec::new(), Vec::new(), text(&e.number), Vec::new()] {
            frame(&mut h, &mut home, &data, &save, Tool::Terminal, events);
        }
        assert_eq!(home.directory.sel, Some(Listed::Entry(e.id)));
        frame(
            &mut h,
            &mut home,
            &data,
            &save,
            Tool::Terminal,
            key(Key::Enter),
        );
        assert_eq!(home.dialing.map(|d| d.entry), Some(e.id), "Enter dials it");
    }

    #[test]
    fn arrows_step_through_the_list() {
        let data = Data::builtin().unwrap();
        let save = deck(&data);
        let (mut h, mut home) = (Harness::new(1280.0, 800.0), Home::new(Vec::new()));
        for _ in 0..2 {
            frame(&mut h, &mut home, &data, &save, Tool::Terminal, Vec::new());
        }
        let first = home.directory.sel.expect("the first line is picked");
        frame(
            &mut h,
            &mut home,
            &data,
            &save,
            Tool::Terminal,
            key(Key::ArrowDown),
        );
        let second = home.directory.sel.unwrap();
        assert_ne!(first, second, "down moves on");
        frame(
            &mut h,
            &mut home,
            &data,
            &save,
            Tool::Terminal,
            key(Key::ArrowUp),
        );
        assert_eq!(home.directory.sel, Some(first), "up comes back");
    }

    #[test]
    fn the_cards_big_button_dials_the_line() {
        let data = Data::builtin().unwrap();
        let save = deck(&data);
        let e = save.phonebook.last().unwrap();
        let (mut h, mut home) = (Harness::new(1280.0, 800.0), Home::new(Vec::new()));
        home.directory.sel = Some(Listed::Entry(e.id));
        for _ in 0..2 {
            frame(&mut h, &mut home, &data, &save, Tool::Terminal, Vec::new());
        }
        let label = format!("Dial {}", e.number);
        click(&mut h, &mut home, &data, &save, Tool::Terminal, &label);
        assert_eq!(home.dialing.map(|d| d.entry), Some(e.id));
    }

    #[test]
    fn hang_up_ends_the_call_and_cuts_the_modem() {
        let data = Data::builtin().unwrap();
        let save = deck(&data);
        let (mut h, mut home) = (Harness::new(1280.0, 800.0), Home::new(Vec::new()));
        home.start_dial(save.phonebook[0].id, None);
        frame(&mut h, &mut home, &data, &save, Tool::Terminal, Vec::new());
        click(&mut h, &mut home, &data, &save, Tool::Terminal, "Hang up");
        assert!(home.dialing.is_none(), "the call is over");
        assert!(home.dial_cut, "and the modem sound with it");
    }

    #[test]
    fn dialing_from_nightcomm_plays_on_term_not_in_carrierterm() {
        let data = Data::builtin().unwrap();
        let mut save = deck(&data);
        save.tools = vec!["nightcomm".into()];
        let (mut h, mut home) = (Harness::new(1280.0, 800.0), Home::new(Vec::new()));
        // Open it (it takes the keys), then `d` dials the picked number.
        for events in [Vec::new(), Vec::new(), text("d"), Vec::new()] {
            frame(&mut h, &mut home, &data, &save, Tool::NightComm, events);
        }
        let d = home.dialing.expect("d dials the picked number");
        assert_eq!(
            home.nightcomm.tab,
            ninsei_core::nightcomm::Tab::Term,
            "the call plays on TERM"
        );
        let number = &save.entry(d.entry).unwrap().number;
        assert!(
            home.nightcomm
                .term_lines
                .iter()
                .any(|l| l == &format!("DIALING {number}"))
        );
        assert!(
            !home.take_opens().contains(&Tool::Terminal),
            "CarrierTerm stays closed"
        );
    }

    #[test]
    fn engaging_in_tiger_team_plays_on_the_console_not_in_carrierterm() {
        let data = Data::builtin().unwrap();
        let mut save = deck(&data);
        save.tools = vec!["tigerteam".into()];
        let (mut h, mut home) = (Harness::new(1280.0, 800.0), Home::new(Vec::new()));
        // Open it, pick a target on Assessment, then Enter engages it.
        let steps = [
            Vec::new(),
            Vec::new(),
            key(Key::ArrowDown),
            key(Key::Enter),
            Vec::new(),
        ];
        for events in steps {
            frame(&mut h, &mut home, &data, &save, Tool::TigerTeam, events);
        }
        let d = home.dialing.expect("Enter engages the picked target");
        assert_eq!(
            home.tigerteam.page,
            ninsei_core::tigerteam::Page::Console,
            "the call plays on the Console"
        );
        let number = &save.entry(d.entry).unwrap().number;
        assert!(
            home.tigerteam
                .nc
                .term_lines
                .iter()
                .any(|l| l == &format!("DIALING {number}"))
        );
        assert_eq!(
            home.nightcomm.term_lines,
            crate::nightcomm_view::NightComm::default().term_lines,
            "NiGHTCOMM's own TERM hears nothing of it"
        );
        assert!(
            !home.take_opens().contains(&Tool::Terminal),
            "CarrierTerm stays closed"
        );
    }

    #[test]
    fn dialing_from_carrierterm_still_plays_there() {
        let data = Data::builtin().unwrap();
        let save = deck(&data);
        let mut home = Home::new(Vec::new());
        let id = save.phonebook[0].id;
        home.dial_from(Tool::Terminal, id, &save);
        assert!(home.dialing.is_some());
        assert!(home.take_opens().contains(&Tool::Terminal));
    }
}
