//! The Ono-Sendai's chrome: btop-style panes. Boxes with their title cut into the border,
//! gradient meters, dot graphs at braille resolution (the font has no braille, so the dots are
//! painted), and waterfalls. Every colour comes from the theme: gradients run
//! `faint → dim → fg → bright`. History comes from `Game::history`; the maths from
//! `ninsei_core::history::plot`.

use egui::{
    Align2, Color32, FontFamily, FontId, Painter, Rect, RichText, ScrollArea, Sense, Stroke, Ui,
    UiBuilder, pos2, vec2,
};
use ninsei_core::history::plot;
use ninsei_core::{Action, ContractStatus, Effect, LogKind, Phase, tuning};

use crate::kit::Cells;
use crate::kit::cells::{Unlit, cut};
use crate::netmap::mix;
use crate::panes::Panes;
use crate::theme::Palette;

/// Timings and sizes for the btop chrome, in one place. Display numbers, not balance.
pub mod anim {
    /// How long meters and counters take to ease to a new value, in seconds.
    pub const EASE_SECS: f32 = 0.22;
    /// Points in a meter's sparkline.
    pub const SPARK_POINTS: usize = 24;
    /// Rows of the damage waterfall shown.
    pub const WATERFALL_ROWS: usize = 6;
    /// Upcoming ICE intents shown in the intent waterfall.
    pub const INTENTS_AHEAD: usize = 4;
    /// Actions a program's row stays lit after it runs.
    pub const RUN_GLOW_TICKS: u32 = 2;
}

const MONO: f32 = 12.0;

fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

/// The gradient every meter and graph uses, 0..1 along its length or height.
pub fn ramp(p: &Palette, k: f32) -> Color32 {
    let k = k.clamp(0.0, 1.0) * 3.0;
    match k {
        k if k < 1.0 => mix(p.faint, p.dim, k),
        k if k < 2.0 => mix(p.dim, p.fg, k - 1.0),
        k => mix(p.fg, p.bright, k - 2.0),
    }
}

/// The chrome's cells: 12pt with rows a little taller than the glyphs.
fn cells_in(ui: &Ui, area: Rect) -> Cells {
    Cells::fit_font(ui, area, mono(MONO), Some(MONO + 2.0))
}

/// A btop box filling the pane, on the chrome's cells: rounded border through the outer
/// cells' centres, the title turned into the top edge (`┐title┌`) and an optional note into
/// the bottom one. `body` draws inside.
pub fn boxed(ui: &mut Ui, p: &Palette, title: &str, note: &str, body: impl FnOnce(&mut Ui)) {
    boxed_lit(ui, p, title, note, false, body);
}

/// [`boxed`], its border lit `bright` while the tile has the keyboard, as the tiling lights
/// the focused tile.
pub fn boxed_lit(
    ui: &mut Ui,
    p: &Palette,
    title: &str,
    note: &str,
    lit: bool,
    body: impl FnOnce(&mut Ui),
) {
    let room = ui.available_rect_before_wrap();
    let mut g = cells_in(ui, room.shrink(3.0));
    (g.border, g.paper) = (if lit { p.bright } else { p.dim }, p.bg);
    let painter = ui.painter().clone();
    g.frame(&painter, 0, 0, g.cols, g.rows);
    // Title and note cut with … to the room the border has, never dropped.
    let title = cut(title, g.cols.saturating_sub(6));
    g.title(&painter, 2, 0, &title, p.bright);
    let spare = g.cols.saturating_sub(title.chars().count() + 10);
    let note = cut(note, spare);
    let n = note.chars().count();
    if n > 0 {
        g.note(&painter, g.cols - n - 4, g.rows - 1, &note, p.dim);
    }
    // The body sits where it always did: 10px and 12px inside the border line; in a box too
    // small for that, it still gets the room (it may hold the only command line on screen).
    let line = g.at(0, 0, 1, 1).center();
    let end = g.at(g.cols - 1, g.rows - 1, 1, 1).center();
    let inner = Rect::from_min_max(line, end).shrink2(vec2(10.0, 12.0));
    let inner = if inner.is_positive() { inner } else { room };
    ui.scope_builder(UiBuilder::new().max_rect(inner), body);
    ui.allocate_rect(room, Sense::hover());
}

/// A gradient meter `cells` wide on the chrome's cells: lit cells take the ramp's colour at
/// their place, the rest are dim dots. The value eases toward its target.
fn meter(ui: &mut Ui, p: &Palette, id: &str, value: u32, max: u32, cells: usize) {
    let shown = ui.ctx().animate_value_with_time(
        ui.id().with(("meter", id)),
        value as f32,
        anim::EASE_SECS,
    );
    let (resp, g) = Cells::allocate(
        ui,
        mono(MONO),
        Some(MONO + 2.0),
        (cells, 1),
        egui::Vec2::ZERO,
        Sense::hover(),
    );
    let lit = if max == 0 {
        0.0
    } else {
        (shown / max as f32).clamp(0.0, 1.0) * cells as f32
    };
    g.gauge(
        &ui.painter_at(resp.rect),
        0,
        0,
        cells,
        lit,
        |i| ramp(p, (i as f32 + 0.5) / cells as f32),
        Unlit::Dot(p.faint),
    );
}

/// Paints a dot graph in `rect`: one dot column per value, `rows` cells (4 dots each) tall,
/// coloured by height. `marks` draws dashed lines at those values (e.g. 75% and 100% trace).
fn dots(
    painter: &Painter,
    rect: Rect,
    p: &Palette,
    values: &[u32],
    max: u32,
    rows: usize,
    marks: &[u32],
) {
    let cols = ((rect.width() / 4.0).floor() as usize).max(1);
    let height = rows * 4;
    let (dx, dy) = (rect.width() / cols as f32, rect.height() / height as f32);
    let r = (dx.min(dy) * 0.32).max(0.8);
    for (c, h) in plot::heights(values, max, cols, height)
        .into_iter()
        .enumerate()
    {
        let Some(h) = h else { continue };
        for d in 0..h {
            let y = rect.bottom() - (d as f32 + 0.5) * dy;
            let x = rect.left() + (c as f32 + 0.5) * dx;
            painter.circle_filled(pos2(x, y), r, ramp(p, (d as f32 + 1.0) / height as f32));
        }
    }
    for &m in marks {
        let y = rect.bottom() - plot::dots(m, max, height) as f32 * dy;
        painter.extend(egui::Shape::dashed_line(
            &[pos2(rect.left(), y), pos2(rect.right(), y)],
            Stroke::new(1.0, p.faint),
            3.0,
            3.0,
        ));
    }
}

/// A one-row sparkline, `width` pixels wide.
fn spark(ui: &mut Ui, p: &Palette, values: &[u32], max: u32, width: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, MONO), Sense::hover());
    dots(&ui.painter_at(rect), rect, p, values, max, 1, &[]);
}

/// A dot graph `rows` cells tall filling the width, with a label in its corner.
fn graph(
    ui: &mut Ui,
    p: &Palette,
    label: &str,
    values: &[u32],
    max: u32,
    rows: usize,
    marks: &[u32],
) {
    let h = rows as f32 * (MONO + 2.0);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_stroke(
        rect,
        2.0,
        Stroke::new(1.0, p.faint),
        egui::StrokeKind::Middle,
    );
    // A short history is stretched, so the first few points read as bars, not a sliver.
    let cols = (rect.width() / 4.0).floor().max(1.0) as usize;
    let k = (cols / values.len().max(1)).clamp(1, 6);
    let wide: Vec<u32> = values
        .iter()
        .flat_map(|&v| std::iter::repeat_n(v, k))
        .collect();
    dots(&painter, rect.shrink(2.0), p, &wide, max, rows, marks);
    painter.text(
        rect.left_top() + vec2(4.0, 2.0),
        Align2::LEFT_TOP,
        label,
        mono(10.0),
        p.dim,
    );
}

/// A program's name in the warez table: its 8.3 file, and `+On` when it's optimized.
fn warez_name(w: &ninsei_core::binary::Spec) -> String {
    if w.opt > 0 {
        format!("{}+O{}", w.file(), w.opt)
    } else {
        w.file()
    }
}

/// The warez table's columns: ID, NAME as wide as the widest name, then CPU while there's
/// room for it. Names are never sliced: the number goes first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WarezCols {
    name: usize,
    cpu: bool,
}

impl WarezCols {
    /// The columns for names up to `name` cells wide in a row of `room` cells.
    fn fit(name: usize, room: usize) -> Self {
        let name = name.max(4);
        let base = 3 + name + 1;
        WarezCols {
            name,
            cpu: room >= base + 4,
        }
    }

    fn line(&self, id: &str, name: &str, cpu: &str) -> String {
        let mut s = format!("{id:<3}{name:<w$} ", w = self.name);
        if self.cpu {
            s.push_str(&format!("{cpu:>4}"));
        }
        s
    }
}

/// One meter line in deck.sys.
struct MeterRow {
    label: &'static str,
    value: u32,
    max: u32,
    note: String,
    /// Its recent history, for the sparkline (empty: no sparkline).
    series: Vec<u32>,
    hot: bool,
}

fn text(t: impl Into<String>, c: Color32) -> RichText {
    RichText::new(t).font(mono(MONO)).color(c)
}

