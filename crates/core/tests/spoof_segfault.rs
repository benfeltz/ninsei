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
        assert!(
            !ice_listing::rows(&g).iter().any(|r| r.next || r.cursor),
            "no mark gives it away"
        );
        assert_eq!(g.ice_next(), None, "the dry run sees nothing either");
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

#[test]
fn segfault_garbles_the_line_that_last_ran_and_a_retype_fixes_it() {
    use ninsei_core::*;
    let data = with_row(
        r"10 IF TARGET RAN icepick\n20 SEGFAULT\n30 ATTACK 1",
        "Segfault, Attack",
    );
    let prog = "10 RUN icepick\n20 RUN icepick\n30 GOTO 10";
    let mut g = fight_on(data.clone(), prog);
    let (integ, trace) = (g.deck.integrity, g.deck.trace);
    g.apply(Action::Finish);
    // The pass ran icepick three times; the last RUN was line 10 or 20, whichever came last.
    let last = g
        .exec
        .trace
        .iter()
        .rev()
        .find(|s| matches!(s.what, game::StepWhat::Ran { .. }))
        .map(|s| s.line)
        .unwrap();
    let garbled = g.program.line(last).unwrap().clone();
    assert!(
        garbled.text.chars().all(|c| !c.is_ascii_alphanumeric()),
        "{garbled:?}"
    );
    assert_eq!(
        g.program.fault(last).as_deref(),
        Some(format!("?SYNTAX ERROR IN {last}").as_str())
    );
    assert!(
        g.log
            .iter()
            .any(|l| l.text == format!("?SYNTAX ERROR IN {last}"))
    );
    // No meter moved on its account (a turn adds its trace; the ICE did no damage).
    assert_eq!(g.deck.integrity, integ);
    assert!(g.deck.trace <= trace + 2);
    // The next pass skips it with the fault.
    g.apply(Action::Finish);
    assert!(g.exec.trace.iter().any(|s| s.line == last
        && matches!(&s.what, game::StepWhat::Skipped(w) if w.contains("SYNTAX ERROR"))));
    // Retyping fixes it.
    g.apply(listing::retype(&g, last, "RUN icepick").unwrap());
    assert_eq!(g.program.fault(last), None);
    // Same seed, same garble.
    let again = {
        let mut h = fight_on(data, prog);
        h.apply(Action::Finish);
        h.program.line(last).unwrap().text.clone()
    };
    assert_eq!(again, garbled.text);
}

#[test]
fn a_garbled_program_goes_home_until_retyped() {
    use ninsei_core::*;
    let d = with_row(r"10 SEGFAULT\n20 ATTACK 1", "Segfault, Attack");
    let mut g = fight_on(d.clone(), "10 RUN icepick");
    g.program.garble(10, "#$%&@!?*");
    g.phase = Phase::JackedOut;
    let mut s = hub::Save::default();
    s.ensure_key(1);
    s.record(&g, &[], &d.rig).unwrap();
    let p = basic::parse(&s.program_text(&d));
    assert!(p.fault(10).is_some(), "still bad at home: {p:?}");
}

#[test]
fn a_turn_played_on_the_hotbar_segfaults_the_first_line_and_the_log_reads_in_order() {
    use ninsei_core::*;
    // It hits on its first turn and garbles on its second.
    let data = with_row(r"10 ATTACK 1\n20 SEGFAULT", "Segfault, Attack");
    let mut g = fight_on(data, "10 REM mine\n20 RUN icepick\n30 RUN icepick");
    // Last turn's pass ran line 30; this turn is played by hand.
    g.apply(Action::Finish);
    let pick = g.slot_named("icepick").unwrap();
    g.apply(Action::Run(pick));
    let before = g.log.len();
    g.apply(Action::EndTurn);
    assert!(
        g.program.fault(20).is_some(),
        "the first line that does something"
    );
    assert!(g.program.fault(30).is_none(), "not last turn's line");
    let said: Vec<&str> = g.log[before..].iter().map(|l| l.text.as_str()).collect();
    let ice = said
        .iter()
        .position(|t| t.contains("SEGFAULT: garbles your line 20"))
        .unwrap();
    let err = said
        .iter()
        .position(|t| *t == "?SYNTAX ERROR IN 20")
        .unwrap();
    assert!(ice < err, "{said:?}");
}

#[test]
fn segfault_keeps_its_target_through_a_hang_and_never_hits_a_dead_line_twice() {
    use ninsei_core::*;
    let data = with_row(r"10 SEGFAULT\n20 SEGFAULT\n30 ATTACK 1", "Segfault, Attack");
    // The pass runs firewall at 10, then hangs at 40/50.
    let mut g = fight_on(
        data,
        "10 RUN firewall\n20 REM\n40 GOTO 50\n50 GOTO 40\n60 RUN patch",
    );
    g.apply(Action::Finish);
    assert_eq!(g.exec.broke, Some(40), "{:?}", g.exec.trace);
    g.apply(Action::EndTurn);
    assert!(
        g.program.fault(10).is_some(),
        "the line that ran, despite the hang"
    );
    // Next turn on the hotbar: it hits the first working line that isn't garbled yet.
    g.apply(Action::EndTurn);
    assert!(
        g.program.fault(40).is_some(),
        "not line 10 again: {}",
        g.program.text()
    );
}
