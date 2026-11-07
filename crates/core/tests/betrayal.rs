//! Contracts can turn on you: when a target falls, the client may double-cross the runner.

use ninsei_core::data::{BetrayalEffect, Sources};
use ninsei_core::hub::Save;
use ninsei_core::tuning::*;
use ninsei_core::*;

const CONTRACTS: &str = include_str!("../../../data/contracts.ron");

/// The built-in contracts with `betrayals` swapped for `list`.
fn with_betrayals(list: &str) -> Result<Data, String> {
    let at = CONTRACTS.find("    betrayals: [").unwrap();
    let src = format!("{}    betrayals: [{list}],\n)\n", &CONTRACTS[..at]);
    Data::parse(Sources {
        contracts: &src,
        ..Sources::builtin()
    })
}

fn only(effect: &str) -> Data {
    with_betrayals(&format!(
        "(id: \"x\", label: \"X\", weight: 1, min_tier: 2, effect: {effect}, says: \"{{client}} did it\")"
    ))
    .unwrap()
}

fn run_on(data: Data, tier: u32, seed: u64, trust: &[(&str, u32)]) -> Game {
    Game::start(
        data,
        seed,
        &Kit {
            site: Some(Site {
                entry: 0,
                name: "test".into(),
                tier,
                traits: Vec::new(),
            }),
            trust: trust.iter().map(|(c, n)| (c.to_string(), *n)).collect(),
            ..Kit::default()
        },
    )
}

fn job(node: usize, reward: u32, ghost: Option<(u32, u32)>) -> Contract {
    Contract {
        client: "Maas".into(),
        brief: String::new(),
        node,
        reward,
        ghost,
        status: ContractStatus::Active,
        paid: 0,
        betrayal: None,
    }
}

/// Takes a contract on the first node ahead, makes it ICE, walks onto it and breaks it.
fn fell(mut g: Game, reward: u32) -> Game {
    let target = g.net.nodes[g.at].links[0];
    for n in &mut g.net.nodes {
        if n.id != g.at && !matches!(n.kind, NodeKind::Exit | NodeKind::Bbs) {
            n.kind = NodeKind::Vault;
            n.cleared = true;
            n.ice = None;
        }
    }
    let t = &mut g.net.nodes[target];
    t.kind = NodeKind::Ice;
    t.ice = Some(0);
    t.cleared = false;
    let k = job(target, reward, None);
    g.contracts.push(k);
    g.deck.integrity = 999;
    g.apply(Action::Move(target));
    assert_eq!(g.phase, Phase::Combat);
    g.combat.as_mut().unwrap().strength = 1;
    let i = g
        .hdd
        .iter()
        .position(|s| s.loaded && matches!(s.spec.effect, Effect::Break { .. }))
        .unwrap();
    g.apply(Action::Run(i));
    assert_ne!(g.contracts[0].status, ContractStatus::Active);
    g
}

/// The first seed whose client turns on you.
fn betrayed(data: &Data, tier: u32, reward: u32) -> Game {
    (0..400)
        .map(|seed| fell(run_on(data.clone(), tier, seed, &[]), reward))
        .find(|g| g.contracts[0].status == ContractStatus::Betrayed)
        .expect("nobody ever turned")
}

#[test]
fn the_chance_follows_tier_pay_trust_and_the_ghost_bonus() {
    let d = Data::builtin().unwrap();
    let pct = |tier, reward, ghost, trust: &[(&str, u32)]| {
        let g = run_on(d.clone(), tier, 1, trust);
        let k = job(1, reward, ghost);
        g.betray_pct(&k)
    };
    // Higher tiers and bigger jobs are riskier.
    assert!(pct(1, 100, None, &[]) < pct(2, 100, None, &[]));
    assert!(pct(2, 100, None, &[]) < pct(3, 100, None, &[]));
    assert!(pct(3, 100, None, &[]) < pct(3, 400, None, &[]));
    // A client you've worked for cleanly trusts you back.
    assert!(pct(3, 100, None, &[("Maas", 2)]) < pct(3, 100, None, &[]));
    assert!(pct(3, 100, None, &[("Someone else", 5)]) == pct(3, 100, None, &[]));
    // Earning the ghost bonus halves it (trace starts at 0, under the cap).
    let ghost = Some((50, 0));
    assert_eq!(pct(3, 100, ghost, &[]), pct(3, 100, None, &[]));
    let ghost = Some((50, 20));
    assert!(pct(3, 100, ghost, &[]) < pct(3, 120, None, &[]));
    // Never more than the cap.
    assert!(pct(3, 100_000, None, &[]) <= BETRAY_MAX_PCT);
}

