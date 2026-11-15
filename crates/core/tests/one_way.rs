//! Links both ways, and some one way (task 143.1).

use ninsei_core::hub::{self, Save};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

/// A save with one phonebook entry of `tier` with exactly `traits`.
fn site(tier: u32, traits: &[&str]) -> Save {
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
    s
}

const TRAITS: [&[&str]; 6] = [
    &[],
    &["mesh"],
    &["chokepoints"],
    &["daisy_chain"],
    &["segmented"],
    &["no_bbs", "vault_rich"],
];

fn fold(h: &mut u64, s: &str) {
    for b in s.bytes() {
        *h = (*h ^ b as u64).wrapping_mul(0x100_0000_01b3);
    }
}

fn fold_net(h: &mut u64, net: &Network) {
    for n in &net.nodes {
        let s = format!(
            "{}/{}/{:?}/{}/{:?}/{}/{:?}",
            n.layer, n.lane, n.kind, n.name, n.ice, n.cleared, n.links
        );
        fold(h, &s);
    }
}

/// Every tier, a spread of traits and 120 seeds: the nodes, their ICE and names, the set of
/// links (subnets' too) and where the run's RNG stands once the run is built are exactly what
/// main generated before links had a direction. Only which links are one way is new.
#[test]
fn a_seed_lays_out_the_network_main_did() {
    let d = std::sync::Arc::new(data());
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for tier in 1..=3 {
        for traits in TRAITS {
            for seed in 0..120u64 {
                let mut s = site(tier, traits);
                let mut g = s.dial(&d, 0, seed).unwrap();
                fold_net(&mut h, &g.net);
                fold(&mut h, &format!("{:x}", g.rng.next_u64()));
                for n in g.net.nodes.iter().filter(|n| n.kind == NodeKind::Subnet) {
                    fold_net(&mut h, &Network::subnet(seed, n.id, &d, &g.params));
                }
            }
        }
    }
    assert_eq!(
        h, 0x3bc9_3827_41ad_6318,
        "a seed lays out the network main did"
    );
}

/// The share of one-way links on many networks of each tier is near the tier's percent.
#[test]
fn a_tiers_share_of_links_is_one_way() {
    let d = std::sync::Arc::new(data());
    for tier in 1..=3 {
        let want = d.networks.tier(tier).unwrap().one_way;
        let (mut links, mut one_way) = (0, 0);
        for seed in 0..300u64 {
            let g = site(tier, &[]).dial(&d, 0, seed).unwrap();
            for n in &g.net.nodes {
                links += n.links.len();
                one_way += n.one_way.len();
                assert!(n.one_way.iter().all(|b| n.links.contains(b)));
            }
        }
        let got = one_way * 100 / links;
        assert!(
            got.abs_diff(want as usize) <= 3,
            "tier {tier}: {got}% one way, wants {want}%"
        );
    }
}

/// None at 0 percent, every one at 100, and the draw is the seed's.
#[test]
fn the_percent_runs_from_none_to_all() {
    let d = data();
    for seed in 0..40u64 {
        let p = NetParams::default();
        let net = Network::generate_with(&mut rng::Rng::new(seed), &d, &p);
        let mut none = net.clone();
        none.orient(seed, 0);
        assert!(none.nodes.iter().all(|n| n.one_way.is_empty()));
        let mut all = net.clone();
        all.orient(seed, 100);
        assert!(all.nodes.iter().all(|n| n.one_way == n.links));
        let (mut a, mut b) = (net.clone(), net);
        a.orient(seed, 50);
        b.orient(seed, 50);
        let ways = |n: &Network| {
            n.nodes
                .iter()
                .map(|x| x.one_way.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(ways(&a), ways(&b), "same seed, same one-way links");
    }
}

/// A one-way link only points deeper, so from every node, subnets' too, the exit is reachable
/// along the routes the runner may take: no choice strands the runner.
#[test]
fn the_exit_is_always_ahead() {
    let d = std::sync::Arc::new(data());
    for tier in 1..=3 {
        for traits in TRAITS {
            for seed in 0..60u64 {
                let g = site(tier, traits).dial(&d, 0, seed).unwrap();
                let mut nets = vec![g.net.clone()];
                for n in g.net.nodes.iter().filter(|n| n.kind == NodeKind::Subnet) {
                    nets.push(Network::subnet(seed, n.id, &d, &g.params));
                }
                for net in &nets {
                    let exit = *net.layers.last().unwrap().first().unwrap();
                    for n in &net.nodes {
                        assert!(
                            net.routes_from(n.id)[exit],
                            "tier {tier} {traits:?} seed {seed}: node {} is stranded",
                            n.id
                        );
                        for to in net.routes(n.id) {
                            let deeper = net.nodes[to].layer == n.layer + 1;
                            let back = net.nodes[to].layer + 1 == n.layer;
                            assert!(deeper || back, "a route is one layer away");
                            if back {
                                assert!(
                                    !net.one_way(to, n.id),
                                    "a one-way link is never walked back"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

/// A trait's shape may say its own share, and a share over 100 is refused.
#[test]
fn a_bad_one_way_share_is_refused() {
    let mut d = data();
    d.validate().unwrap();
    d.networks.tiers[0].one_way = 101;
    assert!(d.validate().unwrap_err().contains("one-way"));
    let mut d = data();
    let shape = Shape {
        one_way: Some(101),
        ..Shape::default()
    };
    d.networks.traits[0].effect = hub::TraitEffect::Shape(shape);
    assert!(d.validate().unwrap_err().contains("one-way"));
    let p = NetParams {
        shape: Shape {
            one_way: Some(90),
            ..Shape::default()
        },
        ..NetParams::default()
    };
    assert_eq!(p.one_way(), 90, "the shape's share wins over the tier's");
    assert_eq!(NetParams::default().one_way(), tuning::NET_ONE_WAY_PCT);
}
