//! 141.3.1 acceptance: ICE run their own programs, through the real save and run
//! (`Save::jack_in`, `Game::apply`), with NEXT a dry run the deck's program reads live.

use ninsei_core::data::Intent;
use ninsei_core::game::{ExecState, LogKind, StepWhat};
use ninsei_core::hub::Save;
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

/// A run from a fresh save, standing in a fight with `ice` too strong to fall.
fn fight_with(ice: &str, program: &str) -> Game {
    let d = data();
    let mut s = Save::default();
    s.ensure_key(0x1413);
    s.ensure_phonebook(&d, 0);
    s.ensure_hardware(&d);
    let mut g = s.jack_in(&d, 11);
    let target = g.net.nodes[g.at].links[0];
    let ix = g.data.ice.iter().position(|i| i.id == ice).unwrap();
    g.net.nodes[target].kind = NodeKind::Ice;
    g.net.nodes[target].ice = Some(ix);
    g.net.nodes[target].cleared = false;
    g.apply(Action::Move(target));
    assert_eq!(g.phase, Phase::Combat);
    let c = g.combat.as_mut().unwrap();
    (c.strength, c.strength_max) = (500, 500);
    g.apply(Action::Program(program.into()));
    g
}

fn ran(g: &Game, name: &str) -> usize {
    g.exec
        .trace
        .iter()
        .filter(|s| match s.what {
            StepWhat::Ran { uid, .. } => g
                .hdd
                .iter()
                .any(|p| p.uid == uid && p.spec.name.eq_ignore_ascii_case(name)),
            _ => false,
        })
        .count()
}

#[test]
fn every_ice_is_a_program_with_its_own_budgets() {
    let d = data();
    assert_eq!(d.ice.len(), 9);
    for ice in &d.ice {
        let p = basic::parse_with(&ice.program, basic::Vocab::Ice(&ice.verbs));
        assert!(p.can_act(), "{}", ice.id);
        assert!(
            ice.cpu >= 1 && ice.ram >= 1 && !ice.verbs.is_empty(),
            "{}",
            ice.id
        );
        // Its program reads the deck: every ICE on the map has at least one IF.
        if ice.tier > 0 {
            assert!(
                p.lines.iter().any(|l| matches!(l.stmt, basic::Stmt::If(_))),
                "{} reads nothing",
                ice.id
            );
        }
    }
}

#[test]
fn a_program_that_firewalls_on_next_attack_sees_a_shield_reader_change_its_mind() {
    // Two guarded firewalls: Enigma reads the shield, so after the first one its NEXT is a
    // TRACE and the second guard is false. The deck's program sees the flip mid-pass.
    let mut g = fight_with(
        "enigma",
        "10 IF NEXT ATTACK\n20 RUN firewall\n30 IF NEXT ATTACK\n40 RUN firewall\n50 STOP",
    );
    assert!(matches!(g.ice_next(), Some(Intent::Attack(_))));
    g.apply(Action::Finish);
    assert_eq!(g.exec.state, ExecState::Stopped(50), "{:?}", g.exec.trace);
    assert_eq!(ran(&g, "firewall"), 1, "{:?}", g.exec.trace);
    assert!(matches!(g.ice_next(), Some(Intent::Trace(_))));
    // And it does what NEXT said.
    let (trace, integ) = (g.deck.trace, g.deck.integrity);
    g.apply(Action::EndTurn);
    assert!(g.deck.trace > trace);
    assert_eq!(g.deck.integrity, integ);
}

#[test]
fn the_ice_turn_is_said_in_the_log_by_name() {
    let mut g = fight_with("rototurret", "");
    let before = g.log.len();
    g.apply(Action::EndTurn);
    let said: Vec<&str> = g.log[before..].iter().map(|l| l.text.as_str()).collect();
    assert!(
        said.iter()
            .any(|t| t.starts_with("Rototurret ") && t.contains("hits for")),
        "{said:?}"
    );
    assert!(
        g.log[before..]
            .iter()
            .any(|l| l.kind == LogKind::Bad && l.text.contains("hits for"))
    );
}

#[test]
fn same_seed_same_fights_with_ice_that_read_you() {
    let d = data();
    for seed in [3, 17, 40] {
        let run = || {
            let (g, log) = probe::play(Game::new(d.clone(), seed), probe::Bot::SafePick);
            let text: Vec<String> = g.log.iter().map(|l| l.text.clone()).collect();
            (text, log.turns, g.deck.integrity, g.deck.trace)
        };
        assert_eq!(run(), run(), "seed {seed}");
    }
}
