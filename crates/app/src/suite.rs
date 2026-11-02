//! The Hosaka's chrome: a 90s productivity suite. Every pane is a document window (a filled
//! title bar, a double border, a drop shadow, its number in the corner) on a plain desk.
//! deck.sys is a form with report charts, warez/ a spreadsheet, target.ice an incident report,
//! the terminal a ledger and todo a table of work orders. An F-key legend sits under the panes
//! and dialogs pop up for the moments worth filing. Calm by design: no glow, no spin. Every
//! colour is a theme role: title bars `panel`, shadows `bg`, selections in reverse video. The
//! rules (sorting, formulas, axes, dialogs) are `ninsei_core::suite`.

use egui::{
    Align2, Color32, FontFamily, FontId, Key, Painter, Rect, RichText, ScrollArea, Sense, Stroke,
    StrokeKind, Ui, UiBuilder, pos2, vec2,
};
use ninsei_core::suite::{self as rules, Notice, SheetCol, Watch};
use ninsei_core::{Action, Command, ContractStatus, Effect, Game, LogKind, Phase, tuning};

use crate::chrome::{ChromeView, FrameCx, MenuPart, Menus, act};
use crate::panes::{Panes, Request, Tab};
use crate::theme::Palette;

/// Sizes for the suite, in one place. Display numbers, not balance.
pub mod size {
    pub const MONO: f32 = 12.0;
    pub const SMALL: f32 = 10.0;
    pub const TITLE_H: f32 = 18.0;
    /// How far a window's shadow falls.
    pub const SHADOW: f32 = 5.0;
    pub const ROW_H: f32 = 17.0;
    /// Spreadsheet column widths, in characters (row numbers, then `SheetCol::ALL`).
    pub const ROWNUM_CH: f32 = 3.0;
    pub const COL_CH: [f32; 6] = [5.0, 14.0, 5.0, 5.0, 5.0, 8.0];
    /// Report charts take this share of a window's height, within these bounds.
    pub const CHART_SHARE: f32 = 0.3;
    pub const CHART_MIN: f32 = 56.0;
    pub const CHART_MAX: f32 = 150.0;
    /// Upcoming turns the incident report's agenda lists.
    pub const AGENDA_ROWS: usize = 5;
    pub const DIALOG_W: f32 = 420.0;
}

/// What the suite remembers between frames: the spreadsheet's cursor (by program uid, since the
/// HDD reshuffles), its sort and filter, and the dialogs waiting to be dismissed.
#[derive(Default)]
pub struct State {
    pub sel: Option<u32>,
    pub col: SheetCol,
    pub sort: SheetCol,
    pub desc: bool,
    pub loaded_only: bool,
    pub watch: Watch,
    pub dialogs: Vec<Notice>,
}

