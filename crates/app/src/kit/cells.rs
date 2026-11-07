//! A character-cell canvas for text-mode screens. Real
//! text-mode software lives on a grid of cells, so everything here is placed by column and
//! row: whole cells only, the leftover pixels split around the edges, every glyph drawn in its
//! own cell so a fallback glyph can never push a column out of line. Borders are lines through
//! the cell centres, the way box-drawing characters join up.

use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, Ui, pos2, vec2};

/// Extra line spacing on top of the font's height, as a share of it.
const LEADING: f32 = 0.18;
/// Corner radius of a box, as a share of a cell's width (a modern TUI's rounded corners).
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
    /// Turbo Vision's look instead of rounded boxes: square corners, titles set in the border with a
    /// space either side instead of cut into it.
    pub vision: bool,
}

/// `ui`'s monospace font and the size of one cell in it.
fn metrics(ui: &Ui) -> (FontId, f32, f32) {
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let cw = ui.fonts_mut(|f| f.glyph_width(&font, 'M')).max(1.0);
    let ch = (font.size * (1.0 + LEADING)).round().max(1.0);
    (font, cw, ch)
}

/// Whole cells of `cw` × `ch` that fit in `area`.
fn grid(cw: f32, ch: f32, area: Rect) -> (usize, usize) {
    (
        (area.width() / cw).floor().max(0.0) as usize,
        (area.height() / ch).floor().max(0.0) as usize,
    )
}

/// How many whole cells of `ui`'s monospace font fit in `area`: columns, rows.
pub fn cells_in(ui: &Ui, area: Rect) -> (usize, usize) {
    let (_, cw, ch) = metrics(ui);
    grid(cw, ch, area)
}

