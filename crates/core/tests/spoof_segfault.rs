//! 141.3.3: ICE verbs that attack the deck's program, SPOOF and SEGFAULT, and the sniffer.

use ninsei_core::Data;
use ninsei_core::basic::{self, Stmt, Verb, Vocab};
use ninsei_core::data::Intent;

const BOTH: [Verb; 3] = [Verb::Attack, Verb::Spoof, Verb::Segfault];

#[test]
fn spoof_and_segfault_are_an_ices_words() {
    let p = basic::parse_with("10 SPOOF 2\n20 SEGFAULT\n30 ATTACK 1", Vocab::Ice(&BOTH));
    let stmts: Vec<&Stmt> = p.lines.iter().map(|l| &l.stmt).collect();
    assert_eq!(
        stmts,
        [
            &Stmt::Act(Intent::Spoof(2)),
            &Stmt::Act(Intent::Segfault),
            &Stmt::Act(Intent::Attack(1))
        ]
    );
    assert_eq!(p.text(), "10 SPOOF 2\n20 SEGFAULT\n30 ATTACK 1");
    let bad = basic::parse_with("10 SEGFAULT 3\n20 SPOOF", Vocab::Ice(&BOTH));
    assert!(
        bad.fault(10).is_some() && bad.fault(20).is_some(),
        "{bad:?}"
    );
    let deck = basic::parse("10 SPOOF 2\n20 SEGFAULT");
    assert!(deck.fault(10).is_some() && deck.fault(20).is_some());
    assert_eq!(Intent::Spoof(2).describe(), "SPOOF 2");
    assert_eq!(Intent::Segfault.describe(), "SEGFAULT");
}

/// The builtin ICE with one more row, `probe`, that has this program and verbs.
fn with_row(program: &str, verbs: &str) -> Data {
    let ice = include_str!("../../../data/ice.ron");
    let end = ice.rfind(']').unwrap();
    let row = format!(
        "    (id: \"probe\", name: \"Probe\", class: Sentry, tier: 1, strength: 10, \
         cpu: 1, ram: 2, verbs: [{verbs}], program: \"{program}\"),\n"
    );
    let ice = format!("{}{row}{}", &ice[..end], &ice[end..]);
    Data::parse(ninsei_core::Sources {
        ice: &ice,
        ..ninsei_core::Sources::builtin()
    })
    .unwrap()
}

/// A fight against `probe` on `data`, strong enough to last, with the deck's `program`.
fn fight_on(data: Data, program: &str) -> ninsei_core::Game {
    fight_carrying(data, program, &[])
}

/// [`fight_on`], carrying these programs too (factory ids) besides the classic four.
fn fight_carrying(data: Data, program: &str, more: &[&str]) -> ninsei_core::Game {
    use ninsei_core::*;
    let mut free: Vec<String> = ["icepick", "firewall", "patch", "decoder"]
        .iter()
        .chain(more)
        .map(|s| s.to_string())
        .collect();
    free.dedup();
    let kit = Kit {
        tools: data::TOOLS.iter().map(|t| t.to_string()).collect(),
        free,
        ..Kit::default()
    };
    let mut g = Game::start(data, 7, &kit);
    let target = g.net.nodes[g.at].links[0];
    let ice = g.data.ice.iter().position(|i| i.id == "probe").unwrap();
    g.net.nodes[target].kind = network::NodeKind::Ice;
    g.net.nodes[target].ice = Some(ice);
    g.net.nodes[target].cleared = false;
    g.apply(Action::Move(target));
    assert_eq!(g.phase, Phase::Combat);
    let c = g.combat.as_mut().unwrap();
    c.strength = 500;
    c.strength_max = 500;
    g.apply(Action::Program(program.into()));
    g
}

#[test]
fn spoof_hides_next_for_its_turns_on_every_screen_and_to_if_next() {
    use ninsei_core::fight::NextMove;
    use ninsei_core::*;
    let data = with_row(
        r"10 SPOOF 2\n20 ATTACK 3\n30 ATTACK 3\n40 ATTACK 3\n50 ATTACK 3",
        "Spoof, Attack",
    );
    let mut g = fight_on(data, "10 IF NEXT ATTACK\n20 STOP");
    assert_eq!(g.next_move(), Some(NextMove::Act(Intent::Spoof(2))));
    g.apply(Action::EndTurn);
    assert!(g.log.iter().any(|l| {
        l.text
            .contains("SPOOF 2: its next moves are hidden for 2 turns.")
    }));
    // Two blind turns: NEXT reads ???, IF NEXT is false but costs its GB, no listing mark.
    for turn in 0..2 {
        assert_eq!(g.next_move(), Some(NextMove::Spoofed), "turn {turn}");
        assert_eq!(NextMove::Spoofed.text("(lagged)"), "???");
        assert!(!g.test(&basic::Cond::Next(basic::Move::Attack)));
        let watch = suite::watches(&g)
            .into_iter()
            .find(|(k, _)| *k == "NEXT")
            .unwrap()
            .1;
        assert_eq!(watch, "???");
        assert!(!ice_listing::rows(&g).iter().any(|r| r.next));
        // The pass evaluates the IF (spending its GB), finds it false and runs off the end,
        // which ends the turn.
        g.apply(Action::Finish);
        assert_eq!(
            g.exec.trace[0].what,
            game::StepWhat::If(false),
            "{:?}",
            g.exec.trace
        );
    }
    // Then it shows again.
    assert_eq!(g.next_move(), Some(NextMove::Act(Intent::Attack(3))));
}

#[test]
fn the_sniffer_clears_a_spoof_and_is_on_dead_sector() {
    use ninsei_core::fight::NextMove;
    use ninsei_core::*;
    let d = Data::builtin().unwrap();
    let w = d
        .warez
        .iter()
        .find(|w| w.id == "sniffer")
        .expect("a sniffer row");
    assert_eq!(w.effect, data::Effect::Sniff);
    // Its binary round-trips and fits its budget.
    let bin = binary::encode(w, 0x51);
    let spec = binary::check(&bin, 0x51).expect("a valid binary");
    assert_eq!(spec.effect, data::Effect::Sniff);
    assert!(d.boards.iter().any(|b| {
        b.id == "deadsector"
            && b.areas
                .iter()
                .flat_map(|a| &a.files)
                .any(|f| f.item == data::Drop::Warez("sniffer".into()))
    }));
    // In a fight: nothing to sniff until it's spoofed, then it clears it.
    let data = with_row(r"10 SPOOF 3\n20 ATTACK 3", "Spoof, Attack");
    let mut g = fight_carrying(data, "", &["sniffer"]);
    let slot = g.slot_named("sniffer").expect("carried");
    assert!(g.preview(slot).unwrap_err().contains("nothing to sniff"));
    g.apply(Action::EndTurn);
    assert_eq!(g.next_move(), Some(NextMove::Spoofed));
    g.apply(Action::Run(slot));
    assert_eq!(g.combat.as_ref().unwrap().spoofed, 0);
    assert_eq!(g.next_move(), Some(NextMove::Act(Intent::Attack(3))));
    assert!(
        g.log
            .iter()
            .any(|l| l.text.contains("sniffs out the spoof"))
    );
}
