//! ninsei: a cyberspace hacking roguelike dressed as a retro IDE.

mod bbs_view;
mod crash;
mod crt;
mod encounter_view;
mod hexview;
mod hubtools;
mod netmap;
mod panes;
mod theme;
mod workbench;

use egui::{Key, RichText};
use egui_dock::{DockArea, DockState, NodeIndex};
use ninsei_core::bbs::Outcome;
use ninsei_core::hexedit::HexBuffer;
use ninsei_core::hub::{Answer, DEFAULT_RUN_THEME, RunEnd, RunReport, SAVE_KEY, Save, Screen};
use ninsei_core::{Action, Command, Data, Game, LogKind, Phase, command::HELP, parse_command};

use bbs_view::BbsView;
use encounter_view::EncounterView;

use panes::{Panes, Request, Tab};
use theme::Palette;
use workbench::{WbOut, Workbench};

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

fn default_layout() -> DockState<Tab> {
    let mut dock = DockState::new(vec![Tab::Network, Tab::Target, Tab::Hex]);
    let tree = dock.main_surface_mut();
    let [center, left] = tree.split_left(NodeIndex::root(), 0.27, vec![Tab::Deck]);
    tree.split_below(left, 0.42, vec![Tab::Warez]);
    tree.split_below(center, 0.68, vec![Tab::Terminal, Tab::Todo]);
    dock
}

