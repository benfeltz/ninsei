//! The dock tabs. Panes only read the game and push [`Request`]s; the app applies them after drawing.

use egui::{Color32, RichText, ScrollArea, Ui, WidgetText};
use ninsei_core::hexedit::HexBuffer;
use ninsei_core::netview::MapView;
use ninsei_core::{Action, Command, ContractStatus, Effect, Game, LogKind, Phase, tuning};

use crate::hexview;
use crate::netmap;
use crate::theme::Palette;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tab {
    Deck,
    Warez,
    Network,
    Target,
    Hex,
    Terminal,
    Todo,
}

pub enum Request {
    Cmd(Command),
    /// Dial the BBS on this node.
    OpenBbs,
    /// The run's over: back to the desktop.
    GoHome,
    /// Open HDD slot `n` in the hex editor.
    OpenHex(usize),
    CloseHex,
    /// A raw line typed into the terminal.
    Line(String),
    /// Plug in a monitor you own.
    Monitor(String),
    /// Run with a deck you own.
    Deck(String),
}

pub struct Panes<'a> {
    pub game: &'a Game,
    pub pal: Palette,
    /// How the network map is drawn: the deck's firmware decides.
    pub map: MapView,
    /// The deck's name for the deck.sys header.
    pub deck: String,
    pub input: &'a mut String,
    pub focus_input: &'a mut bool,
    pub bbs_open: bool,
    pub hex: &'a mut Option<HexBuffer>,
    /// Programs (by uid) picked to keep at the exit.
    pub keep: &'a mut Vec<u32>,
    /// Free stash slots at home: how many programs can be kept.
    pub keep_room: usize,
    pub out: Vec<Request>,
}

impl Panes<'_> {
    fn act(&mut self, a: Action) {
        self.out.push(Request::Cmd(Command::Game(a)));
    }
}

impl egui_dock::TabViewer for Panes<'_> {
    type Tab = Tab;

    fn id(&mut self, tab: &mut Tab) -> egui::Id {
        egui::Id::new(*tab)
    }

    fn title(&mut self, tab: &mut Tab) -> WidgetText {
        match tab {
            Tab::Deck => " deck.sys ",
            Tab::Warez => " warez/ ",
            Tab::Network => " network.map ",
            Tab::Target => {
                if self.game.phase == Phase::Combat {
                    return " target.ice ● ".into();
                }
                " target.ice "
            }
            Tab::Hex => {
                if self.hex.as_ref().is_some_and(HexBuffer::dirty) {
                    return " hexedit ● ".into();
                }
                " hexedit "
            }
            Tab::Terminal => " terminal ",
            Tab::Todo => {
                let open = self
                    .game
                    .contracts
                    .iter()
                    .filter(|k| k.status == ContractStatus::Active)
                    .count();
                if open > 0 {
                    return format!(" todo ({open}) ").into();
                }
                " todo "
            }
        }
        .into()
    }

    fn is_closeable(&self, _tab: &Tab) -> bool {
        false
    }

    fn scroll_bars(&self, tab: &Tab) -> [bool; 2] {
        match tab {
            Tab::Network | Tab::Terminal => [false, false],
            _ => [false, true],
        }
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut Tab) {
        match tab {
            Tab::Deck => self.deck(ui),
            Tab::Warez => self.warez(ui),
            Tab::Network => {
                if let Some(n) = netmap::show(ui, self.game, &self.pal, self.map) {
                    self.act(Action::Move(n));
                }
            }
            Tab::Target => self.target(ui),
            Tab::Hex => hexview::show_run(ui, self.game, self.hex, &self.pal, &mut self.out),
            Tab::Terminal => self.terminal(ui),
            Tab::Todo => self.todo(ui),
        }
    }
}

/// `[█████░░░]` style meter.
fn bar(value: u32, max: u32, cells: usize) -> (String, String) {
    let filled = if max == 0 {
        0
    } else {
        (value.min(max) as usize * cells).div_ceil(max as usize)
    };
    let filled = if value == 0 { 0 } else { filled.max(1) };
    ("█".repeat(filled), "░".repeat(cells - filled))
}

