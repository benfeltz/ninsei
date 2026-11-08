//! How much room a screen needs. A screen that can't scroll its way out of a small window
//! declares the smallest cell grids it works in (a [`Need`]: one [`Fit`], or a few when it
//! lays out differently by shape, counted in its own cells); the desktop checks the room it
//! has against them before drawing the screen and, when none fits, draws the one notice
//! instead, the way a modern TUI does:
//!
//! ```text
//!   <name> needs 64x20 (has 41x12)
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
    /// Whether a grid of `cols` × `rows` is enough.
    pub fn fits(self, (cols, rows): (usize, usize)) -> bool {
        cols >= self.cols && rows >= self.rows
    }
}

/// What a screen needs: the grids it works in (any one will do), and how it counts the cells
/// a rect holds, in its own font.
#[derive(Clone, Debug)]
pub struct Need {
    /// Fixed for a screen in code, or from data (a game's field).
    pub grids: std::borrow::Cow<'static, [Fit]>,
    pub cells: fn(&Ui, Rect) -> (usize, usize),
    /// The face's height scrolls (its rows are a floor, not a fit): it lays out beside the
    /// scroll bar, so the bar's width isn't its to count.
    pub scrolls: bool,
}

impl Need {
    /// Grids counted in the kit's cells (`Cells`).
    pub const fn cells(grids: &'static [Fit]) -> Need {
        Need {
            grids: std::borrow::Cow::Borrowed(grids),
            cells: cells_in,
            scrolls: false,
        }
    }

    /// Grids worked out at run time (from data), counted in the kit's cells.
    pub fn cells_owned(grids: Vec<Fit>) -> Need {
        Need {
            grids: std::borrow::Cow::Owned(grids),
            cells: cells_in,
            scrolls: false,
        }
    }
}

/// Whether a grid of `cols` × `rows` is enough for any of `need`.
pub fn fits(need: &[Fit], have: (usize, usize)) -> bool {
    need.iter().any(|f| f.fits(have))
}

/// The cells `rect` holds, counted the way `need` counts them, and whether that's enough. A
/// face that scrolls gets `rect` less a scroll bar `bar` wide.
pub fn check(ui: &Ui, rect: Rect, need: &Need, bar: f32) -> ((usize, usize), bool) {
    let room = if need.scrolls {
        rect.with_max_x(rect.max.x - bar)
    } else {
        rect
    };
    let have = (need.cells)(ui, room);
    (have, fits(&need.grids, have))
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

/// `lines` wrapped at spaces to `cols` cells, a parenthetical kept whole (a word longer than
/// the width is cut, the only way it fits).
pub fn wrap(lines: &[String], cols: usize) -> Vec<String> {
    let cols = cols.max(1);
    let mut out = Vec::new();
    for l in lines {
        let mut words: Vec<String> = Vec::new();
        let mut open = false;
        for w in l.split(' ') {
            match words.last_mut() {
                Some(last) if open => {
                    last.push(' ');
                    last.push_str(w);
                }
                _ => words.push(w.to_string()),
            }
            open = (open || w.starts_with('(')) && !w.ends_with(')');
        }
        let mut cur = String::new();
        for w in &words {
            let fits = cur.chars().count() + 1 + w.chars().count() <= cols;
            if cur.is_empty() {
                cur = w.chars().take(cols).collect();
            } else if fits {
                cur.push(' ');
                cur.push_str(w);
            } else {
                out.push(std::mem::take(&mut cur));
                cur = w.chars().take(cols).collect();
            }
        }
        out.push(cur);
    }
    out
}

/// Draws the notice over `rect`, in whole cells, on `paper`: what it needs in `ink`, the hint
/// in `dim`, centred, wrapped to the width it has.
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
    let all = lines(name, need, have, hint);
    let first = wrap(&all[..1], g.cols).len();
    let lines = wrap(&all, g.cols);
    let top = g.rows.saturating_sub(lines.len()) / 2;
    for (i, l) in lines.iter().enumerate() {
        let n = l.chars().count().min(g.cols);
        let c = (g.cols - n) / 2;
        g.text_max(&p, c, top + i, l, n, if i < first { ink } else { dim });
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
            lines("Viewer", &[need], (41, 12), Some("Esc f: full screen")),
            vec!["Viewer needs 64x20 (has 41x12)", "Esc f: full screen"]
        );
        let tall = Fit { cols: 40, rows: 30 };
        assert_eq!(
            lines("Viewer", &[need, tall], (41, 12), None),
            vec!["Viewer needs 64x20 or 40x30 (has 41x12)"]
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
            got = Some(check(
                ui,
                rect,
                &Need::cells(&[Fit { cols: 40, rows: 12 }]),
                10.0,
            ));
        });
        let ((cols, rows), ok) = got.unwrap();
        assert_eq!(cols, 40, "half a cell doesn't count");
        assert!(rows > 12 && ok);
    }

    #[test]
    fn the_notice_wraps_to_a_narrow_tile_instead_of_cutting() {
        let l = lines(
            "System",
            &[Fit { cols: 40, rows: 8 }],
            (29, 13),
            Some("Esc f: full screen"),
        );
        let w = wrap(&l, 27);
        assert_eq!(
            w,
            vec!["System needs 40x8", "(has 29x13)", "Esc f: full screen"]
        );
        assert!(w.iter().all(|l| l.chars().count() <= 27));
        assert_eq!(wrap(&l, 80), l, "a wide tile keeps the lines");
    }
}
