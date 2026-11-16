//! The network map's geometry, kept out of the app so it's tested headlessly: which shape each
//! node gets (`data/netmap.ron`), the wireframe meshes, and the isometric projection.

use serde::{Deserialize, Serialize};

use crate::data::{Data, IceClass};
use crate::game::{ContractStatus, Game};
use crate::network::{Network, NodeKind};

/// Which way the network map is drawn. Saved with the hub.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MapView {
    /// Floating wireframe shapes over a glowing grid.
    #[default]
    Iso,
    /// A flat plan, drawn like a technical drawing.
    Draft,
    /// A 1-bit MacPaint picture on a card: nodes filled with patterns.
    Paint,
}

/// A kind of node as the map tells them apart: ICE splits by class.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapNode {
    Entry,
    Barrier,
    CodeGate,
    Sentry,
    BlackIce,
    Vault,
    Bbs,
    Contact,
    Subnet,
    Exit,
}

impl MapNode {
    pub const ALL: [MapNode; 10] = [
        MapNode::Entry,
        MapNode::Barrier,
        MapNode::CodeGate,
        MapNode::Sentry,
        MapNode::BlackIce,
        MapNode::Vault,
        MapNode::Bbs,
        MapNode::Contact,
        MapNode::Subnet,
        MapNode::Exit,
    ];

    /// What node `id` looks like on the map.
    pub fn of(net: &Network, id: usize, data: &Data) -> MapNode {
        let n = &net.nodes[id];
        match n.kind {
            NodeKind::Entry => MapNode::Entry,
            NodeKind::Ice => match n.ice.and_then(|i| data.ice.get(i)).map(|d| d.class) {
                Some(IceClass::Barrier) => MapNode::Barrier,
                Some(IceClass::CodeGate) => MapNode::CodeGate,
                _ => MapNode::Sentry,
            },
            NodeKind::BlackIce => MapNode::BlackIce,
            NodeKind::Vault => MapNode::Vault,
            NodeKind::Bbs => MapNode::Bbs,
            NodeKind::Contact => MapNode::Contact,
            NodeKind::Subnet => MapNode::Subnet,
            NodeKind::Exit => MapNode::Exit,
        }
    }
}

/// Wireframe shapes for the isometric view.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeshKind {
    Cube,
    Ring,
    Ico,
    Spikes,
    Drum,
    Octa,
    Pyramid,
    Nested,
    Pad,
    Portal,
    /// The net's AI: an eye of a ring round a core. Never a node's; the map floats it over one.
    Ai,
}

/// Drafting symbols for the flat view. The app draws them.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Symbol {
    Hatch,
    Valve,
    Eye,
    Crosshatch,
    Column,
    Hex,
    Flag,
    Nested,
    Inlet,
    Door,
}

/// One line of `data/netmap.ron`.
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct MapStyleDef {
    pub node: MapNode,
    pub mesh: MeshKind,
    pub symbol: Symbol,
    pub tag: String,
    /// How the Paint view fills the node: a pattern id (`data/patterns.ron`), or `paper`
    /// (blank, with its tag written inside) or `solid` (all ink).
    pub paint: String,
}

/// Nodes further than this many layers past the runner are fogged on the map.
pub const SIGHT: usize = 1;

/// Whether the runner can see what node `id` is: the BBS number is passed around and a client
/// gave you its targets' hostnames; past the next layer the rest is fog.
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

/// The net's AI as the map marks it: where the runner knows it to be.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiMark {
    /// The node it is on, or was last known on.
    pub node: usize,
    /// The runner knows it is there now (in sight, or its scan was heard); otherwise it's
    /// where it was last known.
    pub now: bool,
    /// It is there now and the node is in sight: the node is the AI's, drawn as it, its name
    /// for the node's tag. There now in the fog (its scan heard), it's marked over the `?`.
    pub held: bool,
    /// What the map writes beside it: its product name and version (`WATCHMAN 1.2`).
    pub label: String,
}

impl AiMark {
    /// Whether it holds node `id` ([`AiMark::held`]).
    pub fn holds(&self, id: usize) -> bool {
        self.held && self.node == id
    }
}

/// Where the map marks the net's AI, from what the runner knows of it (`Game::sighting`).
/// Nothing while it's asleep or down, before they ever knew where it was, or inside a subnet:
/// it's on the main network, and the map shows the subnet.
pub fn ai_mark(game: &Game) -> Option<AiMark> {
    if game.in_subnet() {
        return None;
    }
    let s = game.sighting()?;
    let ice = game.boss.as_ref()?.ice;
    Some(AiMark {
        node: s.node(),
        now: s.now(),
        held: s.now() && visible(game, s.node()),
        label: game.data.ice[ice].name.clone(),
    })
}

