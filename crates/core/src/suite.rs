//! The Hosaka's productivity suite, the rules behind its look: spreadsheet columns and sorting,
//! formulas for programs, report axes, ledger codes, work-order totals, and the dialogs that
//! pop up when something worth filing happens. Pure; the app's `suite.rs` draws it.

use crate::binary::Spec;
use crate::data::Effect;
use crate::game::{Contract, ContractStatus, Game, LogKind, Phase, Slot};
use crate::history::TurnRow;

/// Spreadsheet column letters: 0 is `A`, 25 is `Z`, 26 is `AA`.
pub fn col_name(mut i: usize) -> String {
    let mut out = Vec::new();
    loop {
        out.push(b'A' + (i % 26) as u8);
        if i < 26 {
            break;
        }
        i = i / 26 - 1;
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

/// The ledger's REF column: a short code for each kind of log line.
pub fn log_ref(kind: LogKind) -> &'static str {
    match kind {
        LogKind::Sys => "SYS",
        LogKind::Info => "MSG",
        LogKind::Good => "OK",
        LogKind::Bad => "ERR",
        LogKind::Loot => "CR",
    }
}

/// The network as a document name: `BRIGHTWATER MUTUAL.NET`.
pub fn doc_name(site: Option<&str>) -> String {
    let name = site.unwrap_or("LOCAL").to_uppercase();
    format!("{name}.NET")
}

/// The login on the status line, the same for a whole run: `GUEST-0451`.
pub fn user_id(seed: u64) -> String {
    format!("GUEST-{:04}", seed % 10_000)
}

/// The date on the status line: the company's calendar, somewhere in the early 90s, fixed for a
/// run (the browser has no clock we may read for this).
pub fn date(seed: u64) -> String {
    let month = seed % 12 + 1;
    let day = (seed / 12) % 28 + 1;
    let year = 90 + (seed / 336) % 6;
    format!("{month:02}-{day:02}-{year}")
}

/// A program as a spreadsheet formula: `=BREAK(CodeGate, 6)`.
pub fn formula(spec: &Spec) -> String {
    match spec.effect {
        Effect::Break { power, vs: None } => format!("=BREAK({power})"),
        Effect::Break { power, vs: Some(c) } => format!("=BREAK({c:?}, {power})"),
        Effect::Shield(n) => format!("=SHIELD({n})"),
        Effect::Repair(n) => format!("=REPAIR({n})"),
        Effect::Scrub(n) => format!("=SCRUB(TRACE, {n})"),
        Effect::Stun => "=STUN(ICE, 1)".to_string(),
    }
}

/// `n + 1` evenly spaced axis ticks from 0 to `max`, rounded down.
pub fn ticks(max: u32, n: u32) -> Vec<u32> {
    let n = n.max(1);
    (0..=n).map(|k| max * k / n).collect()
}

/// The warez spreadsheet's columns.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum SheetCol {
    #[default]
    Slot,
    File,
    Ram,
    Cpu,
    Opt,
    Status,
}

impl SheetCol {
    pub const ALL: [SheetCol; 6] = [
        SheetCol::Slot,
        SheetCol::File,
        SheetCol::Ram,
        SheetCol::Cpu,
        SheetCol::Opt,
        SheetCol::Status,
    ];

    pub fn header(self) -> &'static str {
        match self {
            SheetCol::Slot => "SLOT",
            SheetCol::File => "FILE",
            SheetCol::Ram => "RAM",
            SheetCol::Cpu => "CPU",
            SheetCol::Opt => "OPT",
            SheetCol::Status => "STATUS",
        }
    }

    /// What the cell shows for HDD slot `i`.
    pub fn cell(self, i: usize, s: &Slot) -> String {
        match self {
            SheetCol::Slot => i.to_string(),
            SheetCol::File => s.spec.file(),
            SheetCol::Ram => s.spec.ram.to_string(),
            SheetCol::Cpu => s.spec.cpu.to_string(),
            SheetCol::Opt => s.spec.opt.to_string(),
            SheetCol::Status => if s.loaded { "LOADED" } else { "STORED" }.to_string(),
        }
    }
}

/// Characters each sheet column needs to show every value and its header with a sort arrow
/// (` ▲`) whole: the row-number column (the last row's number, or the header row's `1`),
/// then `SheetCol::ALL`'s, sized from the widest real value so nothing is cut.
pub fn sheet_widths(hdd: &[Slot]) -> (usize, [usize; 6]) {
    let rownum = (hdd.len() + 1).to_string().len();
    let mut w = [0; 6];
    for (k, c) in SheetCol::ALL.iter().enumerate() {
        w[k] = hdd
            .iter()
            .enumerate()
            .map(|(i, s)| c.cell(i, s).chars().count())
            .chain([c.header().chars().count() + 2])
            .max()
            .unwrap_or(0);
    }
    (rownum, w)
}

