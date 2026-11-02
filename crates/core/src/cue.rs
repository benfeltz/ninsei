//! Sound cues: what a run's actions sound like. `Game::apply` works out what happened and
//! pushes cues onto `Game::cues`; the app drains them each frame and plays them. Recipes are
//! data (`data/sounds.ron`), rendered here like the modem; nothing here touches the game's RNG.

use serde::Deserialize;

use crate::game::{Game, Phase};

/// Something worth a sound.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cue {
    /// Moved to another node.
    Hop,
    /// ICE woke up: a fight starts.
    IceRez,
    /// A breaker took strength off the ICE.
    Hit,
    /// Shield raised.
    Shield,
    /// Integrity repaired.
    Heal,
    /// The ICE attacked and the shield took it all.
    Block,
    /// The ICE got through: integrity lost.
    Damage,
    /// ICE derezzed.
    Derez,
    /// Loot: credits, programs, fragments, upgrades.
    Loot,
    /// A BBS download finished.
    Download,
    /// Trace crossed into a new quarter.
    TraceUp,
    /// Someone's on the line (an encounter).
    Encounter,
    Flatline,
    JackOut,
}

impl Cue {
    pub const ALL: [Cue; 14] = [
        Cue::Hop,
        Cue::IceRez,
        Cue::Hit,
        Cue::Shield,
        Cue::Heal,
        Cue::Block,
        Cue::Damage,
        Cue::Derez,
        Cue::Loot,
        Cue::Download,
        Cue::TraceUp,
        Cue::Encounter,
        Cue::Flatline,
        Cue::JackOut,
    ];
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum Wave {
    Square,
    Saw,
    Triangle,
    Sine,
    Noise,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Note {
    pub wave: Wave,
    pub from: f32,
    pub to: f32,
    pub ms: u32,
    pub vol: f32,
}

/// One line of `data/sounds.ron`.
#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct SoundDef {
    pub cue: Cue,
    pub notes: Vec<Note>,
}

/// Most cues kept waiting: if nothing drains them, the oldest go.
pub const QUEUE_MAX: usize = 32;

/// Longest a cue may run, all notes together.
pub const MAX_MS: u32 = 900;

impl SoundDef {
    pub fn validate(defs: &[SoundDef]) -> Result<(), String> {
        for c in Cue::ALL {
            let n = defs.iter().filter(|d| d.cue == c).count();
            if n != 1 {
                return Err(format!(
                    "{c:?} is listed {n} times; it needs exactly one entry"
                ));
            }
        }
        for d in defs {
            if d.notes.is_empty() {
                return Err(format!("{:?} has no notes", d.cue));
            }
            let total: u32 = d.notes.iter().map(|n| n.ms).sum();
            if total == 0 || total > MAX_MS {
                return Err(format!("{:?} runs {total} ms; keep it 1-{MAX_MS}", d.cue));
            }
            if let Some(n) = d.notes.iter().find(|n| !(0.0..=1.0).contains(&n.vol)) {
                return Err(format!("{:?} has a note at volume {}", d.cue, n.vol));
            }
            if let Some(n) = d
                .notes
                .iter()
                .find(|n| n.from < 0.0 || n.to < 0.0 || n.from > 12_000.0 || n.to > 12_000.0)
            {
                return Err(format!("{:?} has a note at {}-{} Hz", d.cue, n.from, n.to));
            }
        }
        Ok(())
    }

