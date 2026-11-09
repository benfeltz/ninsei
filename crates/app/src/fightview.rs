//! The Ono-Sendai's fight view: on ICE contact the network map dives into the node and the
//! node's wireframe becomes the ICE, turning over a grid that runs to the horizon. It comes
//! apart as its strength drops (`fight::broken`), jolts when hit, charges before it attacks and
//! freezes when lagged. Along the bottom, the HDD's programs are a hotbar: a digit or a click
//! runs one, and hovering one previews what it would do (`Game::preview`) on the strength meter
//! and on the model. Every colour is a theme role; every timing is in [`anim`]. The rules are
//! `ninsei_core::fight`; animation state lives in egui's memory, never in the game.

use egui::{
    Align2, Color32, FontFamily, FontId, Painter, Pos2, Rect, Sense, Stroke, Ui, pos2, vec2,
};
use ninsei_core::btop_fight;
use ninsei_core::fight::{self, Match, NextMove, Outcome};
use ninsei_core::netview::{self, MapNode, Mesh};
use ninsei_core::{Action, Game, Phase};

use crate::netmap::mix;
use crate::panes::Panes;
use crate::theme::Palette;

/// Timings and sizes for the fight view, in one place. Display numbers, not balance.
pub mod anim {
    /// How long the dive into the node takes.
    pub const DIVE_SECS: f64 = 0.9;
    /// How fast the ICE turns, radians a second.
    pub const SPIN: f32 = 0.35;
    /// How far it tips toward you.
    pub const TILT: f32 = 0.35;
    /// How long a hit jolts it, and how far.
    pub const HIT_SECS: f64 = 0.45;
    pub const HIT_JOLT: f32 = 9.0;
    /// How long a damage number floats up, and how high.
    pub const FLOAT_SECS: f64 = 1.3;
    pub const FLOAT_RISE: f32 = 46.0;
    /// How long a program's packets take to reach the ICE.
    pub const PACKET_SECS: f64 = 0.45;
    /// How long the ICE's own attack line stays.
    pub const STRIKE_SECS: f64 = 0.35;
    /// The charge ring's slow breathing, radians a second.
    pub const CHARGE_PULSE: f32 = 2.2;
    /// A lagged ICE flickers at this rate (kept well under 3 a second).
    pub const STUN_FLICKER_HZ: f32 = 1.5;
    /// Digital rain: columns lit (share), fall speed (px/s), opacity.
    pub const RAIN_SHARE: f32 = 0.35;
    pub const RAIN_SPEED: f32 = 26.0;
    pub const RAIN_ALPHA: f32 = 0.34;
    /// The horizon, as a share of the pane's height.
    pub const HORIZON: f32 = 0.40;
    /// The ICE's size: the projection's focal length, as a share of the pane's smaller side
    /// (a unit mesh at the camera's distance ends up about a third of the pane across).
    pub const SCALE: f32 = 2.1;
    /// Hotbar tiles.
    pub const TILE_W: f32 = 150.0;
    pub const TILE_MIN_W: f32 = 104.0;
    pub const TILE_H: f32 = 70.0;
    pub const END_W: f32 = 176.0;
}

fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

fn alpha(c: Color32, a: f32) -> Color32 {
    c.gamma_multiply(a.clamp(0.0, 1.0))
}

/// What the view remembers between frames (egui memory, not the game).
#[derive(Clone, Default)]
struct Memory {
    /// The fight this is: seed, node and hops (a new fight dives again).
    key: Option<(u64, usize, u32)>,
    start: f64,
    /// ICE strength and deck integrity last frame, to notice hits.
    strength: u32,
    integrity: u32,
    /// The history tick last read, for programs run.
    tick: u32,
    /// (time, amount, strong): hits on the ICE, for the jolt and the floating number.
    hits: Vec<(f64, u32, bool)>,
    /// (time, slot uid): programs run, for their packets.
    packets: Vec<(f64, u32)>,
    /// When the ICE last hit the deck.
    struck: Option<f64>,
}

