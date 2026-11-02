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

pub(crate) const FONT: f32 = 14.0;
const SHELL_KEEP: usize = 200;

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
}

impl Desktops {
    pub fn new() -> Self {
        Self {
            workbench: crate::workbench::Workbench::new(),
            tiling: crate::tiling::TilingDesk::new(),
        }
    }

    pub fn get(&mut self, d: Desktop) -> &mut dyn DesktopView {
        match d {
            Desktop::Workbench => &mut self.workbench,
            Desktop::Tiling => &mut self.tiling,
        }
    }
}

/// Home's shared state.
pub struct Home {
    shell_lines: Vec<String>,
    shell_input: String,
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
}

impl Home {
    pub fn new(boot: Vec<String>) -> Self {
        Self {
            shell_lines: boot,
            shell_input: String::new(),
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
                if self.dialing.is_none() {
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
            self.run(&line, data, save, out);
            self.focus_shell = true;
        }
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
        }
    }
}

/// A dim line for an empty tile or slot.
pub(crate) fn hint(painter: &egui::Painter, rect: Rect, text: &str) {
    painter.text(
        pos2(rect.center().x, rect.center().y),
        Align2::CENTER_CENTER,
        text,
        font(),
        muted(),
    );
}
