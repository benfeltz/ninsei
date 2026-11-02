//! The network as a flat technical drawing: grid bubbles over each layer, a drafting symbol per
//! node (from `data/netmap.ron`), hidden lines for the fog, a revision cloud round you, a
//! dimension line and a title block. Every colour comes from the theme.

use egui::{
    Align2, Color32, FontFamily, FontId, Painter, Pos2, Rect, Sense, Shape, Stroke, Ui, pos2, vec2,
};
use ninsei_core::netview::{MapNode, Symbol, style};
use ninsei_core::{Game, NodeKind, Phase};

use crate::netmap::{hint, interact, kind, tag, target, visible};
use crate::theme::Palette;

/// Half the size of a node symbol, in pixels.
const R: f32 = 15.0;
/// Room round the drawing: bubbles on top, dimension and title block below.
const TOP: f32 = 64.0;
const BOTTOM: f32 = 96.0;
const SIDE: f32 = 56.0;

pub fn show(ui: &mut Ui, game: &Game, p: &Palette) -> Option<usize> {
    let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, p.bg);
    paper(&painter, rect, p);

    let net = &game.net;
    let layers = net.layers.len();
    let inner = Rect::from_min_max(rect.min + vec2(SIDE, TOP), rect.max - vec2(SIDE, BOTTOM));
    let col = |l: usize| inner.left() + inner.width() * l as f32 / (layers - 1).max(1) as f32;
    let pos = |id: usize| {
        let n = &net.nodes[id];
        let width = net.layers[n.layer].len();
        pos2(
            col(n.layer),
            inner.top() + inner.height() * (n.lane as f32 + 0.5) / width as f32,
        )
    };
    let small = FontId::new(10.0, FontFamily::Monospace);
    let body = FontId::new(11.0, FontFamily::Monospace);

    // Grid lines: a chain line down each layer, with its bubble.
    for l in 0..layers {
        let x = col(l);
        painter.extend(Shape::dashed_line(
            &[pos2(x, rect.top() + 40.0), pos2(x, inner.bottom() + 18.0)],
            Stroke::new(1.0, p.faint),
            14.0,
            5.0,
        ));
        let c = pos2(x, rect.top() + 26.0);
        painter.circle_stroke(c, 11.0, Stroke::new(1.2, p.dim));
        let letter = char::from(b'A' + (l % 26) as u8).to_string();
        painter.text(c, Align2::CENTER_CENTER, letter, body.clone(), p.fg);
    }

    // Wires: right-angled runs. Your routes are heavy, known ones light, the rest hidden lines.
    let reach = game.reachable();
    for n in &net.nodes {
        for &to in &n.links {
            let (a, b) = (pos(n.id) + vec2(R, 0.0), pos(to) - vec2(R, 0.0));
            let mid = (a.x + b.x) / 2.0;
            let run = [a, pos2(mid, a.y), pos2(mid, b.y), b];
            if n.id == game.at && reach.contains(&to) {
                painter.add(Shape::line(run.to_vec(), Stroke::new(2.0, p.fg)));
                arrow(&painter, b, p.fg);
            } else if visible(game, n.id) && visible(game, to) {
                painter.add(Shape::line(run.to_vec(), Stroke::new(1.0, p.dim)));
            } else {
                painter.extend(Shape::dashed_line(
                    &run,
                    Stroke::new(1.0, p.faint),
                    5.0,
                    4.0,
                ));
            }
        }
    }

    let mut clicked = None;
    let t = ui.input(|i| i.time);
    for n in &net.nodes {
        let c = pos(n.id);
        let (hit, hovered) = interact(
            ui,
            game,
            n.id,
            Rect::from_center_size(c, vec2(R * 2.6, R * 2.6)),
        );
        clicked = clicked.or(hit);
        let here = n.id == game.at;
        let seen = visible(game, n.id);
        let can_go = reach.contains(&n.id);
        let done = n.cleared && !here;
        let ink = if here || (can_go && hovered) {
            p.bright
        } else if can_go {
            p.fg
        } else if done || !seen {
            p.dim
        } else {
            p.fg.gamma_multiply(0.8)
        };
        if seen {
            let k = kind(game, n.id);
            symbol(
                &painter,
                c,
                style(&game.data, k).symbol,
                ink,
                p,
                k == MapNode::BlackIce,
            );
        } else {
            // Hidden line: what's there isn't known.
            let r = Rect::from_center_size(c, vec2(R * 2.0, R * 2.0));
            let pts = [
                r.left_top(),
                r.right_top(),
                r.right_bottom(),
                r.left_bottom(),
                r.left_top(),
            ];
            painter.extend(Shape::dashed_line(&pts, Stroke::new(1.0, p.dim), 4.0, 3.0));
            painter.text(c, Align2::CENTER_CENTER, "?", body.clone(), p.dim);
        }
        // Callout: the node number in a little circle.
        let tagc = c + vec2(-R - 7.0, -R - 7.0);
        painter.circle_filled(tagc, 7.5, p.bg);
        painter.circle_stroke(tagc, 7.5, Stroke::new(1.0, p.dim));
        painter.text(
            tagc,
            Align2::CENTER_CENTER,
            n.id.to_string(),
            FontId::new(9.0, FontFamily::Monospace),
            p.dim,
        );

        let host = if seen { n.name.as_str() } else { "????-????" };
        painter.text(
            c + vec2(0.0, R + 4.0),
            Align2::CENTER_TOP,
            host,
            small.clone(),
            if done { p.dim } else { p.fg },
        );
        let mut label = tag(game, n.id);
        if done && n.kind != NodeKind::Entry {
            label.push_str(" ok");
        }
        painter.text(
            c + vec2(0.0, R + 15.0),
            Align2::CENTER_TOP,
            label,
            small.clone(),
            p.dim,
        );

        if here {
            revision_cloud(&painter, c + vec2(0.0, 9.0), p);
            if game.phase == Phase::Exploring && (t * 2.0) as i64 % 2 == 0 {
                painter.text(
                    c + vec2(R + 16.0, 0.0),
                    Align2::LEFT_CENTER,
                    "_",
                    body.clone(),
                    p.bright,
                );
            }
        }
        if target(game, n.id) {
            painter.text(
                c + vec2(R + 6.0, -R - 2.0),
                Align2::LEFT_BOTTOM,
                "$",
                body.clone(),
                p.bright,
            );
            painter.circle_stroke(c, R * 1.7, Stroke::new(1.0, p.bright));
        }
    }

    dimension(
        &painter,
        col(0),
        col(layers - 1),
        inner.bottom() + 20.0,
        &format!("{layers} LAYERS"),
        p,
    );
    title_block(&painter, rect, game, p);
    painter.text(
        rect.left_bottom() + vec2(8.0, -6.0),
        Align2::LEFT_BOTTOM,
        hint(game),
        small,
        p.dim,
    );
    clicked
}