impl State {
    /// The sheet's rows in order.
    fn rows(&self, g: &Game) -> Vec<usize> {
        rules::sheet_rows(&g.hdd, self.sort, !self.desc, self.loaded_only)
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

/// A document window filling the pane: shadow, double border, a title bar with the window's
/// number, and `body` inside.
pub fn window(ui: &mut Ui, p: &Palette, num: usize, title: &str, body: impl FnOnce(&mut Ui)) {
    let full = ui.available_rect_before_wrap();
    let painter = ui.painter().clone();
    painter.rect_filled(full, 0.0, p.panel);
    let win = Rect::from_min_max(
        full.min + vec2(4.0, 4.0),
        full.max - vec2(4.0 + size::SHADOW, 4.0 + size::SHADOW),
    );
    painter.rect_filled(win.translate(vec2(size::SHADOW, size::SHADOW)), 0.0, p.bg);
    painter.rect_filled(win, 0.0, p.bg);
    painter.rect_stroke(win, 0.0, Stroke::new(1.0, p.fg), StrokeKind::Inside);
    painter.rect_stroke(
        win.shrink(3.0),
        0.0,
        Stroke::new(1.0, p.dim),
        StrokeKind::Inside,
    );
    let bar = Rect::from_min_size(
        win.min + vec2(4.0, 4.0),
        vec2(win.width() - 8.0, size::TITLE_H),
    );
    painter.rect_filled(bar, 0.0, p.panel);
    let clip = painter.with_clip_rect(bar);
    clip.text(
        bar.left_center() + vec2(4.0, 0.0),
        Align2::LEFT_CENTER,
        "[■]",
        mono(size::SMALL),
        p.dim,
    );
    clip.text(
        bar.center(),
        Align2::CENTER_CENTER,
        title,
        mono(size::MONO),
        p.bright,
    );
    clip.text(
        bar.right_center() - vec2(4.0, 0.0),
        Align2::RIGHT_CENTER,
        format!("[{num}]"),
        mono(size::SMALL),
        p.dim,
    );
    let inner = Rect::from_min_max(
        pos2(win.left() + 9.0, bar.bottom() + 6.0),
        win.max - vec2(9.0, 7.0),
    );
    if inner.width() > 8.0 && inner.height() > 8.0 {
        ui.scope_builder(
            UiBuilder::new()
                .max_rect(inner)
                .layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| {
                ui.set_clip_rect(inner.intersect(ui.clip_rect()));
                // Solid scroll bars, always there, like any document window.
                // The thumb in the text colour on a `panel` track: visible on the window's `bg`.
                ui.spacing_mut().scroll = egui::style::ScrollStyle {
                    foreground_color: true,
                    ..egui::style::ScrollStyle::solid()
                };
                ui.visuals_mut().extreme_bg_color = p.panel;
                body(ui);
            },
        );
    }
    ui.allocate_rect(full, Sense::hover());
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
fn x_labels(painter: &Painter, plot: Rect, p: &Palette, n: usize, label: impl Fn(usize) -> String) {
    if n == 0 {
        return;
    }
    let slot = plot.width() / n as f32;
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
    x_labels(&painter, plot, p, values.len(), |i| format!("H{i}"));
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
    let slot = plot.width() / n as f32;
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
    x_labels(&painter, plot, p, n, label);
}

/// A form: labelled fields with a one-line note each.
fn form(ui: &mut Ui, p: &Palette, id: &str, rows: &[(&str, String, &str, bool)]) {
    egui::Grid::new(id)
        .num_columns(3)
        .spacing([10.0, 3.0])
        .show(ui, |ui| {
            for (label, value, note, hot) in rows {
                ui.label(t(*label, p.dim));
                ui.label(t(
                    format!("[ {value} ]"),
                    if *hot { p.bright } else { p.fg },
                ));
                ui.label(small(*note, p.dim));
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
                        format!("{} / {}", g.ram_used(), d.ram_max),
                        "programs loaded in RAM",
                        g.ram_used() == d.ram_max,
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
                        ("RAM", g.ram_used(), d.ram_max, g.ram_used() == d.ram_max),
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
        window(ui, &p, Tab::Warez.number(), "WAREZ.WK1", |ui| {
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
            ui.horizontal(|ui| {
                let mark = if st.loaded_only { "[x]" } else { "[ ]" };
                if ui
                    .add(
                        egui::Label::new(t(format!("{mark} LOADED ONLY"), p.fg))
                            .sense(Sense::click()),
                    )
                    .clicked()
                {
                    st.loaded_only = !st.loaded_only;
                }
                ui.label(small(
                    "click a header to sort, double-click a row to load, run or unload",
                    p.dim,
                ));
            });
            let cw = ui.fonts_mut(|f| f.glyph_width(&mono(size::MONO), 'M'));
            let widths: Vec<f32> = std::iter::once(size::ROWNUM_CH)
                .chain(size::COL_CH)
                .map(|c| c * cw + 8.0)
                .collect();
            let total_w: f32 = widths.iter().sum();
            let foot_h = size::ROW_H * 2.0 + 6.0;
            ScrollArea::both()
                .id_salt("suite-sheet")
                .auto_shrink([false, false])
                .max_height((ui.available_height() - foot_h).max(size::ROW_H * 3.0))
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                .show(ui, |ui| {
                    // Column letters, then row 1: the headers. Either sorts.
                    for header in [true, false] {
                        let (r, resp) =
                            ui.allocate_exact_size(vec2(total_w, size::ROW_H), Sense::click());
                        let painter = ui.painter_at(r);
                        let mut x = r.left();
                        let mut hit = None;
                        for (k, w) in widths.iter().enumerate() {
                            let cell = Rect::from_min_size(pos2(x, r.top()), vec2(*w, r.height()));
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
                            painter.text(
                                cell.center(),
                                Align2::CENTER_CENTER,
                                text,
                                mono(size::MONO),
                                if header { p.dim } else { p.bright },
                            );
                            if resp
                                .interact_pointer_pos()
                                .is_some_and(|q| cell.contains(q))
                            {
                                hit = Some(k);
                            }
                            x += w;
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
                        let (r, resp) =
                            ui.allocate_exact_size(vec2(total_w, size::ROW_H), Sense::click());
                        let painter = ui.painter_at(r);
                        let mut x = r.left();
                        let mut hit = None;
                        for (k, w) in widths.iter().enumerate() {
                            let cell = Rect::from_min_size(pos2(x, r.top()), vec2(*w, r.height()));
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
                            let (text, col, align) = if k == 0 {
                                (
                                    (n + 2).to_string(),
                                    if on { p.bg } else { p.dim },
                                    Align2::CENTER_CENTER,
                                )
                            } else {
                                let c = SheetCol::ALL[k - 1];
                                let colour = if cursor {
                                    p.bg
                                } else if on {
                                    p.bright
                                } else if s.loaded {
                                    p.fg
                                } else {
                                    p.dim
                                };
                                let align = match c {
                                    SheetCol::File | SheetCol::Status => Align2::LEFT_CENTER,
                                    _ => Align2::RIGHT_CENTER,
                                };
                                (c.cell(slot, s), colour, align)
                            };
                            let at = match align {
                                Align2::LEFT_CENTER => cell.left_center() + vec2(4.0, 0.0),
                                Align2::RIGHT_CENTER => cell.right_center() - vec2(4.0, 0.0),
                                _ => cell.center(),
                            };
                            painter.with_clip_rect(cell).text(
                                at,
                                align,
                                text,
                                mono(size::MONO),
                                col,
                            );
                            if resp
                                .interact_pointer_pos()
                                .is_some_and(|q| cell.contains(q))
                            {
                                hit = Some(k);
                            }
                            x += w;
                        }
                        if resp.clicked() || resp.secondary_clicked() {
                            st.select(g, slot);
                            if let Some(k) = hit.filter(|k| *k > 0) {
                                st.col = SheetCol::ALL[k - 1];
                            }
                        }
                        if resp.double_clicked() {
                            acts.push(if !s.loaded {
                                act(Action::Load(slot))
                            } else if fighting {
                                act(Action::Run(slot))
                            } else {
                                act(Action::Unload(slot))
                            });
                        }
                        let w = &s.spec;
                        resp.on_hover_text(format!(
                            "{}\n{}\n{}",
                            w.name,
                            g.data.warez[s.warez].blurb,
                            w.effect.describe()
                        ))
                        .context_menu(|ui| {
                            if !s.loaded && ui.button("Load        F2").clicked() {
                                acts.push(act(Action::Load(slot)));
                            }
                            if s.loaded && ui.button("Unload      F3").clicked() {
                                acts.push(act(Action::Unload(slot)));
                            }
                            if s.loaded
                                && fighting
                                && ui
                                    .add_enabled(cpu_left >= w.cpu, egui::Button::new("Run"))
                                    .clicked()
                            {
                                acts.push(act(Action::Run(slot)));
                            }
                            if ui.button("Open in hexedit  F4").clicked() {
                                acts.push(Request::OpenHex(slot));
                            }
                        });
                    }
                    if rows.is_empty() {
                        ui.label(t("  (no rows)", p.dim));
                    }
                });
            // The totals row.
            let ram = g.ram_used();
            ui.horizontal(|ui| {
                ui.label(t(
                    format!("RAM USED {ram} / {}", g.deck.ram_max),
                    if ram == g.deck.ram_max {
                        p.bright
                    } else {
                        p.fg
                    },
                ));
                ui.label(t(
                    format!("   HDD {} / {}", g.hdd.len(), g.deck.hdd_max),
                    p.dim,
                ));
            });
            if fighting {
                ui.label(small(
                    format!("loading mid-fight costs {} cpu", tuning::COMBAT_LOAD_CPU),
                    p.dim,
                ));
            }
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
            let ch = chart_h(ui.available_height());
            page(ui, "suite-incident", |ui| {
                let severity = match ice.tier {
                    0 => "LOW",
                    1 => "MODERATE",
                    _ => "CRITICAL",
                };
                let intent = g.intent().map_or_else(String::new, |i| i.describe());
                let next = if c.stunned {
                    format!("{intent} (deferred)")
                } else {
                    intent
                };
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
                        ("NEXT ACTION ", next, "", true),
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
                heading(ui, &p, "AGENDA");
                let n = ice.intents.len().max(1);
                egui::Grid::new("suite-agenda")
                    .num_columns(2)
                    .spacing([16.0, 2.0])
                    .show(ui, |ui| {
                        ui.label(t("TURN", p.dim));
                        ui.label(t("PLANNED ACTION", p.dim));
                        ui.end_row();
                        for k in 0..size::AGENDA_ROWS {
                            let intent = &ice.intents[(c.intent_ix + k) % n];
                            let turn = format!("{:>4}", c.turn as usize + k);
                            if k == 0 {
                                ui.label(t(turn, p.bg).background_color(p.fg));
                                ui.label(t(intent.describe(), p.bg).background_color(p.fg));
                            } else {
                                ui.label(t(turn, p.dim));
                                ui.label(t(intent.describe(), p.fg));
                            }
                            ui.end_row();
                        }
                    });
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    for (i, s) in g.hdd.iter().enumerate().filter(|(_, s)| s.loaded) {
                        let w = &s.spec;
                        let class_hit =
                            matches!(w.effect, Effect::Break { vs: Some(v), .. } if v == ice.class);
                        let label = format!(
                            "[ Run {} ({}c){} ]",
                            w.file(),
                            w.cpu,
                            if class_hit { " x2" } else { "" }
                        );
                        if ui
                            .add_enabled(
                                c.cpu >= w.cpu,
                                egui::Button::new(t(
                                    label,
                                    if class_hit { p.bright } else { p.fg },
                                )),
                            )
                            .on_hover_text(w.effect.describe())
                            .clicked()
                        {
                            acts.push(Action::Run(i));
                        }
                    }
                    if ui.button(t("[ End Turn  F10 ]", p.bright)).clicked() {
                        acts.push(Action::EndTurn);
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
            let cw = ui.fonts_mut(|f| f.glyph_width(&mono(size::MONO), 'M'));
            let cols = [9.0 * cw, 5.0 * cw];
            let header = |ui: &mut Ui, a: &str, b: &str, c: &str, col: Color32| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.add_sized([cols[0], size::ROW_H - 3.0], egui::Label::new(t(a, col)));
                    ui.add_sized([cols[1], size::ROW_H - 3.0], egui::Label::new(t(b, col)));
                    ui.label(t(c, col));
                });
            };
            header(ui, "TIME", "REF", "ENTRY", p.dim);
            let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 2.0), Sense::hover());
            ui.painter()
                .line_segment([r.left_center(), r.right_center()], Stroke::new(1.0, p.dim));
            let input_h = size::MONO + 16.0;
            ScrollArea::vertical()
                .id_salt("suite-log")
                .stick_to_bottom(true)
                .auto_shrink([false, false])
                .max_height(ui.available_height() - input_h)
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for l in &g.log {
                        let col = match l.kind {
                            LogKind::Sys => p.dim,
                            LogKind::Info | LogKind::Good => p.fg,
                            LogKind::Bad | LogKind::Loot => p.bright,
                        };
                        let row = ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            ui.add_sized(
                                [cols[0], size::ROW_H],
                                egui::Label::new(t(format!("{:03}.{:02}", l.hop, l.turn), p.dim)),
                            );
                            ui.add_sized(
                                [cols[1], size::ROW_H],
                                egui::Label::new(t(rules::log_ref(l.kind), col)),
                            );
                            ui.label(t(self.garble(&l.text), col));
                        });
                        let r = row.response.rect;
                        ui.painter().line_segment(
                            [r.left_bottom(), pos2(ui.max_rect().right(), r.bottom())],
                            Stroke::new(1.0, p.faint),
                        );
                    }
                });
            ui.add_space(3.0);
            self.prompt_as(ui, "Command:", true);
        });
        pane.append(&mut self.out);
        self.out = pane;
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
                                ContractStatus::Active => ("[ ]", "OPEN", p.fg),
                                ContractStatus::Done => ("[x]", "PAID", p.dim),
                                ContractStatus::Failed => ("[-]", "VOID", p.faint),
                                ContractStatus::Offered => ("[?]", "OFFERED", p.dim),
                            };
                            ui.label(t(mark, col));
                            ui.label(t(format!("WO-{:04}", 1001 + i), col));
                            ui.label(t(&k.client, col)).on_hover_text(&k.brief);
                            ui.label(t(&n.name, col));
                            ui.label(t(format!("{here}/{}", n.layer), col));
                            ui.label(t(status, col));
                            ui.label(t(
                                format!("{}¢", k.reward),
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

/// The F-key legend: what each key does, wired to the same requests as the keys.
const FKEYS: [(Key, &str, &str); 6] = [
    (Key::F1, "F1", "Help"),
    (Key::F2, "F2", "Load"),
    (Key::F3, "F3", "Unload"),
    (Key::F4, "F4", "Hex"),
    (Key::F8, "F8", "Jack"),
    (Key::F10, "F10", "End turn"),
];

/// What F-key `k` asks for, given the sheet's cursor.
fn fkey(k: Key, st: &State, g: &Game) -> Option<Request> {
    let slot = st.picked(g);
    Some(match k {
        Key::F1 => Request::Cmd(Command::Help),
        Key::F2 => act(Action::Load(slot?)),
        Key::F3 => act(Action::Unload(slot?)),
        Key::F4 => Request::OpenHex(slot?),
        Key::F8 => Request::Type("jack ".into()),
        Key::F10 => act(Action::EndTurn),
        _ => return None,
    })
}

const SUITE_MENUS: Menus = &[
    ("File", &[MenuPart::Decks, MenuPart::Monitors]),
    ("Edit", &[MenuPart::Glitches, MenuPart::Toggles]),
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
        window(ui, &pal, Tab::Network.number(), "PLANVIEW.DWG", |ui| {
            p.network(ui)
        });
    }
    fn target(&self, p: &mut Panes, ui: &mut Ui) {
        p.suite_target(ui);
    }
    fn hex(&self, p: &mut Panes, ui: &mut Ui) {
        let pal = p.pal;
        window(ui, &pal, Tab::Hex.number(), "HEXEDIT.EXE", |ui| {
            p.hex_pane(ui)
        });
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
        })
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
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.label(t(format!(" {mode} "), p.bg).background_color(if hot {
                p.bright
            } else {
                p.fg
            }));
            let sep = |ui: &mut Ui| {
                ui.label(t("  │  ", p.dim));
            };
            ui.label(t(
                format!("  {}  HOP {}", rules::date(g.seed), g.hops),
                p.fg,
            ));
            sep(ui);
            ui.label(t(format!("USER {}", rules::user_id(g.seed)), p.fg));
            sep(ui);
            ui.label(t(
                rules::doc_name(g.site.as_ref().map(|s| s.name.as_str())),
                p.bright,
            ));
            sep(ui);
            ui.label(t(format!("NODE {} OF {n}", g.at + 1), p.fg));
            sep(ui);
            let trace_hot = g.deck.trace * 4 >= tuning::TRACE_MAX * 3;
            ui.label(t(
                format!("TRACE {}%", g.deck.trace),
                if trace_hot { p.bright } else { p.fg },
            ));
        });
        true
    }

    fn has_legend(&self) -> bool {
        true
    }
    fn legend(&self, ui: &mut Ui, cx: &mut FrameCx) -> Vec<Request> {
        let p = cx.pal;
        let mut out = Vec::new();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for (k, cap, label) in FKEYS {
                let r = ui.add(
                    egui::Label::new(t(cap, p.bg).background_color(p.fg)).sense(Sense::click()),
                );
                let l =
                    ui.add(egui::Label::new(t(format!("{label}  "), p.fg)).sense(Sense::click()));
                if (r.clicked() || l.clicked())
                    && let Some(req) = fkey(k, cx.suite, cx.game)
                {
                    out.push(req);
                }
            }
        });
        out
    }

    fn keys(&self, ctx: &egui::Context, cx: &mut FrameCx) -> Vec<Request> {
        if !cx.keys_free {
            return Vec::new();
        }
        let g = cx.game;
        let st = &mut *cx.suite;
        let mut out = Vec::new();
        let (pressed, arrows) = ctx.input(|i| {
            let f: Vec<Key> = FKEYS
                .iter()
                .map(|(k, _, _)| *k)
                .filter(|k| i.key_pressed(*k))
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
            out.extend(fkey(k, st, g));
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

    fn overlay(&self, ctx: &egui::Context, cx: &mut FrameCx) -> Vec<Request> {
        let st = &mut *cx.suite;
        // A new run drops anything left from the last one.
        if !st.watch.watching(cx.game) {
            st.dialogs.clear();
        }
        st.dialogs.extend(st.watch.step(cx.game));
        let Some(n) = st.dialogs.first().cloned() else {
            return Vec::new();
        };
        let p = cx.pal;
        let waiting = st.dialogs.len();
        let mut ok = cx.keys_free && ctx.input(|i| i.key_pressed(Key::Enter));
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
                        ui.set_width(size::DIALOG_W);
                        let (bar, _) = ui.allocate_exact_size(
                            vec2(size::DIALOG_W, size::TITLE_H + 2.0),
                            Sense::hover(),
                        );
                        ui.painter().rect_filled(bar, 0.0, p.panel);
                        ui.painter().text(
                            bar.center(),
                            Align2::CENTER_CENTER,
                            &n.title,
                            mono(size::MONO),
                            p.bright,
                        );
                        egui::Frame::new()
                            .inner_margin(egui::Margin::symmetric(14, 10))
                            .show(ui, |ui| {
                                for l in &n.lines {
                                    ui.label(t(l, p.fg));
                                }
                                ui.add_space(8.0);
                                ui.horizontal(|ui| {
                                    if waiting > 1 {
                                        ui.label(small(format!("1 of {waiting}"), p.dim));
                                    }
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui.button(t("[  OK  ]", p.bright)).clicked() {
                                                ok = true;
                                            }
                                            ui.label(small("Enter", p.dim));
                                        },
                                    );
                                });
                            });
                    });
            });
        if ok {
            st.dialogs.remove(0);
        }
        Vec::new()
    }
}
