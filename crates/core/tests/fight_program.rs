//! 141.2 acceptance: the fight is a program the deck runs, played through the real save and
//! run (`Save::jack_in`, `Game::apply`, `Save::record`), with loading gone.

use ninsei_core::basic;
use ninsei_core::binary;
use ninsei_core::command::{self, Command};
use ninsei_core::game::{ExecState, StepWhat};
use ninsei_core::hub::{Save, Upgrade};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

fn fresh() -> Save {
    let d = data();
    let mut s = Save::default();
    s.ensure_key(0xACCE97);
    s.ensure_phonebook(&d, 0);
    s.ensure_hardware(&d);
    s
}

/// A run from `save`, standing in a fight with `ice` too strong to fall.
fn fight_from(save: &mut Save, ice: &str) -> Game {
    let d = data();
    let mut g = save.jack_in(&d, 11);
    let target = g.net.nodes[g.at].links[0];
    let ice = g.data.ice.iter().position(|i| i.id == ice).unwrap();
    g.net.nodes[target].kind = NodeKind::Ice;
    g.net.nodes[target].ice = Some(ice);
    g.net.nodes[target].cleared = false;
    g.apply(Action::Move(target));
    assert_eq!(g.phase, Phase::Combat);
    let c = g.combat.as_mut().unwrap();
    (c.strength, c.strength_max) = (500, 500);
    g
}

fn ran(g: &Game) -> usize {
    g.exec
        .trace
        .iter()
        .filter(|s| matches!(s.what, StepWhat::Ran { .. }))
        .count()
}

#[test]
fn a_fresh_deck_fights_with_safe_and_a_pass_is_one_turn() {
    let mut s = fresh();
    let mut g = fight_from(&mut s, "bulwark");
    assert!(
        g.program.text().contains("SAFE"),
        "the kit's program is loaded"
    );
    assert!(
        g.program_view().lines[1].current,
        "the cursor on the first line that runs, past SAFE's REM"
    );
    let cpu = g.deck.cpu_max;
    g.apply(Action::Finish);
    // SAFE only defends: at full integrity against a fortify it has nothing to do, comes
    // round to line 20 having run nothing, and hands the turn over.
    assert_eq!(ran(&g), 0, "{:?}", g.exec.trace);
    assert!(g.log.iter().any(|l| l.text == "?BREAK IN 20"));
    assert_eq!(
        g.combat.as_ref().unwrap().turn,
        1,
        "the turn is the player's"
    );
    // The first edit: a breaker in the loop. The pass carries on in the same turn, with the
    // GB the hang already spent gone, so it runs out of RAM before cycles and the turn ends.
    g.apply(Action::Program(format!(
        "{}\n55 RUN icepick",
        g.program.text()
    )));
    g.apply(Action::Finish);
    assert_eq!(g.combat.as_ref().unwrap().turn, 2, "the ICE acted once");
    // A whole turn: two IFs a loop, and the deck's RAM covers a loop per cycle.
    g.apply(Action::Finish);
    assert_eq!(ran(&g) as u32, cpu, "{:?}", g.exec.trace);
    assert_eq!(g.combat.as_ref().unwrap().turn, 3);
}

#[test]
fn safe_names_only_what_the_deck_ships() {
    let d = data();
    let kit = d.starts.kit("");
    let safe = basic::parse(d.starts.program(""));
    let mut ran = Vec::new();
    for line in &safe.lines {
        match &line.stmt {
            basic::Stmt::Run(name) | basic::Stmt::If(basic::Cond::Have(name)) => {
                assert!(
                    kit.free.iter().any(|id| id.eq_ignore_ascii_case(name)),
                    "SAFE names {name}, which the deck doesn't ship"
                );
                ran.push(name.to_ascii_lowercase());
            }
            _ => {}
        }
    }
    // Defence only: the Security Suite's two programs, never a breaker.
    ran.sort();
    ran.dedup();
    assert_eq!(ran, ["firewall", "patch"]);
}

