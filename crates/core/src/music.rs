//! The run's soundtrack: a looping chiptune per network tier, from the deck's sound pack
//! (`music:` in `data/sounds/<id>.ron`). It's look only: it reads the game and never changes
//! it, takes nothing from the RNG and adds no action. Here core decides what should be playing
//! ([`wanted`]) and how it's arranged at the current trace ([`arrangement`]); the app renders
//! the loop (`TuneDef::render_loop`) and plays it.

use serde::Deserialize;

use crate::intro::TuneDef;

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
