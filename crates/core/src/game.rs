//! Run state and the rules that change it. The app only ever calls [`Game::apply`].

use crate::basic::{Cond, Program, Stmt, Target, Var, Vocab};
use crate::binary::{self, Bin, Spec};
use crate::data::{BetrayalEffect, ChoiceEffect, Data, Drop, Intent, LootTable, Penalty};
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
    /// The chassis' ceilings: nothing bought or found takes a line past them.
    pub caps: Caps,
}

/// A deck's ceilings per line (`None`: no ceiling), from its `stats` rows in `hardware.ron`:
/// the lines a deck mod or loot can raise in a run. Integrity only grows at home, where
/// `Save::upgrade_blocker` holds its ceiling.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Caps {
    pub cpu: Option<u32>,
    pub ram: Option<u32>,
    pub hdd: Option<u32>,
}

impl Deck {
    /// For a deck mod or a loot drop: the line's name, where it stands, and its ceiling.
    fn line(&self, item: &Drop) -> Option<(&'static str, u32, Option<u32>)> {
        match item {
            Drop::Cpu(_) => Some(("cpu", self.cpu_max, self.caps.cpu)),
            Drop::Ram(_) => Some(("ram", self.ram_max, self.caps.ram)),
            Drop::Hdd(_) => Some(("hdd", self.hdd_max, self.caps.hdd)),
            _ => None,
        }
    }

    /// How much of a `+n` upgrade the chassis has room for.
    fn room(now: u32, n: u32, cap: Option<u32>) -> u32 {
        cap.map_or(n, |c| n.min(c.saturating_sub(now)))
    }
}

/// One program on the HDD. Every program on the HDD can run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slot {
    /// Unique per run, so the UI can tell programs apart after the HDD reshuffles.
    pub uid: u32,
    /// The factory program this binary started as (index into `Data::warez`), for its blurb.
    pub warez: usize,
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
    /// The ICE's program, parsed in its own vocabulary.
    pub ice_prog: Program,
    /// Where the ICE's program is: the line it carries on from next turn, and its last turn's
    /// pass.
    pub ice_exec: Exec,
    /// The ICE's actions left in its turn (its `cpu` from `ice.ron` at the start of each).
    pub ice_cpu: u32,
    /// The ICE's IFs left in its turn (its `ram`).
    pub ice_ifs: u32,
    /// The programs (by uid) the deck ran this turn, from its program or the hotbar: what
    /// `TARGET RAN` reads.
    pub ran: Vec<u32>,
    /// CPU left this turn.
    pub cpu: u32,
    pub shield: u32,
    pub stunned: bool,
    /// ICE that just shook off a stun can't be stunned again this turn.
    pub stun_immune: bool,
    pub turn: u32,
    /// IFs the program may still evaluate this turn: one GB of the deck's RAM each.
    pub ifs: u32,
    /// Extra rewards for winning (encounter fights).
    pub bonus: Vec<Drop>,
    /// Speaks l33t (it came from a l33t encounter).
    pub leet: bool,
}

/// What the runner brings into a run: the deck's numbers (its base plus RIG, up to its caps)
/// and the programs on the HDD.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Kit {
    pub ram: u32,
    pub cpu: u32,
    pub hdd: u32,
    pub integrity: u32,
    pub caps: Caps,
    /// Programs to install: (factory origin, binary). Re-signed with the run's key on the way in.
    /// Empty means the starter programs.
    pub programs: Vec<(usize, Bin)>,
    /// The deck's program (BASIC text), loaded into every fight.
    pub program: String,
    /// Network shape: tier size and trait biases.
    pub net: NetParams,
    pub mods: RunMods,
    /// The phonebook entry being run, if dialed from home.
    pub site: Option<Site>,
    /// The network's BBS: an index into `Data::boards` (0 is the default board).
    pub board: usize,
    /// Jobs taken from a board at home for this network. They get their targets once it's built.
    pub jobs: Vec<crate::hub::PendingJob>,
    /// Clean jobs done for each client (from the save): trusted clients betray less.
    pub trust: Vec<(String, u32)>,
    /// Desktop tools you own (ids in `hub/tools.ron`): HexEd lets you patch on a board.
    pub tools: Vec<String>,
    /// Games you have a cracked copy of (ids in `hub/games.ron`): a board won't sell you one
    /// twice. A boxed copy doesn't count: the crack is still for sale.
    pub games: Vec<String>,
    /// Tunes you own (ids in `data/music/`): a board won't sell you one twice.
    pub tunes: Vec<String>,
    /// Textfiles already on your disk, as (board id, text id).
    pub texts: Vec<(String, String)>,
    /// For each of `programs`, the stash id it came from (`None` for a free program). The run
    /// remembers them so the loadout comes home in its place.
    pub origins: Vec<Option<u32>>,
    /// Programs that are free for this deck (warez ids): what goes on the HDD when `programs`
    /// is empty. A save always passes its own (`Save::free`); empty means the classic four
    /// (`CLASSIC_WAREZ`), for a bare run.
    pub free: Vec<String>,
}

impl Default for Kit {
    /// A bare deck: every line at its `tuning` constant, no ceilings, no programs.
    fn default() -> Self {
        use crate::hub::Upgrade;
        Self {
            ram: Upgrade::Ram.fallback(),
            cpu: Upgrade::Cpu.fallback(),
            hdd: Upgrade::Hdd.fallback(),
            integrity: Upgrade::Integrity.fallback(),
            caps: Caps::default(),
            programs: Vec::new(),
            program: String::new(),
            net: NetParams::default(),
            mods: RunMods::default(),
            site: None,
            board: 0,
            jobs: Vec::new(),
            trust: Vec::new(),
            tools: Vec::new(),
            games: Vec::new(),
            tunes: Vec::new(),
            texts: Vec::new(),
            origins: Vec::new(),
            free: Vec::new(),
        }
    }
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
    /// The target fell and the client turned on you (`Contract::betrayal` says how).
    Betrayed,
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
    /// What was actually paid, once settled.
    pub paid: u32,
    /// How the client turned on you: an index into `Data::betrayals`.
    pub betrayal: Option<usize>,
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
    /// The deck's program saying a step of its pass.
    Program,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogLine {
    pub kind: LogKind,
    pub text: String,
    /// When it was said: the hop, and the combat turn (0 outside a fight).
    pub hop: u32,
    pub turn: u32,
}

/// One file on today's BBS listing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StockFile {
    /// Index into the area's `files` in `BbsDef`.
    pub def: usize,
    pub bought: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
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
    /// Run the program in HDD slot `n`: the hotbar, the manual override.
    Run(usize),
    /// The deck's program: run the line under the cursor and stop on the next (gdb's `step`).
    /// The line that ends the pass ends the turn too, and the ICE acts.
    Step,
    /// The deck's program: run the rest of this turn's pass (gdb's `finish`), until the turn
    /// ends, a STOP or a hang. Leaves the program running or paused as it was.
    Finish,
    /// The deck's program: keep running, a line at a time, turn after turn (the driver
    /// applies `Step` while `ExecState::Running`); runs the first line now.
    Play,
    /// The deck's program: stop where it is, partway through a pass or not, and hand the turn
    /// to the hotbar. Step or Play carry on from there.
    Pause,
    /// Replace the deck's program with this text. Allowed any time, mid-fight too.
    Program(String),
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
    /// Read a bulletin or a post on the board you're connected to: some open doors.
    Read(Reading),
}

