//! 1-bit fill patterns (`data/patterns.ron`), after MacPaint's palette: 8x8 tiles of ink and
//! paper that the Sense/Net deck's parts shade with. Core only holds the bits; the app draws a
//! tile in two theme roles, pixel-aligned at a whole-number scale.

use serde::Deserialize;

/// Tiles are this many pixels on a side.
pub const SIZE: usize = 8;

/// Patterns the Liza parts draw with: validation insists they're all there.
pub const REQUIRED: &[&str] = &[
    "p12",
    "p25",
    "p50",
    "p75",
    "lines",
    "columns",
    "diagonal",
    "crosshatch",
    "bricks",
    "dots",
    "pinstripes",
];

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PatternDef {
    pub id: String,
    pub label: String,
    /// `SIZE` rows of `SIZE` chars: `#` ink, `.` paper.
    pub rows: Vec<String>,
    /// A coarser pattern to draw instead where this one shimmers through the CRT.
    #[serde(default)]
    pub fallback: Option<String>,
}

impl PatternDef {
    /// Whether the pixel at (`x`, `y`) is ink. The tile repeats, so any coordinate works.
    pub fn ink(&self, x: usize, y: usize) -> bool {
        self.rows
            .get(y % SIZE)
            .and_then(|r| r.as_bytes().get(x % SIZE))
            .is_some_and(|&c| c == b'#')
    }

    /// How much of the tile is ink, in percent (rounded down).
    pub fn coverage_pct(&self) -> u32 {
        let ink = (0..SIZE * SIZE)
            .filter(|i| self.ink(i % SIZE, i / SIZE))
            .count();
        (ink * 100 / (SIZE * SIZE)) as u32
    }

    pub fn validate(defs: &[PatternDef]) -> Result<(), String> {
        for (i, p) in defs.iter().enumerate() {
            if defs[..i].iter().any(|o| o.id == p.id) {
                return Err(format!("pattern `{}` is listed twice", p.id));
            }
            if p.rows.len() != SIZE || p.rows.iter().any(|r| r.chars().count() != SIZE) {
                return Err(format!("pattern `{}` must be {SIZE}x{SIZE}", p.id));
            }
            if let Some(c) = p
                .rows
                .iter()
                .flat_map(|r| r.chars())
                .find(|c| !"#.".contains(*c))
            {
                return Err(format!("pattern `{}` has `{c}`: only `#` and `.`", p.id));
            }
            if let Some(f) = &p.fallback
                && !defs.iter().any(|o| &o.id == f)
            {
                return Err(format!("pattern `{}` falls back to unknown `{f}`", p.id));
            }
        }
        for id in REQUIRED {
            if !defs.iter().any(|p| p.id == *id) {
                return Err(format!("no `{id}` pattern"));
            }
        }
        Ok(())
    }
}