struct App {
    data: Data,
    save: Save,
    /// The save changed and should be written this frame.
    save_dirty: bool,
    /// Programs (by uid) picked to keep when jacking out at the exit.
    keep: Vec<u32>,
    screen: Screen,
    wb: Workbench,
    /// The current run, once booked (exit or flatline).
    booked: Option<RunReport>,
    /// The run ended in a flatline: the crash screen sits behind the returning desktop.
    crashed: bool,
    /// Seed from the command line or URL, used for the first dial.
    first_seed: Option<u64>,
    /// The run. Between runs this is the last one, or a placeholder before the first.
    game: Game,
    dock: DockState<Tab>,
    /// Run theme id (see `data/themes.ron`) and its palette.
    run_theme: String,
    pal: Palette,
    crt: bool,
    input: String,
    focus_input: bool,
    last_phase: Phase,
    last_at: usize,
    bbs: Option<BbsView>,
    hex: Option<HexBuffer>,
    /// Uid of the program open in the hex editor, so trades and sales can't swap it underneath.
    hex_uid: u32,
    talk: EncounterView,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, data: Data, first_seed: Option<u64>) -> Self {
        let mut boot = vec![
            "ninsei Workbench 1.3".to_string(),
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
                    boot.push(format!("Welcome back. {} run{plural} on record.", s.runs));
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
        Self {
            crt: save.crt,
            data,
            save,
            save_dirty: keyed,
            keep: Vec::new(),
            screen: Screen::Hub,
            wb: Workbench::new(boot),
            booked: None,
            crashed: false,
            first_seed,
            last_at: game.at,
            bbs: None,
            hex: None,
            hex_uid: 0,
            talk: EncounterView::default(),
            last_phase: game.phase,
            game,
            dock: default_layout(),
            run_theme,
            pal,
            input: String::new(),
            focus_input: true,
        }
    }

    /// Switches the run theme. Returns false for an unknown or desktop-only theme.
    fn set_run_theme(&mut self, ctx: &egui::Context, id: &str) -> bool {
        let Some(t) = self.data.theme(id) else {
            return false;
        };
        self.pal = Palette::from_def(t);
        self.run_theme = t.id.clone();
        theme::apply(ctx, &self.pal);
        self.save.phosphor = t.id.clone();
        self.save_dirty = true;
        true
    }

    fn run_theme_label(&self) -> String {
        self.data
            .theme(&self.run_theme)
            .map_or(self.run_theme.clone(), |t| t.label.clone())
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
                self.wb.say(format!("NO CARRIER: {e}"));
                return;
            }
        }
        self.save_dirty = true;
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
                    self.wb.say(format!(
                        "Jacked out after {} hops. Banked {}¢.",
                        r.hops, r.banked
                    ));
                    if !r.kept.is_empty() {
                        self.wb.say(format!("Stashed: {}", r.kept.join(", ")));
                    }
                }
                RunEnd::Flatline => self.wb.say(format!(
                    "Flatlined after {} hops. Dead man's switch saved {} of {}¢.",
                    r.hops, r.banked, r.credits
                )),
            }
            if !r.learned.is_empty() {
                self.wb
                    .say(format!("New phone ranges: {}", r.learned.join(", ")));
            }
            self.booked = Some(r);
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
                    self.wb.show_scan(res);
                    Ok(msg)
                }
                Err(e) => Err(e),
            },
            WbOut::Dial(..) | WbOut::Theme(_) | WbOut::Crt => return,
        };
        match result {
            Ok(msg) => {
                if !msg.is_empty() {
                    self.wb.say(msg);
                }
                self.save_dirty = true;
            }
            Err(e) => self.wb.say(format!("Error: {e}")),
        }
    }

    /// Back to the desktop: it drags up over the run.
    fn go_home(&mut self) {
        self.book();
        self.bbs = None;
        self.wb.focus();
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
            Command::Theme(name) => {
                if self.set_run_theme(ctx, &name) {
                    let label = self.run_theme_label();
                    self.game.say(LogKind::Sys, format!("theme: {label}"));
                } else {
                    let ids: Vec<String> = self.data.themes.iter().map(|t| t.id.clone()).collect();
                    self.game.say(
                        LogKind::Sys,
                        format!("err: no run theme `{name}`. try {}.", ids.join(", ")),
                    );
                }
            }
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

    fn menu_bar(&mut self, ui: &mut egui::Ui, pal: &theme::Palette) -> Vec<Request> {
        let mut out = Vec::new();
        egui::MenuBar::new().ui(ui, |ui| {
            ui.label(RichText::new("▌ninsei").color(pal.bright).strong());
            ui.menu_button("run", |ui| {
                if ui.button("end turn   [space]").clicked() {
                    out.push(Request::Cmd(Command::Game(Action::EndTurn)));
                }
                if ui.button("help").clicked() {
                    out.push(Request::Cmd(Command::Help));
                }
            });
            ui.menu_button("view", |ui| {
                if ui.button("reset layout").clicked() {
                    self.dock = default_layout();
                }
                let crt = if self.crt {
                    "[x] crt scanlines"
                } else {
                    "[ ] crt scanlines"
                };
                if ui.button(crt).clicked() {
                    out.push(Request::Cmd(Command::Crt));
                }
            });
            ui.menu_button("theme", |ui| {
                let themes: Vec<(String, String)> = self
                    .data
                    .themes
                    .iter()
                    .map(|t| (t.id.clone(), t.label.clone()))
                    .collect();
                for (id, label) in themes {
                    let mark = if id == self.run_theme { "(•)" } else { "( )" };
                    if ui.button(format!("{mark} {label}")).clicked() {
                        self.set_run_theme(ui.ctx(), &id);
                    }
                }
            });
        });
        out
    }

    fn status_bar(&self, ui: &mut egui::Ui, pal: &theme::Palette) {
        let g = &self.game;
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
                workbench::set_desk(workbench::Desk::from_def(t));
            }
            let offset = self.screen.drag() * ctx.content_rect().height();
            let live = self.screen.hub_live();
            let mut outs = Vec::new();
            egui::Area::new(egui::Id::new("workbench"))
                .order(egui::Order::Foreground)
                .movable(false)
                .fixed_pos(egui::pos2(0.0, 0.0))
                .show(&ctx, |ui| {
                    ui.set_min_size(ctx.content_rect().size());
                    outs = self.wb.show(ui, &self.data, &self.save, offset, live);
                });
            for o in outs {
                match o {
                    WbOut::Dial(entry, seed) if live => self.dial(entry, seed),
                    WbOut::Dial(..) => {}
                    WbOut::Theme(name) => {
                        if self.set_run_theme(&ctx, &name) {
                            let label = self.run_theme_label();
                            self.wb.say(format!("theme: {label}"));
                        }
                    }
                    WbOut::Crt => {
                        self.toggle_crt();
                        self.wb
                            .say(format!("crt {}", if self.crt { "on" } else { "off" }));
                    }
                    other => self.home_action(other),
                }
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }

        if self.crt {
            crt::paint(&ctx, &pal);
        } else if self.game.phase == Phase::Exploring {
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

        let mut panes = Panes {
            game: &self.game,
            pal,
            input: &mut self.input,
            focus_input: &mut self.focus_input,
            // Any modal (BBS or talk) keeps the terminal from grabbing focus.
            bbs_open: self.bbs.is_some() || self.game.encounter.is_some(),
            hex: &mut self.hex,
            keep: &mut self.keep,
            keep_room: (self.save.stash_cap(&self.data.rig) as usize)
                .saturating_sub(self.save.stash.len()),
            out: Vec::new(),
        };
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(pal.bg))
            .show(ui, |ui| {
                DockArea::new(&mut self.dock)
                    .style(theme::dock_style(ui.style(), &pal))
                    .show_close_buttons(false)
                    .show_add_buttons(false)
                    .show_leaf_collapse_buttons(false)
                    .show_leaf_close_all_buttons(false)
                    .show_inside(ui, &mut panes);
            });
        requests.extend(panes.out);

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