/// A bulletin or post on a board, by index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reading {
    Bulletin(usize),
    Post { base: usize, post: usize },
}

impl Reading {
    /// What reading it does on `board`, if anything.
    pub fn effect(self, board: &crate::data::BbsDef) -> Option<&crate::data::ReadEffect> {
        match self {
            Reading::Bulletin(i) => board.bulletins.get(i)?.on_read.as_ref(),
            Reading::Post { base, post } => {
                board.messages.get(base)?.posts.get(post)?.on_read.as_ref()
            }
        }
    }
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
    /// Boards learned from what you read this run (ids). They come home even on a flatline.
    pub boards_read: Vec<String>,
    /// You read a partner directory this run: the starter networks come home with you.
    pub starters_read: bool,
    /// What the run has been doing, for the graphs. Watches the rules, never changes them.
    pub history: crate::history::History,
    /// Sound cues waiting to be played. The app drains them each frame.
    pub cues: Vec<crate::cue::Cue>,
    /// Clean jobs done for each client before this run (from the save's `Kit::trust`).
    pub trust: Vec<(String, u32)>,
    /// Heat a client's sell-out adds to this network's phonebook entry, booked by `Save::record`.
    pub sold_out: u32,
    /// Desktop tools you own, the kit's plus any downloaded this run. They come home even on
    /// a flatline.
    pub tools: Vec<String>,
    /// Games you own, the kit's plus any downloaded this run (cracked copies). They come home
    /// even on a flatline.
    pub games: Vec<String>,
    /// Tunes you own, the kit's plus any downloaded this run. They come home even on a
    /// flatline.
    pub tunes: Vec<String>,
    /// Textfiles on your disk, as (board id, text id): the kit's plus any downloaded this run.
    /// They come home even on a flatline, for the QWK reader.
    pub texts: Vec<(String, String)>,
    /// Programs that came from the stash's loadout: (slot uid, stash id). At the exit they're
    /// ticked to keep, and a kept one goes home under its old id, installed again.
    pub from_stash: Vec<(u32, u32)>,
    /// The deck's program, edited in place. `Save::record` takes it home.
    pub program: Program,
    /// Where the program is in this fight.
    pub exec: Exec,
}

/// Whether the deck's program has the turn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ExecState {
    /// The debugger has the turn: the hotbar plays, Step or Play hands it back.
    #[default]
    Paused,
    /// Playing: the driver steps a line at a time, turn after turn, until a STOP, a hang or
    /// the fight's end.
    Running,
    /// A STOP on this line handed over the turn; the next pass carries on after it.
    Stopped(u32),
}

/// What the program is doing in this fight: the line about to run and this turn's pass.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Exec {
    pub state: ExecState,
    /// The line about to run: the program's first at the start of a turn, partway down while
    /// a pass is under way (after a STOP, a Step or a pause), or a hang's line, held.
    pub pc: Option<u32>,
    /// This turn's pass so far, or the last one: a step per line executed.
    pub trace: Vec<Step>,
    /// The lines this pass has come to since it last spent a budget (a RUN's cycle or an
    /// IF's GB): coming back to one is a hang.
    pub visited: Vec<u32>,
    /// A pass is under way this turn: the next line carries it on instead of starting at the
    /// top. The turn's end closes it.
    pub open: bool,
    /// A hang on this line (`?BREAK IN n`), the cursor held on it until the next Step, Play,
    /// edit or the turn's end.
    pub broke: Option<u32>,
}

/// Whose program a line is from: the deck's, or the ICE's it's fighting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Deck,
    Ice,
}

/// How a line of a program left the pass (`Game::line`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Line {
    /// It ran; the pass carries on at the next line.
    Went,
    /// A STOP: the debugger has the turn, and the next line carries on after it.
    Stopped,
    /// A hang: `?BREAK IN n`, the cursor held on the line.
    Hung,
    /// The pass is over (out of cycles, out of GB or the program's end) and so is the turn:
    /// the ICE has acted.
    Ended,
    /// The fight is over.
    Over,
}

/// One line executed in a pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    pub line: u32,
    pub what: StepWhat,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StepWhat {
    /// A RUN ran the program with this uid.
    Ran {
        uid: u32,
        outcome: crate::fight::Outcome,
    },
    /// A line that couldn't run, and why: the preview's words, or the interpreter's.
    Skipped(String),
    /// An IF, and what it found.
    If(bool),
    Goto(u32),
    /// A STOP: the turn is the hotbar's.
    Stop,
    /// The pass went round without spending anything (no RUN, no IF): `?BREAK IN <line>`.
    Break,
    /// The pass ended: no cycles left.
    OutOfCpu,
    /// The pass ended: no GB left for another IF.
    OutOfRam,
    /// The pass ran off the end of the program.
    End,
    /// The ICE's `ATTACK`: what got through, and what the shield took.
    Hit {
        dmg: u32,
        blocked: u32,
    },
    /// The ICE's `TRACE`: what it added.
    Traced(u32),
    /// The ICE's `FORTIFY`: the strength it added.
    Fortified(u32),
    /// An ICE's verb while it's lagged: it does nothing.
    Lagged,
}

impl StepWhat {
    /// Whether this step ended the turn (the ICE acted after it).
    pub fn ends_turn(&self) -> bool {
        matches!(
            self,
            StepWhat::OutOfCpu | StepWhat::OutOfRam | StepWhat::End
        )
    }
}

impl Game {
    /// A run with the starter deck, the classic four programs (breakers too), and every tool.
    pub fn new(data: Data, seed: u64) -> Self {
        let kit = Kit {
            tools: crate::data::TOOLS.iter().map(|t| t.to_string()).collect(),
            free: CLASSIC_WAREZ.iter().map(|t| t.to_string()).collect(),
            ..Kit::default()
        };
        Self::start(data, seed, &kit)
    }

