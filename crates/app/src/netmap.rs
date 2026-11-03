//! The main editor pane: the network. Three views share what's visible, what's clickable and
//! what each node is called: floating isometric wireframes (`netmap_iso`), a flat technical
//! draft (`netmap_draft`) or a 1-bit MacPaint picture (`netmap_paint`). Returns the node the
//! player clicked.

use egui::{Color32, Rect, Sense, Ui};
use ninsei_core::netview::{MapNode, MapView, style};
use ninsei_core::{ContractStatus, Game, NodeKind, Phase};

use crate::theme::Palette;

/// Nodes further than this many layers past the runner are fogged.
const SIGHT: usize = 1;

/// The map's animation timings and strengths, in one place. Display numbers, not balance.
pub mod anim {
    /// How long a node's ICE takes to blow apart when it falls.
    pub const BURST_SECS: f64 = 1.0;
    /// How far the pieces fly, in shape radii.
    pub const BURST_SPREAD: f32 = 1.6;
    /// How long the map takes to build itself, layer by layer, when a run starts.
    pub const SWEEP_SECS: f64 = 0.5;
    /// Trace (%) above which the grid's pulse quickens.
    pub const TRACE_FAST: u32 = 75;
    /// The grid's pulse, in radians a second: calm, then fast.
    pub const PULSE_SLOW: f32 = 1.6;
    pub const PULSE_FAST: f32 = 5.5;
    /// A hovered route lifts this far (world units) and spins this many times faster.
    pub const HOVER_LIFT: f32 = 0.18;
    pub const HOVER_SPIN: f32 = 3.0;
}

/// What the map remembers between frames: when this network appeared, and which ICE has
/// fallen since, so new derezzes can burst. Kept in egui's memory, not in the game.
#[derive(Clone, Default)]
struct Memory {
    /// Seed, inside a subnet or not, and node count: a new network means a fresh map.
    key: Option<(u64, bool, usize)>,
    hops: u32,
    start: f64,
    cleared: Vec<bool>,
    /// (node, time it fell).
    bursts: Vec<(usize, f64)>,
}

/// This frame's view of the map's animations.
pub struct Anim {
    /// Seconds, for spins, pulses and blinks.
    pub t: f64,
    /// Seconds since this network appeared.
    age: f64,
    bursts: Vec<(usize, f64)>,
}

impl Anim {
    /// Reads and updates the map's memory for this frame.
    pub fn frame(ui: &Ui, game: &Game) -> Anim {
        let t = ui.input(|i| i.time);
        let id = ui.id().with("netmap-anim");
        let net = &game.net;
        let key = (game.seed, game.in_subnet(), net.nodes.len());
        let now: Vec<bool> = net.nodes.iter().map(|n| n.cleared).collect();
        let mem = ui.ctx().data_mut(|d| {
            let m: &mut Memory = d.get_temp_mut_or_default(id);
            if m.key != Some(key) || game.hops < m.hops {
                *m = Memory {
                    key: Some(key),
                    hops: game.hops,
                    start: t,
                    cleared: now.clone(),
                    bursts: Vec::new(),
                };
            }
            for (i, n) in net.nodes.iter().enumerate() {
                let was = m.cleared.get(i).copied().unwrap_or(true);
                if n.cleared && !was && matches!(n.kind, NodeKind::Ice | NodeKind::BlackIce) {
                    m.bursts.push((i, t));
                }
            }
            m.bursts.retain(|&(_, at)| t - at < anim::BURST_SECS);
            m.cleared = now;
            m.hops = game.hops;
            m.clone()
        });
        Anim {
            t,
            age: t - mem.start,
            bursts: mem.bursts,
        }
    }

    /// How far node `id` is through blowing apart (0..1), if it's bursting.
    pub fn burst(&self, id: usize) -> Option<f32> {
        self.bursts
            .iter()
            .find(|&&(n, _)| n == id)
            .map(|&(_, at)| (((self.t - at) / anim::BURST_SECS) as f32).clamp(0.0, 1.0))
    }

    /// How built a layer is (0..1) as the map sweeps in at the start of a run.
    pub fn reveal(&self, layer: usize, layers: usize) -> f32 {
        let at = anim::SWEEP_SECS * layer as f64 / layers.max(1) as f64;
        (((self.age - at) / (anim::SWEEP_SECS / 3.0)) as f32).clamp(0.0, 1.0)
    }

    /// Whether anything is still moving that needs another frame.
    pub fn busy(&self) -> bool {
        !self.bursts.is_empty() || self.age < anim::SWEEP_SECS * 1.5
    }

