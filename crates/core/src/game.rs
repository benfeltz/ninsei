//! Run state and the rules that change it. The app only ever calls [`Game::apply`].

use crate::binary::{self, Bin, Spec};
use crate::data::{ChoiceEffect, Data, Drop, Effect, Intent, LootTable, Penalty};
use crate::network::{NetParams, Network, NodeKind};
use crate::rng::Rng;
use crate::tuning::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Deck {
    pub cpu_max: u32,
    pub ram_max: u32,
    pub hdd_max: u32,
    pub integrity: u32,
    pub integrity_max: u32,
    pub trace: u32,
    pub credits: u32,
    /// Optimizer fragments: the crafting material that raises a program's opt level.
    pub fragments: u32,
}

/// One program on the HDD. Loaded programs occupy RAM and can be run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slot {
    /// Unique per run, so the UI can tell programs apart after the HDD reshuffles.
    pub uid: u32,
    /// The factory program this binary started as (index into `Data::warez`), for its blurb.
    pub warez: usize,
    pub loaded: bool,
    /// The program itself. Edit it in the hex editor with [`Action::Patch`].
    pub bin: Bin,
    /// What `bin` decodes to. Kept in step with `bin`; a slot never holds an invalid binary.
    pub spec: Spec,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Combat {
    pub node: usize,
    /// Index into `Data::ice`.
    pub ice: usize,
    pub strength: u32,
    pub strength_max: u32,
    pub intent_ix: usize,
    /// CPU left this turn.
    pub cpu: u32,
    pub shield: u32,
    pub stunned: bool,
    /// ICE that just shook off a stun can't be stunned again this turn.
    pub stun_immune: bool,
    pub turn: u32,
    /// Extra rewards for winning (encounter fights).
    pub bonus: Vec<Drop>,
    /// Speaks l33t (it came from a l33t encounter).
    pub leet: bool,
}

/// What the runner brings into a run: deck bonuses from the RIG and the programs on the HDD.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Kit {
    pub ram: u32,
    pub cpu: u32,
    pub hdd: u32,
    pub integrity: u32,
    /// Programs to install: (factory origin, binary). Re-signed with the run's key on the way in.
    /// Empty means the starter programs.
    pub programs: Vec<(usize, Bin)>,
    /// Network shape: tier size and trait biases.
    pub net: NetParams,
    pub mods: RunMods,
    /// The phonebook entry being run, if dialed from home.
    pub site: Option<Site>,
    /// The network's BBS: an index into `Data::boards` (0 is the default board).
    pub board: usize,
    /// Jobs taken from a board at home for this network. They get their targets once it's built.
    pub jobs: Vec<crate::hub::PendingJob>,
}

/// Run-wide modifiers from the network's tier, traits and heat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunMods {
    /// Extra strength on every ICE.
    pub ice_bonus: u32,
    /// Trace gain, percent of normal.
    pub trace_pct: u32,
    /// Credit drops and contract pay, percent of normal.
    pub pay_pct: u32,
}

impl Default for RunMods {
    fn default() -> Self {
        Self {
            ice_bonus: 0,
            trace_pct: 100,
            pay_pct: 100,
        }
    }
}

impl RunMods {
    /// Scales a trace gain, never below 1 for a gain of 1 or more.
    pub fn trace(self, n: u32) -> u32 {
        if n == 0 {
            0
        } else {
            ((n * self.trace_pct + 50) / 100).max(1)
        }
    }

    pub fn pay(self, n: u32) -> u32 {
        (n * self.pay_pct + 50) / 100
    }
}

/// Which network a run is on, for the status bar and for booking heat.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Site {
    /// Phonebook entry id.
    pub entry: u32,
    pub name: String,
    pub tier: u32,
    /// Trait labels.
    pub traits: Vec<String>,
}

/// The network the runner dived out of while they're inside a subnet.
#[derive(Clone, Debug)]
pub struct Outer {
    pub net: Network,
    /// The subnet node they went in through; the subnet's exit brings them back here.
    pub node: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContractStatus {
    /// Posted on a jobs board, not taken.
    Offered,
    Active,
    Done,
    /// The target became unreachable.
    Failed,
}

/// A commission to derez one node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Contract {
    pub client: String,
    pub brief: String,
    pub node: usize,
    pub reward: u32,
    /// Ghost bonus: paid on top if trace is at or under `cap` when the target falls.
    pub ghost: Option<(u32, u32)>,
    pub status: ContractStatus,
}

/// An FTL-style event on a MSG node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Encounter {
    /// Index into `Data::events`.
    pub event: usize,
    pub node: usize,
    /// The program a trader is offering (index into `Data::warez`).
    pub offer: Option<usize>,
    /// Set once a choice has been made; the window then shows `result`.
    pub done: bool,
    pub result: Vec<LogLine>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Exploring,
    Combat,
    /// Talking to someone on a MSG node. Moving waits until you choose.
    Encounter,
    Flatlined,
    JackedOut,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogKind {
    /// Plain narration.
    Info,
    /// Something went your way.
    Good,
    /// Something hurt.
    Bad,
    /// Loot.
    Loot,
    /// System chatter and errors.
    Sys,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogLine {
    pub kind: LogKind,
    pub text: String,
}

/// One file on today's BBS listing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StockFile {
    /// Index into the area's `files` in `BbsDef`.
    pub def: usize,
    pub bought: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Write an edited binary over the program in HDD slot `slot`. Refused unless it checks out.
    Patch {
        slot: usize,
        bin: Bin,
    },
    /// Spend fragments to raise a program's opt level (and budget), re-signing it.
    Optimize {
        slot: usize,
    },
    /// Buy file `file` in BBS area `area` (indices into `Game::bbs_stock`).
    Buy {
        area: usize,
        file: usize,
    },
    /// Jack into a linked node.
    Move(usize),
    /// Run the loaded program in HDD slot `n`.
    Run(usize),
    /// Load the program in HDD slot `n` into RAM.
    Load(usize),
    /// Unload the program in HDD slot `n`.
    Unload(usize),
    EndTurn,
    NewRun(u64),
    /// Pick choice `choice` in the current encounter; `slot` names the program for trades and sales.
    Choose {
        choice: usize,
        slot: Option<usize>,
    },
    /// Close a finished encounter.
    EndEncounter,
    /// Take job `job` from the BBS jobs board.
    TakeJob {
        job: usize,
    },
}