impl Panes<'_> {
    /// deck.sys: meters with sparklines, a trace-per-hop graph, rolling counters.
    pub(crate) fn btop_deck(&mut self, ui: &mut Ui) {
        let g = self.game;
        let p = self.pal;
        let d = &g.deck;
        let h = &g.history;
        let note = format!("seed {:016x}", g.seed);
        let deck = self.deck.to_uppercase();
        boxed(ui, &p, "deck.sys", &note, |ui| {
            ui.label(text(deck, p.bright));
            ui.add_space(4.0);
            let cpu = g.combat.as_ref().map_or(d.cpu_max, |c| c.cpu);
            let trace_hot = d.trace >= tuning::TRACE_MAX * 3 / 4;
            let rows = [
                MeterRow {
                    label: "CPU",
                    value: cpu,
                    max: d.cpu_max,
                    note: format!("{cpu}/{} cyc", d.cpu_max),
                    series: h.series(|s| s.cpu),
                    hot: false,
                },
                MeterRow {
                    label: "RAM",
                    value: g.ifs_left(),
                    max: d.ram_max,
                    note: format!("{}/{} GB", g.ifs_left(), d.ram_max),
                    series: h.series(|s| s.ram),
                    hot: false,
                },
                MeterRow {
                    label: "HDD",
                    value: g.hdd.len() as u32,
                    max: d.hdd_max,
                    note: format!("{}/{} slots", g.hdd.len(), d.hdd_max),
                    series: Vec::new(),
                    hot: false,
                },
                MeterRow {
                    label: "INTEG",
                    value: d.integrity,
                    max: d.integrity_max,
                    note: format!("{}/{}", d.integrity, d.integrity_max),
                    series: h.series(|s| s.integrity),
                    hot: d.integrity * 3 <= d.integrity_max,
                },
                MeterRow {
                    label: "TRACE",
                    value: d.trace,
                    max: tuning::TRACE_MAX,
                    note: format!("{}%", d.trace),
                    series: h.series(|s| s.trace),
                    hot: trace_hot,
                },
            ];
            for MeterRow {
                label,
                value: v,
                max,
                note,
                series,
                hot,
            } in rows
            {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    ui.label(text(
                        format!("{label:<5}"),
                        if hot { p.bright } else { p.dim },
                    ));
                    meter(ui, &p, label, v, max, 12);
                    ui.label(text(
                        format!("{note:<10}"),
                        if hot { p.bright } else { p.fg },
                    ));
                    // The sparkline only when it fits; in a narrow pane it would run past the box.
                    if !series.is_empty() && ui.available_width() >= anim::SPARK_POINTS as f32 * 2.0
                    {
                        let pts: Vec<u32> = series
                            .iter()
                            .rev()
                            .take(anim::SPARK_POINTS)
                            .rev()
                            .copied()
                            .collect();
                        spark(ui, &p, &pts, max, anim::SPARK_POINTS as f32 * 2.0);
                    }
                });
            }
            if let Some(c) = &g.combat
                && c.shield > 0
            {
                ui.horizontal(|ui| {
                    ui.label(text("SHLD ", p.dim));
                    meter(ui, &p, "SHIELD", c.shield, c.shield.max(10), 12);
                    ui.label(text(c.shield.to_string(), p.fg));
                });
            }
            ui.add_space(6.0);
            graph(
                ui,
                &p,
                "trace / hop",
                &h.trace_hops,
                tuning::TRACE_MAX,
                3,
                &[tuning::TRACE_MAX * 3 / 4],
            );
            ui.add_space(4.0);
            let ease = |ui: &mut Ui, key: &str, v: u32| {
                ui.ctx()
                    .animate_value_with_time(
                        ui.id().with(("count", key)),
                        v as f32,
                        anim::EASE_SECS * 2.0,
                    )
                    .round() as u32
            };
            let cred = ease(ui, "cred", d.credits);
            let frags = ease(ui, "frags", d.fragments);
            ui.horizontal(|ui| {
                ui.label(text(format!("CRED {cred}¢"), p.bright));
                ui.label(text(format!("FRAGS {frags}"), p.fg));
                ui.label(text(format!("HOPS {}", g.hops), p.dim));
            });
        });
    }

    /// warez/: a process table. Running a program lights its row and its activity graph.
    pub(crate) fn btop_warez(&mut self, ui: &mut Ui) {
        let g = self.game;
        let p = self.pal;
        let fighting = g.phase == Phase::Combat;
        let cpu = g.combat.as_ref().map_or(0, |c| c.cpu);
        let note = format!("hdd {}/{}", g.hdd.len(), g.deck.hdd_max);
        let mut acts = Vec::new();
        let mut hex = None;
        boxed(ui, &p, "warez/", &note, |ui| {
            // NAME is as wide as the widest real name; a narrow box drops CPU before it
            // would slice a name.
            let name_w = g
                .hdd
                .iter()
                .map(|s| warez_name(&s.spec).chars().count())
                .max()
                .unwrap_or(0);
            let cw = ui.fonts_mut(|f| f.glyph_width(&mono(MONO), 'M'));
            // What the row's buttons and activity graph take on its right.
            let controls = 24.0 + 2.0 * (2.0 * cw + 2.0 * ui.spacing().button_padding.x) + 15.0;
            let room = ((ui.available_width() - controls) / cw).floor().max(0.0) as usize;
            let cols = WarezCols::fit(name_w, room);
            ui.label(text(cols.line("ID", "NAME", "CPU"), p.dim));
            let order: Vec<usize> = (0..g.hdd.len()).collect();
            ScrollArea::vertical()
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for i in order {
                        let s = &g.hdd[i];
                        let w = &s.spec;
                        let lit = g
                            .history
                            .since_run(s.uid)
                            .is_some_and(|t| t < anim::RUN_GLOW_TICKS);
                        let col = if lit { p.bright } else { p.fg };
                        let row = ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 3.0;
                            // Buttons pinned right, then the activity graph, then the row text
                            // takes what's left.
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if fighting
                                        && ui
                                            .add_enabled(
                                                cpu >= w.cpu,
                                                egui::Button::new("run").small(),
                                            )
                                            .clicked()
                                    {
                                        acts.push(Action::Run(i));
                                    }
                                    if g.has_tool("hexed")
                                        && ui
                                            .small_button("hx")
                                            .on_hover_text("open in hexedit")
                                            .clicked()
                                    {
                                        hex = Some(i);
                                    }
                                    // One dot column per recent action: up where this program ran.
                                    let now = g.history.tick;
                                    let pts: Vec<u32> = (0..12u32)
                                        .rev()
                                        .map(|back| {
                                            let t = now.saturating_sub(back);
                                            u32::from(
                                                g.history
                                                    .runs
                                                    .iter()
                                                    .any(|&(u, at)| u == s.uid && at == t),
                                            )
                                        })
                                        .collect();
                                    spark(ui, &p, &pts, 1, 24.0);
                                    ui.with_layout(
                                        egui::Layout::left_to_right(egui::Align::Center),
                                        |ui| {
                                            let line = cols.line(
                                                &i.to_string(),
                                                &warez_name(w),
                                                &w.cpu.to_string(),
                                            );
                                            ui.add(egui::Label::new(text(line, col)).truncate())
                                                .on_hover_text(format!(
                                                    "{}\n{}\n{}",
                                                    w.name,
                                                    g.data.warez[s.warez].blurb,
                                                    w.effect.describe()
                                                ));
                                        },
                                    );
                                },
                            );
                        });
                        if lit {
                            ui.painter().rect_filled(
                                row.response.rect,
                                2.0,
                                p.fg.gamma_multiply(0.12),
                            );
                        }
                    }
                });
        });
        for a in acts {
            self.act(a);
        }
        if let Some(i) = hex {
            self.out.push(crate::panes::Request::OpenHex(i));
        }
    }

    /// target.ice in a fight: a draining strength meter, its graph, the intent waterfall and
    /// the damage waterfall. Outside a fight the classic view, boxed.
    pub(crate) fn btop_target(&mut self, ui: &mut Ui) {
        let g = self.game;
        let p = self.pal;
        let Some(c) = g.combat.as_ref().filter(|_| g.phase == Phase::Combat) else {
            let title = match g.phase {
                Phase::Flatlined => "target.ice  FLATLINED",
                Phase::JackedOut => "target.ice  JACKED OUT",
                _ => "target.ice",
            };
            boxed(ui, &p, title, "", |ui| self.target(ui));
            return;
        };
        let ice = &g.data.ice[c.ice];
        let tier = match ice.tier {
            0 => "RUNNER",
            1 => "ICE",
            _ => "BLACK ICE",
        };
        let note = format!("turn {}  cpu {}/{}", c.turn, c.cpu, g.deck.cpu_max);
        let mut acts = Vec::new();
        boxed(
            ui,
            &p,
            &format!("target.ice  {}", ice.name.to_uppercase()),
            &note,
            |ui| {
                ui.horizontal(|ui| {
                    ui.label(text(
                        format!("{tier} // {}", ice.class.label()),
                        if ice.tier >= 2 { p.bright } else { p.dim },
                    ));
                });
                ui.horizontal(|ui| {
                    ui.label(text("STR  ", p.dim));
                    meter(ui, &p, "ice", c.strength, c.strength_max, 20);
                    ui.label(text(format!("{}/{}", c.strength, c.strength_max), p.bright));
                });
                graph(ui, &p, "strength", &g.history.ice, c.strength_max, 2, &[]);
                ui.add_space(4.0);
                ui.columns(2, |cols| {
                    // Intent waterfall: what's coming falls toward now, at the bottom.
                    cols[0].label(text("INTENT", p.dim));
                    let n = ice.intents.len().max(1);
                    for k in (0..anim::INTENTS_AHEAD).rev() {
                        let intent = &ice.intents[(c.intent_ix + k) % n];
                        let (mark, col) = match k {
                            0 => ("▶", p.bright),
                            1 => ("·", p.fg),
                            _ => ("·", p.dim.gamma_multiply(1.0 - k as f32 * 0.18)),
                        };
                        let when = if k == 0 {
                            "now ".to_string()
                        } else {
                            format!("+{k}  ")
                        };
                        let mut line = format!("{mark} {when}{}", intent.describe());
                        if k == 0 && c.stunned {
                            line.push_str(" (lagged)");
                        }
                        cols[0].label(text(line, col));
                    }
                    // Damage waterfall: newest turn on top, older ones fade out below.
                    cols[1].label(text("TURN  dealt  taken  soak", p.dim));
                    let turns = &g.history.turns;
                    if turns.is_empty() {
                        cols[1].label(text("  (first turn)", p.faint));
                    }
                    for (age, t) in turns.iter().rev().take(anim::WATERFALL_ROWS).enumerate() {
                        let fade = 1.0 - age as f32 / anim::WATERFALL_ROWS as f32 * 0.8;
                        let bar = |n: u32| "▮".repeat(n.min(6) as usize);
                        cols[1].horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;
                            ui.label(text(format!("T{:<3}", t.turn), p.dim.gamma_multiply(fade)));
                            ui.label(text(
                                format!("{:>2}{:<6}", t.dealt, bar(t.dealt)),
                                p.fg.gamma_multiply(fade),
                            ));
                            ui.label(text(
                                format!("{:>2}{:<6}", t.taken, bar(t.taken)),
                                p.bright.gamma_multiply(fade),
                            ));
                            ui.label(text(
                                format!("{:>2}", t.absorbed),
                                p.dim.gamma_multiply(fade),
                            ));
                        });
                    }
                });
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    for (i, s) in g.hdd.iter().enumerate() {
                        let w = &s.spec;
                        let class_hit =
                            matches!(w.effect, Effect::Break { vs: Some(v), .. } if v == ice.class);
                        let label = format!(
                            "[{i}] {} ({}c){}",
                            w.name,
                            w.cpu,
                            if class_hit { " x2" } else { "" }
                        );
                        let btn =
                            egui::Button::new(text(label, if class_hit { p.bright } else { p.fg }));
                        if ui
                            .add_enabled(c.cpu >= w.cpu, btn)
                            .on_hover_text(w.effect.describe())
                            .clicked()
                        {
                            acts.push(Action::Run(i));
                        }
                    }
                    if ui.button(text(" END TURN [space] ", p.bright)).clicked() {
                        acts.push(Action::EndTurn);
                    }
                });
            },
        );
        for a in acts {
            self.act(a);
        }
    }

    /// todo: each contract with how far you are toward its layer and its ghost-bonus gauge.
    pub(crate) fn btop_todo(&mut self, ui: &mut Ui) {
        let g = self.game;
        let p = self.pal;
        let open = g
            .contracts
            .iter()
            .filter(|k| k.status == ContractStatus::Active)
            .count();
        boxed(ui, &p, "todo", &format!("{open} open"), |ui| {
            if g.contracts.is_empty() {
                ui.label(text(
                    "no contracts. fixers post on BBS jobs boards and hang around MSG nodes.",
                    p.dim,
                ));
                return;
            }
            let here = g.root().nodes[g.root_at()].layer as u32;
            for k in g.contracts.iter().rev() {
                let n = &g.root().nodes[k.node];
                let (mark, col) = match k.status {
                    ContractStatus::Active => ("[ ]", p.fg),
                    ContractStatus::Done => ("[x]", p.dim),
                    ContractStatus::Failed => ("[-]", p.faint),
                    ContractStatus::Offered => ("[?]", p.dim),
                    ContractStatus::Betrayed => ("[!]", p.dim),
                };
                ui.horizontal(|ui| {
                    ui.label(text(format!("{mark} {}  {}  ", k.client, n.name), col));
                    if let Some(label) = g.betrayal_label(k) {
                        ui.label(text(format!("{label} {}¢", k.paid), p.bright));
                        return;
                    }
                    ui.label(text(
                        format!("{}¢", k.reward),
                        if k.status == ContractStatus::Active {
                            p.bright
                        } else {
                            col
                        },
                    ));
                });
                if k.status != ContractStatus::Active {
                    continue;
                }
                ui.horizontal(|ui| {
                    ui.label(text(format!("    layer {here}/{}", n.layer), p.dim));
                    meter(
                        ui,
                        &p,
                        &format!("todo{}", k.node),
                        here.min(n.layer as u32),
                        (n.layer as u32).max(1),
                        10,
                    );
                    if let Some((cap, bonus)) = k.ghost {
                        let ok = g.deck.trace <= cap;
                        ui.label(text(
                            format!(" ghost +{bonus}¢ trace {}/{cap}", g.deck.trace),
                            if ok { p.fg } else { p.dim },
                        ));
                        meter(
                            ui,
                            &p,
                            &format!("ghost{}", k.node),
                            g.deck.trace.min(cap),
                            cap.max(1),
                            8,
                        );
                    }
                });
            }
        });
    }

    /// terminal: a traffic sparkline, then the log with a stamp column and kind markers.
    pub(crate) fn btop_terminal(&mut self, ui: &mut Ui) {
        let g = self.game;
        let p = self.pal;
        boxed(ui, &p, "terminal", "help", |ui| {
            // Net traffic: log lines per hop, recent hops.
            let mut per_hop = vec![0u32; g.hops as usize + 1];
            for l in &g.log {
                if let Some(c) = per_hop.get_mut(l.hop as usize) {
                    *c += 1;
                }
            }
            let peak = per_hop.iter().copied().max().unwrap_or(1).max(1);
            ui.horizontal(|ui| {
                ui.label(text("net", p.dim));
                spark(
                    ui,
                    &p,
                    &per_hop,
                    peak,
                    (ui.available_width() - 8.0).max(20.0),
                );
            });
            let input_h = MONO + 14.0;
            ScrollArea::vertical()
                .stick_to_bottom(true)
                .auto_shrink([false, false])
                .max_height(ui.available_height() - input_h)
                .show(ui, |ui| {
                    for l in &g.log {
                        let (mark, col) = match l.kind {
                            LogKind::Sys => ("·", p.dim),
                            LogKind::Info => ("›", p.fg),
                            LogKind::Good => ("+", p.fg),
                            LogKind::Bad => ("!", p.bright),
                            LogKind::Loot => ("$", p.bright),
                        };
                        let stamp = if l.turn > 0 {
                            format!("h{:02}.t{:<2}", l.hop, l.turn)
                        } else {
                            format!("h{:02}    ", l.hop)
                        };
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            ui.label(text(stamp, p.faint));
                            ui.label(text(mark, col));
                            ui.label(text(self.garble(&l.text), col));
                        });
                    }
                });
            self.prompt(ui);
        });
    }
}

