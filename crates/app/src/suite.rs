//! The Hosaka's chrome: a 90s productivity suite. Every pane is a document window (a filled
//! title bar, a double border, a drop shadow, its number in the corner) on a plain desk.
//! deck.sys is a form with report charts, warez/ a spreadsheet, target.ice an incident report,
//! the terminal a ledger and todo a table of work orders. An F-key legend sits under the menu bar
//! and dialogs pop up for the moments worth filing. Calm by design: no glow, no spin. Every
//! colour is a theme role: title bars `panel`, shadows `bg`, selections in reverse video. The
//! rules (sorting, formulas, axes, dialogs) are `ninsei_core::suite`.

use egui::{
    Align2, Color32, FontFamily, FontId, Key, Painter, Rect, RichText, ScrollArea, Sense, Stroke,
    StrokeKind, Ui, UiBuilder, pos2, vec2,
};
use ninsei_core::listing::Gutter;
use ninsei_core::suite::{self as rules, FKey, Legend, Notice, SheetCol, Watch};
use ninsei_core::{Action, Command, ContractStatus, Effect, Game, LogKind, Phase, tuning};

use crate::chrome::{ChromeView, FrameCx, MenuPart, Menus, act};
use crate::kit::cells::cut;
use crate::kit::listing::{Bind, Listing, number_w};
use crate::kit::{Cells, Sheet};
use crate::panes::{Panes, Request, Tab};
use crate::theme::Palette;

/// Sizes for the suite, in one place. Display numbers, not balance.
pub mod size {
    pub const MONO: f32 = 12.0;
    pub const SMALL: f32 = 10.0;
    pub const ROW_H: f32 = 17.0;
    /// A pop-out dialog's title bar (the Sound dialog's).
    pub const TITLE_H: f32 = 18.0;
    /// How far a window's shadow falls.
    pub const SHADOW: f32 = 5.0;
    /// Report charts take this share of a window's height, within these bounds.
    pub const CHART_SHARE: f32 = 0.3;
    pub const CHART_MIN: f32 = 56.0;
    pub const CHART_MAX: f32 = 150.0;
    /// Widest a column chart's group of columns gets, so a short fight isn't one wall.
    pub const COL_SLOT_MAX: f32 = 56.0;
    /// Narrowest a form shows its notes column at.
    pub const FORM_NOTES_W: f32 = 440.0;
    pub const DIALOG_W: f32 = 420.0;
    /// The Sound dialog's width.
    pub const SOUND_W: f32 = 300.0;
}

/// What the suite remembers between frames: the spreadsheet's cursor (by program uid, since the
/// HDD reshuffles), its sort, and the dialogs waiting to be dismissed.
#[derive(Default)]
pub struct State {
    pub sel: Option<u32>,
    pub col: SheetCol,
    pub sort: SheetCol,
    pub desc: bool,
    pub watch: Watch,
    pub dialogs: Vec<Notice>,
}

impl State {
    /// The sheet's rows in order.
    fn rows(&self, g: &Game) -> Vec<usize> {
        rules::sheet_rows(&g.hdd, self.sort, !self.desc)
    }

    /// The slot under the cursor, or the sheet's first row.
    pub fn picked(&self, g: &Game) -> Option<usize> {
        self.sel
            .and_then(|uid| g.hdd.iter().position(|s| s.uid == uid))
            .or_else(|| self.rows(g).first().copied())
    }

    fn select(&mut self, g: &Game, slot: usize) {
        self.sel = g.hdd.get(slot).map(|s| s.uid);
    }
}

fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

fn t(s: impl Into<String>, c: Color32) -> RichText {
    RichText::new(s).font(mono(size::MONO)).color(c)
}

fn small(s: impl Into<String>, c: Color32) -> RichText {
    RichText::new(s).font(mono(size::SMALL)).color(c)
}

/// A document window filling the pane, on the suite's cells: a shadow, a double border with
/// the close box `[■]` and the window's number `[n]` set in its top edge, Turbo Vision style,
/// and `body` inside. `heading` is the document's own title row (a form's or a report's
/// name); empty when the tab over the window already names it, since a window doesn't repeat
/// its tab.
pub fn window(ui: &mut Ui, p: &Palette, num: usize, heading: &str, body: impl FnOnce(&mut Ui)) {
    window_with(ui, p, num, heading, "", false, body);
}

/// [`window`] with a `note` set in its bottom edge (the Module window's budgets), its double
/// line `bright` while it's the active window (`lit`), as Turbo Vision marks the one you're in.
pub fn window_with(
    ui: &mut Ui,
    p: &Palette,
    num: usize,
    heading: &str,
    note: &str,
    lit: bool,
    body: impl FnOnce(&mut Ui),
) {
    let full = ui.available_rect_before_wrap();
    let painter = ui.painter().clone();
    painter.rect_filled(full, 0.0, p.panel);
    // Room for the window and its shadow (a drop of paint, not a cell of layout).
    let area = Rect::from_min_max(
        full.min + vec2(4.0, 4.0),
        full.max - vec2(4.0 + size::SHADOW, 4.0 + size::SHADOW),
    );
    let mut g = Cells::fit_font(ui, area, mono(size::MONO), Some(size::ROW_H));
    g.border = if lit { p.bright } else { p.fg };
    painter.rect_filled(
        g.rect.translate(vec2(size::SHADOW, size::SHADOW)),
        0.0,
        p.bg,
    );
    painter.rect_filled(g.rect, 0.0, p.bg);
    g.frame_double(&painter, 0, 0, g.cols, g.rows);
    // Too small for the frame's furniture, the body still gets the window's room: it may hold
    // the only command line on screen.
    let inner = if g.cols < 10 || g.rows < 4 {
        area
    } else {
        // [■] and [n] in the top edge, the border broken around them.
        g.paper = p.bg;
        g.vision = true;
        g.title(&painter, 1, 0, "[■]", p.dim);
        let number = format!("[{num}]");
        g.title(
            &painter,
            g.cols - number.chars().count() - 3,
            0,
            &number,
            p.dim,
        );
        // The note in the bottom edge, right, cut to the room the edge has.
        let note = cut(note, g.cols.saturating_sub(8));
        let n = note.chars().count();
        if n > 0 {
            g.title(&painter, g.cols - n - 4, g.rows - 1, &note, p.dim);
        }
        // The document's heading, if it has one of its own, on the first row.
        let top = if heading.is_empty() || g.rows < 5 {
            1
        } else {
            let w = g.cols - 2;
            let name = cut(heading, w.saturating_sub(2));
            let n = name.chars().count();
            g.fill(&painter, 1, 1, w, 1, p.panel);
            g.text(&painter, 1 + (w - n) / 2, 1, &name, p.bright);
            2
        };
        // The body: under the heading (or the top edge), inside the double line (it runs
        // through the outer cells' centres).
        let edge = g.at(0, g.rows - 1, g.cols, 1).center().y;
        Rect::from_min_max(
            g.at(1, top, 1, 1).min + vec2(2.0, 2.0),
            pos2(g.at(g.cols - 1, 0, 1, 1).center().x - 4.0, edge - 4.0),
        )
    };
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(inner)
            .layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            ui.set_clip_rect(inner.intersect(ui.clip_rect()));
            // Solid scroll bars, always there, like any document window.
            // The thumb in the text colour on a `panel` track: visible on the window's `bg`.
            ui.spacing_mut().scroll = crate::theme::visible_scroll();
            ui.visuals_mut().extreme_bg_color = p.panel;
            body(ui);
        },
    );
    ui.allocate_rect(full, Sense::hover());
}

/// How many of the suite's cells fit across `ui`.
fn cols_across(ui: &Ui) -> usize {
    let area = Rect::from_min_size(ui.cursor().min, vec2(ui.available_width(), size::ROW_H));
    Cells::fit_font(ui, area, mono(size::MONO), Some(size::ROW_H)).cols
}

/// A scrolling document body with its scroll bar always shown.
fn page(ui: &mut Ui, id: &str, body: impl FnOnce(&mut Ui)) {
    ScrollArea::vertical()
        .id_salt(id)
        .auto_shrink([false, false])
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
        .show(ui, body);
}

/// A section heading with a rule under it.
fn heading(ui: &mut Ui, p: &Palette, text: &str) {
    ui.add_space(6.0);
    ui.label(t(text, p.bright));
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 3.0), Sense::hover());
    ui.painter()
        .line_segment([r.left_center(), r.right_center()], Stroke::new(1.0, p.dim));
}

/// How tall a report chart may be in a window `h` high.
fn chart_h(h: f32) -> f32 {
    (h * size::CHART_SHARE).clamp(size::CHART_MIN, size::CHART_MAX)
}

/// The axes and gridlines of a report chart: y ticks at quarters of `max` on the left, and a
/// row on top for the legend if there is one. Returns the plot area.
fn axes(
    painter: &Painter,
    rect: Rect,
    p: &Palette,
    max: f32,
    unit: &str,
    legend: &[(&str, bool)],
) -> Rect {
    let top = if legend.is_empty() { 4.0 } else { 16.0 };
    let plot = Rect::from_min_max(rect.min + vec2(34.0, top), rect.max - vec2(6.0, 14.0));
    if !legend.is_empty() {
        legend_line(
            painter,
            pos2(plot.left() + 12.0, rect.top() + 7.0),
            p,
            legend,
        );
    }
    for k in 0..=4 {
        let v = max * k as f32 / 4.0;
        let y = plot.bottom() - plot.height() * k as f32 / 4.0;
        painter.line_segment(
            [pos2(plot.left(), y), pos2(plot.right(), y)],
            Stroke::new(1.0, if k == 0 { p.dim } else { p.faint }),
        );
        painter.text(
            pos2(plot.left() - 4.0, y),
            Align2::RIGHT_CENTER,
            if max < 10.0 {
                format!("{v:.1}{unit}")
            } else {
                format!("{v:.0}{unit}")
            },
            mono(size::SMALL),
            p.dim,
        );
    }
    painter.line_segment(
        [plot.left_top(), plot.left_bottom()],
        Stroke::new(1.0, p.dim),
    );
    plot
}

/// X labels under a plot, thinned so they don't collide.
fn x_labels(
    painter: &Painter,
    plot: Rect,
    p: &Palette,
    n: usize,
    slot: f32,
    label: impl Fn(usize) -> String,
) {
    if n == 0 {
        return;
    }
    let every = ((28.0 / slot).ceil() as usize).max(1);
    for i in (0..n).step_by(every) {
        painter.text(
            pos2(plot.left() + slot * (i as f32 + 0.5), plot.bottom() + 2.0),
            Align2::CENTER_TOP,
            label(i),
            mono(size::SMALL),
            p.dim,
        );
    }
}

/// A legend line: each entry is a swatch and its name.
fn legend_line(painter: &Painter, at: egui::Pos2, p: &Palette, items: &[(&str, bool)]) {
    let mut x = at.x;
    for (name, solid) in items {
        let sw = Rect::from_min_size(pos2(x - 10.0, at.y - 4.0), vec2(8.0, 8.0));
        if *solid {
            painter.rect_filled(sw, 0.0, p.dim);
        } else {
            painter.rect_stroke(sw, 0.0, Stroke::new(1.0, p.dim), StrokeKind::Inside);
        }
        let g = painter.layout_no_wrap(name.to_string(), mono(size::SMALL), p.dim);
        let w = g.size().x;
        painter.galley(pos2(x, at.y - g.size().y / 2.0), g, p.dim);
        x += w + 22.0;
    }
}

/// A horizontal bar chart: one labelled bar per row against a 0-100% axis with gridlines.
fn bar_chart(ui: &mut Ui, p: &Palette, rows: &[(&str, u32, u32, bool)]) {
    let h = rows.len() as f32 * 15.0 + 16.0;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
    let painter = ui.painter_at(rect);
    let plot = Rect::from_min_max(rect.min + vec2(52.0, 2.0), rect.max - vec2(40.0, 14.0));
    for (k, v) in rules::ticks(100, 4).into_iter().enumerate() {
        let x = plot.left() + plot.width() * k as f32 / 4.0;
        painter.line_segment(
            [pos2(x, plot.top()), pos2(x, plot.bottom())],
            Stroke::new(1.0, if k == 0 { p.dim } else { p.faint }),
        );
        painter.text(
            pos2(x, plot.bottom() + 2.0),
            Align2::CENTER_TOP,
            format!("{v}%"),
            mono(size::SMALL),
            p.dim,
        );
    }
    for (i, (label, v, max, hot)) in rows.iter().enumerate() {
        let y = plot.top() + i as f32 * 15.0 + 7.0;
        painter.text(
            pos2(rect.left(), y),
            Align2::LEFT_CENTER,
            *label,
            mono(size::SMALL),
            p.dim,
        );
        let frac = if *max == 0 {
            0.0
        } else {
            (*v as f32 / *max as f32).clamp(0.0, 1.0)
        };
        let bar = Rect::from_min_size(pos2(plot.left(), y - 4.0), vec2(plot.width() * frac, 8.0));
        painter.rect_filled(bar, 0.0, if *hot { p.bright } else { p.fg });
        painter.text(
            pos2(plot.right() + 4.0, y),
            Align2::LEFT_CENTER,
            format!("{:.0}%", frac * 100.0),
            mono(size::SMALL),
            if *hot { p.bright } else { p.fg },
        );
    }
}

