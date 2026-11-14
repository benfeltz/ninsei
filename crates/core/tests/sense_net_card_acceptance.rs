//! 142 acceptance: the Sense/Net's Script card over real dialed runs on the starter deck,
//! with the probe's programs. A line the card greys at a turn's start never runs that turn;
//! the gauges are the turn's budgets; fast-forward is the same fight, only sooner.

use ninsei_core::game::{ExecState, StepWhat};
use ninsei_core::hub::Save;
use ninsei_core::probe::{CAREFUL, DUMB, SAFE_PICK};
use ninsei_core::stack_fight;
use ninsei_core::*;

/// A run on the Sense/Net from a fresh save, dialed on seed `seed`, its program `program`.
fn run(d: &Data, seed: u64, program: &str) -> Game {
    let mut s = Save {
        program: Some(program.into()),
        ..Save::default()
    };
    s.ensure_key(0x142);
    s.ensure_phonebook(d, 0);
    s.ensure_hardware(d);
    assert_eq!(s.deck, "sensenet");
    s.jack_in(d, seed)
}

/// Moves on, or leaves whoever's talking, until a fight starts or the run ends. `false`: the
/// run is over.
fn to_a_fight(g: &mut Game) -> bool {
    for _ in 0..200 {
        match g.phase {
            Phase::Combat => return true,
            Phase::Exploring => {
                let Some(&to) = g.reachable().first() else {
                    return false;
                };
                g.apply(Action::Move(to));
            }
            Phase::Encounter => {
                let Some(c) = g.leave_choice() else {
                    return false;
                };
                g.apply(Action::Choose {
                    choice: c,
                    slot: None,
                });
                if g.encounter.as_ref().is_some_and(|e| e.done) {
                    g.apply(Action::EndEncounter);
                }
            }
            Phase::JackedOut | Phase::Flatlined => return false,
        }
    }
    false
}

/// Plays one turn a Step at a time, checking the card's gauges against the budgets at every
/// step; returns the lines it executed. A STOP or a hang hands the turn over, and it's ended.
fn play_turn(g: &mut Game) -> Vec<u32> {
    let c = g.combat.as_ref().unwrap();
    let (node, turn) = (c.node, c.turn);
    let mut went = Vec::new();
    for _ in 0..1000 {
        let mark = g.deck_mark();
        g.apply(Action::Step);
        went.extend(g.deck_steps_since(mark).iter().filter_map(|s| s.executed()));
        let Some(c) = g.combat.as_ref().filter(|_| g.phase == Phase::Combat) else {
            return went;
        };
        if (c.node, c.turn) != (node, turn) {
            return went;
        }
        let gauges = stack_fight::card(g).budgets.gauges();
        assert_eq!(gauges[0], ("CPU", c.cpu, g.deck.cpu_max));
        assert_eq!(gauges[1], ("RAM", c.ifs, g.deck.ram_max));
        if matches!(g.exec.state, ExecState::Stopped(_)) || g.exec.broke.is_some() {
            g.apply(Action::EndTurn);
            return went;
        }
    }
    panic!("a turn that never ends");
}

#[test]
fn a_greyed_line_never_runs_that_turn_on_real_runs() {
    let d = Data::builtin().unwrap();
    let (mut turns, mut greyed) = (0, 0);
    for program in [DUMB, CAREFUL, SAFE_PICK] {
        for seed in 0..12u64 {
            let mut g = run(&d, 0x1420 + seed, program);
            while to_a_fight(&mut g) {
                let starved = g.starved();
                // The card greys exactly the forecast.
                let card = stack_fight::card(&g);
                let grey: Vec<u32> = card
                    .blocks
                    .iter()
                    .filter(|b| b.starved)
                    .map(|b| b.no)
                    .collect();
                assert_eq!(grey, starved);
                let went = play_turn(&mut g);
                turns += 1;
                greyed += starved.len();
                for no in &starved {
                    assert!(
                        !went.contains(no),
                        "seed {seed}: {no} greyed and ran ({went:?})"
                    );
                }
                if turns > 3000 {
                    break;
                }
            }
            // Outside a fight the gauges are full.
            if g.phase == Phase::Exploring {
                let b = stack_fight::card(&g).budgets;
                assert_eq!(b.gauges()[0].1, g.deck.cpu_max);
                assert_eq!(b.gauges()[1].1, g.deck.ram_max);
            }
        }
    }
    assert!(turns > 200, "{turns} turns");
    assert!(greyed > 20, "{greyed} lines greyed");
}

#[test]
fn a_fight_on_fast_forward_ends_as_it_does_at_pace_on_real_runs() {
    let d = Data::builtin().unwrap();
    let mut fights = 0;
    for seed in 0..10u64 {
        let mut g = run(&d, 0x1428 + seed, SAFE_PICK);
        if !to_a_fight(&mut g) {
            continue;
        }
        let mut slow = g.clone();
        let mut fast = g;
        slow.apply(Action::Play);
        fast.apply(Action::FastForward);
        assert!(fast.fast_forward());
        // The driver's beat, a Step while running: as many as the fight takes.
        for g in [&mut slow, &mut fast] {
            for _ in 0..5000 {
                if g.phase != Phase::Combat || g.exec.state != ExecState::Running {
                    break;
                }
                g.apply(Action::Step);
            }
        }
        assert!(
            !fast.fast_forward(),
            "seed {seed}: FF pops up when the program stops"
        );
        fast.exec.fast = slow.exec.fast;
        assert_eq!(format!("{fast:?}"), format!("{slow:?}"), "seed {seed}");
        let ran = |g: &Game| {
            g.exec
                .trace
                .iter()
                .filter(|s| matches!(s.what, StepWhat::Ran { .. }))
                .count()
        };
        assert_eq!(ran(&fast), ran(&slow));
        fights += 1;
    }
    assert!(fights >= 5, "{fights} fights");
}