/// Drafting paper: a fine grid with a heavier line every fifth square.
fn paper(painter: &Painter, rect: Rect, p: &Palette) {
    const STEP: f32 = 12.0;
    let (minor, major) = (p.faint.gamma_multiply(0.35), p.faint.gamma_multiply(0.8));
    let mut i = 0;
    let mut x = rect.left();
    while x < rect.right() {
        let c = if i % 5 == 0 { major } else { minor };
        painter.line_segment(
            [pos2(x, rect.top()), pos2(x, rect.bottom())],
            Stroke::new(1.0, c),
        );
        x += STEP;
        i += 1;
    }
    let (mut i, mut y) = (0, rect.top());
    while y < rect.bottom() {
        let c = if i % 5 == 0 { major } else { minor };
        painter.line_segment(
            [pos2(rect.left(), y), pos2(rect.right(), y)],
            Stroke::new(1.0, c),
        );
        y += STEP;
        i += 1;
    }
}

/// A drafting symbol for a node, centred on `c`.
fn symbol(painter: &Painter, c: Pos2, s: Symbol, ink: Color32, p: &Palette, heavy: bool) {
    let st = Stroke::new(1.4, ink);
    let thin = Stroke::new(1.0, ink.gamma_multiply(0.7));
    let sq = Rect::from_center_size(c, vec2(R * 2.0, R * 2.0));
    let boxed = |painter: &Painter| {
        painter.rect_filled(sq, 0.0, p.bg);
        painter.rect_stroke(sq, 0.0, st, egui::StrokeKind::Middle);
    };
    // 45° section hatching inside the square, `gap` pixels apart.
    let hatch = |painter: &Painter, gap: f32, flip: bool| {
        let mut k = -R * 2.0 + gap;
        while k < R * 2.0 {
            let (a, b) = clip_diagonal(k, flip);
            painter.line_segment([c + a, c + b], thin);
            k += gap;
        }
    };
    match s {
        Symbol::Hatch => {
            boxed(painter);
            hatch(painter, 6.0, false);
        }
        Symbol::Crosshatch => {
            boxed(painter);
            hatch(painter, 4.0, false);
            hatch(painter, 4.0, true);
            painter.rect_stroke(sq.expand(3.0), 0.0, st, egui::StrokeKind::Middle);
            if heavy {
                painter.rect_stroke(sq.expand(5.0), 0.0, thin, egui::StrokeKind::Middle);
            }
        }
        Symbol::Valve => {
            painter.circle_filled(c, R, p.bg);
            painter.circle_stroke(c, R, st);
            let d = R * 0.7;
            painter.line_segment([c + vec2(-d, -d), c + vec2(d, d)], st);
            painter.line_segment([c + vec2(-d, d), c + vec2(d, -d)], st);
        }
        Symbol::Eye => {
            painter.circle_filled(c, R, p.bg);
            painter.circle_stroke(c, R, st);
            painter.circle_filled(c, R * 0.3, ink);
            painter.line_segment([c + vec2(-R * 1.4, 0.0), c + vec2(-R, 0.0)], thin);
            painter.line_segment([c + vec2(R, 0.0), c + vec2(R * 1.4, 0.0)], thin);
        }
        Symbol::Column => {
            boxed(painter);
            painter.line_segment([sq.left_top(), sq.right_bottom()], st);
            painter.line_segment([sq.right_top(), sq.left_bottom()], st);
        }
        Symbol::Hex => {
            let pts: Vec<Pos2> = (0..6)
                .map(|i| {
                    let a = std::f32::consts::FRAC_PI_3 * i as f32;
                    c + vec2(a.cos(), a.sin()) * R * 1.1
                })
                .collect();
            painter.add(Shape::convex_polygon(pts, p.bg, st));
            painter.circle_stroke(c, R * 0.45, thin);
        }
        Symbol::Flag => {
            let pts = vec![
                c + vec2(0.0, -R),
                c + vec2(R, R * 0.8),
                c + vec2(-R, R * 0.8),
            ];
            painter.add(Shape::convex_polygon(pts, p.bg, st));
            painter.text(
                c + vec2(0.0, R * 0.25),
                Align2::CENTER_CENTER,
                "!",
                FontId::new(10.0, FontFamily::Monospace),
                ink,
            );
        }
        Symbol::Nested => {
            boxed(painter);
            let inner = sq.shrink(R * 0.55);
            let pts = [
                inner.left_top(),
                inner.right_top(),
                inner.right_bottom(),
                inner.left_bottom(),
                inner.left_top(),
            ];
            painter.extend(Shape::dashed_line(&pts, thin, 3.0, 2.0));
        }
        Symbol::Inlet => {
            boxed(painter);
            let y = c.y;
            painter.line_segment([pos2(sq.left() - 6.0, y), pos2(c.x + 4.0, y)], st);
            arrow(painter, pos2(c.x + 6.0, y), ink);
        }
        Symbol::Door => {
            // A wall with a gap, the door leaf and its swing.
            painter.line_segment([sq.left_top(), sq.left_bottom()], Stroke::new(3.0, ink));
            let hinge = sq.left_bottom();
            painter.line_segment([hinge, hinge + vec2(0.0, -R * 2.0)], st);
            painter.line_segment([hinge, hinge + vec2(R * 2.0, 0.0)], st);
            let arc: Vec<Pos2> = (0..=16)
                .map(|k| {
                    let a = -std::f32::consts::FRAC_PI_2 * k as f32 / 16.0;
                    hinge + vec2(a.cos(), a.sin()) * R * 2.0
                })
                .collect();
            painter.extend(Shape::dashed_line(&arc, thin, 3.0, 2.0));
        }
    }
}

