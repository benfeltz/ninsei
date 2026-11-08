//! The fight view's rules: what running a program would do right now (the preview `run` itself
//! uses, so the hint can't lie), how much of the ICE's wireframe is left at a given strength,
//! and which edges break first. Pure; the app's `fightview.rs` draws it.

use crate::data::{Effect, Intent};
use crate::game::Game;

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