/// A line chart of `values` against `max`, with dashed threshold lines at `marks`.
fn line_chart(ui: &mut Ui, p: &Palette, h: f32, values: &[u32], max: u32, marks: &[u32]) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
    let painter = ui.painter_at(rect);
    let plot = axes(
        &painter,
        rect,
        p,
        max as f32,
        "%",
        &[("TRACE", true), ("LIMITS", false)],
    );
    let n = values.len().max(1);
    let slot = plot.width() / n as f32;
    let at = |i: usize, v: u32| {
        pos2(
            plot.left() + slot * (i as f32 + 0.5),
            plot.bottom() - plot.height() * (v.min(max) as f32 / max.max(1) as f32),
        )
    };
    for &m in marks {
        let y = at(0, m).y;
        painter.extend(egui::Shape::dashed_line(
            &[pos2(plot.left(), y), pos2(plot.right(), y)],
            Stroke::new(1.0, p.fg),
            4.0,
            3.0,
        ));
    }
    let pts: Vec<egui::Pos2> = values.iter().enumerate().map(|(i, &v)| at(i, v)).collect();
    if pts.len() > 1 {
        painter.add(egui::Shape::line(pts.clone(), Stroke::new(1.5, p.fg)));
    }
    for q in &pts {
        painter.rect_filled(Rect::from_center_size(*q, vec2(4.0, 4.0)), 0.0, p.bright);
    }
    x_labels(&painter, plot, p, values.len(), slot, |i| format!("H{i}"));
}

/// A column chart: one group per x, one column per series (solid or outlined).
fn column_chart(
    ui: &mut Ui,
    p: &Palette,
    h: f32,
    series: &[(&str, &[f32], bool)],
    max: f32,
    label: impl Fn(usize) -> String,
    highlight_last: bool,
) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
    let painter = ui.painter_at(rect);
    let max = max.max(1.0);
    let items: Vec<(&str, bool)> = if series.len() > 1 {
        series.iter().map(|s| (s.0, s.2)).collect()
    } else {
        Vec::new()
    };
    let plot = axes(&painter, rect, p, max, "", &items);
    let n = series.iter().map(|s| s.1.len()).max().unwrap_or(0);
    if n == 0 {
        return;
    }
    let slot = (plot.width() / n as f32).min(size::COL_SLOT_MAX);
    let k = series.len().max(1) as f32;
    let w = (slot * 0.7 / k).max(1.0);
    for (si, (_, vals, solid)) in series.iter().enumerate() {
        for (i, &v) in vals.iter().enumerate() {
            let x = plot.left() + slot * i as f32 + slot * 0.15 + w * si as f32;
            let top = plot.bottom() - plot.height() * (v / max).clamp(0.0, 1.0);
            let r = Rect::from_min_max(pos2(x, top), pos2(x + w - 1.0, plot.bottom()));
            let lit = highlight_last && i + 1 == vals.len();
            if *solid {
                painter.rect_filled(r, 0.0, if lit { p.fg } else { p.dim });
            } else {
                painter.rect_stroke(r, 0.0, Stroke::new(1.0, p.fg), StrokeKind::Inside);
            }
        }
    }
    x_labels(&painter, plot, p, n, slot, label);
}

/// A form: labelled fields with a one-line note each. In a narrow window the notes go, so the
/// values never get cut.
fn form(ui: &mut Ui, p: &Palette, id: &str, rows: &[(&str, String, &str, bool)]) {
    let notes = ui.available_width() >= size::FORM_NOTES_W;
    egui::Grid::new(id)
        .num_columns(if notes { 3 } else { 2 })
        .spacing([10.0, 3.0])
        .show(ui, |ui| {
            for (label, value, note, hot) in rows {
                ui.label(t(*label, p.dim));
                ui.label(t(
                    format!("[ {value} ]"),
                    if *hot { p.bright } else { p.fg },
                ));
                if notes {
                    ui.label(small(*note, p.dim));
                }
                ui.end_row();
            }
        });
}

