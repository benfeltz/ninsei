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

#[test]
fn careers_shop_take_contracts_and_talk() {
    let data = Data::builtin().unwrap();
    let mut p = Probe::new(
        &data,
        Config {
            spot_runs: 1,
            careers: 3,
            career_runs: 15,
            seed: 11,
        },
    );
    while !p.step(50) {}
    let s = p.stats();
    assert!(s.taken > 0, "it takes contracts");
    assert!(s.paid > 0 && s.pay > 0, "and some pay");
    assert!(!s.bought.is_empty(), "it buys at BBSes");
    assert!(
        s.talks.iter().any(|(t, _)| t != "walked away"),
        "it does more than walk away: {:?}",
        s.talks
    );
}

#[test]
fn careers_buy_a_bigger_chassis() {
    let mut data = Data::builtin().unwrap();
    // Every deck all but free, so even a short career outgrows the starter.
    for d in &mut data.hardware.decks {
        d.price = d.price.min(1);
    }
    let starter = data
        .hardware
        .starter_deck()
        .stat(hub::Upgrade::Cpu)
        .unwrap()
        .max;
    let mut p = Probe::new(
        &data,
        Config {
            spot_runs: 1,
            careers: 2,
            career_runs: 20,
            seed: 11,
        },
    );
    while !p.step(50) {}
    let s = p.stats();
    assert!(
        s.decks.iter().any(|(cap, _, _)| *cap > starter),
        "a capped career buys a deck that takes more: {:?}",
        s.decks
    );
    let lines = p.report();
    assert!(lines.iter().any(|l| l.contains("tier 3 on a cpu cap")) || s.career[2].runs == 0);
    assert!(lines.iter().any(|l| l.trim_start().starts_with("deck ")));
}

#[test]
fn the_dumb_bot_is_two_lines() {
    let data = Data::builtin().unwrap();
    let dumb = ninsei_core::basic::parse(probe::DUMB);
    assert_eq!(dumb.lines.len(), 2);
    assert_eq!(Bot::Dumb.program(), probe::DUMB);
    // The careful bot has its own program: the kit's SAFE only defends.
    assert_eq!(Bot::Smart.program(), probe::CAREFUL);
    assert_ne!(probe::CAREFUL, data.starts.program(""));
    assert_eq!(ninsei_core::hub::program_fault(probe::CAREFUL, &data), None);
    // Both play fights through the interpreter: what's left in the run is their program.
    for bot in [Bot::Dumb, Bot::Smart] {
        let (g, log) = probe::play(Game::new(data.clone(), 3), bot);
        assert_eq!(g.program, ninsei_core::basic::parse(bot.program()));
        if log.fights > 0 {
            assert!(
                !log.breaks.is_empty(),
                "{bot:?} broke nothing in {} fights",
                log.fights
            );
        }
    }
}