#[derive(Clone, Debug)]
pub struct Game {
    pub data: Data,
    pub seed: u64,
    pub rng: Rng,
    pub deck: Deck,
    pub hdd: Vec<Slot>,
    /// The network the runner is on: the main one, or a subnet while inside one.
    pub net: Network,
    /// Node the runner is on, in `net`.
    pub at: usize,
    /// The main network, set aside while the runner is inside a subnet.
    pub outer: Option<Box<Outer>>,
    /// What the main network was generated from; subnets take their shape from it.
    pub params: NetParams,
    pub combat: Option<Combat>,
    pub phase: Phase,
    pub log: Vec<LogLine>,
    pub hops: u32,
    /// This run's BBS listing: per area, the files on offer.
    pub bbs_stock: Vec<Vec<StockFile>>,
    /// This network's BBS: an index into `Data::boards`. Read it through [`Game::bbs`].
    pub board: usize,
    /// Whether the runner connected to the BBS: its number goes in the phonebook at home.
    pub bbs_seen: bool,
    /// Contracts posted on the BBS jobs board, rolled when you connect.
    pub bbs_jobs: Vec<Contract>,
    /// Contracts you've taken, in every state.
    pub contracts: Vec<Contract>,
    pub encounter: Option<Encounter>,
    /// Events already seen this run, so MSG nodes don't repeat while others remain.
    pub events_seen: Vec<usize>,
    pub next_uid: u32,
    pub mods: RunMods,
    pub site: Option<Site>,
    /// Number ranges learned this run (exchange ids). They come home even on a flatline.
    pub hints: Vec<String>,
}

impl Game {
    /// A run with the starter deck and starter programs.
    pub fn new(data: Data, seed: u64) -> Self {
        Self::start(data, seed, &Kit::default())
    }

    /// A run with a kit from home: RIG bonuses and the chosen loadout.
    pub fn start(data: Data, seed: u64, kit: &Kit) -> Self {
        let mut rng = Rng::new(seed);
        let net = Network::generate_with(&mut rng, &data, &kit.net);
        let board = if kit.board < data.boards.len() {
            kit.board
        } else {
            0
        };
        let bbs_stock = roll_stock(&data.boards[board], &mut rng);
        let deck = Deck {
            cpu_max: START_CPU + kit.cpu,
            ram_max: START_RAM + kit.ram,
            hdd_max: START_HDD + kit.hdd,
            integrity: START_INTEGRITY + kit.integrity,
            integrity_max: START_INTEGRITY + kit.integrity,
            trace: 0,
            credits: 0,
            fragments: 0,
        };
        let mut game = Self {
            at: net.entry(),
            outer: None,
            params: kit.net.clone(),
            data,
            seed,
            rng,
            deck,
            hdd: Vec::new(),
            net,
            combat: None,
            phase: Phase::Exploring,
            log: Vec::new(),
            hops: 0,
            bbs_stock,
            board,
            bbs_seen: false,
            bbs_jobs: Vec::new(),
            contracts: Vec::new(),
            encounter: None,
            events_seen: Vec::new(),
            next_uid: 1,
            mods: kit.mods,
            hints: Vec::new(),
            site: kit.site.clone(),
        };
        let programs: Vec<(usize, Bin)> = if kit.programs.is_empty() {
            START_WAREZ
                .iter()
                .filter_map(|id| game.data.warez_ix(id))
                .filter_map(|w| game.factory_slot(w).map(|s| (w, s.bin)))
                .collect()
        } else {
            kit.programs.clone()
        };
        for (w, mut bin) in programs {
            if game.hdd.len() as u32 >= game.deck.hdd_max || w >= game.data.warez.len() {
                break;
            }
            // Home binaries carry home's signature; the run signs its own.
            binary::seal(&mut bin, game.key());
            if let Ok(spec) = binary::check(&bin, game.key()) {
                let loaded = game.ram_used() + spec.ram <= game.deck.ram_max;
                game.add_slot(Slot {
                    uid: 0,
                    warez: w,
                    loaded,
                    bin,
                    spec,
                });
            }
        }
        game.say(LogKind::Sys, format!("ninsei v0.0.1 // seed {seed:016x}"));
        game.say(LogKind::Sys, "carrier detected. handshake ok.");
        let entry = game.net.nodes[game.at].name.clone();
        game.say(
            LogKind::Info,
            format!("jacked in at {entry}. reach the exit before trace hits {TRACE_MAX}%."),
        );
        game.say(LogKind::Sys, "type `help` for commands.");
        game.bring_jobs(&kit.jobs);
        game
    }

    /// Jobs taken at home: each gets a target node on this network, picked with its own RNG
    /// so the run plays out the same with or without them. A job with no target is dropped.
    fn bring_jobs(&mut self, jobs: &[crate::hub::PendingJob]) {
        for (i, j) in jobs.iter().enumerate() {
            let targets = self.contract_targets();
            if targets.is_empty() {
                continue;
            }
            let mut pick = Rng::new(self.seed ^ 0x10B5_0000 ^ i as u64);
            let node = targets[pick.below(targets.len())];
            let k = Contract {
                client: j.client.clone(),
                brief: j.brief.replace("{host}", &self.root().nodes[node].name),
                node,
                reward: j.reward,
                ghost: j.ghost,
                status: ContractStatus::Active,
            };
            self.say(
                LogKind::Good,
                format!("contract from home: {}", self.contract_line(&k)),
            );
            self.contracts.push(k);
        }
    }

    pub fn ram_used(&self) -> u32 {
        self.hdd
            .iter()
            .filter(|s| s.loaded)
            .map(|s| s.spec.ram)
            .sum()
    }

    /// The key binaries are signed with this run.
    pub fn key(&self) -> u64 {
        self.seed
    }