impl Panes<'_> {
    fn suite_deck(&mut self, ui: &mut Ui) {
        let g = self.game;
        let p = self.pal;
        let d = &g.deck;
        let deck = self.deck.to_uppercase();
        window(ui, &p, Tab::Deck.number(), "SYSTEM RESOURCES", |ui| {
            let ch = chart_h(ui.available_height());
            page(ui, "suite-deck", |ui| {
                ui.label(t(format!("{deck}   ASSET {:08X}", g.seed as u32), p.bright));
                ui.add_space(4.0);
                let cpu = g.combat.as_ref().map_or(d.cpu_max, |c| c.cpu);
                let hdd = g.hdd.len() as u32;
                let trace_hot = d.trace * 4 >= tuning::TRACE_MAX * 3;
                let low = d.integrity * 3 <= d.integrity_max;
                let mut rows = vec![
                    (
                        "CPU CYCLES",
                        format!("{cpu} / {}", d.cpu_max),
                        "cycles left this turn",
                        false,
                    ),
                    (
                        "MEMORY GB ",
                        format!("{} / {}", g.ifs_left(), d.ram_max),
                        "decisions left this turn",
                        false,
                    ),
                    (
                        "STORAGE   ",
                        format!("{hdd} / {}", d.hdd_max),
                        "program slots on disk",
                        hdd == d.hdd_max,
                    ),
                    (
                        "INTEGRITY ",
                        format!("{} / {}", d.integrity, d.integrity_max),
                        "at zero the deck is lost",
                        low,
                    ),
                    (
                        "TRACE %   ",
                        format!("{} / {}", d.trace, tuning::TRACE_MAX),
                        "at 100 you are found",
                        trace_hot,
                    ),
                ];
                if let Some(c) = &g.combat
                    && c.shield > 0
                {
                    rows.push(("SHIELD    ", c.shield.to_string(), "absorbs hits", false));
                }
                rows.push((
                    "CREDITS   ",
                    format!("{}¢", d.credits),
                    "earned this run",
                    false,
                ));
                rows.push((
                    "FRAGMENTS ",
                    d.fragments.to_string(),
                    "for optimizing",
                    false,
                ));
                rows.push(("HOPS      ", g.hops.to_string(), "nodes travelled", false));
                form(ui, &p, "suite-sysres", &rows);
                heading(ui, &p, "UTILIZATION");
                bar_chart(
                    ui,
                    &p,
                    &[
                        ("CPU", d.cpu_max - cpu.min(d.cpu_max), d.cpu_max, false),
                        (
                            "RAM",
                            d.ram_max - g.ifs_left().min(d.ram_max),
                            d.ram_max,
                            false,
                        ),
                        ("HDD", hdd, d.hdd_max, hdd == d.hdd_max),
                        ("INTEG", d.integrity, d.integrity_max, low),
                        ("TRACE", d.trace, tuning::TRACE_MAX, trace_hot),
                    ],
                );
                heading(ui, &p, "TRACE BY HOP");
                line_chart(
                    ui,
                    &p,
                    ch,
                    &g.history.trace_hops,
                    tuning::TRACE_MAX,
                    &[tuning::TRACE_MAX * 3 / 4, tuning::TRACE_MAX],
                );
            });
        });
    }

    fn suite_warez(&mut self, ui: &mut Ui) {
        let g = self.game;
        let p = self.pal;
        let fighting = g.phase == Phase::Combat;
        let cpu_left = g.combat.as_ref().map_or(0, |c| c.cpu);
        let mut acts: Vec<Request> = Vec::new();
        let st = &mut *self.suite;
        window(ui, &p, Tab::Warez.number(), "", |ui| {
            let rows = st.rows(g);
            let picked = st.picked(g);
            // The cell address and formula bar.
            let row_no = picked
                .and_then(|s| rows.iter().position(|&r| r == s))
                .map_or(0, |k| k + 2);
            let col_ix = SheetCol::ALL.iter().position(|c| *c == st.col).unwrap_or(0);
            let (cell, formula) = picked.map_or((String::new(), String::new()), |s| {
                (st.col.cell(s, &g.hdd[s]), rules::formula(&g.hdd[s].spec))
            });
            egui::Frame::new()
                .stroke(Stroke::new(1.0, p.dim))
                .inner_margin(egui::Margin::symmetric(4, 1))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(t(format!("{}{row_no}:", rules::col_name(col_ix)), p.bright));
                        ui.add(egui::Label::new(t(format!("{cell}   {formula}"), p.fg)).truncate());
                    });
                });
            ui.label(small(
                "click a header to sort, double-click a row to run",
                p.dim,
            ));
            // Columns in whole cells, each its widest real value (core's `sheet_widths`) with a
            // cell of padding either side, so nothing is cut.
            let (rownum, value_w) = rules::sheet_widths(&g.hdd);
            let widths: Vec<usize> = std::iter::once(rownum)
                .chain(value_w)
                .map(|w| w + 2)
                .collect();
            let starts: Vec<usize> = widths
                .iter()
                .scan(0, |at, w| {
                    let s = *at;
                    *at += w;
                    Some(s)
                })
                .collect();
            let total: usize = widths.iter().sum();
            // A row of the sheet: its cells, and which column the pointer pressed, if any.
            let row = |ui: &mut Ui| {
                let (resp, g) = Cells::allocate(
                    ui,
                    mono(size::MONO),
                    Some(size::ROW_H),
                    (total, 1),
                    vec2(0.0, 0.0),
                    Sense::click(),
                );
                let hit = resp
                    .interact_pointer_pos()
                    .and_then(|q| g.cell_at(q))
                    .and_then(|(c, _)| (0..widths.len()).rfind(|&k| starts[k] <= c));
                (resp, g, hit)
            };
            // Where column `k`'s text goes: left, right or centred in it.
            let put = |g: &Cells,
                       painter: &egui::Painter,
                       k: usize,
                       text: &str,
                       align: Align2,
                       col: Color32| {
                let (at, w) = (starts[k], widths[k]);
                let n = text.chars().count();
                let c = match align {
                    Align2::LEFT_CENTER => at + 1,
                    Align2::RIGHT_CENTER => at + w - 1 - n,
                    _ => at + (w - n) / 2,
                };
                g.text(painter, c, 0, text, col);
            };
            let foot_h = size::ROW_H * 2.0 + 6.0;
            ScrollArea::both()
                .id_salt("suite-sheet")
                .auto_shrink([false, false])
                .max_height((ui.available_height() - foot_h).max(size::ROW_H * 3.0))
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    // Column letters, then row 1: the headers. Either sorts.
                    for header in [true, false] {
                        let (resp, cells, hit) = row(ui);
                        let painter = ui.painter_at(resp.rect);
                        for (k, w) in widths.iter().enumerate() {
                            let cell = cells.at(starts[k], 0, *w, 1);
                            painter.rect_filled(cell, 0.0, p.panel);
                            painter.rect_stroke(
                                cell,
                                0.0,
                                Stroke::new(1.0, p.faint),
                                StrokeKind::Inside,
                            );
                            let text = match (k, header) {
                                (0, true) => String::new(),
                                (0, false) => "1".to_string(),
                                (k, true) => rules::col_name(k - 1),
                                (k, false) => {
                                    let c = SheetCol::ALL[k - 1];
                                    let arrow = match (c == st.sort, st.desc) {
                                        (true, false) => " ▲",
                                        (true, true) => " ▼",
                                        _ => "",
                                    };
                                    format!("{}{arrow}", c.header())
                                }
                            };
                            let col = if header { p.dim } else { p.bright };
                            put(&cells, &painter, k, &text, Align2::CENTER_CENTER, col);
                        }
                        if resp.clicked()
                            && let Some(k) = hit.filter(|k| *k > 0)
                        {
                            let c = SheetCol::ALL[k - 1];
                            if st.sort == c {
                                st.desc = !st.desc;
                            } else {
                                st.sort = c;
                                st.desc = false;
                            }
                        }
                    }
                    for (n, &slot) in rows.iter().enumerate() {
                        let s = &g.hdd[slot];
                        let on = picked == Some(slot);
                        let (resp, cells, hit) = row(ui);
                        let painter = ui.painter_at(resp.rect);
                        for (k, w) in widths.iter().enumerate() {
                            let cell = cells.at(starts[k], 0, *w, 1);
                            let cursor = on && k > 0 && SheetCol::ALL[k - 1] == st.col;
                            if k == 0 {
                                painter.rect_filled(cell, 0.0, if on { p.dim } else { p.panel });
                            } else if cursor {
                                painter.rect_filled(cell, 0.0, p.fg);
                            }
                            painter.rect_stroke(
                                cell,
                                0.0,
                                Stroke::new(1.0, p.faint),
                                StrokeKind::Inside,
                            );
                            if k == 0 {
                                let col = if on { p.bg } else { p.dim };
                                put(
                                    &cells,
                                    &painter,
                                    0,
                                    &(n + 2).to_string(),
                                    Align2::CENTER_CENTER,
                                    col,
                                );
                            } else {
                                let c = SheetCol::ALL[k - 1];
                                let colour = if cursor {
                                    p.bg
                                } else if on {
                                    p.bright
                                } else {
                                    p.fg
                                };
                                let align = match c {
                                    SheetCol::File => Align2::LEFT_CENTER,
                                    _ => Align2::RIGHT_CENTER,
                                };
                                put(&cells, &painter, k, &c.cell(slot, s), align, colour);
                            }
                        }
                        if resp.clicked() || resp.secondary_clicked() {
                            st.select(g, slot);
                            if let Some(k) = hit.filter(|k| *k > 0) {
                                st.col = SheetCol::ALL[k - 1];
                            }
                        }
                        if resp.double_clicked() && fighting {
                            acts.push(act(Action::Run(slot)));
                        }
                        let w = &s.spec;
                        resp.on_hover_text(format!(
                            "{}\n{}\n{}",
                            w.name,
                            g.data.warez[s.warez].blurb,
                            w.effect.describe()
                        ))
                        .context_menu(|ui| {
                            if fighting
                                && ui
                                    .add_enabled(cpu_left >= w.cpu, egui::Button::new("Run"))
                                    .clicked()
                            {
                                acts.push(act(Action::Run(slot)));
                            }
                            if g.has_tool("hexed") && ui.button("Open in hexedit  F4").clicked() {
                                acts.push(Request::OpenHex(slot));
                            }
                        });
                    }
                    if rows.is_empty() {
                        ui.label(t("  (no rows)", p.dim));
                    }
                });
            // The totals row.
            ui.label(t(format!("HDD {} / {}", g.hdd.len(), g.deck.hdd_max), p.fg));
        });
        self.out.extend(acts);
    }

    fn suite_target(&mut self, ui: &mut Ui) {
        let g = self.game;
        let p = self.pal;
        let Some(c) = g.combat.as_ref().filter(|_| g.phase == Phase::Combat) else {
            let title = match g.phase {
                Phase::Flatlined => "INCIDENT: DECK LOST",
                Phase::JackedOut => "SESSION SUMMARY",
                Phase::Encounter => "MESSAGE",
                _ => "NODE RECORD",
            };
            window(ui, &p, Tab::Target.number(), title, |ui| {
                page(ui, "suite-target", |ui| self.target(ui));
            });
            return;
        };
        let ice = &g.data.ice[c.ice];
        let mut acts = Vec::new();
        window(ui, &p, Tab::Target.number(), "INCIDENT REPORT", |ui| {
            // The toolbar: run buttons and End Turn, always in view above the report.
            ui.horizontal_wrapped(|ui| {
                for (i, s) in g.hdd.iter().enumerate() {
                    let w = &s.spec;
                    let class_hit =
                        matches!(w.effect, Effect::Break { vs: Some(v), .. } if v == ice.class);
                    let label = format!(
                        "[{i}] {} {}c{}",
                        w.file(),
                        w.cpu,
                        if class_hit { " x2" } else { "" }
                    );
                    if ui
                        .add_enabled(
                            c.cpu >= w.cpu,
                            egui::Button::new(t(label, if class_hit { p.bright } else { p.fg })),
                        )
                        .on_hover_text(w.effect.describe())
                        .clicked()
                    {
                        acts.push(Action::Run(i));
                    }
                }
                if ui.button(t("End Turn  F10", p.bright)).clicked() {
                    acts.push(Action::EndTurn);
                }
            });
            let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 6.0), Sense::hover());
            ui.painter()
                .line_segment([r.left_center(), r.right_center()], Stroke::new(1.0, p.dim));
            let ch = chart_h(ui.available_height());
            page(ui, "suite-incident", |ui| {
                let severity = match ice.tier {
                    0 => "LOW",
                    1 => "MODERATE",
                    _ => "CRITICAL",
                };
                let next = g
                    .next_move()
                    .map_or_else(String::new, |m| m.text(ninsei_core::suite::LAGGED));
                form(
                    ui,
                    &p,
                    "suite-incident-form",
                    &[
                        (
                            "INCIDENT NO.",
                            format!("IR-{:03}-{:03}", g.seed % 1000, c.node),
                            "",
                            false,
                        ),
                        ("SUBJECT     ", ice.name.to_uppercase(), "", false),
                        ("CLASS       ", ice.class.label().to_string(), "", false),
                        (
                            "STRENGTH    ",
                            format!("{} / {}", c.strength, c.strength_max),
                            "bring it to zero",
                            false,
                        ),
                        ("NEXT ACTION ", next.clone(), "", true),
                        ("SEVERITY    ", severity.to_string(), "", ice.tier >= 2),
                        (
                            "TURN / CPU  ",
                            format!("{} / {} of {}", c.turn, c.cpu, g.deck.cpu_max),
                            "",
                            false,
                        ),
                    ],
                );
                heading(ui, &p, "STRENGTH BY TURN");
                let mut vals: Vec<f32> = vec![c.strength_max as f32];
                vals.extend(
                    rules::strength_by_turn(c.strength_max, &g.history.turns)
                        .into_iter()
                        .map(|v| v as f32),
                );
                if vals.last() != Some(&(c.strength as f32)) {
                    vals.push(c.strength as f32);
                }
                column_chart(
                    ui,
                    &p,
                    ch,
                    &[("STRENGTH", &vals, true)],
                    c.strength_max as f32,
                    |i| format!("T{i}"),
                    true,
                );
                heading(ui, &p, &ninsei_core::suite::procedure_title(g));
                egui::Grid::new("suite-procedure")
                    .num_columns(2)
                    .spacing([16.0, 2.0])
                    .show(ui, |ui| {
                        let [step, statement] = ninsei_core::suite::PROCEDURE_COLUMNS;
                        ui.label(t(step, p.dim));
                        ui.label(t(statement, p.dim));
                        ui.end_row();
                        // The step NEXT is on is highlighted, as the agenda's current row was.
                        for r in ninsei_core::suite::procedure(g) {
                            let (fg, bg) = match (r.next, r.seen) {
                                (true, _) => (p.bg, Some(p.fg)),
                                (false, true) => (p.fg, None),
                                (false, false) => (p.dim, None),
                            };
                            for cell in [r.step, r.statement] {
                                let text = t(cell, fg);
                                ui.label(match bg {
                                    Some(bg) => text.background_color(bg),
                                    None => text,
                                });
                            }
                            ui.end_row();
                        }
                    });
            });
        });
        for a in acts {
            self.act(a);
        }
    }

    fn suite_terminal(&mut self, ui: &mut Ui) {
        let g = self.game;
        let p = self.pal;
        let mut pane = std::mem::take(&mut self.out);
        window(ui, &p, Tab::Terminal.number(), "ACTIVITY LOG", |ui| {
            // The columns, in the suite's cells: time from 1, ref from 9, the entry from 14.
            let (resp, cells) = Cells::allocate(
                ui,
                mono(size::MONO),
                Some(size::ROW_H),
                (cols_across(ui), 1),
                vec2(0.0, 0.0),
                Sense::hover(),
            );
            let painter = ui.painter_at(resp.rect);
            for (c, head) in [(1, "TIME"), (9, "REF"), (14, "ENTRY")] {
                cells.text(&painter, c, 0, head, p.dim);
            }
            let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 2.0), Sense::hover());
            ui.painter()
                .line_segment([r.left_center(), r.right_center()], Stroke::new(1.0, p.dim));
            let input_h = size::MONO + 16.0;
            // Whole rows: the log sticks to its bottom, so a view a whole number of rows tall
            // never shows part of its top line.
            let rows = ((ui.available_height() - input_h) / size::ROW_H)
                .floor()
                .max(1.0);
            // The log's cells, measured once: every entry is one row of them.
            let across = cols_across(ui);
            let probe = Cells::sized(
                ui,
                egui::Pos2::ZERO,
                mono(size::MONO),
                Some(size::ROW_H),
                (across, 1),
            );
            ui.scope(|ui| {
                // Rows touch: `show_rows` lays them a row apart, and only paints those in view.
                ui.spacing_mut().item_spacing.y = 0.0;
                ScrollArea::vertical()
                    .id_salt("suite-log")
                    .stick_to_bottom(true)
                    .auto_shrink([false, false])
                    .max_height(rows * size::ROW_H)
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                    .show_rows(ui, size::ROW_H, g.log.len(), |ui, shown| {
                        for l in &g.log[shown] {
                            let col = match l.kind {
                                LogKind::Sys | LogKind::Program => p.dim,
                                LogKind::Info | LogKind::Good => p.fg,
                                LogKind::Bad | LogKind::Loot => p.bright,
                            };
                            let (r, _) = ui.allocate_exact_size(probe.rect.size(), Sense::hover());
                            let cells = probe.clone().at_pos(r.min);
                            let painter = ui.painter_at(r);
                            // The time, the ref, then the entry, cut with … where the window
                            // ends; plain text goes out a run at a time.
                            let time = format!("{:03}.{:02}", l.hop, l.turn);
                            cells.run(&painter, 1, 0, &time, p.dim);
                            cells.run(&painter, 9, 0, rules::log_ref(l.kind), col);
                            let entry = cut(&self.garble(&l.text), across.saturating_sub(14));
                            if entry.is_ascii() {
                                cells.run(&painter, 14, 0, &entry, col);
                            } else {
                                cells.text(&painter, 14, 0, &entry, col);
                            }
                            // The rule inside the row's last pixel, so the row's clip keeps all
                            // of it.
                            let y = r.bottom() - 0.5;
                            painter.line_segment(
                                [pos2(r.left(), y), pos2(r.right(), y)],
                                Stroke::new(1.0, p.faint),
                            );
                        }
                    });
            });
            ui.add_space(3.0);
            self.prompt_as(ui, "Command:", true);
        });
        pane.append(&mut self.out);
        self.out = pane;
    }

    /// The Module window, Turbo Debugger's: the program's lines on the suite's cells, the line
    /// about to run in inverse with `▶`, a STOP's `■`, a bad line dim with its fault after it,
    /// the cursor boxed while the window is active; the budgets in the bottom edge. Under it,
    /// in the same window, the Watches. A click activates it and puts the cursor on a line.
    fn suite_script(&mut self, ui: &mut Ui) {
        let g = self.game;
        let p = self.pal;
        let ctx = ui.ctx().clone();
        let l = module();
        let style = Listing::style(&p, mono(size::MONO));
        let note = rules::module_note(g);
        let mut acts = Vec::new();
        // Lit while it's the window you're in, a line being retyped included.
        let lit = l.active(&ctx);
        l.show(ui, g, |ui, v, f| {
            let (rows, focused) = (f.rows, f.focused);
            window_with(ui, &p, Tab::Script.number(), "", &note, lit, |ui| {
                let inner = ui.available_rect_before_wrap();
                let fit = Cells::fit_font(ui, inner, mono(size::MONO), Some(size::ROW_H));
                let c = Cells::sized(
                    ui,
                    inner.min,
                    mono(size::MONO),
                    Some(size::ROW_H),
                    (fit.cols, fit.rows),
                );
                let painter = ui.painter_at(c.rect);
                // The Watches go before the listing does when the window is short.
                let watches = c.rows >= WATCH_ROWS + 3;
                let n = c.rows - if watches { WATCH_ROWS } else { 0 };
                let shown = l.window(ui, &c, v, rows, focused, n);
                let w = number_w(rows);
                let text_c = 2 + w + 2;
                for i in shown {
                    let (r, y) = (&rows[i], v.list.row(i));
                    let current = r.gutter == Gutter::Current;
                    let (num, ink) = if r.dim {
                        (p.dim, p.dim)
                    } else if current {
                        (p.bg, p.bg)
                    } else {
                        (p.dim, p.fg)
                    };
                    if current {
                        c.fill(&painter, 0, y, c.cols, 1, p.fg);
                    }
                    let mark = match r.gutter {
                        Gutter::None => p.dim,
                        Gutter::Current => p.bg,
                        Gutter::Stop => p.bright,
                    };
                    c.text(&painter, 0, y, rules::module_mark(r.gutter), mark);
                    let line = match &r.fault {
                        Some(f) => format!("{}  {f}", r.text),
                        None => r.text.clone(),
                    };
                    let no = r.no.map_or(String::new(), |n| n.to_string());
                    if r.no.is_some() && r.no == v.edit.as_ref().map(|e| e.no) {
                        c.fill(&painter, 0, y, c.cols, 1, p.bg);
                        c.text_right(&painter, 2 + w, y, &no, p.bright);
                        let at = c.at(text_c, y, c.cols.saturating_sub(text_c), 1);
                        acts.extend(l.edit_row(ui, at, v, g, &style));
                        continue;
                    }
                    c.text_right(&painter, 2 + w, y, &no, num);
                    let room = c.cols.saturating_sub(text_c);
                    c.text_max(&painter, text_c, y, &line, room, ink);
                    // The cursor: a box round its row while the window is active.
                    if focused && r.no.is_some() && r.no == v.cursor {
                        painter.rect_stroke(
                            c.at(1, y, c.cols - 1, 1).shrink(0.5),
                            0.0,
                            Stroke::new(1.0, p.bright),
                            StrokeKind::Inside,
                        );
                    }
                }
                if watches {
                    let top = c.rows - WATCH_ROWS;
                    let y = c.at(0, top, 1, 1).center().y;
                    painter.line_segment(
                        [pos2(c.rect.left(), y), pos2(c.rect.right(), y)],
                        Stroke::new(1.0, p.dim),
                    );
                    c.text(&painter, 0, top + 1, "WATCHES", p.bright);
                    let half = c.cols / 2;
                    let field =
                        |at: usize, row: usize, room: usize, (name, value): &(&str, String)| {
                            c.text(&painter, at, row, name, p.dim);
                            c.text_max(&painter, at + 7, row, value, room.saturating_sub(8), p.fg);
                        };
                    let ws = rules::watches(g);
                    // TURN CPU / IFS INTEG / TRACE NEXT in two columns; ICE and LAST a row each.
                    for (k, pair) in ws[..6].chunks(2).enumerate() {
                        field(0, top + 2 + k, half, &pair[0]);
                        field(half, top + 2 + k, c.cols - half, &pair[1]);
                    }
                    field(0, top + 5, c.cols, &ws[6]);
                    field(0, top + 6, c.cols, &ws[7]);
                }
                l.click(&ctx, f.tile, &c, v, rows, n, g);
                ui.allocate_rect(inner, Sense::hover());
            });
        });
        for a in acts {
            self.act(a);
        }
    }

    fn suite_todo(&mut self, ui: &mut Ui) {
        let g = self.game;
        let p = self.pal;
        window(ui, &p, Tab::Todo.number(), "WORK ORDERS", |ui| {
            page(ui, "suite-todo", |ui| {
                if g.contracts.is_empty() {
                    ui.label(t("No open work orders.", p.fg));
                    ui.label(small(
                        "Fixers post jobs on BBS boards and wait at MSG nodes.",
                        p.dim,
                    ));
                    return;
                }
                let here = g.root().nodes[g.root_at()].layer;
                egui::Grid::new("suite-wo")
                    .num_columns(8)
                    .spacing([10.0, 3.0])
                    .striped(false)
                    .show(ui, |ui| {
                        for h in [
                            "",
                            "WO#",
                            "CLIENT",
                            "TARGET",
                            "LAYER",
                            "STATUS",
                            "AMOUNT",
                            "CONDITION",
                        ] {
                            ui.label(t(h, p.dim));
                        }
                        ui.end_row();
                        for (i, k) in g.contracts.iter().enumerate() {
                            let n = &g.root().nodes[k.node];
                            let (mark, status, col) = match k.status {
                                ContractStatus::Active => ("[ ]", "OPEN".into(), p.fg),
                                ContractStatus::Done => ("[x]", "PAID".into(), p.dim),
                                ContractStatus::Failed => ("[-]", "VOID".into(), p.faint),
                                ContractStatus::Offered => ("[?]", "OFFERED".into(), p.dim),
                                ContractStatus::Betrayed => (
                                    "[!]",
                                    format!(
                                        "VOID — {}",
                                        g.betrayal_label(k).unwrap_or("DEFAULTED")
                                    ),
                                    p.bright,
                                ),
                            };
                            ui.label(t(mark, col));
                            ui.label(t(format!("WO-{:04}", 1001 + i), col));
                            ui.label(t(&k.client, col)).on_hover_text(&k.brief);
                            ui.label(t(&n.name, col));
                            ui.label(t(format!("{here}/{}", n.layer), col));
                            ui.label(t(status, col));
                            let amount = if k.status == ContractStatus::Betrayed {
                                k.paid
                            } else {
                                k.reward
                            };
                            ui.label(t(
                                format!("{amount}¢"),
                                if k.status == ContractStatus::Active {
                                    p.bright
                                } else {
                                    col
                                },
                            ));
                            let cond = rules::ghost_cell(k, g.deck.trace).unwrap_or_default();
                            ui.label(small(
                                cond,
                                if k.status == ContractStatus::Active {
                                    p.fg
                                } else {
                                    p.dim
                                },
                            ));
                            ui.end_row();
                        }
                    });
                let (r, _) =
                    ui.allocate_exact_size(vec2(ui.available_width(), 4.0), Sense::hover());
                ui.painter()
                    .line_segment([r.left_center(), r.right_center()], Stroke::new(1.0, p.dim));
                ui.label(t(
                    format!("TOTAL OUTSTANDING   {}¢", rules::outstanding(&g.contracts)),
                    p.bright,
                ));
            });
        });
    }

    /// SOUNDLVL.RPT: the speaker's output as a report: level and peak on a form, and the
    /// level by band as a bar chart.
    fn suite_vis(&mut self, ui: &mut Ui) {
        use ninsei_core::scope;
        let p = self.pal;
        let (samples, rate) = ((self.hear)(), self.scope_rate);
        crate::vispane::alive(ui, &samples);
        let samples = &samples[..];
        // A report's figures are read, not watched: the bands take a new reading a few times a
        // second, like the level.
        let bands: Vec<f32> = crate::vispane::bars(ui, "suite-vis", samples, rate, SUITE_BANDS)
            .iter()
            .enumerate()
            .map(|(i, v)| crate::vispane::held(ui, &format!("suite-vis-band{i}"), *v))
            .collect();
        window(ui, &p, Tab::Vis.number(), "SOUND LEVEL REPORT", |ui| {
            page(ui, "suite-vis", |ui| {
                let pk = crate::vispane::held(ui, "suite-vis-peak", scope::peak(samples));
                let level = crate::vispane::held(ui, "suite-vis-level", scope::level(samples));
                let rows = [
                    (
                        "OUTPUT LEVEL",
                        scope::db_text(level),
                        "average (RMS)",
                        false,
                    ),
                    (
                        "PEAK",
                        scope::db_text(pk),
                        "loudest sample",
                        scope::clipping(pk),
                    ),
                ];
                form(ui, &p, "suite-vis-form", &rows);
                heading(ui, &p, "LEVEL BY BAND");
                let rows: Vec<(String, f32)> = bands
                    .iter()
                    .enumerate()
                    .map(|(b, h)| {
                        let label = format!("{:>5} Hz", scope::band_label(b, SUITE_BANDS, rate));
                        (label, scope::db_at(*h))
                    })
                    .collect();
                db_chart(ui, &p, &rows);
            });
        });
    }

    fn suite_net(&mut self, ui: &mut Ui) {
        let g = self.game;
        let p = self.pal;
        let doc = rules::doc_name(g.site.as_ref().map(|s| s.name.as_str()));
        window(ui, &p, Tab::Net.number(), "NETWORK USAGE REPORT", |ui| {
            let ch = chart_h(ui.available_height());
            page(ui, "suite-net", |ui| {
                ui.label(t(format!("ACCOUNT {doc}"), p.fg));
                let hops = crate::netpane::per_hop(g);
                let down: Vec<f32> = hops.iter().map(|h| h.0).collect();
                let up: Vec<f32> = hops.iter().map(|h| h.1).collect();
                let max = down.iter().chain(&up).copied().fold(1.0_f32, f32::max);
                heading(ui, &p, "TRAFFIC BY HOP (KiB)");
                column_chart(
                    ui,
                    &p,
                    ch,
                    &[("RECEIVED", &down, true), ("SENT", &up, false)],
                    max,
                    |i| format!("H{i}"),
                    false,
                );
                heading(ui, &p, "DETAIL");
                egui::Grid::new("suite-net-detail")
                    .num_columns(3)
                    .spacing([18.0, 2.0])
                    .show(ui, |ui| {
                        for h in ["HOP", "RECEIVED", "SENT"] {
                            ui.label(t(h, p.dim));
                        }
                        ui.end_row();
                        for (i, (d, u)) in hops.iter().enumerate().rev().take(12) {
                            ui.label(t(format!("{i:>3}"), p.dim));
                            ui.label(t(crate::netpane::kib(*d), p.fg));
                            ui.label(t(crate::netpane::kib(*u), p.fg));
                            ui.end_row();
                        }
                        let (td, tu) = hops.iter().fold((0.0, 0.0), |a, h| (a.0 + h.0, a.1 + h.1));
                        ui.label(t("TOTAL", p.bright));
                        ui.label(t(crate::netpane::kib(td), p.bright));
                        ui.label(t(crate::netpane::kib(tu), p.bright));
                        ui.end_row();
                    });
            });
        });
    }
}

