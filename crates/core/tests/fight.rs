//! The fight view's rules: the preview matches what a program does, and the wireframe comes
//! apart with the ICE's strength.

use ninsei_core::fight::{self, Match, Outcome};
use ninsei_core::*;

/// A game in a fight with ICE `ice` on the first node past the entry.
fn fighting(seed: u64, ice: usize) -> Game {
    let mut g = Game::new(Data::builtin().unwrap(), seed);
    let target = g.net.nodes[g.at].links[0];
    g.net.nodes[target].kind = NodeKind::Ice;
    g.net.nodes[target].ice = Some(ice);
    g.net.nodes[target].cleared = false;
    g.apply(Action::Move(target));
    assert_eq!(g.phase, Phase::Combat);
    g
}

#[test]
fn the_preview_is_what_happens() {
    let data = Data::builtin().unwrap();
    for ice in 0..data.ice.len() {
        for seed in 0..6 {
            let g = fighting(seed, ice);
            for slot in 0..g.hdd.len() {
                let mut after = g.clone();
                let before = g.combat.clone().unwrap();
                let p = g.preview(slot);
                after.apply(Action::Run(slot));
                match p {
                    Err(_) => {
                        // Refused: nothing moved.
                        assert_eq!(after.combat.as_ref().map(|c| c.cpu), Some(before.cpu));
                    }
                    Ok(p) => {
                        let left = after.combat.as_ref().map_or(0, |c| c.strength);
                        let won = after.combat.is_none();
                        if !won {
                            let c = after.combat.as_ref().unwrap();
                            assert_eq!(c.cpu, before.cpu - p.cpu, "cpu");
                            assert_eq!(left, before.strength.saturating_sub(p.hit()), "hit");
                        }
                        if let Outcome::Shield(n) = p.outcome
                            && !won
                        {
                            assert_eq!(after.combat.as_ref().unwrap().shield, before.shield + n);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn breakers_match_their_class() {
    let data = Data::builtin().unwrap();
    let g = fighting(3, 0);
    let class = data.ice[g.combat.as_ref().unwrap().ice].class;
    for (slot, s) in g.hdd.iter().enumerate().filter(|(_, s)| s.loaded) {
        if let (Ok(p), Effect::Break { power, vs }) = (g.preview(slot), s.spec.effect) {
            let want = match vs {
                None => (power, Match::Plain),
                Some(v) if v == class => (power * 2, Match::Strong),
                Some(_) => (power / 2, Match::Weak),
            };
            assert_eq!(
                p.outcome,
                Outcome::Break {
                    hit: want.0,
                    vs: want.1
                }
            );
        }
    }
    assert!(g.preview(99).is_err());
    let exploring = Game::new(data, 3);
    assert!(exploring.preview(0).is_err(), "nothing to run against");
}

#[test]
fn the_wireframe_comes_apart_with_strength() {
    assert_eq!(fight::edges_left(30, 17, 17), 30);
    assert_eq!(fight::edges_left(30, 0, 17), 0);
    assert_eq!(fight::edges_left(30, 1, 100), 1, "one edge while it stands");
    assert_eq!(fight::edges_left(30, 9, 17), 16);
    assert_eq!(fight::edges_left(0, 5, 5), 0);
    // Fortified past its start: still whole.
    assert_eq!(fight::edges_left(30, 25, 17), 30);
    let order = fight::break_order(30, 42);
    let mut sorted = order.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, (0..30).collect::<Vec<_>>(), "every edge once");
    assert_eq!(order, fight::break_order(30, 42), "same node, same order");
    assert_ne!(order, fight::break_order(30, 43));
    // Breaking more only adds edges to what's already broken.
    let a = fight::broken(30, 42, 12, 17);
    let b = fight::broken(30, 42, 5, 17);
    assert!(a.iter().all(|e| b.contains(e)) && b.len() > a.len());
    assert!(fight::threatens(Intent::Attack(3)));
    assert!(!fight::threatens(Intent::Fortify(3)));
}
