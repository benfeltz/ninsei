//! The run's soundtrack: a looping chiptune per network tier, from the deck's sound pack
//! (`music:` in `data/sounds/<id>.ron`). It's look only: it reads the game and never changes
//! it, takes nothing from the RNG and adds no action. Here core decides what should be playing
//! ([`wanted`]) and how it's arranged at the current trace ([`arrangement`]); the app renders
//! the loop a slice at a time (`TuneDef::loop_render`, the drop track as its own stem) and
//! plays it.

use serde::Deserialize;

use crate::data::Data;
use crate::game::{Game, Phase};
use crate::hub::{NetworksDef, Save, Screen};
use crate::intro::TuneDef;
use crate::tuning::{MUSIC_DROP_AT, MUSIC_TEMPO_AT, MUSIC_TEMPO_BPM, TRACE_MAX};

/// Longest a tune may run before it repeats, in seconds: a whole loop is rendered and held in
/// memory, so tracks whose lengths only line up after many minutes are refused.
pub const MAX_LOOP_SECS: f32 = 64.0;

/// One tier's tune in a sound pack.
#[derive(Deserialize, Clone, Debug)]
pub struct RunTune {
    /// The network tier it plays on (one in `hub/networks.ron`).
    pub tier: u32,
    /// The track (by index) that falls silent when trace runs high.
    #[serde(default)]
    pub drop: Option<usize>,
    pub tune: TuneDef,
}

/// How a tune plays right now: the track left out, if any, and extra bpm.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Arrangement {
    pub drop: Option<usize>,
    pub tempo: u32,
}

impl Arrangement {
    /// The tune as written.
    pub const PLAIN: Arrangement = Arrangement {
        drop: None,
        tempo: 0,
    };
}

/// What the run should be playing: the sound pack and tier (which tune), how it's arranged,
/// and the tune itself.
#[derive(Clone, Copy, Debug)]
pub struct Wanted<'a> {
    pub pack: &'a str,
    pub tier: u32,
    pub arrangement: Arrangement,
    pub tune: &'a RunTune,
}

impl Wanted<'_> {
    /// Which tune, arrangement aside: the same one keeps its place when the trace rearranges it.
    pub fn same_tune(&self, pack: &str, tier: u32) -> bool {
        self.pack == pack && self.tier == tier
    }
}

/// The arrangement at `trace_pct` (0-100) for a tune whose drop track is `drop`. Coarse on
/// purpose: two steps, so the music never argues with the trace number.
pub fn arrangement(trace_pct: u32, drop: Option<usize>) -> Arrangement {
    Arrangement {
        drop: drop.filter(|_| trace_pct >= MUSIC_DROP_AT),
        tempo: if trace_pct >= MUSIC_TEMPO_AT {
            MUSIC_TEMPO_BPM
        } else {
            0
        },
    }
}

/// What the run should be playing: a tune while you're jacking in or in the run and it's live
/// (moving, fighting or talking); nothing at home, on the crash screen or once the run is
/// over. The deck's firmware picks the pack, and a pack with no tune for the tier is silent.
pub fn wanted<'a>(
    screen: Screen,
    crashed: bool,
    save: &Save,
    game: &Game,
    data: &'a Data,
) -> Option<Wanted<'a>> {
    if !matches!(screen, Screen::JackIn { .. } | Screen::Run) || crashed {
        return None;
    }
    if !matches!(
        game.phase,
        Phase::Exploring | Phase::Combat | Phase::Encounter
    ) {
        return None;
    }
    let pack = save.firmware_def(data).sounds.as_str();
    let tier = game.tier();
    let tune = data.run_tune(pack, tier)?;
    let pct = game.deck.trace.min(TRACE_MAX) * 100 / TRACE_MAX.max(1);
    Some(Wanted {
        pack,
        tier,
        arrangement: arrangement(pct, tune.drop),
        tune,
    })
}

/// A pack's music: each tune on a tier the networks have, each tier at most once, every tune
/// sound and under [`MAX_LOOP_SECS`] a loop, every drop a real track.
pub fn validate(music: &[RunTune], networks: &NetworksDef) -> Result<(), String> {
    for (i, m) in music.iter().enumerate() {
        if networks.tier(m.tier).is_none() {
            return Err(format!("music names unknown tier {}", m.tier));
        }
        if music[..i].iter().any(|o| o.tier == m.tier) {
            return Err(format!("tier {} has two tunes", m.tier));
        }
        m.tune
            .validate()
            .map_err(|e| format!("tier {} tune: {e}", m.tier))?;
        let secs = m.tune.loop_secs(0);
        if secs > MAX_LOOP_SECS {
            return Err(format!(
                "tier {} loops after {secs:.0} s (keep it under {MAX_LOOP_SECS} s)",
                m.tier
            ));
        }
        if let Some(d) = m.drop
            && d >= m.tune.tracks.len()
        {
            return Err(format!(
                "tier {} drops track {d}, but it has {} tracks",
                m.tier,
                m.tune.tracks.len()
            ));
        }
    }
    Ok(())
}