/// The classic status line's mark while the net's AI is up: the OS's process list, short.
pub fn ai_status(game: &Game) -> Option<String> {
    game.hunter()?;
    let (name, _) = game.ai_woke()?;
    Some(format!("▲ {name} UP"))
}

/// The net's AI where the runner knows it holds node `id` now, in sight: the node is the AI's,
/// and the map draws the AI in place of what's there, its name where the node's tag goes. Its
/// name, or `None` when it isn't there, the runner doesn't know it is, or the node's in the fog.
pub fn ai_holds(game: &Game, id: usize) -> Option<String> {
    ai_mark(game).filter(|m| m.holds(id)).map(|m| m.label)
}

/// The short label under a node on the map: its kind's tag, the board's name, a subnet's exit
/// as its uplink, `???` in the fog, and the net's AI's name on a node it holds.
pub fn tag(game: &Game, id: usize) -> String {
    tag_with(game, id, ai_mark(game).as_ref())
}

/// [`tag`] with the frame's [`ai_mark`] already worked out: a map asks it once a frame, not
/// once a node.
pub fn tag_with(game: &Game, id: usize, ai: Option<&AiMark>) -> String {
    if !visible(game, id) {
        return "???".into();
    }
    if let Some(m) = ai.filter(|m| m.holds(id)) {
        return m.label.clone();
    }
    let k = MapNode::of(&game.net, id, &game.data);
    if k == MapNode::Exit && game.in_subnet() {
        return "UPLINK".into();
    }
    if k == MapNode::Bbs {
        return game.bbs().name.clone();
    }
    style(&game.data, k).tag.clone()
}

/// What a map style writes beside the net's AI, in its own drawing's words: the Iso's tag, the
/// Draft's callout in capitals, the Paint card. Last known says so. Where it holds a node in
/// sight there's nothing beside it: the node is drawn as the AI, its name as the tag
/// ([`AiMark::held`]); there now in the fog, its name.
pub fn ai_caption(m: &AiMark, view: MapView) -> Option<String> {
    if m.held {
        return None;
    }
    let l = &m.label;
    Some(match (view, m.now) {
        (MapView::Iso | MapView::Paint, true) => l.clone(),
        (MapView::Draft, true) => l.to_uppercase(),
        (MapView::Iso, false) => format!("{l}  last seen"),
        (MapView::Draft, false) => format!("{} (LAST SEEN)", l.to_uppercase()),
        (MapView::Paint, false) => format!("{l} was last seen here"),
    })
}

/// A link as the map shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkView {
    /// Its shallower end.
    pub from: usize,
    /// Its deeper end.
    pub to: usize,
    /// Walked deeper only, never back: the map draws an arrow pointing deeper.
    pub one_way: bool,
    /// The runner came along it, either way: the hop trail.
    pub walked: bool,
    /// The end the runner would step to along it from where it stands, either way; `None`
    /// when it isn't a route from here.
    pub route: Option<usize>,
}

/// Every link on the network the runner is on, as the map shows it. Its routes are exactly
/// `Game::reachable`.
pub fn links(game: &Game) -> Vec<LinkView> {
    let net = &game.net;
    let reach = game.reachable();
    let route = |a: usize, b: usize| {
        if a == game.at && reach.contains(&b) {
            Some(b)
        } else if b == game.at && reach.contains(&a) {
            Some(a)
        } else {
            None
        }
    };
    net.nodes
        .iter()
        .flat_map(|n| {
            n.links.iter().map(move |&to| LinkView {
                from: n.id,
                to,
                one_way: net.one_way(n.id, to),
                walked: game.travelled(n.id, to),
                route: route(n.id, to),
            })
        })
        .collect()
}

