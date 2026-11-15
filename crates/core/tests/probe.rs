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
    // Three programs' spot checks; the careers twice, with CAREFUL then SAFE_PICK; and
    // SAFE_PICK's spot checks again, against the versions its careers shipped.
    assert_eq!(total, 6 * 3 * 3 + 2 * 2 * 12 + 6 * 3);
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
        // Its program, but for the lines an ICE's SEGFAULT garbled: each one that differs is
        // garbage (a line that no longer parses), and no more than the SEGFAULTs counted.
        let want = ninsei_core::basic::parse(bot.program());
        let nos =
            |p: &ninsei_core::basic::Program| p.lines.iter().map(|l| l.no).collect::<Vec<_>>();
        assert_eq!(nos(&g.program), nos(&want), "{bot:?}");
        let garbled: Vec<_> = g
            .program
            .lines
            .iter()
            .zip(&want.lines)
            .filter(|(got, was)| got != was)
            .map(|(got, _)| got)
            .collect();
        assert!(
            garbled.len() as u32 <= log.segfaults,
            "{bot:?}: {garbled:?}"
        );
        assert!(
            garbled
                .iter()
                .all(|l| matches!(l.stmt, ninsei_core::basic::Stmt::Bad(_))),
            "{bot:?}: {garbled:?}"
        );
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

#[test]
fn the_careers_face_evolving_ice_and_the_report_says_what_shipped() {
    let data = Data::builtin().unwrap();
    let cfg = Config {
        spot_runs: 6,
        careers: 2,
        career_runs: 40,
        seed: 7,
    };
    let mut p = Probe::new(&data, cfg);
    while !p.step(50) {}
    let s = p.stats();
    let shipped: u32 = s.shipped.iter().map(|(_, n)| n.iter().sum::<u32>()).sum();
    let pick: u32 = s
        .pick_shipped
        .iter()
        .map(|(_, n)| n.iter().sum::<u32>())
        .sum();
    assert!(shipped > 0 && pick > 0, "{s:?}");
    // Every careful run is counted by how many kinds were patched when it dialed.
    let runs: u32 = s.by_patched.iter().map(|(r, _)| r).sum();
    assert_eq!(runs, 2 * 40);
    assert_eq!(s.versus.iter().map(|t| t.runs).sum::<u32>(), 6 * 3);
    let lines = p.report();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("ICE versions (spot runs stay on 1.0.0)"))
    );
    let targets: Vec<&String> = lines.iter().filter(|l| l.contains("target:")).collect();
    // Two targets: evolved ICE beat SAFE_PICK more, and every tier-1 kind ships. The careful
    // careers are reported, never judged.
    assert_eq!(targets.len(), 2, "{lines:#?}");
    assert!(
        !targets.iter().any(|l| l.contains("careful")),
        "{targets:#?}"
    );
    assert!(
        targets
            .iter()
            .all(|l| l.ends_with(" met") || l.ends_with(" unmet")),
        "{targets:#?}"
    );
    assert!(lines.iter().all(|l| l.chars().count() <= 70), "{lines:#?}");
    // The probe has no clock: it says the pace it assumed, and what a version cost the sysops.
    let pace = format!(
        "pace: a draft every {}s; a bot action {}s, home {}s a run",
        ninsei_core::tuning::STUDY_DRAFT_MS / 1000,
        ninsei_core::tuning::PROBE_ACTION_MS / 1000,
        ninsei_core::tuning::PROBE_HOME_MS / 1000
    );
    assert!(lines.iter().any(|l| l.trim() == pace), "{lines:#?}");
    assert!(s.drafts[0] > 0 && s.drafts[1] > 0, "{:?}", s.drafts);
    assert!(
        lines
            .iter()
            .any(|l| l.contains("drafts tried a version shipped"))
    );
}

#[test]
fn the_careful_bot_rotates_its_programs_and_they_are_real_programs() {
    for program in probe::ROTATION {
        let p = basic::parse(program);
        assert!(p.stray.is_empty());
        assert!(p.lines.iter().all(|l| p.fault(l.no).is_none()), "{program}");
    }
    assert_eq!(probe::ROTATION[0], probe::CAREFUL);
    let mut all = probe::ROTATION.to_vec();
    all.dedup();
    assert_eq!(all.len(), probe::ROTATION.len());
}

