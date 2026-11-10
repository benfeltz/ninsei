//! ICE run their own programs: the data that holds them, the turn that runs them and the dry
//! run that telegraphs them.

mod common;

use ninsei_core::game::StepWhat;
use ninsei_core::*;

/// The builtin ICE with one more row, `probe`, that has this program, budgets and verbs.
fn with_row(program: &str, cpu: u32, ram: u32, verbs: &str) -> Result<Data, String> {
    common::with_probe_row(
        &format!("class: Sentry, tier: 1, strength: 10, cpu: {cpu}, ram: {ram}"),
        verbs,
        program,
    )
}

#[test]
fn every_builtin_ice_has_a_program_that_parses() {
    let d = Data::builtin().unwrap();
    for ice in &d.ice {
        let p = basic::parse_with(&ice.program, basic::Vocab::Ice(&ice.verbs));
        assert!(!p.lines.is_empty(), "{}", ice.id);
        assert!(
            p.lines.iter().all(|l| p.fault(l.no).is_none()),
            "{}",
            ice.id
        );
        assert!(ice.cpu >= 1 && ice.ram >= 1, "{}", ice.id);
    }
}

#[test]
fn a_good_extra_row_validates() {
    with_row(
        r"10 IF TARGET SHIELD > 0\n20 TRACE 3\n30 ATTACK 2",
        1,
        1,
        "Attack, Trace",
    )
    .unwrap();
}

#[test]
fn a_bad_program_is_rejected() {
    let bad = [
        (
            r"10 ATTACK 2\n20 TRACE 3",
            1,
            1,
            "Attack",
            "TRACE isn't on its row",
        ),
        (
            r"10 RUN icepick\n20 ATTACK 2",
            1,
            1,
            "Attack",
            "the deck's word",
        ),
        (
            r"10 ATTACK 2\n20 GOTO 99",
            1,
            1,
            "Attack",
            "a GOTO to nowhere",
        ),
        (
            r"10 IF TARGET RAN nothing\n20 ATTACK 2",
            1,
            1,
            "Attack",
            "no such program",
        ),
        (r"10 ATTACK 2", 0, 1, "Attack", "no cycles"),
        (r"10 ATTACK 2", 1, 0, "Attack", "no RAM"),
        (r"10 REM idle\n20 GOTO 10", 1, 1, "Attack", "never acts"),
        (
            r"10 GOTO 20\n20 GOTO 10\n30 ATTACK 1",
            1,
            1,
            "Attack",
            "loops before it acts",
        ),
        (
            r"10 STOP\n20 ATTACK 1",
            1,
            1,
            "Attack",
            "stops before it acts",
        ),
        (r"ATTACK 2", 1, 1, "Attack", "no line number"),
    ];
    for (program, cpu, ram, verbs, why) in bad {
        let err = with_row(program, cpu, ram, verbs).expect_err(why);
        assert!(err.contains("probe"), "{why}: {err}");
    }
}

