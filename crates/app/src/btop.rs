//! The Ono-Sendai's chrome: btop-style panes. Boxes with their title cut into the border,
//! gradient meters, dot graphs at braille resolution (the font has no braille, so the dots are
//! painted), and waterfalls. Every colour comes from the theme: gradients run
//! `faint → dim → fg → bright`. History comes from `Game::history`; the maths from
//! `ninsei_core::history::plot`.

use egui::{
    Align2, Color32, FontFamily, FontId, Painter, Pos2, Rect, RichText, ScrollArea, Sense, Stroke,
    Ui, UiBuilder, pos2, vec2,
};
use ninsei_core::history::plot;
use ninsei_core::{Action, ContractStatus, Effect, LogKind, Phase, tuning};

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

/// A btop box filling the pane: rounded border, the title cut into the top edge and an
/// optional note cut into the bottom edge. `body` draws inside.
pub fn boxed(ui: &mut Ui, p: &Palette, title: &str, note: &str, body: impl FnOnce(&mut Ui)) {
    let rect = ui.available_rect_before_wrap().shrink(3.0);
    let painter = ui.painter().clone();
    painter.rect_stroke(rect, 6.0, Stroke::new(1.0, p.dim), egui::StrokeKind::Middle);
    let cut = |at: Pos2, text: &str, color: Color32, align: Align2| {
        let galley = painter.layout_no_wrap(text.to_string(), mono(MONO), color);
        let r = align.anchor_size(at, galley.size()).expand2(vec2(4.0, 0.0));
        painter.rect_filled(r, 0.0, p.bg);
        painter.galley(r.min + vec2(4.0, 0.0), galley, color);
    };
    cut(
        rect.left_top() + vec2(14.0, 0.0),
        title,
        p.bright,
        Align2::LEFT_CENTER,
    );
    if !note.is_empty() {
        cut(
            rect.right_bottom() - vec2(14.0, 0.0),
            note,
            p.dim,
            Align2::RIGHT_CENTER,
        );
    }
    let inner = rect.shrink2(vec2(10.0, 12.0));
    ui.scope_builder(UiBuilder::new().max_rect(inner), body);
    ui.allocate_rect(rect, Sense::hover());
}

/// A gradient meter `cells` wide: filled cells take the ramp's colour at their position,
/// empty cells are dim dots. The value eases toward its target.
fn meter(ui: &mut Ui, p: &Palette, id: &str, value: u32, max: u32, cells: usize) {
    let shown = ui.ctx().animate_value_with_time(
        ui.id().with(("meter", id)),
        value as f32,
        anim::EASE_SECS,
    );
    let cw = ui.fonts_mut(|f| f.glyph_width(&mono(MONO), 'M'));
    let (rect, _) = ui.allocate_exact_size(vec2(cw * cells as f32, MONO + 2.0), Sense::hover());
    let painter = ui.painter_at(rect);
    let frac = if max == 0 {
        0.0
    } else {
        (shown / max as f32).clamp(0.0, 1.0)
    };
    let on = frac * cells as f32;
    for i in 0..cells {
        let r = Rect::from_min_size(
            pos2(rect.left() + i as f32 * cw, rect.top() + 2.0),
            vec2(cw - 1.5, MONO - 2.0),
        );
        let k = (i as f32 + 0.5) / cells as f32;
        let fill = (on - i as f32).clamp(0.0, 1.0);
        if fill > 0.0 {
            let part = Rect::from_min_size(r.min, vec2(r.width() * fill, r.height()));
            painter.rect_filled(part, 1.0, ramp(p, k));
        }
        if fill < 1.0 {
            painter.circle_filled(r.center(), 1.0, p.faint);
        }
    }
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
                    value: g.ram_used(),
                    max: d.ram_max,
                    note: format!("{}/{} GB", g.ram_used(), d.ram_max),
                    series: h.series(|s| s.ram),
                    hot: g.ram_used() == d.ram_max,
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
        let note = format!(
            "ram {}/{}  hdd {}/{}",
            g.ram_used(),
            g.deck.ram_max,
            g.hdd.len(),
            g.deck.hdd_max
        );
        let mut acts = Vec::new();
        let mut hex = None;
        boxed(ui, &p, "warez/", &note, |ui| {
            ui.label(text(
                format!("{:<3}{:<11}{:>3}{:>4}", "ID", "NAME", "RAM", "CPU"),
                p.dim,
            ));
            let mut order: Vec<usize> = (0..g.hdd.len()).collect();
            order.sort_by_key(|&i| !g.hdd[i].loaded);
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
                        let col = if lit {
                            p.bright
                        } else if s.loaded {
                            p.fg
                        } else {
                            p.dim
                        };
                        let row = ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 3.0;
                            // Buttons pinned right, then the activity graph, then the row text
                            // takes what's left.
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if s.loaded {
                                        if ui.small_button("ul").on_hover_text("unload").clicked() {
                                            acts.push(Action::Unload(i));
                                        }
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
                                    } else {
                                        let fits = g.ram_used() + w.ram <= g.deck.ram_max;
                                        if ui
                                            .add_enabled(fits, egui::Button::new("ld").small())
                                            .on_hover_text("load")
                                            .clicked()
                                        {
                                            acts.push(Action::Load(i));
                                        }
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
                                            let name: String = if w.opt > 0 {
                                                let base: String =
                                                    w.file().chars().take(7).collect();
                                                format!("{base}+O{}", w.opt)
                                            } else {
                                                w.file().chars().take(10).collect()
                                            };
                                            ui.add(
                                                egui::Label::new(text(
                                                    format!(
                                                        "{:<3}{:<11}{:>3}{:>4}",
                                                        i, name, w.ram, w.cpu
                                                    ),
                                                    col,
                                                ))
                                                .truncate(),
                                            )
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
            if fighting {
                ui.label(text(
                    format!("loading mid-fight costs {} cpu", tuning::COMBAT_LOAD_CPU),
                    p.dim,
                ));
            }
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
                    for (i, s) in g.hdd.iter().enumerate().filter(|(_, s)| s.loaded) {
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
    use crate::cells::Cells;
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
    use crate::cells::Cells;
    let level = crate::vispane::held(ui, "btop-vis", ninsei_core::scope::level(samples));
    let note = ninsei_core::scope::db_text(level);
    boxed(ui, p, "vis", note.trim(), |ui| {
        let area = ui.available_rect_before_wrap();
        let c = Cells::fit(ui, area, p.dim, p.bg, p.faint);
        let n = (c.cols / 2).max(1);
        let bars = crate::vispane::bars(ui, "btop-vis", samples, rate, n);
        let painter = ui.painter_at(area);
        let left = (c.cols - (n * 2 - 1).min(c.cols)) / 2;
        for (i, lit) in ninsei_core::scope::cells_lit(&bars, c.rows)
            .into_iter()
            .enumerate()
        {
            for r in 0..lit {
                let k = (r as f32 + 1.0) / c.rows as f32;
                let cell = c.at(left + i * 2, c.rows - 1 - r, 1, 1);
                painter.rect_filled(cell.shrink2(vec2(0.0, c.ch * 0.1)), 0.0, ramp(p, k));
            }
        }
        ui.allocate_rect(area, Sense::hover());
    });
}
