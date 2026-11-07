//! The dock tabs. Panes only read the game and push [`Request`]s; the app applies them after drawing.

use egui::{Color32, RichText, ScrollArea, Ui, WidgetText};
use ninsei_core::hardware::Chrome;
use ninsei_core::hexedit::HexBuffer;
use ninsei_core::netview::MapView;
use ninsei_core::{Action, Command, ContractStatus, Effect, Game, LogKind, Phase, tuning};

use crate::hexview;
use crate::netmap;
use crate::theme::Palette;

/// The run screen's panes. Core panes are always there; secondary panes are added from
/// **view → add pane** and can be closed. A new pane is a variant here, a title, and a renderer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Tab {
    Deck,
    Warez,
    Network,
    Target,
    Hex,
    Terminal,
    Todo,
    /// Secondary: network traffic, btop's net box.
    Net,
}

impl Tab {
    /// Panes every layout must have.
    pub const CORE: [Tab; 7] = [
        Tab::Deck,
        Tab::Warez,
        Tab::Network,
        Tab::Target,
        Tab::Hex,
        Tab::Terminal,
        Tab::Todo,
    ];
    /// Panes you add yourself, each with its id in `data/hub/panes.ron` (bought in RIG or
    /// bundled with a firmware).
    pub const SECONDARY: [(Tab, &'static str); 1] = [(Tab::Net, "net")];

    pub fn is_secondary(self) -> bool {
        Self::SECONDARY.iter().any(|(t, _)| *t == self)
    }

    /// The catalog id of a secondary pane.
    pub fn pane_id(self) -> Option<&'static str> {
        Self::SECONDARY
            .iter()
            .find(|(t, _)| *t == self)
            .map(|(_, id)| *id)
    }

    /// The window number the suite shows in each pane's corner.
    pub fn number(self) -> usize {
        match self {
            Tab::Deck => 1,
            Tab::Warez => 2,
            Tab::Network => 3,
            Tab::Target => 4,
            Tab::Hex => 5,
            Tab::Terminal => 6,
            Tab::Todo => 7,
            Tab::Net => 8,
        }
    }
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
    /// Add a secondary pane.
    AddPane(Tab),
    /// Damage glitches on, reduced or off.
    Glitches(ninsei_core::glitch::Glitches),
    /// Put this in the command line and focus it (e.g. `jack `).
    Type(String),
    /// Print a line in the log (completions, a cancelled line).
    Say(String),
    /// How loud everything plays, in percent.
    Volume(u32),
    /// Sound on or off (the same switch as the System app's).
    SoundOn(bool),
}

pub struct Panes<'a> {
    pub game: &'a Game,
    pub pal: Palette,
    /// How the network map is drawn: the deck's firmware decides.
    pub map: MapView,
    /// The deck's name for the deck.sys header.
    pub deck: String,
    /// How the panes are dressed: the deck's firmware decides.
    pub chrome: Chrome,
    /// Damage glitches: how much log text to corrupt, and the seed for this moment.
    pub corrupt: (f32, u64),
    /// The command line: readline keys, history, completion.
    pub term: &'a mut ninsei_term::LineEditor,
    pub focus_input: &'a mut bool,
    pub bbs_open: bool,
    pub hex: &'a mut Option<HexBuffer>,
    /// Programs (by uid) picked to keep at the exit.
    pub keep: &'a mut Vec<u32>,
    /// Free stash slots at home: how many programs can be kept.
    pub keep_room: usize,
    /// The suite chrome's state: the sheet's cursor and sort, and its dialogs.
    pub suite: &'a mut crate::suite::State,
    pub out: Vec<Request>,
}

impl Panes<'_> {
    pub(crate) fn act(&mut self, a: Action) {
        self.out.push(Request::Cmd(Command::Game(a)));
    }
}

impl egui_dock::TabViewer for Panes<'_> {
    type Tab = Tab;

    fn id(&mut self, tab: &mut Tab) -> egui::Id {
        egui::Id::new(*tab)
    }

    fn title(&mut self, tab: &mut Tab) -> WidgetText {
        if let Some(t) = crate::chrome::view(self.chrome).title(*tab) {
            return t.into();
        }
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
            Tab::Net => " net ",
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

    fn is_closeable(&self, tab: &Tab) -> bool {
        tab.is_secondary()
    }

    fn scroll_bars(&self, tab: &Tab) -> [bool; 2] {
        if let Some(b) = crate::chrome::view(self.chrome).scroll_bars(*tab) {
            return b;
        }
        match tab {
            Tab::Network | Tab::Terminal | Tab::Net => [false, false],
            _ => [false, true],
        }
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut Tab) {
        let v = crate::chrome::view(self.chrome);
        match tab {
            Tab::Deck => v.deck(self, ui),
            Tab::Warez => v.warez(self, ui),
            Tab::Network => v.network(self, ui),
            Tab::Target => v.target(self, ui),
            Tab::Hex => v.hex(self, ui),
            Tab::Terminal => v.terminal(self, ui),
            Tab::Todo => v.todo(self, ui),
            Tab::Net => v.net(self, ui),
        }
    }
}