/// A fight against ICE `id` on `data`, strong enough to last, with the deck's program `program`.
fn fight_on(data: Data, id: &str, program: &str) -> Game {
    let mut g = Game::new(data, 7);
    let target = g.net.nodes[g.at].links[0];
    let ice = g.data.ice.iter().position(|i| i.id == id).unwrap();
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

/// What the ICE did in its last turn: the actions in its pass, in order.
fn ice_did(g: &Game) -> Vec<StepWhat> {
    g.combat
        .as_ref()
        .unwrap()
        .ice_exec
        .trace
        .iter()
        .map(|s| s.what.clone())
        .filter(|w| {
            matches!(
                w,
                StepWhat::Hit { .. }
                    | StepWhat::Traced(_)
                    | StepWhat::Fortified(_)
                    | StepWhat::Lagged
            )
        })
        .collect()
}

/// The intent cycle as a program: FORTIFY 3, ATTACK 3, round again.
fn cycler() -> Data {
    with_row(
        r"10 FORTIFY 3\n20 ATTACK 3\n30 GOTO 10",
        1,
        1,
        "Fortify, Attack",
    )
    .unwrap()
}

#[test]
fn a_two_line_ice_cycles_as_its_intents_did() {
    // What Ice Wall's `intents: [Fortify(3), Attack(3)]` became.
    let mut g = fight_on(cycler(), "probe", "");
    let mut seen = Vec::new();
    for _ in 0..5 {
        seen.push(g.ice_next());
        g.apply(Action::EndTurn);
        assert_eq!(ice_did(&g).len(), 1, "one action a turn");
    }
    use data::Intent::*;
    assert_eq!(
        seen,
        [
            Some(Fortify(3)),
            Some(Attack(3)),
            Some(Fortify(3)),
            Some(Attack(3)),
            Some(Fortify(3))
        ]
    );
    let said: Vec<&str> = g.log.iter().map(|l| l.text.as_str()).collect();
    assert!(
        said.contains(&"Probe 20 ATTACK 3: hits for 3."),
        "its steps are in the log: {said:?}"
    );
}

/// An ICE that traces when the deck shields and hits when it doesn't.
const READER: &str =
    r"10 IF TARGET SHIELD > 0\n20 GOTO 50\n30 ATTACK 4\n40 GOTO 10\n50 TRACE 3\n60 GOTO 10";

#[test]
fn an_ice_that_reads_the_shield_changes_its_mind_before_it_acts() {
    let data = with_row(READER, 1, 2, "Attack, Trace").unwrap();
    let mut g = fight_on(data, "probe", "");
    assert_eq!(g.ice_next(), Some(data::Intent::Attack(4)));
    assert!(g.test(&basic::Cond::Next(basic::Move::Attack)));
    let fw = g
        .slot_named("firewall")
        .expect("the classic kit has a firewall");
    g.apply(Action::Run(fw));
    assert!(g.combat.as_ref().unwrap().shield > 0);
    // The shield's up: NEXT says so before the ICE has done anything.
    assert_eq!(g.ice_next(), Some(data::Intent::Trace(3)));
    assert!(g.test(&basic::Cond::Next(basic::Move::Trace)));
    let trace = g.deck.trace;
    let integ = g.deck.integrity;
    g.apply(Action::EndTurn);
    assert_eq!(ice_did(&g), [StepWhat::Traced(3)]);
    assert!(g.deck.trace > trace);
    assert_eq!(g.deck.integrity, integ);
    // A turn with no shield: it hits.
    assert_eq!(g.ice_next(), Some(data::Intent::Attack(4)));
    g.apply(Action::EndTurn);
    assert_eq!(ice_did(&g), [StepWhat::Hit { dmg: 4, blocked: 0 }]);
}

#[test]
fn an_ice_reads_what_the_deck_ran_this_turn() {
    let data = with_row(
        r"10 IF TARGET RAN patch\n20 TRACE 5\n30 ATTACK 1",
        2,
        1,
        "Attack, Trace",
    )
    .unwrap();
    let mut g = fight_on(data, "probe", "");
    assert_eq!(g.ice_next(), Some(data::Intent::Attack(1)));
    g.deck.integrity -= 5;
    let patch = g.slot_named("patch").expect("the classic kit has a patch");
    g.apply(Action::Run(patch));
    assert_eq!(g.ice_next(), Some(data::Intent::Trace(5)));
    g.apply(Action::EndTurn);
    assert_eq!(
        ice_did(&g),
        [StepWhat::Traced(5), StepWhat::Hit { dmg: 1, blocked: 0 }],
        "two cycles: two actions"
    );
    // A new turn forgets what ran in the last one.
    assert_eq!(g.ice_next(), Some(data::Intent::Attack(1)));
}

#[test]
fn the_dry_run_changes_nothing() {
    let data = with_row(READER, 1, 2, "Attack, Trace").unwrap();
    let mut g = fight_on(data, "probe", "");
    g.apply(Action::EndTurn);
    let before = format!("{g:?}");
    for _ in 0..3 {
        assert!(g.ice_next().is_some());
        assert!(g.test(&basic::Cond::Next(basic::Move::Attack)));
    }
    assert_eq!(format!("{g:?}"), before, "no state, no RNG, no log");
}

#[test]
fn a_lagged_ice_has_no_next_and_loses_its_move() {
    let mut g = fight_on(cycler(), "probe", "");
    g.combat.as_mut().unwrap().stunned = true;
    assert_eq!(g.ice_next(), None);
    let strength = g.combat.as_ref().unwrap().strength;
    g.apply(Action::EndTurn);
    assert_eq!(ice_did(&g), [StepWhat::Lagged]);
    assert_eq!(g.combat.as_ref().unwrap().strength, strength);
    // It skipped its FORTIFY, as the intent cycle did.
    assert_eq!(g.ice_next(), Some(data::Intent::Attack(3)));
    let lagged = |g: &Game| {
        g.log
            .iter()
            .filter(|l| l.text == "Probe is lagged and does nothing.")
            .count()
    };
    assert_eq!(lagged(&g), 1, "said once");
    // A lagged ICE whose pass stops before any verb still says the lag worked.
    let data = with_row(r"10 IF TURN = 1\n20 STOP\n30 ATTACK 1", 1, 1, "Attack").unwrap();
    let mut g = fight_on(data, "probe", "");
    g.combat.as_mut().unwrap().stunned = true;
    g.apply(Action::EndTurn);
    assert!(ice_did(&g).is_empty());
    assert_eq!(lagged(&g), 1);
}

#[test]
fn an_ice_that_goes_round_without_spending_stalls() {
    // Strong, it loops between 40 and 50 spending nothing; weak, it would attack.
    let data = with_row(
        r"10 IF SELF STR > 400\n20 GOTO 40\n30 ATTACK 1\n40 GOTO 50\n50 GOTO 40",
        1,
        1,
        "Attack",
    )
    .unwrap();
    let mut g = fight_on(data, "probe", "");
    assert_eq!(g.ice_next(), None);
    let integ = g.deck.integrity;
    g.apply(Action::EndTurn);
    assert_eq!(g.deck.integrity, integ);
    assert!(
        g.log.iter().any(|l| l.text.contains("Probe stalls in")),
        "{:?}",
        g.log
    );
    assert_eq!(g.combat.as_ref().unwrap().turn, 2, "the fight goes on");
}

#[test]
fn an_ice_out_of_ram_comes_back_to_its_if() {
    // Two IFs and one GB: the second IF waits for the next turn.
    let data = with_row(
        r"10 IF TURN > 0\n20 IF TURN > 0\n30 ATTACK 2",
        1,
        1,
        "Attack",
    )
    .unwrap();
    let mut g = fight_on(data, "probe", "");
    assert_eq!(g.ice_next(), None, "it can't reach an action this turn");
    g.apply(Action::EndTurn);
    assert!(ice_did(&g).is_empty());
    assert_eq!(g.combat.as_ref().unwrap().ice_exec.pc, Some(20));
    assert_eq!(g.ice_next(), Some(data::Intent::Attack(2)));
    g.apply(Action::EndTurn);
    assert_eq!(ice_did(&g), [StepWhat::Hit { dmg: 2, blocked: 0 }]);
}

/// The preview is the rule: over many fights, played by hand with a shield now and then, the
/// ICE's first action each turn is what NEXT said just before.
#[test]
fn next_is_what_the_ice_does() {
    let data = Data::builtin().unwrap();
    let mut turns = 0;
    for ice in 0..data.ice.len() {
        for seed in 0..6u32 {
            let id = data.ice[ice].id.clone();
            let mut g = fight_on(data.clone(), &id, "");
            for t in 0..12u32 {
                // Something different each turn: a shield, a patch, a breaker, or nothing.
                let pick = ["firewall", "patch", "icepick", ""][((t + seed) % 4) as usize];
                if let Some(s) = g.slot_named(pick) {
                    g.apply(Action::Run(s));
                }
                let said = g.ice_next();
                // Lagged or spoofed, the deck sees no move coming.
                let hidden = g
                    .combat
                    .as_ref()
                    .is_some_and(|c| c.stunned || c.spoofed > 0);
                g.apply(Action::EndTurn);
                let Some(c) = g.combat.as_ref() else { break };
                let first = c.ice_exec.trace.iter().find(|s| {
                    c.ice_prog
                        .line(s.line)
                        .is_some_and(|l| matches!(l.stmt, basic::Stmt::Act(_)))
                });
                let did = first.and_then(|s| match c.ice_prog.line(s.line)?.stmt {
                    basic::Stmt::Act(a) => Some(a),
                    _ => None,
                });
                if hidden {
                    assert_eq!(said, None);
                } else {
                    assert_eq!(said, did, "{id} turn {t}");
                }
                turns += 1;
            }
        }
    }
    assert!(turns > 100);
}

#[test]
fn next_says_the_same_on_every_screen() {
    use fight::NextMove;
    let data = with_row(
        r"10 IF SELF STR > 400\n20 STOP\n30 ATTACK 1",
        1,
        1,
        "Attack",
    )
    .unwrap();
    let mut g = fight_on(data.clone(), "probe", "");
    // Strong, its program stops before it acts: nothing this turn.
    assert_eq!(g.next_move(), Some(NextMove::Idle));
    let watch = |g: &Game| {
        suite::watches(g)
            .into_iter()
            .find(|(k, _)| *k == "NEXT")
            .unwrap()
            .1
    };
    assert_eq!(watch(&g), NextMove::IDLE);
    g.combat.as_mut().unwrap().strength = 10;
    assert_eq!(g.next_move(), Some(NextMove::Act(data::Intent::Attack(1))));
    assert_eq!(watch(&g), "ATTACK 1");
    g.combat.as_mut().unwrap().stunned = true;
    assert_eq!(g.next_move(), Some(NextMove::Lagged));
    assert_eq!(watch(&g), suite::LAGGED);
    assert_eq!(NextMove::Lagged.text("(lagged)"), "(lagged)");
    g.combat = None;
    assert_eq!(g.next_move(), None);
}
