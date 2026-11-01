//! Run state and the rules that change it. The app only ever calls [`Game::apply`].

use crate::binary::{self, Bin, Spec};
use crate::data::{Data, Drop, Effect, Intent, LootTable};
use crate::network::{Network, NodeKind};
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Exploring,
    Combat,
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
}

#[derive(Clone, Debug)]
pub struct Game {
    pub data: Data,
    pub seed: u64,
    pub rng: Rng,
    pub deck: Deck,
    pub hdd: Vec<Slot>,
    pub net: Network,
    /// Node the runner is on.
    pub at: usize,
    pub combat: Option<Combat>,
    pub phase: Phase,
    pub log: Vec<LogLine>,
    pub hops: u32,
    /// This run's BBS listing: per area, the files on offer.
    pub bbs_stock: Vec<Vec<StockFile>>,
}

impl Game {
    pub fn new(data: Data, seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let net = Network::generate(&mut rng, &data);
        let bbs_stock = data
            .bbs
            .areas
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
            .collect();
        let deck = Deck {
            cpu_max: START_CPU,
            ram_max: START_RAM,
            hdd_max: START_HDD,
            integrity: START_INTEGRITY,
            integrity_max: START_INTEGRITY,
            trace: 0,
            credits: 0,
            fragments: 0,
        };
        let mut game = Self {
            at: net.entry(),
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
        };
        for id in START_WAREZ {
            if let Some(w) = game.data.warez_ix(id)
                && let Some(mut slot) = game.factory_slot(w)
            {
                slot.loaded = game.ram_used() + slot.spec.ram <= game.deck.ram_max;
                game.hdd.push(slot);
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
        game
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
            warez: w,
            loaded: false,
            bin,
            spec,
        })
    }

    /// The next thing the current ICE will do, if fighting.
    pub fn intent(&self) -> Option<Intent> {
        let c = self.combat.as_ref()?;
        let ice = &self.data.ice[c.ice];
        Some(ice.intents[c.intent_ix % ice.intents.len()])
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
        if !self.net.nodes[self.at].links.contains(&to) {
            return Err(format!("no route to node {to}."));
        }
        self.at = to;
        self.hops += 1;
        self.deck.trace += TRACE_PER_HOP;
        let node = self.net.nodes[to].clone();
        self.say(
            LogKind::Info,
            format!("> routing to {} [{}]", node.name, node.kind.label()),
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
                let b = &self.data.bbs;
                let line = format!("ATDT {} ... CONNECT {} to {}", b.phone, b.baud, b.name);
                self.say(LogKind::Good, line);
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
        Ok(())
    }

    fn start_combat(&mut self, node: usize, ice: usize) {
        let def = &self.data.ice[ice];
        let strength = def.strength + ICE_STRENGTH_PER_LAYER * self.net.nodes[node].layer as u32;
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
                if self.combat.as_ref().is_some_and(|c| c.strength == 0) {
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
        self.say(LogKind::Good, format!("{name} derezzed. node clear."));
        self.roll_loot(&table);
    }

    fn end_turn(&mut self) -> Result<(), String> {
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
        self.deck.trace += TRACE_PER_COMBAT_TURN;
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
        let def = &self.data.bbs.areas[area].files[s.def];
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
        let def = self.data.bbs.areas[area].files[s.def].clone();
        self.deck.credits -= def.price;
        self.bbs_stock[area][file].bought = true;
        self.say(
            LogKind::Info,
            format!("downloaded {} (-{}¢)", def.file, def.price),
        );
        self.grant(def.item);
        Ok(())
    }

    fn patch(&mut self, slot: usize, bin: Bin) -> Result<(), String> {
        if self.phase == Phase::Combat {
            return Err("can't recompile mid-fight. the ICE won't wait.".into());
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
        if self.phase == Phase::Combat {
            return Err("can't optimize mid-fight.".into());
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
                    self.hdd.push(slot);
                    self.say(LogKind::Loot, format!("+ warez: {name}"));
                }
            }
            Drop::Credits(lo, hi) => {
                let n = self.rng.range(lo, hi);
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
            Drop::Nothing => self.say(LogKind::Sys, "nothing worth taking."),
        }
    }
}