    /// A fresh, sealed copy of factory program `w`.
    pub fn factory_slot(&self, w: usize) -> Option<Slot> {
        let bin = binary::encode(&self.data.warez[w], self.key());
        let spec = binary::check(&bin, self.key()).ok()?;
        Some(Slot {
            uid: 0,
            warez: w,
            loaded: false,
            bin,
            spec,
        })
    }

    /// Puts a program on the HDD with a fresh uid.
    fn add_slot(&mut self, mut slot: Slot) {
        slot.uid = self.next_uid;
        self.next_uid += 1;
        self.hdd.push(slot);
    }

    /// The next thing the current ICE will do, if fighting.
    pub fn intent(&self) -> Option<Intent> {
        let c = self.combat.as_ref()?;
        let ice = &self.data.ice[c.ice];
        Some(ice.intents[c.intent_ix % ice.intents.len()])
    }

    /// The main network, even while inside a subnet. Contracts point into it.
    /// The BBS on this network.
    pub fn bbs(&self) -> &crate::data::BbsDef {
        &self.data.boards[self.board]
    }

    pub fn root(&self) -> &Network {
        self.outer.as_ref().map_or(&self.net, |o| &o.net)
    }

    /// Where the runner stands in the main network: inside a subnet, its subnet node.
    pub fn root_at(&self) -> usize {
        self.outer.as_ref().map_or(self.at, |o| o.node)
    }

    pub fn in_subnet(&self) -> bool {
        self.outer.is_some()
    }

    /// Whether the runner has been on node `id` of the network they're on. Routes only run
    /// forward and every node clears once you've been there, so visited nodes are your path.
    pub fn visited(&self, id: usize) -> bool {
        id == self.at || self.net.nodes.get(id).is_some_and(|n| n.cleared)
    }

    /// Whether the runner came along the link from `a` to `b`: the hop trail on the map.
    pub fn travelled(&self, a: usize, b: usize) -> bool {
        self.visited(a) && self.visited(b) && self.net.nodes[a].links.contains(&b)
    }