#[test]
fn spot_runs_dial_on_one_point_oh() {
    // A spot run starts from a fresh save: no kind is patched, whatever the careers shipped.
    let data = Data::builtin().unwrap();
    let mut p = Probe::new(&data, small());
    while !p.step(50) {}
    let s = p.stats();
    let spot: u32 = s.spot.iter().flatten().map(|t| t.runs).sum();
    let mut q = Probe::new(&data, small());
    q.step(spot);
    assert_eq!(
        q.stats().spot,
        s.spot,
        "the careers come after, and change nothing before"
    );
}

/// A bare run on `seed`, every link two way, every node but the ends an empty cleared vault,
/// standing on a layer-2 node with a layer-1 node behind it that it never visited: (the run,
/// the node behind).
fn past_a_node(seed: u64) -> Option<(Game, usize)> {
    let mut g = Game::new(Data::builtin().unwrap(), seed);
    g.net.orient(seed, 0);
    for n in &mut g.net.nodes {
        if !matches!(n.kind, NodeKind::Entry | NodeKind::Exit) {
            n.kind = NodeKind::Vault;
            n.cleared = true;
            n.ice = None;
        }
    }
    let l1 = g.net.layers[1].clone();
    let (a, b, c) = l1.iter().find_map(|&a| {
        l1.iter().filter(|&&b| b != a).find_map(|&b| {
            let c = *g.net.nodes[a]
                .links
                .iter()
                .find(|x| g.net.nodes[b].links.contains(x))?;
            Some((a, b, c))
        })
    })?;
    g.apply(Action::Move(a));
    g.apply(Action::Move(c));
    assert_eq!(g.at, c);
    Some((g, b))
}

#[test]
fn the_careful_bot_walks_back_for_a_contract_it_left_behind() {
    let mut checked = 0;
    for seed in 0..30 {
        let Some((mut g, b)) = past_a_node(seed) else {
            continue;
        };
        let ice = g.data.ice.iter().position(|i| i.id == "icewall").unwrap();
        let n = &mut g.net.nodes[b];
        (n.kind, n.ice, n.cleared) = (NodeKind::Ice, Some(ice), false);
        g.contracts.push(Contract {
            client: "test".into(),
            brief: String::new(),
            node: b,
            reward: 100,
            ghost: None,
            status: ContractStatus::Active,
            paid: 0,
            betrayal: None,
        });
        // Too hot to go back for it: it carries on to the exit.
        let mut hot = g.clone();
        hot.deck.trace = 70;
        let (hot, log) = probe::play(hot, Bot::Smart);
        assert_eq!(log.back, 0, "seed {seed}: no trace for the walk back");
        assert_eq!(hot.contracts[0].status, ContractStatus::Active);
        // With the trace for it, it goes back, takes the job and gets out.
        let (g, log) = probe::play(g, Bot::Smart);
        assert!(log.back >= 1, "seed {seed}");
        assert!(
            matches!(
                g.contracts[0].status,
                ContractStatus::Done | ContractStatus::Betrayed
            ),
            "seed {seed}: settled"
        );
        assert_eq!(g.phase, Phase::JackedOut, "seed {seed}");
        assert_eq!(log.rearmed, 0, "it routes around what it beat");
        // The dumb bot never looks back.
        let (_, log) = probe::play(past_a_node(seed).unwrap().0, Bot::Dumb);
        assert_eq!(log.back, 0);
        checked += 1;
    }
    assert!(checked > 10);
}

#[test]
fn the_careful_bot_never_walks_back_for_nothing() {
    for seed in 0..30 {
        let Some((g, _)) = past_a_node(seed) else {
            continue;
        };
        let (g, log) = probe::play(g, Bot::Smart);
        assert_eq!(log.back, 0, "seed {seed}");
        assert_eq!(g.phase, Phase::JackedOut);
    }
}
