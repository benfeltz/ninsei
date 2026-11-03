//! The network as a 1-bit MacPaint picture on a card (the Sense/Net deck): boxes filled with
//! patterns (bricks for barriers, crosshatch for code gates, dots for sentries, solid black
//! ICE), hatched drop shadows, 1-px links with a dotted shadow, a 50% checker for the fog,
//! marching ants round you, and crosshairs on contract targets. Two theme colours, lit ink on
//! dark paper; danger shows as denser patterns and faster ants, never colour.

use egui::{Align2, Color32, FontFamily, FontId, Painter, Pos2, Rect, Sense, Shape, Stroke, Ui};
use egui::{StrokeKind, pos2, vec2};
use ninsei_core::netview::{MapNode, PAINT_PAPER, PAINT_SOLID, flat_layout, style};
use ninsei_core::{Game, NodeKind, Phase};

use crate::netmap::{Anim, hint, interact, kind, tag, target, visible};
use crate::patterns;
use crate::theme::Palette;

/// Every size and timing the Paint view uses.
mod look {
    /// A node's box.
    pub const W: f32 = 58.0;
    pub const H: f32 = 36.0;
    /// How far the hatched shadow falls.
    pub const DROP: f32 = 5.0;
    /// Room round the picture: the card's frame, then labels and the hint below.
    pub const PAD: f32 = 46.0;
    pub const BOTTOM: f32 = 48.0;
    /// Marching ants: dash and gap, and how fast they march (points a second), calm and in
    /// danger.
    pub const ANT: f32 = 4.0;
    pub const ANTS_CALM: f32 = 10.0;
    pub const ANTS_FAST: f32 = 40.0;
    pub const FONT: f32 = 12.0;
}

#[derive(Clone, Copy)]
struct Two {
    ink: Color32,
    paper: Color32,
}

fn small() -> FontId {
    FontId::new(look::FONT, FontFamily::Monospace)
}

/// Fills `r` with a paint: a pattern id, `paper` or `solid`.
fn paint(painter: &Painter, game: &Game, r: Rect, fill: &str, c: Two) {
    match fill {
        PAINT_PAPER => {
            painter.rect_filled(r, 0.0, c.paper);
        }
        PAINT_SOLID => {
            painter.rect_filled(r, 0.0, c.ink);
        }
        id => match game.data.pattern(id) {
            Some(p) => patterns::fill(painter, p, r, c.ink, c.paper),
            None => {
                painter.rect_filled(r, 0.0, c.paper);
            }
        },
    }
}

/// Text on a solid backing, so it never sits on a pattern.
fn label(painter: &Painter, at: Pos2, align: Align2, text: &str, c: Two) -> Rect {
    let g = painter.layout_no_wrap(text.to_string(), small(), c.ink);
    let r = align.anchor_size(at, g.size() + vec2(6.0, 2.0));
    painter.rect_filled(r, 0.0, c.paper);
    painter.galley(r.min + vec2(3.0, 1.0), g, c.ink);
    r
}

/// The pattern the card's frame wears: denser as the trace climbs.
fn pressure_pattern(trace: u32) -> &'static str {
    match trace {
        0..25 => "p12",
        25..50 => "p25",
        50..75 => "p50",
        _ => "p75",
    }
}

/// Marching ants round `r`, marching at `speed` points a second.
fn ants(painter: &Painter, r: Rect, t: f64, speed: f32, ink: Color32) {
    let pts = [
        r.left_top(),
        r.right_top(),
        r.right_bottom(),
        r.left_bottom(),
        r.left_top(),
    ];
    let offset = (t as f32 * speed) % (look::ANT * 2.0);
    painter.extend(Shape::dashed_line_with_offset(
        &pts,
        Stroke::new(1.0, ink),
        &[look::ANT],
        &[look::ANT],
        offset,
    ));
}

