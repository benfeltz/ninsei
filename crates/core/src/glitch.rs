//! How badly the deck is hurting, for the damage glitches the app draws: a steady level from
//! integrity, and text corruption that keeps every line's width. Pure; the app does the drawing.

use serde::{Deserialize, Serialize};

/// The damage glitches setting, saved with the hub.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Glitches {
    /// Everything: fringing, static, shake, tearing, corrupted text, dropouts.
    #[default]
    On,
    /// Colour fringing and the hit flash only.
    Reduced,
    Off,
}

impl Glitches {
    pub const ALL: [Glitches; 3] = [Glitches::On, Glitches::Reduced, Glitches::Off];

    pub fn label(self) -> &'static str {
        match self {
            Glitches::On => "on",
            Glitches::Reduced => "reduced",
            Glitches::Off => "off",
        }
    }

    pub fn next(self) -> Glitches {
        match self {
            Glitches::On => Glitches::Reduced,
            Glitches::Reduced => Glitches::Off,
            Glitches::Off => Glitches::On,
        }
    }
}

/// Integrity (as a share of max) above which the screen stays clean.
pub const CLEAN_ABOVE: f32 = 0.7;

/// The steady damage level, 0 (clean) to 1 (dying): zero above 70% integrity, climbing
/// slowly at first and fast below about 30%.
pub fn level(integrity: u32, max: u32) -> f32 {
    if max == 0 {
        return 0.0;
    }
    let r = (integrity as f32 / max as f32).clamp(0.0, 1.0);
    if r >= CLEAN_ABOVE {
        return 0.0;
    }
    ((CLEAN_ABOVE - r) / CLEAN_ABOVE).powf(1.6)
}

/// Glyphs a corrupted character turns into.
const GARBAGE: [char; 8] = ['░', '▒', '▓', '█', '#', '%', '&', '@'];

/// Swaps about `rate` of the characters in `text` for garbage, one for one, so the line keeps
/// its width. `seed` picks which; the same seed corrupts the same way. Spaces stay spaces.
pub fn corrupt(text: &str, seed: u64, rate: f32) -> String {
    if rate <= 0.0 {
        return text.to_string();
    }
    let cut = (rate.clamp(0.0, 1.0) * 1000.0) as u64;
    text.chars()
        .enumerate()
        .map(|(i, c)| {
            if c == ' ' {
                return c;
            }
            let mut x = seed ^ (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
            x ^= x >> 31;
            x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
            x ^= x >> 29;
            if x % 1000 < cut {
                GARBAGE[(x >> 12) as usize % GARBAGE.len()]
            } else {
                c
            }
        })
        .collect()
}