    /// What a node on the current network is called in the UI: a subnet's exit is its uplink.
    pub fn node_label(&self, node: usize) -> &'static str {
        match self.net.nodes[node].kind {
            NodeKind::Exit if self.in_subnet() => "UPLINK",
            kind => kind.label(),
        }
    }

    /// How deep a node is for ICE strength: a subnet counts from its door's layer.
    fn depth(&self, node: usize) -> u32 {
        let base = self.outer.as_ref().map_or(0, |o| o.net.nodes[o.node].layer);
        (base + self.net.nodes[node].layer) as u32
    }

    /// Nodes the runner may move to right now.
    pub fn reachable(&self) -> &[usize] {
        if self.phase == Phase::Exploring {
            &self.net.nodes[self.at].links
        } else {
            &[]
        }
    }

    pub fn say(&mut self, kind: LogKind, text: impl Into<String>) {
        self.log.push(LogLine {
            kind,
            text: text.into(),
        });
        if self.log.len() > LOG_LIMIT {
            let extra = self.log.len() - LOG_LIMIT;
            self.log.drain(..extra);
        }
    }

    /// Applies one player action. Illegal actions change nothing but a log line.
    pub fn apply(&mut self, action: Action) {
        if let Err(e) = self.try_apply(action) {
            self.say(LogKind::Sys, format!("err: {e}"));
        }
    }

    fn try_apply(&mut self, action: Action) -> Result<(), String> {
        match action {
            Action::NewRun(seed) => {
                let data = self.data.clone();
                *self = Game::new(data, seed);
                Ok(())
            }
            _ if matches!(self.phase, Phase::Flatlined | Phase::JackedOut) => {
                Err("run is over. `newrun` to jack in again.".into())
            }
            Action::Move(to) => self.move_to(to),
            Action::Choose { choice, slot } => self.choose(choice, slot),
            Action::EndEncounter => {
                self.encounter = self.encounter.take().filter(|e| !e.done);
                Ok(())
            }
            Action::TakeJob { job } => self.take_job(job),
            Action::Run(slot) => self.run(slot),
            Action::Load(slot) => self.load(slot),
            Action::Unload(slot) => self.unload(slot),
            Action::EndTurn => self.end_turn(),
            Action::Buy { area, file } => self.buy(area, file),
            Action::Patch { slot, bin } => self.patch(slot, bin),
            Action::Optimize { slot } => self.optimize(slot),
        }
    }

    fn move_to(&mut self, to: usize) -> Result<(), String> {
        if self.phase == Phase::Combat {
            return Err("ICE is holding the connection. break it first.".into());
        }
        if self.phase == Phase::Encounter {
            return Err("someone's still talking. answer them first.".into());
        }
        if !self.net.nodes[self.at].links.contains(&to) {
            return Err(format!("no route to node {to}."));
        }
        self.at = to;
        self.hops += 1;
        self.encounter = None;
        self.deck.trace += self.mods.trace(TRACE_PER_HOP);
        let node = self.net.nodes[to].clone();
        let label = self.node_label(to);
        self.say(
            LogKind::Info,
            format!("> routing to {} [{label}]", node.name),
        );
        if self.check_trace() {
            return Ok(());
        }
        match node.kind {
            NodeKind::Ice | NodeKind::BlackIce if !node.cleared => {
                if let Some(ice) = node.ice {
                    self.start_combat(to, ice);
                }
            }
            NodeKind::Vault if !node.cleared => {
                self.net.nodes[to].cleared = true;
                self.say(LogKind::Good, "vault unlocked. pulling data...");
                let table = self.data.loot.vault.clone();
                self.roll_loot(&table);
            }
            NodeKind::Bbs => {
                self.net.nodes[to].cleared = true;
                let b = self.bbs();
                let line = format!("ATDT {} ... CONNECT {} to {}", b.phone, b.baud, b.name);
                self.say(LogKind::Good, line);
                if !self.bbs_seen {
                    let note = format!(
                        "{} noted. it'll be in your phonebook at home.",
                        self.bbs().phone
                    );
                    self.say(LogKind::Info, note);
                }
                self.bbs_seen = true;
                self.bbs_jobs = (0..BBS_JOBS)
                    .filter_map(|_| self.roll_contract(true))
                    .collect();
            }
            NodeKind::Contact if !node.cleared => {
                self.net.nodes[to].cleared = true;
                self.start_encounter(to);
            }
            NodeKind::Subnet if !node.cleared && self.outer.is_none() => {
                self.dive(to);
            }
            NodeKind::Exit if self.outer.is_some() => {
                self.surface();
            }
            NodeKind::Exit => {
                self.net.nodes[to].cleared = true;
                self.phase = Phase::JackedOut;
                self.say(
                    LogKind::Good,
                    format!(
                        "exit reached. jacked out clean with {}¢.",
                        self.deck.credits
                    ),
                );
            }
            _ => {}
        }
        self.void_unreachable_contracts();
        Ok(())
    }

    /// Drops through subnet node `node` into the private network behind it.
    fn dive(&mut self, node: usize) {
        let sub = Network::subnet(self.seed, node, &self.data, &self.params);
        let name = self.net.nodes[node].name.clone();
        let net = std::mem::replace(&mut self.net, sub);
        self.outer = Some(Box::new(Outer { net, node }));
        self.at = self.net.entry();
        self.say(
            LogKind::Info,
            format!("subnet {name}: dropping into a private segment. its exit uplinks back here."),
        );
    }

    /// Takes the subnet's exit back up to its door in the main network.
    fn surface(&mut self) {
        let Some(o) = self.outer.take() else { return };
        self.net = o.net;
        self.at = o.node;
        self.net.nodes[o.node].cleared = true;
        let name = self.net.nodes[o.node].name.clone();
        self.say(LogKind::Good, format!("uplink: back out at {name}."));
    }

    fn start_combat(&mut self, node: usize, ice: usize) {
        let strength = self.data.ice[ice].strength
            + ICE_STRENGTH_PER_LAYER * self.depth(node)
            + self.mods.ice_bonus;
        let def = &self.data.ice[ice];
        let (name, class) = (def.name.clone(), def.class.label());
        self.combat = Some(Combat {
            node,
            ice,
            strength,
            strength_max: strength,
            intent_ix: 0,
            cpu: self.deck.cpu_max,
            shield: 0,
            stunned: false,
            stun_immune: false,
            turn: 1,
            bonus: Vec::new(),
            leet: false,
        });
        self.phase = Phase::Combat;
        self.say(
            LogKind::Bad,
            format!("!! {name} rezzed [{class}] strength {strength}"),
        );
    }

    fn slot(&self, slot: usize) -> Result<Slot, String> {
        self.hdd
            .get(slot)
            .cloned()
            .ok_or_else(|| format!("no program in slot {slot}."))
    }

    fn run(&mut self, slot: usize) -> Result<(), String> {
        let s = self.slot(slot)?;
        let w = s.spec;
        if !s.loaded {
            return Err(format!("{} is not loaded.", w.name));
        }
        let Some(c) = self.combat.as_mut() else {
            return Err("nothing to run against.".into());
        };
        if c.cpu < w.cpu {
            return Err(format!("{} needs {} cpu, {} left.", w.name, w.cpu, c.cpu));
        }
        if w.effect == Effect::Stun && c.stun_immune {
            return Err("the ICE has adapted to lag. a stun won't land this turn.".into());
        }
        c.cpu -= w.cpu;
        match w.effect {
            Effect::Break { power, vs } => {
                let class = self.data.ice[c.ice].class;
                // Specialised breakers double against their class and halve against the rest.
                let (hit, crit) = match vs {
                    Some(v) if v == class => (power * 2, " (class match x2)"),
                    Some(_) => (power / 2, " (wrong class /2)"),
                    None => (power, ""),
                };
                c.strength = c.strength.saturating_sub(hit);
                self.say(LogKind::Good, format!("{} breaks {hit}{crit}", w.name));
                // Derezzed, or beaten badly enough to quit.
                if self.combat.as_ref().is_some_and(|c| c.strength == 0) || self.gives_up() {
                    self.win_combat();
                }
            }
            Effect::Shield(n) => {
                c.shield += n;
                self.say(LogKind::Good, format!("{} up: shield {n}", w.name));
            }
            Effect::Repair(n) => {
                self.deck.integrity = (self.deck.integrity + n).min(self.deck.integrity_max);
                self.say(LogKind::Good, format!("{} restores {n} integrity", w.name));
            }
            Effect::Scrub(n) => {
                self.deck.trace = self.deck.trace.saturating_sub(n);
                self.say(LogKind::Good, format!("{} scrubs {n} trace", w.name));
            }
            Effect::Stun => {
                c.stunned = true;
                self.say(
                    LogKind::Good,
                    format!("{} lags the ICE. it skips a beat.", w.name),
                );
            }
        }
        Ok(())
    }

    /// Rolls whether a beaten fighter quits. Says its line if it does.
    fn gives_up(&mut self) -> bool {
        let Some(c) = self.combat.as_ref() else {
            return false;
        };
        let Some(y) = self.data.ice[c.ice].yields.clone() else {
            return false;
        };
        let leet = c.leet;
        if c.strength * 100 > y.below * c.strength_max || !self.rng.chance(y.chance) {
            return false;
        }
        let name = self.data.ice[c.ice].name.clone();
        let line = if leet {
            crate::leet::translate(&y.line)
        } else {
            y.line
        };
        self.say(LogKind::Info, format!("<{name}> {line}"));
        self.say(
            LogKind::Good,
            format!("{name} gives up and drops the line."),
        );
        true
    }

    fn win_combat(&mut self) {
        let Some(c) = self.combat.take() else { return };
        self.phase = Phase::Exploring;
        self.net.nodes[c.node].cleared = true;
        let def = &self.data.ice[c.ice];
        let name = def.name.clone();
        let table = if def.tier >= 2 {
            self.data.loot.black_ice.clone()
        } else {
            self.data.loot.ice.clone()
        };
        if c.strength == 0 {
            self.say(LogKind::Good, format!("{name} derezzed. node clear."));
        }
        self.roll_loot(&table);
        if !c.bonus.is_empty() {
            self.say(LogKind::Good, "as promised:");
            for b in c.bonus {
                self.grant(b);
            }
        }
        // Contracts target the main network; a subnet's node ids mean nothing to them.
        if self.outer.is_none() {
            self.settle_contracts(c.node);
        }
    }

    fn end_turn(&mut self) -> Result<(), String> {
        let mods = self.mods;
        let Some(c) = self.combat.as_mut() else {
            return Err("no fight in progress.".into());
        };
        let def = &self.data.ice[c.ice];
        let intent = def.intents[c.intent_ix % def.intents.len()];
        let name = def.name.clone();
        let mut lines = Vec::new();
        c.stun_immune = false;
        if c.stunned {
            c.stunned = false;
            c.stun_immune = true;
            lines.push((LogKind::Good, format!("{name} is lagged and does nothing.")));
        } else {
            match intent {
                Intent::Attack(n) => {
                    let blocked = n.min(c.shield);
                    let dmg = n - blocked;
                    self.deck.integrity = self.deck.integrity.saturating_sub(dmg);
                    let note = if blocked > 0 {
                        format!(" ({blocked} blocked)")
                    } else {
                        String::new()
                    };
                    lines.push((LogKind::Bad, format!("{name} hits for {dmg}{note}")));
                }
                Intent::Trace(n) => {
                    let n = mods.trace(n);
                    self.deck.trace += n;
                    lines.push((LogKind::Bad, format!("{name} traces +{n}")));
                }
                Intent::Fortify(n) => {
                    c.strength += n;
                    c.strength_max = c.strength_max.max(c.strength);
                    lines.push((LogKind::Bad, format!("{name} fortifies +{n}")));
                }
            }
        }
        c.intent_ix += 1;
        c.shield = 0;
        c.cpu = self.deck.cpu_max;
        c.turn += 1;
        self.deck.trace += mods.trace(TRACE_PER_COMBAT_TURN);
        for (k, t) in lines {
            self.say(k, t);
        }
        if self.deck.integrity == 0 {
            self.phase = Phase::Flatlined;
            self.combat = None;
            self.say(LogKind::Bad, "!! deck integrity zero. FLATLINE.");
            return Ok(());
        }
        self.check_trace();
        Ok(())
    }

    // ---- encounters ----------------------------------------------------------

    fn start_encounter(&mut self, node: usize) {
        let n = self.data.events.len();
        let fresh: Vec<usize> = (0..n).filter(|i| !self.events_seen.contains(i)).collect();
        let event = if fresh.is_empty() {
            self.rng.below(n)
        } else {
            fresh[self.rng.below(fresh.len())]
        };
        self.events_seen.push(event);
        let ev = &self.data.events[event];
        let pool = ev.choices.iter().find_map(|c| match &c.effect {
            ChoiceEffect::Trade { pool } => Some(pool.clone()),
            _ => None,
        });
        let offer = pool.and_then(|p| {
            let id = &p[self.rng.below(p.len())];
            self.data.warez_ix(id)
        });
        let line = format!("<{}> ...incoming message: {}", ev.from, ev.title);
        self.encounter = Some(Encounter {
            event,
            node,
            offer,
            done: false,
            result: Vec::new(),
        });
        self.phase = Phase::Encounter;
        self.say(LogKind::Info, line);
    }

    /// Fills `{offer}`, `{per_ram}` and `{cost}` in an event's text.
    pub fn fill(&self, text: &str, choice: Option<usize>) -> String {
        let Some(e) = &self.encounter else {
            return text.to_string();
        };
        let ev = &self.data.events[e.event];
        let offer = e
            .offer
            .map_or("nothing".to_string(), |w| self.data.warez[w].name.clone());
        let mut out = text.replace("{offer}", &offer);
        let effects: Vec<&ChoiceEffect> = match choice {
            Some(c) => ev.choices.get(c).map(|c| &c.effect).into_iter().collect(),
            None => ev.choices.iter().map(|c| &c.effect).collect(),
        };
        for eff in effects {
            match eff {
                ChoiceEffect::Sell { per_ram } => {
                    out = out.replace("{per_ram}", &per_ram.to_string())
                }
                ChoiceEffect::Pay { cost, .. } => out = out.replace("{cost}", &cost.to_string()),
                _ => {}
            }
        }
        if ev.leet {
            crate::leet::translate(&out)
        } else {
            out
        }
    }

    /// The current encounter's title, in its own voice.
    pub fn encounter_title(&self) -> String {
        let Some(e) = &self.encounter else {
            return String::new();
        };
        let ev = &self.data.events[e.event];
        if ev.leet {
            crate::leet::translate(&ev.title)
        } else {
            ev.title.clone()
        }
    }

    /// Whether choice `c` asks you to pick one of your programs.
    pub fn choice_needs_slot(&self, c: usize) -> bool {
        self.encounter.as_ref().is_some_and(|e| {
            matches!(
                self.data.events[e.event].choices.get(c).map(|c| &c.effect),
                Some(ChoiceEffect::Trade { .. } | ChoiceEffect::Sell { .. })
            )
        })
    }

    /// Why choice `c` can't be picked, if it can't (with `slot` where one is needed).
    pub fn choice_blocker(&self, c: usize, slot: Option<usize>) -> Option<String> {
        let e = self.encounter.as_ref()?;
        if e.done || self.phase != Phase::Encounter {
            return Some("this conversation is over.".into());
        }
        let Some(choice) = self.data.events[e.event].choices.get(c) else {
            return Some(format!("no choice {}.", c + 1));
        };
        let needs_slot = self.choice_needs_slot(c);
        if needs_slot {
            match slot {
                None => return Some("pick a program.".into()),
                Some(s) if s >= self.hdd.len() => return Some(format!("no program in slot {s}.")),
                _ => {}
            }
        }
        match &choice.effect {
            ChoiceEffect::Sell { .. } if self.hdd.len() <= 1 => {
                Some("you can't sell your last program.".into())
            }
            ChoiceEffect::Trade { .. } if e.offer.is_none() => Some("nothing on offer.".into()),
            ChoiceEffect::Pay { cost, .. } if self.deck.credits < *cost => {
                Some(format!("needs {cost}¢, you have {}¢.", self.deck.credits))
            }
            ChoiceEffect::Contract if self.contract_targets().is_empty() => {
                Some("no targets left ahead.".into())
            }
            _ => None,
        }
    }

    /// What a program fetches from a fence paying `per_ram` a GB.
    pub fn sale_price(&self, slot: usize, per_ram: u32) -> u32 {
        self.hdd
            .get(slot)
            .map_or(0, |s| per_ram * s.spec.ram + SELL_PER_OPT * s.spec.opt)
    }

    fn choose(&mut self, c: usize, slot: Option<usize>) -> Result<(), String> {
        if self.encounter.is_none() {
            return Err("nobody's talking to you.".into());
        }
        if let Some(why) = self.choice_blocker(c, slot) {
            return Err(why);
        }
        let Some(e) = self.encounter.clone() else {
            return Ok(());
        };
        let effect = self.data.events[e.event].choices[c].effect.clone();
        let from = self.data.events[e.event].from.clone();
        let mark = self.log.len();
        self.phase = Phase::Exploring;
        match effect {
            ChoiceEffect::Leave => {
                self.say(LogKind::Info, format!("you cut the line on <{from}>."))
            }
            ChoiceEffect::Trade { .. } => {
                let (Some(i), Some(w)) = (slot, e.offer) else {
                    return Ok(());
                };
                let gave = self.hdd[i].spec.file();
                if let Some(mut new) = self.factory_slot(w) {
                    new.uid = self.next_uid;
                    self.next_uid += 1;
                    let got = new.spec.file();
                    self.hdd[i] = new;
                    self.say(LogKind::Loot, format!("traded {gave} for {got}."));
                }
            }
            ChoiceEffect::Sell { per_ram } => {
                let Some(i) = slot else { return Ok(()) };
                let price = self.sale_price(i, per_ram);
                let gone = self.hdd.remove(i);
                self.deck.credits += price;
                self.say(
                    LogKind::Loot,
                    format!("sold {} for {price}¢.", gone.spec.file()),
                );
            }
            ChoiceEffect::Contract => {
                if let Some(mut k) = self.roll_contract(false) {
                    k.status = ContractStatus::Active;
                    self.say(
                        LogKind::Good,
                        format!("contract taken: {}", self.contract_line(&k)),
                    );
                    self.contracts.push(k);
                }
            }
            ChoiceEffect::Pay { cost, get } => {
                self.deck.credits -= cost;
                self.say(LogKind::Info, format!("paid {cost}¢."));
                self.grant(get);
            }
            ChoiceEffect::Gamble { pct, win, lose } => {
                if self.rng.chance(pct) {
                    self.say(LogKind::Good, "it pays off.");
                    self.grant(win);
                } else {
                    self.say(LogKind::Bad, "it was a trap.");
                    self.penalize(lose);
                }
            }
            ChoiceEffect::Fight { ice, rewards } => {
                if let Some(ix) = self.data.ice.iter().position(|i| i.id == ice) {
                    let leet = self.data.events[e.event].leet;
                    self.encounter = None;
                    self.start_combat(e.node, ix);
                    if let Some(c) = self.combat.as_mut() {
                        c.bonus = rewards;
                        c.leet = leet;
                    }
                    return Ok(());
                }
            }
        }
        let result = self.log[mark..].to_vec();
        if let Some(enc) = self.encounter.as_mut() {
            enc.done = true;
            enc.result = result;
        }
        Ok(())
    }

    fn penalize(&mut self, p: Penalty) {
        match p {
            Penalty::Trace(n) => {
                let n = self.mods.trace(n);
                self.deck.trace += n;
                self.say(LogKind::Bad, format!("trace +{n}"));
                self.check_trace();
            }
            Penalty::Damage(n) => {
                self.deck.integrity = self.deck.integrity.saturating_sub(n);
                self.say(LogKind::Bad, format!("deck takes {n} damage"));
                if self.deck.integrity == 0 {
                    self.phase = Phase::Flatlined;
                    self.say(LogKind::Bad, "!! deck integrity zero. FLATLINE.");
                }
            }
        }
    }

    /// The choice that walks away, for bots and the escape key.
    pub fn leave_choice(&self) -> Option<usize> {
        let e = self.encounter.as_ref()?;
        self.data.events[e.event]
            .choices
            .iter()
            .position(|c| c.effect == ChoiceEffect::Leave)
    }

    // ---- contracts -----------------------------------------------------------

    /// ICE nodes ahead that nobody has a contract on yet.
    fn contract_targets(&self) -> Vec<usize> {
        let (net, at) = (self.root(), self.root_at());
        let reach = net.reachable_from(at);
        let taken = |n: usize| {
            self.contracts
                .iter()
                .chain(&self.bbs_jobs)
                .any(|k| k.node == n && k.status != ContractStatus::Failed)
        };
        net.nodes
            .iter()
            .filter(|n| reach[n.id] && n.id != at && !n.cleared)
            .filter(|n| matches!(n.kind, NodeKind::Ice | NodeKind::BlackIce))
            .filter(|n| !taken(n.id))
            .map(|n| n.id)
            .collect()
    }

    fn roll_contract(&mut self, offered: bool) -> Option<Contract> {
        let targets = self.contract_targets();
        if targets.is_empty() {
            return None;
        }
        let node = targets[self.rng.below(targets.len())];
        let t = &self.data.contracts[self.rng.below(self.data.contracts.len())];
        let (client, brief) = (
            t.client.clone(),
            t.brief.replace("{host}", &self.root().nodes[node].name),
        );
        let n = &self.root().nodes[node];
        let mult = if n.kind == NodeKind::BlackIce {
            CONTRACT_BLACK_MULT
        } else {
            1
        };
        let layer = n.layer as u32;
        let reward = self.rng.range(CONTRACT_REWARD_MIN, CONTRACT_REWARD_MAX) * mult
            + CONTRACT_PER_LAYER * layer;
        let ghost = self.rng.chance(CONTRACT_GHOST_PCT).then(|| {
            let cap = (self.deck.trace + CONTRACT_GHOST_MARGIN).min(TRACE_MAX - 1);
            (cap, reward * CONTRACT_GHOST_BONUS_PCT / 100)
        });
        let status = if offered {
            ContractStatus::Offered
        } else {
            ContractStatus::Active
        };
        Some(Contract {
            client,
            brief,
            node,
            reward,
            ghost,
            status,
        })
    }

    /// One-line summary of a contract for logs.
    pub fn contract_line(&self, k: &Contract) -> String {
        let n = &self.root().nodes[k.node];
        let ghost = k.ghost.map_or(String::new(), |(cap, b)| {
            format!(" +{b}¢ if trace <= {cap}%")
        });
        format!(
            "derez {} [node {}] for {}, {}¢{ghost}",
            n.name, n.id, k.client, k.reward
        )
    }

    fn take_job(&mut self, job: usize) -> Result<(), String> {
        if !self.at_bbs() {
            return Err("the jobs board is on the BBS.".into());
        }
        let Some(k) = self.bbs_jobs.get(job) else {
            return Err(format!("no job {}.", job + 1));
        };
        if k.status != ContractStatus::Offered {
            return Err("that job's taken.".into());
        }
        let mut k = k.clone();
        k.status = ContractStatus::Active;
        self.bbs_jobs[job].status = ContractStatus::Active;
        self.say(
            LogKind::Good,
            format!("contract taken: {}", self.contract_line(&k)),
        );
        self.contracts.push(k);
        Ok(())
    }

    /// Pays out contracts on a node that just fell.
    fn settle_contracts(&mut self, node: usize) {
        let trace = self.deck.trace;
        let mut pay = Vec::new();
        for k in &mut self.contracts {
            if k.node == node && k.status == ContractStatus::Active {
                k.status = ContractStatus::Done;
                let bonus = k
                    .ghost
                    .filter(|&(cap, _)| trace <= cap)
                    .map_or(0, |(_, b)| b);
                pay.push((k.client.clone(), k.reward, bonus));
            }
        }
        for (client, reward, bonus) in pay {
            let (reward, bonus) = (self.mods.pay(reward), self.mods.pay(bonus));
            self.deck.credits += reward + bonus;
            self.say(
                LogKind::Loot,
                format!("contract complete. {client} pays {reward}¢."),
            );
            if bonus > 0 {
                self.say(
                    LogKind::Loot,
                    format!("+ ghost bonus {bonus}¢. clean work."),
                );
            }
        }
    }

    /// Fails active contracts whose target can no longer be reached.
    fn void_unreachable_contracts(&mut self) {
        let reach = self.root().reachable_from(self.root_at());
        let mut lost = Vec::new();
        for k in &mut self.contracts {
            if k.status == ContractStatus::Active && !reach[k.node] {
                k.status = ContractStatus::Failed;
                lost.push(k.client.clone());
            }
        }
        for client in lost {
            self.say(
                LogKind::Bad,
                format!("contract void: {client}'s target is out of reach."),
            );
        }
    }

    /// Flatlines the runner if trace is maxed. Returns true if it did.
    fn check_trace(&mut self) -> bool {
        if self.deck.trace >= TRACE_MAX {
            self.deck.trace = TRACE_MAX;
            self.phase = Phase::Flatlined;
            self.combat = None;
            self.say(LogKind::Bad, "!! trace complete. they found you. FLATLINE.");
            return true;
        }
        false
    }

    fn load(&mut self, slot: usize) -> Result<(), String> {
        let s = self.slot(slot)?;
        let name = s.spec.name.clone();
        if s.loaded {
            return Err(format!("{name} is already loaded."));
        }
        if self.ram_used() + s.spec.ram > self.deck.ram_max {
            return Err(format!(
                "not enough ram for {name} ({} needed).",
                s.spec.ram
            ));
        }
        if let Some(c) = self.combat.as_mut() {
            if c.cpu < COMBAT_LOAD_CPU {
                return Err(format!("loading mid-fight costs {COMBAT_LOAD_CPU} cpu."));
            }
            c.cpu -= COMBAT_LOAD_CPU;
        }
        self.hdd[slot].loaded = true;
        self.say(LogKind::Info, format!("{name} loaded."));
        Ok(())
    }

    fn unload(&mut self, slot: usize) -> Result<(), String> {
        let s = self.slot(slot)?;
        let name = s.spec.name.clone();
        if !s.loaded {
            return Err(format!("{name} is not loaded."));
        }
        self.hdd[slot].loaded = false;
        self.say(LogKind::Info, format!("{name} unloaded."));
        Ok(())
    }

    pub fn credits_ok(&self, price: u32) -> bool {
        self.deck.credits >= price
    }

    /// Whether the runner is sitting on a BBS and free to browse it.
    pub fn at_bbs(&self) -> bool {
        self.phase == Phase::Exploring && self.net.nodes[self.at].kind == NodeKind::Bbs
    }

    /// Why a BBS file can't be bought right now, if it can't.
    pub fn buy_blocker(&self, area: usize, file: usize) -> Option<String> {
        let Some(s) = self.bbs_stock.get(area).and_then(|a| a.get(file)) else {
            return Some("no such file.".into());
        };
        let def = &self.bbs().areas[area].files[s.def];
        if !self.at_bbs() {
            return Some("carrier lost. you're not on the board.".into());
        }
        if s.bought {
            return Some(format!("{} already downloaded.", def.file));
        }
        if self.deck.credits < def.price {
            return Some(format!(
                "insufficient credits: {} needs {}¢, you have {}¢.",
                def.file, def.price, self.deck.credits
            ));
        }
        if matches!(def.item, Drop::Warez(_)) && self.hdd.len() as u32 >= self.deck.hdd_max {
            return Some("hdd full. free a slot before downloading warez.".into());
        }
        None
    }

    fn buy(&mut self, area: usize, file: usize) -> Result<(), String> {
        if let Some(why) = self.buy_blocker(area, file) {
            return Err(why);
        }
        let s = self.bbs_stock[area][file];
        let def = self.bbs().areas[area].files[s.def].clone();
        self.deck.credits -= def.price;
        self.bbs_stock[area][file].bought = true;
        self.say(
            LogKind::Info,
            format!("downloaded {} (-{}¢)", def.file, def.price),
        );
        self.grant(def.item);
        Ok(())
    }

    /// Why the hex editor can't write right now, if it can't. In a run you need a BBS's compiler.
    pub fn compile_blocker(&self) -> Option<&'static str> {
        if self.phase == Phase::Combat {
            Some("can't recompile mid-fight. the ICE won't wait.")
        } else if !self.at_bbs() {
            Some("no compiler out here. connect to a BBS, or wait till you're home.")
        } else {
            None
        }
    }

    fn patch(&mut self, slot: usize, bin: Bin) -> Result<(), String> {
        if let Some(why) = self.compile_blocker() {
            return Err(why.into());
        }
        let s = self.slot(slot)?;
        let spec = binary::check(&bin, self.key())
            .map_err(|f| format!("link failed: {}", f.join("; ")))?;
        let others = self.ram_used() - if s.loaded { s.spec.ram } else { 0 };
        let still_fits = others + spec.ram <= self.deck.ram_max;
        let old = s.spec.file();
        let new = spec.file();
        let target = &mut self.hdd[slot];
        target.bin = bin;
        target.spec = spec;
        let unloaded = target.loaded && !still_fits;
        if unloaded {
            target.loaded = false;
        }
        let renamed = if old == new {
            String::new()
        } else {
            format!(" (was {old})")
        };
        self.say(LogKind::Good, format!("wrote {new}{renamed}. link ok."));
        if unloaded {
            self.say(
                LogKind::Sys,
                format!("{new} no longer fits in ram. unloaded."),
            );
        }
        Ok(())
    }

    fn optimize(&mut self, slot: usize) -> Result<(), String> {
        if let Some(why) = self.compile_blocker() {
            return Err(why.into());
        }
        let s = self.slot(slot)?;
        if s.spec.opt >= MAX_OPT {
            return Err(format!("{} is fully optimized.", s.spec.file()));
        }
        if self.deck.fragments < FRAGMENTS_PER_OPT {
            return Err(format!(
                "optimizing needs {FRAGMENTS_PER_OPT} fragment(s). you have {}.",
                self.deck.fragments
            ));
        }
        let mut bin = s.bin;
        bin[binary::OFF_OPT] += 1;
        binary::seal(&mut bin, self.key());
        let spec = binary::check(&bin, self.key()).map_err(|f| f.join("; "))?;
        self.deck.fragments -= FRAGMENTS_PER_OPT;
        let msg = format!(
            "{} optimized to O{} (+{BUDGET_PER_OPT} budget).",
            spec.file(),
            spec.opt
        );
        self.hdd[slot].bin = bin;
        self.hdd[slot].spec = spec;
        self.say(LogKind::Good, msg);
        Ok(())
    }

    fn roll_loot(&mut self, table: &LootTable) {
        for _ in 0..table.rolls {
            let Some(i) = self.rng.weighted(table.entries.iter().map(|e| e.weight)) else {
                return;
            };
            let drop = table.entries[i].drop.clone();
            self.grant(drop);
        }
    }

    fn grant(&mut self, drop: Drop) {
        match drop {
            Drop::Warez(id) => {
                let Some(slot) = self.data.warez_ix(&id).and_then(|w| self.factory_slot(w)) else {
                    return;
                };
                let name = slot.spec.name.clone();
                if self.hdd.len() as u32 >= self.deck.hdd_max {
                    self.say(
                        LogKind::Sys,
                        format!("found {name}, but the hdd is full. discarded."),
                    );
                } else {
                    self.add_slot(slot);
                    self.say(LogKind::Loot, format!("+ warez: {name}"));
                }
            }
            Drop::Credits(lo, hi) => {
                let n = self.mods.pay(self.rng.range(lo, hi));
                self.deck.credits += n;
                self.say(LogKind::Loot, format!("+ {n}¢"));
            }
            Drop::Ram(n) => {
                self.deck.ram_max += n;
                self.say(LogKind::Loot, format!("+ ram upgrade: +{n}"));
            }
            Drop::Cpu(n) => {
                self.deck.cpu_max += n;
                self.say(LogKind::Loot, format!("+ cpu upgrade: +{n}"));
            }
            Drop::Hdd(n) => {
                self.deck.hdd_max += n;
                self.say(LogKind::Loot, format!("+ hdd upgrade: +{n} slots"));
            }
            Drop::Repair(n) => {
                self.deck.integrity = (self.deck.integrity + n).min(self.deck.integrity_max);
                self.say(LogKind::Loot, format!("+ repair kit: +{n} integrity"));
            }
            Drop::Fragment(n) => {
                self.deck.fragments += n;
                self.say(LogKind::Loot, format!("+ optimizer fragment x{n}"));
            }
            Drop::Hint(range) => {
                let label = self
                    .data
                    .exchanges
                    .iter()
                    .find(|r| r.id == range)
                    .map_or(range.clone(), |r| format!("{} ({})", r.label, r.pattern()));
                if !self.hints.contains(&range) {
                    self.hints.push(range);
                }
                self.say(LogKind::Loot, format!("+ phone range: {label}"));
            }
            Drop::Scrub(n) => {
                self.deck.trace = self.deck.trace.saturating_sub(n);
                self.say(LogKind::Loot, format!("+ trace scrubbed by {n}"));
            }
            Drop::Nothing => self.say(LogKind::Sys, "nothing worth taking."),
        }
    }
}

/// A board's listing for one visit: per area, `stock` files picked at random.
pub fn roll_stock(def: &crate::data::BbsDef, rng: &mut Rng) -> Vec<Vec<StockFile>> {
    def.areas
        .iter()
        .map(|a| {
            let mut pool: Vec<usize> = (0..a.files.len()).collect();
            let mut picked = Vec::new();
            while picked.len() < a.stock as usize && !pool.is_empty() {
                picked.push(pool.swap_remove(rng.below(pool.len())));
            }
            picked.sort_unstable();
            picked
                .into_iter()
                .map(|def| StockFile { def, bought: false })
                .collect()
        })
        .collect()
}
