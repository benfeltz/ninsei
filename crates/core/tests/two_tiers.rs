//! Acceptance for 58.5: two tiers of machine. A save from before keeps working, and climbing
//! is one purchase to a deck at the one ceiling.

use ninsei_core::hardware::Hardware;
use ninsei_core::hub::{self, Save, Upgrade};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

/// A save written before the four extra models left: it owns all of them and sits on one.
fn old_save(d: &Data) -> Save {
    let mut s = Save::default();
    s.ensure_key(0x0585);
    s.ensure_hardware(d);
    for id in [
        "onosendai5",
        "onosendai",
        "onosendai9",
        "hosaka2",
        "hosaka6",
    ] {
        s.decks.push(id.into());
    }
    s.deck = "onosendai9".into();
    s.firmware = "6.2".into();
    Save::load(&s.to_text()).unwrap()
}

#[test]
fn a_save_from_before_the_two_tiers_loads_and_plays() {
    let d = data();
    let mut s = old_save(&d);
    s.ensure_hardware(&d);
    // Only decks the data sells are left; the one it still sells stays owned.
    assert!(
        s.decks.iter().all(|id| d.hardware.deck(id).is_some()),
        "{:?}",
        s.decks
    );
    assert!(s.decks.iter().any(|id| id == "onosendai"));
    for gone in ["onosendai5", "onosendai9", "hosaka2", "hosaka6"] {
        assert!(!s.decks.iter().any(|id| id == gone), "{gone} is a ghost");
    }
    // The Shell's deck list (what System lists too) names no ghost.
    let (lines, _) = hub::shell("deck", &s, &d);
    for gone in ["onosendai5", "onosendai9", "hosaka2", "hosaka6"] {
        assert!(!lines.iter().any(|l| l.contains(gone)), "{gone}: {lines:?}");
    }
    // It sits on a deck the data has, on firmware that deck runs, and a run plays.
    let deck = s.deck_def(&d);
    assert!(deck.firmware.iter().any(|f| f.id == s.firmware));
    let mut g = s.jack_in(&d, 9);
    assert_eq!(g.phase, Phase::Exploring);
    let next = g.net.nodes[g.at].links[0];
    g.apply(Action::Move(next));
    assert_eq!(g.at, next);
}

#[test]
fn climbing_is_one_purchase_and_both_real_decks_reach_one_ceiling() {
    let d = data();
    let starter = d.hardware.starter_deck();
    let lines = [Upgrade::Cpu, Upgrade::Ram, Upgrade::Hdd, Upgrade::Integrity];
    let mut ceilings = Vec::new();
    for deck in d.hardware.decks.iter().filter(|x| !x.starter) {
        let mut s = Save::default();
        s.ensure_key(1);
        s.ensure_hardware(&d);
        s.bank = deck.price;
        s.buy_hardware(&d, &Hardware::Deck(deck.id.clone()))
            .unwrap();
        s.use_deck(&d, &deck.id).unwrap();
        let caps: Vec<_> = lines.iter().map(|&u| s.stat_cap(&d, u)).collect();
        for (&u, cap) in lines.iter().zip(&caps) {
            let low = starter.stat(u).unwrap().max;
            assert!(cap.unwrap() > low, "{} {u:?} is above the starter", deck.id);
        }
        ceilings.push((deck.id.clone(), caps));
    }
    assert!(ceilings.len() >= 2, "more than one real deck to pick from");
    assert!(
        ceilings.windows(2).all(|w| w[0].1 == w[1].1),
        "every real deck tops out at the same place: {ceilings:?}"
    );
    // Owning both buys nothing more: the second real deck takes no line further.
    let mut s = Save::default();
    s.ensure_key(2);
    s.ensure_hardware(&d);
    s.bank = 1_000_000;
    let real: Vec<_> = d.hardware.decks.iter().filter(|x| !x.starter).collect();
    s.buy_hardware(&d, &Hardware::Deck(real[0].id.clone()))
        .unwrap();
    s.use_deck(&d, &real[0].id).unwrap();
    let first: Vec<_> = lines.iter().map(|&u| s.stat_cap(&d, u)).collect();
    s.buy_hardware(&d, &Hardware::Deck(real[1].id.clone()))
        .unwrap();
    s.use_deck(&d, &real[1].id).unwrap();
    let second: Vec<_> = lines.iter().map(|&u| s.stat_cap(&d, u)).collect();
    assert_eq!(first, second);
}
