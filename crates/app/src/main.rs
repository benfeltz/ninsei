//! ninsei: a cyberspace hacking roguelike dressed as a retro IDE.

mod bbs_view;
mod btop;
mod chrome;
mod crash;
mod crt;
mod crt_gl;
mod encounter_view;
mod fightview;
mod games_view;
mod glitch;
mod hexview;
mod home;
mod hubtools;
mod kit;
mod liza;
mod netmap;
mod netmap_draft;
mod netmap_iso;
mod netmap_paint;
mod netpane;
mod nightcomm_view;
mod pager;
mod panes;
mod patterns;
mod settings;
mod shopper;
mod sound;
mod stack;
mod suite;
mod system;
mod system_tui;
mod theme;
mod tigerteam_view;
mod tiling;
#[cfg(test)]
mod uitest;
mod vispane;
mod volume;
mod workbench;

use egui::{Key, RichText};
use egui_dock::{DockArea, DockState};
use ninsei_core::bbs::Outcome;
use ninsei_core::game::ExecState;
use ninsei_core::hexedit::HexBuffer;
use ninsei_core::hub::{
    Answer, DEFAULT_RUN_THEME, RunEnd, RunReport, SAVE_KEY, Save, Screen, Tool,
};
use ninsei_core::{Action, Command, Data, Game, LogKind, Phase};

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

