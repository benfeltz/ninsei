//! 138.1 step 3: the net's AI in a run, through the real save (`Save::dial`) and `Game::apply`:
//! it wakes at the net's trace threshold ahead of the runner, takes one pass a hop, and meets
//! them in a fight that can't be left.

use ninsei_core::boss::{Hunter, route};
use ninsei_core::game::{BossState, Sighting};
use ninsei_core::hub::{Entry, Save};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

/// A save with one number on it, a tier-`tier` net with `traits`.
fn save(tier: u32, traits: &[&str]) -> Save {
    let mut s = Save::default();
    s.ensure_key(0x138);
    s.ensure_hardware(&data());
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

/// A run on that number with nothing on the net but the AI: every node a looted vault.
fn quiet_run(tier: u32, seed: u64) -> Game {
    let d = data();
    let mut g = save(tier, &[]).dial(&d, 0, seed).unwrap();
    for n in &mut g.net.nodes {
        if !matches!(n.kind, NodeKind::Entry | NodeKind::Exit) {
            n.kind = NodeKind::Vault;
            n.cleared = true;
            n.ice = None;
        }
    }
    g
}

/// The AI's wake threshold this run.
fn wake(g: &Game) -> u32 {
    g.boss.as_ref().unwrap().wake
}

fn passes(g: &Game) -> u32 {
    g.boss.as_ref().map_or(0, |b| b.passes)
}

/// The runner's first route on, deeper.
fn deeper(g: &Game) -> usize {
    g.net.nodes[g.at].links[0]
}

#[test]
fn every_dialed_net_has_an_ai_by_tier_and_heat_and_traits_move_its_wake() {
    let d = data();
    for (tier, wake) in [(1, 25), (2, 20), (3, 15)] {
        let g = save(tier, &[]).dial(&d, 0, 1).unwrap();
        let b = g.boss.as_ref().expect("every number runs an AI");
        assert_eq!(b.wake, wake);
        assert_eq!(Some(b.ice), d.boss_for(tier, 0));
        assert_eq!(b.state, BossState::Asleep);
    }
    let g = save(1, &["graveyard"]).dial(&d, 0, 1).unwrap();
    assert_eq!(wake(&g), 35, "a skeleton crew wakes it late");
    let g = save(1, &["night_shift"]).dial(&d, 0, 1).unwrap();
    assert_eq!(wake(&g), 15, "a night shift early");
    let mut s = save(1, &[]);
    s.phonebook[0].heat = tuning::HEAT_MAX;
    let g = s.dial(&d, 0, 1).unwrap();
    assert_eq!(Some(g.boss.unwrap().ice), d.boss_for(1, tuning::HEAT_MAX));
    // A bare run has none.
    assert!(Game::new(d, 1).boss.is_none());
}

#[test]
fn it_is_absent_below_its_threshold_and_wakes_ahead_at_it() {
    for seed in 0..20 {
        let mut g = quiet_run(1, seed);
        let hop = g.route(deeper(&g)).unwrap().trace;
        // One hop short of the threshold.
        g.deck.trace = wake(&g) - hop - 1;
        g.apply(Action::Move(deeper(&g)));
        assert!(g.hunter().is_none() && passes(&g) == 0, "seed {seed}");
        assert_eq!(g.sighting(), None);
        g.deck.trace = wake(&g) - hop;
        g.apply(Action::Move(deeper(&g)));
        let Some(h) = g.hunter().cloned() else {
            panic!("seed {seed}: awake at {}", g.deck.trace)
        };
        assert_ne!(h.at, g.at, "seed {seed}: never on the runner");
        assert!(
            route(&g.net, g.at, h.at).is_some(),
            "seed {seed}: one they can walk to"
        );
        let exit = ninsei_core::boss::exit(&g.net).unwrap();
        assert!(
            h.at == exit || route(&g.net, h.at, exit).is_some(),
            "seed {seed}: between the runner and the exit"
        );
        assert_eq!(passes(&g), 1, "it takes its pass the hop it wakes");
    }
}

/// A run with the AI awake, held still: its program is `HOLD`, on node `at`.
fn awake_at(g: &mut Game, at: usize, program: &str) {
    let d = &g.data;
    let ice = g.boss.as_ref().unwrap().ice;
    let mut h = Hunter::wake(ice, &d.ice[ice], at, g.at);
    h.prog = basic::parse_with(program, d.ice[ice].vocab());
    g.boss.as_mut().unwrap().state = BossState::Awake(h);
}

#[test]
fn it_takes_one_pass_a_hop_and_none_otherwise() {
    let d = data();
    let mut g = quiet_run(2, 3);
    let exit = ninsei_core::boss::exit(&g.net).unwrap();
    awake_at(&mut g, exit, "10 HOLD");
    let mut hops = 0;
    // Forward, then back along a link that runs both ways.
    g.apply(Action::Move(deeper(&g)));
    hops += 1;
    if let Some(&back) = g.net.back(g.at).collect::<Vec<_>>().first() {
        g.apply(Action::Move(back));
        hops += 1;
    }
    assert_eq!(passes(&g), hops);
    // A program typed, a board read, nothing that isn't a hop moves it.
    g.apply(Action::Program("10 RUN icepick".into()));
    g.apply(Action::Study {
        steps: 5,
        drafts: 5,
    });
    assert_eq!(passes(&g), hops);
    // A fight's turns are not hops.
    let ix = d.ice.iter().position(|i| i.id == "icewall").unwrap();
    let to = deeper(&g);
    g.net.nodes[to].kind = NodeKind::Ice;
    g.net.nodes[to].ice = Some(ix);
    g.net.nodes[to].cleared = false;
    g.apply(Action::Move(to));
    hops += 1;
    assert_eq!(g.phase, Phase::Combat);
    for _ in 0..3 {
        g.apply(Action::EndTurn);
    }
    assert_eq!(passes(&g), hops);
}

#[test]
fn a_subnet_hop_is_a_hop_and_the_ai_never_goes_in() {
    let d = data();
    for seed in 0..10 {
        let mut g = save(2, &[]).dial(&d, 0, seed).unwrap();
        let Some(sub) = g.net.nodes.iter().position(|n| n.kind == NodeKind::Subnet) else {
            continue;
        };
        for n in &mut g.net.nodes {
            if !matches!(n.kind, NodeKind::Entry | NodeKind::Exit | NodeKind::Subnet) {
                n.kind = NodeKind::Vault;
                n.cleared = true;
                n.ice = None;
            }
        }
        let before = *g.net.layers[g.net.nodes[sub].layer - 1]
            .iter()
            .find(|&&n| g.net.nodes[n].links.contains(&sub))
            .unwrap();
        g.at = before;
        let exit = ninsei_core::boss::exit(&g.net).unwrap();
        awake_at(&mut g, exit, "10 HOLD");
        g.apply(Action::Move(sub));
        assert!(g.in_subnet());
        let mut hops = 1;
        while g.in_subnet() && g.phase == Phase::Exploring {
            for n in &mut g.net.nodes {
                if n.kind != NodeKind::Exit {
                    n.kind = NodeKind::Vault;
                    n.cleared = true;
                    n.ice = None;
                }
            }
            g.apply(Action::Move(deeper(&g)));
            hops += 1;
        }
        assert_eq!(passes(&g), hops, "seed {seed}");
        assert!(g.hunter().unwrap().at < g.root().nodes.len());
        assert_eq!(g.hunter().unwrap().at, exit, "it held the main net's exit");
    }
}

#[test]
fn stepping_onto_it_is_a_fight_that_cant_be_left() {
    let mut g = quiet_run(1, 5);
    let to = deeper(&g);
    awake_at(&mut g, to, "10 HOLD");
    // The runner sees it next door, so the move's preview says they'll meet.
    assert!(matches!(g.sighting(), Some(Sighting::Near(n)) if n == to));
    assert!(g.route(to).unwrap().meets);
    g.apply(Action::Move(to));
    assert_eq!(g.phase, Phase::Combat);
    let c = g.combat.as_ref().unwrap();
    assert!(g.data.ice[c.ice].boss);
    assert_eq!(c.strength, g.data.ice[c.ice].strength + g.mods.ice_bonus);
    // No way off the node while it's up.
    assert!(g.reachable().is_empty());
    let any = g.net.nodes[g.at].links[0];
    assert!(g.route(any).is_err());
}

#[test]
fn it_stepping_onto_the_runner_is_a_fight_too() {
    let mut met = 0;
    for seed in 0..20 {
        let mut g = quiet_run(1, seed);
        let next = deeper(&g);
        // Two hops on, by a link it can walk back.
        let Some(&beyond) = g.net.nodes[next]
            .links
            .iter()
            .find(|&&b| !g.net.one_way(next, b))
        else {
            continue;
        };
        awake_at(&mut g, beyond, "10 MOVE TOWARD TARGET");
        // It saw them where they're going.
        if let BossState::Awake(h) = &mut g.boss.as_mut().unwrap().state {
            h.seen = Some(next);
        }
        // Two hops away: out of sight, so the preview can't say.
        assert!(!g.route(next).unwrap().meets);
        g.apply(Action::Move(next));
        assert_eq!(g.phase, Phase::Combat, "seed {seed}");
        assert!(g.data.ice[g.combat.as_ref().unwrap().ice].boss);
        assert_eq!(g.hunter().unwrap().at, next);
        met += 1;
    }
    assert!(met > 5);
}

#[test]
fn beaten_it_is_down_for_the_run_and_the_node_has_its_say() {
    let mut g = quiet_run(1, 7);
    let to = deeper(&g);
    // A vault still full, where it waits.
    g.net.nodes[to].cleared = false;
    awake_at(&mut g, to, "10 HOLD");
    g.apply(Action::Move(to));
    assert_eq!(g.phase, Phase::Combat);
    g.combat.as_mut().unwrap().strength = 1;
    let pick = g.slot_named("icepick").unwrap();
    g.apply(Action::Run(pick));
    assert_eq!(g.phase, Phase::Exploring, "{:?}", g.log.last());
    assert_eq!(g.boss.as_ref().unwrap().state, BossState::Down);
    assert!(g.net.nodes[to].cleared, "the vault opened once it was down");
    assert!(g.log.iter().any(|l| l.text.contains("vault unlocked")));
    // Down: it neither moves nor fights again.
    let n = passes(&g);
    for _ in 0..3 {
        if g.phase != Phase::Exploring || g.net.nodes[g.at].kind == NodeKind::Exit {
            break;
        }
        g.apply(Action::Move(deeper(&g)));
    }
    assert_eq!(passes(&g), n);
    assert!(g.hunter().is_none() && g.sighting().is_none());
}

#[test]
fn losing_to_it_is_a_flatline_like_any_fight() {
    let mut g = quiet_run(1, 8);
    let to = deeper(&g);
    awake_at(&mut g, to, "10 HOLD");
    g.apply(Action::Move(to));
    g.deck.integrity = 1;
    for _ in 0..20 {
        if g.phase != Phase::Combat {
            break;
        }
        g.apply(Action::EndTurn);
    }
    assert_eq!(g.phase, Phase::Flatlined);
    let killer = g.killed_by().unwrap();
    assert!(g.data.ice[killer].boss);
}

#[test]
fn an_ai_that_never_meets_the_runner_draws_nothing_from_the_run() {
    // The same seed, with and without the AI, walked the same way: while they never meet, the
    // run's RNG, its log of fights and loot, and its network are the same.
    for seed in 0..10 {
        let d = data();
        let with = save(1, &[]).dial(&d, 0, seed).unwrap();
        let mut without = with.clone();
        without.boss = None;
        let mut with = with;
        // Asleep until now; woken on the exit holding it, off the runner's way out until the
        // last hop.
        let exit = ninsei_core::boss::exit(&with.net).unwrap();
        awake_at(&mut with, exit, "10 SCAN 1\n20 HOLD");
        let mut hops = 0;
        while with.phase == Phase::Exploring && without.phase == Phase::Exploring && hops < 12 {
            let Some(&to) = with.reachable().iter().find(|&&t| t != exit) else {
                break;
            };
            with.apply(Action::Move(to));
            without.apply(Action::Move(to));
            // Fights on the way: both play them the same.
            for g in [&mut with, &mut without] {
                for _ in 0..30 {
                    if g.phase != Phase::Combat {
                        break;
                    }
                    if let Some(p) = g.slot_named("icepick") {
                        g.apply(Action::Run(p));
                    }
                    if g.phase == Phase::Combat {
                        g.apply(Action::EndTurn);
                    }
                }
            }
            hops += 1;
            let said = |g: &Game| -> Vec<String> { g.log.iter().map(|l| l.text.clone()).collect() };
            assert_eq!(said(&with), said(&without), "seed {seed} hop {hops}");
            assert_eq!(
                with.rng.clone().next_u64(),
                without.rng.clone().next_u64(),
                "seed {seed} hop {hops}"
            );
            assert_eq!(with.deck, without.deck);
        }
        assert!(passes(&with) > 0);
    }
}

#[test]
fn the_careful_bot_routes_around_an_ai_it_can_see() {
    let mut tried = 0;
    for seed in 0..30 {
        let mut g = quiet_run(1, seed);
        let links = g.net.nodes[g.at].links.clone();
        if links.len() < 2 {
            continue;
        }
        awake_at(&mut g, links[0], "10 HOLD");
        assert!(g.route(links[0]).unwrap().meets);
        let name = g.net.nodes[links[0]].name.clone();
        let (g, log) = probe::play(g, probe::Bot::Smart);
        let first = g
            .log
            .iter()
            .find(|l| l.text.starts_with("> routing to"))
            .unwrap();
        assert!(!first.text.contains(&name), "seed {seed}: {}", first.text);
        assert!(log.ai.is_some_and(|a| a.woke));
        tried += 1;
    }
    assert!(tried > 3);
}

#[test]
fn the_probe_reports_the_nets_ais_by_version() {
    let d = data();
    let mut p = probe::Probe::new(
        &d,
        probe::Config {
            spot_runs: 3,
            careers: 2,
            career_runs: 12,
            seed: 7,
        },
    );
    while !p.step(50) {}
    let s = p.stats();
    let runs: u32 = s.ai.iter().map(|(_, t)| t.runs).sum();
    assert_eq!(runs, s.career.iter().map(|t| t.runs).sum::<u32>());
    let lines = p.report();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("the nets' AIs, by version"))
    );
    assert!(
        lines.iter().any(|l| l.contains("WATCHMAN 1.2")),
        "{lines:#?}"
    );
}

