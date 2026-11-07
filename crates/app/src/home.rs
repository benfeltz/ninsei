//! Home: what every desktop shares. The desktop palette, what home asks the app to do
//! ([`WbOut`]), the Shell, CarrierTerm's dialing and scans, the HexEd buffer, and the code that
//! draws each tool's contents. A desktop (Workbench, tiling, ...) is only a front-end on this:
//! it implements [`DesktopView`] and lives in the registry ([`Desktops`]).

use egui::{Align2, Color32, FontFamily, FontId, Id, Pos2, Rect, Ui, Vec2, pos2, vec2};
use ninsei_core::Data;
use ninsei_core::binary::Bin;
use ninsei_core::data::ThemeDef;
use ninsei_core::hardware::Desktop;
use ninsei_core::hexedit::HexBuffer;
use ninsei_core::hub::{self, GameCopy, Save, ScanResult, ShellEffect, Tool};
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
    /// A round of this game (its id) started: count it, so the next deals anew.
    GameStarted(String),
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
    /// Whether its tools may take the keyboard: false while a window over the desktop (a
    /// board called from home) owns it, even though the desktop still takes clicks.
    pub keys: bool,
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

    /// Whether any desktop has `tool` open: a window on Liza or Desktop, a tile on any of
    /// AnarchyOS's workspaces (kept on the save). Every desktop's windows outlive a switch of
    /// firmware, so a game open on one stays open while another is on screen.
    pub fn is_open(&self, save: &Save, tool: Tool) -> bool {
        self.liza.has(tool)
            || self.workbench.has(tool)
            || save.tiling.workspaces.iter().flatten().any(|t| *t == tool)
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
    /// Whether the desktop's tools may take the keyboard this frame (`Cx::keys`): false
    /// while a board called from home owns it.
    pub(crate) desk_keys: bool,
    /// The games whose windows are open, by copy (the desktops say which, [`Home::keep_open`]):
    /// its intro while it plays, then the round being played.
    games: std::collections::HashMap<Tool, GameWin>,
    /// The last round dealt of each game (by id), so two rounds started before the save
    /// counts the first (both copies opening in one frame) still deal two games.
    dealt: std::collections::HashMap<String, u64>,
}

/// A game's open window: its intro (the time it started) until it ends, then the round.
struct GameWin {
    /// The intro's start and end, in the frames' time.
    intro: Option<(f64, f64)>,
    session: Option<crate::games_view::Session>,
}