impl Panes<'_> {
    fn meter(&self, ui: &mut Ui, label: &str, value: u32, max: u32, suffix: String, hot: bool) {
        let p = self.pal;
        let (on, off) = bar(value, max, 14);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.label(RichText::new(format!("{label:<6}")).color(p.dim));
            ui.label(RichText::new(on).color(if hot { p.bright } else { p.fg }));
            ui.label(RichText::new(off).color(p.faint));
            ui.label(RichText::new(format!(" {suffix}")).color(if hot { p.bright } else { p.fg }));
        });
    }

    fn deck(&mut self, ui: &mut Ui) {
        let g = self.game;
        let d = &g.deck;
        let p = self.pal;
        ui.label(RichText::new(self.deck.to_uppercase()).color(p.bright));
        ui.label(
            RichText::new(format!("seed {:016x}", g.seed))
                .small()
                .color(p.dim),
        );
        ui.add_space(6.0);
        let cpu = g.combat.as_ref().map_or(d.cpu_max, |c| c.cpu);
        self.meter(
            ui,
            "CPU",
            cpu,
            d.cpu_max,
            format!("{cpu}/{} cyc", d.cpu_max),
            false,
        );
        let ram = g.ram_used();
        self.meter(
            ui,
            "RAM",
            ram,
            d.ram_max,
            format!("{ram}/{} GB", d.ram_max),
            ram == d.ram_max,
        );
        let hdd = g.hdd.len() as u32;
        self.meter(
            ui,
            "HDD",
            hdd,
            d.hdd_max,
            format!("{hdd}/{} slots", d.hdd_max),
            hdd == d.hdd_max,
        );
        let low = d.integrity * 3 <= d.integrity_max;
        self.meter(
            ui,
            "INTEG",
            d.integrity,
            d.integrity_max,
            format!("{}/{}", d.integrity, d.integrity_max),
            low,
        );
        let hot = d.trace * 4 >= tuning::TRACE_MAX * 3;
        self.meter(
            ui,
            "TRACE",
            d.trace,
            tuning::TRACE_MAX,
            format!("{}%", d.trace),
            hot,
        );
        if let Some(c) = &g.combat
            && c.shield > 0
        {
            self.meter(
                ui,
                "SHIELD",
                c.shield,
                c.shield.max(10),
                format!("{}", c.shield),
                false,
            );
        }
        ui.add_space(6.0);
        ui.label(RichText::new(format!("CRED   {}¢", d.credits)).color(p.fg));
        ui.label(RichText::new(format!("FRAGS  {}", d.fragments)).color(p.fg));
        ui.label(RichText::new(format!("HOPS   {}", g.hops)).color(p.dim));
    }

    fn warez(&mut self, ui: &mut Ui) {
        let g = self.game;
        let p = self.pal;
        let fighting = g.phase == Phase::Combat;
        let cpu = g.combat.as_ref().map_or(0, |c| c.cpu);
        for (heading, loaded) in [("▾ ram/", true), ("▾ hdd/", false)] {
            let note = if loaded {
                format!("{}/{} GB", g.ram_used(), g.deck.ram_max)
            } else {
                format!("{}/{} slots", g.hdd.len(), g.deck.hdd_max)
            };
            ui.horizontal(|ui| {
                ui.label(RichText::new(heading).color(p.bright));
                ui.label(RichText::new(note).small().color(p.dim));
            });
            let mut any = false;
            for (i, s) in g.hdd.iter().enumerate().filter(|(_, s)| s.loaded == loaded) {
                any = true;
                let w = &s.spec;
                let blurb = &g.data.warez[s.warez].blurb;
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    let icon = match w.effect {
                        Effect::Break { .. } => "⚔",
                        Effect::Shield(_) => "◘",
                        _ => "⚙",
                    };
                    let opt = if w.opt > 0 {
                        format!(" O{}", w.opt)
                    } else {
                        String::new()
                    };
                    let name = RichText::new(format!("[{i}] {icon} {}{opt}", w.file()))
                        .color(if loaded { p.fg } else { p.dim });
                    ui.label(name).on_hover_text(format!(
                        "{}\n{}\nram {}  cpu {}  {}",
                        w.name,
                        blurb,
                        w.ram,
                        w.cpu,
                        w.effect.describe()
                    ));
                    ui.label(
                        RichText::new(format!("r{} c{}", w.ram, w.cpu))
                            .small()
                            .color(p.dim),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button("hex")
                            .on_hover_text("open in hexedit")
                            .clicked()
                        {
                            self.out.push(Request::OpenHex(i));
                        }
                        if loaded {
                            if ui.small_button("unload").clicked() {
                                self.act(Action::Unload(i));
                            }
                            if fighting {
                                let ok = cpu >= w.cpu;
                                if ui
                                    .add_enabled(ok, egui::Button::new("run").small())
                                    .clicked()
                                {
                                    self.act(Action::Run(i));
                                }
                            }
                        } else {
                            let fits = g.ram_used() + w.ram <= g.deck.ram_max;
                            if ui
                                .add_enabled(fits, egui::Button::new("load").small())
                                .clicked()
                            {
                                self.act(Action::Load(i));
                            }
                        }
                    });
                });
            }
            if !any {
                ui.label(RichText::new("    (empty)").color(p.faint));
            }
            ui.add_space(6.0);
        }
        if fighting {
            ui.label(
                RichText::new(format!(
                    "loading mid-fight costs {} cpu",
                    tuning::COMBAT_LOAD_CPU
                ))
                .small()
                .color(p.dim),
            );
        }
    }

    fn target(&mut self, ui: &mut Ui) {
        let g = self.game;
        let p = self.pal;
        match g.phase {
            Phase::Flatlined | Phase::JackedOut => {
                let (title, sub) = if g.phase == Phase::Flatlined {
                    (FLATLINE_ART, "connection severed. your deck is smoke.")
                } else {
                    (JACKOUT_ART, "you made it out.")
                };
                ui.label(RichText::new(title).color(p.bright).monospace());
                ui.label(RichText::new(sub).color(p.fg));
                ui.label(
                    RichText::new(format!("credits {}¢   hops {}", g.deck.credits, g.hops))
                        .color(p.dim),
                );
                ui.add_space(8.0);
                if g.phase == Phase::JackedOut {
                    self.keep_picker(ui);
                }
                if ui.button(" jack out: home ").clicked() {
                    self.out.push(Request::GoHome);
                }
                return;
            }
            Phase::Exploring => {
                let n = &g.net.nodes[g.at];
                ui.label(
                    RichText::new(format!("{}  [{}]", n.name, g.node_label(g.at))).color(p.fg),
                );
                if g.at_bbs() {
                    let b = g.bbs();
                    ui.label(
                        RichText::new(format!("{} // sysop {}", b.name, b.sysop)).color(p.bright),
                    );
                    if !self.bbs_open
                        && ui
                            .button(format!(" ATDT {}  (dial in) ", b.phone))
                            .clicked()
                    {
                        self.out.push(Request::OpenBbs);
                    }
                } else {
                    ui.label(RichText::new("no active ICE on this node.").color(p.dim));
                }
                ui.add_space(4.0);
                ui.label(RichText::new("routes:").color(p.dim));
                for &to in g.reachable() {
                    let t = &g.net.nodes[to];
                    if ui
                        .button(format!("jack {to}  {}  [{}]", t.name, g.node_label(to)))
                        .clicked()
                    {
                        self.act(Action::Move(to));
                    }
                }
                return;
            }
            Phase::Encounter => {
                ui.label(
                    RichText::new("incoming message. answer it in the talk window.")
                        .color(p.bright),
                );
                return;
            }
            Phase::Combat => {}
        }
        let Some(c) = &g.combat else { return };
        let ice = &g.data.ice[c.ice];
        let art = match ice.class {
            ninsei_core::IceClass::Barrier => BARRIER_ART,
            ninsei_core::IceClass::CodeGate => CODEGATE_ART,
            ninsei_core::IceClass::Sentry => SENTRY_ART,
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(art).monospace().color(if ice.tier >= 2 {
                p.bright
            } else {
                p.fg
            }));
            ui.vertical(|ui| {
                let tier = match ice.tier {
                    0 => "RUNNER",
                    1 => "ICE",
                    _ => "BLACK ICE",
                };
                ui.label(
                    RichText::new(ice.name.to_uppercase())
                        .heading()
                        .color(p.bright),
                );
                ui.label(RichText::new(format!("{tier} // {}", ice.class.label())).color(p.dim));
                ui.add_space(4.0);
                self.meter(
                    ui,
                    "STR",
                    c.strength,
                    c.strength_max,
                    format!("{}/{}", c.strength, c.strength_max),
                    false,
                );
                let intent = g.intent().map_or_else(String::new, |i| i.describe());
                let next = if c.stunned {
                    format!("{intent} (lagged, skips)")
                } else {
                    intent
                };
                ui.label(RichText::new(format!("NEXT  {next}")).color(p.bright));
                ui.label(
                    RichText::new(format!(
                        "TURN  {}   CPU {}/{}",
                        c.turn, c.cpu, g.deck.cpu_max
                    ))
                    .color(p.dim),
                );
            });
        });
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            for (i, s) in g.hdd.iter().enumerate().filter(|(_, s)| s.loaded) {
                let w = &s.spec;
                let class_hit =
                    matches!(w.effect, Effect::Break { vs: Some(v), .. } if v == ice.class);
                let label = format!(
                    "[{i}] {} ({}c){}",
                    w.name,
                    w.cpu,
                    if class_hit { " x2" } else { "" }
                );
                let btn = egui::Button::new(RichText::new(label).color(if class_hit {
                    p.bright
                } else {
                    p.fg
                }));
                if ui
                    .add_enabled(c.cpu >= w.cpu, btn)
                    .on_hover_text(w.effect.describe())
                    .clicked()
                {
                    self.act(Action::Run(i));
                }
            }
        });
        ui.add_space(4.0);
        if ui
            .button(RichText::new(" END TURN [space] ").color(p.bright))
            .clicked()
        {
            self.act(Action::EndTurn);
        }
    }

    /// At the exit: pick which programs come home to the stash.
    fn keep_picker(&mut self, ui: &mut Ui) {
        let g = self.game;
        let p = self.pal;
        let room = self.keep_room;
        self.keep.retain(|uid| g.hdd.iter().any(|s| s.uid == *uid));
        ui.label(
            RichText::new(format!(
                "keep for the stash: picked {} of {room}",
                self.keep.len()
            ))
            .color(p.bright),
        );
        if room == 0 {
            ui.label(
                RichText::new("the stash is full. scrap something at home to make room.")
                    .color(p.dim),
            );
        }
        for s in &g.hdd {
            let mut on = self.keep.contains(&s.uid);
            let opt = if s.spec.opt > 0 {
                format!(" O{}", s.spec.opt)
            } else {
                String::new()
            };
            let label = format!("{}{opt}  {}", s.spec.file(), s.spec.effect.describe());
            let full = !on && self.keep.len() >= room;
            if ui
                .add_enabled(!full, egui::Checkbox::new(&mut on, label))
                .changed()
            {
                if on {
                    self.keep.push(s.uid);
                } else {
                    self.keep.retain(|u| *u != s.uid);
                }
            }
        }
        ui.label(
            RichText::new("anything not kept is left behind.")
                .small()
                .color(p.dim),
        );
        ui.add_space(6.0);
    }

    /// Contracts as an IDE TODO list.
    fn todo(&mut self, ui: &mut Ui) {
        let g = self.game;
        let p = self.pal;
        if g.contracts.is_empty() {
            ui.label(RichText::new("// no open contracts.").color(p.dim));
            ui.label(
                RichText::new("// fixers post on the BBS jobs board and hang around MSG nodes.")
                    .color(p.dim),
            );
            return;
        }
        for k in g.contracts.iter().rev() {
            let n = &g.root().nodes[k.node];
            let (mark, col) = match k.status {
                ContractStatus::Active => ("[ ]", p.fg),
                ContractStatus::Done => ("[x]", p.dim),
                ContractStatus::Failed => ("[-]", p.faint),
                ContractStatus::Offered => ("[?]", p.dim),
            };
            ui.horizontal(|ui| {
                ui.label(RichText::new(mark).color(col));
                let text = RichText::new(format!(
                    "TODO({}): derez {} [node {}, layer {}]",
                    k.client, n.name, n.id, n.layer
                ))
                .color(col);
                let text = if k.status == ContractStatus::Failed {
                    text.strikethrough()
                } else {
                    text
                };
                ui.label(text);
                ui.label(RichText::new(format!("{}¢", k.reward)).color(
                    if k.status == ContractStatus::Active {
                        p.bright
                    } else {
                        col
                    },
                ));
                if let Some((cap, bonus)) = k.ghost {
                    let ok = g.deck.trace <= cap;
                    let note = format!(
                        "+{bonus}¢ ghost if trace <= {cap}%{}",
                        if ok { "" } else { " (blown)" }
                    );
                    ui.label(RichText::new(note).small().color(
                        if ok && k.status == ContractStatus::Active {
                            p.fg
                        } else {
                            p.dim
                        },
                    ));
                }
            });
            if k.status == ContractStatus::Active {
                ui.label(
                    RichText::new(format!("      // {}", k.brief))
                        .small()
                        .color(p.dim),
                );
            }
        }
    }

    fn terminal(&mut self, ui: &mut Ui) {
        let p = self.pal;
        let input_h = ui.text_style_height(&egui::TextStyle::Monospace) + 10.0;
        ScrollArea::vertical()
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .max_height(ui.available_height() - input_h)
            .show(ui, |ui| {
                for l in &self.game.log {
                    let col: Color32 = match l.kind {
                        LogKind::Info => p.fg,
                        LogKind::Good => p.fg,
                        LogKind::Bad => p.bright,
                        LogKind::Loot => p.bright,
                        LogKind::Sys => p.dim,
                    };
                    ui.label(RichText::new(&l.text).monospace().color(col));
                }
            });
        ui.horizontal(|ui| {
            ui.label(RichText::new("runner@ninsei:~$").color(p.dim));
            let edit = egui::TextEdit::singleline(self.input)
                .frame(egui::Frame::NONE)
                .desired_width(f32::INFINITY)
                .text_color(p.bright);
            let resp = ui.add(edit);
            if *self.focus_input && !self.bbs_open {
                resp.request_focus();
                *self.focus_input = false;
            }
            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                let line = std::mem::take(self.input);
                if !line.trim().is_empty() {
                    self.out.push(Request::Line(line));
                }
                *self.focus_input = true;
            }
        });
    }
}

