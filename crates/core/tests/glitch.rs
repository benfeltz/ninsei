//! Damage glitches: the damage level, the last hit, and width-keeping text corruption.

use ninsei_core::glitch::{self, Glitches};
use ninsei_core::hub::Save;
use ninsei_core::*;

#[test]
fn the_screen_is_clean_until_you_are_hurt() {
    assert_eq!(glitch::level(18, 18), 0.0);
    assert_eq!(glitch::level(13, 18), 0.0, "above 70%: clean");
    let mid = glitch::level(9, 18);
    let low = glitch::level(4, 18);
    assert!(mid > 0.0 && mid < 0.4, "half integrity: a little ({mid})");
    assert!(low > 0.5 && low < 1.0, "below 30%: a lot ({low})");
    assert_eq!(glitch::level(0, 18), 1.0);
    assert_eq!(glitch::level(5, 0), 0.0, "no max, no glitch");
    // Never goes down as integrity falls.
    let mut last = 0.0;
    for i in (0..=18).rev() {
        let l = glitch::level(i, 18);
        assert!(l >= last);
        last = l;
    }
}

#[test]
fn corruption_keeps_the_width() {
    let line = "IcePick breaks 3. Bulwark hits for 4.";
    let bad = glitch::corrupt(line, 42, 0.3);
    assert_eq!(bad.chars().count(), line.chars().count());
    assert_ne!(bad, line);
    assert_eq!(
        bad,
        glitch::corrupt(line, 42, 0.3),
        "same seed, same damage"
    );
    // Spaces stay where they were, so words keep their shape.
    for (a, b) in line.chars().zip(bad.chars()) {
        assert_eq!(a == ' ', b == ' ');
    }
    assert_eq!(glitch::corrupt(line, 42, 0.0), line);
    let some = glitch::corrupt(line, 7, 0.04);
    let changed = line
        .chars()
        .zip(some.chars())
        .filter(|(a, b)| a != b)
        .count();
    assert!(
        changed <= 5,
        "a low rate touches only a few characters ({changed})"
    );
}

#[test]
fn hits_are_remembered() {
    let d = Data::builtin().unwrap();
    for seed in 0..60 {
        let mut g = Game::new(d.clone(), seed);
        let to = g.reachable()[0];
        g.net.nodes[to].kind = NodeKind::Ice;
        g.net.nodes[to].cleared = false;
        g.net.nodes[to].ice = g.net.nodes[to].ice.or(Some(0));
        g.apply(Action::Move(to));
        assert!(g.history.last_hit.is_none());
        for _ in 0..6 {
            if g.phase != Phase::Combat {
                break;
            }
            let before = g.deck.integrity;
            g.apply(Action::EndTurn);
            if g.deck.integrity < before {
                let hit = g.history.last_hit.unwrap();
                assert_eq!(
                    (hit.tick, hit.amount, hit.soaked),
                    (g.history.tick, before - g.deck.integrity, false)
                );
                assert!(g.glitch() >= 0.0);
                return;
            }
        }
    }
    panic!("no seed took a hit");
}

#[test]
fn the_setting_is_saved() {
    assert_eq!(Save::load("(version: 2)").unwrap().glitches, Glitches::On);
    let s = Save {
        glitches: Glitches::Reduced,
        ..Save::default()
    };
    assert_eq!(
        Save::load(&s.to_text()).unwrap().glitches,
        Glitches::Reduced
    );
    assert_eq!(Glitches::On.next().next().next(), Glitches::On);
}