fn remember(ui: &Ui, g: &Game, now: f64) -> Memory {
    let Some(c) = &g.combat else {
        return Memory::default();
    };
    let id = ui.id().with("fightview");
    let key = (g.seed, c.node, g.hops);
    ui.ctx().data_mut(|d| {
        let m: &mut Memory = d.get_temp_mut_or_default(id);
        if m.key != Some(key) {
            *m = Memory {
                key: Some(key),
                start: now,
                strength: c.strength,
                integrity: g.deck.integrity,
                tick: g.history.tick,
                ..Memory::default()
            };
        }
        if c.strength < m.strength {
            let strong = g
                .history
                .runs
                .last()
                .and_then(|(uid, _)| g.hdd.iter().find(|s| s.uid == *uid))
                .is_some_and(|s| {
                    matches!(s.spec.effect, ninsei_core::Effect::Break { vs: Some(v), .. }
                        if v == g.data.ice[c.ice].class)
                });
            m.hits.push((now, m.strength - c.strength, strong));
        }
        if g.deck.integrity < m.integrity {
            m.struck = Some(now);
        }
        for &(uid, at) in &g.history.runs {
            if at > m.tick {
                m.packets.push((now, uid));
            }
        }
        m.strength = c.strength;
        m.integrity = g.deck.integrity;
        m.tick = g.history.tick;
        m.hits.retain(|h| now - h.0 < anim::FLOAT_SECS);
        m.packets.retain(|p| now - p.0 < anim::PACKET_SECS);
        m.clone()
    })
}

/// The floor: lines running to the horizon and rungs closing in on it.
fn floor(painter: &Painter, r: Rect, p: &Palette, horizon: f32, van: f32, a: f32) {
    for i in -14..=14 {
        let xb = van + i as f32 * r.width() / 10.0;
        let far = pos2(xb + (xb - van) * 3.0, r.bottom() + r.height());
        painter.line_segment(
            [pos2(van, horizon), far],
            Stroke::new(1.0, alpha(p.faint, a)),
        );
    }
    for k in 1..16 {
        let t = (k as f32 / 15.0).powf(2.2);
        let y = horizon + t * (r.bottom() - horizon + 40.0);
        let col = if k > 11 { p.dim } else { p.faint };
        painter.line_segment(
            [pos2(r.left(), y), pos2(r.right(), y)],
            Stroke::new(1.0, alpha(col, a)),
        );
    }
    painter.line_segment(
        [pos2(r.left(), horizon), pos2(r.right(), horizon)],
        Stroke::new(1.0, alpha(p.dim, a)),
    );
}

/// Slow digital rain in a band behind the ICE.
fn rain(painter: &Painter, r: Rect, p: &Palette, now: f64, seed: u64) {
    const GLYPHS: &[char] = &[
        '0', '1', 'ア', 'イ', 'ウ', 'カ', 'キ', 'ク', 'サ', 'シ', 'x', '7', 'F', '#',
    ];
    let cols = (r.width() / 14.0) as u64;
    for col in 0..cols {
        let h = splat(seed ^ col.wrapping_mul(0x9E37));
        if (h % 1000) as f32 / 1000.0 > anim::RAIN_SHARE {
            continue;
        }
        let speed = anim::RAIN_SPEED * (0.6 + (h >> 10) as f32 % 100.0 / 100.0);
        let len = 4 + (h >> 20) % 8;
        let head = (now as f32 * speed + (h >> 30) as f32 % r.height()) % (r.height() + 120.0);
        for k in 0..len {
            let y = r.top() + head - k as f32 * 13.0;
            if y < r.top() || y > r.bottom() {
                continue;
            }
            let g = GLYPHS[((h >> 8) as usize + k as usize + (now * 2.0) as usize) % GLYPHS.len()];
            let col_c = if k == 0 { p.fg } else { p.faint };
            let fade = anim::RAIN_ALPHA * (1.0 - k as f32 / len as f32);
            painter.text(
                pos2(r.left() + col as f32 * 14.0 + 7.0, y),
                Align2::CENTER_CENTER,
                g,
                mono(11.0),
                alpha(col_c, fade),
            );
        }
    }
}

