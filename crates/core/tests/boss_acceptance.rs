//! 138.1 acceptance: the net's AI through the real save and run (`Save::dial`, the probe's
//! bots playing `Game::apply`), checked against the task's acceptance criteria.

use ninsei_core::game::BossState;
use ninsei_core::hub::{Entry, Save};
use ninsei_core::probe::{self, Bot};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

fn save(tier: u32, heat: u32, program: &str) -> Save {
    let mut s = Save {
        program: Some(program.into()),
        ..Save::default()
    };
    s.ensure_key(0x1381);
    s.phonebook.push(Entry {
        id: 0,
        name: "probe".into(),
        business: "test".into(),
        number: "555-0000".into(),
        tier,
        traits: Vec::new(),
        heat,
        runs: 0,
        last: Vec::new(),
    });
    s
}

#[test]
fn every_net_runs_a_named_versioned_ai_picked_by_tier_and_heat() {
    let d = data();
    for tier in 1..=3 {
        for heat in 0..=tuning::HEAT_MAX {
            let g = save(tier, heat, probe::CAREFUL).dial(&d, 0, 1).unwrap();
            let b = g.boss.as_ref().expect("every net has an AI");
            let row = &d.ice[b.ice];
            let model = d.boss_model(b.ice).unwrap();
            let version = d.ai.versions.iter().find(|v| v.row_id() == row.id).unwrap();
            assert_eq!(row.name, format!("{} {}", model.name, version.version));
            assert_eq!(Some(b.ice), d.boss_for(tier, heat));
            assert!(!model.company.is_empty());
        }
    }
}

#[test]
fn it_wakes_only_at_its_threshold_and_never_on_the_runner() {
    let d = data();
    let mut woke = 0;
    for tier in 1..=3 {
        for seed in 0..40 {
            let mut g = save(tier, 0, probe::CAREFUL).dial(&d, 0, seed).unwrap();
            let wake = g.boss.as_ref().unwrap().wake;
            // Walk it by hand with the bot's fights, watching each hop.
            for _ in 0..40 {
                if g.phase != Phase::Exploring {
                    break;
                }
                let before = g.deck.trace;
                let asleep = g.hunter().is_none();
                let Some(&to) = g.reachable().first() else {
                    break;
                };
                g.apply(Action::Move(to));
                if asleep && g.hunter().is_some() {
                    woke += 1;
                    assert!(g.deck.trace >= wake && before < wake || before >= wake);
                    if g.phase == Phase::Exploring {
                        assert_ne!(g.hunter().unwrap().at, g.at, "never on the runner");
                    }
                }
                if g.hunter().is_none() && g.boss.as_ref().unwrap().state == BossState::Asleep {
                    assert!(g.deck.trace < wake || g.phase != Phase::Exploring);
                }
                // Fights on the way: the bot's own program plays them, for a while.
                for _ in 0..60 {
                    if g.phase != Phase::Combat {
                        break;
                    }
                    g.apply(Action::Finish);
                    if g.phase == Phase::Combat {
                        g.apply(Action::EndTurn);
                    }
                }
                if g.phase == Phase::Encounter {
                    if let Some(leave) = g.leave_choice() {
                        g.apply(Action::Choose {
                            choice: leave,
                            slot: None,
                        });
                    }
                    g.apply(Action::EndEncounter);
                }
                if !matches!(g.phase, Phase::Exploring) {
                    break;
                }
            }
        }
    }
    assert!(woke > 20, "{woke}");
}

#[test]
fn a_run_where_it_never_wakes_plays_as_if_it_wasnt_there() {
    // Same seed, same game: with the AI asleep throughout, the run is the run without one.
    let d = data();
    let mut asleep = 0;
    for seed in 0..60 {
        let g = save(1, 0, probe::CAREFUL).dial(&d, 0, seed).unwrap();
        let mut bare = g.clone();
        bare.boss = None;
        let (with, a) = probe::play(g, Bot::Smart);
        if a.ai.is_some_and(|a| a.woke) {
            continue;
        }
        let (without, _) = probe::play(bare, Bot::Smart);
        let said = |g: &Game| g.log.iter().map(|l| l.text.clone()).collect::<Vec<_>>();
        assert_eq!(said(&with), said(&without), "seed {seed}");
        assert_eq!(with.phase, without.phase);
        assert_eq!(with.deck, without.deck);
        asleep += 1;
    }
    assert!(asleep > 5, "{asleep}");
}

#[test]
fn meeting_it_is_a_fight_and_beaten_it_stays_down() {
    let d = data();
    let (mut met, mut beaten) = (0, 0);
    for tier in 1..=3 {
        for seed in 0..60 {
            let g = save(tier, 0, probe::CAREFUL).dial(&d, 0, seed).unwrap();
            let (g, log) = probe::play(g, Bot::Smart);
            let ai = log.ai.unwrap();
            let b = g.boss.as_ref().unwrap();
            // Its fights are booked like any fight; it's down only once beaten in one.
            assert_eq!(ai.met, g.bouts.iter().any(|f| f.ice == b.ice));
            if b.state == BossState::Down {
                assert!(g.bouts.iter().any(|f| f.ice == b.ice && f.broken));
                // Down: it was met once and never again.
                assert_eq!(g.bouts.iter().filter(|f| f.ice == b.ice).count(), 1);
                beaten += 1;
            }
            met += u32::from(ai.met);
            // A flatline in its fight names it the killer.
            if ai.killed {
                assert_eq!(g.phase, Phase::Flatlined);
            }
            assert!(b.passes <= g.hops, "one pass a hop at most");
        }
    }
    assert!(met > 10 && beaten > 5, "met {met} beaten {beaten}");
}

#[test]
fn segfault_is_the_ais_alone() {
    let d = data();
    assert!(
        d.ice
            .iter()
            .filter(|i| !i.boss)
            .all(|i| !i.verbs.contains(&basic::Verb::Segfault))
    );
    assert!(
        d.ice
            .iter()
            .filter(|i| i.boss)
            .any(|i| i.verbs.contains(&basic::Verb::Segfault))
    );
}

#[test]
fn the_same_seed_plays_the_same_with_its_ai() {
    let d = data();
    for seed in 0..10 {
        let run = || {
            let g = save(2, 0, probe::CAREFUL).dial(&d, 0, seed).unwrap();
            let (g, log) = probe::play(g, Bot::Smart);
            (
                g.log.iter().map(|l| l.text.clone()).collect::<Vec<_>>(),
                log,
            )
        };
        assert_eq!(run(), run(), "seed {seed}");
    }
}