/// The ends of a 45° line `x - y = k` (or `x + y = k`) clipped to the symbol's square.
fn clip_diagonal(k: f32, flip: bool) -> (egui::Vec2, egui::Vec2) {
    let lo = (-R).max(k - R);
    let hi = R.min(k + R);
    let y = |x: f32| x - k;
    if flip {
        (vec2(lo, -y(lo)), vec2(hi, -y(hi)))
    } else {
        (vec2(lo, y(lo)), vec2(hi, y(hi)))
    }
}

fn arrow(painter: &Painter, tip: Pos2, c: Color32) {
    painter.add(Shape::convex_polygon(
        vec![tip, tip + vec2(-8.0, -4.0), tip + vec2(-8.0, 4.0)],
        c,
        Stroke::NONE,
    ));
}

/// A scalloped revision cloud round the node you're on, with its delta.
fn revision_cloud(painter: &Painter, c: Pos2, p: &Palette) {
    let (r, bumps) = (R * 2.5, 16);
    let tau = std::f32::consts::TAU;
    for i in 0..bumps {
        let a0 = tau * i as f32 / bumps as f32;
        let a1 = tau * (i + 1) as f32 / bumps as f32;
        let (p0, p1) = (
            c + vec2(a0.cos(), a0.sin()) * r,
            c + vec2(a1.cos(), a1.sin()) * r,
        );
        let mid = p0 + (p1 - p0) / 2.0;
        let rad = (p1 - p0).length() / 2.0;
        let out = (mid - c).normalized();
        let start = (p0 - mid).angle();
        let pts: Vec<Pos2> = (0..=8)
            .map(|k| {
                let a = start
                    + std::f32::consts::PI * k as f32 / 8.0
                        * if out.x * (p0 - mid).y - out.y * (p0 - mid).x > 0.0 {
                            -1.0
                        } else {
                            1.0
                        };
                mid + vec2(a.cos(), a.sin()) * rad
            })
            .collect();
        painter.add(Shape::line(pts, Stroke::new(1.3, p.bright)));
    }
    let d = c + vec2(r * 0.75, -r * 0.95);
    painter.add(Shape::convex_polygon(
        vec![d + vec2(0.0, -7.0), d + vec2(7.0, 5.0), d + vec2(-7.0, 5.0)],
        p.bg,
        Stroke::new(1.2, p.bright),
    ));
    painter.text(
        d + vec2(0.0, 1.0),
        Align2::CENTER_CENTER,
        "1",
        FontId::new(8.0, FontFamily::Monospace),
        p.bright,
    );
}

