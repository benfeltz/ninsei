//! The main editor pane: the network drawn as boxes and wires. Returns the node the player clicked.

use egui::{Align2, Color32, FontFamily, FontId, Rect, Sense, Stroke, Ui, pos2, vec2};
use ninsei_core::{Game, NodeKind, Phase};

use crate::theme::Palette;

const NODE_W: f32 = 74.0;
const NODE_H: f32 = 30.0;
const MARGIN: f32 = 48.0;
/// Nodes further than this many layers past the runner are fogged.
const SIGHT: usize = 1;

pub fn show(ui: &mut Ui, game: &Game, p: &Palette) -> Option<usize> {
    let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, p.bg);
    draw_grid(&painter, rect, p);

    let net = &game.net;
    let here = &net.nodes[game.at];
    let layers = net.layers.len();
    let inner = rect.shrink(MARGIN);
    let pos = |id: usize| {
        let n = &net.nodes[id];
        let width = net.layers[n.layer].len();
        let x = inner.left() + inner.width() * n.layer as f32 / (layers - 1).max(1) as f32;
        let y = inner.top() + inner.height() * (n.lane as f32 + 0.5) / width as f32;
        pos2(x, y)
    };
    // The BBS number is passed around; you always know where it is.
    let visible = |id: usize| {
        net.nodes[id].layer <= here.layer + SIGHT || net.nodes[id].kind == NodeKind::Bbs
    };
    let reachable = game.reachable();

    // Wires first, so boxes sit on top.
    for n in &net.nodes {
        for &to in &n.links {
            let live = n.id == game.at && reachable.contains(&to);
            let col = if live {
                p.fg
            } else if visible(to) {
                p.dim.gamma_multiply(0.6)
            } else {
                p.faint
            };
            let (a, b) = (pos(n.id), pos(to));
            let a = pos2(a.x + NODE_W / 2.0, a.y);
            let b = pos2(b.x - NODE_W / 2.0, b.y);
            // Right-angled trace like a circuit board.
            let mid = (a.x + b.x) / 2.0;
            let w = if live { 1.5 } else { 1.0 };
            painter.line(
                vec![a, pos2(mid, a.y), pos2(mid, b.y), b],
                Stroke::new(w, col),
            );
        }
    }

    let blink = (ui.input(|i| i.time) * 2.0) as i64 % 2 == 0;
    let mut clicked = None;
    let small = FontId::new(10.0, FontFamily::Monospace);
    let body = FontId::new(13.0, FontFamily::Monospace);
    for n in &net.nodes {
        let c = pos(n.id);
        let r = Rect::from_center_size(c, vec2(NODE_W, NODE_H));
        let id = ui.id().with(("node", n.id));
        let can_go = reachable.contains(&n.id);
        let resp = ui.interact(
            r,
            id,
            if can_go {
                Sense::click()
            } else {
                Sense::hover()
            },
        );
        let seen = visible(n.id);
        let tag = if seen { kind_tag(n.kind) } else { "???" };

        let (fill, stroke, text) = if n.id == game.at {
            // You are here: inverse video.
            (p.fg, p.bright, p.bg)
        } else if can_go && resp.hovered() {
            (p.faint, p.bright, p.bright)
        } else if can_go {
            (p.bg, p.fg, p.fg)
        } else if n.cleared {
            (p.bg, p.faint, p.dim)
        } else if seen {
            (p.bg, p.dim, p.dim)
        } else {
            (p.bg, p.faint, p.faint)
        };
        painter.rect_filled(r, 0.0, fill);
        let dashed = matches!(n.kind, NodeKind::BlackIce) && seen;
        painter.rect_stroke(
            r,
            0.0,
            Stroke::new(if dashed { 2.0 } else { 1.0 }, stroke),
            egui::StrokeKind::Inside,
        );
        let label = if n.cleared && n.kind != NodeKind::Entry && n.id != game.at {
            format!("{tag} ok")
        } else {
            tag.to_string()
        };
        painter.text(c, Align2::CENTER_CENTER, label, body.clone(), text);
        painter.text(
            r.left_top() + vec2(2.0, -12.0),
            Align2::LEFT_TOP,
            format!("{}", n.id),
            small.clone(),
            p.dim,
        );
        let host = if seen { n.name.as_str() } else { "????-????" };
        painter.text(
            pos2(c.x, r.bottom() + 3.0),
            Align2::CENTER_TOP,
            host,
            small.clone(),
            p.dim,
        );

        if n.id == game.at && game.phase == Phase::Exploring && blink {
            painter.text(
                pos2(r.right() + 4.0, c.y),
                Align2::LEFT_CENTER,
                "_",
                body.clone(),
                p.bright,
            );
        }
        if can_go {
            let resp = resp.on_hover_text(format!(
                "jack {}  →  {}",
                n.id,
                if seen { n.kind.label() } else { "unknown" }
            ));
            if resp.clicked() {
                clicked = Some(n.id);
            }
        }
    }

    let hint = match game.phase {
        Phase::Exploring => "click a lit node (or `jack <n>`) to route there",
        Phase::Combat => "ICE holds the line. break it in target.ice",
        Phase::Flatlined => "FLATLINED. `newrun` to jack in again",
        Phase::JackedOut => "JACKED OUT. `newrun` to go deeper",
    };
    painter.text(
        rect.left_bottom() + vec2(8.0, -6.0),
        Align2::LEFT_BOTTOM,
        hint,
        small,
        p.dim,
    );
    clicked
}

fn kind_tag(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Entry => "GW",
        NodeKind::Ice => "ICE",
        NodeKind::BlackIce => "B.ICE",
        NodeKind::Vault => "DATA",
        NodeKind::Bbs => "BBS",
        NodeKind::Exit => "EXIT",
    }
}

/// Faint dot grid behind the map.
fn draw_grid(painter: &egui::Painter, rect: Rect, p: &Palette) {
    const STEP: f32 = 24.0;
    let dot = Color32::from_rgba_unmultiplied(p.faint.r(), p.faint.g(), p.faint.b(), 160);
    let mut x = rect.left() + STEP / 2.0;
    while x < rect.right() {
        let mut y = rect.top() + STEP / 2.0;
        while y < rect.bottom() {
            painter.rect_filled(Rect::from_center_size(pos2(x, y), vec2(1.0, 1.0)), 0.0, dot);
            y += STEP;
        }
        x += STEP;
    }
}