/// The status bar's sparklines: trace and CPU, small.
pub fn status_sparks(ui: &mut Ui, p: &Palette, g: &ninsei_core::Game) {
    let trace = g.history.series(|s| s.trace);
    let cpu = g.history.series(|s| s.cpu);
    let tail = |v: &[u32]| v.iter().rev().take(16).rev().copied().collect::<Vec<u32>>();
    spark(ui, p, &tail(&trace), tuning::TRACE_MAX, 32.0);
    spark(ui, p, &tail(&cpu), g.deck.cpu_max.max(1), 32.0);
}

/// The volume box: a btop box of whole cells over the status line's speaker.
pub mod vol {
    /// The box, in cells.
    pub const COLS: usize = 24;
    pub const ROWS: usize = 4;
    /// The meter's cells: one notch each, 5% apiece.
    pub const NOTCHES: u32 = 20;
    /// Points between the box and the speaker under it.
    pub const GAP: f32 = 2.0;
}

fn speaker_at() -> egui::Id {
    egui::Id::new("btop-speaker")
}

/// The speaker on the status line, as a bar module shows it: an icon and the level, or
/// `muted`. A click opens the volume box over it, or closes it.
pub fn speaker(ui: &mut Ui, cx: &mut crate::chrome::FrameCx) -> Vec<crate::panes::Request> {
    let l = cx.level;
    let icon = match (l.on, l.volume) {
        (false, _) => "🔇",
        (true, 0) => "🔈",
        (true, v) if v < 50 => "🔉",
        _ => "🔊",
    };
    let what = if l.on {
        format!("{}%", l.volume)
    } else {
        "muted".into()
    };
    let lit = if cx.popup.is_open() {
        cx.pal.bright
    } else {
        cx.pal.dim
    };
    let r = ui.add(egui::Label::new(text(format!("{icon} {what}"), lit)).sense(Sense::click()));
    ui.ctx().data_mut(|d| d.insert_temp(speaker_at(), r.rect));
    // It closes whenever, but only opens when nothing else has the keyboard.
    if r.clicked() && (cx.keys_free || cx.popup.is_open()) {
        cx.popup.toggle(l);
    }
    Vec::new()
}