    /// The cue as mono samples at `rate`, in -1..=1. Same recipe, same sound.
    pub fn render(&self, rate: u32) -> Vec<f32> {
        let rate = rate.max(1) as f32;
        let mut out = Vec::new();
        // Noise from a fixed seed, so a cue always sounds the same.
        let mut seed = 0x2545_F491_u32;
        for n in &self.notes {
            let len = (n.ms as f32 / 1000.0 * rate) as usize;
            let mut phase = 0.0_f32;
            let mut held = 0.0_f32;
            for i in 0..len {
                let k = i as f32 / len.max(1) as f32;
                let f = n.from + (n.to - n.from) * k;
                phase = (phase + f / rate).fract();
                let s = match n.wave {
                    Wave::Square => {
                        if phase < 0.5 {
                            1.0
                        } else {
                            -1.0
                        }
                    }
                    Wave::Saw => phase * 2.0 - 1.0,
                    Wave::Triangle => 1.0 - 4.0 * (phase - 0.5).abs(),
                    Wave::Sine => (phase * std::f32::consts::TAU).sin(),
                    Wave::Noise => {
                        // Sample-and-hold white noise; `from` sets how often it changes.
                        let hold = (rate / n.from.max(50.0)).max(1.0) as usize;
                        if i % hold == 0 {
                            seed ^= seed << 13;
                            seed ^= seed >> 17;
                            seed ^= seed << 5;
                            held = (seed % 2001) as f32 / 1000.0 - 1.0;
                        }
                        held
                    }
                };
                // A click-free envelope: 3 ms in, then a fall to silence.
                let attack = (i as f32 / (0.003 * rate)).min(1.0);
                let env = attack * (1.0 - k).powf(1.5);
                out.push((s * n.vol * env).clamp(-1.0, 1.0));
            }
        }
        out
    }
}

/// What a cue compares before and after an action.
#[derive(Clone, Copy, Debug)]
pub struct Snap {
    hops: u32,
    integrity: u32,
    trace: u32,
    credits: u32,
    fragments: u32,
    programs: usize,
    phase: Phase,
    /// (node, ICE strength, shield) while fighting.
    fight: Option<(usize, u32, u32)>,
    encounter: bool,
}

impl Snap {
    pub fn of(g: &Game) -> Snap {
        Snap {
            hops: g.hops,
            integrity: g.deck.integrity,
            trace: g.deck.trace,
            credits: g.deck.credits,
            fragments: g.deck.fragments,
            programs: g.hdd.len(),
            phase: g.phase,
            fight: g.combat.as_ref().map(|c| (c.node, c.strength, c.shield)),
            encounter: g.encounter.is_some(),
        }
    }
}

/// The cues for what an action did, given the game before (`was`) and after (`g`). `bought`
/// is a BBS download, which isn't loot.
pub fn cues(was: &Snap, g: &Game, bought: bool) -> Vec<Cue> {
    let mut out = Vec::new();
    let now = g.combat.as_ref().map(|c| (c.node, c.strength, c.shield));
    if g.hops > was.hops {
        out.push(Cue::Hop);
    }
    match (was.fight, now) {
        (None, Some(_)) => out.push(Cue::IceRez),
        (Some((n, before, _)), Some((n2, after, _))) if n == n2 && after < before => {
            out.push(Cue::Hit);
        }
        (Some(_), None) if g.phase != Phase::Flatlined => out.push(Cue::Derez),
        _ => {}
    }
    let (shield_was, shield_now) = (was.fight.map_or(0, |f| f.2), now.map_or(0, |f| f.2));
    if g.deck.integrity < was.integrity && g.phase != Phase::Flatlined {
        out.push(Cue::Damage);
    } else if shield_now < shield_was && was.fight.is_some() && now.is_some() {
        out.push(Cue::Block);
    }
    if shield_now > shield_was {
        out.push(Cue::Shield);
    }
    if g.deck.integrity > was.integrity {
        out.push(Cue::Heal);
    }
    if bought {
        out.push(Cue::Download);
    } else if g.deck.credits > was.credits
        || g.deck.fragments > was.fragments
        || g.hdd.len() > was.programs
    {
        out.push(Cue::Loot);
    }
    if g.deck.trace / 25 > was.trace / 25 && g.deck.trace < 100 {
        out.push(Cue::TraceUp);
    }
    if g.encounter.is_some() && !was.encounter {
        out.push(Cue::Encounter);
    }
    if g.phase != was.phase {
        match g.phase {
            Phase::Flatlined => out.push(Cue::Flatline),
            Phase::JackedOut => out.push(Cue::JackOut),
            _ => {}
        }
    }
    out
}