#[test]
fn tier_one_only_ever_shortchanges_you() {
    let d = Data::builtin().unwrap();
    let mut seen = 0;
    for seed in 0..300 {
        let g = fell(run_on(d.clone(), 1, seed, &[]), 100);
        let k = &g.contracts[0];
        match k.status {
            ContractStatus::Done => assert_eq!(k.paid, 100),
            ContractStatus::Betrayed => {
                seen += 1;
                let b = &d.betrayals[k.betrayal.unwrap()];
                assert!(b.min_tier <= 1);
                assert!(matches!(b.effect, BetrayalEffect::Stiffed(_)));
                assert!(k.paid < 100);
            }
            s => panic!("settled as {s:?}"),
        }
    }
    assert!(seen > 0, "tier 1 never turned on you");
    assert!(seen < 60, "tier 1 turned on you {seen} times in 300");
}

#[test]
fn stiffed_pays_short() {
    let g = betrayed(&only("Stiffed(0)"), 3, 200);
    assert_eq!(g.contracts[0].paid, 0);
    assert_eq!(g.deck.credits, 0);
    assert_eq!(g.betrayal_label(&g.contracts[0]), Some("X"));
    assert!(g.log.iter().any(|l| l.text.contains("Maas did it")));
}

#[test]
fn burned_raises_trace_and_the_ice() {
    let d = only("Burned(trace: 15, ice: 4)");
    let before = run_on(d.clone(), 3, 0, &[]).mods.ice_bonus;
    let g = betrayed(&d, 3, 200);
    assert_eq!(g.contracts[0].paid, 200);
    assert_eq!(g.mods.ice_bonus, before + 4);
    assert!(g.deck.trace >= 15);
}

#[test]
fn a_setup_rezzes_a_hunter_on_your_node() {
    let d = only("Setup(\"archer\")");
    let g = betrayed(&d, 3, 200);
    assert_eq!(g.phase, Phase::Combat);
    let c = g.combat.as_ref().unwrap();
    assert_eq!(c.node, g.at);
    assert_eq!(d.ice[c.ice].id, "archer");
    // Still paid; the fight resolves like any other.
    assert_eq!(g.contracts[0].paid, 200);
}

#[test]
fn selling_you_out_heats_the_number_and_betrayals_cost_trust() {
    let d = only("SoldOut(3)");
    let mut s = Save::default();
    s.ensure_key(1);
    s.ensure_phonebook(&d, 1);
    s.add_starters(&d);
    s.learn_board("gutter", "test");
    s.ensure_hardware(&d);
    let entry = s.phonebook[0].id;
    let mut g = betrayed(&d, 3, 200);
    assert_eq!(g.sold_out, 3);
    g.site.as_mut().unwrap().entry = entry;
    g.phase = Phase::JackedOut;
    s.trust = vec![("Maas".into(), 4)];
    s.record(&g, &[], &d.rig).unwrap();
    let heat = s.phonebook.iter().find(|e| e.id == entry).unwrap().heat;
    assert_eq!(heat, (HEAT_PER_RUN + 3).min(HEAT_MAX));
    assert_eq!(
        s.trust,
        vec![("Maas".to_string(), 0)],
        "a betrayal resets trust"
    );

    // A clean job earns a point.
    let mut g = (0..)
        .map(|seed| fell(run_on(Data::builtin().unwrap(), 1, seed, &[]), 100))
        .find(|g| g.contracts[0].status == ContractStatus::Done)
        .unwrap();
    g.phase = Phase::JackedOut;
    s.record(&g, &[], &d.rig).unwrap();
    assert_eq!(s.trust, vec![("Maas".to_string(), 1)]);
}

#[test]
fn bad_betrayals_are_caught() {
    let err = |list: &str| with_betrayals(list).unwrap_err();
    assert!(
        err("(id: \"a\", label: \"A\", weight: 1, min_tier: 1, effect: SoldOut(2), says: \"\")")
            .contains("tier 1")
    );
    assert!(
        err(
            "(id: \"a\", label: \"A\", weight: 1, min_tier: 3, effect: Setup(\"nope\"), says: \"\")"
        )
        .contains("unknown ICE `nope`")
    );
    assert!(
        err("(id: \"a\", label: \"A\", weight: 0, min_tier: 2, effect: Stiffed(0), says: \"\")")
            .contains("weight 0")
    );
    assert!(
        err("(id: \"a\", label: \"A\", weight: 1, min_tier: 2, effect: Stiffed(100), says: \"\")")
            .contains("under 100%")
    );
}
