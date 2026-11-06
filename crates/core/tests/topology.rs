//! Network shapes and subnets (issue #11).

use ninsei_core::hub::{self, Save};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

/// A save with one phonebook entry of `tier` with exactly `traits`.
fn site(tier: u32, traits: &[&str]) -> (Data, Save) {
    let d = data();
    let mut s = Save::default();
    s.ensure_key(7);
    s.phonebook.push(hub::Entry {
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
    (d, s)
}

fn assert_well_formed(net: &Network, what: &str) {
    let last = net.layers.len() - 1;
    for n in &net.nodes {
        if n.layer < last {
            assert!(!n.links.is_empty(), "{what}: node {} is a dead end", n.id);
            assert!(n.links.iter().all(|&l| net.nodes[l].layer == n.layer + 1));
        }
        if n.layer > 0 {
            let incoming = net.nodes.iter().any(|m| m.links.contains(&n.id));
            assert!(incoming, "{what}: node {} is unreachable", n.id);
        }
    }
    assert_eq!(net.nodes[net.layers[last][0]].kind, NodeKind::Exit);
}

#[test]
fn default_networks_are_unchanged() {
    // Fingerprint of the first 50 default networks before shapes and subnets existed.
    // Same seed, same network: old seeds must keep replaying.
    let d = data();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for seed in 0..50u64 {
        let g = Game::new(d.clone(), seed);
        for n in &g.net.nodes {
            let s = format!("{}{:?}{}{:?}{:?}", n.layer, n.kind, n.name, n.ice, n.links);
            for b in s.bytes() {
                h = (h ^ b as u64).wrapping_mul(0x100_0000_01b3);
            }
        }
    }
    assert_eq!(h, 0xae60_d5ab_f919_7ee7);
}

#[test]
fn shapes_change_the_layout() {
    let width = |net: &Network, l: usize| net.layers[l].len();
    for seed in 0..80 {
        let (d, s) = site(2, &["mesh"]);
        let g = s.clone().dial(&d, 0, seed).unwrap();
        assert_well_formed(&g.net, "mesh");
        let mid = 1..g.net.layers.len() - 1;
        assert!(mid.clone().all(|l| (3..=5).contains(&width(&g.net, l))));

        let (d, s) = site(3, &["chokepoints"]);
        let g = s.clone().dial(&d, 0, seed).unwrap();
        assert_well_formed(&g.net, "chokepoints");
        for l in [2, 4, 6] {
            assert_eq!(
                width(&g.net, l),
                1,
                "seed {seed}: layer {l} is a chokepoint"
            );
            let n = &g.net.nodes[g.net.layers[l][0]];
            assert!(
                matches!(n.kind, NodeKind::Ice | NodeKind::BlackIce),
                "seed {seed}: chokepoint on layer {l} is {:?}",
                n.kind
            );
        }
        assert!(g.net.bbs().is_some(), "a chokepoint never takes the BBS");

        let (d, s) = site(2, &["daisy_chain"]);
        let g = s.clone().dial(&d, 0, seed).unwrap();
        assert_well_formed(&g.net, "daisy chain");
        assert!(mid.clone().all(|l| width(&g.net, l) <= 2));
    }
}

#[test]
fn tiers_and_traits_place_subnets() {
    let count = |g: &Game| {
        g.net
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Subnet)
            .count()
    };
    for seed in 0..40 {
        let (d, s) = site(1, &[]);
        assert_eq!(count(&s.clone().dial(&d, 0, seed).unwrap()), 0);
        let (d, s) = site(2, &[]);
        let g = s.clone().dial(&d, 0, seed).unwrap();
        assert_eq!(count(&g), 1);
        let sub = g.net.nodes.iter().find(|n| n.kind == NodeKind::Subnet);
        assert!(sub.unwrap().layer >= tuning::SUBNET_MIN_LAYER);
        let (d, s) = site(2, &["segmented"]);
        assert_eq!(count(&s.clone().dial(&d, 0, seed).unwrap()), 2);
    }
}

/// A tier-2 run standing right next to its subnet node, with the way there cleared.
fn next_to_subnet(seed: u64) -> (Game, usize) {
    let (d, s) = site(2, &[]);
    let mut g = s.clone().dial(&d, 0, seed).unwrap();
    let sub = g
        .net
        .nodes
        .iter()
        .position(|n| n.kind == NodeKind::Subnet)
        .unwrap();
    let before = *g.net.layers[g.net.nodes[sub].layer - 1]
        .iter()
        .find(|&&n| g.net.nodes[n].links.contains(&sub))
        .unwrap();
    g.at = before;
    (g, sub)
}

/// Walks the current network to its exit along first links, with nothing in the way.
fn walk_out(g: &mut Game) {
    for n in &mut g.net.nodes {
        if n.kind != NodeKind::Exit {
            n.kind = NodeKind::Vault;
            n.cleared = true;
            n.ice = None;
        }
    }
    let last = g.net.layers.len() - 1;
    for _ in g.net.nodes[g.at].layer..last {
        let to = g.net.nodes[g.at].links[0];
        g.apply(Action::Move(to));
    }
}

#[test]
fn a_subnet_is_a_side_trip_that_comes_back() {
    for seed in 0..20 {
        let (mut g, sub) = next_to_subnet(seed);
        let main_names: Vec<String> = g.net.nodes.iter().map(|n| n.name.clone()).collect();
        g.apply(Action::Move(sub));
        assert!(g.in_subnet(), "seed {seed}: should be inside");
        assert_eq!(g.at, g.net.entry());
        assert_eq!(g.net.layers.len(), tuning::SUBNET_MID_LAYERS + 2);
        assert!(g.net.bbs().is_none(), "no BBS in a subnet");
        assert!(g.net.nodes.iter().all(|n| n.kind != NodeKind::Subnet));
        assert_well_formed(&g.net, "subnet");
        assert_eq!(g.root().nodes[g.root_at()].kind, NodeKind::Subnet);

        let trace = g.deck.trace;
        walk_out(&mut g);
        assert_eq!(g.phase, Phase::Exploring, "the uplink doesn't end the run");
        assert!(!g.in_subnet());
        assert_eq!(g.at, sub);
        assert!(g.net.nodes[sub].cleared);
        assert!(g.deck.trace > trace, "trace carries through the subnet");
        let names: Vec<String> = g.net.nodes.iter().map(|n| n.name.clone()).collect();
        assert_eq!(names, main_names, "the main network comes back as it was");

        // On through the main network to the real exit.
        walk_out(&mut g);
        assert_eq!(g.phase, Phase::JackedOut);
    }
}

#[test]
fn a_subnet_is_the_same_every_time() {
    let (g, sub) = next_to_subnet(5);
    let inside = |mut g: Game| {
        g.apply(Action::Move(sub));
        g.net
            .nodes
            .iter()
            .map(|n| (n.name.clone(), n.kind, n.links.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(inside(g.clone()), inside(g));
}

#[test]
fn subnet_ice_hits_harder_than_the_same_layer_outside() {
    let (mut g, sub) = next_to_subnet(3);
    let ice = g.data.ice.iter().position(|i| i.tier == 1).unwrap();
    let door = g.net.nodes[sub].layer as u32;
    g.apply(Action::Move(sub));
    let to = g.net.nodes[g.at].links[0];
    g.net.nodes[to].kind = NodeKind::Ice;
    g.net.nodes[to].ice = Some(ice);
    g.net.nodes[to].cleared = false;
    g.apply(Action::Move(to));
    let c = g.combat.as_ref().expect("fight");
    let base = g.data.ice[ice].strength + g.mods.ice_bonus;
    assert_eq!(
        c.strength,
        base + tuning::ICE_STRENGTH_PER_LAYER * (door + 1)
    );
}

#[test]
fn contracts_on_the_main_network_survive_a_subnet() {
    for seed in 0..20 {
        let (mut g, sub) = next_to_subnet(seed);
        // A target past the subnet, on the main network.
        let target = g.net.nodes[sub].links[0];
        g.contracts.push(Contract {
            client: "fixer".into(),
            brief: String::new(),
            node: target,
            reward: 100,
            ghost: None,
            status: ContractStatus::Active,
            paid: 0,
            betrayal: None,
        });
        g.apply(Action::Move(sub));
        // Inside, node ids belong to the subnet: neither a matching id nor
        // the main target being "unreachable" here may touch the contract.
        walk_out(&mut g);
        assert_eq!(g.contracts[0].status, ContractStatus::Active, "seed {seed}");
        assert_eq!(g.at, sub);
        assert!(g.net.reachable_from(g.at)[target]);
    }
}

#[test]
fn bad_shapes_are_rejected() {
    let ok = Shape {
        min_width: 2,
        max_width: 4,
        links: 35,
        choke: 0,
    };
    assert!(ok.validate().is_ok());
    for bad in [
        Shape { min_width: 0, ..ok },
        Shape { min_width: 5, ..ok },
        Shape {
            max_width: tuning::NET_WIDEST + 1,
            ..ok
        },
        Shape { links: 101, ..ok },
    ] {
        assert!(bad.validate().is_err(), "{bad:?}");
    }
    assert_eq!(
        Shape::default(),
        ok,
        "the default shape is the classic network"
    );
}
