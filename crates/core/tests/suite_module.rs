//! The Hosaka's Module window: what it shows (the listing's marks in the suite's glyphs, the
//! budgets, the Watches), the F-key legend that changes with the active window, and each step
//! of a pass filed in the activity log under its own reference.

use ninsei_core::game::ExecState;
use ninsei_core::hub::Save;
use ninsei_core::listing::{self, Gutter};
use ninsei_core::suite::{self, FKey, Legend};
use ninsei_core::*;

fn save() -> (Data, Save) {
    let d = Data::builtin().unwrap();
    let mut s = Save::default();
    s.ensure_key(0x5AFE);
    s.ensure_phonebook(&d, 0);
    s.ensure_hardware(&d);
    (d, s)
}

/// A run in an incident with ICE too strong to fall.
fn fight() -> Game {
    let (d, mut s) = save();
    let mut g = s.jack_in(&d, 21);
    let target = g.net.nodes[g.at].links[0];
    g.net.nodes[target].kind = NodeKind::Ice;
    g.net.nodes[target].ice = Some(0);
    g.net.nodes[target].cleared = false;
    g.apply(Action::Move(target));
    let c = g.combat.as_mut().unwrap();
    (c.strength, c.strength_max) = (500, 500);
    g
}

#[test]
fn the_watches_read_the_incident_and_are_blank_outside_one() {
    let (d, mut s) = save();
    let calm = s.jack_in(&d, 21);
    let w = suite::watches(&calm);
    let names: Vec<&str> = w.iter().map(|(n, _)| *n).collect();
    assert_eq!(
        names,
        [
            "TURN", "CPU", "IFS", "INTEG", "TRACE", "NEXT", "ICE", "LAST"
        ]
    );
    assert!(w.iter().all(|(_, v)| v.is_empty()), "{w:?}");

    let mut g = fight();
    g.apply(Action::Program(
        "10 IF INTEG > 0\n20 RUN icepick\n30 STOP".into(),
    ));
    g.apply(Action::Finish);
    let w: std::collections::HashMap<_, _> = suite::watches(&g).into_iter().collect();
    let c = g.combat.as_ref().unwrap();
    assert_eq!(w["TURN"], c.turn.to_string());
    assert_eq!(w["CPU"], format!("{} / {}", c.cpu, g.deck.cpu_max));
    assert_eq!(w["IFS"], format!("{} / {}", c.ifs, g.deck.ram_max));
    assert_eq!(w["NEXT"], g.intent().unwrap().describe());
    assert_eq!(w["ICE"], g.data.ice[c.ice].name.to_uppercase());
    let last = g.exec.trace.last().unwrap();
    assert_eq!(
        w["LAST"],
        g.step_text(last),
        "the last step, as the log said it"
    );
}

#[test]
fn the_module_marks_the_line_about_to_run_and_the_stops() {
    let mut g = fight();
    g.apply(Action::Program(
        "10 RUN icepick\n20 STOP\n30 JUMP 10".into(),
    ));
    let rows = listing::rows(&g);
    assert_eq!(suite::module_mark(rows[0].gutter), "▶");
    assert_eq!(rows[1].gutter, Gutter::Stop);
    assert_eq!(suite::module_mark(rows[1].gutter), "■");
    assert!(rows[2].fault.is_some());
    let c = g.combat.as_ref().unwrap();
    assert_eq!(
        suite::module_note(&g),
        format!(
            "CPU {}/{}  RAM {}/{}",
            c.cpu, g.deck.cpu_max, c.ifs, g.deck.ram_max
        )
    );
    assert_eq!(suite::MODULE, "PROC-7.BAS");
}

#[test]
fn the_legend_changes_with_the_active_window_and_leaves_the_browser_its_keys() {
    let module: Vec<u8> = Legend::Module.keys().iter().map(|k| k.0).collect();
    assert_eq!(module, [1, 2, 3, 4, 8, 9, 10]);
    let default: Vec<(u8, FKey)> = Legend::Default.keys().iter().map(|k| (k.0, k.1)).collect();
    assert_eq!(
        default,
        [
            (1, FKey::Help),
            (4, FKey::Hex),
            (8, FKey::Jack),
            (10, FKey::EndTurn)
        ],
        "outside the Module window, F4 and F8 keep their meaning"
    );
    for legend in [Legend::Module, Legend::Default] {
        for (n, _, _) in legend.keys() {
            assert!(![5, 6, 7, 11, 12].contains(n), "F{n} is the browser's");
        }
    }
    let labels: Vec<&str> = Legend::Module.keys().iter().map(|k| k.2).collect();
    assert_eq!(
        labels,
        ["Help", "Edit", "Insert", "Pause", "Step", "Run", "End turn"]
    );
}

