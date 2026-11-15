//! 141.3.3 acceptance: the builtin ICE that attack the program, met through the real save and
//! run, and the sniffer bought on Dead Sector answering the spoof.

use ninsei_core::fight::NextMove;
use ninsei_core::hub::Save;
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

/// A run from a fresh save, in a fight with `ice` on black ICE ground, strong enough
/// to last, with the deck's `program`.
fn fight_with(ice: &str, program: &str) -> Game {
    let d = data();
    let mut s = Save::default();
    s.ensure_key(0x3133);
    s.ensure_phonebook(&d, 0);
    s.ensure_hardware(&d);
    let mut g = s.jack_in(&d, 11);
    let target = g.net.nodes[g.at].links[0];
    let ix = g.data.ice.iter().position(|i| i.id == ice).unwrap();
    g.net.nodes[target].kind = NodeKind::BlackIce;
    g.net.nodes[target].ice = Some(ix);
    g.net.nodes[target].cleared = false;
    g.apply(Action::Move(target));
    let c = g.combat.as_mut().unwrap();
    (c.strength, c.strength_max) = (500, 500);
    g.apply(Action::Program(program.into()));
    g
}

#[test]
fn archer_hides_its_next_from_a_deck_behind_a_firewall() {
    let mut g = fight_with("archer", "");
    let fw = g.slot_named("firewall").unwrap();
    g.apply(Action::Run(fw));
    assert!(matches!(
        g.next_move(),
        Some(NextMove::Act(data::Intent::Spoof(2)))
    ));
    g.apply(Action::EndTurn);
    assert_eq!(g.next_move(), Some(NextMove::Spoofed));
    assert_eq!(
        suite::watches(&g)
            .into_iter()
            .find(|(k, _)| *k == "NEXT")
            .unwrap()
            .1,
        "???"
    );
}

#[test]
fn hydra_hides_its_next_the_turn_the_safe_program_raises_a_firewall() {
    // 138.1: SEGFAULT is a net's AI's verb. Hydra answers a firewall with a SPOOF instead, and
    // the deck's program comes out of the turn as it went in.
    let d = data();
    let safe = d.starts.program("").to_string();
    let mut g = fight_with("hydra", &safe);
    g.deck.integrity = 15;
    let fw = g.slot_named("firewall").unwrap();
    g.apply(Action::Run(fw));
    g.apply(Action::Finish);
    assert_eq!(g.next_move(), Some(NextMove::Spoofed));
    assert_eq!(g.program, basic::parse(&safe));
    assert!(
        !d.ice
            .iter()
            .any(|i| !i.boss && i.verbs.contains(&basic::Verb::Segfault))
    );
}

#[test]
fn a_nets_ai_garbles_the_line_that_ran_the_patch() {
    // The tier-1 AI reads a patch and garbles the line that ran it; the player retypes it.
    let d = data();
    let ix = d.boss_for(1, 0).unwrap();
    let id = d.ice[ix].id.clone();
    let program = "10 RUN patch\n20 RUN icepick";
    let mut g = fight_with(&id, program);
    g.deck.integrity = 10;
    g.apply(Action::Finish);
    let original = basic::parse(program);
    assert!(
        g.program
            .fault(10)
            .is_some_and(|f| f.starts_with("?SYNTAX ERROR IN")),
        "{}",
        g.program.text()
    );
    let text = original.line(10).unwrap().text.clone();
    g.apply(listing::retype(&g, 10, &text).unwrap());
    assert_eq!(g.program, original);
}

#[test]
fn the_sniffer_on_dead_sector_answers_a_spoof() {
    let d = data();
    let board = d.boards.iter().find(|b| b.id == "deadsector").unwrap();
    let file = board
        .areas
        .iter()
        .flat_map(|a| &a.files)
        .find(|f| f.item == data::Drop::Warez("sniffer".into()))
        .expect("Dead Sector stocks it");
    assert_eq!(file.file, "SNIF-RW.ZIP");
}
