//! The network you traverse: a layered graph from an entry node to an exit node,
//! Slay-the-Spire style. Links only go forward, one layer at a time.

use crate::data::Data;
use crate::rng::Rng;
use crate::tuning::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Entry,
    Ice,
    BlackIce,
    Vault,
    Bbs,
    /// An encounter: someone wants to talk.
    Contact,
    Exit,
}

impl NodeKind {
    pub fn label(self) -> &'static str {
        match self {
            NodeKind::Entry => "ENTRY",
            NodeKind::Ice => "ICE",
            NodeKind::BlackIce => "BLACK ICE",
            NodeKind::Vault => "DATA VAULT",
            NodeKind::Bbs => "BBS",
            NodeKind::Contact => "MSG",
            NodeKind::Exit => "EXIT",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Node {
    pub id: usize,
    pub layer: usize,
    /// Position within its layer, 0-based.
    pub lane: usize,
    pub kind: NodeKind,
    /// Hostname shown in the UI, e.g. `srv-3f2a`.
    pub name: String,
    /// Index into `Data::ice` for ICE nodes.
    pub ice: Option<usize>,
    pub cleared: bool,
    /// Ids of nodes in the next layer this one links to.
    pub links: Vec<usize>,
}

#[derive(Clone, Debug)]
pub struct Network {
    pub nodes: Vec<Node>,
    /// Node ids grouped by layer.
    pub layers: Vec<Vec<usize>>,
}

impl Network {
    pub fn generate(rng: &mut Rng, data: &Data) -> Self {
        let mut widths = vec![1];
        for _ in 0..NET_MID_LAYERS {
            widths.push(rng.range(NET_MIN_WIDTH as u32, NET_MAX_WIDTH as u32) as usize);
        }
        widths.push(1);
        let last = widths.len() - 1;

        let mut nodes = Vec::new();
        let mut layers = Vec::new();
        for (layer, &width) in widths.iter().enumerate() {
            let mut ids = Vec::new();
            for lane in 0..width {
                let kind = if layer == 0 {
                    NodeKind::Entry
                } else if layer == last {
                    NodeKind::Exit
                } else {
                    pick_kind(rng, layer)
                };
                let ice = match kind {
                    NodeKind::Ice => pick_ice(rng, data, 1),
                    NodeKind::BlackIce => pick_ice(rng, data, 2),
                    _ => None,
                };
                let id = nodes.len();
                nodes.push(Node {
                    id,
                    layer,
                    lane,
                    kind,
                    name: hostname(rng, kind),
                    ice,
                    cleared: kind == NodeKind::Entry,
                    links: Vec::new(),
                });
                ids.push(id);
            }
            layers.push(ids);
        }

        // One BBS per network, on a fixed layer.
        let bbs = layers[BBS_LAYER][rng.below(layers[BBS_LAYER].len())];
        nodes[bbs].kind = NodeKind::Bbs;
        nodes[bbs].ice = None;
        nodes[bbs].name = hostname(rng, NodeKind::Bbs);

        for l in 0..last {
            let (from, to) = (&layers[l], &layers[l + 1]);
            // Every node gets a forward link at its proportional lane.
            for (lane, &a) in from.iter().enumerate() {
                let b = to[scale_lane(lane, from.len(), to.len())];
                link(&mut nodes, a, b);
            }
            // Every next-layer node is reachable from its proportional lane.
            for (lane, &b) in to.iter().enumerate() {
                if !from.iter().any(|&a| nodes[a].links.contains(&b)) {
                    let a = from[scale_lane(lane, to.len(), from.len())];
                    link(&mut nodes, a, b);
                }
            }
            // A few extra cross links to a neighbouring lane give real route choices.
            for &a in from {
                if to.len() > 1 && rng.chance(NET_EXTRA_LINK_PCT) {
                    let base = scale_lane(nodes[a].lane, from.len(), to.len());
                    let other = if base + 1 < to.len() && (base == 0 || rng.chance(50)) {
                        base + 1
                    } else {
                        base - 1
                    };
                    link(&mut nodes, a, to[other]);
                }
            }
        }
        for n in &mut nodes {
            n.links.sort_unstable();
        }
        Self { nodes, layers }
    }

    pub fn bbs(&self) -> Option<usize> {
        self.nodes.iter().position(|n| n.kind == NodeKind::Bbs)
    }

    /// Every node reachable from `from` (inclusive), following links forward.
    pub fn reachable_from(&self, from: usize) -> Vec<bool> {
        let mut seen = vec![false; self.nodes.len()];
        let mut stack = vec![from];
        while let Some(n) = stack.pop() {
            if !std::mem::replace(&mut seen[n], true) {
                stack.extend(&self.nodes[n].links);
            }
        }
        seen
    }

    pub fn entry(&self) -> usize {
        self.layers[0][0]
    }
}

fn link(nodes: &mut [Node], a: usize, b: usize) {
    if !nodes[a].links.contains(&b) {
        nodes[a].links.push(b);
    }
}

/// Maps a lane in a layer of `from` nodes to the nearest lane in a layer of `to` nodes.
fn scale_lane(lane: usize, from: usize, to: usize) -> usize {
    if from <= 1 || to <= 1 {
        return if to <= 1 { 0 } else { (to - 1) / 2 };
    }
    (lane * (to - 1) + (from - 1) / 2) / (from - 1)
}

fn pick_kind(rng: &mut Rng, layer: usize) -> NodeKind {
    let black = if layer >= BLACK_ICE_MIN_LAYER {
        NODE_WEIGHT_BLACK_ICE
    } else {
        0
    };
    match rng.weighted([
        NODE_WEIGHT_ICE,
        NODE_WEIGHT_VAULT,
        black,
        NODE_WEIGHT_CONTACT,
    ]) {
        Some(1) => NodeKind::Vault,
        Some(2) => NodeKind::BlackIce,
        Some(3) => NodeKind::Contact,
        _ => NodeKind::Ice,
    }
}

fn pick_ice(rng: &mut Rng, data: &Data, tier: u32) -> Option<usize> {
    let pool: Vec<usize> = (0..data.ice.len())
        .filter(|&i| data.ice[i].tier == tier)
        .collect();
    (!pool.is_empty()).then(|| pool[rng.below(pool.len())])
}

fn hostname(rng: &mut Rng, kind: NodeKind) -> String {
    let prefix = match kind {
        NodeKind::Entry => "gw",
        NodeKind::Ice => "srv",
        NodeKind::BlackIce => "blk",
        NodeKind::Vault => "dat",
        NodeKind::Bbs => "bbs",
        NodeKind::Contact => "msg",
        NodeKind::Exit => "out",
    };
    format!("{prefix}-{:04x}", rng.next_u64() & 0xffff)
}
