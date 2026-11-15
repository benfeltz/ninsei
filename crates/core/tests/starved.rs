//! 142: the forecast of starved lines (`Game::starved`). A dry run of the rest of the turn's
//! pass that changes nothing, and a line it forecasts is never one the turn then executes.

use ninsei_core::game::{ExecState, Kit, StepWhat};
use ninsei_core::*;

/// A run with these programs on the disk and `cpu` cycles a turn.
fn run_with(seed: u64, cpu: u32, ram: u32, programs: &[&str]) -> Game {
    let kit = Kit {
        cpu,
        ram,
        hdd: 12,
        tools: ninsei_core::data::TOOLS
            .iter()
            .map(|t| t.to_string())
            .collect(),
        free: programs.iter().map(|t| t.to_string()).collect(),
        ..Kit::default()
    };
    Game::start(Data::builtin().unwrap(), seed, &kit)
}

/// Into a fight with ICE `ice` from where the run starts.
fn into_fight(g: &mut Game, ice: usize, strength: Option<u32>) {
    let target = g.net.nodes[g.at].links[0];
    g.net.nodes[target].kind = NodeKind::Ice;
    g.net.nodes[target].ice = Some(ice);
    g.net.nodes[target].cleared = false;
    g.apply(Action::Move(target));
    assert_eq!(g.phase, Phase::Combat);
    if let Some(s) = strength {
        let c = g.combat.as_mut().unwrap();
        (c.strength, c.strength_max) = (s, s);
    }
}

fn turn(g: &Game) -> u32 {
    g.combat.as_ref().map_or(0, |c| c.turn)
}

/// `n` lines of `RUN icepick` from line `from`, ten apart.
fn picks(from: u32, n: u32) -> String {
    (0..n)
        .map(|i| format!("{} RUN icepick\n", from + i * 10))
        .collect()
}

#[test]
fn a_run_with_no_cycles_left_for_it_is_starved_from_the_turns_start() {
    let mut g = run_with(7, 3, 6, &["icepick", "torch"]);
    into_fight(&mut g, 1, Some(500));
    let t = turn(&g);
    // This turn the IF is true: the guarded pick and one more leave the torch at 100 one
    // cycle of the two it needs. Next turn the IF is false and the torch has two.
    g.apply(Action::Program(format!(
        "10 IF TURN = {t}\n20 RUN icepick\n{}100 RUN torch",
        picks(30, 1)
    )));
    assert_eq!(g.starved(), [100], "starved before a line has run");
    g.apply(Action::Step);
    assert_eq!(g.starved(), [100], "and as the pass goes on");
    g.apply(Action::Finish);
    assert_eq!(turn(&g), t + 1);
    assert_eq!(g.starved(), Vec::<u32>::new(), "next turn the torch runs");
    // ... and so it does; the false IF's guarded line was never starved.
    g.apply(Action::Finish);
    assert!(
        g.exec
            .trace
            .iter()
            .any(|s| s.line == 100 && matches!(s.what, StepWhat::Ran { .. }))
    );
}

#[test]
fn the_lines_after_the_last_cycle_are_starved_too() {
    let mut g = run_with(7, 3, 6, &["icepick", "patch", "firewall"]);
    into_fight(&mut g, 1, Some(500));
    // Three picks spend the turn: the patch and the firewall never come.
    g.apply(Action::Program(format!(
        "{}40 RUN patch\n50 IF INTEG > 0\n60 RUN firewall",
        picks(10, 3)
    )));
    assert_eq!(g.starved(), [40, 50, 60]);
    // With GB for one IF, the pass ends on the second; it and what it leads to are starved.
    let mut g = run_with(7, 3, 1, &["icepick"]);
    into_fight(&mut g, 1, Some(500));
    g.apply(Action::Program(
        "10 IF INTEG > 0\n20 RUN icepick\n30 IF INTEG > 0\n40 RUN icepick".into(),
    ));
    assert_eq!(g.starved(), [30, 40]);
}

#[test]
fn a_line_behind_a_false_if_is_not_starved() {
    let mut g = run_with(7, 3, 6, &["icepick", "patch"]);
    into_fight(&mut g, 1, Some(500));
    // INTEG is never above 999: the patch is skipped, not starved, even with the turn spent.
    g.apply(Action::Program(format!(
        "{}40 IF INTEG > 999\n50 RUN patch",
        picks(10, 3)
    )));
    assert_eq!(
        g.starved(),
        [40],
        "the IF itself is starved; its guarded line isn't"
    );
    g.apply(Action::Program(
        "10 IF INTEG > 999\n20 RUN patch\n30 RUN icepick".into(),
    ));
    assert_eq!(g.starved(), Vec::<u32>::new());
}

