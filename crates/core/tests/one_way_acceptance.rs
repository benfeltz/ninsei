//! 143.1 acceptance, from the outside: networks dialed from a save on every tier, walked by a
//! runner that goes back as often as it goes forward, and by the probe's bots. Every hop costs
//! the same, a one-way link is never walked back, every run ends, and what the deck beat comes
//! back up when it steps back onto it.

use std::sync::Arc;

use ninsei_core::hub::{Entry, Save};
use ninsei_core::probe::{self, Bot};
use ninsei_core::rng::Rng;
use ninsei_core::tuning::{TRACE_MAX, TRACE_PER_HOP};
use ninsei_core::*;

fn data() -> Arc<Data> {
    Arc::new(Data::builtin().unwrap())
}

fn save(tier: u32, traits: &[&str]) -> Save {
    let mut s = Save::default();
    s.ensure_key(11);
    s.phonebook.push(Entry {
        id: 0,
        name: "probe".into(),
        business: "test".into(),
        number: "555-0000".into(),
        tier,
        traits: traits.iter().map(|t| t.to_string()).collect(),
        heat: 0,
        runs: 0,
        last: Vec::new(),
    });
    s
}

/// A runner wandering at random, back as readily as forward: every move it makes is one the
/// preview allowed, costs the run's hop and lands where it said; every route the preview
/// refuses moves nothing; and the run always ends, on trace if nowhere else.
#[test]
fn a_wandering_runner_always_ends_and_every_hop_costs_the_same() {
    let d = data();
    for tier in 1..=3 {
        for traits in [&[][..], &["mesh"], &["daisy_chain"], &["segmented"]] {
            for seed in 0..25u64 {
                let mut g = save(tier, traits).dial(&d, 0, seed).unwrap();
                let hop = g.mods.trace(TRACE_PER_HOP);
                let mut pick = Rng::new(seed ^ 0xACCE);
                let mut moves = 0;
                for _ in 0..10_000 {
                    match g.phase {
                        Phase::Exploring => {
                            let reach = g.reachable();
                            assert!(!reach.is_empty(), "tier {tier} seed {seed}: stranded");
                            let to = reach[pick.below(reach.len())];
                            let p = g.route(to).unwrap();
                            assert_eq!(p.trace, hop);
                            let (trace, net) = (g.deck.trace, g.in_subnet());
                            g.apply(Action::Move(to));
                            moves += 1;
                            // A move into or out of a subnet lands on its entry or its door.
                            if g.in_subnet() == net {
                                assert_eq!(g.at, to);
                            }
                            assert!(
                                g.deck.trace == (trace + hop).min(TRACE_MAX)
                                    || g.phase == Phase::Flatlined,
                                "tier {tier} seed {seed}: a hop costs {hop}"
                            );
                            for n in 0..g.net.nodes.len() {
                                if !g.reachable().contains(&n) && g.phase == Phase::Exploring {
                                    assert!(g.route(n).is_err());
                                }
                            }
                        }
                        Phase::Combat => {
                            // Break it or be broken: a few turns of the deck's picks.
                            g.apply(Action::Play);
                            while g.pass_open() {
                                g.apply(Action::Finish);
                            }
                            if g.phase == Phase::Combat {
                                g.apply(Action::EndTurn);
                            }
                        }
                        Phase::Encounter => {
                            let c = g.leave_choice().unwrap();
                            g.apply(Action::Choose {
                                choice: c,
                                slot: None,
                            });
                            g.apply(Action::EndEncounter);
                        }
                        Phase::JackedOut | Phase::Flatlined => break,
                    }
                }
                assert!(
                    matches!(g.phase, Phase::JackedOut | Phase::Flatlined),
                    "tier {tier} seed {seed}: the run never ended after {moves} moves"
                );
            }
        }
    }
}