/// F-key `n` as egui names it.
fn fkey_of(n: u8) -> Option<Key> {
    Some(match n {
        1 => Key::F1,
        2 => Key::F2,
        3 => Key::F3,
        4 => Key::F4,
        8 => Key::F8,
        9 => Key::F9,
        10 => Key::F10,
        _ => return None,
    })
}

/// The legend up now: the Module window's while it's active, else the suite's own.
fn legend_now(ctx: &egui::Context) -> Legend {
    if module().active(ctx) {
        Legend::Module
    } else {
        Legend::Default
    }
}

/// What F-key `k` asks for, given the sheet's cursor and the Module window's: the same
/// whether it's pressed or clicked on the legend. A debugger key that wouldn't press asks for
/// nothing, and nothing does while a line is being retyped (the row has the keyboard). Edit
/// and Insert start a retype through [`retype_next`], so a click on the legend can't take
/// the row's keys away in the frame it opens.
fn fkey(ctx: &egui::Context, k: FKey, st: &State, g: &Game) -> Option<Request> {
    let m = module();
    if m.editing(ctx) || !rules::fkey_live(g, k, m.cursor(ctx)) {
        return None;
    }
    Some(match k {
        FKey::Help => Request::Cmd(Command::Help),
        FKey::Hex => Request::OpenHex(st.picked(g)?),
        FKey::Jack => Request::Type("jack ".into()),
        FKey::EndTurn => act(Action::EndTurn),
        FKey::Step => act(Action::Step),
        FKey::Run => act(Action::Play),
        FKey::Pause => act(Action::Pause),
        FKey::Edit | FKey::Open => {
            retype_next(ctx, Some(k));
            return None;
        }
    })
}

/// What the Module window does at the start of the next frame: take the keys back (a click
/// on its legend lands outside it, and egui lets the focus go), and open a retype if Edit or
/// Insert asked for one.
#[derive(Clone, Copy, Debug, Default)]
struct Pending {
    retake: bool,
    retype: Option<FKey>,
}

fn pending_id() -> egui::Id {
    egui::Id::new("suite-module-pending")
}

/// Asks the Module window to take its keys back next frame, and to retype (Edit) or open
/// (Insert) a line then.
fn retype_next(ctx: &egui::Context, retype: Option<FKey>) {
    ctx.data_mut(|d| {
        let p = d.get_temp_mut_or_default::<Pending>(pending_id());
        p.retake = true;
        p.retype = retype.or(p.retype);
    });
}

/// Does what [`retype_next`] asked last frame.
fn settle(ctx: &egui::Context, g: &Game) {
    let Some(p) = ctx.data_mut(|d| d.remove_temp::<Pending>(pending_id())) else {
        return;
    };
    let m = module();
    if p.retake {
        m.take_keys(ctx);
    }
    let mut v = m.view(ctx, g);
    let line = match p.retype {
        Some(FKey::Edit) => v
            .cursor
            .map(|n| (n, ninsei_core::listing::editing_text(g, n))),
        Some(FKey::Open) => {
            ninsei_core::listing::open_after(g, v.cursor).map(|n| (n, String::new()))
        }
        _ => None,
    };
    if let Some((no, text)) = line {
        m.start_edit(ctx, &mut v, no, &text);
    }
    m.keep(ctx, v);
}

/// The Module window's listing: its focus id and memory.
pub(crate) fn module() -> Listing {
    Listing::new("suite-module")
}

/// Rows the Watches strip takes under the listing: a rule, its heading, five rows of fields.
const WATCH_ROWS: usize = 7;

const SUITE_MENUS: Menus = &[
    ("File", &[MenuPart::Decks, MenuPart::Monitors]),
    (
        "Edit",
        &[MenuPart::Glitches, MenuPart::Toggles, MenuPart::Sound],
    ),
    ("View", &[MenuPart::Layout, MenuPart::Panes]),
    ("Run", &[MenuPart::Turn]),
    ("Window", &[MenuPart::Themes]),
    ("Help", &[MenuPart::Help]),
];

/// The suite chrome.
pub struct Suite;

impl ChromeView for Suite {
    fn deck(&self, p: &mut Panes, ui: &mut Ui) {
        p.suite_deck(ui);
    }
    fn warez(&self, p: &mut Panes, ui: &mut Ui) {
        p.suite_warez(ui);
    }
    fn network(&self, p: &mut Panes, ui: &mut Ui) {
        let pal = p.pal;
        window(ui, &pal, Tab::Network.number(), "", |ui| p.network(ui));
    }
    fn target(&self, p: &mut Panes, ui: &mut Ui) {
        p.suite_target(ui);
    }
    fn hex(&self, p: &mut Panes, ui: &mut Ui) {
        let pal = p.pal;
        window(ui, &pal, Tab::Hex.number(), "", |ui| p.hex_pane(ui));
    }
    fn terminal(&self, p: &mut Panes, ui: &mut Ui) {
        p.suite_terminal(ui);
    }
    fn todo(&self, p: &mut Panes, ui: &mut Ui) {
        p.suite_todo(ui);
    }
    fn net(&self, p: &mut Panes, ui: &mut Ui) {
        p.suite_net(ui);
    }
    fn vis(&self, p: &mut Panes, ui: &mut Ui) {
        p.suite_vis(ui);
    }
    /// The PROC-7.BAS Module window over its Watches, the whole run.
    fn script(&self, p: &mut Panes, ui: &mut Ui) {
        p.suite_script(ui);
    }

