//! 141.3.2: the deck shows the ICE's program as it has seen it run, in a fight and in the
//! ICE DB.

use ninsei_core::hub::Save;
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

fn fresh() -> Save {
    let d = data();
    let mut s = Save::default();
    s.ensure_key(0x1432);
    s.ensure_phonebook(&d, 0);
    s.ensure_hardware(&d);
    s
}

/// A run from `save`, in a fight with `ice` too strong to fall.
fn fight_from(save: &mut Save, ice: &str) -> Game {
    let d = data();
    let mut g = save.jack_in(&d, 11);
    let target = g.net.nodes[g.at].links[0];
    let ix = g.data.ice.iter().position(|i| i.id == ice).unwrap();
    g.net.nodes[target].kind = NodeKind::Ice;
    g.net.nodes[target].ice = Some(ix);
    g.net.nodes[target].cleared = false;
    g.apply(Action::Move(target));
    assert_eq!(g.phase, Phase::Combat);
    let c = g.combat.as_mut().unwrap();
    (c.strength, c.strength_max) = (500, 500);
    g.apply(Action::Program(String::new()));
    g
}

fn ix(g: &Game, id: &str) -> usize {
    g.data.ice.iter().position(|i| i.id == id).unwrap()
}

#[test]
fn the_deck_remembers_the_lines_it_saw_the_ice_run() {
    let mut s = fresh();
    let mut g = fight_from(&mut s, "enigma");
    let e = ix(&g, "enigma");
    assert!(g.ice_seen_lines(e).is_empty(), "nothing seen yet");
    // No shield: line 10's IF is false, it skips 20 and attacks at 30.
    g.apply(Action::EndTurn);
    assert_eq!(g.ice_seen_lines(e), [10, 30]);
    g.apply(Action::EndTurn);
    assert_eq!(g.ice_seen_lines(e), [10, 30, 40], "sorted, each once");
}

#[test]
fn what_it_saw_comes_home_and_back_into_the_next_run() {
    let d = data();
    let mut s = fresh();
    let mut g = fight_from(&mut s, "enigma");
    g.apply(Action::EndTurn);
    // A flatline still brings it home.
    g.deck.integrity = 1;
    while g.phase == Phase::Combat {
        g.apply(Action::EndTurn);
    }
    assert_eq!(g.phase, Phase::Flatlined);
    s.record(&g, &[], &d.rig).unwrap();
    let seen = s.ice_seen.iter().find(|x| x.id == "enigma").unwrap().clone();
    assert!(seen.lines.contains(&10) && seen.lines.contains(&30), "{seen:?}");
    let kit = s.kit(&d);
    assert!(kit.ice_known.iter().any(|(id, l)| id == "enigma" && *l == seen.lines));
    let g = fight_from(&mut s, "enigma");
    assert_eq!(g.ice_seen_lines(ix(&g, "enigma")), seen.lines.as_slice());
}

#[test]
fn a_save_from_before_lines_loads() {
    let mut s = fresh();
    s.ice_seen.push(hub::IceSeen {
        id: "enigma".into(),
        met: 1,
        broken: 0,
        lo: 12,
        hi: 12,
        lines: vec![10],
    });
    let old = s.to_text().replace("lines: [\n                10,\n            ],", "");
    assert!(!old.contains("lines: ["), "the field is gone from the text: {old}");
    let back = Save::load(&old).expect("loads");
    assert_eq!(back.ice_seen[0].lines, Vec::<u32>::new());
}