/// The volume box: a meter of [`vol::NOTCHES`] cells (click one to set the level), `- 70% +`
/// cut into the bottom edge, and the mute switch. `←`/`→` or `-`/`+` step it, `m` mutes,
/// Esc or Enter closes it.
pub fn sound_popup(
    ctx: &egui::Context,
    cx: &mut crate::chrome::FrameCx,
) -> Vec<crate::panes::Request> {
    use crate::kit::cells::Cells;
    let (p, now) = (cx.pal, cx.level);
    let (mut to, k) = crate::volume::start(ctx, now, egui::Key::M, vol::NOTCHES);
    if k.esc || k.ok {
        cx.popup.close();
    }
    let screen = ctx.content_rect();
    let under = ctx
        .data(|d| d.get_temp::<Rect>(speaker_at()))
        .unwrap_or(Rect::from_min_size(screen.left_bottom(), egui::Vec2::ZERO));
    let cell = Cells::cell_size(ctx);
    let size = vec2(vol::COLS as f32 * cell.x, vol::ROWS as f32 * cell.y);
    // Over the speaker (under it if there's no room), its left edge on the speaker's, kept on
    // screen, on whole pixels.
    let left = under.left().min(screen.right() - size.x).max(screen.left());
    let above = under.top() - vol::GAP - size.y;
    let top = if above >= screen.top() {
        above
    } else {
        under.bottom() + vol::GAP
    };
    let rect = Rect::from_min_size(pos2(left.round(), top.round()), size);
    egui::Area::new(egui::Id::new("btop-volume"))
        .order(egui::Order::Foreground)
        .fixed_pos(rect.min)
        .show(ctx, |ui| {
            let (_, painter) = ui.allocate_painter(size, Sense::hover());
            let c = Cells::grid(ui, rect.min, (vol::COLS, vol::ROWS), p.dim, p.bg, p.faint);
            painter.rect_filled(c.rect, 0.0, p.bg);
            c.frame(&painter, 0, 0, vol::COLS, vol::ROWS);
            c.title(&painter, 2, 0, "volume", p.bright);
            let n = vol::NOTCHES as usize;
            let colours: Vec<Color32> = (0..n)
                .map(|i| {
                    if now.on {
                        ramp(&p, (i as f32 + 1.0) / n as f32)
                    } else {
                        p.dim
                    }
                })
                .collect();
            c.meter(
                &painter,
                2,
                1,
                n,
                now.notch(vol::NOTCHES) as usize,
                &colours,
            );
            let hit = |ui: &mut Ui, r: Rect, label: String| {
                crate::volume::hit(ui, r, "btop-volume", &label, Sense::click()).clicked()
            };
            for i in 0..n {
                let pct = ninsei_core::hub::notch_volume(i as u32 + 1, vol::NOTCHES);
                if hit(ui, c.at(2 + i, 1, 1, 1), format!("{pct}%")) {
                    to.volume = pct;
                }
            }
            // The switch, its key lit the way btop lights a box's hotkey.
            c.text(&painter, 2, 2, if now.on { "[ ]" } else { "[x]" }, p.fg);
            c.text(&painter, 6, 2, "m", p.bright);
            c.text(&painter, 7, 2, "ute", p.fg);
            if hit(ui, c.at(2, 2, 8, 1), "mute".into()) {
                to.on = !to.on;
            }
            // The number keeps its width, so `+` never moves under the mouse.
            let level = format!("- {:>3}% +", now.volume);
            let used = c.title(&painter, 2, vol::ROWS - 1, &level, p.fg);
            if hit(ui, c.at(3, vol::ROWS - 1, 1, 1), "-".into()) {
                to = now.nudged(-1, vol::NOTCHES);
            }
            if used > 2 && hit(ui, c.at(2 + used - 2, vol::ROWS - 1, 1, 1), "+".into()) {
                to = now.nudged(1, vol::NOTCHES);
            }
        });
    now.to(to)
}

/// The vis box: the spectrum as columns of whole cells, one cell wide with a cell between,
/// lit bottom up along the ramp; the level cut into the bottom edge.
pub fn vis_box(ui: &mut Ui, p: &Palette, samples: &[f32], rate: u32) {
    use crate::kit::cells::Cells;
    let level = crate::vispane::held(ui, "btop-vis", ninsei_core::scope::level(samples));
    let note = ninsei_core::scope::db_text(level);
    boxed(ui, p, "vis", note.trim(), |ui| {
        let area = ui.available_rect_before_wrap();
        let c = Cells::fit(ui, area, p.dim, p.bg, p.faint);
        let n = (c.cols / 2).max(1);
        let bars = crate::vispane::bars(ui, "btop-vis", samples, rate, n);
        let painter = ui.painter_at(area);
        let left = (c.cols - (n * 2 - 1).min(c.cols)) / 2;
        let colours: Vec<Color32> = (0..c.rows)
            .map(|r| ramp(p, (r as f32 + 1.0) / c.rows as f32))
            .collect();
        let lit = ninsei_core::scope::cells_lit(&bars, c.rows);
        for (i, on) in lit.into_iter().enumerate() {
            c.meter_up(&painter, left + i * 2, c.rows - 1, c.rows, on, &colours);
        }
        ui.allocate_rect(area, Sense::hover());
    });
}

/// The `ice.bas` tile: the Ono-Sendai's Script pane. The listing as gdb's TUI shows a
/// source window, on the chrome's cells; what it shows is core's (`btop_fight`). Its view
/// state (the cursor, the first row in view) is egui memory, by line number.
pub mod script {
    use egui::{Event, Id, Key, Rect, Sense, Ui, UiBuilder, vec2};
    use ninsei_core::btop_fight::{self, Gutter, Row};
    use ninsei_core::{Action, Game};

    use super::{MONO, boxed_lit, cells_in, mono};
    use crate::chrome::{FrameCx, act};
    use crate::panes::{Panes, Request};

    /// The tile's keyboard focus: while it has it, its letters are the debugger's and the
    /// terminal doesn't type.
    pub fn id() -> Id {
        Id::new("btop-ice.bas")
    }

    /// What the tile remembers between frames.
    #[derive(Clone, Debug, Default)]
    pub(crate) struct View {
        /// The line the cursor is on.
        pub cursor: Option<u32>,
        /// The listing's window: core's one scroll rule, the wheel's leftover travel.
        pub list: crate::kit::List,
        /// The line being retyped where it stands, if one is.
        pub edit: Option<Edit>,
    }

    /// A line being retyped: its number (a new one, for a line just opened) and the editor
    /// holding its text. Enter commits it as one `Action::Program`; Esc puts it back.
    #[derive(Clone, Debug)]
    pub(crate) struct Edit {
        pub no: u32,
        pub line: ninsei_term::LineEditor,
    }

    /// The editor row's keyboard focus: while it has it, typing is the line's, and nothing
    /// else (the terminal, the hotkeys, the tile's letters) hears it.
    pub fn edit_id() -> Id {
        id().with("edit")
    }

    fn view_id() -> Id {
        id().with("view")
    }

    pub(crate) fn view(ctx: &egui::Context, g: &Game) -> View {
        let mut v: View = ctx.data(|d| d.get_temp(view_id())).unwrap_or_default();
        if v.cursor.is_none_or(|n| g.program.line(n).is_none()) {
            v.cursor = btop_fight::home(g);
        }
        v
    }

    pub(crate) fn keep(ctx: &egui::Context, v: View) {
        ctx.data_mut(|d| d.insert_temp(view_id(), v));
    }

    pub(crate) fn has_keys(ctx: &egui::Context) -> bool {
        ctx.memory(|m| m.has_focus(id()))
    }

    /// Takes the keyboard for the tile: its arrows, Tab and Esc are its own.
    pub(crate) fn take_keys(ctx: &egui::Context) {
        ctx.memory_mut(|m| {
            m.request_focus(id());
            m.set_focus_lock_filter(
                id(),
                egui::EventFilter {
                    tab: true,
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    escape: true,
                },
            );
        });
    }

    /// Starts retyping line `no` from `text`; the line takes the keyboard.
    fn start_edit(ctx: &egui::Context, v: &mut View, no: u32, text: &str) {
        let mut line = ninsei_term::LineEditor::new(1);
        line.set_text(text);
        v.edit = Some(Edit { no, line });
        ctx.memory_mut(|m| m.request_focus(edit_id()));
    }

    /// The tile's keys, read before the run's hotkeys and only while the tile has the
    /// keyboard: `r` run, `s` step, `c` continue, `p` pause (each when it presses), `j`/`k` or
    /// the arrows move the cursor, `e` or Enter retypes its line, `o` opens a line after it, `d`
    /// deletes it, Esc gives the keyboard back. Letters with Ctrl or Alt are never ours. The
    /// keys it takes and every typed character are taken out of the frame, so nothing after
    /// sees them; Space and the digits stay for the hotbar.
    pub(crate) fn keys(ctx: &egui::Context, cx: &mut FrameCx) -> Vec<Request> {
        if !cx.keys_free || !has_keys(ctx) {
            return Vec::new();
        }
        let g = cx.game;
        let mut v = view(ctx, g);
        let k = btop_fight::keys(g, v.cursor);
        let mut out = Vec::new();
        let mut taken = Vec::new();
        let events = ctx.input(|i| i.events.clone());
        // Where a retype started this frame: the typing after it is the line's.
        let mut edit_from = None;
        for (i, e) in events.iter().enumerate() {
            let Event::Key {
                key,
                pressed: true,
                repeat,
                modifiers,
                ..
            } = e
            else {
                continue;
            };
            if !modifiers.is_none() || v.edit.is_some() {
                continue;
            }
            let once = !*repeat;
            let debugger = match key {
                Key::R if k.run => Some(Action::Play),
                Key::S if k.step => Some(Action::Step),
                Key::C if k.cont => Some(Action::Play),
                Key::P if k.pause => Some(Action::Pause),
                _ => None,
            };
            let took = match key {
                _ if debugger.is_some() => {
                    if once {
                        out.extend(debugger.map(act));
                    }
                    true
                }
                Key::J | Key::ArrowDown => {
                    v.cursor = btop_fight::moved(g, v.cursor, 1);
                    true
                }
                Key::K | Key::ArrowUp => {
                    v.cursor = btop_fight::moved(g, v.cursor, -1);
                    true
                }
                // Taken on a repeat too (a held Enter must not reach the hotkeys), acted on once.
                Key::E | Key::Enter => {
                    if once && let Some(no) = v.cursor.filter(|_| k.edit) {
                        start_edit(ctx, &mut v, no, &btop_fight::editing_text(g, no));
                        edit_from = Some(i);
                    }
                    true
                }
                Key::O => {
                    if once && let Some(no) = btop_fight::open_after(g, v.cursor) {
                        start_edit(ctx, &mut v, no, "");
                        edit_from = Some(i);
                    }
                    true
                }
                Key::D => {
                    if once
                        && let Some(no) = v.cursor
                        && let Some(a) = btop_fight::delete(g, no)
                    {
                        let lines = &g.program.lines;
                        let before = lines.iter().map(|l| l.no).take_while(|&n| n < no).last();
                        v.cursor = g.program.after(no).or(before);
                        out.push(act(a));
                    }
                    true
                }
                Key::Escape => {
                    ctx.memory_mut(|m| m.surrender_focus(id()));
                    true
                }
                // Space, the digits and the rest are the run's.
                _ => false,
            };
            if took {
                taken.push(i);
            }
        }
        let drop = dropped(&events, &taken, edit_from);
        ctx.input_mut(|inp| {
            let mut i = 0;
            inp.events.retain(|_| {
                i += 1;
                !drop.contains(&(i - 1))
            })
        });
        keep(ctx, v);
        out
    }