    /// Trace pressure: how strongly the grid lights up (0..1), pulsing faster past
    /// `TRACE_FAST`.
    pub fn pressure(&self, trace: u32) -> f32 {
        let f = (trace as f32 / 100.0).clamp(0.0, 1.0).powf(1.5);
        let speed = if trace >= anim::TRACE_FAST {
            anim::PULSE_FAST
        } else {
            anim::PULSE_SLOW
        };
        f * (0.7 + 0.3 * (self.t as f32 * speed).sin())
    }
}

/// Mixes two theme colours.
pub fn mix(a: Color32, b: Color32, k: f32) -> Color32 {
    let k = k.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * k).round() as u8;
    Color32::from_rgba_premultiplied(
        m(a.r(), b.r()),
        m(a.g(), b.g()),
        m(a.b(), b.b()),
        m(a.a(), b.a()),
    )
}

/// The BBS node in its board's colours: the board's frame and label CGA colours, mapped onto
/// the theme the same way the BBS window's phosphor mode does. (edge, label)
pub fn board_ink(game: &Game, p: &Palette) -> (Color32, Color32) {
    let l = &game.bbs().look;
    let pick = |c: u8, or: Color32| {
        // Never fainter than `dim`, so a dark board colour still reads as a lit node.
        match crate::bbs_view::mono(c, p) {
            m if m == Color32::TRANSPARENT => or,
            m if m == p.faint => p.dim,
            m => m,
        }
    };
    (pick(l.frame, p.fg), pick(l.label, p.bright))
}

pub fn show(ui: &mut Ui, game: &Game, p: &Palette, view: MapView) -> Option<usize> {
    match view {
        MapView::Iso => crate::netmap_iso::show(ui, game, p),
        MapView::Draft => crate::netmap_draft::show(ui, game, p),
        MapView::Paint => crate::netmap_paint::show(ui, game, p),
    }
}

/// Whether the runner can see what node `id` is. The BBS number is passed around, and the
/// client gave you contract targets' hostnames; the rest is fog past the next layer.
pub fn visible(game: &Game, id: usize) -> bool {
    let net = &game.net;
    net.nodes[id].layer <= net.nodes[game.at].layer + SIGHT
        || net.nodes[id].kind == NodeKind::Bbs
        || target(game, id)
}

/// Whether node `id` is an active contract's target. They live on the main network, so none
/// show inside a subnet.
pub fn target(game: &Game, id: usize) -> bool {
    !game.in_subnet()
        && game
            .contracts
            .iter()
            .any(|k| k.node == id && k.status == ContractStatus::Active)
}

pub fn kind(game: &Game, id: usize) -> MapNode {
    MapNode::of(&game.net, id, &game.data)
}

/// The short label under a node.
pub fn tag(game: &Game, id: usize) -> String {
    if !visible(game, id) {
        return "???".into();
    }
    let k = kind(game, id);
    if k == MapNode::Exit && game.in_subnet() {
        return "UPLINK".into();
    }
    if k == MapNode::Bbs {
        return game.bbs().name.clone();
    }
    style(&game.data, k).tag.clone()
}

/// Makes a node's screen area clickable when it's a route, with a tooltip. Returns the node
/// if it was clicked, and whether it's hovered.
pub fn interact(ui: &mut Ui, game: &Game, id: usize, r: Rect) -> (Option<usize>, bool) {
    let can_go = game.reachable().contains(&id);
    let sense = if can_go {
        Sense::click()
    } else {
        Sense::hover()
    };
    let resp = ui.interact(r, ui.id().with(("node", id)), sense);
    let hovered = resp.hovered();
    if !can_go {
        return (None, hovered);
    }
    let what = if visible(game, id) {
        game.node_label(id)
    } else {
        "unknown"
    };
    let resp = resp.on_hover_text(format!("jack {id}  →  {what}"));
    (resp.clicked().then_some(id), hovered)
}

/// The line along the bottom: what to do now.
pub fn hint(game: &Game) -> &'static str {
    match game.phase {
        Phase::Exploring if game.in_subnet() => {
            "inside a subnet. the UPLINK takes you back to the main network"
        }
        Phase::Exploring => "click a lit node (or `jack <n>`) to route there",
        Phase::Combat => "ICE holds the line. break it in target.ice",
        Phase::Encounter => "someone wants to talk",
        Phase::Flatlined => "FLATLINED. `newrun` to jack in again",
        Phase::JackedOut => "JACKED OUT. `newrun` to go deeper",
    }
}