    fn title(&self, tab: Tab) -> Option<&'static str> {
        Some(match tab {
            Tab::Deck => " SYSRES.FRM ",
            Tab::Warez => " WAREZ.WK1 ",
            Tab::Network => " PLANVIEW.DWG ",
            Tab::Target => " INCIDENT.RPT ",
            Tab::Hex => " HEXEDIT.EXE ",
            Tab::Terminal => " ACTIVITY.LOG ",
            Tab::Todo => " WORKORD.DBF ",
            Tab::Net => " NETUSAGE.RPT ",
            Tab::Vis => " SOUNDLVL.RPT ",
            Tab::Script => " PROC-7.BAS ",
        })
    }
    /// Three columns over the activity log, the Stack's shape: SYSRES.FRM over WAREZ.WK1 on
    /// the left (narrow), the plan view with the incident report in the centre (widest), the
    /// PROC-7.BAS Module window in front of the work orders on the right.
    fn default_layout(&self, extra: &[Tab]) -> egui_dock::DockState<Tab> {
        // The Module window is wider than btop's tile: its Watches sit two to a row.
        crate::chrome::three_columns(
            extra,
            crate::chrome::Shares {
                over_bottom: 0.77,
                left: 0.22,
                deck: 0.6,
                centre_of_rest: 0.56,
                terminal_of_bottom: 0.6,
            },
        )
    }
    fn scroll_bars(&self, _tab: Tab) -> Option<[bool; 2]> {
        // Each window scrolls itself, with its own bars.
        Some([false, false])
    }
    fn brand(&self) -> &'static str {
        "HOSAKA"
    }
    fn menus(&self) -> Menus {
        SUITE_MENUS
    }
    fn sound(&self, ctx: &egui::Context, cx: &mut FrameCx) -> Vec<Request> {
        sound_dialog(ctx, cx)
    }
    fn sound_item(&self) -> &'static str {
        "Sound..."
    }

    fn status(&self, ui: &mut Ui, pal: &Palette, game: &Game) -> bool {
        let g = game;
        let p = *pal;
        let (mode, hot) = match g.phase {
            Phase::Exploring => ("READY", false),
            Phase::Combat => ("INCIDENT", true),
            Phase::Encounter => ("MESSAGE", true),
            Phase::Flatlined => ("FAULT", true),
            Phase::JackedOut => ("LOGGED OFF", false),
        };
        let n = g.net.nodes.len();
        // One row of cells: the mode in inverse, then the fields between rules.
        let mut line: Sheet<()> = Sheet::new(cols_across(ui));
        let mut c = line.inv(
            0,
            0,
            &format!(" {mode} "),
            p.bg,
            if hot { p.bright } else { p.fg },
        );
        let trace_hot = g.deck.trace * 4 >= tuning::TRACE_MAX * 3;
        let fields = [
            (format!("  {}  HOP {}", rules::date(g.seed), g.hops), p.fg),
            (format!("USER {}", rules::user_id(g.seed)), p.fg),
            (
                rules::doc_name(g.site.as_ref().map(|s| s.name.as_str())),
                p.bright,
            ),
            (format!("NODE {} OF {n}", g.at + 1), p.fg),
            (
                format!("TRACE {}%", g.deck.trace),
                if trace_hot { p.bright } else { p.fg },
            ),
        ];
        for (i, (text, col)) in fields.iter().enumerate() {
            if i > 0 {
                c = line.put(c, 0, "  │  ", p.dim);
            }
            c = line.put(c, 0, text, *col);
        }
        line.paint(ui, mono(size::MONO), Some(size::ROW_H));
        true
    }

    fn has_legend(&self) -> bool {
        true
    }
    /// The F-key legend: the Module window's keys while it's active, else the suite's own,
    /// from the one table the keys use. A key that wouldn't do anything now is dim.
    fn legend(&self, ui: &mut Ui, cx: &mut FrameCx) -> Vec<Request> {
        let p = cx.pal;
        let ctx = ui.ctx().clone();
        let cursor = module().cursor(&ctx);
        let editing = module().editing(&ctx);
        let legend = legend_now(&ctx);
        // One row of cells: each key's cap in inverse and what it does, both clickable.
        let mut line: Sheet<FKey> = Sheet::new(cols_across(ui));
        let mut c = 0;
        for &(n, k, label) in legend.keys() {
            if k == FKey::Hex && !cx.game.has_tool("hexed") {
                continue;
            }
            let live = !editing && rules::fkey_live(cx.game, k, cursor);
            let start = c;
            c = line.inv(
                c,
                0,
                &format!("F{n}"),
                p.bg,
                if live { p.fg } else { p.dim },
            );
            c = line.put(c + 1, 0, label, if live { p.fg } else { p.dim });
            line.hit(start, 0, c - start, k);
            c += 2;
        }
        let clicked = line.paint(ui, mono(size::MONO), Some(size::ROW_H));
        // A click on the Module window's legend lands outside the window: it gets its keys
        // back next frame.
        if legend == Legend::Module && !clicked.is_empty() {
            retype_next(&ctx, None);
        }
        clicked
            .into_iter()
            .filter_map(|k| fkey(&ctx, k, cx.suite, cx.game))
            .collect()
    }

    fn keys(&self, ctx: &egui::Context, cx: &mut FrameCx) -> Vec<Request> {
        if !cx.keys_free {
            return Vec::new();
        }
        let g = cx.game;
        settle(ctx, g);
        let mut out = Vec::new();
        // The Module window's own keys while it's active (and no dialog is waiting on Enter).
        if cx.suite.dialogs.is_empty() {
            out.extend(
                module()
                    .keys(ctx, g, |key, _| {
                        Some(match key {
                            Key::ArrowUp => Bind::Up,
                            Key::ArrowDown => Bind::Down,
                            Key::Enter => Bind::Edit,
                            Key::Insert => Bind::Open,
                            // A Mac's delete key is Backspace; it has no Ins (F3 Insert).
                            Key::Delete | Key::Backspace => Bind::Delete,
                            Key::Escape => Bind::Release,
                            _ => return None,
                        })
                    })
                    .into_iter()
                    .map(act),
            );
        }
        let st = &mut *cx.suite;
        let legend = legend_now(ctx);
        let (pressed, arrows) = ctx.input(|i| {
            // A fresh press only: a held F8 steps once, not on every repeat.
            let fresh = |k: Key| {
                i.events.iter().any(|e| {
                    matches!(e, egui::Event::Key { key, pressed: true, repeat: false, .. }
                        if *key == k)
                })
            };
            let f: Vec<FKey> = legend
                .keys()
                .iter()
                .filter(|(n, _, _)| fkey_of(*n).is_some_and(fresh))
                .map(|(_, k, _)| *k)
                .collect();
            let a = [
                i.key_pressed(Key::ArrowUp),
                i.key_pressed(Key::ArrowDown),
                i.key_pressed(Key::ArrowLeft),
                i.key_pressed(Key::ArrowRight),
            ];
            (f, a)
        });
        for k in pressed {
            out.extend(fkey(ctx, k, st, g));
        }
        // The cell cursor moves with the arrows when nothing is taking typing.
        if !ctx.egui_wants_keyboard_input() && arrows.iter().any(|a| *a) {
            let rows = st.rows(g);
            if let Some(at) = st.picked(g).and_then(|s| rows.iter().position(|&r| r == s)) {
                let to = if arrows[0] {
                    at.saturating_sub(1)
                } else if arrows[1] {
                    (at + 1).min(rows.len() - 1)
                } else {
                    at
                };
                st.select(g, rows[to]);
            }
            let ci = SheetCol::ALL.iter().position(|c| *c == st.col).unwrap_or(0);
            if arrows[2] {
                st.col = SheetCol::ALL[ci.saturating_sub(1)];
            }
            if arrows[3] {
                st.col = SheetCol::ALL[(ci + 1).min(SheetCol::ALL.len() - 1)];
            }
        }
        out
    }

    fn script_has_keys(&self, ctx: &egui::Context) -> bool {
        module().has_keys(ctx)
    }

    fn overlay(&self, ctx: &egui::Context, cx: &mut FrameCx) -> Vec<Request> {
        let st = &mut *cx.suite;
        // A new run drops anything left from the last one.
        if !st.watch.watching(cx.game) {
            st.dialogs.clear();
        }
        // Enter only dismisses a dialog that was already up: the Enter that sent the command
        // causing it (`jack` to the exit) mustn't close it in the same frame.
        let was_up = !st.dialogs.is_empty();
        st.dialogs.extend(st.watch.step(cx.game));
        let Some(n) = st.dialogs.first().cloned() else {
            return Vec::new();
        };
        let p = cx.pal;
        let waiting = st.dialogs.len();
        let mut ok = was_up && cx.keys_free && ctx.input(|i| i.key_pressed(Key::Enter));
        egui::Area::new(egui::Id::new("suite-dialog"))
            .order(egui::Order::Foreground)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, -40.0))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(p.bg)
                    .stroke(Stroke::new(2.0, p.fg))
                    .inner_margin(egui::Margin::same(3))
                    .shadow(egui::Shadow {
                        offset: [8, 8],
                        blur: 0,
                        spread: 0,
                        color: p.faint,
                    })
                    .show(ui, |ui| {
                        // The body on the suite's cells: the title row, the lines, then the
                        // count and the OK button.
                        let cols = Cells::sized(
                            ui,
                            egui::Pos2::ZERO,
                            mono(size::MONO),
                            Some(size::ROW_H),
                            (1, 1),
                        );
                        let cols = (size::DIALOG_W / cols.cw) as usize;
                        let mut body: Sheet<()> = Sheet::new(cols);
                        body.inv(0, 0, &" ".repeat(cols), p.bright, p.panel);
                        let title = cut(&n.title, cols.saturating_sub(2));
                        let tn = title.chars().count();
                        body.inv((cols - tn) / 2, 0, &title, p.bright, p.panel);
                        // Lines wrapped to the dialog, two cells in from each side.
                        let lines = crate::kit::fit::wrap(&n.lines, cols.saturating_sub(4));
                        for (i, l) in lines.iter().enumerate() {
                            body.put(2, 2 + i, l, p.fg);
                        }
                        let foot = 3 + lines.len();
                        if waiting > 1 {
                            body.put(2, foot, &format!("1 of {waiting}"), p.dim);
                        }
                        let button = "[  OK  ]";
                        let bx = cols - 2 - button.chars().count();
                        body.put(bx - 6, foot, "Enter", p.dim);
                        body.put(bx, foot, button, p.bright);
                        body.hit(bx, foot, button.chars().count(), ());
                        body.grow_to(foot + 2);
                        if !body
                            .paint(ui, mono(size::MONO), Some(size::ROW_H))
                            .is_empty()
                        {
                            ok = true;
                        }
                    });
            });
        if ok {
            st.dialogs.remove(0);
        }
        Vec::new()
    }
}

/// Bands in the sound level report.
const SUITE_BANDS: usize = 8;

/// A bar chart of levels in dB, from the floor (-60) to full scale, ticked every 20 dB, each
/// row labelled at the left and its reading at the right: the sound report's chart.
fn db_chart(ui: &mut Ui, p: &Palette, rows: &[(String, f32)]) {
    use ninsei_core::scope::{FLOOR_DB, height};
    let h = rows.len() as f32 * 15.0 + 16.0;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
    let painter = ui.painter_at(rect);
    let plot = Rect::from_min_max(rect.min + vec2(64.0, 2.0), rect.max - vec2(56.0, 14.0));
    let ticks = [FLOOR_DB, -40.0, -20.0, 0.0];
    for (k, db) in ticks.iter().enumerate() {
        let x = plot.left() + plot.width() * height(*db);
        painter.line_segment(
            [pos2(x, plot.top()), pos2(x, plot.bottom())],
            Stroke::new(1.0, if k == 0 { p.dim } else { p.faint }),
        );
        painter.text(
            pos2(x, plot.bottom() + 2.0),
            Align2::CENTER_TOP,
            format!("{db:.0}"),
            mono(size::SMALL),
            p.dim,
        );
    }
    for (i, (label, db)) in rows.iter().enumerate() {
        let y = plot.top() + i as f32 * 15.0 + 7.0;
        painter.text(
            pos2(rect.left(), y),
            Align2::LEFT_CENTER,
            label,
            mono(size::SMALL),
            p.dim,
        );
        let bar = Rect::from_min_size(
            pos2(plot.left(), y - 4.0),
            vec2(plot.width() * height(*db), 8.0),
        );
        painter.rect_filled(bar, 0.0, p.fg);
        let reading = if *db <= FLOOR_DB {
            "--- dB".to_string()
        } else {
            format!("{db:.0} dB")
        };
        painter.text(
            pos2(plot.right() + 4.0, y),
            Align2::LEFT_CENTER,
            reading,
            mono(size::SMALL),
            p.fg,
        );
    }
}

/// The volume bar's cells in the Sound dialog: 10% apiece.
const SOUND_NOTCHES: u32 = 10;