    /// The events (by index) the tile takes out of a frame: the keys it took, the character each
    /// of those typed (beside its key: after it natively, before it in a browser), and any other
    /// typing before a retype started (the tile has the keyboard; that typing has nowhere to
    /// go). Typing after a retype started stays for the line.
    pub(crate) fn dropped(
        events: &[Event],
        taken: &[usize],
        edit_from: Option<usize>,
    ) -> Vec<usize> {
        let typed = |i: usize, key: Key| {
            let name = key.name().to_ascii_lowercase();
            matches!(events.get(i), Some(Event::Text(t)) if *t == name)
        };
        let mut out: Vec<usize> = taken.to_vec();
        for &t in taken {
            if let Some(Event::Key { key, .. }) = events.get(t) {
                if typed(t + 1, *key) {
                    out.push(t + 1);
                } else if t > 0 && typed(t - 1, *key) {
                    out.push(t - 1);
                }
            }
        }
        let until = edit_from.unwrap_or(events.len());
        out.extend((0..until).filter(|&i| matches!(events[i], Event::Text(_))));
        out
    }

    /// The row being retyped: its number lit, then the line editor over the text's cells.
    /// Enter commits the one line; Esc (or a click elsewhere) puts the old text back.
    fn edit_row(p: &mut Panes, ui: &mut Ui, at: Rect, v: &mut View) {
        let Some(edit) = v.edit.as_mut() else {
            return;
        };
        let pal = p.pal;
        let style = ninsei_term::widget::Style {
            font: mono(MONO),
            prompt: pal.dim,
            text: pal.bright,
            cursor: pal.fg,
            under_cursor: pal.bg,
            search: pal.fg,
        };
        let h = ui.fonts_mut(|f| f.row_height(&style.font)) + 4.0;
        let rect = Rect::from_center_size(at.center(), vec2(at.width(), h));
        let mut child = ui.new_child(UiBuilder::new().max_rect(rect));
        let r =
            ninsei_term::widget::line(&mut child, edit_id(), &mut edit.line, "", &style, &|_| {
                Vec::new()
            });
        let no = edit.no;
        let submitted = r.outcomes.iter().find_map(|o| match o {
            ninsei_term::Outcome::Submit(t) => Some(t.clone()),
            _ => None,
        });
        // Ctrl+C backs out as Esc does (the editor has already cleared its line).
        let cancelled = r
            .outcomes
            .iter()
            .any(|o| matches!(o, ninsei_term::Outcome::Cancelled(_)));
        let ctx = ui.ctx().clone();
        if let Some(text) = submitted {
            if let Some(a) = btop_fight::retype(p.game, no, &text) {
                p.act(a);
            }
            v.edit = None;
            if !text.trim().is_empty() {
                v.cursor = Some(no);
            }
            take_keys(&ctx);
        } else if cancelled {
            v.edit = None;
            take_keys(&ctx);
        } else if !r.focused {
            // Esc let go of the keyboard (it comes back to the tile), or a click went
            // elsewhere (it stays there): the old text stands.
            v.edit = None;
            if ctx.memory(|m| m.focused().is_none()) {
                take_keys(&ctx);
            }
        }
    }

    /// Rows the listing leaves under it for the legend, a blank row first.
    const LEGEND_ROWS: usize = 3;

    /// The cells the number takes: as wide as the widest number, never under two.
    fn number_w(rows: &[Row]) -> usize {
        rows.iter()
            .filter_map(|r| r.no)
            .map(|n| n.to_string().len())
            .max()
            .unwrap_or(0)
            .max(2)
    }