const BARRIER_ART: &str = "\
 ┌┬┬┬┬┬┬┐
 ├┼┼┼┼┼┼┤
 ├┼┼┼┼┼┼┤
 ├┼┼┼┼┼┼┤
 └┴┴┴┴┴┴┘";
const CODEGATE_ART: &str = "\
 ╔══════╗
 ║ 0x?? ║
 ║ ▓▓▓▓ ║
 ║ ░░░░ ║
 ╚══════╝";
const SENTRY_ART: &str = "\
   ╱▔▔╲
  ▕ ◉◉ ▏
   ╲▁▁╱
  ═╪══╪═
   ╱  ╲";
const FLATLINE_ART: &str = "\
 ______ _      _______ _      _____ _   _ ______
|  ____| |    |__   __| |    |_   _| \\ | |  ____|
| |__  | |       | |  | |      | | |  \\| | |__
|  __| | |       | |  | |      | | | . ` |  __|
| |    | |____   | |  | |____ _| |_| |\\  | |____
|_|    |______|  |_|  |______|_____|_| \\_|______|";
const JACKOUT_ART: &str = "\
     _          _____ _  __   ____  _    _ _______
    | |   /\\   / ____| |/ /  / __ \\| |  | |__   __|
    | |  /  \\ | |    | ' /  | |  | | |  | |  | |
_   | | / /\\ \\| |    |  <   | |  | | |  | |  | |
| |__| |/ ____ \\ |____| . \\  | |__| | |__| |  | |
 \\____//_/    \\_\\_____|_|\\_\\  \\____/ \\____/   |_|";