fn splat(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// Projects a spun mesh point onto the screen, with a little perspective.
fn project(v: [f32; 3], centre: Pos2, scale: f32) -> (Pos2, f32) {
    const DIST: f32 = 3.2;
    let z = v[2] + DIST;
    (
        pos2(centre.x + v[0] * scale / z, centre.y - v[1] * scale / z),
        z,
    )
}

/// A strength meter of `cells`, with `ghost` cells (a preview) outlined at its top end.
fn meter(painter: &Painter, at: Pos2, p: &Palette, cells: usize, frac: f32, ghost: f32) {
    let on = frac * cells as f32;
    let off = (frac - ghost).max(0.0) * cells as f32;
    for i in 0..cells {
        let k = (i as f32 + 0.5) / cells as f32;
        let r = Rect::from_min_size(pos2(at.x + i as f32 * 8.0, at.y - 5.0), vec2(6.5, 10.0));
        if on - i as f32 >= 0.5 {
            painter.rect_filled(r, 0.0, crate::btop::ramp(p, k));
            if ghost > 0.0 && i as f32 >= off {
                painter.rect_filled(r.shrink(1.5), 0.0, p.bg);
                painter.rect_stroke(r, 0.0, Stroke::new(1.0, p.bright), egui::StrokeKind::Inside);
            }
        } else {
            painter.circle_filled(r.center(), 1.0, p.faint);
        }
    }
}

/// The fight, filling the map pane.
pub fn show(ui: &mut Ui, panes: &mut Panes) {
    let g = panes.game;
    let p = panes.pal;
    let Some(c) = g.combat.as_ref().filter(|_| g.phase == Phase::Combat) else {
        return;
    };
    ui.ctx().request_repaint();
    let now = ui.input(|i| i.time);
    let m = remember(ui, g, now);
    let rect = ui.available_rect_before_wrap();
    ui.allocate_rect(rect, Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, p.bg);
    let ice = &g.data.ice[c.ice];
    let dive = ((now - m.start) / anim::DIVE_SECS).clamp(0.0, 1.0) as f32;
    let ease = 1.0 - (1.0 - dive).powi(3);

    // The scene: floor, rain, the ICE.
    let horizon = rect.top() + rect.height() * anim::HORIZON * (0.4 + 0.6 * ease);
    let centre = pos2(rect.center().x, horizon);
    floor(&painter, rect, &p, horizon, centre.x, 0.3 + 0.7 * ease);
    let seed = g.seed ^ c.node as u64;
    if dive >= 1.0 {
        let band = Rect::from_min_max(
            pos2(rect.left() + rect.width() * 0.22, rect.top() + 6.0),
            pos2(rect.right() - rect.width() * 0.22, horizon),
        );
        rain(&painter, band, &p, now, seed);
    }
    let kind = MapNode::of(&g.net, c.node, &g.data);
    let mesh = Mesh::of(netview::style(&g.data, kind).mesh);
    let size = rect.width().min(rect.height()) * anim::SCALE * (0.15 + 0.85 * ease);
    // A hit jolts it; a lagged ICE stops turning and flickers.
    let jolt = m.hits.last().map_or(1.0, |h| {
        ((now - h.0) / anim::HIT_SECS).clamp(0.0, 1.0) as f32
    });
    let shake = if jolt < 1.0 {
        anim::HIT_JOLT * (1.0 - jolt) * ((now * 40.0).sin() as f32)
    } else {
        0.0
    };
    let yaw = if c.stunned {
        0.6
    } else {
        now as f32 * anim::SPIN
    };
    let flicker = if c.stunned {
        0.55 + 0.45
            * ((now as f32 * anim::STUN_FLICKER_HZ * std::f32::consts::TAU).sin() * 0.5 + 0.5)
    } else {
        1.0
    };
    let at = centre + vec2(shake, 0.0);
    let pts: Vec<(Pos2, f32)> = mesh
        .v
        .iter()
        .map(|v| project(netview::spin(*v, yaw, anim::TILT), at, size))
        .collect();

    // How big it ends up on screen, for the ring and its shadow.
    let radius = pts
        .iter()
        .map(|(q, _)| (*q - at).length())
        .fold(1.0_f32, f32::max);

    // The hovered program's preview: which edges it would break.
    let hovered = hotbar_hover(ui, g);
    let preview = hovered.and_then(|s| g.preview(s).ok());
    let total = mesh.e.len();
    let broken = fight::broken(total, seed, c.strength, c.strength_max);
    let would = preview.map_or(Vec::new(), |pv| {
        fight::broken(
            total,
            seed,
            c.strength.saturating_sub(pv.hit()),
            c.strength_max,
        )
    });
    let wire = if jolt < 0.35 { p.bright } else { p.fg };
    for (i, [a, b]) in mesh.e.iter().enumerate() {
        let (pa, za) = pts[*a];
        let (pb, zb) = pts[*b];
        if broken.contains(&i) {
            // A broken edge: a stub hanging off one end.
            let stub = pa + (pb - pa) * 0.3;
            painter.extend(egui::Shape::dashed_line(
                &[pa, stub],
                Stroke::new(1.0, alpha(p.faint, flicker)),
                3.0,
                4.0,
            ));
            continue;
        }
        let back = (za + zb) / 2.0 > 3.2;
        let lit = would.contains(&i);
        let (w, col) = if lit {
            (3.0, p.bright)
        } else if back {
            (1.0, alpha(mix(p.dim, wire, 0.4), 0.7 * flicker))
        } else {
            (2.0, alpha(wire, flicker))
        };
        painter.line_segment([pa, pb], Stroke::new(w, col));
    }
    for (q, _) in &pts {
        painter.rect_filled(
            Rect::from_center_size(*q, vec2(3.0, 3.0)),
            0.0,
            alpha(wire, flicker),
        );
    }
    painter.add(egui::Shape::ellipse_stroke(
        pos2(at.x, at.y + radius * 1.25),
        vec2(radius * 0.9, radius * 0.14),
        Stroke::new(1.0, alpha(p.dim, 0.7 * ease)),
    ));

    if dive < 1.0 {
        // The dive: brackets closing in, and what you're jacking into.
        let s = radius * 1.3 + (1.0 - ease) * rect.width() * 0.3;
        for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            let corner = at + vec2(sx * s, sy * s * 0.8);
            painter.line_segment(
                [corner, corner - vec2(0.0, sy * 20.0)],
                Stroke::new(2.0, p.bright),
            );
            painter.line_segment(
                [corner, corner - vec2(sx * 20.0, 0.0)],
                Stroke::new(2.0, p.bright),
            );
        }
        painter.text(
            pos2(rect.center().x, rect.bottom() - 60.0),
            Align2::CENTER_CENTER,
            format!("JACKING INTO {}", g.net.nodes[c.node].name),
            mono(14.0),
            p.bright,
        );
        return;
    }

    // Its next move, telegraphed: a charge ring before an attack.
    let intent = g.ice_next();
    if let Some(i) = intent
        && fight::threatens(i)
        && !c.stunned
    {
        let breathe = 0.35 + 0.25 * (now as f32 * anim::CHARGE_PULSE).sin();
        let r = radius * 1.35;
        let arc: Vec<Pos2> = (0..=40)
            .map(|k| {
                let a = -1.2 + 3.1 * k as f32 / 40.0;
                at + vec2(a.cos() * r, a.sin() * r)
            })
            .collect();
        painter.extend(egui::Shape::dashed_line(
            &arc,
            Stroke::new(1.5, alpha(p.bright, breathe)),
            6.0,
            6.0,
        ));
    }
    // The ICE's attack landing: a line from it to your deck, bottom left.
    if let Some(t) = m.struck
        && now - t < anim::STRIKE_SECS
    {
        let k = 1.0 - ((now - t) / anim::STRIKE_SECS) as f32;
        painter.line_segment(
            [at, pos2(rect.left() + 30.0, rect.bottom() - 30.0)],
            Stroke::new(2.0, alpha(p.bright, k)),
        );
    }

    hud(&painter, rect, &p, g, preview);
    let tiles = hotbar(
        ui,
        panes,
        rect,
        preview.is_some().then_some(hovered).flatten(),
    );

    // Packets from the program's tile to the ICE, then the damage floating up.
    for (t, uid) in &m.packets {
        let k = ((now - t) / anim::PACKET_SECS) as f32;
        let from = tiles
            .iter()
            .find(|(u, _)| u == uid)
            .map_or(pos2(rect.left() + 40.0, rect.bottom() - 40.0), |(_, r)| {
                r.center_top()
            });
        let ctrl = pos2(from.x, at.y + radius * 0.6);
        for n in 0..5 {
            let s = (k - n as f32 * 0.08).clamp(0.0, 1.0);
            let q = from.lerp(ctrl, s).lerp(ctrl.lerp(at, s), s);
            painter.rect_filled(Rect::from_center_size(q, vec2(5.0, 5.0)), 0.0, p.bright);
        }
    }
    for (t, amount, strong) in &m.hits {
        let k = ((now - t) / anim::FLOAT_SECS) as f32;
        let pos = at + vec2(radius * 1.1, -radius * 0.5 - k * anim::FLOAT_RISE);
        painter.text(
            pos,
            Align2::LEFT_CENTER,
            format!("-{amount}"),
            mono(24.0),
            alpha(p.bright, 1.0 - k),
        );
        if *strong {
            painter.text(
                pos + vec2(2.0, 22.0),
                Align2::LEFT_CENTER,
                format!("x2 vs {}", ice.class.label()),
                mono(11.0),
                alpha(p.fg, 1.0 - k),
            );
        }
    }
}