/// Sets `window.ninseiTyping` for `web/index.html`'s key filter.
#[cfg(target_arch = "wasm32")]
fn set_typing(on: bool) {
    if let Some(w) = web_sys::window() {
        let _ = js_sys::Reflect::set(&w, &"ninseiTyping".into(), &on.into());
    }
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
/// Adds a secondary pane to a layout once: beside the terminal, or else into the focused leaf.
fn add_pane_to(dock: &mut DockState<Tab>, tab: Tab) {
    if dock.find_tab(&tab).is_some() {
        return;
    }
    match dock.find_tab(&Tab::Terminal) {
        Some(at) if at.surface == egui_dock::SurfaceIndex::main() => {
            dock.main_surface_mut()
                .split_right(at.node, 0.55, vec![tab]);
        }
        _ => dock.push_to_focused_leaf(tab),
    }
}

/// The hex pane is HexEd's: gone until you've downloaded it, back the moment you do.
fn fit_tools(dock: &mut DockState<Tab>, game: &Game) {
    let at = dock.find_tab(&Tab::Hex);
    match (at, game.has_tool("hexed")) {
        (Some(at), false) => {
            dock.remove_tab(at);
        }
        (None, true) => add_pane_to(dock, Tab::Hex),
        _ => {}
    }
}

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

/// Seconds between a running program's passes, so a fight can be watched and paused.
const PASS_SECS: f64 = 0.8;

/// Plays the deck's program while it's `Running`: a pass every `PASS_SECS` (`Action::Play` ran
/// the first). `next` is when the next one is due. Returns the seconds until then, or `None`
/// once the program has stopped running.
fn drive_program(game: &mut Game, now: f64, next: &mut Option<f64>) -> Option<f64> {
    if game.exec.state != ExecState::Running || game.phase != Phase::Combat {
        *next = None;
        return None;
    }
    let due = *next.get_or_insert(now + PASS_SECS);
    if now < due {
        return Some(due - now);
    }
    game.apply(Action::Step);
    *next = Some(now + PASS_SECS);
    drive_program(game, now, next)
}

struct App {
    /// When a running program's next pass is due (`drive_program`).
    next_pass: Option<f64>,
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
    /// The palette egui's visuals were last built from (a chrome can fold the run's).
    applied_pal: Option<theme::Palette>,
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
    /// The run terminal's command line (readline keys, history, completion).
    term: ninsei_term::LineEditor,
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
    /// The run screen's sound pop-out.
    sound_popup: volume::Popup,
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
            speaker: {
                let mut s = sound::Speaker::new(save.sound);
                s.set_volume(save.volume_gain());
                s
            },
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
            next_pass: None,
            first_seed,
            last_at: game.at,
            bbs: None,
            home_bbs: None,
            hex: None,
            hex_uid: 0,
            talk: EncounterView::default(),
            suite: suite::State::default(),
            sound_popup: volume::Popup::default(),
            last_phase: game.phase,
            game,
            dock: default_layout(Chrome::Classic, &[]),
            run_theme,
            pal,
            crt_gl: CrtGl::new(),
            term: ninsei_term::LineEditor::new(ninsei_core::command::HISTORY_KEEP),
            focus_input: true,
            layout_checked: 0.0,
            glitch: glitch::Glitch::default(),
            applied_pal: None,
        };
        me.load_layout();
        let history = me.save.term_history.clone();
        me.term.set_history(history);
        let history = me.save.shell_history.clone();
        me.home.set_shell_history(history);
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
        // A layout without HexEd's pane isn't a whole one: keep the last that was.
        if now - self.layout_checked < LAYOUT_CHECK_SECS || self.dock.find_tab(&Tab::Hex).is_none()
        {
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
        add_pane_to(&mut self.dock, tab);
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
        self.applied_pal = Some(self.pal);
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
        let before = (self.save.monitor.clone(), self.save.layout_key(&self.data));
        let msg = change(&mut self.save, &self.data)?;
        self.after_hardware(ctx, before);
        Ok(msg)
    }

    /// Re-applies the look after the monitor, deck or firmware changed from `before` (the
    /// monitor and the layout key): the deck's layout, the theme, and the CRT for a new tube.
    fn after_hardware(&mut self, ctx: &egui::Context, before: (String, String)) {
        self.save_dirty = true;
        if self.save.layout_key(&self.data) != before.1 {
            self.load_layout();
        }
        let theme = self.save.phosphor.clone();
        self.apply_theme(ctx, &theme);
        if self.save.monitor != before.0 && !self.crt {
            self.toggle_crt();
        }
    }

    /// Finishes a setting `settings::apply` put on the save: repaints for it and says so.
    fn settled(
        &mut self,
        ctx: &egui::Context,
        applied: settings::Applied,
        before: (String, String),
    ) {
        use settings::Applied;
        match applied {
            Applied::Said(msg) => {
                self.save_dirty = true;
                self.home.say(msg);
            }
            Applied::Theme(label) => {
                let id = self.save.phosphor.clone();
                self.apply_theme(ctx, &id);
                self.home.say(format!("theme: {label}"));
            }
            Applied::Hardware(msg) => {
                self.after_hardware(ctx, before);
                self.home.say(msg);
            }
            Applied::Refused(e) => self.home.say(e),
            Applied::Other(_) => {}
        }
    }

    /// Orders an item from CPU Shopper. Core charges and grants it through the item's own rules;
    /// here only the side effects: new hardware re-applies the look, a new pane goes on the
    /// run screen.
    fn order(&mut self, ctx: &egui::Context, item: ninsei_core::shopper::Item) {
        use ninsei_core::shopper::Item;
        let r = match &item {
            Item::Monitor(_) | Item::Deck(_) => self.hardware(ctx, |s, d| s.buy_item(d, &item)),
            _ => {
                let r = self.save.buy_item(&self.data, &item);
                self.save_dirty = true;
                r
            }
        };
        let bought = r.is_ok();
        self.home.say(r.unwrap_or_else(|e| e));
        // A pane you just bought shows up on this deck's run screen at once
        // (other decks: View > add pane).
        if bought
            && let Item::Pane(id) = &item
            && let Some(&(tab, _)) = Tab::SECONDARY.iter().find(|(_, t)| t == id)
        {
            self.add_pane(tab);
            if let Ok(text) = ron::to_string(&self.dock) {
                self.save.set_layout(&self.data, text);
            }
            self.home
                .say("it's on your run screen now (other decks: View > add pane).");
        }
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
        // Hung up mid-handshake: the modem goes quiet with it.
        if std::mem::take(&mut self.home.dial_cut) {
            self.speaker.stop();
        }
        // A cracktro started or was skipped: its tune goes with it.
        match self.home.tune.take() {
            Some(home::TuneCue::Play(id)) => {
                if let Some(def) = self.data.intro(&id)
                    && let Some(tune) = &def.tune
                {
                    self.speaker.tune(tune, def.secs);
                }
            }
            Some(home::TuneCue::Stop) => self.speaker.stop_tune(),
            None => {}
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
        // The exit's keep picker starts with the loadout ticked: it comes home with you.
        self.keep = self.game.loadout_keep();
        self.last_at = self.game.at;
        self.last_phase = self.game.phase;
        self.bbs = None;
        self.hex = None;
        self.talk = EncounterView::default();
        self.sound_popup.close();
        self.term.clear();
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
            if let Some(line) = unpacked(&r) {
                self.home.say(line);
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
                Outcome::Act(ninsei_core::Action::Read(r)) => {
                    self.save.home_read(&self.data, board, r)
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
                    Ok(msg) if msg.is_empty() => {}
                    Ok(msg) => self.home.say(msg),
                    Err(e) => view.session.status = Some(format!("|12{e}")),
                }
            }
        }
    }

    /// Applies a hub tool's request to the save and reports the result in the Shell.
    fn home_action(&mut self, o: WbOut) {
        let result = match o {
            WbOut::Install(at) => self.save.install(&self.data, &at),
            WbOut::Remove(at) => self.save.remove(&at),
            WbOut::Defragment => Ok(self.save.defragment()),
            WbOut::Scrap(id) => Ok(self
                .save
                .scrap(id)
                .map_or(String::new(), |_| "Scrapped.".to_string())),
            WbOut::Patch(id, bin) => self.save.patch(id, &bin),
            WbOut::Optimize(id) => self.save.optimize(id),
            WbOut::Scan(range, from) => match self.save.wardial(&self.data, &range) {
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
                    self.home.show_scan(res, &self.save, from);
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
                        let msg = if d.call_cost == 0 {
                            format!("CONNECT {} to {}. A local call: free.", d.baud, d.name)
                        } else {
                            format!(
                                "CONNECT {} to {}. Phone bill -{}¢.",
                                d.baud, d.name, d.call_cost
                            )
                        };
                        self.home_bbs = Some((view, call));
                        Ok(msg)
                    }
                    Err(e) => Err(format!("NO CARRIER: {e}")),
                }
            }
            WbOut::Dial(..)
            | WbOut::At(..)
            | WbOut::Theme(_)
            | WbOut::Crt
            | WbOut::Sound
            | WbOut::Glitches(_)
            | WbOut::Intros(_)
            | WbOut::GuideSeen
            | WbOut::GameStarted(_)
            | WbOut::SawIntro(_)
            | WbOut::HelpSeen
            | WbOut::Tiling(_)
            | WbOut::ShellHistory(_)
            | WbOut::Monitor(_)
            | WbOut::Deck(_)
            | WbOut::Firmware(_)
            | WbOut::Order(_) => {
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

    /// A line typed at NiGHTCOMM's TERM: the modem answers through core's `at::run`, and
    /// whatever it asks for happens here.
    fn modem(&mut self, from: Tool, line: &str) {
        use ninsei_core::at::Effect;
        let asking = self.home.term_for(from).asking.take();
        let reply = ninsei_core::at::run(&mut self.save, &self.data, line, asking);
        self.save_dirty = true;
        self.home.term_for(from).term_say(&reply.lines);
        match reply.effect {
            Effect::None => {}
            Effect::Ask(entry) => self.home.term_for(from).asking = Some(entry),
            Effect::Dial(entry) => self.home.start_dial_here(entry),
            Effect::Call(board) => {
                let id = self.data.boards[board].id.clone();
                self.home_action(WbOut::Call(id));
            }
            Effect::Hang => self.home.hang_up(),
            Effect::Speaker(on) => {
                if self.save.sound != on {
                    self.toggle_sound();
                }
            }
        }
    }

    /// Back to the desktop: it drags up over the run.
    fn go_home(&mut self) {
        self.book();
        self.bbs = None;
        self.sound_popup.close();
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
            Request::Say(text) => {
                self.game.say(LogKind::Sys, text);
                return;
            }
            Request::Volume(pct) => {
                self.save.set_volume(pct);
                self.speaker.set_volume(self.save.volume_gain());
                self.save_dirty = true;
                return;
            }
            Request::SoundOn(on) => {
                if self.save.sound != on {
                    self.toggle_sound();
                }
                return;
            }
            Request::Type(text) => {
                self.term.set_text(&text);
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
                // The history comes home with the save, so ↑ still knows your last commands.
                if self.save.term_history != self.term.history() {
                    self.save.term_history = self.term.history().to_vec();
                    self.save_dirty = true;
                }
                match ninsei_core::command::parse_in(&self.game, &line) {
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
            Command::Print(lines) => {
                for l in lines {
                    self.game.say(LogKind::Sys, l);
                }
            }
            Command::Hex(slot) => self.open_hex(slot),
            Command::Help => {
                let lines: Vec<&str> = ninsei_core::command::help(&self.game).collect();
                for l in lines {
                    self.game.say(LogKind::Sys, l);
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
                self.game.say(LogKind::Sys, format!("sound {state}"));
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
        if !self.game.has_tool("hexed") {
            return;
        }
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
                            RichText::new("every pane you own is open. more in CPU Shopper.")
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
                // The sound switch lives in the sound pop-out (`MenuPart::Sound`).
            }
            MenuPart::Sound => {
                let v = chrome::view(self.save.firmware_def(&self.data).chrome);
                let free = !self.modal_up();
                if ui
                    .add_enabled(free, egui::Button::new(v.sound_item()))
                    .clicked()
                {
                    self.sound_popup.open(volume::Level::of(&self.save));
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

    /// Whether something already has the keyboard (a BBS, a conversation, a suite dialog
    /// waiting to be dismissed), so the sound pop-out can't open over it.
    fn modal_up(&self) -> bool {
        self.bbs.is_some() || self.game.encounter.is_some() || !self.suite.dialogs.is_empty()
    }

    fn status_bar(&mut self, ui: &mut egui::Ui, pal: &theme::Palette) -> Vec<Request> {
        let theme_label = self.run_theme_label();
        let level = volume::Level::of(&self.save);
        let keys_free = !self.modal_up();
        let g = &self.game;
        let v = chrome::view(self.save.firmware_def(&self.data).chrome);
        let mut out = Vec::new();
        if v.status(ui, pal, g) {
            return out;
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
            let mut cx = FrameCx {
                game: g,
                pal: *pal,
                suite: &mut self.suite,
                keys_free,
                level,
                popup: &mut self.sound_popup,
            };
            out.extend(v.status_extra(ui, &mut cx));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new("UTF-8  LF  RUST").color(pal.dim));
                ui.label(RichText::new(theme_label).color(pal.dim));
            });
        });
        out
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // Tell the page whether something is taking typing, so it can keep the browser's own
        // Ctrl chords (Ctrl+R, Ctrl+L, ...) off a terminal (web/index.html).
        #[cfg(target_arch = "wasm32")]
        set_typing(ctx.egui_wants_keyboard_input());
        let pal = self.pal;
        let dt = ctx.input(|i| i.stable_dt).min(0.1);
        self.screen = self.screen.tick(dt);
        self.home.drop_closed_intros(ctx.cumulative_pass_nr());
        self.speaker.tick(ctx.input(|i| i.time));
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
        // The run's tune: core says what should be on; the speaker renders a new loop a slice a
        // frame (keep frames coming until it's ready) and only rearranges on a trace step.
        match ninsei_core::music::wanted(
            self.screen,
            self.crashed,
            &self.save,
            &self.game,
            &self.data,
        ) {
            Some(w) => {
                if self.speaker.run_music(&w) {
                    ctx.request_repaint();
                }
            }
            None => self.speaker.stop_music(),
        }

        let live = self.screen == Screen::Run && !self.crashed;
        if live
            && let Some(wait) =
                drive_program(&mut self.game, ctx.input(|i| i.time), &mut self.next_pass)
        {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(wait));
        }
        self.glitch = glitch::Glitch::frame(&ctx, &self.game, self.save.glitches, live);
        if self.glitch.busy() {
            ctx.request_repaint();
        }
        // The chrome can fold the run's colours (a 1-bit chrome draws in two); the desktop
        // keeps the theme's.
        let pal = if self.screen.shows_run() && !self.crashed {
            chrome::view(self.save.firmware_def(&self.data).chrome).palette(pal)
        } else {
            pal
        };
        if self.applied_pal != Some(pal) {
            theme::apply(&ctx, &pal);
            self.applied_pal = Some(pal);
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
            self.hub_ui(&ctx, &pal);
        }

        // Damage glitches go on top of the preset, for the shader only: the mouse keeps the
        // preset's own geometry.
        match self.effects() {
            Some(e) if !self.crt_gl.failed() => {
                self.glitch.overlay(&ctx, &pal, true);
                let (mut e, tear) = self.glitch.effects(e);
                if chrome::view(self.save.firmware_def(&self.data).chrome).one_bit() {
                    // A 1-bit screen has no colours to split.
                    e.rgb_shift = 0.0;
                }
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

/// The run's keys this frame: the chrome's own first (an alert's Enter and Esc, which it takes),
/// then the shared hotkeys with what's left.
fn run_keys(
    view: &dyn chrome::ChromeView,
    ctx: &egui::Context,
    cx: &mut FrameCx,
    modal: bool,
    focus_input: &mut bool,
) -> Vec<Request> {
    let mut out = view.keys(ctx, cx);
    let script = view.script_has_keys(ctx);
    out.extend(hotkeys(ctx, cx.game, modal, script, focus_input));
    out
}

/// Hotkeys work whenever the terminal isn't taking typing: Space ends the turn, a digit runs
/// that slot, and Enter or / go to the Message box. A Script pane holding the keyboard
/// (`script`) isn't typing: the hotbar's keys still work over it.
fn hotkeys(
    ctx: &egui::Context,
    game: &Game,
    modal: bool,
    script: bool,
    focus_input: &mut bool,
) -> Vec<Request> {
    let hex_typing = ctx.memory(|m| m.has_focus(hexview::grid_id()));
    let talking = game.encounter.is_some();
    let typing = ctx.egui_wants_keyboard_input() && !script;
    if modal || talking || hex_typing || typing {
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
            *focus_input = true;
        }
    });
    out
}

impl App {
    /// The desktop and anything called up over it (a board dialled from home), with what they
    /// send applied: on its own while home, dragging during the transitions.
    fn hub_ui(&mut self, ctx: &egui::Context, pal: &theme::Palette) {
        let (ctx, pal) = (ctx.clone(), *pal);
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
                // A game lives while any desktop still has its window open.
                let (desktops, save) = (&self.desktops, &self.save);
                self.home
                    .keep_open(ui.input(|i| i.time), |t| desktops.is_open(save, t));
                let desk = self.save.firmware_def(&self.data).desktop;
                let view = self.desktops.get(desk);
                // A board called from home owns the keyboard while it's up.
                let keys = self.home_bbs.is_none();
                outs = self
                    .home
                    .show(view, ui, &self.data, &self.save, offset, live, keys);
            });
        for o in outs {
            // Settings go on the save first (tested in `settings`); then the repaint.
            let before = (self.save.monitor.clone(), self.save.layout_key(&self.data));
            let o = match settings::apply(&mut self.save, &self.data, o) {
                settings::Applied::Other(o) => o,
                applied => {
                    self.settled(&ctx, applied, before);
                    continue;
                }
            };
            match o {
                WbOut::Dial(entry, seed) if live => self.dial(entry, seed),
                WbOut::Dial(..) => {}
                WbOut::HelpSeen => {
                    self.save.tiling_help_seen = true;
                    self.save_dirty = true;
                }
                WbOut::Tiling(t) => {
                    self.save.tiling = t;
                    self.save_dirty = true;
                }
                WbOut::ShellHistory(h) => {
                    if self.save.shell_history != h {
                        self.save.shell_history = h;
                        self.save_dirty = true;
                    }
                }
                WbOut::Order(item) => self.order(&ctx, item),
                WbOut::GuideSeen => {
                    self.save.guide_seen = true;
                    self.save_dirty = true;
                }
                WbOut::GameStarted(id) => {
                    self.save.start_game(&id);
                    self.save_dirty = true;
                }
                WbOut::SawIntro(id) => {
                    // Core keeps what's been seen; only a first play changes the save.
                    let seen = self.save.intros_seen.len();
                    self.save.saw_intro(&self.data, &id);
                    self.save_dirty |= self.save.intros_seen.len() != seen;
                }
                WbOut::Crt => {
                    self.toggle_crt();
                    self.home
                        .say(format!("crt {}", if self.crt { "on" } else { "off" }));
                }
                WbOut::Sound => {
                    let state = self.toggle_sound();
                    self.home.say(format!("sound {state}"));
                }
                WbOut::At(from, line) => self.modem(from, &line),
                other => self.home_action(other),
            }
        }
        if live {
            self.home_bbs_ui(&ctx, &pal);
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }

    /// The run's IDE: menu, status bar, dock panes and the run's modal windows.
    fn run_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, pal: &theme::Palette) {
        let pal = *pal;
        let ctx = ctx.clone();
        // A BBS or a conversation has the keyboard: the sound pop-out gives way to it.
        let sound_was_open = self.sound_popup.is_open();
        if self.modal_up() {
            self.sound_popup.close();
        }
        // Keys only drive the run once it's fully on screen.
        let view = chrome::view(self.save.firmware_def(&self.data).chrome);
        let keys_free =
            self.bbs.is_none() && self.game.encounter.is_none() && !self.sound_popup.is_open();
        let level = volume::Level::of(&self.save);
        // A BBS, a conversation or the sound pop-out has the keyboard: no hotkeys.
        let modal = self.bbs.is_some() || self.sound_popup.is_open();
        let mut requests = if self.screen == Screen::Run {
            let mut cx = FrameCx {
                game: &self.game,
                pal,
                suite: &mut self.suite,
                keys_free,
                level,
                popup: &mut self.sound_popup,
            };
            run_keys(view, &ctx, &mut cx, modal, &mut self.focus_input)
        } else {
            Vec::new()
        };

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
                    .fill(view.status_fill(&pal))
                    .inner_margin(egui::Margin::symmetric(6, 1)),
            )
            .show(ui, |ui| requests.extend(self.status_bar(ui, &pal)));
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
                        level,
                        popup: &mut self.sound_popup,
                    };
                    requests.extend(view.legend(ui, &mut cx));
                });
        }

        fit_tools(&mut self.dock, &self.game);
        // The vis pane's samples: taken only by a vis pane that's drawn (a tab behind another
        // isn't), which asks for frames only while it has something to show.
        let speaker = &self.speaker;
        let hear = || speaker.listen(vispane::WINDOW);
        let scope_rate = self.speaker.rate().unwrap_or(0);
        let mut panes = Panes {
            game: &self.game,
            pal,
            map: self.save.firmware_def(&self.data).map,
            deck: self.save.deck_def(&self.data).title(),
            chrome: self.save.firmware_def(&self.data).chrome,
            corrupt: self.glitch.corruption(),
            term: &mut self.term,
            focus_input: &mut self.focus_input,
            // Any modal (BBS, talk, the sound pop-out) keeps the terminal from grabbing focus.
            bbs_open: self.bbs.is_some()
                || self.game.encounter.is_some()
                || self.sound_popup.is_open(),
            hex: &mut self.hex,
            keep: &mut self.keep,
            keep_room: (self.save.stash_cap(&self.data.rig) as usize)
                .saturating_sub(self.save.stash.len()),
            suite: &mut self.suite,
            hear: &hear,
            scope_rate,
            out: Vec::new(),
        };
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(pal.bg))
            .show(ui, |ui| {
                DockArea::new(&mut self.dock)
                    .style(view.dock_style(theme::dock_style(ui.style(), &pal), &pal))
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
        let talking = self.game.encounter.is_some();
        for a in self.talk.show(&ctx, &self.game, &pal) {
            self.game.apply(a);
        }
        // A conversation that just ended hands the keyboard back to the command line.
        if talking && self.game.encounter.is_none() {
            self.focus_input = true;
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

        // The chrome's overlay (the suite's dialogs) sits over everything, and the sound
        // pop-out over that.
        let mut cx = FrameCx {
            game: &self.game,
            pal,
            suite: &mut self.suite,
            keys_free,
            level: volume::Level::of(&self.save),
            popup: &mut self.sound_popup,
        };
        let mut over = view.overlay(&ctx, &mut cx);
        if cx.popup.is_open() {
            over.extend(view.sound(&ctx, &mut cx));
        }
        for r in over {
            self.handle(&ctx, r);
        }
        // However it closed (its own keys, the speaker, a modal coming up), the command line
        // has the keys again.
        if sound_was_open && !self.sound_popup.is_open() {
            self.focus_input = true;
        }

        // Bring the fight forward when ICE rezzes (the chrome says where), and the target pane
        // when a message comes in or the run ends.
        if self.game.phase != self.last_phase {
            let tab = if self.game.phase == Phase::Combat {
                view.combat_tab()
            } else {
                Tab::Target
            };
            if self.game.phase != Phase::Exploring
                && let Some(path) = self.dock.find_tab(&tab)
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

/// The Shell's line for what came home off a run's boards: the tools by name, then the games
/// by their crack's label (`Unpacked: HexEd, BLOX [PHS]`).
fn unpacked(r: &ninsei_core::hub::RunReport) -> Option<String> {
    let all: Vec<&str> = r.tools.iter().chain(&r.games).map(String::as_str).collect();
    (!all.is_empty()).then(|| format!("Unpacked: {}", all.join(", ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_running_program_plays_a_pass_a_beat_and_pause_stops_it() {
        let mut g = Game::new(Data::builtin().unwrap(), 7);
        let target = g.net.nodes[g.at].links[0];
        g.net.nodes[target].kind = ninsei_core::NodeKind::Ice;
        g.net.nodes[target].ice = Some(1);
        g.net.nodes[target].cleared = false;
        g.apply(Action::Move(target));
        let c = g.combat.as_mut().unwrap();
        (c.strength, c.strength_max) = (500, 500);
        g.apply(Action::Program("10 RUN icepick\n20 GOTO 10".into()));
        let turn = |g: &Game| g.combat.as_ref().unwrap().turn;
        let mut next = None;
        assert_eq!(
            drive_program(&mut g, 0.0, &mut next),
            None,
            "paused: nothing runs"
        );
        g.apply(Action::Play);
        assert_eq!(turn(&g), 2, "play runs the first pass");
        assert!(drive_program(&mut g, 0.0, &mut next).is_some());
        assert!(drive_program(&mut g, PASS_SECS / 2.0, &mut next).is_some());
        assert_eq!(turn(&g), 2, "not yet: a pass a beat");
        drive_program(&mut g, PASS_SECS, &mut next);
        assert_eq!(turn(&g), 3);
        g.apply(Action::Pause);
        assert_eq!(drive_program(&mut g, PASS_SECS * 5.0, &mut next), None);
        assert_eq!(turn(&g), 3, "paused between passes");
    }

    #[test]
    fn a_board_called_from_home_takes_the_keyboard_from_a_game() {
        use crate::uitest::{Harness, key};
        use ninsei_core::hub::GameCopy;
        let mut h = Harness::new(1280.0, 800.0);
        let cc = eframe::CreationContext::_new_kittest(h.ctx.clone());
        let mut app = App::new(&cc, Data::builtin().unwrap(), Some(7));
        app.save.decks.push("onosendai".into());
        app.save.deck = "onosendai".into();
        app.save.firmware = app.save.deck_def(&app.data).firmware[0].id.clone();
        app.save.tiling_help_seen = true;
        app.save.intros = false;
        app.save.learn_game("minefield", GameCopy::Cracked);
        app.load_layout();
        let mine = Tool::game("minefield", GameCopy::Cracked).unwrap();
        app.home.open(mine);
        let frame = |h: &mut Harness, app: &mut App, ev: Vec<egui::Event>| {
            let pal = app.pal;
            h.frame(ev, |ui| app.hub_ui(ui.ctx(), &pal));
        };
        for _ in 0..3 {
            frame(&mut h, &mut app, Vec::new());
        }
        assert_eq!(app.save.tiling.focused(), Some(mine));
        let at = app.home.game(mine).unwrap().cursor();
        frame(&mut h, &mut app, key(egui::Key::ArrowRight));
        let moved = app.home.game(mine).unwrap().cursor();
        assert_ne!(moved, at, "the focused tile has the keys");
        app.home_action(WbOut::Call("sensenet".into()));
        assert!(app.home_bbs.is_some(), "the call is up");
        frame(&mut h, &mut app, key(egui::Key::ArrowRight));
        frame(&mut h, &mut app, key(egui::Key::Space));
        let g = app.home.game(mine).unwrap();
        assert_eq!(g.cursor(), moved, "the board has the keys, not the game");
        assert!(g.paused(), "and the game waits behind it");
        // Esc goes down the line, never to AnarchyOS's leader: the keys after it aren't the
        // desktop's (a workspace switch, closing the tile, the launcher).
        let tiling = app.save.tiling.clone();
        for k in [egui::Key::Num2, egui::Key::X, egui::Key::Space] {
            frame(&mut h, &mut app, key(egui::Key::Escape));
            frame(&mut h, &mut app, key(k));
        }
        assert_eq!(app.save.tiling, tiling, "the tiles are as they were");
        assert!(app.home_bbs.is_some(), "still on the call");
    }

    #[test]
    fn a_game_open_on_one_desktop_survives_a_switch_of_firmware_and_back() {
        use crate::uitest::Harness;
        use ninsei_core::hub::GameCopy;
        let mut h = Harness::new(1280.0, 800.0);
        let cc = eframe::CreationContext::_new_kittest(h.ctx.clone());
        let mut app = App::new(&cc, Data::builtin().unwrap(), Some(7));
        app.save.decks.push("onosendai".into());
        app.save.guide_seen = true;
        app.save.tiling_help_seen = true;
        app.save.intros = false;
        app.save.learn_game("nibbler", GameCopy::Cracked);
        let use_deck = |app: &mut App, deck: &str| {
            app.save.deck = deck.into();
            app.save.firmware = app.save.deck_def(&app.data).firmware[0].id.clone();
        };
        use_deck(&mut app, "sensenet");
        let nib = Tool::game("nibbler", GameCopy::Cracked).unwrap();
        app.home.open(nib);
        let frames = |h: &mut Harness, app: &mut App, n: usize| {
            for _ in 0..n {
                let pal = app.pal;
                h.frame(Vec::new(), |ui| app.hub_ui(ui.ctx(), &pal));
            }
        };
        frames(&mut h, &mut app, 3);
        let round = app.home.round(nib).expect("Nibbler plays on Liza");
        assert_eq!(app.save.game_round("nibbler"), 1);
        use_deck(&mut app, "onosendai");
        frames(&mut h, &mut app, 3);
        use_deck(&mut app, "sensenet");
        frames(&mut h, &mut app, 3);
        assert_eq!(app.home.round(nib), Some(round), "the same round");
        assert_eq!(app.save.game_round("nibbler"), 1, "no new round dealt");
    }

    #[test]
    fn games_downloaded_on_a_run_are_unpacked_with_the_tools() {
        let mut r = ninsei_core::hub::RunReport {
            end: RunEnd::Exit,
            credits: 0,
            banked: 0,
            hops: 1,
            seed: 0,
            kept: Vec::new(),
            learned: Vec::new(),
            boards: Vec::new(),
            tools: Vec::new(),
            games: Vec::new(),
        };
        assert_eq!(unpacked(&r), None);
        r.games = vec!["BLOX [PHS]".into()];
        assert_eq!(unpacked(&r).as_deref(), Some("Unpacked: BLOX [PHS]"));
        r.tools = vec!["HexEd".into()];
        assert_eq!(unpacked(&r).as_deref(), Some("Unpacked: HexEd, BLOX [PHS]"));
    }

    mod sound_pop_out {
        use super::*;
        use crate::uitest::{Harness, click, key, text};

        /// The app in a run on `deck`, the command line focused, nothing playing (only
        /// `run_ui` runs here, and it never opens the speaker).
        fn running(deck: &str) -> (Harness, App) {
            let h = Harness::new(1280.0, 800.0);
            let cc = eframe::CreationContext::_new_kittest(h.ctx.clone());
            let mut app = App::new(&cc, Data::builtin().unwrap(), Some(7));
            if !app.save.decks.iter().any(|d| d == deck) {
                app.save.decks.push(deck.into());
            }
            app.save.deck = deck.into();
            app.save.firmware = app.save.deck_def(&app.data).firmware[0].id.clone();
            app.load_layout();
            let entry = app.save.phonebook[0].id;
            app.dial(entry, Some(7));
            app.screen = Screen::Run;
            (h, app)
        }

        fn frame(h: &mut Harness, app: &mut App, events: Vec<egui::Event>) {
            let pal = app.pal;
            h.frame(events, |ui| {
                let ctx = ui.ctx().clone();
                app.run_ui(ui, &ctx, &pal);
            });
        }

        fn typed(h: &mut Harness, app: &mut App, s: &str) {
            frame(h, app, text(s));
            frame(h, app, Vec::new());
        }

        fn press(h: &mut Harness, app: &mut App, what: &str) {
            let at = h
                .find(what)
                .unwrap_or_else(|| panic!("no {what:?} in {:?}", h.texts()))
                .center();
            frame(h, app, click(at));
            frame(h, app, Vec::new());
        }

        #[test]
        fn a_slow_frames_command_runs_whole_on_the_run_terminal() {
            let (mut h, mut app) = running("onosendai");
            for _ in 0..3 {
                frame(&mut h, &mut app, Vec::new());
            }
            // The keys of `help` and Enter land in one frame.
            let mut ev = text("h");
            ev.extend(text("elp"));
            ev.extend(key(egui::Key::Enter));
            frame(&mut h, &mut app, ev);
            assert_eq!(app.term.history().last().map(String::as_str), Some("help"));
            assert_eq!(app.term.text(), "");
        }

        #[test]
        fn esc_in_a_slow_frame_keeps_the_typing_before_it() {
            let (mut h, mut app) = running("onosendai");
            for _ in 0..3 {
                frame(&mut h, &mut app, Vec::new());
            }
            let before = app.game.log.len();
            // `a b` and Esc in one frame: the line has them, and the run's hotkeys
            // (Space ends the turn) don't.
            let mut ev = text("a");
            ev.extend(key(egui::Key::Space));
            ev.extend(text(" "));
            ev.extend(text("b"));
            ev.extend(key(egui::Key::Escape));
            frame(&mut h, &mut app, ev);
            assert_eq!(app.term.text(), "a b", "the command line kept its keys");
            assert_eq!(
                app.game.log.len(),
                before,
                "the run's hotkeys heard none of them"
            );
            // The Esc let go of the keyboard: now Space is the run's.
            frame(&mut h, &mut app, key(egui::Key::Space));
            assert!(app.game.log.len() > before, "Space ends the turn again");
        }

        #[test]
        fn it_takes_the_keys_from_the_command_line_and_gives_them_back() {
            let (mut h, mut app) = running("onosendai");
            for _ in 0..3 {
                frame(&mut h, &mut app, Vec::new());
            }
            typed(&mut h, &mut app, "ab");
            assert_eq!(app.term.text(), "ab", "the command line has the keys");
            // Open it from the speaker: its keys are its own.
            press(&mut h, &mut app, "🔊 100%");
            assert!(app.sound_popup.is_open());
            let mut ev = text("m");
            ev.extend(key(egui::Key::M));
            frame(&mut h, &mut app, ev);
            frame(&mut h, &mut app, key(egui::Key::ArrowLeft));
            frame(&mut h, &mut app, key(egui::Key::Num1));
            assert!(!app.save.sound, "m muted");
            assert_eq!(app.save.volume, 95);
            assert_eq!(app.term.text(), "ab", "the command line heard none of it");
            assert_eq!(app.game.hops, 0, "nor did the run's hotkeys");
            // Esc closes it and the command line has the keys again.
            frame(&mut h, &mut app, key(egui::Key::Escape));
            assert!(!app.sound_popup.is_open());
            typed(&mut h, &mut app, "c");
            assert_eq!(app.term.text(), "abc");
            // So does closing it from the speaker.
            press(&mut h, &mut app, "🔇 muted");
            assert!(app.sound_popup.is_open());
            press(&mut h, &mut app, "🔇 muted");
            assert!(!app.sound_popup.is_open());
            typed(&mut h, &mut app, "d");
            assert_eq!(app.term.text(), "abcd");
        }

        #[test]
        fn it_never_opens_over_a_bbs_and_gives_way_to_one() {
            let (mut h, mut app) = running("onosendai");
            for _ in 0..3 {
                frame(&mut h, &mut app, Vec::new());
            }
            let speaker = h.find("🔊 100%").unwrap().center();
            // A board on the line (held there: off a BBS node the run hangs it up).
            let on_the_line = |h: &mut Harness, app: &mut App, ev: Vec<egui::Event>| {
                app.bbs.get_or_insert_with(BbsView::new);
                frame(h, app, ev);
            };
            on_the_line(&mut h, &mut app, Vec::new());
            on_the_line(&mut h, &mut app, click(speaker));
            assert!(!app.sound_popup.is_open(), "the BBS has the keyboard");
            // Open, then a board picks up: the pop-out closes.
            app.bbs = None;
            frame(&mut h, &mut app, click(speaker));
            assert!(app.sound_popup.is_open());
            on_the_line(&mut h, &mut app, Vec::new());
            assert!(!app.sound_popup.is_open());
        }

        #[test]
        fn a_bought_sound_scope_is_on_the_run_screen() {
            let (mut h, mut app) = running("onosendai");
            app.save.bank = 1000;
            assert!(app.dock.find_tab(&Tab::Vis).is_none(), "not owned yet");
            let ctx = h.ctx.clone();
            app.order(&ctx, ninsei_core::shopper::Item::Pane("vis".into()));
            assert!(app.save.has_pane(&app.data, "vis"));
            assert!(
                app.dock.find_tab(&Tab::Vis).is_some(),
                "on the run screen at once"
            );
            // Brought forward, the run screen draws it.
            let path = app.dock.find_tab(&Tab::Vis).unwrap();
            let _ = app.dock.set_active_tab(path);
            frame(&mut h, &mut app, Vec::new());
            frame(&mut h, &mut app, Vec::new());
            assert!(app.dock.find_tab(&Tab::Vis).is_some());
            // And this deck's layout keeps it.
            assert!(app.extra_panes().contains(&Tab::Vis));
        }

        #[test]
        fn the_suite_s_dialogs_come_one_at_a_time() {
            let (mut h, mut app) = running("hosaka");
            for _ in 0..3 {
                frame(&mut h, &mut app, Vec::new());
            }
            let notice = || ninsei_core::suite::Notice {
                title: "Notice".into(),
                lines: vec!["Filed.".into()],
            };
            // A notice waiting: Sound... stands down.
            app.suite.dialogs.push(notice());
            assert!(app.modal_up());
            // The pop-out open, then a notice arrives: the pop-out gives way to it.
            app.suite.dialogs.clear();
            assert!(!app.modal_up());
            app.sound_popup.open(volume::Level::of(&app.save));
            frame(&mut h, &mut app, Vec::new());
            assert!(app.sound_popup.is_open());
            app.suite.dialogs.push(notice());
            frame(&mut h, &mut app, Vec::new());
            assert!(!app.sound_popup.is_open());
            assert_eq!(
                app.suite.dialogs.len(),
                1,
                "the notice is the one dialog up"
            );
        }
    }

    #[test]
    fn default_layouts_round_trip() {
        for chrome in Chrome::ALL {
            for extra in [&[][..], &[Tab::Net][..], &[Tab::Net, Tab::Vis][..]] {
                let dock = default_layout(chrome, extra);
                let text = ron::to_string(&dock).unwrap();
                let back = parse_layout(&text, |_| true).expect("a default layout reads back");
                let tabs =
                    |d: &DockState<Tab>| d.iter_all_tabs().map(|(_, t)| *t).collect::<Vec<_>>();
                assert_eq!(tabs(&back), tabs(&dock));
                assert_eq!(dock.find_tab(&Tab::Net).is_some(), !extra.is_empty());
                assert_eq!(
                    dock.find_tab(&Tab::Vis).is_some(),
                    extra.contains(&Tab::Vis),
                    "{chrome:?}"
                );
            }
        }
    }

    #[test]
    fn the_script_is_in_every_chromes_layout_and_in_front_on_the_stack() {
        for chrome in Chrome::ALL {
            assert!(
                default_layout(chrome, &[]).find_tab(&Tab::Script).is_some(),
                "{chrome:?}"
            );
        }
        let dock = default_layout(Chrome::Stack, &[]);
        let at = |t: Tab| dock.find_tab(&t).unwrap();
        let script = at(Tab::Script);
        assert_eq!(script.tab.0, 0, "Script is the front tab");
        assert_eq!(at(Tab::Todo).node, script.node, "To Do behind it");
        // Its own column: not with the card, the deck, the programs or the Message box.
        for t in [
            Tab::Network,
            Tab::Target,
            Tab::Deck,
            Tab::Warez,
            Tab::Terminal,
        ] {
            assert_ne!(at(t).node, script.node, "{t:?}");
        }
        assert_eq!(
            at(Tab::Target).node,
            at(Tab::Network).node,
            "ICE is a card of the centre"
        );
        assert_ne!(
            at(Tab::Deck).node,
            at(Tab::Warez).node,
            "Deck over Programs"
        );
    }

    #[test]
    fn a_bought_pane_joins_a_saved_layout_once() {
        for chrome in Chrome::ALL {
            // A layout saved before the pane was bought.
            let mut dock = default_layout(chrome, &[]);
            assert!(dock.find_tab(&Tab::Net).is_none());
            add_pane_to(&mut dock, Tab::Net);
            assert!(dock.find_tab(&Tab::Net).is_some(), "{chrome:?}");
            let tabs =
                |d: &DockState<Tab>| d.iter_all_tabs().filter(|(_, t)| **t == Tab::Net).count();
            add_pane_to(&mut dock, Tab::Net);
            assert_eq!(tabs(&dock), 1, "only once");
            // And it reads back.
            let text = ron::to_string(&dock).unwrap();
            assert!(
                parse_layout(&text, |_| true)
                    .unwrap()
                    .find_tab(&Tab::Net)
                    .is_some()
            );
        }
    }

    #[test]
    fn panes_you_lack_leave_a_saved_layout() {
        let text = ron::to_string(&default_layout(Chrome::Btop, &[Tab::Net, Tab::Vis])).unwrap();
        let back = parse_layout(&text, |_| false).expect("core panes are all there");
        assert!(back.find_tab(&Tab::Net).is_none());
        assert!(back.find_tab(&Tab::Vis).is_none());
        // Owning one keeps that one.
        let back = parse_layout(&text, |t| t == Tab::Vis).unwrap();
        assert!(back.find_tab(&Tab::Vis).is_some() && back.find_tab(&Tab::Net).is_none());
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
    fn the_btop_chrome_fights_in_the_map_pane() {
        assert_eq!(chrome::view(Chrome::Btop).combat_tab(), Tab::Network);
        assert_eq!(chrome::view(Chrome::Classic).combat_tab(), Tab::Target);
        assert_eq!(chrome::view(Chrome::Suite).combat_tab(), Tab::Target);
    }

    #[test]
    fn the_page_blocks_exactly_the_terminal_chords() {
        let html = include_str!("../../../web/index.html");
        let keys: String = ninsei_term::keys::BROWSER_CTRL.iter().collect();
        assert!(
            html.contains(&format!("\"{keys}\".includes")),
            "web/index.html should block {keys}"
        );
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

    /// One frame of the run's keys on the Stack chrome; returns what they asked for and
    /// whether the Message box was asked for.
    fn stack_keys(
        h: &mut crate::uitest::Harness,
        g: &ninsei_core::Game,
        ev: Vec<egui::Event>,
    ) -> (Vec<Request>, bool) {
        let mut suite = crate::suite::State::default();
        let (mut out, mut focus) = (Vec::new(), false);
        h.frame(ev, |ui| {
            let mut popup = crate::volume::Popup::default();
            let mut cx = FrameCx {
                game: g,
                pal: theme::Palette::from_def(&g.data.themes[0]),
                suite: &mut suite,
                keys_free: true,
                level: crate::volume::Level {
                    on: true,
                    volume: 100,
                },
                popup: &mut popup,
            };
            out = run_keys(&crate::stack::Stack, ui.ctx(), &mut cx, false, &mut focus);
        });
        (out, focus)
    }

    #[test]
    fn enter_in_a_stack_fight_presses_run_and_leaves_the_message_box_alone() {
        let d = Data::builtin().unwrap();
        let g = crate::stack::tests::a_fight(&d);
        let mut h = crate::uitest::Harness::new(1280.0, 800.0);
        stack_keys(&mut h, &g, Vec::new());
        let (out, focus) = stack_keys(&mut h, &g, crate::uitest::key(Key::Enter));
        assert!(
            out.iter()
                .any(|r| matches!(r, Request::Cmd(Command::Game(Action::Play)))),
            "Enter pressed Run"
        );
        assert!(!focus, "and didn't also go to the Message box");
        // Out of a fight, Enter goes to the Message box as before.
        let calm = ninsei_core::Game::new(d.clone(), 3);
        let (out, focus) = stack_keys(&mut h, &calm, crate::uitest::key(Key::Enter));
        assert!(out.is_empty() && focus);
    }

    /// One frame of the run's keys on the btop chrome with the `ice.bas` tile holding the
    /// keyboard; returns what they asked for and whether the command line was asked for.
    fn ice_bas_keys(
        h: &mut crate::uitest::Harness,
        g: &ninsei_core::Game,
        ev: Vec<egui::Event>,
    ) -> (Vec<Request>, bool) {
        let mut suite = crate::suite::State::default();
        let (mut out, mut focus) = (Vec::new(), false);
        h.frame(ev, |ui| {
            crate::btop::script::take_keys(ui.ctx());
            let mut popup = crate::volume::Popup::default();
            let mut cx = FrameCx {
                game: g,
                pal: theme::Palette::from_def(&g.data.themes[0]),
                suite: &mut suite,
                keys_free: true,
                level: crate::volume::Level {
                    on: true,
                    volume: 100,
                },
                popup: &mut popup,
            };
            out = run_keys(&chrome::Btop, ui.ctx(), &mut cx, false, &mut focus);
        });
        (out, focus)
    }

    #[test]
    fn over_the_ice_bas_tile_space_and_the_digits_still_work_the_hotbar() {
        let d = Data::builtin().unwrap();
        let mut g = crate::stack::tests::a_fight(&d);
        g.apply(Action::Program("10 RUN icepick\n20 GOTO 10".into()));
        let mut h = crate::uitest::Harness::new(1280.0, 800.0);
        let game = |out: &[Request]| -> Vec<Action> {
            out.iter()
                .filter_map(|r| match r {
                    Request::Cmd(Command::Game(a)) => Some(a.clone()),
                    _ => None,
                })
                .collect()
        };
        let (out, _) = ice_bas_keys(&mut h, &g, crate::uitest::key(Key::Space));
        assert_eq!(game(&out), [Action::EndTurn]);
        let (out, _) = ice_bas_keys(&mut h, &g, crate::uitest::key(Key::Num0));
        assert_eq!(game(&out), [Action::Run(0)]);
        // Enter is the tile's (it retypes the line), never the command line's.
        let (out, focus) = ice_bas_keys(&mut h, &g, crate::uitest::key(Key::Enter));
        assert!(game(&out).is_empty() && !focus);
        assert!(
            crate::btop::script::view(&h.ctx, &g).edit.is_some(),
            "retyping"
        );
    }
}
