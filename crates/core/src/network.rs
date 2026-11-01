//! The network you traverse: a layered graph from an entry node to an exit node,
//! Slay-the-Spire style. Links only go forward, one layer at a time.
//!
//! A network's [`Shape`] sets how wide its layers run, how often routes cross and
//! where it funnels through chokepoints. A `Subnet` node is a door to a smaller
//! network of its own: its exit is an uplink back to the subnet node.

use serde::Deserialize;

use crate::data::{Data, IceClass};
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
    /// A door to a private segment: a smaller network whose exit leads back here.
    Subnet,
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
            NodeKind::Subnet => "SUBNET",
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
    /// A network with default parameters.
    pub fn generate(rng: &mut Rng, data: &Data) -> Self {
        Self::generate_with(rng, data, &NetParams::default())
    }

    /// A network shaped by `p`: tier size, trait biases.
    pub fn generate_with(rng: &mut Rng, data: &Data, p: &NetParams) -> Self {
        let shape = p.shape;
        let mut widths = vec![1];
        for layer in 1..=p.mid_layers.max(BBS_LAYER) {
            if shape.is_choke(layer) {
                widths.push(1);
            } else {
                widths.push(rng.range(shape.min_width as u32, shape.max_width as u32) as usize);
            }
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
                } else if shape.is_choke(layer) {
                    pick_guard(rng, layer, p)
                } else {
                    pick_kind(rng, layer, p)
                };
                let ice = match kind {
                    NodeKind::Ice => pick_ice(rng, data, 1, p.ice_bias),
                    NodeKind::BlackIce => pick_ice(rng, data, 2, p.ice_bias),
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

        if p.bbs {
            // One BBS per network, on a fixed layer.
            let bbs = layers[BBS_LAYER][rng.below(layers[BBS_LAYER].len())];
            nodes[bbs].kind = NodeKind::Bbs;
            nodes[bbs].ice = None;
            nodes[bbs].name = hostname(rng, NodeKind::Bbs);
        }

        // Subnets sit deeper in, never on a chokepoint or the BBS.
        if p.subnets > 0 {
            let mut pool: Vec<usize> = (SUBNET_MIN_LAYER..last)
                .filter(|&l| !shape.is_choke(l))
                .flat_map(|l| layers[l].iter().copied())
                .filter(|&n| nodes[n].kind != NodeKind::Bbs)
                .collect();
            for _ in 0..p.subnets {
                if pool.is_empty() {
                    break;
                }
                let n = pool.swap_remove(rng.below(pool.len()));
                nodes[n].kind = NodeKind::Subnet;
                nodes[n].ice = None;
                nodes[n].name = hostname(rng, NodeKind::Subnet);
            }
        }

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
                if to.len() > 1 && rng.chance(shape.links) {
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

    /// The private network behind subnet node `node`: shorter, no BBS, vault-rich,
    /// in the parent's shape. The same run and node always lead to the same subnet.
    pub fn subnet(seed: u64, node: usize, data: &Data, parent: &NetParams) -> Self {
        let p = NetParams {
            mid_layers: SUBNET_MID_LAYERS,
            vault_bonus: parent.vault_bonus + SUBNET_VAULT_BONUS,
            bbs: false,
            subnets: 0,
            ..parent.clone()
        };
        let mut rng = Rng::new(seed ^ (node as u64 + 1).wrapping_mul(0xD1B5_4A32_D192_ED03));
        Self::generate_with(&mut rng, data, &p)
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

fn pick_kind(rng: &mut Rng, layer: usize, p: &NetParams) -> NodeKind {
    let black = if layer >= BLACK_ICE_MIN_LAYER {
        NODE_WEIGHT_BLACK_ICE + p.black_bonus
    } else {
        0
    };
    match rng.weighted([
        NODE_WEIGHT_ICE,
        NODE_WEIGHT_VAULT + p.vault_bonus,
        black,
        NODE_WEIGHT_CONTACT + p.contact_bonus,
    ]) {
        Some(1) => NodeKind::Vault,
        Some(2) => NodeKind::BlackIce,
        Some(3) => NodeKind::Contact,
        _ => NodeKind::Ice,
    }
}

/// A chokepoint is always guarded.
fn pick_guard(rng: &mut Rng, layer: usize, p: &NetParams) -> NodeKind {
    let black = if layer >= BLACK_ICE_MIN_LAYER {
        NODE_WEIGHT_BLACK_ICE + p.black_bonus
    } else {
        0
    };
    match rng.weighted([NODE_WEIGHT_ICE, black]) {
        Some(1) => NodeKind::BlackIce,
        _ => NodeKind::Ice,
    }
}

fn pick_ice(rng: &mut Rng, data: &Data, tier: u32, bias: Option<IceClass>) -> Option<usize> {
    let pool: Vec<usize> = (0..data.ice.len())
        .filter(|&i| data.ice[i].tier == tier)
        .collect();
    // A biased network mostly runs ICE of its favourite class.
    if let Some(class) = bias {
        let favoured: Vec<usize> = pool
            .iter()
            .copied()
            .filter(|&i| data.ice[i].class == class)
            .collect();
        if !favoured.is_empty() && rng.chance(ICE_BIAS_PCT) {
            return Some(favoured[rng.below(favoured.len())]);
        }
    }
    (!pool.is_empty()).then(|| pool[rng.below(pool.len())])
}

/// Knobs network generation takes: a tier's size and its traits' biases.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetParams {
    /// Layers between the entry and the exit (at least `BBS_LAYER`).
    pub mid_layers: usize,
    /// Extra weight for black ICE, vaults and MSG nodes.
    pub black_bonus: u32,
    pub vault_bonus: u32,
    pub contact_bonus: u32,
    /// ICE class this network favours.
    pub ice_bias: Option<IceClass>,
    /// Whether the network has its one BBS.
    pub bbs: bool,
    /// How the layers are laid out.
    pub shape: Shape,
    /// Subnet nodes to place.
    pub subnets: usize,
}

/// A network's topology: layer widths, how often routes cross, and chokepoints.
/// Picked by a trait in `data/hub/networks.ron`.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shape {
    /// Nodes per middle layer, rolled in `min_width..=max_width`.
    pub min_width: usize,
    pub max_width: usize,
    /// Chance (percent) a node gets an extra link to a neighbouring lane.
    pub links: u32,
    /// Every `choke`th middle layer is a single guarded node (0 = none). Never the BBS layer.
    #[serde(default)]
    pub choke: usize,
}

impl Shape {
    pub fn is_choke(self, layer: usize) -> bool {
        self.choke > 0 && layer > 0 && layer.is_multiple_of(self.choke) && layer != BBS_LAYER
    }

    pub fn validate(self) -> Result<(), String> {
        if self.min_width == 0 || self.min_width > self.max_width {
            return Err("shape widths must be 1 or more and min <= max".into());
        }
        if self.max_width > NET_WIDEST {
            return Err(format!("shape is wider than {NET_WIDEST} lanes"));
        }
        if self.links > 100 {
            return Err("shape link chance is over 100%".into());
        }
        Ok(())
    }
}

impl Default for Shape {
    fn default() -> Self {
        Self {
            min_width: NET_MIN_WIDTH,
            max_width: NET_MAX_WIDTH,
            links: NET_EXTRA_LINK_PCT,
            choke: 0,
        }
    }
}

impl Default for NetParams {
    fn default() -> Self {
        Self {
            mid_layers: NET_MID_LAYERS,
            black_bonus: 0,
            vault_bonus: 0,
            contact_bonus: 0,
            ice_bias: None,
            bbs: true,
            shape: Shape::default(),
            subnets: 0,
        }
    }
}

fn hostname(rng: &mut Rng, kind: NodeKind) -> String {
    let prefix = match kind {
        NodeKind::Entry => "gw",
        NodeKind::Ice => "srv",
        NodeKind::BlackIce => "blk",
        NodeKind::Vault => "dat",
        NodeKind::Bbs => "bbs",
        NodeKind::Contact => "msg",
        NodeKind::Subnet => "sub",
        NodeKind::Exit => "out",
    };
    format!("{prefix}-{:04x}", rng.next_u64() & 0xffff)
}
