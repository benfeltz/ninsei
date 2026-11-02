//! The network as floating wireframe shapes over a grid, in true isometric. Shapes come from
//! `data/netmap.ron`; every colour comes from the theme: the grid is `faint`, the wires `fg`,
//! shape fills `dim`, highlights `bright`.

use egui::{
    Align2, Color32, FontFamily, FontId, Painter, Pos2, Rect, Sense, Shape, Stroke, Ui, pos2, vec2,
};
use ninsei_core::netview::{self, HOVER, Iso, MapNode, Mesh, SPACING, style, world};
use ninsei_core::{Game, Phase};

use crate::netmap::{Anim, anim, board_ink, hint, interact, kind, mix, tag, target, visible};
use crate::theme::Palette;

/// Clickable area around each shape, in pixels.
const HIT: f32 = 56.0;
/// How far fogged nodes' links stay visible: dashed, in `faint`.
const DASH: (f32, f32) = (3.0, 6.0);

pub fn show(ui: &mut Ui, game: &Game, p: &Palette) -> Option<usize> {
    let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, p.bg);
    let fx = Anim::frame(ui, game);
    let t = fx.t as f32;
    ui.ctx().request_repaint();

    let net = &game.net;
    let cam = Iso::fit(
        net,
        rect.width(),
        rect.height() - 24.0,
        [rect.center().x, rect.center().y - 6.0],
    );
    let at = |p3: [f32; 3]| {
        let s = cam.project(p3);
        pos2(s[0], s[1])
    };
    grid(&painter, &cam, game, p, fx.pressure(game.deck.trace));

    let reach = game.reachable();
    let layers = net.layers.len();
    let shown = |id: usize| fx.reveal(net.nodes[id].layer, layers);
    // A hovered route lifts; its click area stays where it was.
    let pointer = ui.input(|i| i.pointer.hover_pos());
    let resting = |id: usize| {
        let w = world(net, id);
        at([w[0], HOVER, w[2]])
    };
    let hover = |id: usize| {
        reach.contains(&id)
            && pointer
                .is_some_and(|q| Rect::from_center_size(resting(id), vec2(HIT, HIT)).contains(q))
    };
    let bob = |id: usize| (t * 2.0 + id as f32).sin() * 0.06;
    let top = |id: usize| {
        let w = world(net, id);
        let lift =
            if id == game.at { 0.15 } else { 0.0 } + if hover(id) { anim::HOVER_LIFT } else { 0.0 };
        [w[0], HOVER + lift + bob(id), w[2]]
    };

    // Links between the floating nodes.
    for n in &net.nodes {
        for &to in &n.links {
            let show = shown(n.id).min(shown(to));
            if show <= 0.0 {
                continue;
            }
            let (a, b) = (at(top(n.id)), at(top(to)));
            let live = n.id == game.at && reach.contains(&to);
            if game.travelled(n.id, to) {
                // The hop trail: the way you came stays lit.
                glow(&painter, a, b, 1.8, p.bright, 0.8 * show);
            } else if live {
                glow(&painter, a, b, 2.2, p.fg, 1.0);
                // A pulse running down the route.
                let k = (t * 0.6 + to as f32 * 0.37).fract();
                let c = a + (b - a) * k;
                painter.circle_filled(c, 6.0, p.bright.gamma_multiply(0.25));
                painter.circle_filled(c, 3.0, p.bright);
            } else if visible(game, n.id) && visible(game, to) {
                let both = net.nodes[n.id].cleared && net.nodes[to].cleared;
                glow(
                    &painter,
                    a,
                    b,
                    1.2,
                    p.fg,
                    show * if both { 0.45 } else { 0.35 },
                );
            } else {
                painter.extend(Shape::dashed_line(
                    &[a, b],
                    Stroke::new(1.0, p.faint),
                    DASH.0,
                    DASH.1,
                ));
            }
        }
    }

    // Nodes, back to front.
    let mut order: Vec<usize> = (0..net.nodes.len()).collect();
    order.sort_by(|&a, &b| cam.project(top(b))[2].total_cmp(&cam.project(top(a))[2]));
    let mut clicked = None;
    let small = FontId::new(10.0, FontFamily::Monospace);
    let body = FontId::new(11.0, FontFamily::Monospace);
    for id in order {
        let n = &net.nodes[id];
        let show = shown(id);
        if show <= 0.0 {
            continue;
        }
        let (base, head) = (at(world(net, id)), at(top(id)));
        painter.extend(Shape::dashed_line(
            &[base, head],
            Stroke::new(1.0, p.dim.gamma_multiply(0.5)),
            2.0,
            4.0,
        ));
        let (hit, hovered) = interact(
            ui,
            game,
            id,
            Rect::from_center_size(resting(id), vec2(HIT, HIT)),
        );
        clicked = clicked.or(hit);
        if !visible(game, id) {
            painter.circle_filled(head, 3.0, p.dim.gamma_multiply(0.7));
            painter.text(
                head - vec2(0.0, 6.0),
                Align2::CENTER_BOTTOM,
                "?",
                body.clone(),
                p.dim,
            );
            continue;
        }
        let here = id == game.at;
        if here {
            floor_ring(&painter, &cam, world(net, id), p);
        }
        let can_go = reach.contains(&id);
        let done = n.cleared && !here;
        let k = kind(game, id);
        let (board_edge, board_label) = board_ink(game, p);
        let edge = if here || (can_go && hovered) {
            p.bright
        } else if done {
            p.dim
        } else if k == MapNode::Bbs {
            board_edge
        } else {
            p.fg
        };
        let mesh = Mesh::of(style(&game.data, k).mesh);
        let spin = t
            * 0.5
            * if hovered && can_go {
                anim::HOVER_SPIN
            } else {
                1.0
            }
            + id as f32;
        if let Some(b) = fx.burst(id) {
            // The ICE blows apart: edges fly outward and fade.
            burst(&painter, &cam, &mesh, top(id), spin, b, p.bright);
        }
        let black = k == MapNode::BlackIce;
        let fill = match (done, black) {
            (true, _) => None,
            (false, true) => Some(p.faint.gamma_multiply(0.85)),
            (false, false) => Some(p.dim.gamma_multiply(0.22)),
        };
        let tilt = if k == MapNode::Exit { 0.0 } else { 0.35 };
        // While bursting, the cleared outline fades back in under the pieces.
        let fade = fx.burst(id).map_or(1.0, |b| (b * 2.0 - 1.0).max(0.0)) * show;
        let ink = Ink {
            edge: edge.gamma_multiply(fade),
            fill: fill.map(|f| f.gamma_multiply(fade)),
            lit: !done,
        };
        draw_mesh(&painter, &cam, &mesh, top(id), (spin, tilt), ink);

        // Labels on the floor under the shape.
        let name = if done {
            p.dim
        } else if k == MapNode::Bbs {
            board_label
        } else {
            p.fg
        };
        painter.text(
            base + vec2(0.0, 10.0),
            Align2::CENTER_TOP,
            &n.name,
            body.clone(),
            name,
        );
        let mut label = tag(game, id);
        if done {
            label.push_str(" ok");
        }
        painter.text(
            base + vec2(0.0, 23.0),
            Align2::CENTER_TOP,
            label,
            small.clone(),
            if done { p.dim } else { p.bright },
        );
        if target(game, id) {
            bounty(&painter, head, p);
        }
        if here && game.phase == Phase::Exploring && (t * 2.0) as i64 % 2 == 0 {
            painter.text(
                head + vec2(HIT / 2.0, 0.0),
                Align2::LEFT_CENTER,
                "_",
                body.clone(),
                p.bright,
            );
        }
    }

    painter.text(
        rect.left_bottom() + vec2(8.0, -6.0),
        Align2::LEFT_BOTTOM,
        hint(game),
        small,
        p.dim,
    );
    clicked
}