#[test]
fn ram_is_the_ifs_a_turn_and_grows_with_the_rig() {
    let mut s = fresh();
    let base = s.stat_value(&data(), Upgrade::Ram);
    s.rig.ram = 1;
    let mut g = fight_from(&mut s, "bulwark");
    assert_eq!(g.combat.as_ref().unwrap().ifs, base + 1);
    // Eight IFs on a deck with base + 1 GB: the pass runs out of RAM, it doesn't error.
    // Each IF guards a line of its own (IFs in a row are one AND, skipped together).
    let ifs: String = (1..=12)
        .map(|n| format!("{} IF TURN = 99\n{} REM\n", n * 10, n * 10 + 5))
        .collect();
    g.apply(Action::Program(format!("{ifs}200 RUN icepick")));
    g.apply(Action::Finish);
    assert_eq!(g.exec.trace.last().unwrap().what, StepWhat::OutOfRam);
    let ifs_run = g
        .exec
        .trace
        .iter()
        .filter(|s| matches!(s.what, StepWhat::If(_)))
        .count() as u32;
    assert_eq!(ifs_run, base + 1);
    assert_eq!(ran(&g), 0);
}

#[test]
fn nothing_loads_every_program_on_the_disk_is_a_verb() {
    let d = data();
    for w in &d.warez {
        let bin = binary::encode(w, 7);
        assert_eq!(bin[binary::OFF_SPARE], 0, "{}", w.id);
        assert!(binary::check(&bin, 7).is_ok(), "{} fits its budget", w.id);
    }
    let mut s = fresh();
    let mut g = fight_from(&mut s, "bulwark");
    g.deck.ram_max = 0;
    for slot in 0..g.hdd.len() {
        assert!(g.preview(slot).is_ok(), "slot {slot}");
    }
    // A RUN the preview refuses is skipped with its words; the fight goes on.
    g.combat.as_mut().unwrap().cpu = 1;
    let pick = g.slot_named("icepick").unwrap();
    g.apply(Action::Run(pick));
    let why = g.preview(pick).unwrap_err();
    g.apply(Action::Program("10 RUN icepick\n20 STOP".into()));
    let before = g.combat.as_ref().unwrap().turn;
    g.apply(Action::Finish);
    // CPU is already 0: the pass ends at once, the turn with it.
    assert!(why.contains("cpu"), "{why}");
    assert_eq!(g.exec.trace[0].what, StepWhat::OutOfCpu);
    assert_eq!(g.combat.as_ref().unwrap().turn, before + 1);
}

#[test]
fn stop_and_hang_hand_the_turn_to_the_hotbar() {
    let mut s = fresh();
    let mut g = fight_from(&mut s, "bulwark");
    g.apply(Action::Program(
        "10 RUN firewall\n20 STOP\n30 GOTO 30".into(),
    ));
    g.apply(Action::Play);
    g.apply(Action::Finish);
    assert_eq!(g.exec.state, ExecState::Stopped(20));
    let shield = g.slot_named("firewall").unwrap();
    g.apply(Action::Run(shield));
    assert_eq!(g.combat.as_ref().unwrap().shield, 10, "the hotbar is live");
    g.apply(Action::Finish);
    assert_eq!(g.exec.trace.last().unwrap().what, StepWhat::Break);
    assert!(g.log.iter().any(|l| l.text == "?BREAK IN 30"));
    assert_eq!(g.exec.state, ExecState::Paused);
    assert_eq!(g.combat.as_ref().unwrap().turn, 1, "neither ended the turn");
}

#[test]
fn the_deck_keeps_what_you_leave_in_its_program() {
    let d = data();
    let mut s = fresh();
    assert_eq!(s.program, None, "a new deck ships the kit's");
    let mut g = fight_from(&mut s, "bulwark");
    // Edited mid-fight from the terminal, line by line.
    for line in ["prog 5 IF TRACE > 90", "prog 6 STOP"] {
        match command::parse_in(&g, line).unwrap() {
            Command::Game(a) => g.apply(a),
            other => panic!("{other:?}"),
        }
    }
    let edited = g.program.text();
    assert!(edited.starts_with("5 IF TRACE > 90\n6 STOP\n10 REM SAFE"));
    g.phase = Phase::JackedOut;
    s.record(&g, &[], &d.rig).unwrap();
    assert_eq!(s.program.as_deref(), Some(edited.as_str()));
    // It survives the save file and comes back for the next run.
    let back = Save::load(&s.to_text()).unwrap();
    let g = back.clone().jack_in(&d, 12);
    assert_eq!(g.program.text(), edited);
}

