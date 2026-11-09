//! The deck's BASIC: parsing and printing programs, and running them in a fight.

use ninsei_core::basic::{self, Cond, Move, Op, Stmt, Var};

const SAMPLE: &str = "10 RUN icepick\n20 IF INTEG < 10\n30 RUN patch\n40 GOTO 10";

#[test]
fn parses_the_sample() {
    let p = basic::parse(SAMPLE);
    let stmts: Vec<&Stmt> = p.lines.iter().map(|l| &l.stmt).collect();
    assert_eq!(
        stmts,
        [
            &Stmt::Run("icepick".into()),
            &Stmt::If(Cond::Cmp(Var::Integ, Op::Lt, 10)),
            &Stmt::Run("patch".into()),
            &Stmt::Goto(10),
        ]
    );
    assert_eq!(
        p.lines.iter().map(|l| l.no).collect::<Vec<_>>(),
        [10, 20, 30, 40]
    );
    assert!(p.lines.iter().all(|l| p.fault(l.no).is_none()));
}

#[test]
fn bad_lines_are_kept_and_named() {
    let p = basic::parse("10 RUN\n20 JUMP 10\n30 IF INTEG 10\n40 STOP");
    assert_eq!(p.lines.len(), 4, "{p:?}");
    for no in [10, 20, 30] {
        let why = p
            .fault(no)
            .unwrap_or_else(|| panic!("line {no} should be bad: {p:?}"));
        assert!(why.starts_with('?'), "{why}");
    }
    assert_eq!(p.line(40).map(|l| &l.stmt), Some(&Stmt::Stop));
}

#[test]
fn a_line_with_no_number_never_takes_anothers() {
    // Kept as typed, outside the program: it can't replace line 11 or be replaced by it.
    for text in ["11 RUN a\n10 RUN b\nRUN c", "10 RUN a\nRUN c\n11 RUN b"] {
        let p = basic::parse(text);
        assert_eq!(p.lines.len(), 2, "{text}: {p:?}");
        assert_eq!(p.stray, ["RUN c"], "{text}");
        // Printed back without a number, so a round trip never makes it a runnable line.
        let back = basic::parse(&p.text());
        assert_eq!(back, p, "{text}");
        assert!(back.lines.iter().all(|l| l.stmt != Stmt::Run("c".into())));
    }
    assert_eq!(basic::stray_fault("RUN c"), "?SYNTAX ERROR: no line number");
    // A number too big is a number all the same: it says so.
    let big = basic::parse("70000 RUN icepick");
    assert_eq!(big.stray, ["70000 RUN icepick"]);
    assert_eq!(basic::stray_fault(&big.stray[0]), "?ILLEGAL LINE NUMBER");
    let d = Data::builtin().unwrap();
    let why = hub::program_fault("70000 RUN icepick", &d).unwrap();
    assert!(why.contains("?ILLEGAL LINE NUMBER"), "{why}");
    let mut g = fight("bulwark", "10 RUN icepick\n70000 RUN icepick");
    let list = command::listing(&g);
    assert!(
        list.iter().any(|l| l.contains("?ILLEGAL LINE NUMBER")),
        "{list:?}"
    );
    g.apply(Action::Finish);
}

#[test]
fn text_round_trips() {
    let p = basic::parse(SAMPLE);
    assert_eq!(p.text(), SAMPLE);
    assert_eq!(basic::parse(&p.text()), p);
    let messy = basic::parse("40 GOTO 10\n  10   RUN icepick  \n20 IF INTEG<10\n10 RUN decoder");
    assert_eq!(messy.text(), "10 RUN decoder\n20 IF INTEG<10\n40 GOTO 10");
    assert_eq!(
        messy.line(20).map(|l| &l.stmt),
        Some(&Stmt::If(Cond::Cmp(Var::Integ, Op::Lt, 10)))
    );
}