/// A dimension line with architectural ticks and its figure.
fn dimension(painter: &Painter, x0: f32, x1: f32, y: f32, label: &str, p: &Palette) {
    let st = Stroke::new(1.0, p.dim);
    painter.line_segment([pos2(x0 - 8.0, y), pos2(x1 + 8.0, y)], st);
    for x in [x0, x1] {
        painter.line_segment([pos2(x, y - 10.0), pos2(x, y + 4.0)], st);
        painter.line_segment(
            [pos2(x - 4.0, y + 4.0), pos2(x + 4.0, y - 4.0)],
            Stroke::new(2.0, p.fg),
        );
    }
    let mid = pos2((x0 + x1) / 2.0, y);
    let font = FontId::new(10.0, FontFamily::Monospace);
    let w = label.len() as f32 * 6.5 + 10.0;
    painter.rect_filled(Rect::from_center_size(mid, vec2(w, 12.0)), 0.0, p.bg);
    painter.text(mid, Align2::CENTER_CENTER, label, font, p.fg);
}

/// The title block in the corner: what this sheet is.
fn title_block(painter: &Painter, rect: Rect, game: &Game, p: &Palette) {
    let (w, h) = (250.0, 58.0);
    let r = Rect::from_min_size(rect.right_bottom() - vec2(w + 8.0, h + 8.0), vec2(w, h));
    painter.rect_filled(r, 0.0, p.bg);
    painter.rect_stroke(r, 0.0, Stroke::new(1.5, p.dim), egui::StrokeKind::Middle);
    let row = h / 3.0;
    for i in 1..3 {
        let y = r.top() + row * i as f32;
        painter.line_segment(
            [pos2(r.left(), y), pos2(r.right(), y)],
            Stroke::new(1.0, p.faint),
        );
    }
    painter.line_segment(
        [
            pos2(r.left() + 62.0, r.top()),
            pos2(r.left() + 62.0, r.bottom()),
        ],
        Stroke::new(1.0, p.faint),
    );
    let site = game
        .site
        .as_ref()
        .map_or("UNLISTED NETWORK".to_string(), |s| s.name.to_uppercase());
    let sheet = if game.in_subnet() {
        "SUBNET DETAIL"
    } else {
        "SHEET 1 OF 1"
    };
    let font = FontId::new(10.0, FontFamily::Monospace);
    for (i, (k, v)) in [
        ("PROJECT", site),
        ("SEED", format!("{:016x}", game.seed)),
        ("DRAWN", format!("runner   {sheet}   NTS")),
    ]
    .into_iter()
    .enumerate()
    {
        let y = r.top() + row * (i as f32 + 0.5);
        painter.text(
            pos2(r.left() + 6.0, y),
            Align2::LEFT_CENTER,
            k,
            font.clone(),
            p.dim,
        );
        let v: String = v.chars().take(28).collect();
        painter.text(
            pos2(r.left() + 68.0, y),
            Align2::LEFT_CENTER,
            v,
            font.clone(),
            p.fg,
        );
    }
}