/// Name, class and strength (with the preview) on the left; its next moves on the right.
fn hud(painter: &Painter, rect: Rect, p: &Palette, g: &Game, preview: Option<fight::Preview>) {
    let Some(c) = &g.combat else { return };
    let ice = &g.data.ice[c.ice];
    let x = rect.left() + 18.0;
    let y = rect.top() + 22.0;
    let tier = match ice.tier {
        0 => "RUNNER",
        1 => "ICE",
        _ => "BLACK ICE",
    };
    painter.text(
        pos2(x, y),
        Align2::LEFT_CENTER,
        ice.name.to_uppercase(),
        mono(16.0),
        p.bright,
    );
    painter.text(
        pos2(x, y + 20.0),
        Align2::LEFT_CENTER,
        format!("{tier} // {}", ice.class.label()),
        mono(12.0),
        if ice.tier >= 2 { p.bright } else { p.dim },
    );
    painter.text(
        pos2(x, y + 42.0),
        Align2::LEFT_CENTER,
        "STR",
        mono(12.0),
        p.dim,
    );
    let cells = (c.strength_max as usize).clamp(8, 24);
    let max = c.strength_max.max(c.strength).max(1) as f32;
    let ghost = preview.map_or(0.0, |pv| pv.hit().min(c.strength) as f32 / max);
    meter(
        painter,
        pos2(x + 34.0, y + 42.0),
        p,
        cells,
        c.strength as f32 / max,
        ghost,
    );
    painter.text(
        pos2(x + 40.0 + cells as f32 * 8.0, y + 42.0),
        Align2::LEFT_CENTER,
        format!("{}/{}", c.strength, c.strength_max),
        mono(12.0),
        p.bright,
    );
    if let Some(pv) = preview {
        let line = match pv.outcome {
            Outcome::Break { hit, vs } => {
                let note = match vs {
                    Match::Strong => "  x2 vs class",
                    Match::Weak => "  /2 wrong class",
                    Match::Plain => "",
                };
                if hit >= c.strength {
                    format!("-{hit}: derezzes it{note}")
                } else {
                    format!("-{hit} strength{note}")
                }
            }
            Outcome::Shield(n) => format!("+{n} shield"),
            Outcome::Repair(n) => format!("+{n} integrity"),
            Outcome::Scrub(n) => format!("-{n} trace"),
            Outcome::Stun => "lags it: it skips its next move".to_string(),
        };
        painter.text(
            pos2(x, y + 62.0),
            Align2::LEFT_CENTER,
            line,
            mono(11.0),
            p.bright,
        );
    }
    // Next moves, top right.
    let rx = rect.right() - 250.0;
    painter.text(pos2(rx, y), Align2::LEFT_CENTER, "NEXT", mono(12.0), p.dim);
    let now_line = match g.next_move() {
        Some(NextMove::Act(i)) if fight::threatens(i) => format!("▲ {}", i.describe()),
        Some(m) => m.text("(lagged)"),
        None => String::new(),
    };
    painter.text(
        pos2(rx + 42.0, y),
        Align2::LEFT_CENTER,
        now_line,
        mono(14.0),
        p.bright,
    );
    // Under it, the ICE's program from where it carries on, as the deck has seen it, cut
    // with `…` to the panel's edge rather than clipped there.
    let cw = painter
        .layout_no_wrap("0".into(), mono(11.0), p.fg)
        .size()
        .x
        .max(1.0);
    let room = ((rect.right() - 8.0 - (rx + 42.0)) / cw).floor().max(0.0) as usize;
    for (k, r) in btop_fight::ice_window(g, 3).iter().enumerate() {
        let col = match (r.next, r.text.is_some()) {
            (true, _) => p.bright,
            (false, true) => p.fg,
            (false, false) => alpha(p.dim, 0.8),
        };
        painter.text(
            pos2(rx + 42.0, y + 20.0 + k as f32 * 16.0),
            Align2::LEFT_CENTER,
            crate::kit::cells::cut(&btop_fight::ice_line(r), room),
            mono(11.0),
            col,
        );
    }
}

