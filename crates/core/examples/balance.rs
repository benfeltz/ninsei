//! Balance probe: plays many seeds with a dumb bot and prints the outcome spread.
//! `cargo run -p ninsei-core --example balance`
use ninsei_core::*;

fn main() {
    let (mut wins, mut hp, mut trace, mut turns) = (0, 0, 0, 0);
    let runs = 500;
    for seed in 0..runs {
        let mut g = Game::new(Data::builtin().unwrap(), seed);
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
                _ => break,
            }
        }
        if g.phase == Phase::JackedOut {
            wins += 1;
            hp += g.deck.integrity;
            trace += g.deck.trace;
        }
    }
    println!(
        "win {wins}/{runs}  avg hp left {}  avg trace {}  ice turns/run {}",
        hp / wins.max(1),
        trace / wins.max(1),
        turns / runs as u32
    );
}