#[test]
fn the_debuggers_keys_press_only_when_they_do_something() {
    let (d, mut s) = save();
    let calm = s.jack_in(&d, 21);
    let first = calm.program.first();
    assert!(!suite::fkey_live(&calm, FKey::Step, first), "no incident");
    assert!(
        suite::fkey_live(&calm, FKey::Edit, first),
        "editing works between incidents"
    );
    let mut g = fight();
    let first = g.program.first();
    assert!(suite::fkey_live(&g, FKey::Step, first) && suite::fkey_live(&g, FKey::Run, first));
    assert!(!suite::fkey_live(&g, FKey::Pause, first));
    g.apply(Action::Program("10 STOP\n20 GOTO 10".into()));
    g.apply(Action::Finish);
    assert!(
        suite::fkey_live(&g, FKey::Run, None),
        "F9 carries on after a STOP"
    );
}

#[test]
fn each_step_of_a_pass_is_filed_under_its_own_reference() {
    let mut g = fight();
    let before = g.log.len();
    g.apply(Action::Program(
        "10 IF INTEG > 0\n20 RUN icepick\n30 GOTO 10".into(),
    ));
    g.apply(Action::Finish);
    let steps: Vec<_> = g.log[before..]
        .iter()
        .filter(|l| l.kind == LogKind::Program)
        .collect();
    assert!(!steps.is_empty());
    assert!(steps.iter().any(|l| l.text.starts_with("10 IF INTEG > 0")));
    assert_eq!(suite::log_ref(LogKind::Program), "PRG");
    let kinds = [
        LogKind::Info,
        LogKind::Good,
        LogKind::Bad,
        LogKind::Loot,
        LogKind::Sys,
        LogKind::Program,
    ];
    let mut refs: Vec<&str> = kinds.iter().map(|k| suite::log_ref(*k)).collect();
    refs.sort();
    refs.dedup();
    assert_eq!(refs.len(), kinds.len(), "every kind its own reference");
}

/// 141.6 acceptance: one program across a run through the Module window on a Hosaka. Between
/// incidents the Watches are blank, the debugger's F-keys dark and the editor's live; a line
/// opened and one retyped go in through `listing`; in an incident the Watches fill, F8 steps
/// to the STOP, F9 carries on, each spoken step is filed PRG; the deck keeps what was left.
#[test]
fn proc_7_is_one_program_before_during_and_after_an_incident() {
    let (d, mut s) = save();
    s.decks.push("hosaka".into());
    s.deck = "hosaka".into();
    s.firmware = s.deck_def(&d).firmware[0].id.clone();
    let mut g = s.jack_in(&d, 21);
    assert!(suite::watches(&g).iter().all(|(_, v)| v.is_empty()));
    let first = g.program.first();
    assert!(!suite::fkey_live(&g, FKey::Step, first));
    assert!(suite::fkey_live(&g, FKey::Edit, first) && suite::fkey_live(&g, FKey::Open, first));
    // Insert a RUN inside SAFE's loop, and make the first line a STOP.
    let goto = g.program.lines.last().unwrap().no;
    let before_goto = g
        .program
        .lines
        .iter()
        .map(|l| l.no)
        .take_while(|&n| n < goto)
        .last();
    let at = listing::open_after(&g, before_goto).unwrap();
    g.apply(listing::retype(&g, at, "RUN icepick").unwrap());
    let first = first.unwrap();
    g.apply(listing::retype(&g, first, "STOP").unwrap());
    let edited = g.program.text();
    // Into ICE.
    let target = g.net.nodes[g.at].links[0];
    g.net.nodes[target].kind = NodeKind::Ice;
    g.net.nodes[target].ice = Some(0);
    g.net.nodes[target].cleared = false;
    g.apply(Action::Move(target));
    assert_eq!(g.phase, Phase::Combat);
    let c = g.combat.as_mut().unwrap();
    (c.strength, c.strength_max) = (500, 500);
    assert_eq!(
        g.program.text(),
        edited,
        "the incident opens with what was edited"
    );
    assert_eq!(suite::module_mark(listing::rows(&g)[0].gutter), "▶");
    let w: std::collections::HashMap<_, _> = suite::watches(&g).into_iter().collect();
    assert!(!w["ICE"].is_empty() && !w["NEXT"].is_empty());
    // F8 Step hits the STOP; F9 Run carries on past it and the ICE loses strength.
    assert!(suite::fkey_live(&g, FKey::Step, Some(first)));
    let logged = g.log.len();
    g.apply(Action::Finish);
    assert!(matches!(g.exec.state, ExecState::Stopped(_)));
    assert!(suite::fkey_live(&g, FKey::Run, None));
    let before = g.combat.as_ref().unwrap().strength;
    g.apply(Action::Play);
    g.apply(Action::Finish);
    g.apply(Action::Pause);
    assert!(g.combat.as_ref().map_or(0, |c| c.strength) < before);
    assert!(
        g.log[logged..]
            .iter()
            .any(|l| suite::log_ref(l.kind) == "PRG"),
        "the pass is filed PRG"
    );
    // The deck keeps the program.
    let kept = g.program.text();
    g.phase = Phase::JackedOut;
    s.record(&g, &[], &d.rig).unwrap();
    assert_eq!(s.program_text(&d), kept);
}