/// The line a route says, where the map's hover and the route list name it: `jack 3  →  VAULT`
/// deeper, `jack 1  ←  ENTRY` back, `, back up` where ICE beaten there wakes, and
/// `, WATCHMAN 1.2 there` where the runner knows the net's AI is; on an ICE node the AI stands in
/// for, its name in the ICE's place (`jack 3  →  WATCHMAN 1.2`), the only fight there. `None`
/// when node `id` isn't a route from here.
pub fn route_tip(game: &Game, id: usize) -> Option<String> {
    if !game.reachable().contains(&id) {
        return None;
    }
    // A route is a layer either side of the runner, so never in the fog.
    let hop = game.route(id).ok()?;
    let way = if hop.back { "←" } else { "→" };
    let up = if hop.rearms { ", back up" } else { "" };
    // The net's AI, where the runner knows it is: the step meets it, and on an ICE node its
    // fight is the only one there, so nothing of the ICE is named, not even its coming back up.
    let (what, up, ai) = match (hop.meets, ai_mark(game)) {
        (true, Some(m)) if game.ice_replaced(id, true) => (m.label, "", String::new()),
        (true, Some(m)) => (
            game.node_label(id).to_string(),
            up,
            format!(", {} there", m.label),
        ),
        _ => (game.node_label(id).to_string(), up, String::new()),
    };
    Some(format!("jack {id}  {way}  {what}{up}{ai}"))
}

/// Paint fills that aren't patterns.
pub const PAINT_PAPER: &str = "paper";
pub const PAINT_SOLID: &str = "solid";

/// Where each node sits on a flat map, as fractions (0..1) of the drawing's width and height:
/// layers left to right, a layer's nodes spread evenly top to bottom.
pub fn flat_layout(net: &Network) -> Vec<(f32, f32)> {
    let layers = net.layers.len();
    net.nodes
        .iter()
        .map(|n| {
            let width = net.layers[n.layer].len().max(1);
            (
                n.layer as f32 / (layers - 1).max(1) as f32,
                (n.lane as f32 + 0.5) / width as f32,
            )
        })
        .collect()
}

impl MapStyleDef {
    pub fn validate(defs: &[MapStyleDef]) -> Result<(), String> {
        for k in MapNode::ALL {
            let n = defs.iter().filter(|d| d.node == k).count();
            if n != 1 {
                return Err(format!(
                    "{k:?} is listed {n} times; it needs exactly one entry"
                ));
            }
        }
        Ok(())
    }
}

/// A node's map style, found by kind. `validate` guarantees there is one.
pub fn style(data: &Data, k: MapNode) -> &MapStyleDef {
    data.netmap
        .iter()
        .find(|d| d.node == k)
        .unwrap_or(&data.netmap[0])
}

/// A wireframe: points, edges between them, and faces (for the see-through fill).
#[derive(Clone, Debug, PartialEq)]
pub struct Mesh {
    pub v: Vec<[f32; 3]>,
    pub e: Vec<[usize; 2]>,
    pub f: Vec<Vec<usize>>,
}

impl Mesh {
    fn from_faces(v: Vec<[f32; 3]>, f: Vec<Vec<usize>>) -> Self {
        let mut e: Vec<[usize; 2]> = Vec::new();
        for face in &f {
            for i in 0..face.len() {
                let (a, b) = (face[i], face[(i + 1) % face.len()]);
                let k = [a.min(b), a.max(b)];
                if !e.contains(&k) {
                    e.push(k);
                }
            }
        }
        Self { v, e, f }
    }

    fn scaled(mut self, s: f32) -> Self {
        for p in &mut self.v {
            *p = p.map(|x| x * s);
        }
        self
    }

    fn merge(mut self, other: Mesh) -> Self {
        let o = self.v.len();
        self.v.extend(other.v);
        self.e.extend(other.e.iter().map(|[a, b]| [a + o, b + o]));
        self.f.extend(
            other
                .f
                .into_iter()
                .map(|f| f.into_iter().map(|i| i + o).collect()),
        );
        self
    }

    /// The mesh for a shape, about one unit across, centred on the origin.
    pub fn of(kind: MeshKind) -> Mesh {
        match kind {
            MeshKind::Cube => cube().scaled(0.42),
            MeshKind::Ring => torus(0.42, 0.14, 16, 8, false),
            MeshKind::Ico => ico().scaled(0.5),
            MeshKind::Spikes => spikes(0.42),
            MeshKind::Drum => drum(0.38, 0.32, 12),
            MeshKind::Octa => octa().scaled(0.55),
            MeshKind::Pyramid => pyramid().scaled(0.42),
            MeshKind::Nested => cube().scaled(0.45).merge(cube().scaled(0.2)),
            MeshKind::Pad => pad(0.55),
            MeshKind::Portal => torus(0.5, 0.07, 20, 6, true),
            MeshKind::Ai => octa().scaled(0.28).merge(torus(0.48, 0.05, 20, 6, false)),
        }
    }
}

