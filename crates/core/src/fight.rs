//! The fight view's rules: what running a program would do right now (the preview `run` itself
//! uses, so the hint can't lie), how much of the ICE's wireframe is left at a given strength,
//! and which edges break first. Pure; the app's `fightview.rs` draws it.

use crate::data::{Effect, Intent};
use crate::game::{ExecState, Game, Step, StepWhat};

/// How a breaker matches the ICE's class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Match {
    /// A general breaker.
    Plain,
    /// Built for this class: double.
    Strong,
    /// Built for another class: half.
    Weak,
}

/// What a program does if it runs now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Takes `hit` strength off the ICE.
    Break {
        hit: u32,
        vs: Match,
    },
    Shield(u32),
    Repair(u32),
    Scrub(u32),
    Stun,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preview {
    pub cpu: u32,
    pub outcome: Outcome,
}

impl Preview {
    /// Strength it takes off the ICE (0 for anything but a breaker).
    pub fn hit(&self) -> u32 {
        match self.outcome {
            Outcome::Break { hit, .. } => hit,
            _ => 0,
        }
    }
}

impl Game {
    /// What running HDD slot `slot` would do now, or why it can't run. `Action::Run` resolves
    /// through this, so the fight view's preview is always what happens.
    pub fn preview(&self, slot: usize) -> Result<Preview, String> {
        let s = self
            .hdd
            .get(slot)
            .ok_or_else(|| format!("no program in slot {slot}."))?;
        let w = &s.spec;
        let Some(c) = self.combat.as_ref() else {
            return Err("nothing to run against.".into());
        };
        if c.cpu < w.cpu {
            return Err(format!("{} needs {} cpu, {} left.", w.name, w.cpu, c.cpu));
        }
        if w.effect == Effect::Stun && c.stun_immune {
            return Err("the ICE has adapted to lag. a stun won't land this turn.".into());
        }
        let outcome = match w.effect {
            Effect::Break { power, vs } => {
                let class = self.data.ice[c.ice].class;
                // Specialised breakers double against their class and halve against the rest.
                let (hit, vs) = match vs {
                    Some(v) if v == class => (power * 2, Match::Strong),
                    Some(_) => (power / 2, Match::Weak),
                    None => (power, Match::Plain),
                };
                Outcome::Break { hit, vs }
            }
            Effect::Shield(n) => Outcome::Shield(n),
            Effect::Repair(n) => Outcome::Repair(n),
            Effect::Scrub(n) => Outcome::Scrub(n),
            Effect::Stun => Outcome::Stun,
        };
        Ok(Preview {
            cpu: w.cpu,
            outcome,
        })
    }
}

/// One line of the deck's program, as every chrome shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineView {
    pub no: u32,
    /// The statement as typed.
    pub text: String,
    /// The line about to run.
    pub current: bool,
    /// A STOP: the breakpoint.
    pub stop: bool,
    /// Why the line won't run as written, in the interpreter's words.
    pub fault: Option<String>,
}

/// The deck's program in this fight, for the views: its lines with their marks, the budgets
/// left this turn, and this turn's pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProgramView {
    pub lines: Vec<LineView>,
    /// Text typed with no line number: shown after the program, never run.
    pub stray: Vec<String>,
    pub state: ExecState,
    /// Whether there's a fight for it to run in.
    pub fighting: bool,
    pub cpu_left: u32,
    pub ifs_left: u32,
    pub turn: u32,
    pub trace: Vec<Step>,
    /// A pass is under way this turn: the cursor is partway down it.
    pub open: bool,
    /// A hang on this line: `?BREAK IN n`, the cursor held there.
    pub broke: Option<u32>,
}

impl Game {
    /// GB left for IFs this turn: the deck's RAM outside a fight.
    pub fn ifs_left(&self) -> u32 {
        self.combat.as_ref().map_or(self.deck.ram_max, |c| c.ifs)
    }