#[test]
fn the_bots_play_programs_and_the_probe_still_tells_the_same_story() {
    use ninsei_core::probe::{self, Bot, Config, Probe};
    let d = data();
    assert_eq!(basic::parse(probe::DUMB).lines.len(), 2);
    for seed in 0..6 {
        for bot in [Bot::Dumb, Bot::Smart] {
            let (g, _) = probe::play(Game::new(d.clone(), seed), bot);
            assert!(matches!(g.phase, Phase::JackedOut | Phase::Flatlined));
        }
    }
    let cfg = Config {
        spot_runs: 6,
        careers: 2,
        career_runs: 6,
        seed: 0x5EED,
    };
    let report = || {
        let mut p = Probe::new(&d, cfg);
        while !p.step(50) {}
        p.report()
    };
    assert_eq!(report(), report());
}

/// The names of the programs a pass ran, in order.
fn ran_names(g: &Game) -> Vec<String> {
    g.exec
        .trace
        .iter()
        .filter_map(|s| match s.what {
            StepWhat::Ran { uid, .. } => g.hdd.iter().find(|w| w.uid == uid),
            _ => None,
        })
        .map(|w| w.spec.name.to_ascii_lowercase())
        .collect()
}

#[test]
fn safe_patches_when_low_and_raises_the_firewall_before_an_attack() {
    let mut s = fresh();
    let mut g = fight_from(&mut s, "bulwark");
    // Bulwark's second move is an attack: put it next, and the deck low.
    g.combat.as_mut().unwrap().intent_ix = 1;
    assert!(matches!(g.intent(), Some(Intent::Attack(_))));
    g.deck.integrity = 5;
    g.apply(Action::Finish);
    let ran = ran_names(&g);
    assert_eq!(ran[..2], ["patch", "firewall"], "{:?}", g.exec.trace);
    assert!(
        ran.iter().all(|n| n == "patch" || n == "firewall"),
        "SAFE never attacks: {ran:?}"
    );
}

#[test]
fn play_with_safe_hands_back_every_quiet_turn() {
    let mut s = fresh();
    let mut g = fight_from(&mut s, "bulwark");
    // Fortify next, the deck whole: nothing to defend against.
    assert!(matches!(g.intent(), Some(Intent::Fortify(_))));
    g.apply(Action::Play);
    g.apply(Action::Finish);
    assert_ne!(g.exec.state, ExecState::Running, "a hang stops Run");
    assert_eq!(
        g.combat.as_ref().unwrap().turn,
        1,
        "the turn is the player's"
    );
    assert!(g.log.iter().any(|l| l.text == "?BREAK IN 20"));
    // The hotbar still plays it by hand.
    let pick = g.slot_named("icepick").unwrap();
    let before = g.combat.as_ref().unwrap().strength;
    g.apply(Action::Run(pick));
    assert!(g.combat.as_ref().unwrap().strength < before);
}

/// A turn of SAFE with a breaker in its loop, as the trace says it: (line, what).
fn turn_of_safe_and_icepick() -> (Game, Vec<(u32, String)>) {
    let mut s = fresh();
    let mut g = fight_from(&mut s, "bulwark");
    g.combat.as_mut().unwrap().intent_ix = 1;
    g.deck.integrity = 5;
    g.apply(Action::Program(format!(
        "{}\n55 RUN icepick",
        g.program.text()
    )));
    g.apply(Action::Finish);
    let trace = g
        .exec
        .trace
        .iter()
        .map(|s| (s.line, format!("{:?}", s.what)))
        .collect();
    (g, trace)
}

