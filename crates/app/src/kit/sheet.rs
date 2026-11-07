//! A sheet: a character grid written first and painted after, for a screen whose height comes
//! from what it holds (it scrolls in its window). Text goes in cells, runs of cells can be
//! clicked, and each clickable run is a widget labelled with its text, so a screen reader (or
//! a test) finds it by what it says. Painting goes through [`Cells`], in the screen's own type.

use egui::{Color32, FontId, Sense, Ui};

use super::Cells;

/// A character grid `cols` wide, as tall as what's written on it. A click on a run gives back
/// its `C`.
pub struct Sheet<C> {
    pub cols: usize,
    rows: usize,
    cells: Vec<(usize, usize, char, Color32, Option<Color32>)>,
    /// Clickable runs: (col, row, len, what it gives back, its name if not its text).
    hits: Vec<(usize, usize, usize, C, Option<String>)>,
}

impl<C: Clone> Sheet<C> {
    pub fn new(cols: usize) -> Self {
        Self {
            cols,
            rows: 0,
            cells: Vec::new(),
            hits: Vec::new(),
        }
    }

    /// Rows written so far.
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Makes the sheet at least `rows` tall (room kept under what's written).
    pub fn grow_to(&mut self, rows: usize) {
        self.rows = self.rows.max(rows);
    }

    /// Writes `other` (a narrower sheet, for text wrapped to a column) onto this one with its
    /// top-left at (col, row): its text, and its clickable runs.
    pub fn stamp(&mut self, other: Sheet<C>, col: usize, row: usize) {
        for (c, r, ch, fg, bg) in other.cells {
            if c + col < self.cols {
                self.cells.push((c + col, r + row, ch, fg, bg));
            }
        }
        for (c, r, len, give, name) in other.hits {
            self.push_hit(c + col, r + row, len, give, name);
        }
        self.rows = self.rows.max(row + other.rows);
    }

    /// Writes `s` at (col, row) in `fg`; returns the column after it. Text that runs past the
    /// sheet's edge ends in `…` in its last cell, the period's mark, never mid-word unmarked.
    pub fn put(&mut self, col: usize, row: usize, s: &str, fg: Color32) -> usize {
        self.write(col, row, s, fg, None)
    }

    /// Writes `s` in `fg` on `bg`: inverse video.
    pub fn inv(&mut self, col: usize, row: usize, s: &str, fg: Color32, bg: Color32) -> usize {
        self.write(col, row, s, fg, Some(bg))
    }

    fn write(
        &mut self,
        col: usize,
        row: usize,
        s: &str,
        fg: Color32,
        bg: Option<Color32>,
    ) -> usize {
        let n = s.chars().count();
        let cut = col + n > self.cols;
        let mut c = col;
        for ch in s.chars() {
            if c < self.cols {
                let ch = if cut && c + 1 == self.cols { '…' } else { ch };
                self.cells.push((c, row, ch, fg, bg));
            }
            c += 1;
        }
        self.rows = self.rows.max(row + 1);
        c
    }

    /// Makes `len` cells from (col, row) clickable, never past the sheet's edge.
    pub fn hit(&mut self, col: usize, row: usize, len: usize, give: C) {
        self.push_hit(col, row, len, give, None);
    }

    /// Like [`Sheet::hit`], named `label` for a screen reader instead of the glyphs it shows.
    pub fn hit_as(&mut self, col: usize, row: usize, len: usize, give: C, label: String) {
        self.push_hit(col, row, len, give, Some(label));
    }

    fn push_hit(&mut self, col: usize, row: usize, len: usize, give: C, name: Option<String>) {
        let len = len.min(self.cols.saturating_sub(col));
        if len > 0 {
            self.hits.push((col, row, len, give, name));
        }
    }

    /// A key hint: key `k` in the first colour, what it `does` in the second, clickable when
    /// `give` is given. Returns where the next hint starts.
    pub fn key(
        &mut self,
        col: usize,
        row: usize,
        k: &str,
        does: &str,
        give: Option<C>,
        (key, what): (Color32, Color32),
    ) -> usize {
        let mid = self.put(col, row, k, key);
        let end = self.put(mid + 1, row, does, what);
        if let Some(give) = give {
            self.hit(col, row, end - col, give);
        }
        end + 2
    }

    /// A line across the sheet on `row`.
    pub fn rule(&mut self, row: usize, color: Color32) {
        let line = "─".repeat(self.cols);
        self.put(0, row, &line, color);
    }

    /// The text in `len` cells from (col, row), its spaces closed up: what a hit there says.
    pub fn text(&self, col: usize, row: usize, len: usize) -> String {
        let mut line = vec![' '; len];
        for &(c, r, ch, _, _) in &self.cells {
            if r == row && (col..col + len).contains(&c) {
                line[c - col] = ch;
            }
        }
        let s: String = line.into_iter().collect();
        s.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    /// Paints the sheet in `ui` in `font` (rows `ch` tall, or the font's line height), and
    /// returns what was clicked. Where clickable runs overlap, the last one added wins.
    pub fn paint(&self, ui: &mut Ui, font: FontId, ch: Option<f32>) -> Vec<C> {
        let (resp, g) = Cells::allocate(
            ui,
            font,
            ch,
            (self.cols, self.rows),
            egui::Vec2::ZERO,
            Sense::hover(),
        );
        let p = ui.painter_at(resp.rect);
        for &(c, r, chr, fg, bg) in &self.cells {
            if let Some(bg) = bg {
                // A hair over the cell, so a run of them joins with no seam.
                p.rect_filled(g.at(c, r, 1, 1).expand(0.3), 0.0, bg);
            }
            if chr != ' ' {
                g.text(&p, c, r, &chr.to_string(), fg);
            }
        }
        let mut given = Vec::new();
        for (i, (hc, hr, len, give, name)) in self.hits.iter().enumerate() {
            let r = g.at(*hc, *hr, (*len).max(1), 1);
            // Salted with this sheet's own room, so two sheets in one Ui never share an id.
            let hit = ui.interact(r, resp.id.with(("sheet-hit", i)), Sense::CLICK);
            let label = name.clone().unwrap_or_else(|| self.text(*hc, *hr, *len));
            hit.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &label));
            if hit.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if hit.clicked() {
                given.push(give.clone());
            }
        }
        given
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::uitest::{Harness, click};

    #[test]
    fn a_sheet_is_as_tall_as_what_it_holds_and_its_hits_say_their_text() {
        let mut s: Sheet<u8> = Sheet::new(20);
        s.put(0, 0, "hello", Color32::WHITE);
        let next = s.key(0, 2, "x", "scrap", Some(7), (Color32::RED, Color32::GRAY));
        assert_eq!(next, 9, "x, a space, scrap, then two spaces");
        s.hit_as(0, 0, 5, 1, "greeting".into());
        s.put(18, 1, "cut here", Color32::WHITE);
        assert_eq!(s.rows(), 3);
        assert_eq!(
            s.text(18, 1, 2),
            "c…",
            "cut at the sheet's edge, and marked"
        );
        let mut col: Sheet<u8> = Sheet::new(4);
        col.put(0, 0, "wrap", Color32::WHITE);
        col.put(0, 1, "ped", Color32::WHITE);
        s.stamp(col, 10, 3);
        assert_eq!(s.text(10, 4, 3), "ped");
        assert_eq!(s.rows(), 5, "as tall as the stamp's foot");
        s.grow_to(7);
        assert_eq!(s.rows(), 7);
        s.grow_to(2);
        assert_eq!(s.rows(), 7, "never shorter");
        // A run is never wider than the sheet, however it was asked for.
        s.hit(15, 5, 10, 9);
        let mut wide: Sheet<u8> = Sheet::new(10);
        wide.hit(0, 0, 10, 8);
        s.stamp(wide, 14, 6);
        assert!(s.hits.iter().all(|(c, _, len, _, _)| c + len <= s.cols));
        assert_eq!(
            s.hits.iter().filter(|h| h.3 == 8).map(|h| h.2).next(),
            Some(6)
        );

        let mut h = Harness::new(400.0, 200.0);
        let mut got = Vec::new();
        let mut draw = |h: &mut Harness, events| {
            h.frame(events, |ui| {
                got = s.paint(ui, FontId::monospace(13.0), None)
            });
        };
        draw(&mut h, Vec::new());
        let scrap = h
            .find("x scrap")
            .expect("the key hint is a widget by its text");
        assert!(
            h.find("greeting").is_some(),
            "a hit named for a screen reader"
        );
        draw(&mut h, click(scrap.center()));
        assert_eq!(got, vec![7]);
    }

    #[test]
    fn two_sheets_in_one_ui_keep_their_clicks_apart() {
        let (mut a, mut b): (Sheet<char>, Sheet<char>) = (Sheet::new(10), Sheet::new(10));
        a.put(0, 0, "alpha", Color32::WHITE);
        a.hit(0, 0, 5, 'a');
        b.put(0, 0, "beta", Color32::WHITE);
        b.hit(0, 0, 4, 'b');
        let mut h = Harness::new(400.0, 200.0);
        let mut got = (Vec::new(), Vec::new());
        let mut draw = |h: &mut Harness, events| {
            h.frame(events, |ui| {
                got = (
                    a.paint(ui, FontId::monospace(13.0), None),
                    b.paint(ui, FontId::monospace(13.0), None),
                );
            });
        };
        draw(&mut h, Vec::new());
        let beta = h.find("beta").expect("the second sheet's run");
        draw(&mut h, click(beta.center()));
        assert_eq!(got, (vec![], vec!['b']), "only the second sheet heard it");
    }
}