/// HDD slot indices in spreadsheet order: sorted by `col` (ties by slot), optionally only the
/// loaded rows.
pub fn sheet_rows(hdd: &[Slot], col: SheetCol, ascending: bool, loaded_only: bool) -> Vec<usize> {
    let mut rows: Vec<usize> = (0..hdd.len())
        .filter(|&i| !loaded_only || hdd[i].loaded)
        .collect();
    rows.sort_by(|&a, &b| {
        let (x, y) = (&hdd[a], &hdd[b]);
        let ord = match col {
            SheetCol::Slot => a.cmp(&b),
            SheetCol::File => x.spec.file().cmp(&y.spec.file()),
            SheetCol::Ram => x.spec.ram.cmp(&y.spec.ram),
            SheetCol::Cpu => x.spec.cpu.cmp(&y.spec.cpu),
            SheetCol::Opt => x.spec.opt.cmp(&y.spec.opt),
            SheetCol::Status => y.loaded.cmp(&x.loaded),
        };
        let ord = if ascending { ord } else { ord.reverse() };
        ord.then(a.cmp(&b))
    });
    rows
}

/// Money still to be made on open work orders.
pub fn outstanding(contracts: &[Contract]) -> u32 {
    contracts
        .iter()
        .filter(|k| k.status == ContractStatus::Active)
        .map(|k| k.reward + k.ghost.map_or(0, |(_, b)| b))
        .sum()
}

/// The ghost bonus as a conditional cell: `+57¢ if TRACE ≤ 40%`, or why it's gone.
pub fn ghost_cell(k: &Contract, trace: u32) -> Option<String> {
    let (cap, bonus) = k.ghost?;
    Some(if trace <= cap {
        format!("+{bonus}¢ if TRACE ≤ {cap}%")
    } else {
        format!("+{bonus}¢ VOID (TRACE > {cap}%)")
    })
}

/// The ICE's strength at the end of each turn of the fight, from the turn rows.
pub fn strength_by_turn(max: u32, turns: &[TurnRow]) -> Vec<u32> {
    let mut left = max;
    turns
        .iter()
        .map(|t| {
            left = left.saturating_sub(t.dealt);
            left
        })
        .collect()
}

/// A boxed dialog the suite shows when something is worth filing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub title: String,
    pub lines: Vec<String>,
}

/// Watches a run for the moments that get a dialog: a contract paid, jacking out. (A flatline
/// has the crash screen.)
/// Feed it the game after every frame; it only reads.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Watch {
    seed: u64,
    phase: Option<Phase>,
    paid: usize,
}

impl Watch {
    /// Whether this is still watching the run `g` (false once a new run has started).
    pub fn watching(&self, g: &Game) -> bool {
        self.phase.is_some() && self.seed == g.seed
    }

    pub fn step(&mut self, g: &Game) -> Vec<Notice> {
        let settled =
            |k: &&Contract| matches!(k.status, ContractStatus::Done | ContractStatus::Betrayed);
        let paid = g.contracts.iter().filter(settled).count();
        // A new run (or the first look) starts quiet.
        if self.phase.is_none() || self.seed != g.seed || paid < self.paid {
            *self = Watch {
                seed: g.seed,
                phase: Some(g.phase),
                paid,
            };
            return Vec::new();
        }
        let mut out = Vec::new();
        if paid > self.paid {
            for k in g.contracts.iter().filter(settled).skip(self.paid) {
                out.push(match g.betrayal_label(k) {
                    None => Notice {
                        title: "WORK ORDER COMPLETE".into(),
                        lines: vec![
                            format!("CLIENT   {}", k.client),
                            format!("AMOUNT   {}¢ credited to your account.", k.paid),
                        ],
                    },
                    Some(label) => Notice {
                        title: "WORK ORDER VOID".into(),
                        lines: vec![
                            format!("CLIENT   {}", k.client),
                            format!("STATUS   client defaulted ({label})."),
                            format!("AMOUNT   {}¢ of {}¢ received.", k.paid, k.reward),
                        ],
                    },
                });
            }
        }
        if self.phase != Some(g.phase) && g.phase == Phase::JackedOut {
            out.push(Notice {
                title: "SESSION ENDED".into(),
                lines: vec![
                    "You have been logged off. Thank you for using Hosaka.".into(),
                    format!("CREDITS  {}¢    HOPS  {}", g.deck.credits, g.hops),
                ],
            });
        }
        self.phase = Some(g.phase);
        self.paid = paid;
        out
    }
}