    /// The deck's program as the views show it.
    pub fn program_view(&self) -> ProgramView {
        let fighting = self.combat.is_some() && self.phase == crate::game::Phase::Combat;
        let lines = self
            .program
            .lines
            .iter()
            .map(|l| LineView {
                no: l.no,
                text: l.text.clone(),
                current: fighting && self.exec.pc == Some(l.no),
                stop: l.stmt == crate::basic::Stmt::Stop,
                fault: self.program.fault(l.no),
            })
            .collect();
        let c = self.combat.as_ref();
        ProgramView {
            lines,
            stray: self.program.stray.clone(),
            state: self.exec.state,
            fighting,
            cpu_left: c.map_or(self.deck.cpu_max, |c| c.cpu),
            ifs_left: self.ifs_left(),
            turn: c.map_or(0, |c| c.turn),
            trace: self.exec.trace.clone(),
            open: fighting && self.exec.open,
            broke: self.exec.broke.filter(|_| fighting),
        }
    }

    /// A step of a pass as the deck's OS says it: one sentence per line executed.
    pub fn step_text(&self, s: &Step) -> String {
        let text = self.program.line(s.line).map_or("", |l| l.text.as_str());
        let no = s.line;
        match &s.what {
            StepWhat::Ran { uid, outcome } => {
                let name = self
                    .hdd
                    .iter()
                    .find(|p| p.uid == *uid)
                    .map_or("the program".to_string(), |p| p.spec.name.clone());
                let did = match outcome {
                    Outcome::Break { hit, .. } => format!("breaks {hit}"),
                    Outcome::Shield(n) => format!("puts up shield {n}"),
                    Outcome::Repair(n) => format!("restores {n} integrity"),
                    Outcome::Scrub(n) => format!("scrubs {n} trace"),
                    Outcome::Stun => "lags the ICE".to_string(),
                };
                format!("{no} {text}: {name} {did}.")
            }
            StepWhat::Skipped(why) => format!("{no} {text}: skipped. {why}"),
            StepWhat::If(true) => format!("{no} {text}: yes."),
            StepWhat::If(false) => format!("{no} {text}: no. skipping the next line."),
            StepWhat::Goto(to) => format!("{no} GOTO {to}."),
            StepWhat::Stop => format!("BREAK IN {no}"),
            StepWhat::Break => format!("?BREAK IN {no}"),
            StepWhat::OutOfCpu => "out of cycles. the turn is over.".to_string(),
            StepWhat::OutOfRam => format!("{no} {text}: no memory left for it. the turn is over."),
            StepWhat::End => "end of program. the turn is over.".to_string(),
        }
    }
}

/// How many of a wireframe's `total` edges are still up at `strength` of `max`: all of them at
/// full strength, at least one while it stands, none once it falls.
pub fn edges_left(total: usize, strength: u32, max: u32) -> usize {
    if strength == 0 || total == 0 {
        return 0;
    }
    let max = max.max(strength);
    ((total as u64 * strength as u64).div_ceil(max as u64) as usize).clamp(1, total)
}

/// The order a wireframe's edges break in: the same for the same ICE node, mixed so the damage
/// spreads over the shape.
pub fn break_order(total: usize, seed: u64) -> Vec<usize> {
    let mut order: Vec<usize> = (0..total).collect();
    let mut x = seed ^ 0x9E37_79B9_7F4A_7C15;
    for i in (1..total).rev() {
        x ^= x >> 30;
        x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        x ^= x >> 27;
        x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
        x ^= x >> 31;
        order.swap(i, (x % (i as u64 + 1)) as usize);
    }
    order
}

/// The edges (by index) that are broken at `strength` of `max`.
pub fn broken(total: usize, seed: u64, strength: u32, max: u32) -> Vec<usize> {
    let gone = total - edges_left(total, strength, max);
    break_order(total, seed).into_iter().take(gone).collect()
}

/// Whether the ICE's next move hurts you (the view charges up before it).
pub fn threatens(intent: Intent) -> bool {
    matches!(intent, Intent::Attack(_))
}