#[test]
fn finish_is_the_whole_turn_it_always_was() {
    let (g, trace) = turn_of_safe_and_icepick();
    let c = g.combat.as_ref().unwrap();
    // Snapshot of today's whole-turn pass (141.8 step 1): the trace, the budgets and what the
    // ICE has left.
    let got = (trace, c.turn, c.cpu, c.ifs, c.strength, g.deck.integrity);
    assert_eq!(format!("{got:?}"), SNAPSHOT, "{got:?}");
}

const SNAPSHOT: &str = "([(20, \"If(true)\"), (30, \"Ran { uid: 2, outcome: Repair(3) }\"), \
    (40, \"If(true)\"), (50, \"Ran { uid: 1, outcome: Shield(5) }\"), \
    (55, \"Ran { uid: 3, outcome: Break { hit: 3, vs: Plain } }\"), (55, \"OutOfCpu\")], \
    2, 3, 6, 497, 8)";

#[test]
fn a_hang_holds_the_line_until_an_edit() {
    let mut s = fresh();
    let mut g = fight_from(&mut s, "bulwark");
    g.apply(Action::Finish);
    // SAFE comes round to line 20 having run nothing: the cursor stays on it.
    assert_eq!(g.exec.broke, Some(20));
    assert_eq!(g.exec.pc, Some(20));
    assert_eq!(ninsei_core::listing::broke(&g), Some(20));
    let v = g.program_view();
    assert!(v.lines.iter().find(|l| l.no == 20).unwrap().current);
    assert_eq!(
        g.combat.as_ref().unwrap().turn,
        1,
        "the turn is the player's"
    );
    // An edit clears it, and the next pass starts at the top.
    g.apply(Action::Program(g.program.text()));
    assert_eq!(g.exec.broke, None);
    assert_eq!(g.exec.pc, Some(20), "the top, past the REM");
}

/// A fight against an ICE that won't fall, running `program`.
fn fight_with(program: &str) -> Game {
    let mut s = fresh();
    let mut g = fight_from(&mut s, "bulwark");
    g.apply(Action::Program(program.into()));
    g
}

fn budgets(g: &Game) -> (u32, u32, u32) {
    let c = g.combat.as_ref().unwrap();
    (c.cpu, c.ifs, c.turn)
}

const LOOP: &str = "10 IF INTEG > 0\n20 RUN icepick\n30 GOTO 10";

#[test]
fn a_step_is_one_line_and_spends_as_it_goes() {
    let mut g = fight_with(LOOP);
    let (cpu, ifs, turn) = budgets(&g);
    let strength = g.combat.as_ref().unwrap().strength;
    assert_eq!(g.exec.pc, Some(10));
    g.apply(Action::Step);
    assert_eq!(g.exec.pc, Some(20), "the ▶ moves to the next line");
    assert_eq!(budgets(&g), (cpu, ifs - 1, turn), "the IF took its GB");
    assert!(g.program_view().lines[1].current);
    g.apply(Action::Step);
    let pick = g.slot_named("icepick").unwrap();
    let cost = g.hdd[pick].spec.cpu;
    assert_eq!(g.exec.pc, Some(30));
    assert_eq!(
        budgets(&g),
        (cpu - cost, ifs - 1, turn),
        "the RUN took its cycles"
    );
    assert!(g.combat.as_ref().unwrap().strength < strength);
    assert_eq!(g.exec.state, ExecState::Paused, "a step leaves it paused");
}

#[test]
fn the_line_that_spends_the_last_cycle_ends_the_turn() {
    let mut g = fight_with(LOOP);
    let pick = g.slot_named("icepick").unwrap();
    g.combat.as_mut().unwrap().cpu = g.hdd[pick].spec.cpu;
    let turn = budgets(&g).2;
    g.apply(Action::Step);
    assert_eq!(budgets(&g).2, turn, "the IF leaves the turn going");
    g.apply(Action::Step);
    assert_eq!(budgets(&g).2, turn + 1, "the ICE acted");
    assert_eq!(g.exec.trace.last().unwrap().what, StepWhat::OutOfCpu);
    assert_eq!(g.exec.pc, Some(10), "the ▶ is back at the top");
    assert!(!g.exec.open);
    // The program's end ends it too, on its last line.
    let mut g = fight_with("10 IF INTEG > 0\n20 RUN icepick");
    g.apply(Action::Step);
    g.apply(Action::Step);
    assert_eq!(budgets(&g).2, turn + 1);
    assert_eq!(g.exec.trace.last().unwrap().what, StepWhat::End);
    // And an IF with no GB left.
    let mut g = fight_with(LOOP);
    g.combat.as_mut().unwrap().ifs = 0;
    g.apply(Action::Step);
    assert_eq!(budgets(&g).2, turn + 1);
    assert_eq!(g.exec.trace.last().unwrap().what, StepWhat::OutOfRam);
}

