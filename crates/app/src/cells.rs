//! A character-cell canvas for the text-mode tools (NiGHTCOMM and the ones after it). Real
//! text-mode software lives on a grid of cells, so everything here is placed by column and
//! row: whole cells only, the leftover pixels split around the edges, every glyph drawn in its
//! own cell so a fallback glyph can never push a column out of line. Borders are lines through
//! the cell centres, the way box-drawing characters join up.

use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, Ui, pos2, vec2};

/// Extra line spacing on top of the font's height, as a share of it.
const LEADING: f32 = 0.18;
/// Corner radius of a box, as a share of a cell's width (btop's rounded corners).
const CORNER: f32 = 0.5;

/// A grid of `cols` × `rows` cells laid over `rect`.
#[derive(Clone, Debug)]
pub struct Cells {
    pub rect: Rect,
    pub cw: f32,
    pub ch: f32,
    pub cols: usize,
    pub rows: usize,
    font: FontId,
    /// Border lines.
    pub border: Color32,
    /// The paper under titles cut into a border.
    pub paper: Color32,
    /// The outline of an unlit meter cell.
    pub empty: Color32,
}

impl Cells {
    /// The biggest whole grid that fits in `area`, centred, in `ui`'s monospace font, drawing
    /// borders in `border` on `paper` and unlit meter cells in `empty`.
    pub fn fit(ui: &Ui, area: Rect, border: Color32, paper: Color32, empty: Color32) -> Cells {
        let font = egui::TextStyle::Monospace.resolve(ui.style());
        let cw = ui.fonts_mut(|f| f.glyph_width(&font, 'M')).max(1.0);
        let ch = (font.size * (1.0 + LEADING)).round().max(1.0);
        let cols = (area.width() / cw).floor().max(1.0) as usize;
        let rows = (area.height() / ch).floor().max(1.0) as usize;
        let size = vec2(cols as f32 * cw, rows as f32 * ch);
        let min = pos2(
            (area.left() + (area.width() - size.x) / 2.0).round(),
            (area.top() + (area.height() - size.y) / 2.0).round(),
        );
        Cells {
            rect: Rect::from_min_size(min, size),
            cw,
            ch,
            cols,
            rows,
            font,
            border,
            paper,
            empty,
        }
    }

    /// The pixel rect of `w` × `h` cells from column `c`, row `r`.
    pub fn at(&self, c: usize, r: usize, w: usize, h: usize) -> Rect {
        Rect::from_min_size(
            self.rect.min + vec2(c as f32 * self.cw, r as f32 * self.ch),
            vec2(w as f32 * self.cw, h as f32 * self.ch),
        )
    }

    /// The centre of a cell: where border lines run.
    fn mid(&self, c: usize, r: usize) -> Pos2 {
        self.rect.min + vec2((c as f32 + 0.5) * self.cw, (r as f32 + 0.5) * self.ch)
    }

    /// Text from cell (`c`, `r`), one glyph per cell, cut at `max` cells (with `…` when it
    /// had to cut). Returns the cells used.
    pub fn text(&self, p: &Painter, c: usize, r: usize, s: &str, color: Color32) -> usize {
        self.text_max(p, c, r, s, usize::MAX, color)
    }

    /// Like [`Cells::text`], cut on purpose at `max` cells.
    pub fn text_max(
        &self,
        p: &Painter,
        c: usize,
        r: usize,
        s: &str,
        max: usize,
        color: Color32,
    ) -> usize {
        let max = max.min(self.cols.saturating_sub(c));
        let n = s.chars().count();
        let cut = n > max;
        let mut used = 0;
        for (i, ch) in s.chars().enumerate() {
            if i >= max {
                break;
            }
            let ch = if cut && i + 1 == max { '…' } else { ch };
            if ch != ' ' {
                p.text(
                    self.at(c + i, r, 1, 1).center(),
                    Align2::CENTER_CENTER,
                    ch,
                    self.font.clone(),
                    color,
                );
            }
            used = i + 1;
        }
        used
    }

    /// Text ending at column `end` (exclusive), for numbers in right-aligned columns.
    pub fn text_right(&self, p: &Painter, end: usize, r: usize, s: &str, color: Color32) {
        let n = s.chars().count();
        self.text(p, end.saturating_sub(n), r, s, color);
    }

    /// Fills `w` × `h` cells.
    pub fn fill(&self, p: &Painter, c: usize, r: usize, w: usize, h: usize, color: Color32) {
        p.rect_filled(self.at(c, r, w, h), 0.0, color);
    }

    /// A box with rounded corners, its border through the centres of its outer cells.
    pub fn frame(&self, p: &Painter, c: usize, r: usize, w: usize, h: usize) {
        let color = self.border;
        if w < 2 || h < 2 {
            return;
        }
        let (a, b) = (self.mid(c, r), self.mid(c + w - 1, r + h - 1));
        let rect = Rect::from_min_max(a, b);
        let radius = (self.cw * CORNER).min(rect.width() / 2.0);
        p.rect_stroke(
            rect,
            radius,
            Stroke::new(1.0, color),
            egui::StrokeKind::Middle,
        );
    }

    /// A title cut into a border at (`c`, `r`), btop style: `┐title┌`, the border under it
    /// painted out. Returns the cells used.
    pub fn title(&self, p: &Painter, c: usize, r: usize, s: &str, color: Color32) -> usize {
        let (border, bg) = (self.border, self.paper);
        let n = s.chars().count();
        if c + n + 2 > self.cols {
            return 0;
        }
        self.fill(p, c, r, n + 2, 1, bg);
        // The ┐ and ┌ either side: the border turns down into the title's row.
        let half = self.ch * 0.5;
        let left = self.mid(c, r);
        let right = self.mid(c + n + 1, r);
        let stroke = Stroke::new(1.0, border);
        p.line_segment([left - vec2(self.cw * 0.5, 0.0), left], stroke);
        p.line_segment([left, left + vec2(0.0, half * 0.6)], stroke);
        p.line_segment([right, right + vec2(self.cw * 0.5, 0.0)], stroke);
        p.line_segment([right, right + vec2(0.0, half * 0.6)], stroke);
        self.text(p, c + 1, r, s, color);
        n + 2
    }

    /// A btop meter: `w` cells, `on` of them lit along the `ramp` (faint → bright), the rest
    /// drawn as empty cells.
    pub fn meter(&self, p: &Painter, c: usize, r: usize, w: usize, on: usize, ramp: &[Color32]) {
        let empty = self.empty;
        for i in 0..w {
            let cell = self
                .at(c + i, r, 1, 1)
                .shrink2(vec2(self.cw * 0.12, self.ch * 0.22));
            if i < on {
                let k = if w <= 1 {
                    ramp.len() - 1
                } else {
                    i * (ramp.len() - 1) / (w - 1)
                };
                p.rect_filled(cell, 0.0, ramp[k]);
            } else {
                // Unlit: an outline, so it reads apart from a lit cell on any tube.
                p.rect_stroke(cell, 0.0, Stroke::new(1.0, empty), egui::StrokeKind::Inside);
            }
        }
    }
}