/// How an intro's frame went.
enum IntroFrame {
    /// Still on screen: draw nothing else.
    Showing,
    /// Over, run to its end or skipped this frame.
    Ended { skipped: bool },
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
            desk_keys: true,
            games: Default::default(),
            dealt: Default::default(),
        }
    }

    /// The tools whose windows (or tiles) the desktop has open, every frame before it draws:
    /// a game whose window is no longer among them is over, its intro's tune cut. Hidden, too
    /// small for a frame, on another workspace: still open, and it waits where it was.
    pub fn keep_open(&mut self, now: f64, open: impl Fn(Tool) -> bool) {
        if self.games.is_empty() {
            return;
        }
        let mut cut = false;
        self.games.retain(|t, w| {
            let keep = open(*t);
            // Only an intro still playing has a tune to cut (one not drawn since it ran out
            // must not cut another window's).
            cut |= !keep && w.intro.is_some_and(|(_, end)| now < end);
            keep
        });
        if cut {
            self.tune = Some(TuneCue::Stop);
        }
    }

    /// A copy of a game in its window. Opened: its copy's intro (the crack's cracktro, the
    /// box's splash), skippable, unless intros are off. Then a round, and another when the
    /// player asks once it's over; the engine played and drawn.
    #[allow(clippy::too_many_arguments)]
    fn play(
        &mut self,
        ui: &mut Ui,
        data: &Data,
        save: &Save,
        tool: Tool,
        copy: GameCopy,
        keys: bool,
        out: &mut Vec<WbOut>,
    ) {
        let Some((def, _)) = tool.game_def(data) else {
            return;
        };
        let intro = data.intro(match copy {
            GameCopy::Cracked => &def.crack,
            GameCopy::Boxed => &def.splash,
        });
        if !self.games.contains_key(&tool) {
            let now = ui.input(|i| i.time);
            let intro = intro.filter(|_| save.intros);
            if let Some(i) = intro {
                self.tune = Some(TuneCue::Play(i.id.clone()));
            }
            let w = GameWin {
                intro: intro.map(|i| (now, now + f64::from(i.secs))),
                session: None,
            };
            self.games.insert(tool, w);
        }
        let w = self.games.get_mut(&tool).expect("opened above");
        if let (Some((start, _)), Some(i)) = (w.intro, intro) {
            match intro_frame(ui, data, save, i, start, SkipBy::Keyboard(keys)) {
                IntroFrame::Showing => return,
                // Skipped: the game starts next frame, so the key that skipped it isn't the
                // game's.
                IntroFrame::Ended { skipped: true } => {
                    w.intro = None;
                    self.tune = Some(TuneCue::Stop);
                    ui.ctx().request_repaint();
                    return;
                }
                // Run to its end: the game starts on this frame.
                IntroFrame::Ended { skipped: false } => w.intro = None,
            }
        }
        let dealt = &mut self.dealt;
        let mut start = |out: &mut Vec<WbOut>| {
            let next = dealt.get(&def.id).map_or(0, |r| r + 1);
            let round = save.game_round(&def.id).max(next);
            dealt.insert(def.id.clone(), round);
            out.push(WbOut::GameStarted(def.id.clone()));
            crate::games_view::Session::new(def, save.key, round)
        };
        let s = w.session.get_or_insert_with(|| start(out));
        if crate::games_view::body(ui, data, def, s, keys).again {
            *s = start(out);
        }
    }

    /// The round being played in `tool`'s window (tests).
    #[cfg(test)]
    pub(crate) fn round(&self, tool: Tool) -> Option<u64> {
        self.games.get(&tool)?.session.as_ref().map(|s| s.round)
    }

    /// The game being played in `tool`'s window (tests).
    #[cfg(test)]
    pub(crate) fn game(&self, tool: Tool) -> Option<&dyn ninsei_core::games::Arcade> {
        self.games
            .get(&tool)
            .and_then(|w| w.session.as_ref())
            .map(|s| s.game.as_ref())
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
        match intro_frame(ui, data, save, def, start, SkipBy::Pointer(self.desk_keys)) {
            IntroFrame::Showing => true,
            IntroFrame::Ended { skipped } => {
                self.intro_playing.remove(id);
                if skipped {
                    self.tune = Some(TuneCue::Stop);
                }
                false
            }
        }
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
    #[allow(clippy::too_many_arguments)]
    pub fn show(
        &mut self,
        view: &mut dyn DesktopView,
        ui: &mut Ui,
        data: &Data,
        save: &Save,
        offset: f32,
        live: bool,
        keys: bool,
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
            keys: live && keys,
        };
        self.desk_keys = cx.keys;
        view.show(ui, self, &cx, &mut out);
        out
    }

    #[cfg(test)]
    /// The text-mode tools: the ones laid out in cells, which the fit test draws at every size
    /// the tiler gives.
    pub const TEXT_TOOLS: [Tool; 3] = [Tool::NightComm, Tool::TigerTeam, Tool::System];

    /// What each tool's face on `desktop` needs to be drawn at all: the one place a tool
    /// says so. Faces that scroll (the GUI ones) need nothing: a small window scrolls them.
    pub fn fit(tool: Tool, desktop: Desktop, data: &Data) -> Option<fit::Need> {
        match (tool, desktop) {
            // A game's field and its status line, in cells, on every desktop.
            (Tool::Game(..), _) => tool
                .game_def(data)
                .map(|(g, _)| fit::Need::cells_owned(crate::games_view::grids(g))),
            // Laid out in cells on every desktop: the list's header and a few rows, and
            // every page's columns.
            (Tool::NightComm | Tool::TigerTeam, _) => {
                Some(fit::Need::cells(&[Fit { cols: 56, rows: 12 }]))
            }
            // AnarchyOS's System is text mode, in its own grid.
            (Tool::System, Desktop::Tiling) => Some(crate::system_tui::NEED),
            _ => None,
        }
    }

    /// Draws `tool` in `ui` (a desktop's window or tile, dressed by `hubtools::wb_visuals`)
    /// unless its face needs more room than `ui` has ([`Home::fit`]); then the one notice in
    /// its place, with the desktop's `hint`. A tool taller than `ui` scrolls with the desktop's
    /// `scroller`, or egui's bar in the theme's colours. `keys`: the tool has the keyboard
    /// (the front window, the focused tile, and nothing over the desktop owns it). Returns
    /// whether the tool was drawn.
    #[allow(clippy::too_many_arguments)]
    pub fn tool_in(
        &mut self,
        ui: &mut Ui,
        tool: Tool,
        keys: bool,
        data: &Data,
        save: &Save,
        out: &mut Vec<WbOut>,
        hint: Option<&str>,
        scroller: Option<&Scroller>,
        salt: impl std::hash::Hash + std::fmt::Debug,
    ) -> bool {
        // The room the face gets: the scroller's view (in from its pads, beside its bar), or the
        // whole rect less egui's bar when the face scrolls.
        let (room, bar) = match scroller {
            Some(s) => (
                crate::kit::scrollbar::view_rect(ui.max_rect(), s.bar_w, s.pad),
                0.0,
            ),
            None => (ui.max_rect(), scroll_bar()),
        };
        if tool == Tool::System {
            self.system.keys = keys;
            self.system.blocked = !self.desk_keys;
        }
        let desktop = save.firmware_def(data).desktop;
        let need = Self::fit(tool, desktop, data);
        if let Some(need) = need
            && let (have, false) = fit::check(ui, room, &need, bar)
        {
            #[cfg(test)]
            self.notices.push(tool);
            fit::notice(
                ui,
                room,
                &tool.name(data),
                &need.grids,
                have,
                hint,
                (backdrop(), ink(), muted()),
            );
            return false;
        }
        match scroller {
            Some(s) => (s.show)(ui, Id::new(salt), &mut |ui| {
                self.tool_body(ui, tool, keys, data, save, out)
            }),
            None => scrolled(ui, salt, |ui| {
                self.tool_body(ui, tool, keys, data, save, out)
            }),
        }
        true
    }

    /// Draws a tool's contents (everything but the Shell) into `child`. Only through
    /// [`Home::tool_in`], which knows the room is enough.
    fn tool_body(
        &mut self,
        child: &mut Ui,
        tool: Tool,
        keys: bool,
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
            Tool::Game(_, copy) => self.play(child, data, save, tool, copy, keys, out),
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

/// What may skip an intro.
#[derive(Clone, Copy)]
enum SkipBy {
    /// A game's window: a key or a click, only while it has the keyboard (it's the front
    /// window or the focused tile), so nothing meant for a window over it skips it.
    Keyboard(bool),
    /// A tool's window: a click on it, or a key with the pointer over it, unless something
    /// over the desktop owns the keyboard (false).
    Pointer(bool),
}

/// One frame of intro `def`, started at `start`: painted in the desk's colours, until it runs
/// to its end or input `by` allows skips it.
fn intro_frame(
    ui: &mut Ui,
    data: &Data,
    save: &Save,
    def: &ninsei_core::intro::IntroDef,
    start: f64,
    by: SkipBy,
) -> IntroFrame {
    let now = ui.input(|i| i.time);
    let over = |p: egui::Pos2| ui.clip_rect().contains(p);
    let skip = ui.input(|i| {
        let key = i.events.iter().any(|e| {
            matches!(
                e,
                egui::Event::Key { pressed: true, .. } | egui::Event::Text(_)
            )
        });
        let click = i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(over);
        match by {
            SkipBy::Keyboard(keys) => keys && (key || click),
            SkipBy::Pointer(free) => {
                free && (click || key && i.pointer.hover_pos().is_some_and(over))
            }
        }
    });
    let t = (now - start) as f32;
    if skip || def.done(t) {
        return IntroFrame::Ended { skipped: skip };
    }
    let calm = save.glitches != ninsei_core::glitch::Glitches::On;
    let lines = def.screen(data, t, calm);
    crate::bbs_view::paint_screen_fit(ui, &lines, false, &desk_palette());
    ui.ctx().request_repaint();
    IntroFrame::Showing
}

/// Where a game's window opens on a desktop of windows: the crack's at `origin`, the box's
/// down and to the right of it, so both copies show when both are open.
pub(crate) fn game_spot(copy: GameCopy, origin: Pos2) -> Pos2 {
    match copy {
        GameCopy::Cracked => origin,
        GameCopy::Boxed => origin + vec2(50.0, 30.0),
    }
}

/// A window of games' size, worked out the first time it's drawn (zero until then: the room
/// it needs is counted in `ui`'s cells). The desktop's window puts its tool in
/// `scrolled(rect)`, beside a scroll bar `bar_w` wide with `pad` around it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn size_game_window(
    size: &mut Vec2,
    ui: &Ui,
    data: &Data,
    save: &Save,
    tool: Tool,
    scrolled: fn(Rect) -> Rect,
    bar_w: f32,
    pad: crate::kit::scrollbar::Pad,
) {
    if *size != Vec2::ZERO {
        return;
    }
    let view = |r: Rect| crate::kit::scrollbar::view_rect(scrolled(r), bar_w, pad);
    *size = game_window(ui, data, save, tool, view).unwrap_or(vec2(400.0, 300.0));
}

