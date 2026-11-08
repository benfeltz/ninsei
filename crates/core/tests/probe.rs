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

/// Every deck all but free, so even a short career outgrows the starter.
fn cheap_decks() -> Data {
    let mut data = Data::builtin().unwrap();
    for d in &mut data.hardware.decks {
        d.price = d.price.min(1);
    }
    data
}

fn short_careers() -> Config {
    Config {
        spot_runs: 1,
        careers: 2,
        career_runs: 20,
        seed: 11,
    }
}

#[test]
fn careers_buy_a_bigger_chassis() {
    let data = cheap_decks();
    let starter = data.hardware.starter_deck();
    let mut p = Probe::new(&data, short_careers());
    while !p.step(50) {}
    let s = p.stats();
    let bigger: Vec<_> = data
        .hardware
        .decks
        .iter()
        .filter(|d| {
            d.stat(hub::Upgrade::Cpu).unwrap().max > starter.stat(hub::Upgrade::Cpu).unwrap().max
        })
        .map(|d| d.title())
        .collect();
    assert!(
        s.decks.iter().any(|(t, _, _)| bigger.contains(t)),
        "a capped career buys a deck that takes more: {:?}",
        s.decks
    );
    let lines = p.report();
    assert!(lines.iter().any(|l| l.contains("tier 3 on ")) || s.career[2].runs == 0);
    assert!(lines.iter().any(|l| l.trim_start().starts_with("deck ")));
    assert!(
        lines.iter().any(|l| l.contains("cheapest deck")),
        "it says how it chose"
    );
}

#[test]
fn careers_climb_to_the_deck_they_are_given() {
    let data = cheap_decks();
    for deck in data.hardware.decks.iter().filter(|d| !d.starter) {
        let mut p = Probe::new(&data, short_careers()).with_deck(&deck.id);
        while !p.step(50) {}
        let bought: Vec<&str> = p.stats().decks.iter().map(|(t, _, _)| t.as_str()).collect();
        assert_eq!(bought, [deck.title().as_str()], "{}", deck.id);
        let lines = p.report();
        assert!(
            lines
                .iter()
                .any(|l| l.contains(&format!("careers climb to {}", deck.title()))),
            "the report says which: {lines:?}"
        );
    }
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

#[test]
fn a_deck_careers_cannot_buy_means_they_never_climb_and_the_report_says_so() {
    let data = cheap_decks();
    let starter = data.hardware.starter_deck().id.clone();
    for id in [starter.as_str(), "no-such-deck"] {
        let mut p = Probe::new(&data, short_careers()).with_deck(id);
        while !p.step(50) {}
        assert!(p.stats().decks.is_empty(), "{id}: {:?}", p.stats().decks);
        let lines = p.report();
        assert!(
            !lines.iter().any(|l| l.contains("careers climb to")),
            "{id}"
        );
        assert!(
            lines.iter().any(|l| l.contains("never climbs")),
            "{id}: {lines:?}"
        );
    }
}
