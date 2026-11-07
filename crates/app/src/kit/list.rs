//! A scrolling list in cells. The list keeps only where its window starts; the window follows
//! the selection by core's one scroll rule (`ninsei_core::scroll`), and the border says where
//! you are. Drawing the rows is the caller's: it gets the range and draws them in cells.

use std::ops::Range;

use egui::{Color32, Painter};
use ninsei_core::scroll;

use super::Cells;

/// A list's window between frames.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct List {
    /// The first item shown.
    pub top: usize,
    /// Wheel travel not yet a whole row (a trackpad sends a little every frame).
    wheel: f32,
    len: usize,
    sel: Option<usize>,
    rows: usize,
}

impl List {
    /// The items to draw in `rows` rows over `len` of them, kept on `sel`. Remembers them for
    /// [`List::mark`].
    pub fn view(&mut self, len: usize, sel: Option<usize>, rows: usize) -> Range<usize> {
        self.top = scroll::window(len, sel, self.top, rows);
        (self.len, self.sel, self.rows) = (len, sel, rows);
        self.top..(self.top + rows).min(len)
    }

    /// Scrolls by the wheel: `rows` rows up (negative: down), moving only by whole rows and
    /// keeping the rest for the next turn. The next `view` keeps the window in the list.
    pub fn wheel(&mut self, rows: f32) {
        self.wheel += rows;
        let whole = self.wheel.trunc();
        self.wheel -= whole;
        let n = whole.abs() as usize;
        self.top = if whole > 0.0 {
            self.top.saturating_sub(n)
        } else {
            self.top + n
        };
    }

    /// Whether item `i` was in the window last drawn.
    #[cfg(test)]
    pub fn shows(&self, i: usize) -> bool {
        (self.top..self.top + self.rows).contains(&i) && i < self.len
    }

    /// The row on screen (counted from the window's first) that item `i` is drawn on.
    pub fn row(&self, i: usize) -> usize {
        i - self.top
    }

    /// Where you are (`3/40`, `↓ 12 more`), cut into the right of a box's bottom border: the
    /// box is `w` cells wide from column `c`, its bottom border on row `r`. Draw it after
    /// whatever else is drawn on that border; it only goes where it starts at column `from`
    /// or later (what's on the border's left runs to there). Nothing when the whole list
    /// shows, or there's no room.
    #[allow(clippy::too_many_arguments)]
    pub fn mark(
        &self,
        g: &Cells,
        p: &Painter,
        c: usize,
        r: usize,
        w: usize,
        from: usize,
        color: Color32,
    ) {
        let Some(s) = scroll::position(self.len, self.sel, self.top, self.rows) else {
            return;
        };
        let n = s.chars().count();
        if n + 4 <= w && c + w - n - 4 >= from {
            g.title(p, c + w - n - 4, r, &s, color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_view_follows_the_selection_and_remembers_its_place() {
        let mut l = List::default();
        assert_eq!(l.view(40, Some(0), 8), 0..8);
        assert_eq!(l.view(40, Some(10), 8), 3..11, "down past the bottom");
        assert_eq!(l.row(10), 7, "the last row");
        assert_eq!(l.view(40, Some(5), 8), 3..11, "inside: it stays");
        assert_eq!(l.view(40, Some(1), 8), 1..9, "up past the top");
        assert_eq!(l.view(3, Some(2), 8), 0..3, "a short list");
        assert_eq!(l.view(0, None, 8), 0..0);
    }

    #[test]
    fn the_wheel_moves_by_whole_rows_and_keeps_the_rest() {
        let mut l = List::default();
        l.view(40, None, 8);
        // A trackpad's little deltas: nothing until they make a row.
        for _ in 0..4 {
            l.wheel(-0.2);
        }
        assert_eq!(l.top, 0);
        l.wheel(-0.3);
        assert_eq!(l.top, 1, "a row down");
        l.wheel(-2.5);
        assert_eq!(l.top, 3);
        // 0.6 of a row down is still kept: two rows up comes to 1.4, one row.
        l.wheel(2.0);
        assert_eq!(l.top, 2, "and up");
        l.wheel(9.0);
        assert_eq!(l.top, 0, "never above the top");
        l.wheel(-100.0);
        assert_eq!(l.view(40, None, 8), 32..40, "the view keeps it in the list");
    }
}
