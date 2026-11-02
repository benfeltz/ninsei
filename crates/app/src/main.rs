//! ninsei: a cyberspace hacking roguelike dressed as a retro IDE.

mod bbs_view;
mod btop;
mod chrome;
mod crash;
mod crt;
mod crt_gl;
mod encounter_view;
mod glitch;
mod hexview;
mod home;
mod hubtools;
mod netmap;
mod netmap_draft;
mod netmap_iso;
mod netpane;
mod panes;
mod sound;
mod suite;
mod theme;
mod tiling;
mod workbench;

use egui::{Key, RichText};
use egui_dock::{DockArea, DockState};
use ninsei_core::bbs::Outcome;
use ninsei_core::hexedit::HexBuffer;
use ninsei_core::hub::{Answer, DEFAULT_RUN_THEME, RunEnd, RunReport, SAVE_KEY, Save, Screen};
use ninsei_core::{Action, Command, Data, Game, LogKind, Phase, command::HELP, parse_command};

use bbs_view::BbsView;
use encounter_view::EncounterView;
use ninsei_core::hardware::Chrome;
use ninsei_core::home_bbs::HomeCall;

use chrome::{FrameCx, MenuPart};
use crt_gl::CrtGl;
use home::{Desktops, Home, WbOut};
use ninsei_core::display::{Effects, screen_to_content};
use panes::{Panes, Request, Tab};
use theme::Palette;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    let data = match Data::builtin() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("bad game data: {e}");
            std::process::exit(1);
        }
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("ninsei")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([900.0, 560.0]),
        ..Default::default()
    };
    // A hex seed on the command line is used for the first dial.
    let seed = std::env::args()
        .nth(1)
        .and_then(|s| u64::from_str_radix(&s, 16).ok());
    eframe::run_native(
        "ninsei",
        options,
        Box::new(move |cc| Ok(Box::new(App::new(cc, data, seed)))),
    )
}

/// Web entry point: mounts the game on the page's `<canvas id="ninsei">`.
/// A seed can be shared in the URL: `.../#1a2b`.
#[cfg(target_arch = "wasm32")]
fn main() {
    use wasm_bindgen::JsCast;

    wasm_bindgen_futures::spawn_local(async {
        let Some(document) = web_sys::window().and_then(|w| w.document()) else {
            return;
        };
        let canvas = document
            .get_element_by_id("ninsei")
            .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok());
        let status = document.get_element_by_id("status");
        let say = |msg: &str| {
            if let Some(s) = &status {
                s.set_text_content(Some(msg));
            }
        };
        let Some(canvas) = canvas else {
            say("no <canvas id=\"ninsei\"> on this page.");
            return;
        };
        let data = match Data::builtin() {
            Ok(d) => d,
            Err(e) => {
                say(&format!("bad game data: {e}"));
                return;
            }
        };
        let seed = web_sys::window()
            .and_then(|w| w.location().hash().ok())
            .and_then(|h| u64::from_str_radix(h.trim_start_matches('#'), 16).ok());
        let started = eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(move |cc| Ok(Box::new(App::new(cc, data, seed)))),
            )
            .await;
        match started {
            Ok(()) => {
                if let Some(s) = &status {
                    s.remove();
                }
            }
            Err(e) => say(&format!("NO CARRIER: {e:?}")),
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn fresh_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0x5eed, |d| d.as_nanos() as u64)
}

/// The browser has no `SystemTime`; mix the clock with a random draw instead.
#[cfg(target_arch = "wasm32")]
fn fresh_seed() -> u64 {
    let now = js_sys::Date::now() as u64;
    let r = (js_sys::Math::random() * u32::MAX as f64) as u64;
    (now << 20) ^ r
}

/// Only used if the theme file somehow lacks the default theme (validation forbids it).
const FALLBACK_PALETTE: Palette = Palette {
    bg: egui::Color32::BLACK,
    panel: egui::Color32::BLACK,
    faint: egui::Color32::DARK_GRAY,
    dim: egui::Color32::GRAY,
    fg: egui::Color32::LIGHT_GRAY,
    bright: egui::Color32::WHITE,
};

/// The run screen's layout a deck firmware starts with: the chrome's own, with the secondary
/// panes the player has (`extra`) beside the terminal.
fn default_layout(chrome: Chrome, extra: &[Tab]) -> DockState<Tab> {
    chrome::view(chrome).default_layout(extra)
}

/// A saved layout, if it reads back and still has every core pane; otherwise `None`. Secondary
/// panes the player doesn't have (`has` says no) are taken out.
fn parse_layout(text: &str, has: impl Fn(Tab) -> bool) -> Option<DockState<Tab>> {
    let mut dock: DockState<Tab> = ron::from_str(text).ok()?;
    let all: Vec<Tab> = dock.iter_all_tabs().map(|(_, t)| *t).collect();
    if !Tab::CORE.iter().all(|t| all.contains(t)) {
        return None;
    }
    for tab in all.into_iter().filter(|t| t.is_secondary() && !has(*t)) {
        if let Some(at) = dock.find_tab(&tab) {
            dock.remove_tab(at);
        }
    }
    Some(dock)
}

/// How often the run screen's layout is checked for changes to save, in seconds.
const LAYOUT_CHECK_SECS: f64 = 2.0;

