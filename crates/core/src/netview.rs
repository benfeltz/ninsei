//! The network map's geometry, kept out of the app so it's tested headlessly: which shape each
//! node gets (`data/netmap.ron`), the wireframe meshes, and the isometric projection.

use serde::{Deserialize, Serialize};

use crate::data::{Data, IceClass};
use crate::network::{Network, NodeKind};

/// Which way the network map is drawn. Saved with the hub.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MapView {
    /// Floating wireframe shapes over a glowing grid.
    #[default]
    Iso,
    /// A flat plan, drawn like a technical drawing.
    Draft,
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
