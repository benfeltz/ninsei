//! 138.2: what the runner sees of the net's AI: the map's mark, the preview's words, and the
//! wake as each deck says it.

use ninsei_core::boss::Hunter;
use ninsei_core::game::BossState;
use ninsei_core::hub::{Entry, Save};
use ninsei_core::netview::{ai_mark, route_tip};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

fn save(tier: u32) -> Save {
    let mut s = Save::default();
    s.ensure_key(0x1382);
    s.ensure_hardware(&data());
    s.phonebook.push(Entry {
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
    s
}

/// A run with nothing on the net but the AI: every node a looted vault.
fn quiet_run(tier: u32, seed: u64) -> Game {
    let d = data();
    let mut g = save(tier).dial(&d, 0, seed).unwrap();
    for n in &mut g.net.nodes {
        if !matches!(n.kind, NodeKind::Entry | NodeKind::Exit) {
            n.kind = NodeKind::Vault;
            n.cleared = true;
            n.ice = None;
        }
    }
    g
}

/// The AI awake on `at` running `program`.
fn awake_at(g: &mut Game, at: usize, program: &str) {
    let ice = g.boss.as_ref().unwrap().ice;
    let mut h = Hunter::wake(ice, &g.data.ice[ice], at, g.at);
    h.prog = basic::parse_with(program, g.data.ice[ice].vocab());
    g.boss.as_mut().unwrap().state = BossState::Awake(h);
}

fn deeper(g: &Game) -> usize {
    g.net.nodes[g.at].links[0]
}

#[test]
fn the_map_marks_it_only_from_what_the_runner_knows() {
    let mut g = quiet_run(1, 2);
    assert_eq!(ai_mark(&g), None, "asleep: nothing to mark");
    let next = deeper(&g);
    awake_at(&mut g, next, "10 HOLD");
    let m = ai_mark(&g).unwrap();
    assert_eq!((m.node, m.now), (next, true), "a hop away: in sight");
    assert_eq!(m.label, g.data.ice[g.boss.as_ref().unwrap().ice].name);
    // Seen there once, then it goes out of sight: the map keeps where it was last known.
    g.boss.as_mut().unwrap().known = Some(next);
    if let BossState::Awake(h) = &mut g.boss.as_mut().unwrap().state {
        h.at = ninsei_core::boss::exit(&g.net).unwrap();
    }
    let m = ai_mark(&g).unwrap();
    assert_eq!((m.node, m.now), (next, false));
    // Down: nothing.
    g.boss.as_mut().unwrap().state = BossState::Down;
    assert_eq!(ai_mark(&g), None);
}

#[test]
fn a_heard_scan_marks_where_it_scanned_from() {
    let mut g = quiet_run(1, 4);
    let exit = ninsei_core::boss::exit(&g.net).unwrap();
    awake_at(&mut g, exit, "10 SCAN 3");
    let mut found = false;
    for _ in 0..8 {
        if g.phase != Phase::Exploring {
            break;
        }
        g.apply(Action::Move(deeper(&g)));
        if let Some(m) = ai_mark(&g)
            && m.now
            && m.node == exit
        {
            found = true;
            break;
        }
    }
    assert!(
        found,
        "its scan finds them on the way to it, and they hear it"
    );
}

#[test]
fn inside_a_subnet_the_map_marks_nothing() {
    let d = data();
    for seed in 0..10 {
        let mut g = save(2).dial(&d, 0, seed).unwrap();
        let Some(sub) = g.net.nodes.iter().position(|n| n.kind == NodeKind::Subnet) else {
            continue;
        };
        let before = *g.net.layers[g.net.nodes[sub].layer - 1]
            .iter()
            .find(|&&n| g.net.nodes[n].links.contains(&sub))
            .unwrap();
        g.at = before;
        awake_at(&mut g, before, "10 HOLD");
        g.boss.as_mut().unwrap().known = Some(before);
        g.apply(Action::Move(sub));
        if g.in_subnet() {
            assert_eq!(ai_mark(&g), None, "seed {seed}");
            return;
        }
    }
}

#[test]
fn the_preview_names_it_only_where_the_runner_knows_it_is() {
    let mut g = quiet_run(1, 5);
    let next = deeper(&g);
    let name = {
        let ice = g.boss.as_ref().unwrap().ice;
        g.data.ice[ice].name.clone()
    };
    assert!(!route_tip(&g, next).unwrap().contains(&name));
    awake_at(&mut g, next, "10 HOLD");
    let tip = route_tip(&g, next).unwrap();
    assert!(tip.ends_with(&format!(", {name} there")), "{tip}");
    // Elsewhere, nothing.
    for &other in g.reachable().iter().filter(|&&n| n != next) {
        assert!(!route_tip(&g, other).unwrap().contains(&name));
    }
    // Two hops off and unheard: it may be anywhere, so the preview can't say.
    let beyond = g.net.nodes[next].links[0];
    awake_at(&mut g, beyond, "10 HOLD");
    g.boss.as_mut().unwrap().known = None;
    assert!(!route_tip(&g, next).unwrap().contains(&name));
}
