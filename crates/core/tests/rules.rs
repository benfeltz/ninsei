use ninsei_core::*;

fn game(seed: u64) -> Game {
    Game::new(Data::builtin().expect("builtin data parses"), seed)
}

#[test]
fn builtin_data_is_valid() {
    let d = Data::builtin().unwrap();
    assert!(d.warez.len() >= 6);
    assert!(d.ice.len() >= 4);
}

#[test]
fn bad_loot_reference_is_rejected() {
    let loot = include_str!("../../../data/loot.ron").replace("\"torch\"", "\"nope\"");
    let err = Data::parse(Sources {
        warez: include_str!("../../../data/warez.ron"),
        ice: include_str!("../../../data/ice.ron"),
        loot: &loot,
        bbs: include_str!("../../../data/bbs/board.ron"),
        splash: include_str!("../../../data/bbs/splash.txt"),
        events: include_str!("../../../data/events.ron"),
        contracts: include_str!("../../../data/contracts.ron"),
        icons: include_str!("../../../data/hub/icons.ron"),
        rig: include_str!("../../../data/hub/rig.ron"),
        networks: include_str!("../../../data/hub/networks.ron"),
        exchanges: include_str!("../../../data/hub/exchanges.ron"),
        themes: include_str!("../../../data/themes.ron"),
    })
    .unwrap_err();
    assert!(err.contains("nope"), "{err}");
}

#[test]
fn same_seed_same_network() {
    let (a, b) = (game(42), game(42));
    let names = |g: &Game| {
        g.net
            .nodes
            .iter()
            .map(|n| n.name.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(names(&a), names(&b));
    assert_ne!(names(&a), names(&game(43)));
}

#[test]
fn every_node_is_reachable_and_leads_to_exit() {
    for seed in 0..300 {
        let g = game(seed);
        let net = &g.net;
        let last = net.layers.len() - 1;
        for n in &net.nodes {
            if n.layer < last {
                assert!(
                    !n.links.is_empty(),
                    "seed {seed}: node {} is a dead end",
                    n.id
                );
                assert!(n.links.iter().all(|&l| net.nodes[l].layer == n.layer + 1));
            }
            if n.layer > 0 {
                let incoming = net.nodes.iter().any(|m| m.links.contains(&n.id));
                assert!(incoming, "seed {seed}: node {} is unreachable", n.id);
            }
        }
        assert_eq!(net.nodes[net.layers[last][0]].kind, NodeKind::Exit);
    }
}

#[test]
fn starting_loadout_fits_in_ram() {
    let g = game(1);
    assert!(g.ram_used() <= g.deck.ram_max);
    assert!(g.hdd.iter().any(|s| s.loaded));
}

#[test]
fn cannot_move_to_unlinked_node_or_leave_a_fight() {
    let mut g = game(7);
    let exit = *g.net.layers.last().unwrap().first().unwrap();
    g.apply(Action::Move(exit));
    assert_eq!(g.at, g.net.entry(), "skipping to the exit must be refused");

    let target = g.net.nodes[g.at].links[0];
    g.net.nodes[target].kind = NodeKind::Ice;
    g.net.nodes[target].ice = Some(0);
    g.net.nodes[target].cleared = false;
    g.apply(Action::Move(target));
    assert_eq!(g.phase, Phase::Combat);
    let next = g.net.nodes[target].links[0];
    g.apply(Action::Move(next));
    assert_eq!(g.at, target, "moving mid-fight must be refused");
}

/// Plays a run by always attacking with the strongest loaded breaker and picking the first link.
fn autoplay(seed: u64) -> Game {
    let mut g = game(seed);
    for _ in 0..2000 {
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
                    None => g.apply(Action::EndTurn),
                }
            }
            Phase::Encounter => {
                let leave = g.leave_choice().unwrap();
                g.apply(Action::Choose {
                    choice: leave,
                    slot: None,
                });
            }
            Phase::Flatlined | Phase::JackedOut => return g,
        }
    }
    panic!("seed {seed}: run never ended");
}

#[test]
fn runs_always_end() {
    let mut wins = 0;
    for seed in 0..200 {
        let g = autoplay(seed);
        if g.phase == Phase::JackedOut {
            wins += 1;
        }
    }
    // A brainless bot should win sometimes but not always.
    assert!(wins > 0, "the bot never wins: too hard");
    assert!(wins < 200, "the bot always wins: too easy");
}

#[test]
fn class_matched_breaker_hits_double() {
    let mut g = game(3);
    let target = g.net.nodes[g.at].links[0];
    let gate = g
        .data
        .ice
        .iter()
        .position(|i| i.class == IceClass::CodeGate)
        .unwrap();
    g.net.nodes[target].kind = NodeKind::Ice;
    g.net.nodes[target].ice = Some(gate);
    g.net.nodes[target].cleared = false;
    g.apply(Action::Move(target));
    let decoder = g
        .hdd
        .iter()
        .position(|s| g.data.warez[s.warez].id == "decoder")
        .unwrap();
    g.hdd[decoder].loaded = true;
    let before = g.combat.as_ref().unwrap().strength;
    g.apply(Action::Run(decoder));
    let after = g.combat.as_ref().map_or(0, |c| c.strength);
    assert_eq!(before - after, 8.min(before));
}

#[test]
fn shield_blocks_attack() {
    let mut g = game(5);
    let target = g.net.nodes[g.at].links[0];
    let turret = g
        .data
        .ice
        .iter()
        .position(|i| i.id == "rototurret")
        .unwrap();
    g.net.nodes[target].kind = NodeKind::Ice;
    g.net.nodes[target].ice = Some(turret);
    g.net.nodes[target].cleared = false;
    g.apply(Action::Move(target));
    let fw = g
        .hdd
        .iter()
        .position(|s| g.data.warez[s.warez].id == "firewall")
        .unwrap();
    let hp = g.deck.integrity;
    g.apply(Action::Run(fw)); // shield 5 vs attack 4
    g.apply(Action::EndTurn);
    assert_eq!(g.deck.integrity, hp);
}

#[test]
fn commands_parse() {
    assert_eq!(parse_command("run 2"), Ok(Command::Game(Action::Run(2))));
    assert_eq!(parse_command("e"), Ok(Command::Game(Action::EndTurn)));
    assert_eq!(parse_command("jack 4"), Ok(Command::Game(Action::Move(4))));
    assert_eq!(
        parse_command("newrun ff"),
        Ok(Command::Game(Action::NewRun(255)))
    );
    assert_eq!(parse_command("newrun"), Ok(Command::NewRun));
    assert!(parse_command("run x").is_err());
    assert!(parse_command("sudo").is_err());
}