fn cube() -> Mesh {
    let v = [
        [-1., -1., -1.],
        [1., -1., -1.],
        [1., 1., -1.],
        [-1., 1., -1.],
        [-1., -1., 1.],
        [1., -1., 1.],
        [1., 1., 1.],
        [-1., 1., 1.],
    ];
    let f = [
        [0, 1, 2, 3],
        [4, 5, 6, 7],
        [0, 1, 5, 4],
        [2, 3, 7, 6],
        [1, 2, 6, 5],
        [0, 3, 7, 4],
    ];
    Mesh::from_faces(v.to_vec(), f.iter().map(|f| f.to_vec()).collect())
}

fn octa() -> Mesh {
    let v = [
        [1., 0., 0.],
        [-1., 0., 0.],
        [0., 1., 0.],
        [0., -1., 0.],
        [0., 0., 1.],
        [0., 0., -1.],
    ];
    let f = [
        [0, 2, 4],
        [2, 1, 4],
        [1, 3, 4],
        [3, 0, 4],
        [2, 0, 5],
        [1, 2, 5],
        [3, 1, 5],
        [0, 3, 5],
    ];
    Mesh::from_faces(v.to_vec(), f.iter().map(|f| f.to_vec()).collect())
}

fn ico() -> Mesh {
    let t = (1.0 + 5f32.sqrt()) / 2.0;
    let k = 1.0 / (1.0 + t * t).sqrt();
    let v: Vec<[f32; 3]> = [
        [-1., t, 0.],
        [1., t, 0.],
        [-1., -t, 0.],
        [1., -t, 0.],
        [0., -1., t],
        [0., 1., t],
        [0., -1., -t],
        [0., 1., -t],
        [t, 0., -1.],
        [t, 0., 1.],
        [-t, 0., -1.],
        [-t, 0., 1.],
    ]
    .iter()
    .map(|p| p.map(|x| x * k))
    .collect();
    let f = [
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ];
    Mesh::from_faces(v, f.iter().map(|f| f.to_vec()).collect())
}

/// An icosahedron with a spike out of every face.
fn spikes(s: f32) -> Mesh {
    let base = ico().scaled(s * 0.7);
    let mut v = base.v.clone();
    let mut f = Vec::new();
    for face in &base.f {
        let (a, b, c) = (face[0], face[1], face[2]);
        let m: [f32; 3] =
            std::array::from_fn(|i| (base.v[a][i] + base.v[b][i] + base.v[c][i]) / 3.0);
        let len = (m[0] * m[0] + m[1] * m[1] + m[2] * m[2]).sqrt();
        v.push(m.map(|x| x / len * s * 1.45));
        let tip = v.len() - 1;
        f.extend([vec![a, b, tip], vec![b, c, tip], vec![c, a, tip]]);
    }
    Mesh::from_faces(v, f)
}

fn pyramid() -> Mesh {
    let v = vec![
        [-1., -0.8, -1.],
        [1., -0.8, -1.],
        [1., -0.8, 1.],
        [-1., -0.8, 1.],
        [0., 1.1, 0.],
    ];
    let f = vec![
        vec![0, 1, 2, 3],
        vec![0, 1, 4],
        vec![1, 2, 4],
        vec![2, 3, 4],
        vec![3, 0, 4],
    ];
    Mesh::from_faces(v, f)
}

fn torus(big: f32, small: f32, n: usize, m: usize, upright: bool) -> Mesh {
    let tau = std::f32::consts::TAU;
    let mut v = Vec::new();
    for i in 0..n {
        for j in 0..m {
            let (u, w) = (i as f32 / n as f32 * tau, j as f32 / m as f32 * tau);
            let r = big + small * w.cos();
            let (x, z, y) = (r * u.cos(), r * u.sin(), small * w.sin());
            v.push(if upright { [x, z, y] } else { [x, y, z] });
        }
    }
    let mut f = Vec::new();
    for i in 0..n {
        for j in 0..m {
            let (i2, j2) = ((i + 1) % n, (j + 1) % m);
            f.push(vec![i * m + j, i2 * m + j, i2 * m + j2, i * m + j2]);
        }
    }
    Mesh::from_faces(v, f)
}

