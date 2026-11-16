//! The network as a 1-bit MacPaint picture on a card (the Sense/Net deck): boxes filled with
//! patterns (bricks for barriers, crosshatch for code gates, dots for sentries, solid black
//! ICE), hatched drop shadows, 1-px links with a dotted shadow, a 50% checker for the fog,
//! marching ants round you, and crosshairs on contract targets. Two theme colours, lit ink on
//! dark paper; danger shows as denser patterns and faster ants, never colour.

use egui::{Align2, Color32, FontFamily, FontId, Painter, Pos2, Rect, Sense, Shape, Stroke, Ui};
use egui::{StrokeKind, pos2, vec2};
use ninsei_core::netview::{
    self, AiMark, MapNode, MapView, PAINT_PAPER, PAINT_SOLID, flat_layout, style,
};
use ninsei_core::{Game, NodeKind, Phase};

use crate::netmap::{Anim, arrowhead, hint, interact, kind, target, visible};
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
    /// How long the arrowhead on a one-way link is.
    pub const ARROW: f32 = 10.0;
}

/// The picture's area on the card's inside: room for labels and the hint below.
fn picture(inside: Rect) -> Rect {
    Rect::from_min_max(
        inside.min + vec2(look::PAD, look::PAD - 10.0),
        inside.max - vec2(look::PAD, look::BOTTOM),
    )
}

/// A spot from `flat_layout` placed on the picture, in whole pixels.
fn place(area: Rect, (x, y): (f32, f32)) -> Pos2 {
    pos2(
        (area.left() + area.width() * x).round(),
        (area.top() + area.height() * y).round(),
    )
}

/// The card's inside on a map drawn in `rect`: the frame's band, then a rule.
fn inside(rect: Rect) -> Rect {
    rect.shrink(6.0).shrink(6.0)
}