struct App {
    data: Data,
    save: Save,
    /// The save changed and should be written this frame.
    save_dirty: bool,
    /// Programs (by uid) picked to keep when jacking out at the exit.
    keep: Vec<u32>,
    screen: Screen,
    /// Home's shared state (the Shell, CarrierTerm, HexEd) and every desktop that draws it.
    home: Home,
    desktops: Desktops,
    /// The current run, once booked (exit or flatline).
    booked: Option<RunReport>,
    /// The run ended in a flatline: the crash screen sits behind the returning desktop.
    crashed: bool,
    /// Seed from the command line or URL, used for the first dial.
    first_seed: Option<u64>,
    /// The run. Between runs this is the last one, or a placeholder before the first.
    game: Game,
    dock: DockState<Tab>,
    /// When the layout was last compared with the saved one.
    layout_checked: f64,
    /// This frame's damage glitches.
    glitch: glitch::Glitch,
    /// The CRT shader pass (falls back to `crt.rs`'s overlay if GL can't run it).
    crt_gl: CrtGl,
    /// Run theme id (see `data/themes.ron`) and its palette.
    run_theme: String,
    pal: Palette,
    crt: bool,
    /// Plays the modem handshake when you dial.
    speaker: sound::Speaker,
    /// Whether the BBS window was up last frame, to dial or drop the line's sound.
    bbs_heard: bool,
    input: String,
    focus_input: bool,
    last_phase: Phase,
    last_at: usize,
    bbs: Option<BbsView>,
    /// A BBS called from home, over the desktop.
    home_bbs: Option<(BbsView, HomeCall)>,
    hex: Option<HexBuffer>,
    /// Uid of the program open in the hex editor, so trades and sales can't swap it underneath.
    hex_uid: u32,
    talk: EncounterView,
    /// The suite chrome's sheet cursor, sort and dialogs.
    suite: suite::State,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, data: Data, first_seed: Option<u64>) -> Self {
        let mut boot = vec![
            String::new(),
            String::new(),
            "Open CarrierTerm and dial a network,".to_string(),
            "or type `phonebook` here. `help` for more.".to_string(),
        ];
        // A missing or unreadable save starts a fresh hub; it never stops the game.
        let save = match cc.storage.and_then(|s| s.get_string(SAVE_KEY)) {
            None => Save::default(),
            Some(text) => match Save::load(&text) {
                Ok(s) => {
                    let plural = if s.runs == 1 { "" } else { "s" };
                    boot.push(if s.runs == 0 {
                        "Welcome back.".to_string()
                    } else {
                        format!("Welcome back. {} run{plural} on record.", s.runs)
                    });
                    s
                }
                Err(e) => {
                    boot.push(format!("Save unreadable ({e}). Starting fresh."));
                    Save::default()
                }
            },
        };
        if let Some(seed) = first_seed {
            boot.push(format!("Seed {seed:x} queued for your first dial."));
        }
        // Stash binaries are signed with a key that belongs to this save.
        let mut save = save;
        let keyed = save.ensure_key(fresh_seed());
        // A new save starts with a handful of numbers to dial.
        let keyed = save.ensure_phonebook(&data, fresh_seed()) || keyed;
        // Everyone starts with a deck and a monitor; a mono monitor picks the theme.
        let keyed = save.ensure_hardware(&data) || keyed;
        boot[0] = format!(
            "{} // {}",
            save.firmware_def(&data).desktop.name(),
            save.deck_def(&data).title()
        );
        let run_theme = match data.theme(&save.phosphor) {
            Some(t) => t.id.clone(),
            None => DEFAULT_RUN_THEME.to_string(),
        };
        let pal = data
            .theme(&run_theme)
            .map(Palette::from_def)
            .unwrap_or(FALLBACK_PALETTE);
        theme::apply(&cc.egui_ctx, &pal);
        let game = Game::new(data.clone(), first_seed.unwrap_or(0));
        let mut me = Self {
            crt: save.crt,
            speaker: sound::Speaker::new(save.sound),
            bbs_heard: false,
            data,
            save,
            save_dirty: keyed,
            keep: Vec::new(),
            screen: Screen::Hub,
            home: Home::new(boot),
            desktops: Desktops::new(),
            booked: None,
            crashed: false,
            first_seed,
            last_at: game.at,
            bbs: None,
            home_bbs: None,
            hex: None,
            hex_uid: 0,
            talk: EncounterView::default(),
            suite: suite::State::default(),
            last_phase: game.phase,
            game,
            dock: default_layout(Chrome::Classic, &[]),
            run_theme,
            pal,
            crt_gl: CrtGl::new(),
            input: String::new(),
            focus_input: true,
            layout_checked: 0.0,
            glitch: glitch::Glitch::default(),
        };
        me.load_layout();
        me
    }

    /// The layout for the deck and firmware in use: the saved one, or its default.
    fn load_layout(&mut self) {
        let chrome = self.save.firmware_def(&self.data).chrome;
        let extra = self.extra_panes();
        self.dock = self
            .save
            .layout(&self.data)
            .and_then(|text| parse_layout(text, |t| extra.contains(&t)))
            .unwrap_or_else(|| default_layout(chrome, &extra));
    }

    /// The secondary panes the player has right now: bought, or bundled with the firmware.
    fn extra_panes(&self) -> Vec<Tab> {
        Tab::SECONDARY
            .iter()
            .filter(|(_, id)| self.save.has_pane(&self.data, id))
            .map(|(t, _)| *t)
            .collect()
    }

    /// Writes the layout into the save when it's changed (checked every couple of seconds).
    fn keep_layout(&mut self, now: f64) {
        if now - self.layout_checked < LAYOUT_CHECK_SECS {
            return;
        }
        self.layout_checked = now;
        if let Ok(text) = ron::to_string(&self.dock)
            && self.save.set_layout(&self.data, text)
        {
            self.save_dirty = true;
        }
    }

    /// Adds a secondary pane: the net pane goes beside the terminal; anything else into the
    /// focused leaf.
    fn add_pane(&mut self, tab: Tab) {
        if self.dock.find_tab(&tab).is_some() {
            return;
        }
        match self.dock.find_tab(&Tab::Terminal) {
            Some(at) if at.surface == egui_dock::SurfaceIndex::main() => {
                self.dock
                    .main_surface_mut()
                    .split_right(at.node, 0.55, vec![tab]);
            }
            _ => self.dock.push_to_focused_leaf(tab),
        }
    }

    /// Switches the theme, if the monitor can show it. Returns its label, or why not.
    fn set_run_theme(&mut self, ctx: &egui::Context, id: &str) -> Result<String, String> {
        let Some(label) = self.data.theme(id).map(|t| t.label.clone()) else {
            return Err(format!("no theme {id}."));
        };
        if let Some(why) = self.save.theme_blocker(&self.data, id) {
            return Err(why);
        }
        self.apply_theme(ctx, id);
        Ok(label)
    }

    /// Paints with theme `id` and saves it. No monitor check: callers have done that.
    fn apply_theme(&mut self, ctx: &egui::Context, id: &str) {
        let Some(t) = self.data.theme(id) else {
            return;
        };
        self.pal = Palette::from_def(t);
        self.run_theme = t.id.clone();
        theme::apply(ctx, &self.pal);
        self.save.phosphor = t.id.clone();
        self.save_dirty = true;
    }

    /// The CRT effects to draw, if they're on.
    fn effects(&self) -> Option<Effects> {
        if !self.crt {
            return None;
        }
        self.data
            .display(self.save.display_id(&self.data))
            .or_else(|| self.data.displays.first())
            .map(|d| Effects::from_def(&d.effects))
    }

    /// Changes hardware through one of `Save`'s methods, then repaints in whatever theme the
    /// monitor now shows. Plugging in a monitor switches CRT effects on.
    fn hardware(
        &mut self,
        ctx: &egui::Context,
        change: impl FnOnce(&mut Save, &Data) -> Result<String, String>,
    ) -> Result<String, String> {
        let before = self.save.monitor.clone();
        let layout = self.save.layout_key(&self.data);
        let msg = change(&mut self.save, &self.data)?;
        self.save_dirty = true;
        if self.save.layout_key(&self.data) != layout {
            self.load_layout();
        }
        let theme = self.save.phosphor.clone();
        self.apply_theme(ctx, &theme);
        if self.save.monitor != before && !self.crt {
            self.toggle_crt();
        }
        Ok(msg)
    }

    fn run_theme_label(&self) -> String {
        self.data
            .theme(&self.run_theme)
            .map_or(self.run_theme.clone(), |t| t.label.clone())
    }

    fn toggle_sound(&mut self) -> &'static str {
        self.save.sound = !self.save.sound;
        self.speaker.set_on(self.save.sound);
        self.save_dirty = true;
        if self.save.sound { "on" } else { "off" }
    }

    /// Starts the handshake sound when a dial begins, and cuts it when a BBS line drops.
    fn sync_sound(&mut self) {
        if let Some(id) = self.home.dial_started.take()
            && let Some(number) = self.save.entry(id).map(|e| e.number.clone())
        {
            self.speaker.dial(&number);
        }
        let open = self.bbs.is_some() || self.home_bbs.is_some();
        if open && !self.bbs_heard {
            // A call from home dials that board's number; in a run, the network's board.
            let phone = match &self.home_bbs {
                Some((_, call)) => ninsei_core::bbs::Line::board(call).phone.clone(),
                None => self.game.bbs().phone.clone(),
            };
            self.speaker.dial(&phone);
        } else if !open && self.bbs_heard {
            self.speaker.stop();
        }
        self.bbs_heard = open;
    }

    fn toggle_crt(&mut self) {
        self.crt = !self.crt;
        self.save.crt = self.crt;
        self.save_dirty = true;
    }

    /// Dials a phonebook entry: the run starts behind the desktop as it drags away.
    fn dial(&mut self, entry: u32, seed: Option<u64>) {
        let seed = seed.or(self.first_seed.take()).unwrap_or_else(fresh_seed);
        // The loadout and RIG come along; stash programs taken leave the stash.
        match self.save.dial(&self.data, entry, seed) {
            Ok(g) => self.game = g,
            Err(e) => {
                self.home.say(format!("NO CARRIER: {e}"));
                return;
            }
        }
        self.save_dirty = true;
        self.home_bbs = None;
        self.keep.clear();
        self.last_at = self.game.at;
        self.last_phase = self.game.phase;
        self.bbs = None;
        self.hex = None;
        self.talk = EncounterView::default();
        self.input.clear();
        self.focus_input = true;
        self.booked = None;
        self.crashed = false;
        if let Some(path) = self.dock.find_tab(&Tab::Network) {
            let _ = self.dock.set_active_tab(path);
        }
        self.screen = Screen::JackIn { t: 0.0 };
    }

    /// Books the run in the save once it's over (exit or flatline).
    fn book(&mut self) {
        if self.booked.is_some() {
            return;
        }
        let keep: Vec<usize> = self
            .keep
            .iter()
            .filter_map(|uid| self.game.hdd.iter().position(|s| s.uid == *uid))
            .collect();
        if let Some(r) = self.save.record(&self.game, &keep, &self.data.rig) {
            self.save_dirty = true;
            match r.end {
                RunEnd::Exit => {
                    self.home.say(format!(
                        "Jacked out after {} hops. Banked {}¢.",
                        r.hops, r.banked
                    ));
                    if !r.kept.is_empty() {
                        self.home.say(format!("Stashed: {}", r.kept.join(", ")));
                    }
                }
                RunEnd::Flatline => self.home.say(format!(
                    "Flatlined after {} hops. Dead man's switch saved {} of {}¢.",
                    r.hops, r.banked, r.credits
                )),
            }
            if !r.learned.is_empty() {
                self.home
                    .say(format!("New phone ranges: {}", r.learned.join(", ")));
            }
            if !r.boards.is_empty() {
                self.home
                    .say(format!("New BBS in CarrierTerm: {}", r.boards.join(", ")));
            }
            self.booked = Some(r);
        }
    }

    /// A board called from home: draws it, and applies downloads and jobs to the save.
    fn home_bbs_ui(&mut self, ctx: &egui::Context, pal: &Palette) {
        let Some((view, call)) = &mut self.home_bbs else {
            return;
        };
        let outs = view.show(ctx, &*call, pal);
        let board = call.board;
        let rig = self.data.rig.clone();
        for o in outs {
            let result = match o {
                Outcome::Act(ninsei_core::Action::Buy { area, file }) => {
                    self.save.home_buy(&self.data, &rig, board, area, file)
                }
                Outcome::Act(ninsei_core::Action::TakeJob { job }) => {
                    self.save.home_take(&self.data, board, job)
                }
                Outcome::Closed => {
                    self.home_bbs = None;
                    self.home.say("NO CARRIER");
                    return;
                }
                _ => continue,
            };
            self.save_dirty = true;
            let fresh = self.save.home_call(&self.data, &rig, board);
            if let Some((view, call)) = &mut self.home_bbs {
                *call = fresh;
                match result {
                    Ok(msg) => self.home.say(msg),
                    Err(e) => view.session.status = Some(format!("|12{e}")),
                }
            }
        }
    }

    /// Applies a hub tool's request to the save and reports the result in the Shell.
    fn home_action(&mut self, o: WbOut) {
        let rig = self.data.rig.clone();
        let result = match o {
            WbOut::Buy(u) => self.save.buy(&rig, u),
            WbOut::ToggleStarter(id) => self.save.toggle_starter(&rig, &id).map(|()| String::new()),
            WbOut::ToggleStash(id) => self.save.toggle_stash(&rig, id).map(|()| String::new()),
            WbOut::Scrap(id) => Ok(self
                .save
                .scrap(id)
                .map_or(String::new(), |_| "Scrapped.".to_string())),
            WbOut::Patch(id, bin) => self.save.patch(id, &bin),
            WbOut::Optimize(id) => self.save.optimize(id),
            WbOut::Scan(range) => match self.save.wardial(&self.data, &range) {
                Ok(res) => {
                    let new = res
                        .cells
                        .iter()
                        .filter(|(_, a)| matches!(a, Answer::Carrier { new: true, .. }))
                        .count();
                    let traps = res
                        .cells
                        .iter()
                        .filter(|(_, a)| matches!(a, Answer::Trap))
                        .count();
                    let msg = format!(
                        "Wardialed block {:02}: {new} new carrier(s), {traps} trap(s). -{}¢",
                        res.block, res.cost
                    );
                    self.home.show_scan(res);
                    Ok(msg)
                }
                Err(e) => Err(e),
            },
            WbOut::Call(id) => {
                let Some(board) = self.data.board(&id) else {
                    return;
                };
                let rig = self.data.rig.clone();
                match self.save.call_board(&self.data, &rig, board) {
                    Ok(call) => {
                        let mut view = BbsView::new();
                        view.order = egui::Order::Foreground;
                        let d = &self.data.boards[board];
                        let msg = format!(
                            "CONNECT {} to {}. Phone bill -{}¢.",
                            d.baud, d.name, d.call_cost
                        );
                        self.home_bbs = Some((view, call));
                        Ok(msg)
                    }
                    Err(e) => Err(format!("NO CARRIER: {e}")),
                }
            }
            WbOut::Dial(..)
            | WbOut::Theme(_)
            | WbOut::Crt
            | WbOut::Sound
            | WbOut::Glitches(_)
            | WbOut::HelpSeen
            | WbOut::Tiling(_)
            | WbOut::BuyAddon(_)
            | WbOut::BuyPane(_)
            | WbOut::Monitor(_)
            | WbOut::Deck(_)
            | WbOut::Firmware(_)
            | WbOut::BuyHardware(_) => {
                return;
            }
        };
        match result {
            Ok(msg) => {
                if !msg.is_empty() {
                    self.home.say(msg);
                }
                self.save_dirty = true;
            }
            Err(e) => self.home.say(format!("Error: {e}")),
        }
    }

    /// Back to the desktop: it drags up over the run.
    fn go_home(&mut self) {
        self.book();
        self.bbs = None;
        self.home.focus();
        self.screen = Screen::JackOut { t: 0.0 };
    }

    fn handle(&mut self, ctx: &egui::Context, req: Request) {
        let cmd = match req {
            Request::OpenBbs => {
                if self.game.at_bbs() {
                    self.bbs = Some(BbsView::new());
                }
                return;
            }
            Request::Monitor(id) => {
                let r = self.hardware(ctx, |s, d| s.plug_monitor(d, &id));
                self.game.say(LogKind::Sys, r.unwrap_or_else(|e| e));
                return;
            }
            Request::Glitches(g) => {
                self.save.glitches = g;
                self.save_dirty = true;
                self.game
                    .say(LogKind::Sys, format!("damage glitches {}", g.label()));
                return;
            }
            Request::AddPane(tab) => {
                self.add_pane(tab);
                return;
            }
            Request::Type(text) => {
                self.input = text;
                self.focus_input = true;
                return;
            }
            Request::Deck(id) => {
                let r = self.hardware(ctx, |s, d| s.use_deck(d, &id));
                self.game.say(LogKind::Sys, r.unwrap_or_else(|e| e));
                return;
            }
            Request::GoHome => {
                if matches!(self.game.phase, Phase::JackedOut | Phase::Flatlined) {
                    self.go_home();
                }
                return;
            }
            Request::OpenHex(slot) => {
                self.open_hex(slot);
                return;
            }
            Request::CloseHex => {
                self.hex = None;
                return;
            }
            Request::Cmd(c) => c,
            Request::Line(line) => {
                self.game.say(LogKind::Sys, format!("$ {line}"));
                match parse_command(&line) {
                    Ok(c) => c,
                    Err(e) => {
                        self.game.say(LogKind::Sys, format!("err: {e}"));
                        return;
                    }
                }
            }
        };
        match cmd {
            // New runs start at home now, by dialing from the desktop.
            Command::Game(Action::NewRun(_)) | Command::NewRun => {
                if self.game.phase == Phase::JackedOut {
                    self.go_home();
                } else {
                    self.game.say(
                        LogKind::Sys,
                        "err: new runs start at home. reach the exit first.",
                    );
                }
            }
            Command::Game(a) => self.game.apply(a),
            Command::Hex(slot) => self.open_hex(slot),
            Command::Help => {
                for l in HELP {
                    self.game.say(LogKind::Sys, *l);
                }
            }
            Command::Clear => self.game.log.clear(),
            Command::Crt => {
                self.toggle_crt();
                let state = if self.crt { "on" } else { "off" };
                self.game.say(LogKind::Sys, format!("crt {state}"));
            }
            Command::Sound => {
                let state = self.toggle_sound();
                self.game.say(LogKind::Sys, format!("modem sounds {state}"));
            }
            Command::Theme(name) => match self.set_run_theme(ctx, &name) {
                Ok(label) => self.game.say(LogKind::Sys, format!("theme: {label}")),
                Err(e) if self.data.theme(&name).is_some() => {
                    self.game.say(LogKind::Sys, format!("err: {e}"));
                }
                Err(_) => {
                    let ids: Vec<String> = self.data.themes.iter().map(|t| t.id.clone()).collect();
                    self.game.say(
                        LogKind::Sys,
                        format!("err: no run theme `{name}`. try {}.", ids.join(", ")),
                    );
                }
            },
        }
    }

    fn open_hex(&mut self, slot: usize) {
        let Some(s) = self.game.hdd.get(slot) else {
            self.game
                .say(LogKind::Sys, format!("err: no program in slot {slot}."));
            return;
        };
        self.hex = Some(HexBuffer::open(slot, s.bin));
        self.hex_uid = s.uid;
        if let Some(path) = self.dock.find_tab(&Tab::Hex) {
            let _ = self.dock.set_active_tab(path);
        }
    }

    /// Keeps the open buffer in step with the game: a successful write or an optimize
    /// becomes the new baseline, and a vanished slot closes the editor.
    fn sync_hex(&mut self) {
        let Some(h) = &mut self.hex else { return };
        // Follow the program if the HDD reshuffled; close if it's gone.
        match self.game.hdd.iter().position(|s| s.uid == self.hex_uid) {
            Some(i) => h.slot = i,
            None => {
                self.hex = None;
                return;
            }
        }
        match self.game.hdd.get(h.slot) {
            None => self.hex = None,
            Some(s) if h.buf == s.bin || !h.dirty() => h.committed(s.bin),
            Some(_) => {}
        }
    }

    /// Hotkeys work whenever the terminal isn't taking typing.
    fn hotkeys(&mut self, ctx: &egui::Context) -> Vec<Request> {
        let hex_typing = ctx.memory(|m| m.has_focus(hexview::grid_id()));
        let talking = self.game.encounter.is_some();
        if self.bbs.is_some() || talking || hex_typing || ctx.egui_wants_keyboard_input() {
            return Vec::new();
        }
        let mut out = Vec::new();
        ctx.input(|i| {
            if i.key_pressed(Key::Space) {
                out.push(Request::Cmd(Command::Game(Action::EndTurn)));
            }
            let digits = [
                Key::Num0,
                Key::Num1,
                Key::Num2,
                Key::Num3,
                Key::Num4,
                Key::Num5,
                Key::Num6,
                Key::Num7,
                Key::Num8,
                Key::Num9,
            ];
            for (n, k) in digits.iter().enumerate() {
                if i.key_pressed(*k) {
                    out.push(Request::Cmd(Command::Game(Action::Run(n))));
                }
            }
            if i.key_pressed(Key::Enter) || i.key_pressed(Key::Slash) {
                self.focus_input = true;
            }
        });
        out
    }

    /// The menu bar: the chrome's name, then its menus, each built from shared parts.
    fn menu_bar(&mut self, ui: &mut egui::Ui, pal: &theme::Palette) -> Vec<Request> {
        let mut out = Vec::new();
        let v = chrome::view(self.save.firmware_def(&self.data).chrome);
        egui::MenuBar::new().ui(ui, |ui| {
            ui.label(RichText::new(v.brand()).color(pal.bright).strong());
            for (name, parts) in v.menus() {
                ui.menu_button(*name, |ui| {
                    for (i, part) in parts.iter().enumerate() {
                        if i > 0 && matches!(part, MenuPart::Monitors | MenuPart::Decks) {
                            ui.separator();
                        }
                        self.menu_part(ui, *part, pal, &mut out);
                    }
                });
            }
        });
        out
    }

    fn menu_part(
        &mut self,
        ui: &mut egui::Ui,
        part: MenuPart,
        pal: &theme::Palette,
        out: &mut Vec<Request>,
    ) {
        match part {
            MenuPart::Turn => {
                if ui.button("end turn   [space]").clicked() {
                    out.push(Request::Cmd(Command::Game(Action::EndTurn)));
                }
            }
            MenuPart::Help => {
                if ui.button("help").clicked() {
                    out.push(Request::Cmd(Command::Help));
                }
            }
            MenuPart::Layout => {
                if ui.button("reset layout").clicked() {
                    self.save.clear_layout(&self.data);
                    self.save_dirty = true;
                    self.load_layout();
                }
            }
            MenuPart::Glitches => {
                let g = self.save.glitches;
                if ui
                    .button(format!("damage glitches: {}", g.label()))
                    .clicked()
                {
                    out.push(Request::Glitches(g.next()));
                }
            }
            MenuPart::Panes => {
                ui.menu_button("add pane", |ui| {
                    let mut any = false;
                    for tab in self.extra_panes() {
                        if self.dock.find_tab(&tab).is_none() {
                            any = true;
                            let id = tab.pane_id().unwrap_or_default();
                            let name = self
                                .data
                                .panes
                                .iter()
                                .find(|d| d.id == id)
                                .map_or(id, |d| d.name.as_str());
                            if ui.button(name).clicked() {
                                out.push(Request::AddPane(tab));
                            }
                        }
                    }
                    if !any {
                        ui.label(
                            RichText::new("every pane you own is open. more in RIG.")
                                .color(pal.dim),
                        );
                    }
                });
            }
            MenuPart::Toggles => {
                let crt = if self.crt {
                    "[x] crt effects"
                } else {
                    "[ ] crt effects"
                };
                if ui.button(crt).clicked() {
                    out.push(Request::Cmd(Command::Crt));
                }
                let sound = if self.save.sound {
                    "[x] modem sounds"
                } else {
                    "[ ] modem sounds"
                };
                if ui.button(sound).clicked() {
                    out.push(Request::Cmd(Command::Sound));
                }
            }
            MenuPart::Monitors => {
                ui.label(RichText::new("monitor").color(pal.dim));
                for id in self.save.monitors.clone() {
                    let Some(m) = self.data.hardware.monitor(&id) else {
                        continue;
                    };
                    let mark = if id == self.save.monitor {
                        "(•)"
                    } else {
                        "( )"
                    };
                    if ui.button(format!("{mark} {}", m.title())).clicked() {
                        out.push(Request::Monitor(id));
                    }
                }
            }
            MenuPart::Decks => {
                ui.label(RichText::new("deck").color(pal.dim));
                for id in self.save.decks.clone() {
                    let Some(d) = self.data.hardware.deck(&id) else {
                        continue;
                    };
                    let mark = if id == self.save.deck { "(•)" } else { "( )" };
                    if ui.button(format!("{mark} {}", d.title())).clicked() {
                        out.push(Request::Deck(id));
                    }
                }
            }
            MenuPart::Themes => {
                let themes: Vec<(String, String)> = self
                    .data
                    .themes
                    .iter()
                    .map(|t| (t.id.clone(), t.label.clone()))
                    .collect();
                for (id, label) in themes {
                    let mark = if id == self.run_theme { "(•)" } else { "( )" };
                    let ok = self.save.theme_blocker(&self.data, &id).is_none();
                    let b = ui.add_enabled(ok, egui::Button::new(format!("{mark} {label}")));
                    let b = b.on_disabled_hover_text("your monitor is a mono tube");
                    if b.clicked() {
                        let _ = self.set_run_theme(ui.ctx(), &id);
                    }
                }
            }
        }
    }

    fn status_bar(&self, ui: &mut egui::Ui, pal: &theme::Palette) {
        let g = &self.game;
        let v = chrome::view(self.save.firmware_def(&self.data).chrome);
        if v.status(ui, pal, g) {
            return;
        }
        let node = &g.net.nodes[g.at];
        ui.horizontal(|ui| {
            let (state, col) = match g.phase {
                Phase::Exploring => ("● LINKED", pal.fg),
                Phase::Combat => ("● ICE CONTACT", pal.bright),
                Phase::Encounter => ("● INCOMING MSG", pal.bright),
                Phase::Flatlined => ("○ FLATLINE", pal.bright),
                Phase::JackedOut => ("○ OFFLINE", pal.dim),
            };
            ui.label(RichText::new(state).color(col));
            if let Some(site) = &g.site {
                let traits = if site.traits.is_empty() {
                    String::new()
                } else {
                    format!(" · {}", site.traits.join(", "))
                };
                ui.label(
                    RichText::new(format!("{} T{}{traits}", site.name, site.tier)).color(pal.fg),
                );
            }
            if let Some(o) = &g.outer {
                let door = &o.net.nodes[o.node].name;
                ui.label(RichText::new(format!("SUBNET via {door}")).color(pal.bright));
            }
            ui.label(
                RichText::new(format!(
                    "{}  layer {}/{}",
                    node.name,
                    node.layer,
                    g.net.layers.len() - 1
                ))
                .color(pal.dim),
            );
            ui.label(RichText::new(format!("trace {}%", g.deck.trace)).color(pal.dim));
            v.status_extra(ui, pal, g);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new("UTF-8  LF  RUST").color(pal.dim));
                ui.label(RichText::new(self.run_theme_label()).color(pal.dim));
            });
        });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let pal = self.pal;
        let dt = ctx.input(|i| i.stable_dt).min(0.1);
        self.screen = self.screen.tick(dt);
        self.sync_sound();
        // The run's sound cues: whatever happened since last frame, played together.
        // The deck's firmware picks the sound pack.
        let pack = self.save.firmware_def(&self.data).sounds.clone();
        let sounds = self
            .data
            .sound_pack(&pack)
            .unwrap_or(&self.data.sounds[0].sounds);
        for cue in std::mem::take(&mut self.game.cues) {
            self.speaker.cue(cue, &pack, sounds);
        }

        let live = self.screen == Screen::Run && !self.crashed;
        self.glitch = glitch::Glitch::frame(&ctx, &self.game, self.save.glitches, live);
        if self.glitch.busy() {
            ctx.request_repaint();
        }
        if self.screen.shows_run() {
            if self.crashed {
                let clicked = crash::show(ui, self.booked.as_ref());
                if clicked && self.screen == Screen::Crash {
                    self.go_home();
                }
            } else {
                self.run_ui(ui, &ctx, &pal);
            }
            // A flatline books the run and crashes the deck.
            if self.screen == Screen::Run && self.game.phase == Phase::Flatlined && !self.crashed {
                self.book();
                self.bbs = None;
                self.crashed = true;
                self.screen = Screen::Crash;
            }
        }

        // The desktop: on its own while home, dragging during the transitions.
        if !matches!(self.screen, Screen::Run | Screen::Crash) {
            // One theme colours everything: load the desktop's palette from it.
            if let Some(t) = self.data.theme(&self.run_theme) {
                home::set_desk(home::Desk::from_def(t));
            }
            let offset = self.screen.drag() * ctx.content_rect().height();
            let live = self.screen.hub_live();
            // Dev key: free credits at home, for trying out RIG and the BBSes.
            if live && ctx.input(|i| i.key_pressed(egui::Key::F8)) {
                let msg = self.save.dev_credits();
                self.save_dirty = true;
                self.home.say(msg);
            }
            let mut outs = Vec::new();
            egui::Area::new(egui::Id::new("workbench"))
                .order(egui::Order::Foreground)
                .movable(false)
                .fixed_pos(egui::pos2(0.0, 0.0))
                .show(&ctx, |ui| {
                    ui.set_min_size(ctx.content_rect().size());
                    let desk = self.save.firmware_def(&self.data).desktop;
                    let view = self.desktops.get(desk);
                    outs = self
                        .home
                        .show(view, ui, &self.data, &self.save, offset, live);
                });
            for o in outs {
                match o {
                    WbOut::Dial(entry, seed) if live => self.dial(entry, seed),
                    WbOut::Dial(..) => {}
                    WbOut::Theme(name) => match self.set_run_theme(&ctx, &name) {
                        Ok(label) => self.home.say(format!("theme: {label}")),
                        Err(e) => self.home.say(e),
                    },
                    WbOut::HelpSeen => {
                        self.save.tiling_help_seen = true;
                        self.save_dirty = true;
                    }
                    WbOut::Tiling(t) => {
                        self.save.tiling = t;
                        self.save_dirty = true;
                    }
                    WbOut::BuyAddon(id) => {
                        let r = self.save.buy_addon(&self.data, &id);
                        self.save_dirty = true;
                        self.home.say(r.unwrap_or_else(|e| e));
                    }
                    WbOut::BuyPane(id) => {
                        let r = self.save.buy_pane(&self.data, &id);
                        self.save_dirty = true;
                        self.home.say(r.unwrap_or_else(|e| e));
                    }
                    WbOut::Glitches(g) => {
                        self.save.glitches = g;
                        self.save_dirty = true;
                        self.home.say(format!("damage glitches {}", g.label()));
                    }
                    WbOut::Monitor(id) => {
                        let r = self.hardware(&ctx, |s, d| s.plug_monitor(d, &id));
                        self.home.say(r.unwrap_or_else(|e| e));
                    }
                    WbOut::Deck(id) => {
                        let r = self.hardware(&ctx, |s, d| s.use_deck(d, &id));
                        self.home.say(r.unwrap_or_else(|e| e));
                    }
                    WbOut::Firmware(id) => {
                        let r = self.hardware(&ctx, |s, d| s.use_firmware(d, &id));
                        self.home.say(r.unwrap_or_else(|e| e));
                    }
                    WbOut::BuyHardware(hw) => {
                        let r = self.hardware(&ctx, |s, d| s.buy_hardware(d, &hw));
                        self.home.say(r.unwrap_or_else(|e| e));
                    }
                    WbOut::Crt => {
                        self.toggle_crt();
                        self.home
                            .say(format!("crt {}", if self.crt { "on" } else { "off" }));
                    }
                    WbOut::Sound => {
                        let state = self.toggle_sound();
                        self.home.say(format!("modem sounds {state}"));
                    }
                    other => self.home_action(other),
                }
            }
            if live {
                self.home_bbs_ui(&ctx, &pal);
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }

        // Damage glitches go on top of the preset, for the shader only: the mouse keeps the
        // preset's own geometry.
        match self.effects() {
            Some(e) if !self.crt_gl.failed() => {
                self.glitch.overlay(&ctx, &pal, true);
                let (e, tear) = self.glitch.effects(e);
                self.crt_gl.paint(&ctx, e, tear);
            }
            Some(_) => {
                self.glitch.overlay(&ctx, &pal, false);
                crt::paint(&ctx, &pal);
            }
            None => self.glitch.overlay(&ctx, &pal, false),
        }
        if !self.crt && self.game.phase == Phase::Exploring {
            // Keep the cursor blinking.
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
        if self.screen != Screen::Hub && self.screen != Screen::Run {
            ctx.request_repaint();
        }

        if self.save_dirty
            && let Some(storage) = frame.storage_mut()
        {
            storage.set_string(SAVE_KEY, self.save.to_text());
            storage.flush();
            self.save_dirty = false;
        }
    }

    /// Keeps the mouse on what you see: on curved glass, a point on the window shows a
    /// different point of the picture, so pointer and touch positions are warped to match.
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        let Some(e) = self.effects() else { return };
        if self.crt_gl.failed() || !e.warps() {
            return;
        }
        let screen = ctx.content_rect();
        if screen.width() <= 0.0 || screen.height() <= 0.0 {
            return;
        }
        let warp = |p: egui::Pos2| {
            let uv = [
                (p.x - screen.min.x) / screen.width(),
                (p.y - screen.min.y) / screen.height(),
            ];
            screen_to_content(uv, &e).map(|c| {
                egui::pos2(
                    screen.min.x + c[0] * screen.width(),
                    screen.min.y + c[1] * screen.height(),
                )
            })
        };
        raw.events.retain_mut(|ev| match ev {
            egui::Event::PointerMoved(p) => {
                match warp(*p) {
                    Some(q) => *p = q,
                    None => *ev = egui::Event::PointerGone,
                }
                true
            }
            // Presses on the bezel go nowhere; releases always get through.
            egui::Event::PointerButton { pos, pressed, .. } => match warp(*pos) {
                Some(q) => {
                    *pos = q;
                    true
                }
                None => !*pressed,
            },
            egui::Event::Touch { pos, .. } => {
                if let Some(q) = warp(*pos) {
                    *pos = q;
                }
                true
            }
            _ => true,
        });
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(SAVE_KEY, self.save.to_text());
    }
}