#[test]
fn keywords_are_case_insensitive() {
    let p =
        basic::parse("10 run IcePick\n20 if next attack\n30 Goto 10\n40 stop\n50 if ice codegate");
    assert_eq!(
        p.line(10).map(|l| &l.stmt),
        Some(&Stmt::Run("IcePick".into()))
    );
    assert_eq!(
        p.line(20).map(|l| &l.stmt),
        Some(&Stmt::If(Cond::Next(Move::Attack)))
    );
    assert_eq!(p.line(30).map(|l| &l.stmt), Some(&Stmt::Goto(10)));
    assert_eq!(p.line(40).map(|l| &l.stmt), Some(&Stmt::Stop));
    assert_eq!(
        p.line(50).map(|l| &l.stmt),
        Some(&Stmt::If(Cond::IceClass(ninsei_core::IceClass::CodeGate)))
    );
}

#[test]
fn goto_to_a_missing_line_is_bad() {
    let mut p = basic::parse("10 RUN icepick\n20 GOTO 15");
    assert!(p.fault(20).is_some_and(|w| w.contains("15")));
    p.set_line(15, "STOP");
    assert_eq!(p.fault(20), None);
    p.remove(15);
    assert!(p.fault(20).is_some());
}

#[test]
fn a_new_line_takes_a_free_number_between_its_neighbours() {
    use basic::Program;
    assert_eq!(Program::free_line_between(Some(20), Some(30)), Some(25));
    assert_eq!(Program::free_line_between(Some(20), Some(21)), None);
    assert_eq!(Program::free_line_between(None, Some(10)), Some(5));
    assert_eq!(Program::free_line_between(None, Some(0)), None);
    assert_eq!(Program::free_line_between(Some(40), None), Some(50));
    assert_eq!(Program::free_line_between(None, None), Some(10));
    assert_eq!(
        Program::free_line_between(Some(basic::MAX_LINE), None),
        None,
        "nothing past the last number"
    );
    let p = basic::parse(SAMPLE);
    assert_eq!(p.next_line_after(20), Some(25), "between 20 and 30");
    assert_eq!(p.next_line_after(40), Some(50), "ten on at the end");
    let tight = basic::parse("10 RUN icepick\n11 GOTO 10");
    assert_eq!(tight.next_line_after(10), None, "no room between 10 and 11");
}

#[test]
fn edits_insert_replace_and_remove() {
    let mut p = basic::parse(SAMPLE);
    p.set_line(25, "REM patch when low");
    p.set_line(10, "RUN decoder");
    p.set_line(40, "");
    assert_eq!(
        p.text(),
        "10 RUN decoder\n20 IF INTEG < 10\n25 REM patch when low\n30 RUN patch"
    );
    assert_eq!(p.after(20), Some(25));
    assert_eq!(p.after(30), None);
}

// ---- running it --------------------------------------------------------------------------

use ninsei_core::game::{ExecState, StepWhat};
use ninsei_core::hub::Save;
use ninsei_core::*;

