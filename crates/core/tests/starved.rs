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

/// The lines a step executed: RUNs that ran, IFs, GOTOs and STOPs.
fn executed(s: &ninsei_core::game::Step) -> Option<u32> {
    matches!(
        s.what,
        StepWhat::Ran { .. } | StepWhat::If(_) | StepWhat::Goto(_) | StepWhat::Stop
    )
    .then_some(s.line)
}

/// Plays a turn a Step at a time, as the driver does, and returns the lines it executed. A
/// STOP or a hang hands the turn over and the turn is ended there, as the probe's bots do.
fn play_turn(g: &mut Game) -> Vec<u32> {
    let t = turn(g);
    let ice = g.combat.as_ref().map(|c| c.node);
    let mut went = Vec::new();
    for _ in 0..1000 {
        let from = if g.exec.open { g.exec.trace.len() } else { 0 };
        g.apply(Action::Step);
        let trace = &g.exec.trace;
        let new = if trace.len() < from || g.combat.as_ref().map(|c| c.node) != ice {
            &trace[..]
        } else {
            &trace[from..]
        };
        went.extend(new.iter().filter_map(executed));
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