fn drum(r: f32, h: f32, n: usize) -> Mesh {
    let tau = std::f32::consts::TAU;
    let mut v = Vec::new();
    let (mut bottom, mut top, mut f) = (Vec::new(), Vec::new(), Vec::new());
    for i in 0..n {
        let a = i as f32 / n as f32 * tau;
        v.push([r * a.cos(), -h, r * a.sin()]);
        v.push([r * a.cos(), h, r * a.sin()]);
        let j = (i + 1) % n;
        f.push(vec![2 * i, 2 * j, 2 * j + 1, 2 * i + 1]);
        bottom.push(2 * i);
        top.push(2 * i + 1);
    }
    f.push(bottom);
    f.push(top);
    Mesh::from_faces(v, f)
}

fn pad(s: f32) -> Mesh {
    let mut v = Vec::new();
    let mut f = Vec::new();
    for k in [1.0, 0.6] {
        let o = v.len();
        let d = s * k;
        v.extend([[-d, 0., -d], [d, 0., -d], [d, 0., d], [-d, 0., d]]);
        f.push(vec![o, o + 1, o + 2, o + 3]);
    }
    Mesh::from_faces(v, f)
}

/// Spacing between layers and lanes, in world units.
pub const SPACING: f32 = 2.2;
/// How high nodes float over the grid.
pub const HOVER: f32 = 0.9;

/// Where node `id` sits on the grid (y is up; the node floats `HOVER` above it).
pub fn world(net: &Network, id: usize) -> [f32; 3] {
    let n = &net.nodes[id];
    let width = net.layers[n.layer].len() as f32;
    [
        n.layer as f32 * SPACING,
        0.0,
        (n.lane as f32 - (width - 1.0) / 2.0) * SPACING * 1.05,
    ]
}

/// The middle of the network on the grid: what the camera looks at.
pub fn centre(net: &Network) -> [f32; 3] {
    let last = net.layers.len().saturating_sub(1) as f32;
    [last * SPACING / 2.0, 0.0, 0.0]
}

/// A true isometric camera: 45° around, ~35° down. `project` gives screen x, y (y down)
/// and a depth (bigger is further away).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Iso {
    pub focus: [f32; 3],
    /// Pixels per world unit.
    pub scale: f32,
    /// Screen point the focus lands on.
    pub origin: [f32; 2],
}

impl Iso {
    pub fn project(&self, p: [f32; 3]) -> [f32; 3] {
        let (yaw, pitch) = (-std::f32::consts::FRAC_PI_4, (1.0 / 2f32.sqrt()).atan());
        let (x, y, z) = (
            p[0] - self.focus[0],
            p[1] - self.focus[1],
            p[2] - self.focus[2],
        );
        let cx = x * yaw.cos() - z * yaw.sin();
        let cz = x * yaw.sin() + z * yaw.cos();
        let sy = y * pitch.cos() - cz * pitch.sin();
        let depth = y * pitch.sin() + cz * pitch.cos();
        [
            self.origin[0] + cx * self.scale,
            self.origin[1] - sy * self.scale,
            -depth,
        ]
    }

    /// A camera that fits the whole network into a `w` x `h` view, with room for labels.
    pub fn fit(net: &Network, w: f32, h: f32, origin: [f32; 2]) -> Iso {
        let unit = Iso {
            focus: centre(net),
            scale: 1.0,
            origin: [0.0, 0.0],
        };
        let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
        for id in 0..net.nodes.len() {
            let base = world(net, id);
            for p in [base, [base[0], HOVER + 0.6, base[2]]] {
                let s = unit.project(p);
                for i in 0..2 {
                    lo[i] = lo[i].min(s[i]);
                    hi[i] = hi[i].max(s[i]);
                }
            }
        }
        let (bw, bh) = ((hi[0] - lo[0]).max(1.0), (hi[1] - lo[1]).max(1.0));
        let scale = ((w * 0.84) / bw).min((h * 0.78) / bh).clamp(20.0, 90.0);
        let mid = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
        Iso {
            focus: unit.focus,
            scale,
            origin: [origin[0] - mid[0] * scale, origin[1] - mid[1] * scale],
        }
    }
}

/// Turns a point about the vertical axis, then tips it forward.
pub fn spin(p: [f32; 3], yaw: f32, tilt: f32) -> [f32; 3] {
    let (x, y, z) = (
        p[0],
        p[1] * tilt.cos() - p[2] * tilt.sin(),
        p[1] * tilt.sin() + p[2] * tilt.cos(),
    );
    [
        x * yaw.cos() - z * yaw.sin(),
        y,
        x * yaw.sin() + z * yaw.cos(),
    ]
}