    /// A run with a kit from home: the deck's numbers and the chosen loadout.
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
            cpu_max: kit.cpu,
            ram_max: kit.ram,
            hdd_max: kit.hdd,
            integrity: kit.integrity,
            integrity_max: kit.integrity,
            trace: 0,
            credits: 0,
            fragments: 0,
            caps: kit.caps,
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
            boards_read: Vec::new(),
            starters_read: false,
            history: crate::history::History::default(),
            cues: Vec::new(),
            site: kit.site.clone(),
            trust: kit.trust.clone(),
            sold_out: 0,
            tools: kit.tools.clone(),
            games: kit.games.clone(),
            tunes: kit.tunes.clone(),
            texts: kit.texts.clone(),
            from_stash: Vec::new(),
            program: crate::basic::parse(&kit.program),
            exec: Exec::default(),
        };
        let classic: Vec<String> = CLASSIC_WAREZ.iter().map(|s| s.to_string()).collect();
        let free = if kit.free.is_empty() {
            &classic
        } else {
            &kit.free
        };
        let programs: Vec<(usize, Bin)> = if kit.programs.is_empty() {
            free.iter()
                .filter_map(|id| game.data.warez_ix(id))
                .filter_map(|w| game.factory_slot(w).map(|s| (w, s.bin)))
                .collect()
        } else {
            kit.programs.clone()
        };
        for (n, (w, mut bin)) in programs.into_iter().enumerate() {
            if game.hdd.len() as u32 >= game.deck.hdd_max || w >= game.data.warez.len() {
                break;
            }
            let origin = kit.origins.get(n).copied().flatten();
            // Home binaries carry home's signature; the run signs its own.
            binary::seal(&mut bin, game.key());
            if let Ok(spec) = binary::check(&bin, game.key()) {
                game.add_slot(Slot {
                    uid: 0,
                    warez: w,
                    bin,
                    spec,
                });
                if let (Some(id), Some(slot)) = (origin, game.hdd.last()) {
                    game.from_stash.push((slot.uid, id));
                }
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
        game.history = crate::history::History::start(&game);
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
                paid: 0,
                betrayal: None,
            };
            self.say(
                LogKind::Good,
                format!("contract from home: {}", self.contract_line(&k)),
            );
            self.contracts.push(k);
        }
    }

    /// The key binaries are signed with this run.
    pub fn key(&self) -> u64 {
        self.seed
    }

    /// The programs to tick at the exit: the loadout's, still on the HDD (by uid).
    pub fn loadout_keep(&self) -> Vec<u32> {
        self.from_stash
            .iter()
            .filter(|(uid, _)| self.hdd.iter().any(|s| s.uid == *uid))
            .map(|(uid, _)| *uid)
            .collect()
    }

    /// A fresh, sealed copy of factory program `w`.
    pub fn factory_slot(&self, w: usize) -> Option<Slot> {
        let bin = binary::encode(&self.data.warez[w], self.key());
        let spec = binary::check(&bin, self.key()).ok()?;
        Some(Slot {
            uid: 0,
            warez: w,
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

    /// What the current ICE will do next, if fighting: a dry run of its program from where it
    /// is to its first action, its IFs read against the fight as it stands now. It changes
    /// nothing and draws nothing from the RNG, so it's recomputed whenever it's asked, and it
    /// changes as the turn does (a shield up turns an ICE that reads `TARGET SHIELD`). `None`
    /// when it won't act: lagged, or its program would stall, stop or run out of RAM or lines
    /// first. Its turn (`end_turn`) runs the same program the same way, so this is what it does
    /// if nothing changes.
    pub fn ice_next(&self) -> Option<Intent> {
        let c = self.combat.as_ref()?;
        if c.stunned {
            return None;
        }
        let p = &c.ice_prog;
        let mut pc = c.ice_exec.pc.or(p.first());
        let mut ifs = self.data.ice[c.ice].ram;
        let mut visited = Vec::new();
        loop {
            let no = pc?;
            if visited.contains(&no) {
                return None;
            }
            visited.push(no);
            let Some(l) = p.line(no) else {
                pc = p.after(no);
                continue;
            };
            let next = p.after(no);
            pc = next;
            match &l.stmt {
                Stmt::If(cond) => {
                    if ifs == 0 {
                        return None;
                    }
                    ifs -= 1;
                    visited.clear();
                    if !self.test(cond) {
                        pc = p.skip(next);
                    }
                }
                Stmt::Goto(to) if p.line(*to).is_some() => pc = Some(*to),
                Stmt::Stop => return None,
                stmt => {
                    if let Some(a) = stmt.action() {
                        return Some(a);
                    }
                }
            }
        }
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

    /// How badly the deck is hurting, 0 (clean) to 1 (dying), for the damage glitches.
    pub fn glitch(&self) -> f32 {
        crate::glitch::level(self.deck.integrity, self.deck.integrity_max)
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
            hop: self.hops,
            turn: self.combat.as_ref().map_or(0, |c| c.turn),
        });
        if self.log.len() > LOG_LIMIT {
            let extra = self.log.len() - LOG_LIMIT;
            self.log.drain(..extra);
        }
    }

    /// Applies one player action. Illegal actions change nothing but a log line.
    pub fn apply(&mut self, action: Action) {
        // A pass is made of actions (each RUN, then the end of the turn), applied one by one
        // so history and cues hear each of them; the pass itself adds nothing on top.
        if matches!(action, Action::Step | Action::Play | Action::Finish) {
            if let Err(e) = self.try_apply(action) {
                self.say(LogKind::Sys, format!("err: {e}"));
            }
            return;
        }
        let was = crate::history::Snap::of(self);
        let heard = crate::cue::Snap::of(self);
        let uid = match action {
            Action::Run(slot) => self.hdd.get(slot).map(|s| s.uid),
            _ => None,
        };
        let ok = match self.try_apply(action.clone()) {
            Ok(()) => true,
            Err(e) => {
                self.say(LogKind::Sys, format!("err: {e}"));
                false
            }
        };
        // Refused actions make no sound; a new run starts quiet.
        if ok && !matches!(action, Action::NewRun(_)) {
            let bought = matches!(action, Action::Buy { .. });
            let cues = crate::cue::cues(&heard, self, bought);
            self.cues.extend(cues);
            // Nobody listening (tests, the balance probe): keep only the latest few.
            let extra = self.cues.len().saturating_sub(crate::cue::QUEUE_MAX);
            self.cues.drain(..extra);
        }
        // A new run starts its own history; everything else is recorded.
        if !matches!(action, Action::NewRun(_)) {
            let mut h = std::mem::take(&mut self.history);
            h.record(&was, self, action, uid);
            self.history = h;
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
            Action::Read(r) => self.read(r),
            Action::Run(slot) => self.run(slot),
            Action::Step => self.go(false).map(|()| {
                self.line(Side::Deck);
            }),
            Action::Play => self.go(true).map(|()| {
                self.line(Side::Deck);
            }),
            Action::Finish => self.pass(false),
            Action::Pause => {
                if self.exec.state == ExecState::Running {
                    self.exec.state = ExecState::Paused;
                }
                Ok(())
            }
            Action::Program(text) => {
                self.edit_program(&text);
                Ok(())
            }
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
        let ice_prog = crate::basic::parse_with(&def.program, Vocab::Ice(&def.verbs));
        self.combat = Some(Combat {
            node,
            ice,
            strength,
            strength_max: strength,
            ice_prog,
            ice_exec: Exec::default(),
            ice_cpu: def.cpu,
            ice_ifs: def.ram,
            ran: Vec::new(),
            cpu: self.deck.cpu_max,
            shield: 0,
            stunned: false,
            stun_immune: false,
            turn: 1,
            ifs: self.deck.ram_max,
            bonus: Vec::new(),
            leet: false,
        });
        self.phase = Phase::Combat;
        // Every fight opens with the program loaded and the cursor on its first line.
        self.exec = Exec {
            pc: self.top(Side::Deck),
            ..Exec::default()
        };
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
        // Everything that decides what happens is in `preview`, which the fight view shows.
        let p = self.preview(slot)?;
        let name = self.hdd[slot].spec.name.clone();
        let uid = self.hdd[slot].uid;
        let Some(c) = self.combat.as_mut() else {
            return Err("nothing to run against.".into());
        };
        c.cpu -= p.cpu;
        c.ran.push(uid);
        match p.outcome {
            crate::fight::Outcome::Break { hit, vs } => {
                let crit = match vs {
                    crate::fight::Match::Strong => " (class match x2)",
                    crate::fight::Match::Weak => " (wrong class /2)",
                    crate::fight::Match::Plain => "",
                };
                c.strength = c.strength.saturating_sub(hit);
                self.say(LogKind::Good, format!("{name} breaks {hit}{crit}"));
                // Derezzed, or beaten badly enough to quit.
                if self.combat.as_ref().is_some_and(|c| c.strength == 0) || self.gives_up() {
                    self.win_combat();
                }
            }
            crate::fight::Outcome::Shield(n) => {
                c.shield += n;
                self.say(LogKind::Good, format!("{name} up: shield {n}"));
            }
            crate::fight::Outcome::Repair(n) => {
                self.deck.integrity = (self.deck.integrity + n).min(self.deck.integrity_max);
                self.say(LogKind::Good, format!("{name} restores {n} integrity"));
            }
            crate::fight::Outcome::Scrub(n) => {
                self.deck.trace = self.deck.trace.saturating_sub(n);
                self.say(LogKind::Good, format!("{name} scrubs {n} trace"));
            }
            crate::fight::Outcome::Stun => {
                c.stunned = true;
                self.say(
                    LogKind::Good,
                    format!("{name} lags the ICE. it skips a beat."),
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
        self.end_pass();
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

    /// The end of the deck's turn: the ICE takes its turn, running its program (`Game::line`
    /// from its side) until its cycles, its RAM or its program run out, or it stalls; then the
    /// next turn starts.
    fn end_turn(&mut self) -> Result<(), String> {
        let mods = self.mods;
        let Some(c) = self.combat.as_mut() else {
            return Err("no fight in progress.".into());
        };
        let def = &self.data.ice[c.ice];
        c.stun_immune = false;
        c.ice_cpu = def.cpu;
        c.ice_ifs = def.ram;
        // A lagged ICE runs its program and does nothing it says: it loses this turn's moves.
        while self.line(Side::Ice) == Line::Went && self.deck.integrity > 0 {}
        let Some(c) = self.combat.as_mut() else {
            return Ok(());
        };
        if c.stunned {
            c.stunned = false;
            c.stun_immune = true;
        }
        c.ice_exec.open = false;
        c.ice_exec.visited.clear();
        c.shield = 0;
        c.cpu = self.deck.cpu_max;
        c.ifs = self.deck.ram_max;
        c.turn += 1;
        c.ran.clear();
        self.end_pass();
        self.exec.pc = self.top(Side::Deck);
        if let ExecState::Stopped(_) = self.exec.state {
            self.exec.state = ExecState::Paused;
        }
        self.deck.trace += mods.trace(TRACE_PER_COMBAT_TURN);
        if self.deck.integrity == 0 {
            self.phase = Phase::Flatlined;
            self.combat = None;
            self.end_pass();
            self.say(LogKind::Bad, "!! deck integrity zero. FLATLINE.");
            return Ok(());
        }
        self.check_trace();
        Ok(())
    }

    /// What an ICE's verb does, as `end_turn` used to do its intent: a hit less the shield
    /// (which the hit uses up), a trace, or strength. A lagged ICE does nothing.
    fn act(&mut self, act: Intent) -> StepWhat {
        let mods = self.mods;
        let Some(c) = self.combat.as_mut() else {
            return StepWhat::Lagged;
        };
        if c.stunned {
            return StepWhat::Lagged;
        }
        match act {
            Intent::Attack(n) => {
                let blocked = n.min(c.shield);
                c.shield -= blocked;
                let dmg = n - blocked;
                self.deck.integrity = self.deck.integrity.saturating_sub(dmg);
                StepWhat::Hit { dmg, blocked }
            }
            // Saturating: a program's numbers are data, and data can be silly.
            Intent::Trace(n) => {
                let n = mods.trace(n);
                self.deck.trace = self.deck.trace.saturating_add(n);
                StepWhat::Traced(n)
            }
            Intent::Fortify(n) => {
                c.strength = c.strength.saturating_add(n);
                c.strength_max = c.strength_max.max(c.strength);
                StepWhat::Fortified(n)
            }
        }
    }

    // ---- the deck's program -----------------------------------------------------

    /// The HDD slot a program name in a `RUN` or `HAVE` means: the program's name, its file
    /// name or its factory id, in any case. The first match wins.
    pub fn slot_named(&self, name: &str) -> Option<usize> {
        self.hdd
            .iter()
            .position(|s| crate::basic::names(name, &s.spec.name, &self.data.warez[s.warez].id))
    }

    /// What an `IF` finds right now, the deck's or the ICE's. Lagged ICE has no next move.
    pub fn test(&self, cond: &Cond) -> bool {
        let c = self.combat.as_ref();
        match cond {
            Cond::Cmp(var, op, n) => {
                let v = match var {
                    Var::Integ => self.deck.integrity,
                    Var::Trace => self.deck.trace,
                    Var::Cpu => c.map_or(0, |c| c.cpu),
                    Var::Turn => c.map_or(0, |c| c.turn),
                    Var::Ice => c.map_or(0, |c| c.strength),
                };
                op.test(v, *n)
            }
            Cond::IceClass(k) => c.is_some_and(|c| self.data.ice[c.ice].class == *k),
            Cond::Next(m) => self.ice_next().is_some_and(|i| m.is(i)),
            Cond::Have(name) => self.slot_named(name).is_some(),
            Cond::Target(t, op, n) => {
                let v = match t {
                    Target::Integ => self.deck.integrity,
                    Target::Shield => c.map_or(0, |c| c.shield),
                    Target::Cpu => c.map_or(0, |c| c.cpu),
                };
                op.test(v, *n)
            }
            Cond::TargetRan(name) => c.is_some_and(|c| {
                self.hdd.iter().any(|s| {
                    c.ran.contains(&s.uid)
                        && crate::basic::names(name, &s.spec.name, &self.data.warez[s.warez].id)
                })
            }),
            Cond::SelfStr(op, n) => op.test(c.map_or(0, |c| c.strength), *n),
        }
    }

    /// Replaces the deck's program. Stopped partway through a turn, it carries on from the
    /// first line after the STOP; paused partway, from the line it was on (or the next one);
    /// otherwise the next pass starts at the top.
    fn edit_program(&mut self, text: &str) {
        self.program = crate::basic::parse(text);
        self.exec.broke = None;
        self.exec.visited.clear();
        self.exec.pc = match (self.exec.state, self.exec.pc) {
            (ExecState::Stopped(at), _) => self.program.rest(self.program.after(at)),
            (_, Some(at)) if self.exec.open => match self.program.line(at) {
                Some(_) => self.program.rest(Some(at)),
                None => self.program.rest(self.program.after(at)),
            },
            _ => self.top(Side::Deck),
        };
    }

    /// Where a side's cursor rests at the top of a pass: the first line that does something.
    fn top(&self, side: Side) -> Option<u32> {
        let p = self.prog(side);
        p.rest(p.first())
    }

    /// The program a side runs: the deck's, or the ICE's in this fight.
    fn prog(&self, side: Side) -> &Program {
        match (side, self.combat.as_ref()) {
            (Side::Ice, Some(c)) => &c.ice_prog,
            _ => &self.program,
        }
    }

    /// Where a side's program is in this fight.
    fn ex(&self, side: Side) -> &Exec {
        match (side, self.combat.as_ref()) {
            (Side::Ice, Some(c)) => &c.ice_exec,
            _ => &self.exec,
        }
    }

    /// Where a side's program is in this fight, to move it. The ICE's only runs in a fight.
    fn ex_mut(&mut self, side: Side) -> &mut Exec {
        match (side, self.combat.as_mut()) {
            (Side::Ice, Some(c)) => &mut c.ice_exec,
            _ => &mut self.exec,
        }
    }

    /// A side's budgets left this turn: (cycles, IFs).
    fn budgets(&self, side: Side) -> (u32, u32) {
        match (side, self.combat.as_ref()) {
            (Side::Deck, Some(c)) => (c.cpu, c.ifs),
            (Side::Ice, Some(c)) => (c.ice_cpu, c.ice_ifs),
            (_, None) => (0, 0),
        }
    }

    /// Adds a step to the pass and says it in the log as it happens: the OS's sentence, before
    /// anything the turn does after it. A RUN says nothing more: the program says what it did.
    /// The ICE's steps are said too, in the same voice, except the end of its turn, which the
    /// next turn says well enough; what it does to the deck is said as bad news.
    fn trace_step(&mut self, side: Side, line: u32, what: StepWhat) {
        let step = Step { line, what };
        let said = match side {
            Side::Deck => !matches!(step.what, StepWhat::Ran { .. }),
            Side::Ice => !step.what.ends_turn(),
        };
        if said {
            let text = self.step_text_of(side, &step);
            let kind = match step.what {
                StepWhat::Hit { .. } | StepWhat::Traced(_) | StepWhat::Fortified(_) => LogKind::Bad,
                _ => LogKind::Program,
            };
            self.say(kind, text);
        }
        self.ex_mut(side).trace.push(step);
    }

    /// The rest of this turn's pass of the deck's program, a line at a time (`Game::line`)
    /// until the turn ends, a STOP, a hang or the fight's end. `play` sets the program running
    /// (`Action::Play`); otherwise it stays running or paused as it was.
    fn pass(&mut self, play: bool) -> Result<(), String> {
        self.go(play)?;
        while self.line(Side::Deck) == Line::Went {}
        Ok(())
    }

    /// Hands the turn to the deck's program: a STOP or a hang is behind it now.
    fn go(&mut self, play: bool) -> Result<(), String> {
        if self.phase != Phase::Combat {
            return Err("no fight in progress.".into());
        }
        self.exec.broke = None;
        self.exec.state = if play || self.exec.state == ExecState::Running {
            ExecState::Running
        } else {
            ExecState::Paused
        };
        Ok(())
    }

    /// One line of a side's program: the line about to run, and any REMs after it, so it
    /// stops on the next line that does something. The only place a statement executes, the
    /// deck's or the ICE's.
    ///
    /// The deck's pass runs from the first line (or where a STOP or a pause left it) until the
    /// program ends, a STOP, the cycles run out or there's no GB for another IF. Every RUN
    /// resolves through `preview`, and runs as `Action::Run` would; one that can't run is
    /// skipped with the preview's words. Coming back to a line without spending anything (no
    /// RUN's cycle, no IF's GB) since it was last there is a hang: `?BREAK IN <line>`, the
    /// cursor held there and the debugger has the turn. When the pass is over, on the line that
    /// spends the last cycle, reaches the end or finds no GB, the turn ends and the ICE acts.
    ///
    /// The ICE's pass (`end_turn`) carries on from where its last one left off, with its own
    /// budgets: each verb spends one of its cycles, each IF one of its IFs. Its pass ends when
    /// either runs out, at its program's end (the next starts at the top), on a STOP, or when
    /// it hangs, which is a stall: it does nothing more this turn.
    fn line(&mut self, side: Side) -> Line {
        if self.phase != Phase::Combat || self.combat.is_none() {
            return Line::Over;
        }
        if !self.ex(side).open {
            let first = self.prog(side).first();
            let e = self.ex_mut(side);
            e.open = true;
            e.trace.clear();
            e.visited.clear();
            // The ICE carries on where it left off; the deck starts at the top.
            if side == Side::Deck || e.pc.is_none() {
                e.pc = first;
            }
        }
        let mut last = self.ex(side).pc.unwrap_or(0);
        // A line ran: the pass carries on, unless what it did ended it.
        let mut ran = false;
        loop {
            if self.combat.is_none() || self.phase != Phase::Combat {
                // The fight ended mid-pass: won, or the deck went down.
                return Line::Over;
            }
            let (cpu, ifs) = self.budgets(side);
            if cpu == 0 {
                self.trace_step(side, last, StepWhat::OutOfCpu);
                return self.close(side);
            }
            let Some(no) = self.ex(side).pc else {
                self.trace_step(side, last, StepWhat::End);
                return self.close(side);
            };
            // Stop on the next line that does something: a REM is passed over.
            let rem = self
                .prog(side)
                .line(no)
                .is_none_or(|l| matches!(l.stmt, Stmt::Rem));
            if ran && !rem {
                return Line::Went;
            }
            last = no;
            if self.ex(side).visited.contains(&no) {
                self.trace_step(side, no, StepWhat::Break);
                let e = self.ex_mut(side);
                e.open = false;
                e.visited.clear();
                if side == Side::Deck {
                    e.state = ExecState::Paused;
                    e.broke = Some(no);
                }
                return Line::Hung;
            }
            self.ex_mut(side).visited.push(no);
            let Some(stmt) = self.prog(side).line(no).map(|l| l.stmt.clone()) else {
                let after = self.prog(side).after(no);
                self.ex_mut(side).pc = after;
                continue;
            };
            let next = self.prog(side).after(no);
            self.ex_mut(side).pc = next;
            ran |= !matches!(stmt, Stmt::Rem);
            match stmt {
                Stmt::Rem => {}
                Stmt::Bad(why) => self.trace_step(side, no, StepWhat::Skipped(why)),
                Stmt::Run(name) if side == Side::Deck => {
                    let Some(slot) = self.slot_named(&name) else {
                        let why = format!("no program called {name} on the disk.");
                        self.trace_step(side, no, StepWhat::Skipped(why));
                        continue;
                    };
                    match self.preview(slot) {
                        Ok(p) => {
                            self.exec.visited.clear();
                            let uid = self.hdd[slot].uid;
                            self.apply(Action::Run(slot));
                            // The RUN broke this ICE and the next one rezzed, its fight starting
                            // with a fresh `Exec`: the pass carries on into it, from its first
                            // line. A fight that's over closed the pass (`end_pass`).
                            if self.phase == Phase::Combat && !self.exec.open {
                                self.exec.open = true;
                            }
                            self.trace_step(
                                side,
                                no,
                                StepWhat::Ran {
                                    uid,
                                    outcome: p.outcome,
                                },
                            );
                        }
                        Err(why) => self.trace_step(side, no, StepWhat::Skipped(why)),
                    }
                }
                Stmt::Attack(_) | Stmt::Trace(_) | Stmt::Fortify(_) if side == Side::Ice => {
                    if let Some(c) = self.combat.as_mut() {
                        c.ice_cpu -= 1;
                    }
                    self.ex_mut(side).visited.clear();
                    let what = stmt.action().map_or(StepWhat::Lagged, |a| self.act(a));
                    self.trace_step(side, no, what);
                }
                // The other side's words: its parser never lets them in.
                Stmt::Run(_) | Stmt::Attack(_) | Stmt::Trace(_) | Stmt::Fortify(_) => {
                    let why = "?SYNTAX ERROR: unknown keyword".to_string();
                    self.trace_step(side, no, StepWhat::Skipped(why));
                }
                Stmt::If(cond) => {
                    if ifs == 0 {
                        // It stays on the IF it couldn't afford: the ICE comes back to it.
                        self.ex_mut(side).pc = Some(no);
                        self.trace_step(side, no, StepWhat::OutOfRam);
                        return self.close(side);
                    }
                    if let Some(c) = self.combat.as_mut() {
                        match side {
                            Side::Deck => c.ifs -= 1,
                            Side::Ice => c.ice_ifs -= 1,
                        }
                    }
                    // Spending a GB is progress, as a RUN's cycle is: a loop of IFs idles
                    // through the RAM and ends the turn, it doesn't hang.
                    self.ex_mut(side).visited.clear();
                    let yes = self.test(&cond);
                    self.trace_step(side, no, StepWhat::If(yes));
                    if !yes {
                        let to = self.prog(side).skip(next);
                        self.ex_mut(side).pc = to;
                    }
                }
                Stmt::Goto(to) => {
                    if self.prog(side).line(to).is_some() {
                        self.trace_step(side, no, StepWhat::Goto(to));
                        self.ex_mut(side).pc = Some(to);
                    } else {
                        let why = format!("?UNDEF'D STATEMENT {to}");
                        self.trace_step(side, no, StepWhat::Skipped(why));
                    }
                }
                Stmt::Stop if side == Side::Ice => {
                    self.trace_step(side, no, StepWhat::Stop);
                    return self.close(side);
                }
                Stmt::Stop => {
                    self.trace_step(side, no, StepWhat::Stop);
                    self.exec.state = ExecState::Stopped(no);
                    // Carrying on after a STOP looks for a hang afresh; the cursor rests past
                    // any REMs after it, which that carrying on passes over.
                    self.exec.visited.clear();
                    while let Some(n) = self
                        .exec
                        .pc
                        .filter(|&n| self.program.rest(Some(n)) != Some(n))
                    {
                        self.exec.visited.push(n);
                        self.exec.pc = self.program.after(n);
                    }
                    return Line::Stopped;
                }
            }
        }
    }

    /// Whether this turn's pass carries on at the next line: under way, in a fight, and not
    /// handed over by a STOP or a hang.
    pub fn pass_open(&self) -> bool {
        self.phase == Phase::Combat
            && self.exec.open
            && !matches!(self.exec.state, ExecState::Stopped(_))
    }

    /// No pass is under way: the turn or the fight is over. The next line starts at the top.
    fn end_pass(&mut self) {
        self.exec.open = false;
        self.exec.visited.clear();
        self.exec.broke = None;
    }

    /// The pass is over. The deck's ends the turn, and the ICE acts; the ICE's ends its turn.
    fn close(&mut self, side: Side) -> Line {
        match side {
            Side::Deck => self.apply(Action::EndTurn),
            Side::Ice => {
                let e = self.ex_mut(side);
                e.open = false;
                e.visited.clear();
            }
        }
        Line::Ended
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

    /// Fills `{offer}`, `{per_cpu}` and `{cost}` in an event's text.
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
                ChoiceEffect::Sell { per_cpu } => {
                    out = out.replace("{per_cpu}", &per_cpu.to_string())
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

    /// What a program fetches from a fence paying `per_cpu` for each CPU it takes.
    pub fn sale_price(&self, slot: usize, per_cpu: u32) -> u32 {
        self.hdd
            .get(slot)
            .map_or(0, |s| per_cpu * s.spec.cpu + SELL_PER_OPT * s.spec.opt)
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
            ChoiceEffect::Sell { per_cpu } => {
                let Some(i) = slot else { return Ok(()) };
                let price = self.sale_price(i, per_cpu);
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
            paid: 0,
            betrayal: None,
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

    /// The chance (percent) that `k`'s client turns on you if its target falls now: higher on
    /// bigger networks and bigger jobs, lower for a client you've worked for cleanly, halved if
    /// the ghost bonus would pay. Zero if nothing in `contracts.ron` fits this tier.
    pub fn betray_pct(&self, k: &Contract) -> u32 {
        let tier = self.tier();
        if !self.data.betrayals.iter().any(|b| b.min_tier <= tier) {
            return 0;
        }
        let owed = self.mods.pay(k.reward + self.ghost_due(k));
        let trust = self
            .trust
            .iter()
            .find(|(c, _)| *c == k.client)
            .map_or(0, |&(_, n)| n.min(BETRAY_TRUST_MAX));
        let base = BETRAY_PCT[(tier as usize).clamp(1, 3) - 1] + owed / BETRAY_PER_CREDITS;
        let pct = base.saturating_sub(BETRAY_TRUST_PCT * trust);
        let pct = if self.ghost_due(k) > 0 { pct / 2 } else { pct };
        pct.min(BETRAY_MAX_PCT)
    }

    /// How `k`'s client turned on you, as the panes show it: e.g. `DEFAULTED`.
    pub fn betrayal_label(&self, k: &Contract) -> Option<&str> {
        k.betrayal
            .and_then(|b| self.data.betrayals.get(b))
            .map(|b| b.label.as_str())
    }

    /// The network's tier (a plain run counts as tier 1).
    pub fn tier(&self) -> u32 {
        self.site.as_ref().map_or(1, |s| s.tier)
    }

    /// The ghost bonus `k` pays at the current trace.
    fn ghost_due(&self, k: &Contract) -> u32 {
        k.ghost
            .filter(|&(cap, _)| self.deck.trace <= cap)
            .map_or(0, |(_, b)| b)
    }

    /// Pays out contracts on a node that just fell, then rolls whether each client turns on you.
    fn settle_contracts(&mut self, node: usize) {
        let due: Vec<usize> = (0..self.contracts.len())
            .filter(|&i| self.contracts[i].node == node)
            .filter(|&i| self.contracts[i].status == ContractStatus::Active)
            .collect();
        for i in due {
            let k = self.contracts[i].clone();
            let (reward, bonus) = (self.mods.pay(k.reward), self.mods.pay(self.ghost_due(&k)));
            let betrayal = self
                .rng
                .chance(self.betray_pct(&k))
                .then(|| self.pick_betrayal())
                .flatten();
            let paid = match betrayal.map(|b| &self.data.betrayals[b].effect) {
                Some(BetrayalEffect::Stiffed(pct)) => (reward + bonus) * pct / 100,
                _ => reward + bonus,
            };
            self.deck.credits += paid;
            let c = &mut self.contracts[i];
            c.paid = paid;
            c.betrayal = betrayal;
            c.status = if betrayal.is_some() {
                ContractStatus::Betrayed
            } else {
                ContractStatus::Done
            };
            match betrayal {
                None => {
                    self.say(
                        LogKind::Loot,
                        format!("contract complete. {} pays {reward}¢.", k.client),
                    );
                    if bonus > 0 {
                        self.say(
                            LogKind::Loot,
                            format!("+ ghost bonus {bonus}¢. clean work."),
                        );
                    }
                }
                Some(b) => self.betray(b, &k.client, node, paid),
            }
        }
    }

    /// One of the betrayals that fit this network's tier, by weight.
    fn pick_betrayal(&mut self) -> Option<usize> {
        let tier = self.tier();
        let fits: Vec<usize> = (0..self.data.betrayals.len())
            .filter(|&b| self.data.betrayals[b].min_tier <= tier)
            .collect();
        let total: u32 = fits.iter().map(|&b| self.data.betrayals[b].weight).sum();
        if total == 0 {
            return None;
        }
        let mut roll = self.rng.below(total as usize) as u32;
        fits.into_iter().find(|&b| {
            let w = self.data.betrayals[b].weight;
            if roll < w {
                true
            } else {
                roll -= w;
                false
            }
        })
    }

    /// The double-cross itself: says what happened and does it.
    fn betray(&mut self, b: usize, client: &str, node: usize, paid: u32) {
        let def = self.data.betrayals[b].clone();
        if paid > 0 {
            self.say(
                LogKind::Loot,
                format!("contract complete. {client} pays {paid}¢."),
            );
        }
        self.say(
            LogKind::Bad,
            format!("!! {}", def.says.replace("{client}", client)),
        );
        match def.effect {
            BetrayalEffect::Stiffed(_) => {}
            BetrayalEffect::Burned { trace, ice } => {
                self.deck.trace += trace;
                self.mods.ice_bonus += ice;
                self.check_trace();
            }
            BetrayalEffect::SoldOut(heat) => self.sold_out += heat,
            BetrayalEffect::Setup(id) => {
                // The runner is standing on the node that just fell (in the main network).
                if let Some(ice) = self.data.ice_by_id(&id)
                    && self.outer.is_none()
                    && self.at == node
                    && self.phase != Phase::Flatlined
                {
                    self.start_combat(node, ice);
                }
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
            self.end_pass();
            self.say(LogKind::Bad, "!! trace complete. they found you. FLATLINE.");
            return true;
        }
        false
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
        if let Some((line, now, Some(cap))) = self.deck.line(&def.item)
            && let Drop::Cpu(n) | Drop::Ram(n) | Drop::Hdd(n) = def.item
            && now + n > cap
        {
            return Some(format!(
                "your deck can't take it: {line} tops out at {cap}."
            ));
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
        if let Some(why) = tool_blocker(
            &self.data.tools,
            &self.data.crack_labels(),
            &self.data.tune_titles(),
            &def.item,
            self,
        ) {
            return Some(why);
        }
        None
    }

    /// Whether you own a desktop tool (an id in `hub/tools.ron`).
    pub fn has_tool(&self, id: &str) -> bool {
        self.tools.iter().any(|t| t == id)
    }

    /// Whether you have a cracked copy of a game (an id in `hub/games.ron`).
    pub fn has_crack(&self, id: &str) -> bool {
        self.games.iter().any(|g| g == id)
    }

    /// Whether you own a tune (an id in `data/music/`).
    pub fn has_tune(&self, id: &str) -> bool {
        self.tunes.iter().any(|t| t == id)
    }

    /// Whether a textfile on this run's board is already on your disk.
    pub fn has_text(&self, id: &str) -> bool {
        let board = &self.bbs().id;
        self.texts.iter().any(|(b, t)| b == board && t == id)
    }

    fn buy(&mut self, area: usize, file: usize) -> Result<(), String> {
        if let Some(why) = self.buy_blocker(area, file) {
            return Err(why);
        }
        let s = self.bbs_stock[area][file];
        let def = self.bbs().areas[area].files[s.def].clone();
        self.deck.credits -= def.price;
        self.bbs_stock[area][file].bought = true;
        let cost = if def.price == 0 {
            "free".to_string()
        } else {
            format!("-{}¢", def.price)
        };
        self.say(LogKind::Info, format!("downloaded {} ({cost})", def.file));
        self.grant(def.item);
        Ok(())
    }

    fn read(&mut self, r: Reading) -> Result<(), String> {
        if !self.at_bbs() {
            return Err("carrier lost. you're not on the board.".into());
        }
        let Some(effect) = r.effect(self.bbs()).cloned() else {
            return Err("nothing to note.".into());
        };
        match effect {
            crate::data::ReadEffect::Board(id) => {
                let Some(b) = self.data.board(&id) else {
                    return Err("a dead number.".into());
                };
                if self.boards_read.contains(&id) {
                    return Ok(());
                }
                let d = &self.data.boards[b];
                let line = format!("+ number noted: {} ({})", d.name, d.phone);
                self.boards_read.push(id);
                self.say(LogKind::Loot, line);
            }
            crate::data::ReadEffect::Starters => {
                if self.starters_read {
                    return Ok(());
                }
                self.starters_read = true;
                self.say(LogKind::Loot, "+ partner numbers noted for your phonebook");
            }
            crate::data::ReadEffect::Hint(range) => {
                if self.hints.contains(&range) {
                    return Ok(());
                }
                self.grant(Drop::Hint(range));
            }
        }
        Ok(())
    }

    /// Why the hex editor can't write right now, if it can't. In a run you need a BBS's compiler.
    pub fn compile_blocker(&self) -> Option<&'static str> {
        if !self.has_tool("hexed") {
            Some("hex: unknown command.")
        } else if self.phase == Phase::Combat {
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
        let old = s.spec.file();
        let new = spec.file();
        let target = &mut self.hdd[slot];
        target.bin = bin;
        target.spec = spec;
        let renamed = if old == new {
            String::new()
        } else {
            format!(" (was {old})")
        };
        self.say(LogKind::Good, format!("wrote {new}{renamed}. link ok."));
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
            Drop::Ram(n) | Drop::Cpu(n) | Drop::Hdd(n) => {
                let (line, now, cap) = self.deck.line(&drop).expect("a deck line");
                let n = Deck::room(now, n, cap);
                if n == 0 {
                    self.say(
                        LogKind::Sys,
                        format!("found a {line} upgrade, but the chassis is full. wasted."),
                    );
                    return;
                }
                match drop {
                    Drop::Ram(_) => self.deck.ram_max += n,
                    Drop::Cpu(_) => self.deck.cpu_max += n,
                    _ => self.deck.hdd_max += n,
                }
                let unit = if line == "hdd" { " slots" } else { "" };
                self.say(LogKind::Loot, format!("+ {line} upgrade: +{n}{unit}"));
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
            Drop::Tool(id) => {
                let name = self.data.tool(&id).map_or(id.clone(), |t| t.name.clone());
                if !self.has_tool(&id) {
                    self.tools.push(id);
                }
                self.say(LogKind::Loot, format!("+ tool: {name}"));
            }
            Drop::Game(id) => {
                let name = self.data.game_label(&id, crate::hub::GameCopy::Cracked);
                if !self.has_crack(&id) {
                    self.games.push(id);
                }
                self.say(LogKind::Loot, format!("+ game: {name}"));
            }
            Drop::Tune(id) => {
                let title = self.data.tune(&id).map_or(id.clone(), |t| t.title.clone());
                if !self.has_tune(&id) {
                    self.tunes.push(id);
                }
                self.say(LogKind::Loot, format!("+ music disk: {title}"));
            }
            Drop::Text(id) => {
                if !self.has_text(&id) {
                    let board = self.bbs().id.clone();
                    self.texts.push((board, id));
                }
                self.say(LogKind::Loot, "+ textfile, for your mail reader");
            }
            Drop::Nothing => self.say(LogKind::Sys, "nothing worth taking."),
        }
    }
}

/// Why a tool, game or textfile can't be downloaded on `line`, wherever you are: it's
/// already yours. `tools` and `cracks` (each game's id and crack label) name it.
pub fn tool_blocker(
    tools: &[crate::data::ToolDef],
    cracks: &[(String, String)],
    tunes: &[(String, String)],
    item: &Drop,
    line: &dyn crate::bbs::Line,
) -> Option<String> {
    let name = match item {
        Drop::Text(id) if line.has_text(id) => {
            return Some("that textfile's already on your disk.".into());
        }
        _ if !line.owns(item) => return None,
        Drop::Tool(id) => tools.iter().find(|t| t.id == *id).map(|t| &t.name),
        Drop::Game(id) => cracks.iter().find(|(g, _)| g == id).map(|(_, l)| l),
        Drop::Tune(id) => tunes.iter().find(|(t, _)| t == id).map(|(_, l)| l),
        _ => None,
    };
    Some(format!("you already have {}.", name.map_or("that", |n| n)))
}

/// A board's listing for one visit: per area, `stock` files picked at random.
pub fn roll_stock(def: &crate::data::BbsDef, rng: &mut Rng) -> Vec<Vec<StockFile>> {
    def.areas
        .iter()
        .map(|a| {
            // Always-stocked areas take no roll: they never touch the RNG.
            if a.always {
                return (0..a.files.len())
                    .map(|def| StockFile { def, bought: false })
                    .collect();
            }
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
