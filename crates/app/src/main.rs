//! ninsei: a cyberspace hacking roguelike dressed as a retro IDE.

mod bbs_view;
mod crt;
mod encounter_view;
mod hexview;
mod netmap;
mod panes;
mod theme;

use egui::{Key, RichText};
use egui_dock::{DockArea, DockState, NodeIndex};
use ninsei_core::bbs::Outcome;
use ninsei_core::hexedit::HexBuffer;
use ninsei_core::{Action, Command, Data, Game, LogKind, Phase, command::HELP, parse_command};

use bbs_view::BbsView;
use encounter_view::EncounterView;

use panes::{Panes, Request, Tab};
use theme::Phosphor;

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
    let seed = std::env::args()
        .nth(1)
        .and_then(|s| u64::from_str_radix(&s, 16).ok())
        .unwrap_or_else(fresh_seed);
    eframe::run_native(
        "ninsei",
        options,
        Box::new(move |cc| Ok(Box::new(App::new(&cc.egui_ctx, data, seed)))),
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
            .and_then(|h| u64::from_str_radix(h.trim_start_matches('#'), 16).ok())
            .unwrap_or_else(fresh_seed);
        let started = eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(move |cc| Ok(Box::new(App::new(&cc.egui_ctx, data, seed)))),
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

fn default_layout() -> DockState<Tab> {
    let mut dock = DockState::new(vec![Tab::Network, Tab::Target, Tab::Hex]);
    let tree = dock.main_surface_mut();
    let [center, left] = tree.split_left(NodeIndex::root(), 0.27, vec![Tab::Deck]);
    tree.split_below(left, 0.42, vec![Tab::Warez]);
    tree.split_below(center, 0.68, vec![Tab::Terminal, Tab::Todo]);
    dock
}

struct App {
    game: Game,
    dock: DockState<Tab>,
    phosphor: Phosphor,
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
    fn new(ctx: &egui::Context, data: Data, seed: u64) -> Self {
        let phosphor = Phosphor::Amber;
        theme::apply(ctx, &phosphor.palette());
        let game = Game::new(data, seed);
        Self {
            last_at: game.at,
            bbs: None,
            hex: None,
            hex_uid: 0,
            talk: EncounterView::default(),
            last_phase: game.phase,
            game,
            dock: default_layout(),
            phosphor,
            crt: true,
            input: String::new(),
            focus_input: true,
        }
    }

    fn set_phosphor(&mut self, ctx: &egui::Context, p: Phosphor) {
        self.phosphor = p;
        theme::apply(ctx, &p.palette());
    }

    fn handle(&mut self, ctx: &egui::Context, req: Request) {
        let cmd = match req {
            Request::OpenBbs => {
                if self.game.at_bbs() {
                    self.bbs = Some(BbsView::new());
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
            Command::Game(a) => self.game.apply(a),
            Command::NewRun => self.game.apply(Action::NewRun(fresh_seed())),
            Command::Hex(slot) => self.open_hex(slot),
            Command::Help => {
                for l in HELP {
                    self.game.say(LogKind::Sys, *l);
                }
            }
            Command::Clear => self.game.log.clear(),
            Command::Crt => {
                self.crt = !self.crt;
                let state = if self.crt { "on" } else { "off" };
                self.game.say(LogKind::Sys, format!("crt {state}"));
            }
            Command::Theme(name) => match Phosphor::from_name(&name) {
                Some(p) => {
                    self.set_phosphor(ctx, p);
                    self.game
                        .say(LogKind::Sys, format!("phosphor: {}", p.label()));
                }
                None => self.game.say(
                    LogKind::Sys,
                    format!("err: no theme `{name}`. try amber, green or ice."),
                ),
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

    fn menu_bar(&mut self, ui: &mut egui::Ui, pal: &theme::Palette) -> Vec<Request> {
        let mut out = Vec::new();
        egui::MenuBar::new().ui(ui, |ui| {
            ui.label(RichText::new("▌ninsei").color(pal.bright).strong());
            ui.menu_button("run", |ui| {
                if ui.button("new run").clicked() {
                    out.push(Request::Cmd(Command::NewRun));
                }
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
            ui.menu_button("phosphor", |ui| {
                for p in Phosphor::ALL {
                    let mark = if p == self.phosphor { "(•)" } else { "( )" };
                    if ui.button(format!("{mark} {}", p.label())).clicked() {
                        self.set_phosphor(ui.ctx(), p);
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
                ui.label(RichText::new(self.phosphor.label()).color(pal.dim));
            });
        });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let pal = self.phosphor.palette();
        let mut requests = self.hotkeys(&ctx);

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

        if self.crt {
            crt::paint(&ctx, &pal);
        } else if self.game.phase == Phase::Exploring {
            // Keep the cursor blinking.
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
    }
}
