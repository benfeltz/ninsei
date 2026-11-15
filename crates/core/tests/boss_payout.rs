//! 138.3: beating the net's AI: the payout, the net's sources with its notes, its ICE DB page.

use ninsei_core::boss::Hunter;
use ninsei_core::game::BossState;
use ninsei_core::hub::{Entry, Save};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

fn save(tier: u32) -> Save {
    let mut s = Save::default();
    s.ensure_key(0x1383);
    s.ensure_hardware(&data());
    s.phonebook.push(Entry {
        id: 0,
        name: "probe".into(),
        business: "test".into(),
        number: "555-0000".into(),
        tier,
        traits: Vec::new(),
        heat: 0,
        runs: 0,
        last: Vec::new(),
    });
    s
}

/// A run on `s`'s number with the AI awake and holding the runner's next node: stepping on
/// opens its fight.
fn facing_it(s: &mut Save, seed: u64) -> Game {
    let d = data();
    let mut g = s.dial(&d, 0, seed).unwrap();
    let to = g.net.nodes[g.at].links[0];
    g.net.nodes[to].kind = NodeKind::Vault;
    g.net.nodes[to].ice = None;
    g.net.nodes[to].cleared = true;
    let ice = g.boss.as_ref().unwrap().ice;
    let mut h = Hunter::wake(ice, &g.data.ice[ice], to, g.at);
    h.prog = basic::parse_with("10 HOLD", g.data.ice[ice].vocab());
    g.boss.as_mut().unwrap().state = BossState::Awake(h);
    g.apply(Action::Move(to));
    assert_eq!(g.phase, Phase::Combat);
    g
}

/// Beats the AI in front of the runner.
fn beat(g: &mut Game) {
    g.combat.as_mut().unwrap().strength = 1;
    let pick = g.slot_named("icepick").unwrap();
    g.apply(Action::Run(pick));
    assert_eq!(
        g.boss.as_ref().unwrap().state,
        BossState::Down,
        "{:?}",
        g.log.last()
    );
}

#[test]
fn beating_it_pays_the_nets_big_payout_once() {
    for tier in 1..=3 {
        let mut s = save(tier);
        let mut g = facing_it(&mut s, 3);
        let before = g.deck.credits;
        let name = g.data.ice[g.boss.as_ref().unwrap().ice].name.clone();
        beat(&mut g);
        let pay = g.mods.pay(tuning::AI_PAY[tier as usize - 1]);
        let line = format!("+ {pay}¢ from {name}'s accounts.");
        assert_eq!(
            g.log.iter().filter(|l| l.text == line).count(),
            1,
            "tier {tier}"
        );
        assert!(
            g.deck.credits >= before + pay,
            "the payout and whatever the loot rolled"
        );
    }
}

#[test]
fn losing_to_it_pays_nothing() {
    let mut s = save(1);
    let mut g = facing_it(&mut s, 4);
    g.deck.integrity = 1;
    for _ in 0..20 {
        if g.phase != Phase::Combat {
            break;
        }
        g.apply(Action::EndTurn);
    }
    assert_eq!(g.phase, Phase::Flatlined);
    assert!(!g.log.iter().any(|l| l.text.contains("'s accounts")));
}