/// The Sound dialog (Edit > Sound...): a Volume bar of [`SOUND_NOTCHES`] cells, a Mute check
/// box, OK and Cancel. ←/→ move the bar, Space checks the box, Enter is OK and Esc is Cancel,
/// which puts back what was set when it opened. Changes are heard as they're made.
fn sound_dialog(ctx: &egui::Context, cx: &mut FrameCx) -> Vec<Request> {
    let (p, now) = (cx.pal, cx.level);
    let (mut to, k) = crate::volume::start(ctx, now, Key::Space, SOUND_NOTCHES);
    let (mut ok, mut cancel) = (k.ok, k.esc);
    let click = |ui: &mut Ui, text: RichText, label: &str| {
        let r = ui.add(egui::Label::new(text).sense(Sense::click()));
        crate::volume::named(r, label).clicked()
    };
    dialog(ctx, "suite-sound", &p, "Sound", size::SOUND_W, |ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.horizontal(|ui| {
            ui.label(t("Volume   ", p.fg));
            let lit = now.notch(SOUND_NOTCHES);
            for n in 1..=SOUND_NOTCHES {
                let c = if n <= lit { p.bright } else { p.faint };
                let pct = ninsei_core::hub::notch_volume(n, SOUND_NOTCHES);
                if click(ui, t("■", c), &format!("{pct}%")) {
                    to.volume = pct;
                }
            }
            ui.label(t(format!("  {:>3}%", now.volume), p.fg));
        });
        ui.add_space(6.0);
        let mark = if now.on { "[ ]" } else { "[X]" };
        if click(ui, t(format!("{mark} Mute"), p.fg), "Mute") {
            to.on = !to.on;
        }
        ui.add_space(10.0);
        ui.spacing_mut().item_spacing.x = 6.0;
        match buttons(ui, &p, None, &[("[Cancel]", "Esc"), ("[  OK  ]", "Enter")]) {
            Some(0) => cancel = true,
            Some(_) => ok = true,
            None => {}
        }
    });
    if cancel {
        return cx.popup.cancel(now);
    }
    if ok {
        cx.popup.close();
    }
    now.to(to)
}

/// A suite dialog: a hard shadow, a double-weight frame, the title on a filled bar, and `body`
/// inside, centred a little above the middle of the screen.
fn dialog(
    ctx: &egui::Context,
    id: &str,
    p: &Palette,
    title: &str,
    width: f32,
    body: impl FnOnce(&mut Ui),
) {
    egui::Area::new(egui::Id::new(id))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, vec2(0.0, -40.0))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(p.bg)
                .stroke(Stroke::new(2.0, p.fg))
                .inner_margin(egui::Margin::same(3))
                .shadow(egui::Shadow {
                    offset: [8, 8],
                    blur: 0,
                    spread: 0,
                    color: p.faint,
                })
                .show(ui, |ui| {
                    ui.set_width(width);
                    let (bar, _) =
                        ui.allocate_exact_size(vec2(width, size::TITLE_H + 2.0), Sense::hover());
                    ui.painter().rect_filled(bar, 0.0, p.panel);
                    ui.painter().text(
                        bar.center(),
                        Align2::CENTER_CENTER,
                        title,
                        mono(size::MONO),
                        p.bright,
                    );
                    egui::Frame::new()
                        .inner_margin(egui::Margin::symmetric(14, 10))
                        .show(ui, body);
                });
        });
}

/// A dialog's button row: an optional note at the left, then `buttons` from the right (the
/// last is the default, lit), each with its key beside it. Which one was clicked.
fn buttons(
    ui: &mut Ui,
    p: &Palette,
    note: Option<String>,
    buttons: &[(&str, &str)],
) -> Option<usize> {
    let mut hit = None;
    ui.horizontal(|ui| {
        if let Some(note) = note {
            ui.label(small(note, p.dim));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            for (i, (label, key)) in buttons.iter().enumerate() {
                let lit = if i + 1 == buttons.len() {
                    p.bright
                } else {
                    p.fg
                };
                if ui.button(t(*label, lit)).clicked() {
                    hit = Some(i);
                }
                ui.label(small(*key, p.dim));
            }
        });
    });
    hit
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrome::FrameCx;
    use crate::uitest::{Harness, click};
    use ninsei_core::Data;
    use ninsei_core::hardware::Chrome;
    use ninsei_core::hub::Save;

    /// A run whose first program has a long name, the sheet drawn in a 900x500 window.
    fn sheet(h: &mut Harness, d: &Data, g: &Game, st: &mut State, events: Vec<egui::Event>) {
        let (mut term, mut focus, mut hex, mut keep) =
            (ninsei_term::LineEditor::default(), false, None, Vec::new());
        for ev in [events, Vec::new()] {
            h.frame(ev, |ui| {
                let mut child = ui.new_child(
                    UiBuilder::new()
                        .max_rect(Rect::from_min_size(egui::Pos2::ZERO, vec2(900.0, 500.0))),
                );
                let mut panes = Panes {
                    game: g,
                    pal: Palette::from_def(&d.themes[0]),
                    map: ninsei_core::netview::MapView::Draft,
                    deck: String::new(),
                    chrome: Chrome::Suite,
                    corrupt: (0.0, 0),
                    term: &mut term,
                    focus_input: &mut focus,
                    bbs_open: false,
                    hex: &mut hex,
                    keep: &mut keep,
                    keep_room: 0,
                    suite: &mut *st,
                    hear: &Vec::new,
                    scope_rate: 0,
                    out: Vec::new(),
                };
                panes.suite_warez(&mut child);
            });
        }
    }

    /// Each row of single glyphs the last frame painted, left to right: (where the row was
    /// placed, each glyph's ink and text).
    fn glyph_rows(h: &Harness) -> Vec<(i32, Vec<(Rect, String)>)> {
        let mut rows: Vec<(i32, Vec<(Rect, String)>)> = Vec::new();
        for s in h.painted() {
            if let egui::Shape::Text(t) = s
                && t.galley.text().chars().count() == 1
            {
                // A row's glyphs share where they were placed, whatever their ink's shape.
                let r = t.visual_bounding_rect();
                let y = t.pos.y.round() as i32;
                match rows.iter_mut().find(|(ry, _)| *ry == y) {
                    Some((_, v)) => v.push((r, t.galley.text().to_string())),
                    None => rows.push((y, vec![(r, t.galley.text().to_string())])),
                }
            }
        }
        for (_, v) in &mut rows {
            v.sort_by(|a, b| a.0.left().total_cmp(&b.0.left()));
        }
        rows
    }

    #[test]
    fn the_sheet_shows_every_value_whole_and_a_click_lands_on_its_cell() {
        let d = Data::builtin().unwrap();
        let mut s = Save::default();
        s.ensure_hardware(&d);
        let mut g = s.jack_in(&d, 1);
        g.hdd[0].spec.name = "A.MUCH.LONGER.PROGRAM.NAME".into();
        let long = g.hdd[0].spec.file();
        let uid = g.hdd[0].uid;
        let mut st = State::default();
        let mut h = Harness::new(900.0, 500.0);
        sheet(&mut h, &d, &g, &mut st, Vec::new());
        let rows = glyph_rows(&h);
        let (_, name_row) = rows
            .iter()
            .find(|(_, v)| {
                v.iter()
                    .map(|g| g.1.as_str())
                    .collect::<String>()
                    .contains(&long)
            })
            .unwrap_or_else(|| panic!("{long} painted whole: {rows:?}"));
        // The name's first glyph: click it, and the cursor goes to that program's FILE cell.
        let joined: String = name_row.iter().map(|g| g.1.as_str()).collect();
        let at = joined.find(&long).unwrap();
        let first = name_row[joined[..at].chars().count()].0;
        sheet(&mut h, &d, &g, &mut st, click(first.center()));
        assert_eq!((st.sel, st.col), (Some(uid), SheetCol::File));
        // The header row's RAM sorts.
        let rows = glyph_rows(&h);
        let (_, head) = rows
            .iter()
            .find(|(_, v)| {
                v.iter()
                    .map(|g| g.1.as_str())
                    .collect::<String>()
                    .contains("SLOT")
            })
            .expect("the headers");
        let joined: String = head.iter().map(|g| g.1.as_str()).collect();
        let cpu = head[joined[..joined.find("CPU").unwrap()].chars().count()].0;
        sheet(&mut h, &d, &g, &mut st, click(cpu.center()));
        assert_eq!(st.sort, SheetCol::Cpu);
    }

    #[test]
    fn the_legends_keys_click_through_to_their_requests() {
        let d = Data::builtin().unwrap();
        let mut s = Save::default();
        s.ensure_hardware(&d);
        let g = s.jack_in(&d, 1);
        let mut st = State::default();
        let mut h = Harness::new(900.0, 100.0);
        let mut got = Vec::new();
        let mut draw = |h: &mut Harness, st: &mut State, events| {
            h.frame(events, |ui| {
                let mut popup = crate::volume::Popup::default();
                let mut cx = FrameCx {
                    game: &g,
                    pal: Palette::from_def(&d.themes[0]),
                    suite: st,
                    keys_free: true,
                    level: crate::volume::Level {
                        on: true,
                        volume: 100,
                    },
                    popup: &mut popup,
                };
                got = Suite.legend(ui, &mut cx);
            });
        };
        draw(&mut h, &mut st, Vec::new());
        let end = h
            .find("F10 End turn")
            .unwrap_or_else(|| panic!("{:?}", h.texts()));
        draw(&mut h, &mut st, click(end.center()));
        assert!(
            matches!(
                got.as_slice(),
                [Request::Cmd(Command::Game(Action::EndTurn))]
            ),
            "{:?}",
            got.len()
        );
    }

    #[test]
    fn a_window_keeps_to_its_pane_with_its_body_inside_the_frame() {
        let d = Data::builtin().unwrap();
        let p = Palette::from_def(&d.themes[0]);
        let mut h = Harness::new(900.0, 600.0);
        let pane = Rect::from_min_size(egui::pos2(20.0, 30.0), vec2(401.0, 233.0));
        let mut body = Rect::NOTHING;
        h.frame(Vec::new(), |ui| {
            let mut child = ui.new_child(UiBuilder::new().max_rect(pane));
            window(&mut child, &p, 3, "SYSTEM RESOURCES", |ui| {
                body = ui.max_rect()
            });
        });
        for s in h.painted() {
            let r = s.visual_bounding_rect();
            assert!(pane.expand(0.5).contains_rect(r), "{r:?} outside {pane:?}");
        }
        assert!(pane.contains_rect(body) && body.height() > 120.0);
        let painted: String = h
            .painted()
            .iter()
            .filter_map(|s| match s {
                egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                _ => None,
            })
            .collect();
        for part in ["[■]", "SYSTEMRESOURCES", "[3]"] {
            assert!(
                painted.contains(part),
                "{part} in the window's top: {painted}"
            );
        }
    }

    #[test]
    fn a_dialog_says_its_piece_and_ok_dismisses_it() {
        let d = Data::builtin().unwrap();
        let mut s = Save::default();
        s.ensure_hardware(&d);
        let g = s.jack_in(&d, 1);
        let mut st = State::default();
        st.watch.step(&g);
        st.dialogs.push(ninsei_core::suite::Notice {
            title: "PAYMENT RECEIVED".into(),
            lines: vec![
                "Ninsei Community College paid 120¢.".into(),
                "You have been logged off. Thank you for using Hosaka. Have a pleasant day.".into(),
            ],
        });
        let mut h = Harness::new(900.0, 600.0);
        let draw = |h: &mut Harness, st: &mut State, events| {
            h.frame(events, |ui| {
                let mut popup = crate::volume::Popup::default();
                let mut cx = FrameCx {
                    game: &g,
                    pal: Palette::from_def(&d.themes[0]),
                    suite: st,
                    keys_free: true,
                    level: crate::volume::Level {
                        on: true,
                        volume: 100,
                    },
                    popup: &mut popup,
                };
                Suite.overlay(ui.ctx(), &mut cx);
            });
        };
        // An area sits out its first frame while it measures itself.
        draw(&mut h, &mut st, Vec::new());
        draw(&mut h, &mut st, Vec::new());
        let painted: String = h
            .painted()
            .iter()
            .filter_map(|s| match s {
                egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                _ => None,
            })
            .collect();
        assert!(painted.contains("PAYMENTRECEIVED") || painted.contains("PAYMENT RECEIVED"));
        assert!(painted.contains("120"), "its line: {painted}");
        // The long line wraps whole onto the next row, never cut.
        assert!(painted.contains("pleasantday."), "{painted}");
        assert!(!painted.contains('…'), "nothing cut: {painted}");
        let ok = h
            .find("[ OK ]")
            .unwrap_or_else(|| panic!("{:?}", h.texts()));
        draw(&mut h, &mut st, click(ok.center()));
        assert!(st.dialogs.is_empty(), "OK dismissed it");
    }

    #[test]
    fn the_log_lays_its_entries_a_row_apart() {
        let d = Data::builtin().unwrap();
        let mut s = Save::default();
        s.ensure_hardware(&d);
        let g = s.jack_in(&d, 1);
        assert!(g.log.len() >= 3);
        let p = Palette::from_def(&d.themes[0]);
        let mut st = State::default();
        let mut h = Harness::new(900.0, 600.0);
        let (mut term, mut focus, mut hex, mut keep) =
            (ninsei_term::LineEditor::default(), false, None, Vec::new());
        for _ in 0..2 {
            h.frame(Vec::new(), |ui| {
                let room = Rect::from_min_size(egui::pos2(0.0, 0.0), vec2(840.0, 400.0));
                let mut child = ui.new_child(UiBuilder::new().max_rect(room));
                let mut panes = Panes {
                    game: &g,
                    pal: p,
                    map: ninsei_core::netview::MapView::Draft,
                    deck: String::new(),
                    chrome: Chrome::Suite,
                    corrupt: (0.0, 0),
                    term: &mut term,
                    focus_input: &mut focus,
                    bbs_open: false,
                    hex: &mut hex,
                    keep: &mut keep,
                    keep_room: 0,
                    suite: &mut st,
                    hear: &Vec::new,
                    scope_rate: 0,
                    out: Vec::new(),
                };
                panes.suite_terminal(&mut child);
            });
        }
        // The rules under each entry, in the faint colour: a row apart, every one, each whole
        // inside its row (a 1px line centred on the row's edge would lose half to the clip).
        let mut ys: Vec<f32> = h
            .painted()
            .iter()
            .filter_map(|s| match s {
                egui::Shape::LineSegment { points, stroke } if stroke.color == p.faint => {
                    Some(points[0].y)
                }
                _ => None,
            })
            .collect();
        ys.sort_by(f32::total_cmp);
        assert!(ys.len() >= g.log.len(), "a rule under each entry: {ys:?}");
        for y in &ys {
            assert!(
                (y.fract() - 0.5).abs() < 0.01,
                "on a pixel's centre, not its edge: {y}"
            );
        }
        for w in ys.windows(2) {
            assert!((w[1] - w[0] - size::ROW_H).abs() < 0.01, "{ys:?}");
        }
    }

    #[test]
    fn a_window_its_tab_names_doesnt_name_itself_again() {
        let d = Data::builtin().unwrap();
        let p = Palette::from_def(&d.themes[0]);
        let pane = Rect::from_min_size(egui::pos2(20.0, 30.0), vec2(401.0, 233.0));
        let body_top = |heading: &str| {
            let mut h = Harness::new(900.0, 600.0);
            let mut body = Rect::NOTHING;
            h.frame(Vec::new(), |ui| {
                let mut child = ui.new_child(UiBuilder::new().max_rect(pane));
                window(&mut child, &p, 2, heading, |ui| body = ui.max_rect());
            });
            let painted: String = h
                .painted()
                .iter()
                .filter_map(|s| match s {
                    egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                    _ => None,
                })
                .collect();
            (body.top(), painted)
        };
        let (bare, painted) = body_top("");
        assert_eq!(painted, "[■][2]", "only the badges in the edge");
        let (headed, _) = body_top("SYSTEM RESOURCES");
        assert!(
            (headed - bare - size::ROW_H).abs() < 0.5,
            "a heading takes one row"
        );
    }

    #[test]
    fn a_window_too_small_for_its_frame_still_draws_its_body() {
        let d = Data::builtin().unwrap();
        let p = Palette::from_def(&d.themes[0]);
        let mut h = Harness::new(900.0, 600.0);
        for size in [vec2(400.0, 60.0), vec2(60.0, 300.0), vec2(30.0, 30.0)] {
            let mut ran = false;
            h.frame(Vec::new(), |ui| {
                let pane = Rect::from_min_size(egui::pos2(20.0, 30.0), size);
                let mut child = ui.new_child(UiBuilder::new().max_rect(pane));
                window(&mut child, &p, 6, "ACTIVITY LOG", |_| ran = true);
            });
            assert!(ran, "the body (and its command line) at {size:?}");
        }
    }

    #[test]
    fn a_long_log_paints_only_what_shows() {
        let d = Data::builtin().unwrap();
        let mut s = Save::default();
        s.ensure_hardware(&d);
        let mut g = s.jack_in(&d, 1);
        let line = g.log[0].clone();
        g.log = vec![line; 400];
        let mut st = State::default();
        let mut h = Harness::new(900.0, 600.0);
        let (mut term, mut focus, mut hex, mut keep) =
            (ninsei_term::LineEditor::default(), false, None, Vec::new());
        for _ in 0..2 {
            h.frame(Vec::new(), |ui| {
                let room = Rect::from_min_size(egui::pos2(0.0, 0.0), vec2(840.0, 400.0));
                let mut child = ui.new_child(UiBuilder::new().max_rect(room));
                let mut panes = Panes {
                    game: &g,
                    pal: Palette::from_def(&d.themes[0]),
                    map: ninsei_core::netview::MapView::Draft,
                    deck: String::new(),
                    chrome: Chrome::Suite,
                    corrupt: (0.0, 0),
                    term: &mut term,
                    focus_input: &mut focus,
                    bbs_open: false,
                    hex: &mut hex,
                    keep: &mut keep,
                    keep_room: 0,
                    suite: &mut st,
                    hear: &Vec::new,
                    scope_rate: 0,
                    out: Vec::new(),
                };
                panes.suite_terminal(&mut child);
            });
        }
        let texts = h
            .painted()
            .iter()
            .filter(|s| matches!(s, egui::Shape::Text(_)))
            .count();
        // A screenful of rows, three pieces each, and the window's own furniture.
        assert!(texts < 150, "{texts} pieces of text for a 400-entry log");
    }
}

