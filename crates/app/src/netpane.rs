//! The net pane: network traffic, after btop's net box. A mirrored dot graph scrolls in real
//! time, download growing up from the middle and upload growing down, with rate, peak and
//! total on the side. Traffic comes from what actually happens in the run (`Game::history`):
//! hits taken and credits coming in are downloads, CPU spent and hops are uploads. Between
//! events the line idles with a little noise. The byte amounts are display numbers, here.

use std::collections::VecDeque;

use egui::{Align2, FontFamily, FontId, Painter, Rect, Sense, Stroke, Ui, pos2, vec2};
use ninsei_core::Game;

use crate::btop::ramp;
use crate::theme::Palette;

/// Traffic sizes and timings, in one place. Display numbers, not balance.
pub mod traffic {
    /// One graph column per this many seconds.
    pub const STEP_SECS: f64 = 0.25;
    /// Columns kept (about a minute at 0.25 s).
    pub const KEEP: usize = 240;
    /// KiB down for each point of integrity an ICE hit takes.
    pub const KIB_PER_HIT: f32 = 1.8;
    /// KiB down for each credit coming in (loot, pay).
    pub const KIB_PER_CREDIT_IN: f32 = 0.35;
    /// KiB down for each credit spent (BBS downloads).
    pub const KIB_PER_CREDIT_SPENT: f32 = 0.6;
    /// KiB up for each CPU cycle spent running programs.
    pub const KIB_PER_CPU: f32 = 2.4;
    /// KiB up for each hop (the handshake with the next node).
    pub const KIB_PER_HOP: f32 = 3.2;
    /// Idle chatter per column, up to this many KiB each way.
    pub const IDLE_KIB: f32 = 0.35;
    /// How fast a burst of traffic drains into the graph: a share of what's pending per column.
    pub const DRAIN: f32 = 0.45;
}

/// What the pane remembers between frames (egui memory, not the game).
#[derive(Clone, Default)]
struct Memory {
    /// Seed and node count: a new run starts a fresh graph.
    key: Option<(u64, usize)>,
    /// The history tick last read.
    tick: u32,
    /// When the last column went in.
    stepped: f64,
    /// Traffic waiting to drain into the graph: (down, up) KiB.
    pending: (f32, f32),
    /// Columns, oldest first: (down, up) KiB.
    cols: VecDeque<(f32, f32)>,
    total: (f32, f32),
    top: (f32, f32),
}

/// Traffic (down, up) in KiB for what happened between two samples.
fn between(a: &ninsei_core::history::Sample, b: &ninsei_core::history::Sample) -> (f32, f32) {
    use traffic::*;
    let mut down = a.integrity.saturating_sub(b.integrity) as f32 * KIB_PER_HIT;
    down += b.credits.saturating_sub(a.credits) as f32 * KIB_PER_CREDIT_IN;
    down += a.credits.saturating_sub(b.credits) as f32 * KIB_PER_CREDIT_SPENT;
    let mut up = a.cpu.saturating_sub(b.cpu) as f32 * KIB_PER_CPU;
    up += b.hops.saturating_sub(a.hops) as f32 * KIB_PER_HOP;
    (down, up)
}

/// A small, steady wobble from the column number, so idle traffic isn't flat.
fn idle(n: u64, salt: u64) -> f32 {
    let mut x = n.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt;
    x ^= x >> 29;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 32;
    (x % 1000) as f32 / 1000.0 * traffic::IDLE_KIB
}

/// Reads new history, steps the graph in real time, and returns its state.
fn update(ui: &Ui, game: &Game) -> Memory {
    let id = ui.id().with("netpane");
    let now = ui.input(|i| i.time);
    let h = &game.history;
    let key = (game.seed, game.net.nodes.len());
    ui.ctx().data_mut(|d| {
        let m: &mut Memory = d.get_temp_mut_or_default(id);
        if m.key != Some(key) || h.tick < m.tick {
            *m = Memory {
                key: Some(key),
                tick: h.tick,
                stepped: now,
                ..Memory::default()
            };
        }
        // New actions since last frame: their traffic waits to drain into the graph.
        let new = (h.tick - m.tick) as usize;
        let n = h.samples.len();
        let from = n.saturating_sub(new + 1);
        for w in h.samples[from..].windows(2) {
            let (dn, up) = between(&w[0], &w[1]);
            m.pending.0 += dn;
            m.pending.1 += up;
        }
        m.tick = h.tick;
        // One column per step, however many frames that took.
        let mut steps = 0;
        while now - m.stepped >= traffic::STEP_SECS && steps < 40 {
            m.stepped += traffic::STEP_SECS;
            steps += 1;
            let c = (m.stepped / traffic::STEP_SECS) as u64;
            let take = (m.pending.0 * traffic::DRAIN, m.pending.1 * traffic::DRAIN);
            m.pending = (m.pending.0 - take.0, m.pending.1 - take.1);
            let col = (take.0 + idle(c, 1), take.1 + idle(c, 2));
            m.total = (m.total.0 + col.0, m.total.1 + col.1);
            m.top = (m.top.0.max(col.0), m.top.1.max(col.1));
            m.cols.push_back(col);
            if m.cols.len() > traffic::KEEP {
                m.cols.pop_front();
            }
        }
        if now - m.stepped > 2.0 {
            m.stepped = now;
        }
        m.clone()
    })
}