impl Panes<'_> {
    /// The network map, drawn the way the firmware's map part says.
    pub(crate) fn network(&mut self, ui: &mut Ui) {
        if let Some(n) = netmap::show(ui, self.game, &self.pal, self.map) {
            self.act(Action::Move(n));
        }
    }

    /// The run's hex editor.
    pub(crate) fn hex_pane(&mut self, ui: &mut Ui) {
        hexview::show_run(ui, self.game, self.hex, &self.pal, &mut self.out);
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

    pub(crate) fn deck(&mut self, ui: &mut Ui) {
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

    pub(crate) fn warez(&mut self, ui: &mut Ui) {
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
                    let hover = format!(
                        "{}\n{}\nram {}  cpu {}  {}",
                        w.name,
                        blurb,
                        w.ram,
                        w.cpu,
                        w.effect.describe()
                    );
                    // Buttons first, from the right; the name and stats get what's left and
                    // shorten rather than run under the buttons.
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.game.has_tool("hexed")
                            && ui
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
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.add(egui::Label::new(name).truncate())
                                .on_hover_text(hover);
                            ui.add(
                                egui::Label::new(
                                    RichText::new(format!("r{} c{}", w.ram, w.cpu))
                                        .small()
                                        .color(p.dim),
                                )
                                .truncate(),
                            );
                        });
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

    pub(crate) fn target(&mut self, ui: &mut Ui) {
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
    pub(crate) fn todo(&mut self, ui: &mut Ui) {
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
                ContractStatus::Betrayed => ("[!]", p.dim),
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
                if let Some(label) = g.betrayal_label(k) {
                    ui.label(
                        RichText::new(format!("{label}, paid {}¢", k.paid))
                            .small()
                            .color(p.bright),
                    );
                    return;
                }
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

    pub(crate) fn terminal(&mut self, ui: &mut Ui) {
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
                    let line = self.garble(&l.text);
                    ui.label(RichText::new(line).monospace().color(col));
                }
            });
        self.prompt(ui);
    }

    /// A log line as the damaged deck shows it: a few characters corrupted, same width.
    pub(crate) fn garble(&self, text: &str) -> String {
        let (rate, seed) = self.corrupt;
        if rate <= 0.0 {
            return text.to_string();
        }
        let line_seed = text
            .bytes()
            .fold(seed, |h, b| h.wrapping_mul(31).wrapping_add(u64::from(b)));
        ninsei_core::glitch::corrupt(text, line_seed, rate)
    }

    /// The command line under the log.
    pub(crate) fn prompt(&mut self, ui: &mut Ui) {
        self.prompt_as(ui, "runner@ninsei:~$", false);
    }

    /// The command line with its own label; `boxed` draws it as a form field. Keys are
    /// readline's (`ninsei_term`): history, `Ctrl+R`, Tab to complete.
    pub(crate) fn prompt_as(&mut self, ui: &mut Ui, label: &str, boxed: bool) {
        let p = self.pal;
        let id = ui.id().with("command-line");
        let style = ninsei_term::widget::Style {
            font: egui::TextStyle::Monospace.resolve(ui.style()),
            prompt: p.dim,
            text: p.bright,
            cursor: p.fg,
            under_cursor: p.bg,
            search: p.fg,
        };
        if *self.focus_input && !self.bbs_open {
            ui.memory_mut(|m| m.request_focus(id));
            *self.focus_input = false;
        }
        let g = self.game;
        let complete = |before: &str| complete_run(g, before);
        let frame = if boxed {
            egui::Frame::new()
                .stroke(egui::Stroke::new(1.0, p.dim))
                .inner_margin(egui::Margin::symmetric(4, 1))
        } else {
            egui::Frame::NONE
        };
        let term = &mut *self.term;
        let r = frame
            .show(ui, |ui| {
                ninsei_term::widget::line(ui, id, term, label, &style, &complete)
            })
            .inner;
        for o in r.outcomes {
            match o {
                ninsei_term::Outcome::Submit(line) => {
                    if !line.trim().is_empty() {
                        self.out.push(Request::Line(line));
                    }
                }
                ninsei_term::Outcome::Cancelled(was) => {
                    self.out.push(Request::Say(format!("{label} {was}^C")));
                }
                ninsei_term::Outcome::ClearScreen => {
                    self.out.push(Request::Cmd(Command::Clear));
                }
                ninsei_term::Outcome::List(words) => {
                    self.out.push(Request::Say(words.join("   ")));
                }
                ninsei_term::Outcome::Nothing => {}
            }
        }
    }
}

