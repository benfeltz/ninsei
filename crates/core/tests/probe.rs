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
    // Three programs' spot checks; the careers twice, with CAREFUL then SAFE_PICK.
    assert_eq!(total, 6 * 3 * 3 + 2 * 2 * 12);
    let mut last = 0;
    while !p.step(5) {
        let (done, _) = p.progress();
        assert!(done > last, "it moves forward");
        last = done;
    }
    assert!(p.done());
    let s = p.stats();
    let spot: u32 = s.spot.iter().flatten().map(|t| t.runs).sum();
    assert_eq!(spot, 54);
    assert_eq!(s.career.iter().map(|t| t.runs).sum::<u32>(), 24);
    assert_eq!(s.career_pick.iter().map(|t| t.runs).sum::<u32>(), 24);
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
        for bot in Bot::ALL {
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
        // RIG lines count the levels this deck takes, not every level RIG sells.
        for def in &data.rig {
            let Some(st) = deck.stat(def.kind) else {
                continue;
            };
            let room = ((st.max - st.base) / def.step).min(def.max);
            let label = format!("{} ", def.label);
            if let Some(line) = lines.iter().find(|l| l.trim_start().starts_with(&label)) {
                assert!(
                    line.contains(&format!("(of {room})")),
                    "{}: {line}",
                    deck.id
                );
            }
        }
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
    // All three play fights through the interpreter: what's left in the run is their program.
    for bot in Bot::ALL {
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

#[test]
fn deck_lines_fit_the_shell_whatever_the_decks_are_called() {
    let mut data = cheap_decks();
    for d in &mut data.hardware.decks {
        d.maker = "Ono-Sendai Heavy Industries Consolidated".into();
        d.name = format!("{} Professional Workstation Deluxe", d.name);
    }
    let climb = data
        .hardware
        .decks
        .iter()
        .find(|d| !d.starter)
        .unwrap()
        .id
        .clone();
    for id in [
        "",
        climb.as_str(),
        "a-deck-id-much-longer-than-any-line-has-room-for",
    ] {
        let cfg = Config {
            career_runs: 40,
            ..short_careers()
        };
        let mut p = Probe::new(&data, cfg).with_deck(id);
        while !p.step(50) {}
        let lines = p.report();
        for l in &lines {
            assert!(l.chars().count() <= 70, "{id:?}: {l:?}");
        }
        if id.is_empty() {
            assert!(lines.iter().any(|l| l.contains("tier 3 on ")), "{lines:?}");
            assert!(lines.iter().any(|l| l.trim_start().starts_with("deck ")));
        }
    }
}

#[test]
fn safe_pick_is_the_starters_safe_with_a_breaker_typed_in() {
    let data = Data::builtin().unwrap();
    let safe = data.starts.program("");
    let typed = ninsei_core::basic::parse(&format!("{safe}\n55 RUN icepick"));
    assert_eq!(ninsei_core::basic::parse(probe::SAFE_PICK), typed);
    assert_eq!(Bot::SafePick.program(), probe::SAFE_PICK);
}

#[test]
fn the_report_names_all_three_programs_with_turns_per_fight() {
    let data = Data::builtin().unwrap();
    let mut p = Probe::new(&data, small());
    while !p.step(50) {}
    let lines = p.report();
    for bot in Bot::ALL {
        let rows = lines
            .iter()
            .filter(|l| l.trim_start().starts_with(bot.label()))
            .count();
        // A row per tier, and one for what ICE did to its program.
        assert_eq!(rows, 4, "{} at each tier: {lines:#?}", bot.label());
    }
    assert!(lines.iter().any(|l| l.contains("turns/fight")));
    assert!(lines.iter().any(|l| l.contains("fighting with SAFE_PICK")));
    assert!(lines.iter().any(|l| l.contains("SPOOF  SEGFAULT")));
    let s = p.stats();
    assert!(
        s.spot
            .iter()
            .flatten()
            .any(|t| t.fights > 0 && t.turns >= t.fights)
    );
}

#[test]
fn every_fight_a_run_has_is_counted_with_its_turns() {
    let data = Data::builtin().unwrap();
    let mut encounter_fights = 0;
    for seed in 0..60 {
        let (g, log) = probe::play(Game::new(data.clone(), seed), Bot::Smart);
        // Every fight the run met, the map's and the encounters' alike.
        assert_eq!(log.fought as usize, g.history.met.len(), "seed {seed}");
        assert!(log.fought >= log.fights, "seed {seed}");
        assert!(
            log.turns >= log.fought,
            "a fight takes a turn at least: seed {seed}"
        );
        encounter_fights += log.fought - log.fights;
    }
    assert!(encounter_fights > 0, "some fights came from encounters");
}