/// "3.49 KiB" / "1.20 MiB".
fn size(kib: f32) -> String {
    if kib >= 1024.0 * 1024.0 {
        format!("{:.2} GiB", kib / 1024.0 / 1024.0)
    } else if kib >= 1024.0 {
        format!("{:.2} MiB", kib / 1024.0)
    } else {
        format!("{kib:.2} KiB")
    }
}

/// Kibibits per second from KiB per second, shortened.
fn bits(kib_s: f32) -> String {
    let kib = kib_s * 8.0;
    if kib >= 1024.0 {
        format!("{:.2} Mibps", kib / 1024.0)
    } else {
        format!("{kib:.1} Kibps")
    }
}

/// The mirrored graph: down above the middle line, up below, newest on the right.
fn draw(painter: &Painter, rect: Rect, p: &Palette, cols: &VecDeque<(f32, f32)>) {
    let n = ((rect.width() / 4.0).floor() as usize).max(1);
    let half = ((rect.height() / 2.0 / 4.0).floor() as usize).max(1);
    let peak = cols
        .iter()
        .rev()
        .take(n)
        .map(|c| c.0.max(c.1))
        .fold(0.5_f32, f32::max);
    let dx = rect.width() / n as f32;
    let dy = rect.height() / 2.0 / half as f32;
    let mid = rect.center().y;
    let r = (dx.min(dy) * 0.32).max(0.8);
    let shown: Vec<&(f32, f32)> = cols.iter().rev().take(n).collect();
    for (i, c) in shown.iter().enumerate() {
        let x = rect.right() - (i as f32 + 0.5) * dx;
        for (v, dir) in [(c.0, -1.0), (c.1, 1.0)] {
            let h = ((v / peak) * half as f32).round() as usize;
            for d in 0..h.min(half) {
                let y = mid + dir * (d as f32 + 0.5) * dy;
                painter.circle_filled(pos2(x, y), r, ramp(p, (d as f32 + 1.0) / half as f32));
            }
        }
    }
    painter.line_segment(
        [pos2(rect.left(), mid), pos2(rect.right(), mid)],
        Stroke::new(1.0, p.faint),
    );
    let font = FontId::new(10.0, FontFamily::Monospace);
    let label = format!("{:.0}K", peak / traffic::STEP_SECS as f32);
    painter.text(
        rect.left_top(),
        Align2::LEFT_TOP,
        &label,
        font.clone(),
        p.dim,
    );
    painter.text(rect.left_bottom(), Align2::LEFT_BOTTOM, &label, font, p.dim);
}

/// Traffic (down, up) in KiB for each hop of the run so far, from the history: what the
/// suite's network usage report tabulates.
pub fn per_hop(game: &Game) -> Vec<(f32, f32)> {
    let mut out = vec![(0.0, 0.0); game.hops as usize + 1];
    for w in game.history.samples.windows(2) {
        let (dn, up) = between(&w[0], &w[1]);
        if let Some(c) = out.get_mut(w[1].hops as usize) {
            c.0 += dn;
            c.1 += up;
        }
    }
    out
}

/// "3.49 KiB": sizes the way the net pane prints them.
pub fn kib(v: f32) -> String {
    size(v)
}

/// The live traffic graph with its stats box, filling the pane (each chrome frames it).
pub fn live(ui: &mut Ui, game: &Game, p: &Palette) {
    ui.ctx().request_repaint();
    let m = update(ui, game);
    let body = |ui: &mut Ui| {
        let rect = ui.available_rect_before_wrap();
        ui.allocate_rect(rect, Sense::hover());
        let painter = ui.painter_at(rect);
        let stats_w = if rect.width() > 330.0 { 172.0 } else { 0.0 };
        let graph = Rect::from_min_max(
            rect.min,
            rect.max - vec2(stats_w + 8.0 * (stats_w > 0.0) as u8 as f32, 0.0),
        );
        draw(&painter, graph, p, &m.cols);
        if stats_w == 0.0 {
            return;
        }
        let last = m.cols.back().copied().unwrap_or_default();
        let rate = |kib: f32| kib / traffic::STEP_SECS as f32;
        let font = FontId::new(10.0, FontFamily::Monospace);
        let x = graph.right() + 10.0;
        let lines = [
            ("download", String::new(), p.bright),
            (
                "▼",
                format!("{}/s ({})", size(rate(last.0)), bits(rate(last.0))),
                p.fg,
            ),
            ("▼ Top:", bits(rate(m.top.0)), p.fg),
            ("▼ Total:", size(m.total.0), p.fg),
            ("", String::new(), p.fg),
            (
                "▲",
                format!("{}/s ({})", size(rate(last.1)), bits(rate(last.1))),
                p.fg,
            ),
            ("▲ Top:", bits(rate(m.top.1)), p.fg),
            ("▲ Total:", size(m.total.1), p.fg),
            ("upload", String::new(), p.bright),
        ];
        let row = 15.0;
        let top = rect.center().y - row * lines.len() as f32 / 2.0;
        let r = Rect::from_min_max(
            pos2(x - 4.0, top - 2.0),
            pos2(rect.right(), top + row * lines.len() as f32 + 2.0),
        );
        painter.rect_stroke(r, 4.0, Stroke::new(1.0, p.faint), egui::StrokeKind::Middle);
        for (i, (k, v, c)) in lines.iter().enumerate() {
            let y = top + row * i as f32;
            painter.text(pos2(x, y), Align2::LEFT_TOP, *k, font.clone(), *c);
            painter.text(
                pos2(rect.right() - 6.0, y),
                Align2::RIGHT_TOP,
                v,
                font.clone(),
                *c,
            );
        }
    };
    body(ui);
}