pub fn show(ui: &mut Ui, game: &Game, p: &Palette) -> Option<usize> {
    let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
    let painter = ui.painter_at(rect);
    let c = Two {
        ink: p.fg,
        paper: p.bg,
    };
    let fx = Anim::frame(ui, game);
    let t = fx.t;
    // Ants always march.
    ui.ctx().request_repaint();

    // The card: paper, framed by a band whose pattern darkens with the trace.
    painter.rect_filled(rect, 0.0, c.paper);
    let card = rect.shrink(6.0);
    let band = pressure_pattern(game.deck.trace);
    paint(&painter, game, card, band, c);
    let inside = card.shrink(6.0);
    painter.rect_filled(inside, 0.0, c.paper);
    painter.rect_stroke(inside, 0.0, Stroke::new(1.0, c.ink), StrokeKind::Inside);

    let net = &game.net;
    let layers = net.layers.len();
    let area = Rect::from_min_max(
        inside.min + vec2(look::PAD, look::PAD - 10.0),
        inside.max - vec2(look::PAD, look::BOTTOM),
    );
    let spots = flat_layout(net);
    let pos = |id: usize| {
        let (x, y) = spots[id];
        pos2(
            (area.left() + area.width() * x).round(),
            (area.top() + area.height() * y).round(),
        )
    };
    let shown = |id: usize| fx.reveal(net.nodes[id].layer, layers) > 0.0;
    let reach = game.reachable();

    // Links, each with a dotted shadow below and right of it.
    for n in &net.nodes {
        for &to in &n.links {
            if !shown(n.id) || !shown(to) {
                continue;
            }
            let (a, b) = (
                pos(n.id) + vec2(look::W / 2.0, 0.0),
                pos(to) - vec2(look::W / 2.0, 0.0),
            );
            let known = visible(game, n.id) && visible(game, to);
            let shadow = [a + vec2(3.0, 3.0), b + vec2(3.0, 3.0)];
            painter.extend(Shape::dashed_line(
                &shadow,
                Stroke::new(1.0, c.ink),
                1.0,
                2.0,
            ));
            let width = if game.travelled(n.id, to) {
                3.0
            } else if n.id == game.at && reach.contains(&to) {
                2.0
            } else {
                1.0
            };
            if known {
                painter.line_segment([a, b], Stroke::new(width, c.ink));
            } else {
                painter.extend(Shape::dashed_line(
                    &[a, b],
                    Stroke::new(1.0, c.ink),
                    4.0,
                    4.0,
                ));
            }
        }
    }

    let mut clicked = None;
    let danger = game.deck.trace >= crate::netmap::anim::TRACE_FAST || game.phase == Phase::Combat;
    for n in &net.nodes {
        if !shown(n.id) {
            continue;
        }
        let ctr = pos(n.id);
        let r = Rect::from_center_size(ctr, vec2(look::W, look::H));
        let (hit, hovered) = interact(ui, game, n.id, r.expand(6.0));
        clicked = clicked.or(hit);
        let here = n.id == game.at;
        let seen = visible(game, n.id);
        let can_go = reach.contains(&n.id);
        let done = n.cleared && !here && n.kind != NodeKind::Entry;
        let k = kind(game, n.id);

        // The hatched drop shadow, then the box.
        paint(
            &painter,
            game,
            r.translate(vec2(look::DROP, look::DROP)),
            "diagonal",
            c,
        );
        let fill = if !seen {
            "p50"
        } else if let Some(b) = fx.burst(n.id) {
            // Blowing apart: dense, then thinning out.
            if b < 0.5 { "crosshatch" } else { "p50" }
        } else if done && matches!(n.kind, NodeKind::Ice | NodeKind::BlackIce) {
            "p25"
        } else if done {
            "p12"
        } else {
            style(&game.data, k).paint.as_str()
        };
        paint(&painter, game, r, fill, c);
        let edge = if can_go && hovered { 2.5 } else { 1.0 };
        painter.rect_stroke(r, 0.0, Stroke::new(edge, c.ink), StrokeKind::Inside);

        // What's inside: a glyph on blank nodes, a `?` in the fog. Always on paper.
        if !seen {
            label(&painter, ctr, Align2::CENTER_CENTER, "?", c);
        } else if fill == PAINT_PAPER {
            let text = tag(game, n.id);
            let text = if k == MapNode::Bbs {
                "BBS".to_string()
            } else {
                text
            };
            painter.text(ctr, Align2::CENTER_CENTER, text, small(), c.ink);
        }
        if seen && k == MapNode::Subnet && !done {
            // A box in the box: the private network behind the door.
            let inner = r.shrink2(vec2(16.0, 9.0));
            painter.rect_filled(inner, 0.0, c.paper);
            painter.rect_stroke(inner, 0.0, Stroke::new(1.0, c.ink), StrokeKind::Inside);
        }

        // The name under it, on paper.
        let name = if seen { n.name.as_str() } else { "???" };
        let under = if done && seen {
            format!("{name} ok")
        } else {
            name.to_string()
        };
        label(
            &painter,
            pos2(ctr.x, r.bottom() + look::DROP + 3.0),
            Align2::CENTER_TOP,
            &under,
            c,
        );

        if here {
            let speed = if danger {
                look::ANTS_FAST
            } else {
                look::ANTS_CALM
            };
            ants(&painter, r.expand(7.0), t, speed, c.ink);
        }
        if target(game, n.id) {
            // Crosshairs and a `$`.
            let s = Stroke::new(1.5, c.ink);
            let (x, y) = (ctr.x, ctr.y);
            let (hw, hh) = (look::W / 2.0, look::H / 2.0);
            painter.line_segment([pos2(x, y - hh - 14.0), pos2(x, y - hh - 4.0)], s);
            painter.line_segment([pos2(x, y + hh + 4.0), pos2(x, y + hh + 9.0)], s);
            painter.line_segment([pos2(x - hw - 14.0, y), pos2(x - hw - 4.0, y)], s);
            painter.line_segment([pos2(x + hw + 4.0, y), pos2(x + hw + 14.0, y)], s);
            label(
                &painter,
                pos2(x, y - hh - 14.0),
                Align2::CENTER_BOTTOM,
                "$",
                c,
            );
        }
    }

    // The hint along the bottom of the card.
    label(
        &painter,
        pos2(inside.left() + 10.0, inside.bottom() - 8.0),
        Align2::LEFT_BOTTOM,
        hint(game),
        c,
    );
    clicked
}

#[cfg(test)]
mod tests {
    use super::*;
    use ninsei_core::Data;

    #[test]
    fn the_frame_darkens_with_the_trace() {
        let d = Data::builtin().unwrap();
        let cover = |t| d.pattern(pressure_pattern(t)).unwrap().coverage_pct();
        assert!(cover(0) < cover(30));
        assert!(cover(30) < cover(60));
        assert!(cover(60) < cover(90));
    }

    #[test]
    fn it_draws_a_run_without_panicking() {
        let d = Data::builtin().unwrap();
        let ctx = egui::Context::default();
        let p = Palette::from_def(&d.themes[0]);
        for seed in 0..6 {
            let g = Game::new(d.clone(), seed);
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 600.0))),
                ..Default::default()
            };
            let mut out = ctx.run_ui(raw, |ui| {
                show(ui, &g, &p);
            });
            out.textures_delta.clear();
        }
    }
}
