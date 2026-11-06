//! The network map's geometry: shapes per node kind (data), meshes, and the isometric camera.

use ninsei_core::hub::Save;
use ninsei_core::netview::{self, Iso, MapNode, MapStyleDef, MapView, Mesh, MeshKind};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

#[test]
fn every_node_kind_has_a_style() {
    let d = data();
    for k in MapNode::ALL {
        assert_eq!(netview::style(&d, k).node, k);
    }
    let src = include_str!("../../../data/netmap.ron");
    let parse = |netmap: &str| {
        Data::parse(Sources {
            netmap,
            ..Sources::builtin()
        })
    };
    let missing: String = src
        .lines()
        .filter(|l| !l.contains("(node: Sentry,"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(parse(&missing).unwrap_err().contains("Sentry"));
    let twice = src.replace("(node: Sentry,", "(node: Barrier,");
    assert!(parse(&twice).unwrap_err().contains("Barrier"));
    assert!(MapStyleDef::validate(&d.netmap).is_ok());
}

#[test]
fn ice_shows_by_class() {
    let d = data();
    for seed in 0..40 {
        let g = Game::new(d.clone(), seed);
        for n in &g.net.nodes {
            let k = MapNode::of(&g.net, n.id, &d);
            if n.kind == NodeKind::Ice {
                let class = d.ice[n.ice.unwrap()].class;
                let want = match class {
                    IceClass::Barrier => MapNode::Barrier,
                    IceClass::CodeGate => MapNode::CodeGate,
                    IceClass::Sentry => MapNode::Sentry,
                };
                assert_eq!(k, want);
            }
        }
    }
}

#[test]
fn meshes_hold_together() {
    for kind in [
        MeshKind::Cube,
        MeshKind::Ring,
        MeshKind::Ico,
        MeshKind::Spikes,
        MeshKind::Drum,
        MeshKind::Octa,
        MeshKind::Pyramid,
        MeshKind::Nested,
        MeshKind::Pad,
        MeshKind::Portal,
    ] {
        let m = Mesh::of(kind);
        assert!(
            !m.v.is_empty() && !m.e.is_empty() && !m.f.is_empty(),
            "{kind:?}"
        );
        for [a, b] in &m.e {
            assert!(a < b && *b < m.v.len(), "{kind:?} edge {a}-{b}");
        }
        let mut edges = m.e.clone();
        edges.sort_unstable();
        edges.dedup();
        assert_eq!(edges.len(), m.e.len(), "{kind:?} has a doubled edge");
        assert!(m.f.iter().flatten().all(|&i| i < m.v.len()), "{kind:?}");
        // About a unit across, so every shape sits in its grid square.
        let r =
            m.v.iter()
                .map(|p| (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt())
                .fold(0.0, f32::max);
        assert!((0.3..=0.8).contains(&r), "{kind:?} radius {r}");
    }
}

#[test]
fn the_camera_is_isometric() {
    let cam = Iso {
        focus: [0.0; 3],
        scale: 10.0,
        origin: [100.0, 100.0],
    };
    let o = cam.project([0.0; 3]);
    let len = |p: [f32; 3]| ((p[0] - o[0]).powi(2) + (p[1] - o[1]).powi(2)).sqrt();
    let (x, y, z) = (
        cam.project([1.0, 0.0, 0.0]),
        cam.project([0.0, 1.0, 0.0]),
        cam.project([0.0, 0.0, 1.0]),
    );
    // All three axes come out the same length on screen.
    assert!((len(x) - len(z)).abs() < 1e-3 && (len(x) - len(y)).abs() < 1e-3);
    assert!(y[1] < o[1], "up is up");
    assert_eq!([o[0], o[1]], [100.0, 100.0]);
    // Of two points on the same screen spot, the higher one is nearer.
    assert!(cam.project([0.0, 1.0, 0.0])[2] < o[2]);
}

#[test]
fn the_whole_network_fits_the_view() {
    let d = data();
    let (w, h) = (900.0, 480.0);
    for seed in 0..60 {
        for tier in 1..=3 {
            let mut s = Save::default();
            s.phonebook.push(hub::Entry {
                id: 0,
                name: "probe".into(),
                business: "test".into(),
                number: "555-0000".into(),
                tier,
                traits: Vec::new(),
                heat: 0,
                runs: 0,
                last: Vec::new(),
            });
            let g = s.clone().dial(&d, 0, seed).unwrap();
            let cam = Iso::fit(&g.net, w, h, [w / 2.0, h / 2.0]);
            for id in 0..g.net.nodes.len() {
                let p = cam.project(netview::world(&g.net, id));
                assert!(
                    (0.0..=w).contains(&p[0]) && (0.0..=h).contains(&p[1]),
                    "seed {seed} tier {tier} node {id} at {p:?}"
                );
            }
        }
    }
}

#[test]
fn the_deck_picks_the_map() {
    let d = data();
    let mut s = Save::default();
    s.ensure_hardware(&d);
    // The starter Sense/Net paints a 1-bit picture.
    assert_eq!(s.deck, "sensenet");
    assert_eq!(s.firmware_def(&d).map, MapView::Paint);
    s.decks.push("hosaka".into());
    s.use_deck(&d, "hosaka").unwrap();
    assert_eq!(
        s.firmware_def(&d).map,
        MapView::Draft,
        "the Hosaka sees a plan"
    );
    s.decks.push("onosendai".into());
    s.use_deck(&d, "onosendai").unwrap();
    assert_eq!(
        s.firmware_def(&d).map,
        MapView::Iso,
        "the Ono-Sendai sees wireframes"
    );
    assert!(d.theme("synthwave").is_some());
}

#[test]
fn every_node_kind_paints_with_a_known_fill() {
    let d = data();
    for k in MapNode::ALL {
        let p = &netview::style(&d, k).paint;
        assert!(
            p == netview::PAINT_PAPER || p == netview::PAINT_SOLID || d.pattern(p).is_some(),
            "{k:?} paints with {p}"
        );
    }
    // ICE classes are told apart by their fill.
    let fill = |k| netview::style(&d, k).paint.clone();
    let ice = [
        MapNode::Barrier,
        MapNode::CodeGate,
        MapNode::Sentry,
        MapNode::BlackIce,
    ]
    .map(fill);
    for (i, a) in ice.iter().enumerate() {
        assert!(!ice[..i].contains(a), "two ICE kinds paint with {a}");
    }
    let src = include_str!("../../../data/netmap.ron");
    let err = Data::parse(data::Sources {
        netmap: &src.replace("paint: \"bricks\"", "paint: \"plaid\""),
        ..data::Sources::builtin()
    })
    .unwrap_err();
    assert!(err.contains("unknown pattern `plaid`"), "{err}");
}

#[test]
fn the_flat_layout_runs_left_to_right_inside_the_picture() {
    let d = data();
    for seed in 0..20 {
        let g = Game::new(d.clone(), seed);
        let net = &g.net;
        let spots = netview::flat_layout(net);
        assert_eq!(spots.len(), net.nodes.len());
        for (n, &(x, y)) in net.nodes.iter().zip(&spots) {
            assert!((0.0..=1.0).contains(&x) && (0.0..1.0).contains(&y));
            for &to in &n.links {
                assert!(spots[to].0 > x, "links run left to right");
            }
        }
        // A layer's nodes never share a spot.
        for layer in &net.layers {
            for (i, &a) in layer.iter().enumerate() {
                assert!(layer[..i].iter().all(|&b| spots[b] != spots[a]));
            }
        }
    }
}