/// The tiles' rects: the HDD's programs (slot, rect), then END TURN.
fn tile_rects(rect: Rect, g: &Game) -> (Vec<(usize, Rect)>, Rect) {
    let slots: Vec<usize> = (0..g.hdd.len()).collect();
    let top = rect.bottom() - anim::TILE_H - 14.0;
    let room = rect.width() - 36.0 - anim::END_W - 12.0;
    let n = slots.len().max(1) as f32;
    let w = ((room - (n - 1.0) * 10.0) / n).clamp(anim::TILE_MIN_W, anim::TILE_W);
    let tiles = slots
        .iter()
        .enumerate()
        .map(|(k, &s)| {
            (
                s,
                Rect::from_min_size(
                    pos2(rect.left() + 18.0 + k as f32 * (w + 10.0), top),
                    vec2(w, anim::TILE_H),
                ),
            )
        })
        .collect();
    let end = Rect::from_min_size(
        pos2(rect.right() - 18.0 - anim::END_W, top),
        vec2(anim::END_W, anim::TILE_H),
    );
    (tiles, end)
}

/// The program tile under the mouse, as egui saw it last frame (the same hover that shows the
/// tile's tooltip), so the model can light its preview before the tiles are drawn.
fn hotbar_hover(ui: &Ui, g: &Game) -> Option<usize> {
    (0..g.hdd.len()).find(|&i| {
        ui.ctx()
            .read_response(tile_id(ui, g.hdd[i].uid))
            .is_some_and(|r| r.hovered())
    })
}