/// Tab completion for the run terminal: verbs first, then what each verb takes (a route from
/// here, an HDD slot, a theme).
fn complete_run(g: &Game, before: &str) -> Vec<String> {
    let words: Vec<&str> = before.split_whitespace().collect();
    let fresh = before.ends_with(char::is_whitespace) || before.is_empty();
    let arg = words.len() + usize::from(fresh) - 1;
    if arg == 0 {
        return ninsei_core::command::verbs(g).map(String::from).collect();
    }
    if arg > 1 || !ninsei_core::command::knows(g, words[0]) {
        return Vec::new();
    }
    match words[0] {
        "jack" | "j" | "cd" | "ssh" => g.reachable().iter().map(|n| n.to_string()).collect(),
        v if ninsei_core::command::SLOT_VERBS.contains(&v) || v == "r" => {
            (0..g.hdd.len()).map(|i| i.to_string()).collect()
        }
        "theme" => g.data.themes.iter().map(|t| t.id.clone()).collect(),
        _ => Vec::new(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::uitest::Harness;
    use ninsei_core::binary;
    use ninsei_core::disk::Disk;
    use ninsei_core::hub::{Save, StashItem};
    use ninsei_core::{Data, hardware::Chrome as Ch};

    /// A run jacked in with two stash programs installed (torch and garrote).
    fn run() -> (Data, Save, Game) {
        let d = Data::builtin().unwrap();
        let mut s = Save::default();
        s.ensure_hardware(&d);
        for (id, w) in [(7, "torch"), (8, "garrote")] {
            let ix = d.warez_ix(w).unwrap();
            let bin = binary::encode(&d.warez[ix], s.key);
            s.stash.push(StashItem {
                id,
                origin: w.into(),
                hex: binary::to_hex(&bin),
            });
            s.install(&d, &Disk::Stash(id)).unwrap();
        }
        s.next_item = 9;
        let g = s.jack_in(&d, &d.rig, 1);
        (d, s, g)
    }

    /// Runs the exit's keep picker with `events` and a frame to settle.
    fn picker(
        h: &mut Harness,
        d: &Data,
        g: &Game,
        keep: &mut Vec<u32>,
        room: usize,
        ev: Vec<egui::Event>,
    ) {
        let (mut term, mut focus, mut hex, mut suite) = (
            ninsei_term::LineEditor::default(),
            false,
            None,
            crate::suite::State::default(),
        );
        for events in [ev, Vec::new()] {
            h.frame(events, |ui| {
                let mut panes = Panes {
                    game: g,
                    pal: Palette::from_def(&d.themes[0]),
                    map: MapView::Iso,
                    deck: String::new(),
                    chrome: Ch::Classic,
                    corrupt: (0.0, 0),
                    term: &mut term,
                    focus_input: &mut focus,
                    bbs_open: false,
                    hex: &mut hex,
                    keep: &mut *keep,
                    keep_room: room,
                    suite: &mut suite,
                    out: Vec::new(),
                };
                panes.keep_picker(ui);
            });
        }
    }

    fn press(h: &Harness, text: &str) -> Vec<egui::Event> {
        let r = h
            .find(text)
            .unwrap_or_else(|| panic!("no {text:?}: {:?}", h.texts()));
        crate::uitest::click(r.center())
    }

    fn label_of(g: &Game, uid: u32) -> String {
        g.hdd.iter().find(|s| s.uid == uid).unwrap().spec.file()
    }

    #[test]
    fn the_exit_picker_starts_with_the_loadout_ticked() {
        let (d, _s, g) = run();
        let mut keep = g.loadout_keep();
        assert_eq!(keep.len(), 2, "both programs from home");
        let mut h = Harness::new(900.0, 700.0);
        picker(&mut h, &d, &g, &mut keep, 4, Vec::new());
        // Clicking a ticked one leaves it behind; clicking again keeps it.
        let uid = keep[0];
        let ev = press(&h, &label_of(&g, uid));
        picker(&mut h, &d, &g, &mut keep, 4, ev);
        assert!(!keep.contains(&uid), "unticked");
        let ev = press(&h, &label_of(&g, uid));
        picker(&mut h, &d, &g, &mut keep, 4, ev);
        assert!(keep.contains(&uid), "ticked again");
    }

    #[test]
    fn the_picker_cant_keep_more_than_the_stash_holds() {
        let (d, _s, g) = run();
        let mut keep = g.loadout_keep();
        let mut h = Harness::new(900.0, 700.0);
        picker(&mut h, &d, &g, &mut keep, 2, Vec::new());
        // Room for two, two ticked: a third (a free program) can't be ticked.
        let free = g.hdd.iter().find(|s| !keep.contains(&s.uid)).unwrap().uid;
        let ev = press(&h, &label_of(&g, free));
        picker(&mut h, &d, &g, &mut keep, 2, ev);
        assert!(!keep.contains(&free));
        assert_eq!(keep.len(), 2);
    }
}