#[cfg(test)]
mod module_tests {
    use super::*;
    use crate::uitest::Harness;
    use ninsei_core::Data;

    /// The right column of the default layout, about.
    const PANE: (f32, f32) = (420.0, 560.0);

    fn fight(program: &str) -> Game {
        let mut g = crate::stack::tests::a_fight(&Data::builtin().unwrap());
        g.apply(Action::Program(program.into()));
        g
    }

    fn pal(g: &Game) -> Palette {
        Palette::from_def(&g.data.themes[0])
    }

    /// The legend sits on top, as under the run's menu bar; the panes under it.
    fn legend_rect() -> Rect {
        Rect::from_min_size(pos2(0.0, 0.0), vec2(900.0, 20.0))
    }

    fn pane_rect((w, h): (f32, f32)) -> Rect {
        Rect::from_min_size(pos2(0.0, legend_rect().bottom() + 10.0), vec2(w, h))
    }

    /// One frame as the run draws it: the chrome's keys, the legend, then the Module window in a
    /// `size` pane and the activity log under it; returns what they asked for.
    fn frame_with(
        h: &mut Harness,
        g: &Game,
        size: (f32, f32),
        term: &mut ninsei_term::LineEditor,
        ev: Vec<egui::Event>,
    ) -> Vec<Request> {
        let (mut focus, mut hex, mut keep) = (false, None, Vec::new());
        let (mut suite, mut suite2) = (State::default(), State::default());
        let mut out = Vec::new();
        h.frame(ev, |ui| {
            let mut popup = crate::volume::Popup::default();
            let mut cx = FrameCx {
                game: g,
                pal: pal(g),
                suite: &mut suite2,
                keys_free: true,
                level: crate::volume::Level {
                    on: true,
                    volume: 100,
                },
                popup: &mut popup,
            };
            out = Suite.keys(ui.ctx(), &mut cx);
            // The legend before the panes, as the run draws its bottom panel first.
            let mut l = ui.new_child(UiBuilder::new().max_rect(legend_rect()));
            out.extend(Suite.legend(&mut l, &mut cx));
            let mut panes = Panes {
                game: g,
                pal: pal(g),
                map: ninsei_core::netview::MapView::Draft,
                deck: String::new(),
                chrome: ninsei_core::hardware::Chrome::Suite,
                corrupt: (0.0, 0),
                term: &mut *term,
                focus_input: &mut focus,
                bbs_open: false,
                hex: &mut hex,
                keep: &mut keep,
                keep_room: 0,
                suite: &mut suite,
                hear: &Vec::new,
                scope_rate: 0,
                out: Vec::new(),
            };
            let mut w = ui.new_child(UiBuilder::new().max_rect(pane_rect(size)));
            Suite.script(&mut panes, &mut w);
            let log = Rect::from_min_size(pos2(0.0, 950.0), vec2(800.0, 300.0));
            let mut t = ui.new_child(UiBuilder::new().max_rect(log));
            Suite.terminal(&mut panes, &mut t);
            out.extend(panes.out);
        });
        out
    }

    fn frame(h: &mut Harness, g: &Game, ev: Vec<egui::Event>) -> Vec<Request> {
        let mut term = ninsei_term::LineEditor::default();
        frame_with(h, g, PANE, &mut term, ev)
    }

    fn actions(out: Vec<Request>) -> Vec<Action> {
        out.into_iter()
            .filter_map(|r| match r {
                Request::Cmd(Command::Game(a)) => Some(a),
                _ => None,
            })
            .collect()
    }

    fn key(k: Key) -> Vec<egui::Event> {
        crate::uitest::key(k)
    }

    fn typed(s: &str) -> Vec<egui::Event> {
        crate::uitest::text(s)
    }

    /// The text painted in `area`, glyph by glyph, in reading order, with each glyph's ink.
    fn rows_in(h: &Harness, area: Rect) -> Vec<(String, Vec<Color32>)> {
        let mut glyphs: Vec<(f32, f32, String, Color32)> = h
            .painted()
            .iter()
            .filter_map(|s| match s {
                egui::Shape::Text(t) => {
                    let r = Rect::from_min_size(t.pos, t.galley.size());
                    area.contains(r.center()).then(|| {
                        (
                            r.center().y.round(),
                            r.center().x,
                            t.galley.text().to_string(),
                            t.fallback_color,
                        )
                    })
                }
                _ => None,
            })
            .collect();
        glyphs.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        let mut out: Vec<(f32, String, Vec<Color32>)> = Vec::new();
        for (y, _, t, c) in glyphs {
            match out.last_mut() {
                Some((ly, s, cs)) if (*ly - y).abs() < 2.0 => {
                    s.push_str(&t);
                    cs.push(c);
                }
                _ => out.push((y, t, vec![c])),
            }
        }
        out.into_iter().map(|(_, s, c)| (s, c)).collect()
    }