    /// The tile: the listing on whole cells, the line about to run lit with `▶` in the gutter,
    /// a STOP's `●`, bad lines dim with their fault after them, the cursor in inverse while the
    /// tile has the keyboard; the budgets cut into the bottom edge. A click puts the cursor on
    /// a line and gives the tile the keyboard.
    pub(crate) fn show(p: &mut Panes, ui: &mut Ui) {
        let g = p.game;
        let pal = p.pal;
        let ctx = ui.ctx().clone();
        let area = ui.available_rect_before_wrap();
        // The whole tile takes the click that gives it the keyboard.
        let tile = ui.interact(area, id(), Sense::click());
        let focused = has_keys(&ctx);
        if focused {
            take_keys(&ctx);
        }
        let mut v = view(&ctx, g);
        let mut rows = btop_fight::rows(g);
        // A line just opened has a row before it's in the program.
        if let Some(e) = &v.edit
            && g.program.line(e.no).is_none()
        {
            let at = rows
                .iter()
                .position(|r| r.no.is_none_or(|n| n > e.no))
                .unwrap_or(rows.len());
            rows.insert(
                at,
                Row {
                    no: Some(e.no),
                    text: String::new(),
                    gutter: Gutter::None,
                    dim: false,
                    fault: None,
                },
            );
        }
        let keys = btop_fight::keys(g, v.cursor);
        let note = btop_fight::note(g);
        boxed_lit(ui, &pal, btop_fight::title(g), &note, focused, |ui| {
            let inner = ui.available_rect_before_wrap();
            let c = cells_in(ui, inner);
            let painter = ui.painter_at(c.rect);
            // The legend goes before the listing does when the tile is short.
            let legend = c.rows >= LEGEND_ROWS + 3;
            let n = c.rows - if legend { LEGEND_ROWS } else { 0 };
            // Kept in view: the row being retyped, the cursor while the tile has the keys, else
            // the line about to run. Out of a fight and unfocused, the wheel is free.
            let current = rows.iter().find(|r| r.gutter == Gutter::Current);
            let follow = if let Some(e) = &v.edit {
                Some(e.no)
            } else if focused {
                v.cursor
            } else {
                current.and_then(|r| r.no)
            };
            let sel = follow.and_then(|no| rows.iter().position(|r| r.no == Some(no)));
            // The wheel scrolls whole rows and keeps the rest; the window still keeps the
            // followed row (the one being retyped, say) in view.
            if let Some(dy) = crate::kit::scrollbar::take_wheel(ui, c.rect, id()) {
                v.list.wheel(dy / c.ch);
            }
            let shown = v.list.view(rows.len(), sel, n);
            let w = number_w(&rows);
            let text_c = 2 + w + 2;
            for i in shown {
                let (r, y) = (&rows[i], v.list.row(i));
                let cursor = focused && r.no.is_some() && r.no == v.cursor;
                let lit = r.gutter == Gutter::Current;
                let (num, ink) = if r.dim {
                    (pal.dim, pal.dim)
                } else if lit {
                    (pal.bright, pal.bright)
                } else {
                    (pal.dim, pal.fg)
                };
                if lit {
                    c.fill(&painter, 0, y, c.cols, 1, pal.faint);
                }
                let mark = if r.gutter == Gutter::None {
                    pal.dim
                } else {
                    pal.bright
                };
                c.text(&painter, 0, y, r.gutter.mark(), mark);
                let line = match &r.fault {
                    Some(f) => format!("{}  {f}", r.text),
                    None => r.text.clone(),
                };
                let no = r.no.map_or(String::new(), |n| n.to_string());
                if r.no.is_some() && r.no == v.edit.as_ref().map(|e| e.no) {
                    c.text_right(&painter, 2 + w, y, &no, pal.bright);
                    let at = c.at(text_c, y, c.cols.saturating_sub(text_c), 1);
                    edit_row(p, ui, at, &mut v);
                } else if cursor {
                    let lead = format!("{no:>w$}  ");
                    let room = c.cols.saturating_sub(2);
                    c.inverse(
                        &painter,
                        2,
                        y,
                        room,
                        &format!("{lead}{line}"),
                        pal.bg,
                        pal.fg,
                    );
                } else {
                    c.text_right(&painter, 2 + w, y, &no, num);
                    let room = c.cols.saturating_sub(text_c);
                    c.text_max(&painter, text_c, y, &line, room, ink);
                }
            }
            if legend {
                for (k, keys_row) in keys.legend().iter().enumerate() {
                    let y = c.rows - 2 + k;
                    let mut x = 0;
                    for (key, word, presses) in keys_row.iter() {
                        let live = focused && *presses;
                        let ink = if live { pal.fg } else { pal.dim };
                        x += c.text(&painter, x, y, key, ink);
                        x += c.text(&painter, x, y, &format!(" {word}  "), pal.dim);
                    }
                }
            }
            // A click on a row puts the cursor there.
            if tile.clicked()
                && let Some(pos) = tile.interact_pointer_pos()
                && let Some((_, y)) = c.cell_at(pos)
                && let Some(r) = rows.get(v.list.top + y).filter(|_| y < n)
                && let Some(no) = r.no
            {
                v.cursor = Some(no);
                if tile.double_clicked() && v.edit.is_none() {
                    start_edit(&ctx, &mut v, no, &btop_fight::editing_text(g, no));
                }
            }
            ui.allocate_rect(inner, Sense::hover());
        });
        if tile.clicked() && v.edit.is_none() {
            take_keys(&ctx);
        }
        keep(&ctx, v);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::uitest::Harness;

    #[test]
    fn a_box_sits_on_whole_cells_with_its_body_inside() {
        let data = ninsei_core::Data::builtin().unwrap();
        let p = Palette::from_def(&data.themes[0]);
        let mut h = Harness::new(800.0, 600.0);
        let room = Rect::from_min_size(pos2(10.0, 10.0), vec2(301.0, 177.0));
        let mut inner = Rect::NOTHING;
        h.frame(Vec::new(), |ui| {
            let mut child = ui.new_child(UiBuilder::new().max_rect(room));
            boxed(&mut child, &p, "deck.sys", "seed 01", |ui| {
                inner = ui.max_rect()
            });
        });
        for s in h.painted() {
            let r = s.visual_bounding_rect();
            assert!(room.contains_rect(r), "{r:?} outside the pane {room:?}");
        }
        assert!(room.shrink(3.0).contains_rect(inner) && inner.width() > 250.0);
        // 10px and 12px inside the border line, as before the box sat on cells.
        let line = h
            .rects()
            .into_iter()
            .find(|(_, fill, stroke)| *fill == Color32::TRANSPARENT && *stroke == p.dim)
            .expect("the border")
            .0;
        assert!(
            (inner.left() - line.left() - 10.0).abs() < 0.6,
            "{inner:?} in {line:?}"
        );
        assert!((inner.top() - line.top() - 12.0).abs() < 0.6);
        // The title's glyphs sit in the top row of cells, over the border.
        let d = h
            .painted()
            .iter()
            .find_map(|s| match s {
                egui::Shape::Text(t) if t.galley.text() == "d" => Some(t.visual_bounding_rect()),
                _ => None,
            })
            .expect("the title");
        assert!(d.top() < inner.top(), "on the top edge, above the body");
    }

    #[test]
    fn a_narrow_box_cuts_its_title_and_note_rather_than_dropping_them() {
        let data = ninsei_core::Data::builtin().unwrap();
        let p = Palette::from_def(&data.themes[0]);
        let painted = |w: f32| {
            let mut h = Harness::new(800.0, 600.0);
            let room = Rect::from_min_size(pos2(10.0, 10.0), vec2(w, 120.0));
            h.frame(Vec::new(), |ui| {
                let mut child = ui.new_child(UiBuilder::new().max_rect(room));
                boxed(
                    &mut child,
                    &p,
                    "net  NINSEI COMMUNITY COLLEGE",
                    "a long note",
                    |_| {},
                );
            });
            h.painted()
                .iter()
                .filter_map(|s| match s {
                    egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                    _ => None,
                })
                .collect::<String>()
        };
        // Narrow: the title takes the edge, cut and marked.
        let narrow = painted(200.0);
        assert!(
            narrow.starts_with("net") && narrow.contains('…'),
            "{narrow}"
        );
        // Wider: the title whole, the note cut into what's left and marked.
        let wide = painted(330.0);
        assert!(wide.contains("NINSEICOMMUNITYCOLLEGE"), "{wide}");
        assert!(wide.ends_with('…'), "the note, cut: {wide}");
    }

    #[test]
    fn a_box_too_small_for_its_frame_still_draws_its_body() {
        let data = ninsei_core::Data::builtin().unwrap();
        let p = Palette::from_def(&data.themes[0]);
        let mut h = Harness::new(800.0, 600.0);
        let mut ran = false;
        h.frame(Vec::new(), |ui| {
            let room = Rect::from_min_size(pos2(10.0, 10.0), vec2(40.0, 24.0));
            let mut child = ui.new_child(UiBuilder::new().max_rect(room));
            boxed(&mut child, &p, "terminal", "help", |_| ran = true);
        });
        assert!(ran);
    }

    #[test]
    fn the_warez_table_drops_numbers_before_it_slices_a_name() {
        let cols = WarezCols::fit("firewall.exe+O2".len(), 40);
        assert!(cols.cpu);
        let row = cols.line("0", "firewall.exe+O2", "1");
        assert!(row.contains("firewall.exe+O2"), "{row}");
        assert_eq!(
            cols.line("ID", "NAME", "CPU").len(),
            row.len(),
            "header on the row"
        );
        // Narrower: CPU goes; the name stays whole.
        let cols = WarezCols::fit(15, 3 + 16 + 3);
        assert!(!cols.cpu);
        assert!(
            cols.line("0", "firewall.exe+O2", "1")
                .contains("firewall.exe+O2")
        );
    }
}

#[cfg(test)]
mod script_tests {
    use super::*;
    use crate::chrome::{ChromeView, FrameCx};
    use crate::panes::Request;
    use crate::uitest::Harness;
    use ninsei_core::{Data, Game};

    /// The right column of the default layout, about.
    const PANE: (f32, f32) = (380.0, 520.0);

    fn fight(program: &str) -> Game {
        let mut g = crate::stack::tests::a_fight(&Data::builtin().unwrap());
        g.apply(Action::Program(program.into()));
        g
    }

    fn pal(g: &Game) -> Palette {
        Palette::from_def(&g.data.themes[0])
    }

    /// Where the tile sits in its test frames.
    fn pane_rect((w, h): (f32, f32)) -> Rect {
        Rect::from_min_size(pos2(0.0, 0.0), vec2(w, h))
    }

    /// One frame as the run draws it: the chrome's keys, then the tile in a `w`x`h` pane and,
    /// below it, the terminal; returns what they asked for. `term` is the command line.
    pub(crate) fn frame_with(
        h: &mut Harness,
        g: &Game,
        size: (f32, f32),
        term: &mut ninsei_term::LineEditor,
        focus_term: bool,
        ev: Vec<egui::Event>,
    ) -> Vec<Request> {
        let (mut focus, mut hex, mut keep) = (focus_term, None, Vec::new());
        let (mut suite, mut suite2) = (
            crate::suite::State::default(),
            crate::suite::State::default(),
        );
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
            out = crate::chrome::Btop.keys(ui.ctx(), &mut cx);
            let mut panes = Panes {
                game: g,
                pal: pal(g),
                map: ninsei_core::netview::MapView::Iso,
                deck: String::new(),
                chrome: ninsei_core::hardware::Chrome::Btop,
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
            let mut tile = ui.new_child(UiBuilder::new().max_rect(pane_rect(size)));
            crate::chrome::Btop.script(&mut panes, &mut tile);
            let below = Rect::from_min_size(pos2(0.0, size.1 + 10.0), vec2(600.0, 200.0));
            let mut t = ui.new_child(UiBuilder::new().max_rect(below));
            crate::chrome::Btop.terminal(&mut panes, &mut t);
            out.extend(panes.out);
        });
        out
    }

    /// A frame of the tile in `size`; the terminal sits below it, unfocused.
    pub(crate) fn frame(
        h: &mut Harness,
        g: &Game,
        size: (f32, f32),
        ev: Vec<egui::Event>,
    ) -> Vec<Request> {
        let mut term = ninsei_term::LineEditor::default();
        frame_with(h, g, size, &mut term, false, ev)
    }

