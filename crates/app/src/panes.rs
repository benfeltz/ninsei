//! The dock tabs. Panes only read the game and push [`Request`]s; the app applies them after drawing.

use egui::{Color32, RichText, ScrollArea, Ui, WidgetText};
use ninsei_core::{Action, Command, Effect, Game, LogKind, Phase, WarezKind, tuning};

use crate::netmap;
use crate::theme::Palette;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tab {
    Deck,
    Warez,
    Network,
    Target,
    Terminal,
}

pub enum Request {
    Cmd(Command),
    /// Dial the BBS on this node.
    OpenBbs,
    /// A raw line typed into the terminal.
    Line(String),
}

pub struct Panes<'a> {
    pub game: &'a Game,
    pub pal: Palette,
    pub input: &'a mut String,
    pub focus_input: &'a mut bool,
    pub bbs_open: bool,
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
            Tab::Terminal => " terminal ",
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
                if let Some(n) = netmap::show(ui, self.game, &self.pal) {
                    self.act(Action::Move(n));
                }
            }
            Tab::Target => self.target(ui),
            Tab::Terminal => self.terminal(ui),
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
        ui.label(RichText::new("ONO-SENDAI CYBERSPACE VII").color(p.bright));
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
                let w = &g.data.warez[s.warez];
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    let icon = match w.kind {
                        WarezKind::Breaker => "⚔",
                        WarezKind::Shield => "◘",
                        WarezKind::Utility => "⚙",
                    };
                    let name = RichText::new(format!("[{i}] {icon} {}.exe", w.id))
                        .color(if loaded { p.fg } else { p.dim });
                    ui.label(name).on_hover_text(format!(
                        "{}\n{}\nram {}  cpu {}  {}",
                        w.name,
                        w.blurb,
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
                if ui.button(" newrun ").clicked() {
                    self.out.push(Request::Cmd(Command::NewRun));
                }
                return;
            }
            Phase::Exploring => {
                let n = &g.net.nodes[g.at];
                ui.label(RichText::new(format!("{}  [{}]", n.name, n.kind.label())).color(p.fg));
                if g.at_bbs() {
                    let b = &g.data.bbs;
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
                        .button(format!("jack {to}  {}  [{}]", t.name, t.kind.label()))
                        .clicked()
                    {
                        self.act(Action::Move(to));
                    }
                }
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
                let tier = if ice.tier >= 2 { "BLACK ICE" } else { "ICE" };
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
                let w = &g.data.warez[s.warez];
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