#[test]
fn a_program_not_on_the_disk_is_starved_in_and_out_of_a_fight() {
    let mut g = run_with(7, 3, 6, &["icepick"]);
    let text = "10 RUN torch\n20 RUN icepick\n30 IF INTEG > 999\n40 RUN wyrm";
    g.apply(Action::Program(text.into()));
    assert_eq!(g.phase, Phase::Exploring);
    assert_eq!(g.starved(), [10, 40], "outside a fight: what can never run");
    into_fight(&mut g, 1, Some(500));
    assert_eq!(g.starved(), [10, 40], "in one: the same, wherever it is");
    // A program that costs more than the deck's cycles a turn can never run either.
    let mut g = run_with(7, 1, 6, &["icepick", "torch"]);
    g.apply(Action::Program("10 RUN torch\n20 RUN icepick".into()));
    assert_eq!(g.starved(), [10]);
}

#[test]
fn the_line_under_the_cursor_starves_when_the_hotbar_spent_the_cycles() {
    let mut g = run_with(7, 3, 6, &["icepick"]);
    into_fight(&mut g, 1, Some(500));
    g.apply(Action::Program(
        "10 IF INTEG > 0\n20 RUN icepick\n30 RUN icepick".into(),
    ));
    g.apply(Action::Step);
    assert_eq!(g.exec.pc, Some(20));
    let pick = g.slot_named("icepick").unwrap();
    for _ in 0..3 {
        g.apply(Action::Run(pick));
    }
    assert_eq!(g.combat.as_ref().unwrap().cpu, 0);
    assert_eq!(g.starved(), [20, 30], "the line under the ▶ too");
}

#[test]
fn the_walk_past_the_last_cycle_reads_what_the_pass_did() {
    let mut g = run_with(7, 3, 6, &["icepick", "patch", "firewall"]);
    into_fight(&mut g, 1, Some(500));
    g.deck.integrity = 4;
    // The patch lifts INTEG to 7 before the cycles run out, so the IF the walk comes to is
    // false and its firewall is behind it, not starved.
    g.apply(Action::Program(
        "10 RUN patch\n20 RUN icepick\n30 RUN icepick\n40 IF INTEG < 5\n50 RUN firewall".into(),
    ));
    assert_eq!(g.starved(), [40]);
}

#[test]
fn the_forecast_never_rolls_whether_the_ice_quits() {
    let data = Data::builtin().unwrap();
    let rival = data.ice.iter().position(|i| i.id == "rival").unwrap();
    assert!(data.ice[rival].yields.is_some());
    let program = picks(10, 4);
    // Strong enough that no pick this turn could make it quit: the fourth pick starves.
    let mut g = run_with(7, 3, 6, &["icepick"]);
    into_fight(&mut g, rival, Some(500));
    g.apply(Action::Program(program.clone()));
    assert_eq!(g.starved(), [40]);
    // One pick takes it under its yield line: whether it quits is the turn's roll, so the
    // forecast stops there, whatever the roll would say.
    for seed in 0..20 {
        let mut g = run_with(seed, 3, 6, &["icepick"]);
        into_fight(&mut g, rival, None);
        let c = g.combat.as_mut().unwrap();
        (c.strength, c.strength_max) = (12, 20);
        g.apply(Action::Program(program.clone()));
        assert_eq!(g.starved(), Vec::<u32>::new(), "seed {seed}");
    }
}

#[test]
fn the_forecast_never_reads_a_next_its_own_runs_may_have_changed() {
    let data = Data::builtin().unwrap();
    let hunter = data.ice.iter().position(|i| i.id == "hunter").unwrap();
    let mut g = run_with(7, 3, 6, &["icepick", "patch", "firewall"]);
    into_fight(&mut g, hunter, Some(500));
    // Hunter's unseen `10 IF TARGET RAN patch` turns its NEXT to an attack once the patch has
    // run: the forecast mustn't show it before the screen does, so it stops at the IF.
    g.apply(Action::Program(
        "10 RUN patch\n20 IF NEXT ATTACK\n30 RUN firewall\n40 RUN icepick\n50 RUN icepick".into(),
    ));
    assert_eq!(g.starved(), Vec::<u32>::new());
    // A NEXT read before any RUN is what's on screen now: the forecast goes on.
    g.apply(Action::Program(
        "10 IF NEXT ATTACK\n20 RUN firewall\n30 RUN icepick\n40 RUN icepick\n50 RUN icepick\n60 RUN icepick"
            .into(),
    ));
    assert_eq!(g.starved(), [60]);
}