/// How big a window must open for a game: the room its first (square-ish) grid needs in the
/// kit's cells, or its intro's 80x24 screen if that's bigger and it plays one (intros on, and
/// the copy has one), plus what the window's frame takes around its `view` (the rect its
/// tool is drawn in, from the window's rect). `None` for anything but a game.
pub(crate) fn game_window(
    ui: &Ui,
    data: &Data,
    save: &Save,
    tool: Tool,
    view: impl Fn(Rect) -> Rect,
) -> Option<Vec2> {
    let (def, copy) = tool.game_def(data)?;
    let grid = crate::games_view::grids(def)[0];
    let cell = crate::kit::Cells::cell_size(ui.ctx());
    let field = vec2(grid.cols as f32 * cell.x, grid.rows as f32 * cell.y);
    let intro = match copy {
        GameCopy::Cracked => &def.crack,
        GameCopy::Boxed => &def.splash,
    };
    let room = if save.intros && data.intro(intro).is_some() {
        field.max(crate::bbs_view::screen_size(ui))
    } else {
        field
    };
    let probe = Rect::from_min_size(Pos2::ZERO, vec2(2000.0, 2000.0));
    let frame = probe.size() - view(probe).size();
    // A point of slack either way, so rounding never costs a cell.
    Some((room + frame + vec2(2.0, 2.0)).ceil())
}

