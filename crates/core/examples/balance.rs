//! Balance probe: plays many seeds with a dumb bot and prints the outcome spread,
//! on the default network and on a plain (no-trait, no-heat) network of each tier.
//! `cargo run -p ninsei-core --example balance`
use ninsei_core::hub::{Entry, Save};
use ninsei_core::*;

const RUNS: u64 = 500;

/// Breaks with the first breaker it can afford, never shields, walks away from every encounter.
fn play(mut g: Game) -> (Game, u32) {
    let mut turns = 0;
    loop {
        match g.phase {
            Phase::Exploring => {
                let to = g.reachable()[0];
                g.apply(Action::Move(to));
            }
            Phase::Combat => {
                let cpu = g.combat.as_ref().unwrap().cpu;
                let pick = g.hdd.iter().enumerate().find(|(_, s)| {
                    let w = &s.spec;
                    s.loaded && w.cpu <= cpu && matches!(w.effect, Effect::Break { .. })
                });
                match pick {
                    Some((i, _)) => g.apply(Action::Run(i)),
                    None => {
                        turns += 1;
                        g.apply(Action::EndTurn)
                    }
                }
            }
            Phase::Encounter => {
                let leave = g.leave_choice().unwrap();
                g.apply(Action::Choose {
                    choice: leave,
                    slot: None,
                });
            }
            _ => return (g, turns),
        }
    }
}

fn report(label: &str, mut new_game: impl FnMut(u64) -> Game) {
    let (mut wins, mut hp, mut trace, mut turns, mut credits) = (0, 0, 0, 0, 0);
    for seed in 0..RUNS {
        let (g, t) = play(new_game(seed));
        turns += t;
        if g.phase == Phase::JackedOut {
            wins += 1;
            hp += g.deck.integrity;
            trace += g.deck.trace;
            credits += g.deck.credits;
        }
    }
    let w = wins.max(1);
    println!(
        "{label:<10} win {wins:>3}/{RUNS}  avg hp left {:>2}  avg trace {:>2}  ice turns/run {}  avg haul {}¢",
        hp / w,
        trace / w,
        turns / RUNS as u32,
        credits / w
    );
}

fn main() {
    let data = Data::builtin().unwrap();
    report("default", |seed| Game::new(data.clone(), seed));
    for tier in 1..=3 {
        let mut save = Save::default();
        save.phonebook.push(Entry {
            id: 0,
            name: "probe".into(),
            business: "test".into(),
            number: "555-0000".into(),
            tier,
            traits: Vec::new(),
            heat: 0,
            runs: 0,
        });
        report(&format!("tier {tier}"), |seed| {
            save.clone().dial(&data, 0, seed).unwrap()
        });
    }
}