impl App {
    /// The run's IDE: menu, status bar, dock panes and the run's modal windows.
    fn run_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, pal: &theme::Palette) {
        let pal = *pal;
        let ctx = ctx.clone();
        // Keys only drive the run once it's fully on screen.
        let mut requests = if self.screen == Screen::Run {
            self.hotkeys(&ctx)
        } else {
            Vec::new()
        };
        let view = chrome::view(self.save.firmware_def(&self.data).chrome);
        let keys_free = self.bbs.is_none() && self.game.encounter.is_none();
        if self.screen == Screen::Run {
            let mut cx = FrameCx {
                game: &self.game,
                pal,
                suite: &mut self.suite,
                keys_free,
            };
            requests.extend(view.keys(&ctx, &mut cx));
        }

        egui::Panel::top("menu")
            .frame(
                egui::Frame::new()
                    .fill(pal.bg)
                    .inner_margin(egui::Margin::symmetric(6, 2)),
            )
            .show(ui, |ui| requests.extend(self.menu_bar(ui, &pal)));
        egui::Panel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(pal.faint)
                    .inner_margin(egui::Margin::symmetric(6, 1)),
            )
            .show(ui, |ui| self.status_bar(ui, &pal));
        if view.has_legend() {
            egui::Panel::bottom("legend")
                .frame(
                    egui::Frame::new()
                        .fill(pal.bg)
                        .inner_margin(egui::Margin::symmetric(6, 2)),
                )
                .show(ui, |ui| {
                    let mut cx = FrameCx {
                        game: &self.game,
                        pal,
                        suite: &mut self.suite,
                        keys_free,
                    };
                    requests.extend(view.legend(ui, &mut cx));
                });
        }

        let mut panes = Panes {
            game: &self.game,
            pal,
            map: self.save.firmware_def(&self.data).map,
            deck: self.save.deck_def(&self.data).title(),
            chrome: self.save.firmware_def(&self.data).chrome,
            corrupt: self.glitch.corruption(),
            input: &mut self.input,
            focus_input: &mut self.focus_input,
            // Any modal (BBS or talk) keeps the terminal from grabbing focus.
            bbs_open: self.bbs.is_some() || self.game.encounter.is_some(),
            hex: &mut self.hex,
            keep: &mut self.keep,
            keep_room: (self.save.stash_cap(&self.data.rig) as usize)
                .saturating_sub(self.save.stash.len()),
            suite: &mut self.suite,
            out: Vec::new(),
        };
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(pal.bg))
            .show(ui, |ui| {
                DockArea::new(&mut self.dock)
                    .style(theme::dock_style(ui.style(), &pal))
                    .show_close_buttons(true)
                    .show_add_buttons(false)
                    .show_leaf_collapse_buttons(false)
                    .show_leaf_close_all_buttons(false)
                    .show_inside(ui, &mut panes);
            });
        requests.extend(panes.out);
        self.keep_layout(ctx.input(|i| i.time));

        for r in requests {
            self.handle(&ctx, r);
        }
        self.sync_hex();

        // Landing on a BBS dials it; leaving (or dying) drops the line.
        if self.game.at != self.last_at {
            self.last_at = self.game.at;
            if self.game.at_bbs() {
                self.bbs = Some(BbsView::new());
            }
        }
        if !self.game.at_bbs() {
            self.bbs = None;
        }
        for a in self.talk.show(&ctx, &self.game, &pal) {
            self.game.apply(a);
        }
        self.sync_hex();
        if let Some(view) = &mut self.bbs {
            for o in view.show(&ctx, &self.game, &pal) {
                match o {
                    Outcome::Act(a) => self.game.apply(a),
                    Outcome::Closed => {
                        self.bbs = None;
                        self.game.say(LogKind::Sys, "NO CARRIER");
                        self.focus_input = true;
                        break;
                    }
                    Outcome::None => {}
                }
            }
        }

        // The chrome's overlay (the suite's dialogs) sits over everything.
        let mut cx = FrameCx {
            game: &self.game,
            pal,
            suite: &mut self.suite,
            keys_free,
        };
        for r in view.overlay(&ctx, &mut cx) {
            self.handle(&ctx, r);
        }

        // Bring the target pane forward when ICE rezzes or the run ends.
        if self.game.phase != self.last_phase {
            if self.game.phase != Phase::Exploring
                && let Some(path) = self.dock.find_tab(&Tab::Target)
            {
                let _ = self.dock.set_active_tab(path);
            }
            if self.game.phase == Phase::Exploring
                && let Some(path) = self.dock.find_tab(&Tab::Network)
            {
                let _ = self.dock.set_active_tab(path);
            }
            self.last_phase = self.game.phase;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_layouts_round_trip() {
        for chrome in Chrome::ALL {
            for extra in [&[][..], &[Tab::Net][..]] {
                let dock = default_layout(chrome, extra);
                let text = ron::to_string(&dock).unwrap();
                let back = parse_layout(&text, |_| true).expect("a default layout reads back");
                let tabs =
                    |d: &DockState<Tab>| d.iter_all_tabs().map(|(_, t)| *t).collect::<Vec<_>>();
                assert_eq!(tabs(&back), tabs(&dock));
                assert_eq!(dock.find_tab(&Tab::Net).is_some(), !extra.is_empty());
            }
        }
    }

    #[test]
    fn panes_you_lack_leave_a_saved_layout() {
        let text = ron::to_string(&default_layout(Chrome::Btop, &[Tab::Net])).unwrap();
        let back = parse_layout(&text, |_| false).expect("core panes are all there");
        assert!(back.find_tab(&Tab::Net).is_none());
        assert!(Tab::CORE.iter().all(|t| back.find_tab(t).is_some()));
    }

    #[test]
    fn every_secondary_pane_is_in_the_catalog() {
        let data = Data::builtin().unwrap();
        for (tab, id) in Tab::SECONDARY {
            assert_eq!(tab.pane_id(), Some(id));
            assert!(
                data.panes.iter().any(|d| d.id == id),
                "{id} has no catalog entry"
            );
        }
        for d in &data.panes {
            assert!(
                Tab::SECONDARY.iter().any(|(_, id)| *id == d.id),
                "pane {} has no renderer",
                d.id
            );
        }
    }

    #[test]
    fn bad_layouts_fall_back() {
        assert!(parse_layout("not a layout", |_| true).is_none());
        // A layout missing a core pane (say, from an older build) is refused.
        let dock = DockState::new(vec![Tab::Network]);
        assert!(parse_layout(&ron::to_string(&dock).unwrap(), |_| true).is_none());
    }

    #[test]
    fn only_secondary_panes_close() {
        for t in Tab::CORE {
            assert!(!t.is_secondary());
        }
        assert!(Tab::Net.is_secondary());
    }
}