/// A bare run (the classic four on the HDD: icepick, firewall, patch, decoder) in a fight with
/// ICE `ice`, too strong to fall or give up, running `program`.
fn fight(ice: &str, program: &str) -> Game {
    let mut g = Game::new(Data::builtin().unwrap(), 7);
    let target = g.net.nodes[g.at].links[0];
    let ice = g.data.ice.iter().position(|i| i.id == ice).unwrap();
    g.net.nodes[target].kind = NodeKind::Ice;
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

fn combat(g: &Game) -> &Combat {
    g.combat.as_ref().expect("still fighting")
}

fn ran(g: &Game) -> usize {
    g.exec
        .trace
        .iter()
        .filter(|s| matches!(s.what, StepWhat::Ran { .. }))
        .count()
}

#[test]
fn a_pass_is_a_turn() {
    let mut g = fight("bulwark", SAMPLE);
    assert_eq!((g.deck.cpu_max, combat(&g).turn), (3, 1));
    assert!(
        g.program_view().lines[0].current,
        "the cursor on the first line"
    );
    g.apply(Action::Finish);
    // A backward GOTO keeps the pass going while cycles remain: three cycles, three RUNs.
    assert_eq!(ran(&g), 3, "{:?}", g.exec.trace);
    // Three icepicks for 3 each; then Bulwark, far from hurt, hits instead of fortifying.
    assert_eq!(500 - combat(&g).strength, 9);
    assert_eq!(g.deck.integrity, g.deck.integrity_max - 4);
    assert_eq!(g.exec.trace.last().unwrap().what, StepWhat::OutOfCpu);
    // Then the ICE acted once: the turn is over and the budgets are back.
    assert_eq!(combat(&g).turn, 2);
    assert_eq!((combat(&g).cpu, combat(&g).ifs), (3, g.deck.ram_max));
    assert_eq!(g.exec.state, ExecState::Paused);
}

#[test]
fn an_if_costs_a_gb_true_or_false() {
    let mut g = fight("bulwark", "10 IF INTEG < 100\n20 STOP");
    g.apply(Action::Finish);
    assert_eq!(combat(&g).ifs, g.deck.ram_max - 1, "a true IF");
    let mut g = fight("bulwark", "10 IF INTEG > 100\n20 RUN icepick\n30 STOP");
    g.apply(Action::Finish);
    assert_eq!(combat(&g).ifs, g.deck.ram_max - 1, "a false IF");
    assert_eq!(ran(&g), 0, "the false IF skipped its line");
    assert_eq!(g.exec.state, ExecState::Stopped(30));
    // GOTO and STOP are free.
    let mut g = fight("bulwark", "10 GOTO 30\n20 RUN icepick\n30 STOP");
    g.apply(Action::Finish);
    assert_eq!(combat(&g).ifs, g.deck.ram_max);
}

#[test]
fn two_ifs_in_a_row_are_an_and() {
    let mut g = fight(
        "bulwark",
        "10 IF INTEG > 100\n20 IF INTEG > 0\n30 RUN icepick\n40 STOP",
    );
    g.apply(Action::Finish);
    assert_eq!(ran(&g), 0, "the first IF skips the second and its line");
    assert_eq!(
        combat(&g).ifs,
        g.deck.ram_max - 1,
        "the skipped IF costs nothing"
    );
}

#[test]
fn out_of_ram_ends_the_pass() {
    let program =
        "10 IF TURN = 9\n20 STOP\n30 IF TURN = 9\n40 STOP\n50 IF TURN = 9\n60 STOP\n70 RUN icepick";
    let mut g = fight("bulwark", program);
    g.combat.as_mut().unwrap().ifs = 2;
    g.apply(Action::Finish);
    let last = g.exec.trace.last().unwrap();
    assert_eq!((last.line, &last.what), (50, &StepWhat::OutOfRam));
    assert_eq!(ran(&g), 0);
    assert_eq!(combat(&g).turn, 2, "the turn ended: the ICE acted");
}

#[test]
fn stop_hands_over_the_turn() {
    let mut g = fight("bulwark", "10 RUN icepick\n20 STOP\n30 RUN icepick");
    g.apply(Action::Finish);
    assert_eq!(g.exec.state, ExecState::Stopped(20));
    assert_eq!((combat(&g).turn, combat(&g).cpu), (1, 2), "no end of turn");
    assert!(g.log.iter().any(|l| l.text == "BREAK IN 20"));
    // The hotbar is live.
    let pick = g.slot_named("icepick").unwrap();
    g.apply(Action::Run(pick));
    assert_eq!(combat(&g).cpu, 1);
    // Step carries on after the STOP, in the same turn.
    g.apply(Action::Finish);
    assert_eq!(ran(&g), 2, "{:?}", g.exec.trace);
    assert_eq!(combat(&g).turn, 2);
}

#[test]
fn play_after_a_stop_carries_on_past_it() {
    let mut g = fight("bulwark", "10 RUN icepick\n20 STOP\n30 RUN icepick");
    g.apply(Action::Finish);
    assert_eq!(g.exec.state, ExecState::Stopped(20));
    g.apply(Action::Play);
    // Line 30 ran in the same turn, then the turn ended: the pass didn't start over at 10.
    let lines: Vec<u32> = g.exec.trace.iter().map(|s| s.line).collect();
    assert_eq!(&lines[..3], [10, 20, 30], "{:?}", g.exec.trace);
    assert_eq!(ran(&g), 2);
    assert_eq!((g.exec.state, combat(&g).turn), (ExecState::Running, 2));
}

#[test]
fn a_cycle_less_loop_breaks() {
    let mut g = fight("bulwark", "10 GOTO 10");
    g.apply(Action::Play);
    g.apply(Action::Finish);
    assert_eq!(g.exec.trace.last().unwrap().what, StepWhat::Break);
    assert!(g.log.iter().any(|l| l.text == "?BREAK IN 10"));
    assert_eq!(g.exec.state, ExecState::Paused, "the debugger has the turn");
    assert_eq!(combat(&g).turn, 1, "a hang doesn't end the turn");
    // A loop whose only RUN is guarded by a false IF spends a GB each time round: that's
    // progress, so it idles through the RAM and the turn ends, no hang.
    let mut g = fight("bulwark", "10 IF INTEG > 100\n20 RUN icepick\n30 GOTO 10");
    g.apply(Action::Finish);
    assert_eq!(g.exec.trace.last().unwrap().what, StepWhat::OutOfRam);
    assert!(!g.exec.trace.iter().any(|s| s.what == StepWhat::Break));
    assert_eq!(combat(&g).turn, 2, "the ICE acted");
    // A GOTO loop spends nothing, however long it is: still a hang.
    let mut g = fight("bulwark", "10 GOTO 20\n20 GOTO 30\n30 GOTO 10");
    g.apply(Action::Finish);
    let last = g.exec.trace.last().unwrap();
    assert_eq!((last.line, &last.what), (10, &StepWhat::Break));
    assert_eq!(combat(&g).turn, 1);
}

#[test]
fn a_run_that_cant_run_is_skipped_with_the_previews_words() {
    let mut g = fight("bulwark", "10 RUN lagspike\n20 RUN torch\n30 RUN icepick");
    let lag = g.data.warez_ix("lagspike").unwrap();
    let slot = g.factory_slot(lag).unwrap();
    g.hdd.push(slot);
    g.combat.as_mut().unwrap().cpu = 1;
    let why = g.preview(g.hdd.len() - 1).unwrap_err();
    g.apply(Action::Finish);
    let t = &g.exec.trace;
    assert_eq!((t[0].line, &t[0].what), (10, &StepWhat::Skipped(why)));
    assert!(
        matches!(&t[1].what, StepWhat::Skipped(w) if w.contains("torch")),
        "{t:?}"
    );
    assert!(matches!(t[2].what, StepWhat::Ran { .. }));
    assert_eq!(t[3].what, StepWhat::OutOfCpu);
}

#[test]
fn next_reads_the_intent() {
    use basic::Cond;
    let g = fight("rototurret", "");
    assert!(g.test(&Cond::Next(Move::Attack)));
    assert!(!g.test(&Cond::Next(Move::Fortify)));
    let mut g = fight(
        "icewall",
        "10 IF NEXT FORTIFY\n20 RUN firewall\n30 IF NEXT ATTACK\n40 STOP",
    );
    g.apply(Action::Finish);
    assert_eq!(
        (combat(&g).shield, g.exec.state),
        (0, ExecState::Stopped(40))
    );
    // Lagged ICE has no next move.
    let mut g = fight("rototurret", "");
    g.combat.as_mut().unwrap().stunned = true;
    assert!(!g.test(&Cond::Next(Move::Attack)));
}

#[test]
fn have_reads_the_hdd() {
    use basic::Cond;
    let g = fight("bulwark", "");
    assert!(g.test(&Cond::Have("decoder".into())));
    assert!(g.test(&Cond::Have("DECODER.EXE".into())));
    assert!(!g.test(&Cond::Have("torch".into())));
    assert!(g.test(&Cond::IceClass(IceClass::Barrier)));
}

#[test]
fn edits_apply_to_the_next_pass() {
    let mut g = fight("bulwark", "10 RUN icepick\n20 STOP");
    g.apply(Action::Finish);
    assert_eq!(g.exec.state, ExecState::Stopped(20));
    // Edited mid-pass: it carries on past the STOP into the new line.
    g.apply(Action::Program(
        "10 RUN icepick\n20 STOP\n30 RUN firewall".into(),
    ));
    g.apply(Action::Finish);
    assert_eq!(combat(&g).turn, 2);
    assert!(
        g.exec.trace.iter().any(|s| s.line == 30),
        "{:?}",
        g.exec.trace
    );
    // Between passes: the next one starts at the top of the new program.
    g.apply(Action::Program("10 RUN firewall\n20 STOP".into()));
    g.apply(Action::Finish);
    assert_eq!(
        (combat(&g).shield, g.exec.state),
        (5, ExecState::Stopped(20))
    );
}

#[test]
fn play_keeps_running_and_pause_hands_back() {
    let mut g = fight("bulwark", SAMPLE);
    g.apply(Action::Play);
    g.apply(Action::Finish);
    assert_eq!((g.exec.state, combat(&g).turn), (ExecState::Running, 2));
    g.apply(Action::Finish);
    assert_eq!((g.exec.state, combat(&g).turn), (ExecState::Running, 3));
    g.apply(Action::Pause);
    assert_eq!(g.exec.state, ExecState::Paused);
    // An empty program is the hand-played fight: a step is the end of the turn.
    g.apply(Action::Program(String::new()));
    g.apply(Action::Finish);
    assert_eq!(combat(&g).turn, 4);
    assert_eq!(g.exec.trace.last().unwrap().what, StepWhat::End);
}

#[test]
fn the_program_survives_a_run() {
    let d = Data::builtin().unwrap();
    let mut s = Save::default();
    s.ensure_key(0xC0FFEE);
    s.ensure_phonebook(&d, 0);
    let shipped = d.starts.program("").to_string();
    assert!(shipped.contains("SAFE"), "the default kit ships SAFE");
    assert_eq!(s.program_text(&d), shipped);
    let mut g = s.jack_in(&d, 5);
    assert_eq!(
        g.program,
        basic::parse(&shipped),
        "every fight opens with it loaded"
    );
    g.apply(Action::Program("10 RUN icepick\n20 GOTO 10".into()));
    g.phase = Phase::Flatlined;
    s.record(&g, &[], &d.rig).unwrap();
    assert_eq!(
        s.kit(&d).program,
        "10 RUN icepick\n20 GOTO 10",
        "kept, even on a flatline"
    );
    // A save from before programs loads with the kit's.
    let full = ron::to_string(&Save::default()).unwrap();
    assert!(full.contains("program:None,"), "{full}");
    let text = full.replace("program:None,", "");
    let back: Save = ron::from_str(&text).unwrap();
    assert_eq!(back.program, None);
}

#[test]
fn every_kit_ships_a_program_that_reads() {
    let d = Data::builtin().unwrap();
    for k in &d.starts.kits {
        let program = d.starts.program(&k.id);
        assert!(!program.is_empty(), "{}", k.id);
        assert_eq!(hub::program_fault(program, &d), None, "{}", k.id);
        // SAFE is written once: a kit without a program ships the default kit's.
        if k.program.is_empty() {
            assert_eq!(program, d.starts.kit("").program, "{}", k.id);
        }
    }
    assert!(d.starts.kits.iter().any(|k| k.program.is_empty()));
    let mut s = Save {
        kit: "classic".into(),
        ..Save::default()
    };
    assert_eq!(s.program_text(&d), d.starts.kit("").program);
    s.kit = "pick".into();
    assert_eq!(s.program_text(&d), d.starts.kit("").program);
    assert!(hub::program_fault("10 RUN nosuch", &d).is_some());
    // What a fight can run, validation accepts: name, file name or id, any case.
    let mut g = fight("bulwark", "");
    for typed in ["IcePick", "ICEPICK.EXE", "icepick"] {
        assert!(g.slot_named(typed).is_some(), "{typed}");
        let text = format!("10 RUN {typed}\n20 IF HAVE {typed}");
        assert_eq!(hub::program_fault(&text, &d), None, "{typed}");
    }
    g.apply(Action::Program("10 RUN icepick.exe".into()));
    g.apply(Action::Finish);
    assert_eq!(ran(&g), 1);
    assert!(hub::program_fault("10 GOTO 20", &d).is_some());
}

// ---- from the run terminal ---------------------------------------------------------------

use ninsei_core::command::{self, Command};

/// Types `line` into the run terminal and does what the app does with it. Returns what it
/// printed: the log's new lines.
fn type_in(g: &mut Game, line: &str) -> Vec<String> {
    let before = g.log.len();
    match command::parse_in(g, line).unwrap_or_else(|e| panic!("{line}: {e}")) {
        Command::Game(a) => g.apply(a),
        Command::Print(lines) => return lines,
        other => panic!("{line}: {other:?}"),
    }
    g.log[before..].iter().map(|l| l.text.clone()).collect()
}

#[test]
fn prog_edits_and_lists() {
    let mut g = fight("bulwark", SAMPLE);
    let list = type_in(&mut g, "prog");
    assert!(
        list[0].starts_with("turn 1  cpu 3/3  ram 6/6 GB"),
        "{list:?}"
    );
    assert_eq!(list[1], ">   10 RUN icepick", "the line about to run");
    assert_eq!(list.len(), 5);
    type_in(&mut g, "prog 25 RUN firewall");
    type_in(&mut g, "prog 30");
    type_in(&mut g, "prog 15 JUMP");
    assert_eq!(
        g.program.text(),
        "10 RUN icepick\n15 JUMP\n20 IF INTEG < 10\n25 RUN firewall\n40 GOTO 10"
    );
    type_in(&mut g, "prog del 15");
    assert!(g.program.line(15).is_none());
    // A bad line lists with the interpreter's reason under it.
    type_in(&mut g, "prog 50 GOTO 99");
    let list = type_in(&mut g, "list");
    assert!(
        list.iter().any(|l| l.contains("?UNDEF'D STATEMENT 99")),
        "{list:?}"
    );
    assert!(command::parse_in(&g, "prog x RUN icepick").is_err());
}

#[test]
fn finish_from_the_shell_says_the_pass_as_it_happens() {
    let mut g = fight("bulwark", SAMPLE);
    let said = type_in(&mut g, "finish");
    assert_eq!(said[0], "IcePick breaks 3", "{said:?}");
    assert_eq!(said[1], "20 IF INTEG < 10: no. skipping the next line.");
    // The pass's last line comes before what the ICE did after it.
    let over = said
        .iter()
        .position(|l| l == "out of cycles. the turn is over.");
    let ice = said
        .iter()
        .position(|l| l.contains("Bulwark 30 ATTACK 4: hits for 4."));
    assert!(over.is_some() && over < ice, "{said:?}");
    assert_eq!(combat(&g).turn, 2);
    // A STOP is said once, and carrying on says only what's new.
    let mut g = fight("bulwark", "10 IF INTEG > 0\n20 STOP\n30 GOTO 40\n40 STOP");
    let said = type_in(&mut g, "finish");
    assert_eq!(
        said.iter().filter(|l| *l == "BREAK IN 20").count(),
        1,
        "{said:?}"
    );
    let said = type_in(&mut g, "finish");
    assert_eq!(said, ["30 GOTO 40.", "BREAK IN 40"]);
    // A hang is said once too.
    let mut g = fight("bulwark", "10 GOTO 10");
    let said = type_in(&mut g, "finish");
    assert_eq!(
        said.iter().filter(|l| *l == "?BREAK IN 10").count(),
        1,
        "{said:?}"
    );
    // `play` keeps it running (the screen steps it a beat at a time) until the fight ends;
    // `pause` takes the turn back.
    let mut g = fight("icewall", SAMPLE);
    let c = g.combat.as_mut().unwrap();
    (c.strength, c.strength_max) = (20, 20);
    type_in(&mut g, "play");
    assert_eq!(g.exec.state, ExecState::Running);
    while g.phase == Phase::Combat && g.exec.state == ExecState::Running {
        g.apply(Action::Step);
    }
    assert_eq!(g.phase, Phase::Exploring);
    let mut g = fight("bulwark", SAMPLE);
    type_in(&mut g, "play");
    type_in(&mut g, "pause");
    assert_eq!(g.exec.state, ExecState::Paused);
    assert!(command::VERBS.contains(&"prog") && command::VERBS.contains(&"step"));
}

// ---- the ICE's vocabulary ------------------------------------------------------------

const ICE_VERBS: [basic::Verb; 3] = [
    basic::Verb::Attack,
    basic::Verb::Trace,
    basic::Verb::Fortify,
];

#[test]
fn an_ice_program_parses_and_prints_back() {
    use basic::{Target, Vocab};
    let text = "10 IF TARGET SHIELD > 0\n20 TRACE 4\n30 IF TARGET INTEG < 7\n40 ATTACK 5\n\
                50 IF TARGET CPU = 0\n60 FORTIFY 2\n70 IF TARGET RAN patch\n80 IF SELF STR < 10\n\
                90 IF TURN > 3\n100 STOP\n110 REM idle\n120 GOTO 10";
    let p = basic::parse_with(text, Vocab::Ice(&ICE_VERBS));
    let stmts: Vec<&Stmt> = p.lines.iter().map(|l| &l.stmt).collect();
    assert_eq!(
        stmts,
        [
            &Stmt::If(Cond::Target(Target::Shield, Op::Gt, 0)),
            &Stmt::Trace(4),
            &Stmt::If(Cond::Target(Target::Integ, Op::Lt, 7)),
            &Stmt::Attack(5),
            &Stmt::If(Cond::Target(Target::Cpu, Op::Eq, 0)),
            &Stmt::Fortify(2),
            &Stmt::If(Cond::TargetRan("patch".into())),
            &Stmt::If(Cond::SelfStr(Op::Lt, 10)),
            &Stmt::If(Cond::Cmp(Var::Turn, Op::Gt, 3)),
            &Stmt::Stop,
            &Stmt::Rem,
            &Stmt::Goto(10),
        ]
    );
    assert_eq!(p.text(), text);
    assert!(p.lines.iter().all(|l| p.fault(l.no).is_none()), "{p:?}");
}

#[test]
fn the_deck_cannot_say_what_an_ice_says() {
    let p = basic::parse("10 ATTACK 3\n20 IF TARGET SHIELD > 0\n30 IF SELF STR < 4");
    for no in [10, 20, 30] {
        assert!(p.fault(no).is_some(), "line {no} is the ICE's: {p:?}");
    }
}

#[test]
fn an_ice_cannot_say_what_the_deck_says() {
    let p = basic::parse_with(
        "10 RUN icepick\n20 IF INTEG < 7\n30 IF NEXT ATTACK\n40 IF HAVE patch",
        basic::Vocab::Ice(&ICE_VERBS),
    );
    for no in [10, 20, 30, 40] {
        assert!(p.fault(no).is_some(), "line {no} is the deck's: {p:?}");
    }
}

#[test]
fn an_ice_only_has_its_own_verbs() {
    let verbs = [basic::Verb::Attack];
    let p = basic::parse_with(
        "10 ATTACK 3\n20 TRACE 4\n30 ATTACK",
        basic::Vocab::Ice(&verbs),
    );
    assert_eq!(p.line(10).map(|l| &l.stmt), Some(&Stmt::Attack(3)));
    assert!(p.fault(20).is_some(), "TRACE isn't on its row");
    assert!(p.fault(30).is_some(), "a verb takes a number");
    let big = basic::parse_with("10 ATTACK 999\n20 ATTACK 1000", basic::Vocab::Ice(&verbs));
    assert_eq!(
        big.line(10).map(|l| &l.stmt),
        Some(&Stmt::Attack(basic::MAX_ARG))
    );
    assert!(big.fault(20).is_some(), "past MAX_ARG: {big:?}");
}
