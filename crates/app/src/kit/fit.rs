//! How much room a screen needs. A screen declares the smallest cell grids it works in (one
//! [`Fit`], or a few when it lays out differently by shape: wide and short, or narrow and
//! tall); the desktop checks the room it has against them before drawing the screen and,
//! when none fits, draws the one notice instead, btop style:
//!
//! ```text
//!   NiGHTCOMM needs 64x20 (has 41x12)
//!   Esc f: full screen
//! ```
//!
//! The hint is the desktop's to give (or not). No screen draws its own "too small".

use egui::{Color32, Rect, Ui};

use super::Cells;
use super::cells::cells_in;

/// The smallest grid of cells a screen works in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fit {
    pub cols: usize,
    pub rows: usize,
}

impl Fit {
    /// For a screen that declares nothing.
    pub const DEFAULT: Fit = Fit { cols: 40, rows: 12 };

    /// Whether a grid of `cols` × `rows` is enough.
    pub fn fits(self, (cols, rows): (usize, usize)) -> bool {
        cols >= self.cols && rows >= self.rows
    }
}

/// Whether a grid of `cols` × `rows` is enough for any of `need`.
pub fn fits(need: &[Fit], have: (usize, usize)) -> bool {
    need.iter().any(|f| f.fits(have))
}

/// The cells `rect` holds in `ui`'s monospace font, and whether that's enough for `need`.
pub fn check(ui: &Ui, rect: Rect, need: &[Fit]) -> ((usize, usize), bool) {
    let have = cells_in(ui, rect);
    (have, fits(need, have))
}

/// The notice's lines: what needs how much and has how much, then the desktop's hint.
pub fn lines(
    name: &str,
    need: &[Fit],
    (cols, rows): (usize, usize),
    hint: Option<&str>,
) -> Vec<String> {
    let grids: Vec<String> = need
        .iter()
        .map(|f| format!("{}x{}", f.cols, f.rows))
        .collect();
    let mut out = vec![format!(
        "{name} needs {} (has {cols}x{rows})",
        grids.join(" or ")
    )];
    out.extend(hint.map(str::to_string));
    out
}

/// Draws the notice over `rect`, in whole cells, on `paper`: the first line in `ink`, the hint
/// in `dim`, centred, each cut to the width it has.
#[allow(clippy::too_many_arguments)]
pub fn notice(
    ui: &Ui,
    rect: Rect,
    name: &str,
    need: &[Fit],
    have: (usize, usize),
    hint: Option<&str>,
    (paper, ink, dim): (Color32, Color32, Color32),
) {
    let g = Cells::fit(ui, rect, dim, paper, dim);
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 0.0, paper);
    let lines = lines(name, need, have, hint);
    let top = g.rows.saturating_sub(lines.len()) / 2;
    for (i, l) in lines.iter().enumerate() {
        let n = l.chars().count().min(g.cols);
        let c = (g.cols - n) / 2;
        g.text_max(&p, c, top + i, l, n, if i == 0 { ink } else { dim });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_grid_fits_when_both_sides_are_enough() {
        let need = Fit { cols: 64, rows: 20 };
        assert!(need.fits((64, 20)));
        assert!(need.fits((100, 40)));
        assert!(!need.fits((63, 40)));
        assert!(!need.fits((100, 19)));
        // Either shape will do.
        let tall = Fit { cols: 40, rows: 30 };
        assert!(fits(&[need, tall], (40, 30)));
        assert!(fits(&[need, tall], (64, 20)));
        assert!(!fits(&[need, tall], (63, 29)));
    }

    #[test]
    fn the_notice_says_what_it_needs_and_has_then_the_hint() {
        let need = Fit { cols: 64, rows: 20 };
        assert_eq!(
            lines("NiGHTCOMM", &[need], (41, 12), Some("Esc f: full screen")),
            vec!["NiGHTCOMM needs 64x20 (has 41x12)", "Esc f: full screen"]
        );
        let tall = Fit { cols: 40, rows: 30 };
        assert_eq!(
            lines("System", &[need, tall], (41, 12), None),
            vec!["System needs 64x20 or 40x30 (has 41x12)"]
        );
    }

    #[test]
    fn check_counts_whole_cells_in_the_monospace_font() {
        let mut h = crate::uitest::Harness::new(800.0, 600.0);
        let mut got = None;
        h.frame(Vec::new(), |ui| {
            let font = egui::TextStyle::Monospace.resolve(ui.style());
            let cw = ui.fonts_mut(|f| f.glyph_width(&font, 'M'));
            let rect = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(cw * 40.5, 1000.0));
            got = Some(check(ui, rect, &[Fit::DEFAULT]));
        });
        let ((cols, rows), ok) = got.unwrap();
        assert_eq!(cols, 40, "half a cell doesn't count");
        assert!(rows > 12 && ok);
    }
}