#[test]
fn pause_keeps_the_place_and_end_turn_closes_the_pass() {
    let mut g = fight_with(LOOP);
    g.apply(Action::Play);
    assert_eq!(g.exec.state, ExecState::Running);
    assert_eq!(g.exec.pc, Some(20), "play ran a line");
    g.apply(Action::Pause);
    let left = budgets(&g);
    assert_eq!((g.exec.state, g.exec.pc), (ExecState::Paused, Some(20)));
    // The hotbar mid-pass spends its cycles and leaves the ▶ alone.
    let pick = g.slot_named("icepick").unwrap();
    let cost = g.hdd[pick].spec.cpu;
    g.apply(Action::Run(pick));
    assert_eq!(g.exec.pc, Some(20));
    assert_eq!(budgets(&g), (left.0 - cost, left.1, left.2));
    // Step carries on where it was, with the budgets as left.
    g.apply(Action::Step);
    assert_eq!(g.exec.pc, Some(30));
    assert_eq!(budgets(&g), (left.0 - 2 * cost, left.1, left.2));
    let lines: Vec<u32> = g.exec.trace.iter().map(|s| s.line).collect();
    assert_eq!(lines, [10, 20], "one pass, carried on");
    // End Turn mid-pass ends it: the ICE acts, the ▶ to the top, and the next Step starts a
    // pass of its own.
    g.apply(Action::EndTurn);
    assert_eq!(budgets(&g).2, left.2 + 1);
    assert_eq!(g.exec.pc, Some(10));
    assert!(!g.exec.open);
    g.apply(Action::Step);
    let lines: Vec<u32> = g.exec.trace.iter().map(|s| s.line).collect();
    assert_eq!(lines, [10]);
}

#[test]
fn a_step_passes_over_rems() {
    let mut g = fight_with("10 REM a\n20 REM b\n30 IF INTEG > 0\n40 REM c\n50 RUN icepick");
    g.apply(Action::Step);
    let lines: Vec<u32> = g.exec.trace.iter().map(|s| s.line).collect();
    assert_eq!(lines, [30], "the REMs ran nothing");
    assert_eq!(g.exec.pc, Some(50), "and the ▶ never rests on one");
}

#[test]
fn a_step_after_a_stop_carries_on_after_it() {
    let mut g = fight_with("10 STOP\n20 IF INTEG > 0\n30 GOTO 10");
    g.apply(Action::Step);
    assert_eq!(g.exec.state, ExecState::Stopped(10));
    assert_eq!(g.exec.pc, Some(20));
    g.apply(Action::Step);
    assert_eq!(g.exec.state, ExecState::Paused);
    assert_eq!(g.exec.trace.last().unwrap().line, 20);
    g.apply(Action::Step);
    g.apply(Action::Step);
    assert_eq!(
        g.exec.state,
        ExecState::Stopped(10),
        "round to the STOP again"
    );
}

#[test]
fn a_hang_found_by_a_step_holds_the_line_until_the_next_one() {
    let mut g = fight_with("10 GOTO 20\n20 GOTO 10");
    let turn = budgets(&g).2;
    g.apply(Action::Step);
    g.apply(Action::Step);
    assert_eq!((g.exec.pc, g.exec.broke), (Some(10), None));
    g.apply(Action::Step);
    assert_eq!((g.exec.pc, g.exec.broke), (Some(10), Some(10)));
    assert_eq!(g.exec.trace.last().unwrap().what, StepWhat::Break);
    assert_eq!(budgets(&g).2, turn, "a hang doesn't end the turn");
    // Play stops on it too.
    g.apply(Action::Play);
    g.apply(Action::Finish);
    assert_eq!(g.exec.state, ExecState::Paused);
    assert_eq!(g.exec.broke, Some(10));
    // The next Step clears it and starts the pass again at the top.
    g.apply(Action::Step);
    assert_eq!((g.exec.pc, g.exec.broke), (Some(20), None));
}