/// The "too small" hint on a desktop of windows (Liza, Desktop): a window dragged low is cut
/// short by the screen's bottom edge, so dragging it higher gives it room, unless its top is
/// already as `highest` as it goes.
pub(crate) fn drag_hint(top: f32, highest: f32) -> Option<&'static str> {
    (top > highest + 0.5).then_some("Drag the window higher for more room.")
}

/// A dim line for an empty tile or slot.
/// The room `scrolled`'s bar takes beside a face that scrolls.
pub(crate) fn scroll_bar() -> f32 {
    crate::theme::visible_scroll().allocated_width()
}

/// A desktop's own scroll bar for a tool taller than its window: the bar's width and the view's
/// pads (so the fit check measures the room the face really gets,
/// `kit::scrollbar::view_rect`), and the view that scrolls it (given an id and the body).
pub(crate) struct Scroller<'a> {
    pub bar_w: f32,
    pub pad: crate::kit::scrollbar::Pad,
    pub show: &'a ScrollView<'a>,
}

/// Draws a body (the last argument) in a view that scrolls, keyed by the id.
pub(crate) type ScrollView<'a> = dyn Fn(&mut Ui, Id, &mut dyn FnMut(&mut Ui)) + 'a;

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
    use ninsei_core::hub::GameCopy;

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

    /// One frame of `tool` drawn in `rect` through the desktops' one door (so never at a size
    /// its face can't take: a test that tries fails here), with these events. Returns what
    /// it sent.
    pub(crate) fn frame_at(
        h: &mut Harness,
        home: &mut Home,
        data: &Data,
        save: &Save,
        tool: Tool,
        rect: Rect,
        events: Vec<egui::Event>,
    ) -> Vec<WbOut> {
        frame_keys(h, home, data, save, tool, rect, false, events)
    }

    /// [`frame_at`], saying whether the tool has the keyboard.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn frame_keys(
        h: &mut Harness,
        home: &mut Home,
        data: &Data,
        save: &Save,
        tool: Tool,
        rect: Rect,
        keys: bool,
        events: Vec<egui::Event>,
    ) -> Vec<WbOut> {
        let mut out = Vec::new();
        h.frame(events, |ui| {
            let mut child = ui.new_child(UiBuilder::new().max_rect(rect));
            let drawn = home.tool_in(
                &mut child, tool, keys, data, save, &mut out, None, None, "test",
            );
            assert!(drawn, "{tool:?} drawn at {rect:?}, which it can't take");
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
                    crate::kit::anarchy::size::GAP,
                );
                for (i, (_, [x, y, w, h])) in tiles.into_iter().enumerate() {
                    let r = crate::kit::anarchy::tile_content(Rect::from_min_size(
                        pos2(x, y),
                        vec2(w, h),
                    ));
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

    /// The faces laid out in cells: the text-mode tools, and every copy of every game.
    fn text_tools(data: &Data) -> Vec<Tool> {
        let games = data.games.iter().flat_map(|g| {
            [GameCopy::Cracked, GameCopy::Boxed].map(|c| Tool::game(&g.id, c).unwrap())
        });
        Home::TEXT_TOOLS.into_iter().chain(games).collect()
    }

    /// A deck on AnarchyOS that owns every tool and both copies of every game, so each draws
    /// its text-mode face.
    fn anarchy_deck(data: &Data) -> Save {
        let mut s = deck(data);
        s.tools = ninsei_core::data::TOOLS
            .iter()
            .map(|t| t.to_string())
            .collect();
        for g in &data.games {
            s.learn_game(&g.id, GameCopy::Cracked);
            s.learn_game(&g.id, GameCopy::Boxed);
        }
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
                have = Home::fit(tool, Desktop::Tiling, data).map_or((0, 0), |need| {
                    fit::check(&child, rect, &need, scroll_bar()).0
                });
                drawn = home.tool_in(
                    &mut child,
                    tool,
                    false,
                    data,
                    save,
                    &mut Vec::new(),
                    None,
                    None,
                    "t",
                );
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
    fn carrierterm_in_a_narrow_tile_shows_whole_rows_wherever_it_scrolls() {
        let data = Data::builtin().unwrap();
        let mut save = anarchy_deck(&data);
        save.add_starters(&data);
        let mut rng = ninsei_core::rng::Rng::new(7);
        while save.phonebook.len() < 14 {
            save.add_number(&data, &mut rng, 1)
                .expect("a company to list");
        }
        let mut h = Harness::new(1280.0, 800.0);
        crate::uitest::themed(&h.ctx, &data);
        let mut home = Home::new(Vec::new());
        // An AnarchyOS stack tile: the card goes under the list.
        let rect = Rect::from_min_size(pos2(10.0, 10.0), vec2(508.0, 700.0));
        let run = |h: &mut Harness, home: &mut Home, events| {
            frame_at(h, home, &data, &save, Tool::Terminal, rect, events);
            frame_at(h, home, &data, &save, Tool::Terminal, rect, Vec::new());
        };
        run(&mut h, &mut home, Vec::new());
        let total = format!("{} of {} numbers", 15, 15);
        let check = |h: &Harness, what: &str| {
            let head = h.find("NUMBER").expect("the list's header").bottom();
            let footer = h
                .find(&total)
                .unwrap_or_else(|| panic!("{what}: {:?}", h.texts()))
                .top();
            let rows: Vec<Rect> = h
                .all("555-")
                .into_iter()
                .filter(|r| r.width() > 400.0)
                .collect();
            assert!(rows.len() >= 5, "{what}: a list: {:?}", h.texts());
            assert!(rows.len() < 15, "{what}: a window on it");
            for r in rows {
                assert!(
                    r.top() >= head && r.bottom() <= footer - 6.0,
                    "{what}: a row cut by the box: {r:?}, box {head}..{}",
                    footer - 6.0
                );
            }
        };
        check(&h, "at the top");
        for _ in 0..9 {
            run(&mut h, &mut home, crate::uitest::key(Key::ArrowDown));
        }
        check(&h, "arrowed down");
        assert!(
            h.find(&format!("{total}  ·  10/15")).is_some(),
            "the footer says where: {:?}",
            h.texts()
        );
        let wheel = vec![
            egui::Event::PointerMoved(pos2(200.0, 150.0)),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: vec2(0.0, 33.0),
                modifiers: egui::Modifiers::NONE,
                phase: egui::TouchPhase::Move,
            },
        ];
        let before = home.directory.list.top;
        assert!(before > 0, "scrolled to follow the pick");
        // The wheel over the card under the list is the card's, not the list's.
        let card = h.find("DIRECTORY CARD").expect("the card").center();
        let over_card = vec![
            egui::Event::PointerMoved(card),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: vec2(0.0, 3.0),
                modifiers: egui::Modifiers::NONE,
                phase: egui::TouchPhase::Move,
            },
        ];
        run(&mut h, &mut home, over_card);
        assert_eq!(
            home.directory.list.top, before,
            "the card's wheel left the list alone"
        );
        run(&mut h, &mut home, wheel);
        check(&h, "wheeled up");
        assert!(home.directory.list.top < before, "the wheel scrolled it up");
    }

    #[test]
    fn a_search_brings_carrierterms_pick_into_view() {
        let data = Data::builtin().unwrap();
        let mut save = anarchy_deck(&data);
        save.add_starters(&data);
        let mut rng = ninsei_core::rng::Rng::new(7);
        while save.phonebook.len() < 14 {
            save.add_number(&data, &mut rng, 1)
                .expect("a company to list");
        }
        let mut h = Harness::new(1280.0, 800.0);
        crate::uitest::themed(&h.ctx, &data);
        let mut home = Home::new(Vec::new());
        let rect = Rect::from_min_size(pos2(10.0, 10.0), vec2(508.0, 700.0));
        let run = |h: &mut Harness, home: &mut Home, events| {
            frame_at(h, home, &data, &save, Tool::Terminal, rect, events);
            frame_at(h, home, &data, &save, Tool::Terminal, rect, Vec::new());
        };
        run(&mut h, &mut home, Vec::new());
        for _ in 0..11 {
            run(&mut h, &mut home, crate::uitest::key(Key::ArrowDown));
        }
        assert!(home.directory.list.top > 0, "scrolled down");
        // A search that drops the pick but keeps more numbers than the box shows: the pick
        // goes back to the first, and the window must go with it.
        let picked = home.directory.sel;
        let query = ('a'..='z')
            .map(|c| c.to_string())
            .find(|q| {
                let shown = ninsei_core::directory::search(&save, &data, q, home.directory.filter);
                shown.len() >= 8 && !shown.iter().any(|l| Some(l.key) == picked)
            })
            .expect("a letter most names have and the pick hasn't");
        run(&mut h, &mut home, text(&query));
        let shown = ninsei_core::directory::search(
            &save,
            &data,
            &home.directory.query,
            home.directory.filter,
        );
        let at = shown
            .iter()
            .position(|l| Some(l.key) == home.directory.sel)
            .expect("a pick");
        assert!(
            home.directory.list.shows(at),
            "the pick ({at}) is in the window from {}",
            home.directory.list.top
        );
    }

    #[test]
    fn a_list_box_takes_the_wheel_over_its_rows_and_leaves_none_for_the_tool() {
        let data = Data::builtin().unwrap();
        let mut h = Harness::new(600.0, 400.0);
        crate::uitest::themed(&h.ctx, &data);
        let mut list = crate::kit::List::default();
        // One frame; returns the wheel left over once the list has drawn.
        let frame = |h: &mut Harness, list: &mut crate::kit::List, events, wheel_on| {
            let mut left = 0.0;
            h.frame(events, |ui| {
                hubtools::list_box(ui, 200.0, 40, list, None, wheel_on, |ui, i| {
                    ui.label(format!("row {i}"));
                });
                left = ui.input(|i| i.smooth_scroll_delta.y);
            });
            left
        };
        let over = || vec![egui::Event::PointerMoved(pos2(100.0, 50.0))];
        frame(&mut h, &mut list, over(), true);
        let wheel = vec![egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: vec2(0.0, -3.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
        }];
        let mut left = frame(&mut h, &mut list, wheel.clone(), true);
        for _ in 0..10 {
            assert_eq!(left, 0.0, "the list left some wheel for the tool");
            left = frame(&mut h, &mut list, Vec::new(), true);
        }
        assert!(list.top > 0, "and scrolled itself");
        // A read-only list (the dialing sidebar) leaves the wheel to the tool.
        let mut still = crate::kit::List::default();
        frame(&mut h, &mut still, over(), false);
        let left = frame(&mut h, &mut still, wheel, false);
        assert!(left != 0.0, "the wheel is the tool's");
        assert_eq!(still.top, 0);
    }

    /// Widgets `rect` would cut: anything outside it for a face laid out to fill it, and
    /// anything past its sides for a face whose height scrolls.
    fn cut(h: &Harness, rect: Rect, need: &fit::Need) -> Vec<(String, Rect)> {
        h.outside(rect)
            .into_iter()
            .filter(|(_, r)| {
                !need.scrolls || r.left() < rect.left() - 0.5 || r.right() > rect.right() + 0.5
            })
            .collect()
    }

    #[test]
    fn a_list_box_made_n_rows_tall_shows_n_rows() {
        for pitch in [21.3_f32, 22.0, 19.7, 23.1, 20.9] {
            let gap = 4.0;
            for n in 1..40 {
                let height = n as f32 * pitch - gap;
                assert_eq!(
                    hubtools::rows_in(height, pitch, gap),
                    n,
                    "{n} rows of {pitch}"
                );
            }
        }
    }

    #[test]
    fn a_window_is_told_to_go_higher_only_when_it_can() {
        assert!(drag_hint(300.0, 20.0).is_some());
        assert_eq!(drag_hint(20.0, 20.0), None, "already at the top");
    }

    #[test]
    fn every_text_tool_fits_its_tiles_or_says_so() {
        let data = Data::builtin().unwrap();
        let save = anarchy_deck(&data);
        let sizes = tile_sizes();
        assert!(sizes.len() > 8, "a spread of tiles: {sizes:?}");
        for tool in text_tools(&data) {
            let need =
                Home::fit(tool, Desktop::Tiling, &data).expect("a text face declares its need");
            for page in 0..Home::pages(tool) {
                for (what, rect) in &sizes {
                    let (have, drawn, h) = draw_at(&data, &save, tool, *rect, page);
                    assert_eq!(
                        drawn,
                        fit::fits(&need.grids, have),
                        "{tool:?} page {page} as {what}: {have:?} cells, needs {:?}",
                        need.grids
                    );
                    if drawn {
                        assert!(
                            cut(&h, *rect, &need).is_empty(),
                            "{tool:?} page {page} as {what} is cut: {:?}",
                            cut(&h, *rect, &need)
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
        let mut h = Harness::new(100.0, 100.0);
        crate::uitest::themed(&h.ctx, &data);
        for tool in text_tools(&data) {
            let need = Home::fit(tool, Desktop::Tiling, &data).unwrap();
            // The smallest rect holding `cols` x `rows` of the face's own cells, counted the
            // way the desktops count them (beside the scroll bar, for a face that scrolls).
            let mut at = |cols: usize, rows: usize| {
                let mut size = vec2(0.0, 2000.0);
                h.frame(Vec::new(), |ui| {
                    let have = |s| {
                        fit::check(
                            ui,
                            Rect::from_min_size(pos2(10.0, 10.0), s),
                            &need,
                            scroll_bar(),
                        )
                        .0
                    };
                    while have(size).0 < cols {
                        size.x += 0.25;
                    }
                    size.y = 0.0;
                    while have(size).1 < rows {
                        size.y += 0.25;
                    }
                });
                Rect::from_min_size(pos2(10.0, 10.0), size)
            };
            for page in 0..Home::pages(tool) {
                for grid in need.grids.iter() {
                    let exact = at(grid.cols, grid.rows);
                    let (have, drawn, h) = draw_at(&data, &save, tool, exact, page);
                    assert_eq!(have, (grid.cols, grid.rows));
                    assert!(drawn, "{tool:?} page {page} at exactly {grid:?} is drawn");
                    assert!(
                        cut(&h, exact, &need).is_empty(),
                        "{tool:?} page {page} at exactly {grid:?} is cut: {:?}",
                        cut(&h, exact, &need)
                    );
                    // A cell short either way: the notice, unless another grid it declared fits.
                    for short in [(grid.cols - 1, grid.rows), (grid.cols, grid.rows - 1)] {
                        let drawn = draw_at(&data, &save, tool, at(short.0, short.1), page).1;
                        assert_eq!(
                            drawn,
                            fit::fits(&need.grids, short),
                            "{tool:?} at {short:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn gui_faces_declare_nothing_and_scroll() {
        let data = Data::builtin().unwrap();
        for tool in Tool::ALL {
            for desktop in [Desktop::Liza, Desktop::Workbench] {
                let text = matches!(tool, Tool::NightComm | Tool::TigerTeam);
                assert_eq!(
                    Home::fit(tool, desktop, &data).is_some(),
                    text,
                    "{tool:?} on {desktop:?}"
                );
            }
        }
        for tool in [
            Tool::Terminal,
            Tool::Rig,
            Tool::HexEd,
            Tool::Qwk,
            Tool::IceDb,
            Tool::Guide,
        ] {
            assert!(
                Home::fit(tool, Desktop::Tiling, &data).is_none(),
                "{tool:?} scrolls in a tile"
            );
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

    #[test]
    fn under_a_board_called_from_home_system_takes_no_keys_even_with_the_pointer_on_it() {
        let data = Data::builtin().unwrap();
        let save = anarchy_deck(&data);
        let rect = Rect::from_min_size(Pos2::ZERO, vec2(880.0, 500.0));
        let over = egui::Event::PointerMoved(pos2(400.0, 250.0));
        for (free, want) in [
            (false, ninsei_core::system::Panel::ALL[0]),
            (true, ninsei_core::system::Panel::ALL[1]),
        ] {
            let mut h = Harness::new(1280.0, 800.0);
            let mut home = Home::new(Vec::new());
            home.desk_keys = free;
            frame_keys(
                &mut h,
                &mut home,
                &data,
                &save,
                Tool::System,
                rect,
                false,
                vec![over.clone()],
            );
            let mut two = vec![over.clone()];
            two.extend(key(Key::Num2));
            frame_keys(
                &mut h,
                &mut home,
                &data,
                &save,
                Tool::System,
                rect,
                false,
                two,
            );
            frame_keys(
                &mut h,
                &mut home,
                &data,
                &save,
                Tool::System,
                rect,
                false,
                vec![over.clone()],
            );
            assert_eq!(home.system.panel, want, "keys free: {free}");
        }
    }

    #[test]
    fn under_a_board_called_from_home_a_key_never_skips_a_tools_intro() {
        let data = Data::builtin().unwrap();
        let save = Save {
            intros: true,
            ..anarchy_deck(&data)
        };
        let rect = Rect::from_min_size(Pos2::ZERO, vec2(880.0, 500.0));
        let over = egui::Event::PointerMoved(pos2(400.0, 250.0));
        for (free, skipped) in [(false, false), (true, true)] {
            let mut h = Harness::new(1280.0, 800.0);
            let mut home = Home::new(Vec::new());
            home.desk_keys = free;
            frame_at(
                &mut h,
                &mut home,
                &data,
                &save,
                Tool::HexEd,
                rect,
                vec![over.clone()],
            );
            assert!(
                home.intro_playing.contains_key("hexed"),
                "the cracktro is up"
            );
            let mut k = vec![over.clone()];
            k.extend(key(Key::Space));
            frame_at(&mut h, &mut home, &data, &save, Tool::HexEd, rect, k);
            assert_eq!(
                !home.intro_playing.contains_key("hexed"),
                skipped,
                "keys free: {free}"
            );
        }
    }
}
