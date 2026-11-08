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
        g.program_view().lines[0].current,
        "the cursor on the first line"
    );
    let cpu = g.deck.cpu_max;
    g.apply(Action::Step);
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
    g.apply(Action::Step);
    assert_eq!(g.combat.as_ref().unwrap().turn, 2, "the ICE acted once");
    // A whole turn: two IFs a loop, and the deck's RAM covers a loop per cycle.
    g.apply(Action::Step);
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
    g.apply(Action::Step);
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
    g.apply(Action::Step);
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
    assert_eq!(g.exec.state, ExecState::Stopped(20));
    let shield = g.slot_named("firewall").unwrap();
    g.apply(Action::Run(shield));
    assert_eq!(g.combat.as_ref().unwrap().shield, 10, "the hotbar is live");
    g.apply(Action::Step);
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