fn tile_id(ui: &Ui, uid: u32) -> egui::Id {
    ui.id().with(("fight-tile", uid))
}

/// Draws the hotbar and runs what's clicked. Returns each tile's (uid, rect).
fn hotbar(ui: &mut Ui, panes: &mut Panes, rect: Rect, hovered: Option<usize>) -> Vec<(u32, Rect)> {
    let g = panes.game;
    let p = panes.pal;
    let Some(c) = &g.combat else {
        return Vec::new();
    };
    let painter = ui.painter_at(rect);
    let (tiles, end) = tile_rects(rect, g);
    painter.text(
        pos2(
            rect.left() + 18.0,
            tiles.first().map_or(end.top(), |t| t.1.top()) - 12.0,
        ),
        Align2::LEFT_CENTER,
        "PROGRAMS",
        mono(11.0),
        p.dim,
    );
    let mut out = Vec::new();
    for &(slot, r) in &tiles {
        let s = &g.hdd[slot];
        let pv = g.preview(slot);
        let ok = pv.is_ok();
        let on = hovered == Some(slot);
        painter.rect_filled(r, 5.0, if on { alpha(p.fg, 0.12) } else { p.bg });
        painter.rect_stroke(
            r,
            5.0,
            Stroke::new(
                if on { 2.0 } else { 1.0 },
                if on {
                    p.bright
                } else if ok {
                    p.dim
                } else {
                    p.faint
                },
            ),
            egui::StrokeKind::Inside,
        );
        let key = Rect::from_min_size(r.min + vec2(8.0, 8.0), vec2(18.0, 18.0));
        painter.rect_filled(key, 2.0, if ok { p.fg } else { p.faint });
        painter.text(
            key.center(),
            Align2::CENTER_CENTER,
            slot.to_string(),
            mono(12.0),
            p.bg,
        );
        let name: String = s
            .spec
            .file()
            .chars()
            .take(((r.width() - 40.0) / 7.5) as usize)
            .collect();
        painter.text(
            pos2(r.left() + 34.0, r.top() + 17.0),
            Align2::LEFT_CENTER,
            name,
            mono(12.0),
            if ok { p.bright } else { p.dim },
        );
        painter.text(
            pos2(r.left() + 10.0, r.top() + 40.0),
            Align2::LEFT_CENTER,
            s.spec.effect.describe(),
            mono(11.0),
            if ok { p.fg } else { p.dim },
        );
        if let Ok(fight::Preview {
            outcome: Outcome::Break { vs, .. },
            ..
        }) = pv
        {
            let badge = match vs {
                Match::Strong => Some("x2"),
                Match::Weak => Some("/2"),
                Match::Plain => None,
            };
            if let Some(b) = badge {
                let br =
                    Rect::from_min_size(pos2(r.right() - 32.0, r.top() + 32.0), vec2(24.0, 16.0));
                painter.rect_filled(
                    br,
                    2.0,
                    if vs == Match::Strong {
                        p.bright
                    } else {
                        p.faint
                    },
                );
                painter.text(br.center(), Align2::CENTER_CENTER, b, mono(11.0), p.bg);
            }
        }
        for k in 0..s.spec.cpu {
            painter.circle_filled(
                pos2(r.left() + 14.0 + k as f32 * 10.0, r.bottom() - 13.0),
                3.5,
                if ok { p.fg } else { p.faint },
            );
        }
        painter.text(
            pos2(
                r.left() + 16.0 + s.spec.cpu as f32 * 10.0,
                r.bottom() - 13.0,
            ),
            Align2::LEFT_CENTER,
            "cyc",
            mono(10.0),
            p.dim,
        );
        let resp = ui.interact(r, tile_id(ui, s.uid), Sense::click());
        let resp = match &pv {
            Err(why) => resp.on_hover_text(why),
            Ok(_) => resp.on_hover_text(s.spec.name.clone()),
        };
        if resp.clicked() && ok {
            panes.act(Action::Run(slot));
        }
        out.push((s.uid, r));
    }
    // End turn, with the CPU left as slots.
    painter.rect_stroke(
        end,
        5.0,
        Stroke::new(1.0, p.bright),
        egui::StrokeKind::Inside,
    );
    painter.text(
        pos2(end.left() + 12.0, end.top() + 17.0),
        Align2::LEFT_CENTER,
        "CPU",
        mono(11.0),
        p.dim,
    );
    for k in 0..g.deck.cpu_max {
        let r = Rect::from_min_size(
            pos2(end.left() + 46.0 + k as f32 * 20.0, end.top() + 10.0),
            vec2(15.0, 14.0),
        );
        painter.rect_filled(r, 1.0, if k < c.cpu { p.fg } else { p.faint });
    }
    painter.text(
        pos2(end.center().x, end.top() + 46.0),
        Align2::CENTER_CENTER,
        "[space] END TURN",
        mono(13.0),
        p.bright,
    );
    if ui
        .interact(end, ui.id().with("fight-end"), Sense::click())
        .clicked()
    {
        panes.act(Action::EndTurn);
    }
    out
}