    fn row_with<'a>(rows: &'a [(String, Vec<Color32>)], text: &str) -> &'a (String, Vec<Color32>) {
        rows.iter()
            .find(|(s, _)| s.contains(text))
            .unwrap_or_else(|| panic!("no row with {text:?} in {rows:?}"))
    }

    /// Clicks line `no`'s number in the Module window: the window is active, its cursor there.
    fn activate_on(h: &mut Harness, g: &Game, no: u32) {
        frame(h, g, vec![]);
        let n = no.to_string();
        let mut glyphs: Vec<(String, Rect)> = h
            .painted_text()
            .into_iter()
            .filter(|(_, r)| pane_rect(PANE).contains(r.center()))
            .collect();
        glyphs.sort_by(|a, b| {
            a.1.center()
                .y
                .round()
                .total_cmp(&b.1.center().y.round())
                .then(a.1.center().x.total_cmp(&b.1.center().x))
        });
        let at = glyphs
            .windows(n.len())
            .find(|w| {
                w.iter().map(|g| g.0.as_str()).collect::<String>() == n && w[0].1.center().x < 60.0
            })
            .unwrap_or_else(|| panic!("no line {no}"))[0]
            .1
            .center();
        frame(h, g, crate::uitest::click(at));
        assert!(module().has_keys(&h.ctx), "the Module window is active");
        assert_eq!(module().view(&h.ctx, g).cursor, Some(no));
    }

    fn program(g: &Game) -> Vec<String> {
        g.program
            .lines
            .iter()
            .map(|l| format!("{} {}", l.no, l.text))
            .collect()
    }

    #[test]
    fn clicking_the_debuggers_keys_on_the_legend_keeps_the_window_active() {
        let g = fight("10 RUN icepick\n20 STOP\n30 GOTO 10");
        let mut h = Harness::new(1280.0, 1400.0);
        activate_on(&mut h, &g, 10);
        frame(&mut h, &g, vec![]);
        for _ in 0..2 {
            let at = h
                .find("F8 Step")
                .unwrap_or_else(|| panic!("{:?}", h.texts()))
                .center();
            let out = frame(&mut h, &g, crate::uitest::click(at));
            let acts = actions(out);
            assert_eq!(acts, [Action::Step], "the same key, twice");
            frame(&mut h, &g, vec![]);
            assert!(module().has_keys(&h.ctx), "the window is active again");
        }
        // F2 on the legend opens the retype, and it stays open.
        let at = h.find("F2 Edit").unwrap().center();
        frame(&mut h, &g, crate::uitest::click(at));
        frame(&mut h, &g, vec![]);
        frame(&mut h, &g, vec![]);
        assert!(module().editing(&h.ctx), "retyping line 10");
    }

    #[test]
    fn while_a_line_is_retyped_the_window_stays_active_and_the_f_keys_wait() {
        let g = fight("10 RUN icepick\n20 STOP\n30 GOTO 10");
        let mut h = Harness::new(1280.0, 1400.0);
        activate_on(&mut h, &g, 20);
        frame(&mut h, &g, key(Key::F2));
        frame(&mut h, &g, vec![]);
        assert!(module().editing(&h.ctx));
        let legend: String = rows_in(&h, legend_rect())
            .into_iter()
            .map(|(s, _)| s)
            .collect();
        assert!(
            legend.contains("F8Step") && !legend.contains("Jack"),
            "{legend}"
        );
        for k in [Key::F8, Key::F4] {
            let out = frame(&mut h, &g, key(k));
            assert!(out.is_empty(), "{k:?} does nothing mid-retype");
            assert!(
                module().editing(&h.ctx),
                "and the line is still being retyped"
            );
        }
    }

    #[test]
    fn a_held_f8_steps_once() {
        let g = fight("10 RUN icepick\n20 STOP\n30 GOTO 10");
        let mut h = Harness::new(1280.0, 1400.0);
        activate_on(&mut h, &g, 10);
        let down = || {
            vec![egui::Event::Key {
                key: Key::F8,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }]
        };
        assert_eq!(actions(frame(&mut h, &g, down())), [Action::Step]);
        // Still held: egui calls the next press a repeat.
        assert!(actions(frame(&mut h, &g, down())).is_empty());
    }

    #[test]
    fn on_a_mac_f3_inserts_and_backspace_deletes() {
        let mut g = fight("10 RUN icepick\n20 STOP\n30 GOTO 10");
        let mut h = Harness::new(1280.0, 1400.0);
        activate_on(&mut h, &g, 20);
        frame(&mut h, &g, key(Key::F3));
        frame(&mut h, &g, vec![]);
        frame(&mut h, &g, typed("RUN patch"));
        for a in actions(frame(&mut h, &g, key(Key::Enter))) {
            g.apply(a);
        }
        assert_eq!(g.program.line(25).unwrap().text, "RUN patch");
        frame(&mut h, &g, vec![]);
        let acts = actions(frame(&mut h, &g, key(Key::Backspace)));
        assert_eq!(acts.len(), 1);
        for a in acts {
            g.apply(a);
        }
        assert!(g.program.line(25).is_none(), "{:?}", g.program.text());
    }

    #[test]
    fn the_window_stays_whole_in_its_pane_at_three_sizes() {
        let g = fight(
            "10 REM PROC-7 REV C  PROPERTY OF HOSAKA CORP.\n20 IF INTEG < 9\n30 RUN patch\n40 STOP\n50 GOTO 20",
        );
        for size in [(240.0, 180.0), PANE, (700.0, 900.0)] {
            let mut h = Harness::new(1280.0, 1400.0);
            let mut term = ninsei_term::LineEditor::default();
            frame_with(&mut h, &g, size, &mut term, vec![]);
            frame_with(&mut h, &g, size, &mut term, vec![]);
            let room = pane_rect(size).expand(0.5);
            for s in h.painted() {
                let r = s.visual_bounding_rect();
                if room.contains(r.center()) {
                    assert!(room.contains_rect(r), "{size:?}: {r:?} outside {room:?}");
                }
            }
            let cut: Vec<_> = h
                .cut_text()
                .into_iter()
                .filter(|(_, r)| room.contains(r.min))
                .collect();
            assert!(cut.is_empty(), "{size:?}: cut at a clip rect: {cut:?}");
        }
    }

    #[test]
    fn the_line_about_to_run_is_inverse_a_stop_carries_its_block_and_the_watches_read() {
        let g = fight("10 RUN icepick\n20 STOP\n30 JUMP 10\n40 GOTO 10");
        let p = pal(&g);
        let mut h = Harness::new(1280.0, 1400.0);
        frame(&mut h, &g, vec![]);
        let rows = rows_in(&h, pane_rect(PANE));
        let (s, cs) = row_with(&rows, "RUNicepick");
        assert!(s.starts_with('▶'), "{s}");
        assert!(cs.iter().all(|&c| c == p.bg), "inverse: {cs:?}");
        assert!(
            h.rects().iter().any(|(_, fill, _)| *fill == p.fg),
            "the row's bar"
        );
        let (s, _) = row_with(&rows, "STOP");
        assert!(s.starts_with('■'), "{s}");
        let (s, cs) = row_with(&rows, "JUMP10");
        assert!(s.contains("?SYNTAX"), "{s}");
        assert!(cs.iter().all(|&c| c == p.dim));
        for name in [
            "WATCHES", "TURN", "CPU", "IFS", "INTEG", "TRACE", "NEXT", "ICE", "LAST",
        ] {
            assert!(
                rows.iter().any(|(s, _)| s.contains(name)),
                "{name}: {rows:?}"
            );
        }
        let c = g.combat.as_ref().unwrap();
        let note = format!(
            "CPU{}/{}RAM{}/{}",
            c.cpu, g.deck.cpu_max, c.ifs, g.deck.ram_max
        );
        assert!(
            rows.iter().any(|(s, _)| s.contains(&note)),
            "the budgets in the edge"
        );
    }

    #[test]
    fn the_legend_swaps_when_the_module_window_is_active() {
        let g = fight("10 RUN icepick\n20 STOP\n30 GOTO 10");
        let mut h = Harness::new(1280.0, 1400.0);
        frame(&mut h, &g, vec![]);
        let legend = |h: &Harness| -> String {
            rows_in(h, legend_rect())
                .into_iter()
                .map(|(s, _)| s)
                .collect()
        };
        let calm = legend(&h);
        assert!(
            calm.contains("F8Jack") && calm.contains("F10Endturn"),
            "{calm}"
        );
        assert!(!calm.contains("Step"));
        activate_on(&mut h, &g, 10);
        frame(&mut h, &g, vec![]);
        let active = legend(&h);
        for k in [
            "F1Help",
            "F2Edit",
            "F4Pause",
            "F8Step",
            "F9Run",
            "F10Endturn",
        ] {
            assert!(active.contains(k), "{k} in {active}");
        }
        assert!(
            !active.contains("F7") && !active.contains("Jack"),
            "{active}"
        );
        // Esc puts the suite's own legend back.
        frame(&mut h, &g, key(Key::Escape));
        frame(&mut h, &g, vec![]);
        assert!(legend(&h).contains("F8Jack"));
    }

    #[test]
    fn with_the_window_active_the_f_keys_are_the_debuggers() {
        let g = fight("10 RUN icepick\n20 STOP\n30 GOTO 10");
        let mut h = Harness::new(1280.0, 1400.0);
        activate_on(&mut h, &g, 10);
        assert_eq!(actions(frame(&mut h, &g, key(Key::F8))), [Action::Step]);
        assert_eq!(actions(frame(&mut h, &g, key(Key::F9))), [Action::Play]);
        assert!(
            actions(frame(&mut h, &g, key(Key::F4))).is_empty(),
            "nothing to pause"
        );
        assert!(
            frame(&mut h, &g, key(Key::F7)).is_empty(),
            "F7 is the browser's"
        );
        // The arrows move the window's cursor, not the sheet's.
        frame(&mut h, &g, key(Key::ArrowDown));
        assert_eq!(module().view(&h.ctx, &g).cursor, Some(20));
    }

    #[test]
    fn with_another_window_active_f4_and_f8_keep_their_meaning() {
        let g = fight("10 RUN icepick\n20 STOP\n30 GOTO 10");
        let mut h = Harness::new(1280.0, 1400.0);
        frame(&mut h, &g, vec![]);
        let out = frame(&mut h, &g, key(Key::F8));
        assert!(
            matches!(out.as_slice(), [Request::Type(t)] if t == "jack "),
            "F8 types jack"
        );
        let out = frame(&mut h, &g, key(Key::F4));
        assert_eq!(
            out.iter().any(|r| matches!(r, Request::OpenHex(_))),
            g.has_tool("hexed"),
            "F4 opens the hex editor when there is one"
        );
        assert!(actions(out).is_empty());
    }

    #[test]
    fn a_line_is_retyped_as_a_field_in_its_row() {
        let mut g = fight("10 RUN icepick\n20 STOP\n30 GOTO 10");
        let before = program(&g);
        let mut h = Harness::new(1280.0, 1400.0);
        activate_on(&mut h, &g, 20);
        let mut term = ninsei_term::LineEditor::default();
        frame_with(&mut h, &g, PANE, &mut term, key(Key::F2));
        frame_with(&mut h, &g, PANE, &mut term, vec![]);
        assert!(
            h.ctx.memory(|m| m.has_focus(module().edit_id())),
            "the row has the keys"
        );
        let mut ev = Vec::new();
        for _ in 0.."STOP".len() {
            ev.extend(key(Key::Backspace));
        }
        ev.extend(typed("RUN patch"));
        frame_with(&mut h, &g, PANE, &mut term, ev);
        assert_eq!(term.text(), "", "the command line heard none of it");
        let acts = actions(frame_with(&mut h, &g, PANE, &mut term, key(Key::Enter)));
        assert_eq!(acts.len(), 1, "{acts:?}");
        for a in acts {
            g.apply(a);
        }
        let after = program(&g);
        let differ: Vec<_> = before.iter().zip(&after).filter(|(a, b)| a != b).collect();
        assert_eq!(differ.len(), 1, "{after:?}");
        assert_eq!(after[1], "20 RUN patch");
    }

    #[test]
    fn esc_cancels_a_retype_and_enter_starts_one() {
        let g = fight("10 RUN icepick\n20 STOP\n30 GOTO 10");
        let mut h = Harness::new(1280.0, 1400.0);
        activate_on(&mut h, &g, 20);
        frame(&mut h, &g, key(Key::Enter));
        frame(&mut h, &g, typed("xyz"));
        assert!(actions(frame(&mut h, &g, key(Key::Escape))).is_empty());
        frame(&mut h, &g, vec![]);
        let rows = rows_in(&h, pane_rect(PANE));
        assert!(row_with(&rows, "20").0.ends_with("STOP"));
        assert!(module().has_keys(&h.ctx), "the window is active again");
    }

    #[test]
    fn ins_opens_a_numbered_line_and_del_deletes() {
        let mut g = fight("10 RUN icepick\n20 STOP\n30 GOTO 10");
        let mut h = Harness::new(1280.0, 1400.0);
        activate_on(&mut h, &g, 20);
        frame(&mut h, &g, key(Key::Insert));
        frame(&mut h, &g, typed("RUN patch"));
        for a in actions(frame(&mut h, &g, key(Key::Enter))) {
            g.apply(a);
        }
        assert_eq!(g.program.line(25).unwrap().text, "RUN patch");
        frame(&mut h, &g, vec![]);
        let acts = actions(frame(&mut h, &g, key(Key::Delete)));
        assert_eq!(acts.len(), 1);
        for a in acts {
            g.apply(a);
        }
        assert_eq!(program(&g), ["10 RUN icepick", "20 STOP", "30 GOTO 10"]);
    }

    #[test]
    fn each_step_of_a_pass_is_an_activity_row_with_its_reference() {
        let mut g = fight("10 IF INTEG > 0\n20 RUN icepick\n30 GOTO 10");
        g.apply(Action::Finish);
        let mut h = Harness::new(1280.0, 1400.0);
        frame(&mut h, &g, vec![]);
        frame(&mut h, &g, vec![]);
        let log: Vec<String> = h
            .painted_text()
            .into_iter()
            .filter(|(_, r)| r.top() > 950.0 && r.top() < 1250.0)
            .map(|(t, _)| t)
            .collect();
        assert!(log.iter().any(|t| t == "PRG"), "{log:?}");
        assert!(
            log.iter().any(|t| t.starts_with("10 IF INTEG > 0")),
            "{log:?}"
        );
    }

    #[test]
    fn a_narrow_module_window_cuts_the_hang_before_the_budgets() {
        let mut g = fight("10 GOTO 20\n20 GOTO 10");
        for _ in 0..3 {
            g.apply(Action::Step);
        }
        let mut h = Harness::new(1280.0, 800.0);
        let mut term = ninsei_term::LineEditor::default();
        // Room for the budgets, not for the hang after them.
        let size = (280.0, 400.0);
        frame_with(&mut h, &g, size, &mut term, vec![]);
        frame_with(&mut h, &g, size, &mut term, vec![]);
        let rows = rows_in(&h, pane_rect(size));
        let edge = rows
            .iter()
            .map(|(s, _)| s.replace(' ', ""))
            .find(|s| s.contains("RAM"))
            .unwrap_or_else(|| panic!("{rows:?}"));
        let ram = format!("RAM{}/{}", g.ifs_left(), g.deck.ram_max);
        assert!(edge.contains(&ram), "the budgets whole: {edge}");
        assert!(
            edge.contains('…') && !edge.contains("IN10"),
            "the hang cut: {edge}"
        );
    }

    #[test]
    fn a_hang_shows_in_the_module_windows_edge_and_its_last_watch() {
        let mut g = fight("10 GOTO 20\n20 GOTO 10");
        for _ in 0..3 {
            g.apply(Action::Step);
        }
        assert_eq!(g.exec.broke, Some(10));
        let mut h = Harness::new(1280.0, 800.0);
        frame(&mut h, &g, vec![]);
        frame(&mut h, &g, vec![]);
        let rows = rows_in(&h, pane_rect(PANE));
        let flat: Vec<String> = rows.iter().map(|(s, _)| s.replace(' ', "")).collect();
        assert!(
            flat.iter().filter(|s| s.contains("?BREAKIN10")).count() >= 2,
            "the edge and LAST: {rows:?}"
        );
        assert!(
            flat.iter()
                .any(|s| s.contains('▶') && s.contains("10GOTO20")),
            "the ▶ held on line 10: {rows:?}"
        );
        g.apply(Action::Step);
        frame(&mut h, &g, vec![]);
        let rows = rows_in(&h, pane_rect(PANE));
        assert!(
            !rows
                .iter()
                .any(|(s, _)| s.replace(' ', "").contains("?BREAK")),
            "{rows:?}"
        );
    }
}
