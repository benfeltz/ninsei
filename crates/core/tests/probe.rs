//! The balance probe: it finishes, it's deterministic, and its bots are sane.

use ninsei_core::probe::{self, Bot, Config, Probe};
use ninsei_core::*;

fn small() -> Config {
    Config {
        spot_runs: 6,
        careers: 2,
        career_runs: 12,
        seed: 7,
    }
}

#[test]
fn the_probe_finishes_and_reports() {
    let data = Data::builtin().unwrap();
    let mut p = Probe::new(&data, small());
    let (_, total) = p.progress();
    assert_eq!(total, 6 * 3 * 2 + 2 * 12);
    let mut last = 0;
    while !p.step(5) {
        let (done, _) = p.progress();
        assert!(done > last, "it moves forward");
        last = done;
    }
    assert!(p.done());
    let s = p.stats();
    let spot: u32 = s.spot.iter().flatten().map(|t| t.runs).sum();
    assert_eq!(spot, 36);
    assert_eq!(s.career.iter().map(|t| t.runs).sum::<u32>(), 24);
    assert_eq!(s.first_exit.len(), 2);
    let lines = p.report();
    assert!(lines.iter().any(|l| l.contains("careers")));
    assert!(
        lines.iter().all(|l| l.chars().count() <= 70),
        "fits the Shell"
    );
}

#[test]
fn same_seed_same_report() {
    let data = Data::builtin().unwrap();
    let run = || {
        let mut p = Probe::new(&data, small());
        while !p.step(50) {}
        p.report()
    };
    assert_eq!(run(), run());
}

#[test]
fn bots_always_finish_their_runs() {
    let data = Data::builtin().unwrap();
    for seed in 0..20 {
        for bot in [Bot::Dumb, Bot::Smart] {
            let (g, _) = probe::play(Game::new(data.clone(), seed), bot);
            assert!(
                matches!(g.phase, Phase::JackedOut | Phase::Flatlined),
                "seed {seed} {bot:?} stopped in {:?}",
                g.phase
            );
        }
    }
}