    /// The glyphs painted inside the tile, row by row (top to bottom), each row's text and the
    /// colour of its first glyph past the gutter.
    fn rows_painted(h: &Harness, area: Rect) -> Vec<(String, Vec<Color32>)> {
        let mut glyphs: Vec<(f32, f32, String, Color32)> = h
            .painted()
            .iter()
            .filter_map(|s| match s {
                egui::Shape::Text(t) => {
                    // The galley's own box: centred on its cell, whatever the glyph.
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

    /// A letter as a keyboard sends it: the key, then the character it types.
    fn letter(c: char) -> Vec<egui::Event> {
        let key = egui::Key::from_name(&c.to_ascii_uppercase().to_string()).unwrap();
        let mut ev = crate::uitest::key(key);
        ev.insert(1, egui::Event::Text(c.to_string()));
        ev
    }

    /// Typed text, a key and its character at a time.
    fn typed(s: &str) -> Vec<egui::Event> {
        s.chars()
            .flat_map(|c| {
                if c == ' ' {
                    let mut ev = crate::uitest::key(egui::Key::Space);
                    ev.insert(1, egui::Event::Text(" ".into()));
                    ev
                } else if c.is_ascii_alphabetic() {
                    letter(c)
                } else {
                    crate::uitest::text(&c.to_string())
                }
            })
            .collect()
    }

    /// The game actions in `out`.
    fn actions(out: Vec<Request>) -> Vec<Action> {
        out.into_iter()
            .filter_map(|r| match r {
                Request::Cmd(ninsei_core::Command::Game(a)) => Some(a),
                _ => None,
            })
            .collect()
    }

    /// The tile drawn, then clicked on line `no` so it has the keys and its cursor is there.
    fn focus_on(h: &mut Harness, g: &Game, no: u32) {
        frame(h, g, PANE, vec![]);
        let tile = pane_rect(PANE);
        let n = no.to_string();
        // The number's last digit, right-aligned in its column.
        let last = n.chars().last().unwrap().to_string();
        let first = n.chars().next().unwrap().to_string();
        let mut glyphs: Vec<(String, Rect)> = h
            .painted_text()
            .into_iter()
            .filter(|(_, r)| tile.contains(r.center()))
            .collect();
        glyphs.sort_by(|a, b| {
            a.1.center()
                .y
                .total_cmp(&b.1.center().y)
                .then(a.1.center().x.total_cmp(&b.1.center().x))
        });
        let at = glyphs
            .windows(n.len())
            .find(|w| {
                w[0].0 == first
                    && w[w.len() - 1].0 == last
                    && w.iter().map(|g| g.0.as_str()).collect::<String>() == n
                    && w[0].1.center().x < tile.left() + 60.0
            })
            .unwrap_or_else(|| panic!("no line {no}"))[0]
            .1
            .center();
        frame(h, g, PANE, crate::uitest::click(at));
        assert!(script::has_keys(&h.ctx));
        assert_eq!(script::view(&h.ctx, g).cursor, Some(no));
    }

    fn program(g: &Game) -> Vec<String> {
        g.program
            .lines
            .iter()
            .map(|l| format!("{} {}", l.no, l.text))
            .collect()
    }

    #[test]
    fn with_the_tile_focused_the_letters_drive_the_debugger() {
        let g = fight("10 RUN icepick\n20 STOP\n30 GOTO 10");
        let mut h = Harness::new(1280.0, 800.0);
        focus_on(&mut h, &g, 10);
        assert_eq!(
            actions(frame(&mut h, &g, PANE, letter('s'))),
            [Action::Step]
        );
        assert_eq!(
            actions(frame(&mut h, &g, PANE, letter('r'))),
            [Action::Play]
        );
        assert!(
            actions(frame(&mut h, &g, PANE, letter('p'))).is_empty(),
            "nothing to pause while paused"
        );
        // j and k move the cursor, and so do the arrows.
        frame(&mut h, &g, PANE, letter('j'));
        frame(&mut h, &g, PANE, crate::uitest::key(egui::Key::ArrowDown));
        assert_eq!(script::view(&h.ctx, &g).cursor, Some(30));
        frame(&mut h, &g, PANE, letter('k'));
        assert_eq!(script::view(&h.ctx, &g).cursor, Some(20));
        // Esc gives the keyboard back.
        frame(&mut h, &g, PANE, crate::uitest::key(egui::Key::Escape));
        assert!(!script::has_keys(&h.ctx));
        assert!(actions(frame(&mut h, &g, PANE, letter('s'))).is_empty());
    }

    #[test]
    fn with_the_terminal_focused_it_keeps_typing() {
        let g = fight("10 RUN icepick\n20 STOP\n30 GOTO 10");
        let mut h = Harness::new(1280.0, 800.0);
        let mut term = ninsei_term::LineEditor::default();
        frame_with(&mut h, &g, PANE, &mut term, true, vec![]);
        let out = frame_with(&mut h, &g, PANE, &mut term, false, letter('s'));
        assert!(actions(out).is_empty(), "the tile heard nothing");
        assert_eq!(term.text(), "s", "the terminal typed it");
    }

    #[test]
    fn a_line_is_retyped_where_it_stands_and_enter_commits_one_line() {
        let mut g = fight("10 RUN icepick\n20 STOP\n30 GOTO 10");
        let before = program(&g);
        let mut h = Harness::new(1280.0, 800.0);
        focus_on(&mut h, &g, 20);
        let mut term = ninsei_term::LineEditor::default();
        frame_with(&mut h, &g, PANE, &mut term, false, letter('e'));
        assert!(!script::has_keys(&h.ctx), "the row has the keyboard now");
        let mut ev = Vec::new();
        for _ in 0.."STOP".len() {
            ev.extend(crate::uitest::key(egui::Key::Backspace));
        }
        ev.extend(typed("RUN patch"));
        let out = frame_with(&mut h, &g, PANE, &mut term, false, ev);
        assert!(actions(out).is_empty(), "nothing commits before Enter");
        assert_eq!(term.text(), "", "the terminal heard none of it");
        let out = frame_with(
            &mut h,
            &g,
            PANE,
            &mut term,
            false,
            crate::uitest::key(egui::Key::Enter),
        );
        let acts = actions(out);
        assert_eq!(acts.len(), 1, "{acts:?}");
        for a in acts {
            g.apply(a);
        }
        let after = program(&g);
        let differ: Vec<_> = before.iter().zip(&after).filter(|(a, b)| a != b).collect();
        assert_eq!(differ.len(), 1, "{after:?}");
        assert_eq!(after[1], "20 RUN patch");
        frame(&mut h, &g, PANE, vec![]);
        assert!(script::has_keys(&h.ctx), "the tile has the keys back");
    }

    #[test]
    fn esc_puts_the_old_text_back() {
        let g = fight("10 RUN icepick\n20 STOP\n30 GOTO 10");
        let mut h = Harness::new(1280.0, 800.0);
        focus_on(&mut h, &g, 20);
        frame(&mut h, &g, PANE, crate::uitest::key(egui::Key::Enter));
        frame(&mut h, &g, PANE, typed("xyz"));
        let out = frame(&mut h, &g, PANE, crate::uitest::key(egui::Key::Escape));
        assert!(actions(out).is_empty());
        frame(&mut h, &g, PANE, vec![]);
        let rows = rows_painted(&h, pane_rect(PANE));
        let (s, _) = row_with(&rows, "20");
        assert!(s.ends_with("STOP"), "{s}");
        assert!(script::has_keys(&h.ctx), "the keyboard back on the tile");
    }

    #[test]
    fn o_opens_a_numbered_line_after_the_cursor_and_d_deletes() {
        let mut g = fight("10 RUN icepick\n20 STOP\n30 GOTO 10");
        let mut h = Harness::new(1280.0, 800.0);
        focus_on(&mut h, &g, 20);
        frame(&mut h, &g, PANE, letter('o'));
        frame(&mut h, &g, PANE, vec![]);
        let rows = rows_painted(&h, pane_rect(PANE));
        assert!(
            rows.iter().any(|(s, _)| s.trim() == "25"),
            "the new row: {rows:?}"
        );
        frame(&mut h, &g, PANE, typed("RUN patch"));
        let acts = actions(frame(
            &mut h,
            &g,
            PANE,
            crate::uitest::key(egui::Key::Enter),
        ));
        for a in acts {
            g.apply(a);
        }
        assert_eq!(g.program.line(25).unwrap().text, "RUN patch");
        frame(&mut h, &g, PANE, vec![]);
        assert_eq!(script::view(&h.ctx, &g).cursor, Some(25));
        let acts = actions(frame(&mut h, &g, PANE, letter('d')));
        assert_eq!(acts.len(), 1);
        for a in acts {
            g.apply(a);
        }
        assert_eq!(program(&g), ["10 RUN icepick", "20 STOP", "30 GOTO 10"]);
        assert_eq!(
            script::view(&h.ctx, &g).cursor,
            Some(30),
            "on the next line"
        );
    }

    #[test]
    fn a_double_click_retypes_the_line() {
        let g = fight("10 RUN icepick\n20 STOP\n30 GOTO 10");
        let mut h = Harness::new(1280.0, 800.0);
        focus_on(&mut h, &g, 30);
        let at = h
            .painted_text()
            .into_iter()
            .find(|(t, r)| t == "G" && pane_rect(PANE).contains(r.center()))
            .unwrap()
            .1
            .center();
        // Well after the click that focused it, or the pair reads as a triple click.
        h.wait(1.0);
        frame(&mut h, &g, PANE, crate::uitest::click(at));
        frame(&mut h, &g, PANE, crate::uitest::click(at));
        frame(&mut h, &g, PANE, vec![]);
        assert!(h.ctx.memory(|m| m.has_focus(script::edit_id())), "editing");
    }

    #[test]
    fn the_legend_lights_only_the_keys_that_press() {
        let g = fight("");
        let p = pal(&g);
        let mut h = Harness::new(1280.0, 800.0);
        frame(&mut h, &g, PANE, vec![]);
        script::take_keys(&h.ctx);
        frame(&mut h, &g, PANE, vec![]);
        let rows = rows_painted(&h, pane_rect(PANE));
        let (s, cs) = row_with(&rows, "eedit");
        let at = |word: &str| cs[s.find(word).unwrap()];
        assert_eq!(at("eedit"), p.dim, "no line to edit: {s}");
        assert_eq!(at("ddel"), p.dim, "no line to delete");
        assert_eq!(at("oopen"), p.fg, "a first line to open");
        let (s, cs) = row_with(&rows, "rrun");
        assert_eq!(
            cs[s.find("rrun").unwrap()],
            p.fg,
            "paused in a fight: run presses"
        );
    }

    #[test]
    fn typing_in_the_frame_that_starts_a_retype_goes_into_the_line() {
        let mut g = fight("10 RUN icepick\n20 STOP\n30 GOTO 10");
        let mut h = Harness::new(1280.0, 800.0);
        focus_on(&mut h, &g, 20);
        let mut ev = letter('e');
        ev.extend(typed("X"));
        frame(&mut h, &g, PANE, ev);
        let acts = actions(frame(
            &mut h,
            &g,
            PANE,
            crate::uitest::key(egui::Key::Enter),
        ));
        for a in acts {
            g.apply(a);
        }
        assert_eq!(
            g.program.line(20).unwrap().text,
            "STOPX",
            "the e itself typed nothing"
        );
    }

    #[test]
    fn ctrl_c_in_a_retype_puts_the_line_back() {
        let g = fight("10 RUN icepick\n20 STOP\n30 GOTO 10");
        let mut h = Harness::new(1280.0, 800.0);
        focus_on(&mut h, &g, 20);
        frame(&mut h, &g, PANE, letter('e'));
        let ctrl_c = vec![egui::Event::Key {
            key: egui::Key::C,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::CTRL,
        }];
        let out = frame(&mut h, &g, PANE, ctrl_c);
        assert!(actions(out).is_empty());
        frame(&mut h, &g, PANE, vec![]);
        assert!(
            script::view(&h.ctx, &g).edit.is_none(),
            "the retype is over"
        );
        assert!(script::has_keys(&h.ctx), "the tile has the keys back");
        // Enter now retypes again, it doesn't commit an empty line over 20.
        let out = frame(&mut h, &g, PANE, crate::uitest::key(egui::Key::Enter));
        assert!(actions(out).is_empty());
    }

    #[test]
    fn a_held_enter_is_the_tiles_and_nothing_after_hears_it() {
        let g = fight("10 RUN icepick\n20 STOP");
        let mut h = Harness::new(1280.0, 800.0);
        let held = vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }];
        // Enter goes down while something else has it (the row that committed), and is
        // still held when the tile has the keys back: egui calls that press a repeat.
        h.frame(held.clone(), |_| {});
        let mut left = true;
        h.frame(held, |ui| {
            script::take_keys(ui.ctx());
            let (mut suite, mut popup) = (
                crate::suite::State::default(),
                crate::volume::Popup::default(),
            );
            let mut cx = FrameCx {
                game: &g,
                pal: pal(&g),
                suite: &mut suite,
                keys_free: true,
                level: crate::volume::Level {
                    on: true,
                    volume: 100,
                },
                popup: &mut popup,
            };
            crate::chrome::Btop.keys(ui.ctx(), &mut cx);
            left = ui.input(|i| i.key_pressed(egui::Key::Enter));
        });
        assert!(
            !left,
            "the run's hotkeys would have sent it to the terminal"
        );
    }

    fn long_program() -> String {
        (1..=60)
            .map(|i| format!("{} REM line{i}\n", i * 10))
            .chain(["610 STOP".to_string()])
            .collect()
    }

    #[test]
    fn a_trackpads_little_deltas_scroll_the_listing() {
        let mut g = fight(&long_program());
        g.phase = ninsei_core::Phase::Exploring;
        let mut h = Harness::new(1280.0, 800.0);
        let size = (380.0, 300.0);
        frame(&mut h, &g, size, vec![]);
        let at = pane_rect(size).center();
        // A fifth of a row a frame, under any one frame's half row.
        for _ in 0..40 {
            let ev = vec![
                egui::Event::PointerMoved(at),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: vec2(0.0, -3.0),
                    modifiers: egui::Modifiers::NONE,
                    phase: egui::TouchPhase::Move,
                },
            ];
            frame(&mut h, &g, size, ev);
        }
        assert!(
            script::view(&h.ctx, &g).list.top > 0,
            "the listing scrolled"
        );
    }

    #[test]
    fn a_smaller_tile_keeps_the_line_about_to_run_in_view() {
        let mut g = fight(&long_program());
        g.exec.pc = Some(400);
        let mut h = Harness::new(1280.0, 1400.0);
        let tall = (380.0, 900.0);
        frame(&mut h, &g, tall, vec![]);
        let small = (380.0, 300.0);
        frame(&mut h, &g, small, vec![]);
        let rows = rows_painted(&h, pane_rect(small));
        let (s, _) = row_with(&rows, "400");
        assert!(s.starts_with('▶'), "{s}");
    }

    #[test]
    fn the_wheel_never_scrolls_a_retyped_row_away() {
        let g = fight(&long_program());
        let mut h = Harness::new(1280.0, 800.0);
        let size = (380.0, 300.0);
        frame(&mut h, &g, size, vec![]);
        let at = pane_rect(size).center();
        frame(&mut h, &g, size, crate::uitest::click(at));
        let no = script::view(&h.ctx, &g).cursor.unwrap();
        frame(&mut h, &g, size, letter('e'));
        // Off the row being retyped, over the listing below it.
        let below = at + vec2(0.0, 60.0);
        for _ in 0..10 {
            let ev = vec![
                egui::Event::PointerMoved(below),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: vec2(0.0, -200.0),
                    modifiers: egui::Modifiers::NONE,
                    phase: egui::TouchPhase::Move,
                },
            ];
            frame(&mut h, &g, size, ev);
        }
        assert!(
            h.ctx.memory(|m| m.has_focus(script::edit_id())),
            "the row being retyped is still on screen with the keys"
        );
        let rows = rows_painted(&h, pane_rect(size));
        assert!(
            rows.iter().any(|(s, _)| s.starts_with(&no.to_string())),
            "{rows:?}"
        );
    }