#[test]
fn the_hop_onto_the_exit_that_crosses_the_threshold_jacks_out() {
    let mut checked = 0;
    for seed in 0..20 {
        let mut g = quiet_run(1, seed);
        let exit = ninsei_core::boss::exit(&g.net).unwrap();
        // One hop short of the exit.
        let Some(&before) = g
            .net
            .nodes
            .iter()
            .map(|n| n.id)
            .collect::<Vec<_>>()
            .iter()
            .find(|&&n| g.net.nodes[n].links.contains(&exit))
        else {
            continue;
        };
        g.at = before;
        let hop = g.route(exit).unwrap().trace;
        g.deck.trace = wake(&g) - hop;
        g.apply(Action::Move(exit));
        assert_eq!(g.phase, Phase::JackedOut, "seed {seed}: {:?}", g.log.last());
        checked += 1;
    }
    assert!(checked > 0);
}

#[test]
fn the_ice_db_has_no_page_for_an_ai_met_or_not() {
    let d = data();
    let mut s = save(1, &[]);
    let mut g = s.dial(&d, 0, 9).unwrap();
    for n in &mut g.net.nodes {
        if !matches!(n.kind, NodeKind::Entry | NodeKind::Exit) {
            n.kind = NodeKind::Vault;
            n.cleared = true;
            n.ice = None;
        }
    }
    let to = deeper(&g);
    awake_at(&mut g, to, "10 HOLD");
    g.apply(Action::Move(to));
    assert!(g.data.ice[g.combat.as_ref().unwrap().ice].boss);
    g.deck.integrity = 1;
    while g.phase == Phase::Combat {
        g.apply(Action::EndTurn);
    }
    s.record(&g, &[], &d.rig).unwrap();
    assert!(
        s.ice_seen
            .iter()
            .all(|seen| !d.ice.iter().any(|i| i.boss && i.id == seen.id))
    );
    assert!(icedb::entries(&s, &d).iter().all(|e| !e.def.boss));
    assert_eq!(
        icedb::entries(&s, &d).len(),
        d.ice.iter().filter(|i| !i.boss).count()
    );
}
