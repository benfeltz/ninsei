//! A beaten ICE re-arms with its kind's next version when the deck steps back onto its node
//! (task 143.1).

use ninsei_core::hub::Save;
use ninsei_core::ice_version::Version;
use ninsei_core::probe::SAFE_PICK;
use ninsei_core::tuning::{STUDY_PATIENCE, TRACE_MAX};
use ninsei_core::*;

const KEY: u64 = 0x9_1494;

fn ix(d: &Data, id: &str) -> usize {
    d.ice.iter().position(|i| i.id == id).unwrap()
}

fn rng_at(g: &Game) -> u64 {
    g.rng.clone().next_u64()
}

/// Plays the fight to its end as a bot with no hands does.
fn play_out(g: &mut Game) {
    for _ in 0..500 {
        if g.phase != Phase::Combat {
            return;
        }
        let now = g.combat.as_ref().map(|c| c.turn);
        g.apply(Action::Play);
        while g.pass_open() {
            g.apply(Action::Finish);
        }
        if g.phase == Phase::Combat && g.combat.as_ref().map(|c| c.turn) == now {
            g.apply(Action::EndTurn);
        }
    }
}

/// Wins the fight under way: a turn of the program, then the ICE worn down to 1 and picked at.
fn win(g: &mut Game) {
    let now = g.combat.as_ref().unwrap().turn;
    g.apply(Action::Play);
    while g.pass_open() {
        g.apply(Action::Finish);
    }
    if g.phase == Phase::Combat && g.combat.as_ref().unwrap().turn == now {
        g.apply(Action::EndTurn);
    }
    if g.phase == Phase::Combat {
        let c = g.combat.as_mut().unwrap();
        (c.strength, c.strength_max) = (1, 1);
        play_out(g);
    }
    assert_eq!(g.phase, Phase::Exploring, "won");
}

/// A run on `seed` that beat Enigma 1.0.0 on the node next to the entry with SAFE_PICK and
/// stepped back to the entry: (the run, the beaten node). Every link runs both ways.
fn stepped_back(seed: u64) -> (Game, usize) {
    let mut g = Game::new(Data::builtin().unwrap(), seed);
    g.study_key = KEY;
    g.net.orient(seed, 0);
    g.apply(Action::Program(SAFE_PICK.into()));
    let entry = g.at;
    let node = g.net.nodes[entry].links[0];
    let e = ix(&g.data, "enigma");
    let n = &mut g.net.nodes[node];
    (n.kind, n.ice, n.cleared) = (NodeKind::Ice, Some(e), false);
    g.apply(Action::Move(node));
    win(&mut g);
    assert!(g.ice_study.iter().any(|s| s.id == "enigma" && !s.rest));
    g.apply(Action::Move(entry));
    assert_eq!(g.at, entry);
    (g, node)
}

/// Seeds whose sysops find Enigma's next version within their patience.
fn shipping_seeds() -> Vec<u64> {
    let seeds: Vec<u64> = (0..12)
        .filter(|&seed| {
            let (mut g, _) = stepped_back(seed);
            g.apply(Action::Study {
                steps: STUDY_PATIENCE,
                drafts: u32::MAX,
            });
            !g.shipped.is_empty()
        })
        .collect();
    assert!(!seeds.is_empty(), "some seed's sysops find a counter");
    seeds
}