    #[test]
    fn the_tile_stays_whole_inside_its_pane_at_three_sizes() {
        let g = fight("10 REM yo wintermute\n20 IF INTEG < 8\n30 RUN patch\n40 STOP\n50 GOTO 20");
        for size in [(220.0, 160.0), PANE, (700.0, 900.0)] {
            let mut h = Harness::new(1280.0, 1400.0);
            frame(&mut h, &g, size, vec![]);
            frame(&mut h, &g, size, vec![]);
            let room = pane_rect(size).expand(0.5);
            let tile = pane_rect(size);
            for s in h.painted() {
                let r = s.visual_bounding_rect();
                if room.contains(r.center()) {
                    assert!(room.contains_rect(r), "{size:?}: {r:?} outside {room:?}");
                }
            }
            let cut: Vec<_> = h
                .cut_text()
                .into_iter()
                .filter(|(_, r)| tile.contains(r.min))
                .collect();
            assert!(cut.is_empty(), "{size:?}: cut at a clip rect: {cut:?}");
            let rows = rows_painted(&h, tile);
            assert!(
                rows.iter().any(|(s, _)| s.starts_with("ice.bas")),
                "{rows:?}"
            );
        }
    }

    #[test]
    fn the_line_about_to_run_is_lit_and_the_others_are_not() {
        let g = fight("10 RUN icepick\n20 STOP\n30 JUMP 10\n40 GOTO 10");
        let p = pal(&g);
        let mut h = Harness::new(1280.0, 800.0);
        frame(&mut h, &g, PANE, vec![]);
        let rows = rows_painted(&h, pane_rect(PANE));
        let (s, cs) = row_with(&rows, "RUNicepick");
        assert!(s.starts_with('▶'), "the gutter: {s}");
        assert!(cs.iter().all(|&c| c == p.bright), "lit: {cs:?}");
        let (s, cs) = row_with(&rows, "STOP");
        assert!(s.starts_with('●'), "a STOP's dot: {s}");
        assert!(!cs[1..].contains(&p.bright), "not lit past its dot: {cs:?}");
        let (s, cs) = row_with(&rows, "JUMP10");
        assert!(s.contains("?SYNTAX"), "its fault after it: {s}");
        assert!(cs.iter().all(|&c| c == p.dim), "dim: {cs:?}");
        let (_, cs) = row_with(&rows, "GOTO10");
        assert!(!cs.contains(&p.bright));
        // The budgets in the bottom edge, btop's words.
        let c = g.combat.as_ref().unwrap();
        let note = format!(
            "cpu{}/{}ram{}/{}",
            c.cpu, g.deck.cpu_max, c.ifs, g.deck.ram_max
        );
        assert!(rows.iter().any(|(s, _)| s.contains(&note)), "{rows:?}");
    }

    #[test]
    fn a_long_program_scrolls_to_the_line_about_to_run() {
        let text: String = (1..=60)
            .map(|i| format!("{} REM line{i}\n", i * 10))
            .chain(["610 STOP".to_string()])
            .collect();
        let mut g = fight(&text);
        g.exec.pc = Some(600);
        let mut h = Harness::new(1280.0, 800.0);
        let size = (380.0, 300.0);
        frame(&mut h, &g, size, vec![]);
        let rows = rows_painted(&h, pane_rect(size));
        let (s, _) = row_with(&rows, "600");
        assert!(s.starts_with('▶'), "{s}");
        assert!(
            !rows.iter().any(|(s, _)| s.contains("REMline1")),
            "the top scrolled off"
        );
    }

    #[test]
    fn a_click_on_a_row_puts_the_cursor_there_and_takes_the_keys() {
        let g = fight("10 RUN icepick\n20 STOP\n30 GOTO 10");
        let p = pal(&g);
        let mut h = Harness::new(1280.0, 800.0);
        frame(&mut h, &g, PANE, vec![]);
        let at = h
            .painted_text()
            .into_iter()
            .find(|(t, r)| t == "S" && pane_rect(PANE).contains(r.center()))
            .expect("STOP's S")
            .1
            .center();
        frame(&mut h, &g, PANE, crate::uitest::click(at));
        assert!(script::has_keys(&h.ctx), "the tile has the keys");
        assert_eq!(script::view(&h.ctx, &g).cursor, Some(20));
        frame(&mut h, &g, PANE, vec![]);
        // The cursor row in inverse: its glyphs on the paper's colour, and the border lit.
        let rows = rows_painted(&h, pane_rect(PANE));
        let (_, cs) = row_with(&rows, "STOP");
        assert!(cs[1..].iter().all(|&c| c == p.bg), "{cs:?}");
        assert!(
            h.rects()
                .iter()
                .any(|(_, fill, stroke)| *fill == Color32::TRANSPARENT && *stroke == p.bright),
            "the border lit"
        );
    }
}