/// Every link the runner walked back is two way: one marked one way was only ever walked
/// deeper, whatever the wandering did.
#[test]
fn a_one_way_link_is_only_ever_walked_deeper() {
    let d = data();
    for tier in 1..=3 {
        for seed in 0..40u64 {
            let mut g = save(tier, &[]).dial(&d, 0, seed).unwrap();
            for n in &mut g.net.nodes {
                if !matches!(n.kind, NodeKind::Entry | NodeKind::Exit) {
                    (n.kind, n.ice, n.cleared) = (NodeKind::Vault, None, true);
                }
            }
            let mut pick = Rng::new(seed);
            while g.phase == Phase::Exploring {
                let reach = g.reachable();
                let to = reach[pick.below(reach.len())];
                let from = g.at;
                g.apply(Action::Move(to));
                if g.net.nodes[to].layer < g.net.nodes[from].layer {
                    assert!(!g.net.one_way(to, from), "tier {tier} seed {seed}");
                }
            }
        }
    }
}

/// The probe's bots on dialed networks of every tier: every run ends, the dumb bot never steps
/// back, and a careful bot that steps back does it for something and pays the hop.
#[test]
fn the_bots_finish_every_run_and_only_the_careful_ones_look_back() {
    let d = data();
    for tier in 1..=3 {
        for seed in 0..30u64 {
            for bot in Bot::ALL {
                let g = save(tier, &[]).dial(&d, 0, seed).unwrap();
                let (g, log) = probe::play(g, bot);
                assert!(
                    matches!(g.phase, Phase::JackedOut | Phase::Flatlined),
                    "{bot:?} tier {tier} seed {seed}"
                );
                if bot == Bot::Dumb {
                    assert_eq!(log.back, 0);
                    assert_eq!(log.rearmed, 0);
                }
                assert!(g.hops >= log.back);
            }
        }
    }
}

/// From a save: beat a kind on a dialed network, step off and back, and the node meets the
/// kind's version in force, whatever it is, in a fight that pays nothing; home books that
/// version for the kind.
#[test]
fn a_dialed_run_re_arms_what_it_beat_and_home_books_the_version() {
    let d = data();
    let mut rearmed = 0;
    for seed in 0..20u64 {
        let mut s = save(1, &[]);
        s.program = Some(probe::SAFE_PICK.into());
        let mut g = s.dial(&d, 0, seed).unwrap();
        let entry = g.at;
        let node = g.net.nodes[entry].links[0];
        if g.net.one_way(entry, node) {
            continue;
        }
        let e = d.ice.iter().position(|i| i.id == "enigma").unwrap();
        let n = &mut g.net.nodes[node];
        (n.kind, n.ice, n.cleared) = (NodeKind::Ice, Some(e), false);
        g.apply(Action::Move(node));
        let c = g.combat.as_mut().unwrap();
        (c.strength, c.strength_max) = (1, 1);
        while g.phase == Phase::Combat {
            g.apply(Action::Play);
            while g.pass_open() {
                g.apply(Action::Finish);
            }
            if g.phase == Phase::Combat {
                g.apply(Action::EndTurn);
            }
        }
        if g.phase != Phase::Exploring {
            continue;
        }
        g.apply(Action::Move(entry));
        assert!(g.route(node).unwrap().rearms);
        let credits = g.deck.credits;
        g.apply(Action::Move(node));
        assert_eq!(g.phase, Phase::Combat, "seed {seed}");
        let c = g.combat.as_ref().unwrap();
        assert!(c.rearmed);
        assert_eq!(c.ice_version, g.ice_version(e), "the version in force");
        let c = g.combat.as_mut().unwrap();
        (c.strength, c.strength_max) = (1, 1);
        while g.phase == Phase::Combat {
            g.apply(Action::Play);
            while g.pass_open() {
                g.apply(Action::Finish);
            }
            if g.phase == Phase::Combat {
                g.apply(Action::EndTurn);
            }
        }
        if g.phase == Phase::Exploring {
            assert_eq!(g.deck.credits, credits, "seed {seed}: pays nothing");
        }
        let version = g.ice_version(e);
        g.phase = Phase::JackedOut;
        s.record(&g, &[], &d.rig).unwrap();
        assert_eq!(s.ice_program("enigma", &d).unwrap().0, version);
        rearmed += 1;
    }
    assert!(rearmed > 5);
}