#[test]
fn a_beaten_ice_stepped_back_onto_comes_back_as_the_sysops_next_version() {
    for seed in shipping_seeds() {
        let (g, node) = stepped_back(seed);
        let e = ix(&g.data, "enigma");
        let rng = rng_at(&g);
        // The clock gets there first...
        let mut clock = g.clone();
        clock.apply(Action::Study {
            steps: STUDY_PATIENCE,
            drafts: u32::MAX,
        });
        // The preview says it re-arms, before the step.
        let hop = g.route(node).unwrap();
        assert!(hop.rearms && !hop.back, "seed {seed}: {hop:?}");
        // ...or the step back does.
        let mut back = g.clone();
        back.apply(Action::Move(node));
        assert_eq!(back.phase, Phase::Combat, "seed {seed}");
        assert_eq!(back.shipped, clock.shipped, "seed {seed}: the same version");
        assert_eq!(
            rng_at(&back),
            rng,
            "seed {seed}: nothing drawn from the run's RNG"
        );
        let (_, was, next) = back.shipped[0].clone();
        assert_eq!(was, Version::FIRST);
        let new = was.bumped(next.bump.unwrap());
        assert_eq!(back.ice_version(e), new, "in force for the kind");
        let c = back.combat.as_ref().unwrap();
        assert_eq!((c.ice, c.ice_version), (e, new), "the node meets it");
        assert_eq!(c.ice_prog.text(), next.program);
        assert!(c.rearmed);
        assert!(!back.ice_listed(), "blind: nobody has its source");
        assert!(
            back.cracks
                .iter()
                .any(|&(i, v, left)| i == e && v == new && left > 0)
        );
        let news = |g: &Game| format!("{:?}", g.news);
        assert_eq!(news(&back), news(&clock), "the board hears of it");
        assert_ne!(news(&back), news(&g));
        // The clock first, then the step back: the version in force, and nothing more ships.
        clock.apply(Action::Move(node));
        assert_eq!(clock.combat.as_ref().unwrap().ice_version, new);
        assert_eq!(clock.shipped.len(), 1, "seed {seed}: no second version");
    }
}

#[test]
fn with_their_patience_spent_it_re_arms_at_the_version_in_force() {
    let (mut g, node) = stepped_back(1);
    for s in &mut g.ice_study {
        s.rest = true;
    }
    g.apply(Action::Move(node));
    let e = ix(&g.data, "enigma");
    assert!(g.shipped.is_empty());
    let c = g.combat.as_ref().unwrap();
    assert_eq!((c.ice, c.ice_version), (e, Version::FIRST));
    assert!(c.rearmed);
}

#[test]
fn a_re_armed_ice_pays_nothing() {
    for seed in 0..6 {
        let (mut g, node) = stepped_back(seed);
        g.apply(Action::Move(node));
        let k = Contract {
            client: "test".into(),
            brief: String::new(),
            node,
            reward: 500,
            ghost: None,
            status: ContractStatus::Active,
            paid: 0,
            betrayal: None,
        };
        g.contracts.push(k);
        let (credits, fragments, programs) = (g.deck.credits, g.deck.fragments, g.hdd.len());
        let loot = |g: &Game| g.log.iter().filter(|l| l.kind == LogKind::Loot).count();
        let before = loot(&g);
        win(&mut g);
        assert_eq!(g.deck.credits, credits, "seed {seed}: no credits");
        assert_eq!(g.deck.fragments, fragments, "seed {seed}: no fragments");
        assert_eq!(g.hdd.len(), programs, "seed {seed}: no programs");
        assert_eq!(loot(&g), before, "seed {seed}: no loot roll");
        assert_eq!(g.contracts.last().unwrap().status, ContractStatus::Active);
        assert!(g.net.nodes[node].cleared, "beaten again");
        assert!(g.bouts.last().unwrap().broken, "and the sysops read it");
    }
}

#[test]
fn a_flatline_in_a_re_armed_fight_still_brings_the_version_home() {
    let seed = shipping_seeds()[0];
    let (mut g, node) = stepped_back(seed);
    g.apply(Action::Move(node));
    let (_, was, next) = g.shipped[0].clone();
    g.deck.trace = TRACE_MAX - 1;
    while g.phase == Phase::Combat {
        g.apply(Action::EndTurn);
    }
    assert_eq!(g.phase, Phase::Flatlined);
    let d = Data::builtin().unwrap();
    let mut save = Save::default();
    save.ensure_key(KEY);
    save.ensure_phonebook(&d, 0);
    save.ensure_hardware(&d);
    save.record(&g, &[], &d.rig).unwrap();
    let (v, program) = save.ice_program("enigma", &d).unwrap();
    assert_eq!(v, was.bumped(next.bump.unwrap()));
    assert_eq!(program, next.program);
}