/// A line with a soft glow: a wide faint stroke under a thin bright one.
fn glow(painter: &Painter, a: Pos2, b: Pos2, w: f32, c: Color32, alpha: f32) {
    painter.line_segment([a, b], Stroke::new(w * 5.0, c.gamma_multiply(0.12 * alpha)));
    painter.line_segment([a, b], Stroke::new(w * 2.2, c.gamma_multiply(0.3 * alpha)));
    painter.line_segment([a, b], Stroke::new(w, c.gamma_multiply(alpha)));
}

/// The floor grid, a little past the network on every side. `pressure` (0..1, from trace)
/// lights it up from `faint` toward `bright`.
fn grid(painter: &Painter, cam: &Iso, game: &Game, p: &Palette, pressure: f32) {
    let ink = mix(p.faint, p.bright, pressure * 0.55);
    let net = &game.net;
    let widest = net.layers.iter().map(Vec::len).max().unwrap_or(1) as f32;
    let (x0, x1) = (-4.0 * SPACING, (net.layers.len() as f32 + 3.0) * SPACING);
    let (z0, z1) = (-(widest + 2.0) * SPACING, (widest + 2.0) * SPACING);
    let step = SPACING / 2.0;
    let line = |a: [f32; 3], b: [f32; 3]| {
        let (a, b) = (cam.project(a), cam.project(b));
        painter.line_segment([pos2(a[0], a[1]), pos2(b[0], b[1])], Stroke::new(1.0, ink));
    };
    let mut x = x0;
    while x <= x1 {
        line([x, 0.0, z0], [x, 0.0, z1]);
        x += step;
    }
    let mut z = z0;
    while z <= z1 {
        line([x0, 0.0, z], [x1, 0.0, z]);
        z += step;
    }
}

