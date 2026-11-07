//! The run's soundtrack: a looping chiptune per network tier, from the deck's sound pack
//! (`music:` in `data/sounds/<id>.ron`). It's look only: it reads the game and never changes
//! it, takes nothing from the RNG and adds no action. Here core decides what should be playing
//! ([`wanted`]) and how it's arranged at the current trace ([`arrangement`]); the app renders
//! the loop (`TuneDef::render_loop`) and plays it.

use serde::Deserialize;

use crate::intro::TuneDef;
use crate::tuning::{MUSIC_DROP_AT, MUSIC_TEMPO_AT, MUSIC_TEMPO_BPM};

/// Network tiers run 1 to this.
pub const TIERS: u32 = 3;

/// One tier's tune in a sound pack.
#[derive(Deserialize, Clone, Debug)]
pub struct RunTune {
    /// The network tier it plays on (1-3).
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

/// What should be on the music channel: the sound pack, the tier and the arrangement. The app
/// re-renders the loop only when this changes.
pub type MusicKey = (String, u32, Arrangement);

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

/// A pack's music: tiers 1-3, each at most once, every tune sound, every drop a real track.
pub fn validate(music: &[RunTune]) -> Result<(), String> {
    for (i, m) in music.iter().enumerate() {
        if !(1..=TIERS).contains(&m.tier) {
            return Err(format!("music for tier {} (tiers are 1-{TIERS})", m.tier));
        }
        if music[..i].iter().any(|o| o.tier == m.tier) {
            return Err(format!("tier {} has two tunes", m.tier));
        }
        m.tune
            .validate()
            .map_err(|e| format!("tier {} tune: {e}", m.tier))?;
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