/// A screen keeps the forecast while `forecast_key` holds: through real play (Steps, hotbar
/// runs, edits), an unchanged key never hides a changed forecast.
#[test]
fn the_forecast_key_moves_whenever_the_forecast_could() {
    let data = Data::builtin().unwrap();
    // Keyed by the ICE too: a screen keeps its key through one fight, against one ICE.
    let mut seen: Vec<((usize, game::ForecastKey), Vec<u32>)> = Vec::new();
    let mut check = |g: &Game| {
        let ice = g.combat.as_ref().map_or(usize::MAX, |c| c.ice);
        let (key, starved) = ((ice, g.forecast_key()), g.starved());
        if let Some((_, was)) = seen.iter().find(|(k, _)| *k == key) {
            assert_eq!(*was, starved, "same key, another forecast");
        } else {
            seen.push((key, starved));
        }
    };
    for program in probe::ROTATION {
        for seed in 0..3u64 {
            for ice in 0..data.ice.len() {
                let mut g = run_with(
                    seed + ice as u64,
                    3,
                    6,
                    &["icepick", "patch", "firewall", "torch"],
                );
                g.apply(Action::Program(program.to_string()));
                into_fight(&mut g, ice, None);
                for i in 0..40 {
                    if g.phase != Phase::Combat {
                        break;
                    }
                    check(&g);
                    match i % 7 {
                        3 => {
                            let pick = g.slot_named("icepick").unwrap();
                            g.apply(Action::Run(pick));
                        }
                        5 => g.apply(Action::Program(format!("{program}\n9 RUN torch"))),
                        _ => g.apply(Action::Step),
                    }
                }
            }
        }
    }
    assert!(seen.len() > 200, "{} keys", seen.len());
}

#[test]
fn the_forecast_changes_nothing() {
    let mut g = run_with(11, 3, 6, &["icepick", "patch", "firewall", "torch"]);
    into_fight(&mut g, 1, None);
    g.apply(Action::Program(format!(
        "10 IF INTEG < 99\n20 RUN patch\n{}90 RUN torch\n100 GOTO 10",
        picks(30, 3)
    )));
    g.apply(Action::Step);
    let before = format!("{g:?}");
    let first = g.starved();
    assert_eq!(format!("{g:?}"), before, "state and RNG as they were");
    assert_eq!(g.starved(), first, "and asked again, the same answer");
}

/// Plays a turn a Step at a time, as the driver does, and returns the lines it executed. A
/// STOP or a hang hands the turn over and the turn is ended there, as the probe's bots do.
fn play_turn(g: &mut Game) -> Vec<u32> {
    let t = turn(g);
    let ice = g.combat.as_ref().map(|c| c.node);
    let mut went = Vec::new();
    for _ in 0..1000 {
        let mark = g.deck_mark();
        g.apply(Action::Step);
        went.extend(g.deck_steps_since(mark).iter().filter_map(|s| s.executed()));
        if g.phase != Phase::Combat || turn(g) != t || g.combat.as_ref().map(|c| c.node) != ice {
            return went;
        }
        if matches!(g.exec.state, ExecState::Stopped(_)) || g.exec.broke.is_some() {
            g.apply(Action::EndTurn);
            return went;
        }
    }
    panic!("a turn that never ends");
}

/// The property: over the probe's three bot programs (and the careful bot's other two), across
/// seeded fights with every ICE and a range of decks, every line forecast as starved at a
/// turn's start is one the turn then doesn't execute.
#[test]
fn what_is_forecast_starved_never_runs_that_turn() {
    let data = Data::builtin().unwrap();
    let mut programs: Vec<&str> = probe::Bot::ALL.iter().map(|b| b.program()).collect();
    programs.extend(probe::ROTATION);
    let disk = [
        "icepick", "decoder", "garrote", "crowbar", "wyrm", "torch", "firewall", "patch",
        "scrubber",
    ];
    let (mut turns, mut greyed) = (0, 0);
    for (pi, program) in programs.iter().enumerate() {
        for seed in 0..6u64 {
            for ice in 0..data.ice.len() {
                let cpu = 2 + (seed as u32 + ice as u32) % 5;
                let ram = 2 + (seed as u32 * 3 + pi as u32) % 6;
                // Half the decks carry everything; the rest only the starter's four.
                let carried = if (seed + ice as u64).is_multiple_of(2) {
                    &disk[..]
                } else {
                    &["icepick", "decoder", "firewall", "patch"][..]
                };
                let mut g = run_with(seed * 31 + ice as u64, cpu, ram, carried);
                g.apply(Action::Program(program.to_string()));
                into_fight(&mut g, ice, None);
                // Some fights start hurt, so the patches and firewalls have their say.
                if seed % 3 == 1 {
                    g.deck.integrity = g.deck.integrity.min(6);
                }
                for _ in 0..12 {
                    if g.phase != Phase::Combat {
                        break;
                    }
                    let starved = g.starved();
                    let went = play_turn(&mut g);
                    turns += 1;
                    greyed += starved.len();
                    for no in &starved {
                        assert!(
                            !went.contains(no),
                            "program {pi}, seed {seed}, ICE {ice}: line {no} was forecast \
                             starved and ran (starved {starved:?}, ran {went:?})"
                        );
                    }
                }
            }
        }
    }
    assert!(turns > 500, "{turns} turns played");
    assert!(
        greyed > 100,
        "the forecast greyed {greyed} lines: not vacuous"
    );
}