#[test]
fn the_terminal_steps_a_line_and_finishes_the_turn() {
    let mut g = fight_with(LOOP);
    let turn = budgets(&g).2;
    assert_eq!(
        command::parse_in(&g, "step").unwrap(),
        Command::Game(Action::Step)
    );
    assert_eq!(
        command::parse_in(&g, "fin").unwrap(),
        Command::Game(Action::Finish)
    );
    g.apply(Action::Step);
    g.apply(Action::Finish);
    assert_eq!(budgets(&g).2, turn + 1);
    assert!(command::HELP.iter().any(|l| l.starts_with("finish")));
}

#[test]
fn every_deck_says_the_hang_where_you_look_until_the_next_step() {
    use ninsei_core::{btop_fight, listing, stack_fight, suite};
    // A fresh deck's SAFE with nothing to defend against: Step until it hangs on line 20.
    let mut s = fresh();
    let mut g = fight_from(&mut s, "bulwark");
    while g.exec.broke.is_none() {
        g.apply(Action::Step);
    }
    assert_eq!(g.exec.broke, Some(20));
    let card = stack_fight::card(&g);
    assert_eq!(card.broke_words().as_deref(), Some("?BREAK IN 20"));
    assert!(btop_fight::note(&g).starts_with("?break in 20  cpu "));
    assert!(suite::module_note(&g).starts_with("?BREAK IN 20  CPU "));
    let w: std::collections::HashMap<_, _> = suite::watches(&g).into_iter().collect();
    assert_eq!(w["LAST"], "?BREAK IN 20");
    // The ▶ is held on the line, on every listing.
    let rows = listing::rows(&g);
    let held = rows.iter().find(|r| r.no == Some(20)).unwrap();
    assert_eq!(held.gutter, listing::Gutter::Current);
    // The next Step clears it.
    g.apply(Action::Step);
    assert_eq!(stack_fight::card(&g).broke_words(), None);
    assert!(btop_fight::note(&g).starts_with("cpu "));
    assert!(suite::module_note(&g).starts_with("CPU "));
}

#[test]
fn a_run_that_wins_the_fight_leaves_no_pass_open() {
    let mut g = fight_with(LOOP);
    g.combat.as_mut().unwrap().strength = 1;
    g.apply(Action::Step);
    g.apply(Action::Step);
    assert_eq!(g.phase, Phase::Exploring, "the RUN broke it");
    assert!(!g.exec.open);
    // An edit out of the fight puts the cursor at the top, not where the pass left it.
    g.apply(Action::Program(LOOP.into()));
    assert_eq!(g.exec.pc, Some(10));
}

#[test]
fn the_cursor_never_rests_on_a_rem() {
    let mut g = fight_with("10 REM setup\n20 RUN icepick\n30 GOTO 10");
    // At the top of a fight the ▶ is on the line that runs first.
    assert_eq!(g.exec.pc, Some(20));
    g.apply(Action::Step);
    let lines: Vec<u32> = g.exec.trace.iter().map(|s| s.line).collect();
    assert_eq!(lines, [20], "the line under the ▶, and only it");
    assert_eq!(g.exec.pc, Some(30));
    g.apply(Action::Step);
    assert_eq!(g.exec.pc, Some(20), "back round, past the REM");
    // At the top of the next turn too.
    g.apply(Action::EndTurn);
    assert_eq!(g.exec.pc, Some(20));
    // After a STOP, and after an edit.
    let mut g = fight_with("10 STOP\n20 REM after\n30 RUN icepick");
    g.apply(Action::Step);
    assert_eq!(g.exec.pc, Some(30));
    g.apply(Action::Program(
        "5 REM new\n10 STOP\n20 REM after\n30 RUN icepick".into(),
    ));
    assert_eq!(g.exec.pc, Some(30));
}