impl Cells {
    /// The biggest whole grid that fits in `area`, centred, in `ui`'s monospace font, drawing
    /// borders in `border` on `paper` and unlit meter cells in `empty`.
    pub fn fit(ui: &Ui, area: Rect, border: Color32, paper: Color32, empty: Color32) -> Cells {
        let (font, cw, ch) = metrics(ui);
        let (cols, rows) = grid(cw, ch, area);
        let (cols, rows) = (cols.max(1), rows.max(1));
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
            vision: false,
        }
    }

    /// A grid of `cols` × `rows` cells from `min`, in a face's own type: `font`, with rows
    /// `ch` tall (the font's own line height when `None`). For a screen whose cells aren't
    /// the app's monospace ones, like an 80x24 terminal at its own size. Geometry only: a face
    /// that draws boxes or meters sets `border`, `paper` and `empty` (transparent until then).
    pub fn sized(
        ui: &Ui,
        min: Pos2,
        font: FontId,
        ch: Option<f32>,
        (cols, rows): (usize, usize),
    ) -> Cells {
        let cw = ui.fonts_mut(|f| f.glyph_width(&font, 'M')).max(1.0);
        let ch = ch
            .unwrap_or_else(|| ui.fonts_mut(|f| f.row_height(&font)))
            .max(1.0);
        Cells {
            rect: Rect::from_min_size(min, vec2(cols as f32 * cw, rows as f32 * ch)),
            cw,
            ch,
            cols,
            rows,
            font,
            border: Color32::TRANSPARENT,
            paper: Color32::TRANSPARENT,
            empty: Color32::TRANSPARENT,
            vision: false,
        }
    }

    /// A grid of exactly `cols` × `rows` cells from `min` (a box sized in cells, rather than
    /// cells fitted to an area).
    pub fn grid(
        ui: &Ui,
        min: Pos2,
        (cols, rows): (usize, usize),
        border: Color32,
        paper: Color32,
        empty: Color32,
    ) -> Cells {
        let egui::Vec2 { x: cw, y: ch } = Cells::cell_size(ui.ctx());
        Cells {
            rect: Rect::from_min_size(min, vec2(cols as f32 * cw, rows as f32 * ch)),
            cw,
            ch,
            cols,
            rows,
            font: egui::TextStyle::Monospace.resolve(ui.style()),
            border,
            paper,
            empty,
            vision: false,
        }
    }

    /// Allocates a face's grid in `ui`: `cols` × `rows` cells in `font` (rows `ch` tall, or the
    /// font's line height) with `pad` around them, sensing `sense`. Measures once, so the room
    /// taken and the cells drawn in it can't disagree. Returns the response for the whole
    /// room (its rect includes the padding) and the cells inside it.
    pub fn allocate(
        ui: &mut Ui,
        font: FontId,
        ch: Option<f32>,
        dims: (usize, usize),
        pad: egui::Vec2,
        sense: egui::Sense,
    ) -> (egui::Response, Cells) {
        let probe = Cells::sized(ui, Pos2::ZERO, font, ch, dims);
        let (rect, resp) = ui.allocate_exact_size(probe.rect.size() + pad * 2.0, sense);
        (resp, probe.at_pos(rect.min + pad))
    }

    /// The same grid moved so it starts at `min`.
    pub fn at_pos(mut self, min: Pos2) -> Cells {
        self.rect = Rect::from_min_size(min, self.rect.size());
        self
    }

    /// A block element (`█▀▄▌▐▪░▒▓`) at (`c`, `r`), painted as a solid part of the cell so
    /// block art joins up with no font gaps. Returns false (and paints nothing) for any other
    /// character.
    pub fn block(&self, p: &Painter, c: usize, r: usize, ch: char, color: Color32) -> bool {
        let cell = self.at(c, r, 1, 1);
        let part = |x0: f32, y0: f32, x1: f32, y1: f32| {
            let r = Rect::from_min_max(
                pos2(
                    cell.left() + cell.width() * x0,
                    cell.top() + cell.height() * y0,
                ),
                pos2(
                    cell.left() + cell.width() * x1,
                    cell.top() + cell.height() * y1,
                ),
            );
            // Snapped outward a hair so neighbouring cells meet with no seam.
            p.rect_filled(r.expand(0.25), 0.0, color);
        };
        match ch {
            '█' => part(0.0, 0.0, 1.0, 1.0),
            '▀' => part(0.0, 0.0, 1.0, 0.5),
            '▄' => part(0.0, 0.5, 1.0, 1.0),
            '▌' => part(0.0, 0.0, 0.5, 1.0),
            '▐' => part(0.5, 0.0, 1.0, 1.0),
            '▪' => part(0.25, 0.3, 0.75, 0.7),
            '░' | '▒' | '▓' => {
                let a = match ch {
                    '░' => 0.25,
                    '▒' => 0.5,
                    _ => 0.75,
                };
                p.rect_filled(cell, 0.0, color.gamma_multiply(a));
            }
            _ => return false,
        }
        true
    }

    /// One cell's size in the monospace font: a grid of `cols` × `rows` of these fits
    /// exactly.
    pub fn cell_size(ctx: &egui::Context) -> egui::Vec2 {
        let font = egui::TextStyle::Monospace.resolve(&ctx.global_style());
        let cw = ctx.fonts_mut(|f| f.glyph_width(&font, 'M')).max(1.0);
        vec2(cw, (font.size * (1.0 + LEADING)).round().max(1.0))
    }

    /// The pixel rect of `w` × `h` cells from column `c`, row `r`.
    pub fn at(&self, c: usize, r: usize, w: usize, h: usize) -> Rect {
        Rect::from_min_size(
            self.rect.min + vec2(c as f32 * self.cw, r as f32 * self.ch),
            vec2(w as f32 * self.cw, h as f32 * self.ch),
        )
    }

    /// The cell under `pos`, if it's on the grid: the inverse of [`Cells::at`].
    pub fn cell_at(&self, pos: Pos2) -> Option<(usize, usize)> {
        if !self.rect.contains(pos) {
            return None;
        }
        let c = ((pos.x - self.rect.left()) / self.cw).floor() as usize;
        let r = ((pos.y - self.rect.top()) / self.ch).floor() as usize;
        (c < self.cols && r < self.rows).then_some((c, r))
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

    /// A run of ordinary glyphs from cell (`c`, `r`) as one piece of text: cheaper than a glyph
    /// a cell, for a screen that repaints every frame. Only for text the font holds in its
    /// monospace advance (one cell a glyph); [`Cells::text`] keeps each glyph in its own cell
    /// whatever it is. Cut at the grid's edge.
    pub fn run(&self, p: &Painter, c: usize, r: usize, s: &str, color: Color32) {
        let room = self.cols.saturating_sub(c);
        if room == 0 || s.is_empty() {
            return;
        }
        let s: String = s.chars().take(room).collect();
        p.text(
            self.at(c, r, 1, 1).left_center(),
            Align2::LEFT_CENTER,
            s,
            self.font.clone(),
            color,
        );
    }

    /// Text ending at column `end` (exclusive), for numbers in right-aligned columns.
    pub fn text_right(&self, p: &Painter, end: usize, r: usize, s: &str, color: Color32) {
        let n = s.chars().count();
        self.text(p, end.saturating_sub(n), r, s, color);
    }

    /// Inverse video, the period's selection: `w` cells filled with `bg`, the text on them in
    /// `fg`, cut at the row's end.
    #[allow(clippy::too_many_arguments)]
    pub fn inverse(
        &self,
        p: &Painter,
        c: usize,
        r: usize,
        w: usize,
        s: &str,
        fg: Color32,
        bg: Color32,
    ) {
        self.fill(p, c, r, w, 1, bg);
        if !s.is_empty() {
            self.text_max(p, c, r, s, w, fg);
        }
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
        let radius = if self.vision {
            0.0
        } else {
            (self.cw * CORNER).min(rect.width() / 2.0)
        };
        p.rect_stroke(
            rect,
            radius,
            Stroke::new(1.0, color),
            egui::StrokeKind::Middle,
        );
    }

    /// A box with `title` (if any) cut into its top border, two cells in.
    #[allow(clippy::too_many_arguments)]
    pub fn boxed(
        &self,
        p: &Painter,
        c: usize,
        r: usize,
        w: usize,
        h: usize,
        title: &str,
        color: Color32,
    ) {
        self.frame(p, c, r, w, h);
        if !title.is_empty() {
            self.title(p, c + 2, r, title, color);
        }
    }

    /// A title cut into a border at (`c`, `r`), modern-TUI style: `┐title┌`, the border under it
    /// painted out. Returns the cells used.
    pub fn title(&self, p: &Painter, c: usize, r: usize, s: &str, color: Color32) -> usize {
        let (border, bg) = (self.border, self.paper);
        let n = s.chars().count();
        if c + n + 2 > self.cols {
            return 0;
        }
        self.fill(p, c, r, n + 2, 1, bg);
        if self.vision {
            self.text(p, c + 1, r, s, color);
            return n + 2;
        }
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

    /// A Turbo Vision window frame: a double line, square corners.
    pub fn frame_double(&self, p: &Painter, c: usize, r: usize, w: usize, h: usize) {
        if w < 2 || h < 2 {
            return;
        }
        let (a, b) = (self.mid(c, r), self.mid(c + w - 1, r + h - 1));
        let gap = (self.cw * 0.18).max(1.5);
        let stroke = Stroke::new(1.0, self.border);
        for d in [-gap, gap] {
            let rect = Rect::from_min_max(a, b).expand(d);
            p.rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Middle);
        }
    }

    /// A meter standing up: `h` cells in column `c` rising from row `bottom`, `on` of
    /// them lit along the `ramp` from the bottom, the rest drawn as empty cells, as
    /// [`Cells::meter`] does lying down.
    pub fn meter_up(
        &self,
        p: &Painter,
        c: usize,
        bottom: usize,
        h: usize,
        on: usize,
        ramp: &[Color32],
    ) {
        for i in 0..h.min(bottom + 1) {
            let cell = self
                .at(c, bottom - i, 1, 1)
                .shrink2(vec2(self.cw * 0.12, self.ch * 0.22));
            if i < on {
                let k = if h <= 1 {
                    ramp.len() - 1
                } else {
                    i * (ramp.len() - 1) / (h - 1)
                };
                p.rect_filled(cell, 0.0, ramp[k]);
            } else {
                p.rect_stroke(
                    cell,
                    0.0,
                    Stroke::new(1.0, self.empty),
                    egui::StrokeKind::Inside,
                );
            }
        }
    }

    /// A meter: `w` cells, `on` of them lit along the `ramp` (faint → bright), the rest
    /// drawn as empty cells.
    pub fn meter(&self, p: &Painter, c: usize, r: usize, w: usize, on: usize, ramp: &[Color32]) {
        self.meter_in(p, c, r, w, on, ramp, self.empty);
    }

    /// A meter whose unlit cells are outlined in `empty`: on an inverse row, lit and unlit
    /// cells are both drawn in the row's text colour so the meter stays readable.
    #[allow(clippy::too_many_arguments)]
    pub fn meter_in(
        &self,
        p: &Painter,
        c: usize,
        r: usize,
        w: usize,
        on: usize,
        ramp: &[Color32],
        empty: Color32,
    ) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_at_finds_the_cell_at_drew() {
        let mut h = crate::uitest::Harness::new(400.0, 300.0);
        h.frame(Vec::new(), |ui| {
            let g = Cells::sized(
                ui,
                pos2(10.0, 20.0),
                FontId::monospace(15.0),
                Some(22.0),
                (12, 4),
            );
            for (c, r) in [(0, 0), (11, 3), (5, 2)] {
                let cell = g.at(c, r, 1, 1);
                assert_eq!(g.cell_at(cell.center()), Some((c, r)));
                assert_eq!(g.cell_at(cell.min), Some((c, r)), "its top-left is its own");
            }
            assert_eq!(g.cell_at(pos2(5.0, 25.0)), None, "left of the grid");
            assert_eq!(g.cell_at(g.rect.right_bottom() + vec2(1.0, 1.0)), None);
        });
    }
}
