//! 138.2: what the runner sees of the net's AI: the map's mark, the preview's words, and the
//! wake as each deck says it.

use ninsei_core::boss::Hunter;
use ninsei_core::game::BossState;
use ninsei_core::hub::{Entry, Save};
use ninsei_core::netview::{self, ai_mark, route_tip};
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

/// A run walked deeper, hop by hop, until the AI wakes. Returns the cues and log of that hop.
fn to_the_wake(g: &mut Game) -> (Vec<cue::Cue>, Vec<String>) {
    let hop = g.route(deeper(g)).unwrap().trace;
    g.deck.trace = g.boss.as_ref().unwrap().wake - hop;
    g.cues.clear();
    let said = g.log.len();
    g.apply(Action::Move(deeper(g)));
    assert!(g.ai_woke().is_some());
    let log = g.log[said..].iter().map(|l| l.text.clone()).collect();
    (std::mem::take(&mut g.cues), log)
}

#[test]
fn the_wake_is_said_and_sounded_once() {
    let mut g = quiet_run(1, 6);
    let name = g.data.ice[g.boss.as_ref().unwrap().ice].name.clone();
    let (cues, log) = to_the_wake(&mut g);
    assert_eq!(cues.iter().filter(|c| **c == cue::Cue::AiWake).count(), 1);
    let line = format!("!! {name} is up on the net.");
    assert_eq!(log.iter().filter(|l| **l == line).count(), 1, "{log:?}");
    assert_eq!(g.boss.as_ref().unwrap().woke_at, Some(g.hops));
    // Not again.
    for _ in 0..3 {
        if g.phase != Phase::Exploring {
            break;
        }
        g.cues.clear();
        g.apply(Action::Move(deeper(&g)));
        assert!(!g.cues.contains(&cue::Cue::AiWake));
    }
    assert_eq!(g.log.iter().filter(|l| l.text == line).count(), 1);
}

#[test]
fn every_deck_says_it_in_its_own_words_and_they_fit() {
    let mut g = quiet_run(3, 6);
    assert!(suite::ai_notice(&g).is_none() && stack_fight::ai_alert(&g).is_none());
    assert!(btop_fight::ai_banner(&g).is_none());
    let mut watch = suite::Watch::default();
    watch.step(&g);
    to_the_wake(&mut g);
    let notices = watch.step(&g);
    assert_eq!(notices.len(), 1, "{notices:?}");
    assert_eq!(notices[0].title, "SECURITY NOTICE");
    assert!(watch.step(&g).is_empty(), "filed once");
    let (name, company) = g.ai_woke().unwrap();
    assert!(name.starts_with("WARDEN") && company == "Hosaka");
    for l in &notices[0].lines {
        assert!(l.chars().count() <= 60, "{l}");
    }
    let card = stack_fight::ai_alert(&g).unwrap();
    assert!(card.iter().all(|l| l.chars().count() <= 48), "{card:?}");
    assert!(
        card[0].contains(name) && card[1].contains(company),
        "{card:?}"
    );
    let bar = btop_fight::ai_banner(&g).unwrap();
    assert!(bar.contains(name) && bar.chars().count() <= 60, "{bar}");
    // Nobody coaches: no keys, no advice.
    let all: Vec<String> = notices[0]
        .lines
        .iter()
        .cloned()
        .chain(stack_fight::ai_alert(&g).unwrap())
        .chain([bar])
        .collect();
    for l in all {
        let low = l.to_ascii_lowercase();
        for w in ["press", "route", "avoid", "click", "esc", "run away"] {
            assert!(!low.contains(w), "{l}");
        }
    }
}

#[test]
fn the_wake_is_one_moment_every_deck_reads() {
    let mut g = quiet_run(1, 7);
    assert!(!g.ai_woke_this_hop());
    to_the_wake(&mut g);
    assert!(g.ai_woke_this_hop(), "the hop it woke on");
    let mut watch = stack_fight::AlertWatch::default();
    assert!(watch.step(&g).is_some());
    assert!(watch.step(&g).is_none(), "once a run");
    if g.phase == Phase::Exploring {
        g.apply(Action::Move(deeper(&g)));
        assert!(!g.ai_woke_this_hop(), "a hop on, it's past");
    }
    // The same seed again, a new run: a new watch gives it its card again.
    let mut again = quiet_run(1, 7);
    to_the_wake(&mut again);
    assert!(stack_fight::AlertWatch::default().step(&again).is_some());
}

#[test]
fn the_classic_status_marks_it_while_it_is_up() {
    let mut g = quiet_run(1, 8);
    assert_eq!(netview::ai_status(&g), None);
    to_the_wake(&mut g);
    let name = g.ai_woke().unwrap().0.to_string();
    assert_eq!(netview::ai_status(&g), Some(format!("▲ {name} UP")));
    g.boss.as_mut().unwrap().state = BossState::Down;
    assert_eq!(netview::ai_status(&g), None, "down: not up");
    assert_eq!(btop_fight::ai_banner(&g), None);
}