/// Node `id`'s click area's centre, on a map drawn in `rect`.
#[cfg(test)]
pub fn spot_in(rect: Rect, game: &Game, id: usize) -> Pos2 {
    place(picture(inside(rect)), flat_layout(&game.net)[id])
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

/// A node's box with its shadow and the label under it: what the AI's card keeps off.
fn on_card(at: Pos2) -> Rect {
    Rect::from_min_max(
        at + vec2(-look::W / 2.0 - 8.0, -look::H / 2.0 - 4.0),
        at + vec2(look::W / 2.0 + 8.0, look::H / 2.0 + look::DROP + 20.0),
    )
}

/// The net's AI on the picture where it doesn't hold a node in sight: a little card pinned near
/// the node, with a line from the node to it: there now in the fog, its name on a card with its
/// own hatched shadow; where it was last known, the card is dotted and says that. Where it holds
/// a node in sight, the node is drawn as it. The card goes where it keeps off every other node
/// and its label (`place_clear`): above, below, right, left, then further out.
fn ai_card(painter: &Painter, at: Pos2, m: &AiMark, c: Two, area: Rect, avoid: &[Rect]) {
    let Some(text) = netview::ai_caption(m, MapView::Paint) else {
        return;
    };
    let g = painter.layout_no_wrap(text, small(), c.ink);
    let size = g.size() + vec2(12.0, 8.0);
    let own = on_card(at);
    let mut spots = Vec::new();
    for gap in [10.0, 30.0, 60.0, 90.0] {
        spots.push(pos2(at.x, own.top() - gap - size.y / 2.0));
        spots.push(pos2(at.x, own.bottom() + gap + size.y / 2.0));
        spots.push(pos2(own.right() + gap + size.x / 2.0, at.y));
        spots.push(pos2(own.left() - gap - size.x / 2.0, at.y));
    }
    // Then anywhere on the card, nearest first.
    let within = area.shrink(4.0);
    let mut rest: Vec<Pos2> = (0..=(within.width() / 8.0) as i32)
        .flat_map(|i| {
            (0..=(within.height() / 8.0) as i32).map(move |j| {
                pos2(
                    within.left() + 8.0 * i as f32,
                    within.top() + 8.0 * j as f32,
                )
            })
        })
        .collect();
    rest.sort_by(|a, b| (*a - at).length().total_cmp(&(*b - at).length()));
    spots.extend(rest);
    let mut keep_off = avoid.to_vec();
    keep_off.push(own);
    // The hint along the card's bottom.
    keep_off.push(Rect::from_min_max(
        pos2(area.left(), area.bottom() - 24.0),
        area.right_bottom(),
    ));
    let r = crate::netmap::place_clear(&spots, size, &keep_off, within);
    let tip = pos2(r.center().x.clamp(r.left(), r.right()), r.center().y);
    let from = pos2(
        tip.x.clamp(at.x - look::W / 2.0, at.x + look::W / 2.0),
        tip.y.clamp(at.y - look::H / 2.0, at.y + look::H / 2.0),
    );
    let to = pos2(
        from.x.clamp(r.left(), r.right()),
        from.y.clamp(r.top(), r.bottom()),
    );
    painter.line_segment([from, to], Stroke::new(1.0, c.ink));
    if m.now {
        painter.rect_filled(r.translate(vec2(look::DROP, look::DROP)), 0.0, c.ink);
        painter.rect_filled(r, 0.0, c.paper);
        painter.rect_stroke(r, 0.0, Stroke::new(2.0, c.ink), StrokeKind::Inside);
    } else {
        painter.rect_filled(r, 0.0, c.paper);
        let pts = [
            r.left_top(),
            r.right_top(),
            r.right_bottom(),
            r.left_bottom(),
            r.left_top(),
        ];
        painter.extend(Shape::dashed_line(&pts, Stroke::new(1.0, c.ink), 1.0, 2.0));
    }
    painter.galley(r.min + vec2(6.0, 4.0), g, c.ink);
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
    let inside = inside(rect);
    painter.rect_filled(inside, 0.0, c.paper);
    painter.rect_stroke(inside, 0.0, Stroke::new(1.0, c.ink), StrokeKind::Inside);

    let net = &game.net;
    let layers = net.layers.len();
    let spots = flat_layout(net);
    let area = picture(inside);
    let pos = |id: usize| place(area, spots[id]);
    let shown = |id: usize| fx.reveal(net.nodes[id].layer, layers) > 0.0;
    let reach = game.reachable();

    // Links, each with a dotted shadow below and right of it; a one-way link carries an
    // arrowhead at its middle, pointing deeper.
    for l in netview::links(game) {
        if !shown(l.from) || !shown(l.to) {
            continue;
        }
        let (a, b) = (
            pos(l.from) + vec2(look::W / 2.0, 0.0),
            pos(l.to) - vec2(look::W / 2.0, 0.0),
        );
        let known = visible(game, l.from) && visible(game, l.to);
        let shadow = [a + vec2(3.0, 3.0), b + vec2(3.0, 3.0)];
        painter.extend(Shape::dashed_line(
            &shadow,
            Stroke::new(1.0, c.ink),
            1.0,
            2.0,
        ));
        let width = if l.walked {
            3.0
        } else if l.route.is_some() {
            2.0
        } else {
            1.0
        };
        if known {
            painter.line_segment([a, b], Stroke::new(width, c.ink));
            if l.one_way {
                arrowhead(&painter, a + (b - a) * 0.5, b - a, look::ARROW, c.ink);
            }
        } else {
            painter.extend(Shape::dashed_line(
                &[a, b],
                Stroke::new(1.0, c.ink),
                4.0,
                4.0,
            ));
        }
    }

    let mut clicked = None;
    // Where the runner knows the net's AI is, worked out once for the frame.
    let ai = netview::ai_mark(game);
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
        // A node the net's AI holds is drawn as the AI: all ink, its eye in paper, its name
        // under the node's.
        let held = ai.as_ref().filter(|m| m.holds(n.id)).map(|m| &m.label);

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
        } else if held.is_some() {
            PAINT_SOLID
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
        // A route, forward or back, is a heavier box; hovered, heavier still.
        let edge = match (can_go, hovered) {
            (true, true) => 2.5,
            (true, false) => 2.0,
            _ => 1.0,
        };
        painter.rect_stroke(r, 0.0, Stroke::new(edge, c.ink), StrokeKind::Inside);

        // What's inside: a glyph on blank nodes, a `?` in the fog. Always on paper.
        if !seen {
            label(&painter, ctr, Align2::CENTER_CENTER, "?", c);
        } else if fill == PAINT_PAPER {
            let text = netview::tag_with(game, n.id, ai.as_ref());
            let text = if k == MapNode::Bbs {
                "BBS".to_string()
            } else {
                text
            };
            painter.text(ctr, Align2::CENTER_CENTER, text, small(), c.ink);
        }
        if held.is_some() {
            let s = Stroke::new(1.0, c.paper);
            painter.rect_stroke(r.shrink(3.0), 0.0, s, StrokeKind::Inside);
            painter.circle_stroke(ctr, 9.0, Stroke::new(1.5, c.paper));
            painter.circle_filled(ctr, 3.0, c.paper);
        } else if seen && k == MapNode::Subnet && !done {
            // A box in the box: the private network behind the door.
            let inner = r.shrink2(vec2(16.0, 9.0));
            painter.rect_filled(inner, 0.0, c.paper);
            painter.rect_stroke(inner, 0.0, Stroke::new(1.0, c.ink), StrokeKind::Inside);
        }

        // The name under it, on paper.
        let name = if seen { n.name.as_str() } else { "???" };
        let under = if done && seen && held.is_none() {
            format!("{name} ok")
        } else {
            name.to_string()
        };
        let below = label(
            &painter,
            pos2(ctr.x, r.bottom() + look::DROP + 3.0),
            Align2::CENTER_TOP,
            &under,
            c,
        );
        if let Some(ai) = &held {
            label(
                &painter,
                pos2(ctr.x, below.bottom()),
                Align2::CENTER_TOP,
                ai,
                c,
            );
        }

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

    // Where it holds a node, the node is drawn as it; anywhere else a card marks it.
    if let Some(m) = ai.filter(|m| !m.held) {
        let avoid: Vec<Rect> = (0..net.nodes.len())
            .filter(|&id| id != m.node)
            .map(|id| on_card(pos(id)))
            .collect();
        ai_card(&painter, pos(m.node), &m, c, inside, &avoid);
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