/// The rings on the floor under the node you're on.
fn floor_ring(painter: &Painter, cam: &Iso, at: [f32; 3], p: &Palette) {
    for (r, w, a) in [(0.75, 2.0, 0.9), (0.97, 1.0, 0.35)] {
        let pts: Vec<Pos2> = (0..=40)
            .map(|k| {
                let a = k as f32 / 40.0 * std::f32::consts::TAU;
                let s = cam.project([at[0] + a.cos() * r, 0.0, at[2] + a.sin() * r]);
                pos2(s[0], s[1])
            })
            .collect();
        painter.add(Shape::line(
            pts.clone(),
            Stroke::new(w * 3.0, p.bright.gamma_multiply(0.12 * a)),
        ));
        painter.add(Shape::line(pts, Stroke::new(w, p.bright.gamma_multiply(a))));
    }
}

/// How a shape is inked: edge colour, see-through fill, and whether it glows.
struct Ink {
    edge: Color32,
    fill: Option<Color32>,
    lit: bool,
}

/// A shape blowing apart: each edge flies out from the centre along its midpoint, and fades.
/// `k` runs 0..1.
fn burst(painter: &Painter, cam: &Iso, mesh: &Mesh, at: [f32; 3], yaw: f32, k: f32, c: Color32) {
    let ease = 1.0 - (1.0 - k) * (1.0 - k);
    let alpha = 1.0 - k;
    for &[a, b] in &mesh.e {
        let (va, vb) = (
            netview::spin(mesh.v[a], yaw, 0.35),
            netview::spin(mesh.v[b], yaw, 0.35),
        );
        let mid = [
            (va[0] + vb[0]) / 2.0,
            (va[1] + vb[1]) / 2.0,
            (va[2] + vb[2]) / 2.0,
        ];
        let push = mid.map(|x| x * anim::BURST_SPREAD * ease);
        let end = |v: [f32; 3]| {
            let s = cam.project([
                at[0] + v[0] + push[0],
                at[1] + v[1] + push[1],
                at[2] + v[2] + push[2],
            ]);
            pos2(s[0], s[1])
        };
        painter.line_segment(
            [end(va), end(vb)],
            Stroke::new(3.0, c.gamma_multiply(0.15 * alpha)),
        );
        painter.line_segment(
            [end(va), end(vb)],
            Stroke::new(1.2, c.gamma_multiply(alpha)),
        );
    }
}

/// A spinning wireframe: see-through faces back to front, then glowing edges.
/// `turn` is (yaw, tilt) in radians.
fn draw_mesh(painter: &Painter, cam: &Iso, mesh: &Mesh, at: [f32; 3], turn: (f32, f32), ink: Ink) {
    let Ink { edge, fill, lit } = ink;
    let (yaw, tilt) = turn;
    let pts: Vec<[f32; 3]> = mesh
        .v
        .iter()
        .map(|&v| {
            let r = netview::spin(v, yaw, tilt);
            cam.project([r[0] + at[0], r[1] + at[1], r[2] + at[2]])
        })
        .collect();
    let scr = |i: usize| pos2(pts[i][0], pts[i][1]);
    if let Some(fill) = fill {
        let mut faces: Vec<(&Vec<usize>, f32)> = mesh
            .f
            .iter()
            .map(|f| {
                (
                    f,
                    f.iter().map(|&i| pts[i][2]).sum::<f32>() / f.len() as f32,
                )
            })
            .collect();
        faces.sort_by(|a, b| b.1.total_cmp(&a.1));
        for (f, _) in faces {
            painter.add(Shape::convex_polygon(
                f.iter().map(|&i| scr(i)).collect(),
                fill,
                Stroke::NONE,
            ));
        }
    }
    if lit {
        for &[a, b] in &mesh.e {
            painter.line_segment(
                [scr(a), scr(b)],
                Stroke::new(3.5, edge.gamma_multiply(0.12)),
            );
        }
    }
    let w = if lit { 1.3 } else { 1.0 };
    for &[a, b] in &mesh.e {
        painter.line_segment(
            [scr(a), scr(b)],
            Stroke::new(w, edge.gamma_multiply(if lit { 1.0 } else { 0.6 })),
        );
    }
}

/// Crosshair corners and a `$` round a contract target.
fn bounty(painter: &Painter, c: Pos2, p: &Palette) {
    let r = Rect::from_center_size(c, vec2(HIT, HIT));
    let st = Stroke::new(1.5, p.bright);
    for (k, dx, dy) in [
        (r.left_top(), 1.0, 1.0),
        (r.right_top(), -1.0, 1.0),
        (r.left_bottom(), 1.0, -1.0),
        (r.right_bottom(), -1.0, -1.0),
    ] {
        painter.line_segment([k, k + vec2(8.0 * dx, 0.0)], st);
        painter.line_segment([k, k + vec2(0.0, 8.0 * dy)], st);
    }
    painter.text(
        r.right_top() + vec2(3.0, -2.0),
        Align2::LEFT_BOTTOM,
        "$",
        FontId::new(13.0, FontFamily::Monospace),
        p.bright,
    );
}
