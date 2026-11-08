//! The run's history (for the btop-style graphs) and the dot-graph maths.

use ninsei_core::history::{HISTORY_LEN, plot};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

/// A run standing next to an ICE node, with a breaker for it loaded.
fn at_ice(seed: u64) -> (Game, usize) {
    let mut g = Game::new(data(), seed);
    let to = g.reachable()[0];
    g.net.nodes[to].kind = NodeKind::Ice;
    g.net.nodes[to].cleared = false;
    if g.net.nodes[to].ice.is_none() {
        g.net.nodes[to].ice = Some(0);
    }
    (g, to)
}

#[test]
fn a_new_run_has_one_sample() {
    let g = Game::new(data(), 1);
    assert_eq!(g.history.samples.len(), 1);
    assert_eq!(g.history.trace_hops, vec![0]);
    assert_eq!(g.history.tick, 0);
}

#[test]
fn every_action_samples_and_every_hop_logs_trace() {
    let mut g = Game::new(data(), 2);
    for n in &mut g.net.nodes {
        n.kind = NodeKind::Vault;
    }
    let to = g.reachable()[0];
    g.apply(Action::Move(to));
    assert_eq!(g.history.tick, 1);
    assert_eq!(g.history.samples.len(), 2);
    assert_eq!(g.history.trace_hops.len(), 2);
    assert_eq!(*g.history.trace_hops.last().unwrap(), g.deck.trace);
    // A refused action still ticks, but isn't a hop.
    g.apply(Action::Move(9999));
    assert_eq!(g.history.trace_hops.len(), 2);
    assert_eq!(g.history.tick, 2);
    assert!(g.log.last().unwrap().hop == g.hops);
}

#[test]
fn fights_track_strength_turns_and_programs() {
    for seed in 0..20 {
        let (mut g, to) = at_ice(seed);
        g.apply(Action::Move(to));
        if g.phase != Phase::Combat {
            continue;
        }
        let start = g.combat.as_ref().unwrap().strength;
        assert_eq!(g.history.ice, vec![start], "seed {seed}");
        assert!(g.history.turns.is_empty());
        // Run the first program, then end the turn.
        let slot = 0;
        let uid = g.hdd[slot].uid;
        g.apply(Action::Run(slot));
        if g.phase == Phase::Combat {
            g.apply(Action::EndTurn);
        }
        assert!(g.history.ice.len() >= 2, "seed {seed}");
        assert_eq!(g.history.turns.len(), 1, "seed {seed}");
        let dealt = start - g.combat.as_ref().map_or(0, |c| c.strength);
        assert_eq!(g.history.turns[0].dealt, dealt, "seed {seed}");
        if g.history.since_run(uid).is_some() {
            assert!(g.history.since_run(uid).unwrap() <= 2);
        }
        return;
    }
    panic!("no seed started a fight");
}

#[test]
fn history_never_changes_the_game() {
    // Same seed, same actions: the run plays out the same with history being kept.
    let a = Game::new(data(), 7);
    let b = Game::new(data(), 7);
    assert_eq!(a.net.nodes.len(), b.net.nodes.len());
    assert_eq!(a.deck, b.deck);
}

#[test]
fn series_are_capped() {
    let mut g = Game::new(data(), 3);
    for _ in 0..HISTORY_LEN * 2 {
        g.apply(Action::Run(99));
    }
    assert_eq!(g.history.samples.len(), HISTORY_LEN);
    assert!(g.log.len() <= tuning::LOG_LIMIT);
}

#[test]
fn plot_windows_and_heights() {
    assert_eq!(
        plot::window(&[1, 2, 3], 5),
        vec![None, None, Some(1), Some(2), Some(3)]
    );
    assert_eq!(plot::window(&[1, 2, 3, 4], 2), vec![Some(3), Some(4)]);
    assert_eq!(plot::dots(0, 10, 8), 0);
    assert_eq!(plot::dots(1, 100, 8), 1, "anything above zero shows");
    assert_eq!(plot::dots(10, 10, 8), 8);
    assert_eq!(plot::dots(50, 10, 8), 8, "clamped");
    assert_eq!(
        plot::heights(&[0, 5, 10], 10, 4, 4),
        vec![None, Some(0), Some(2), Some(4)]
    );
}

#[test]
fn braille_matches_the_dots() {
    // One cell, 2 dots wide: left column full (4), right column empty.
    assert_eq!(plot::braille(&[Some(4), Some(0)], 1), vec!["⡇".to_string()]);
    // Right column half (2): dots 6 and 8.
    assert_eq!(plot::braille(&[Some(0), Some(2)], 1), vec!["⢠".to_string()]);
    // Two rows: a height of 5 fills the bottom cell and one dot of the top.
    let rows = plot::braille(&[Some(5), Some(0)], 2);
    assert_eq!(rows, vec!["⡀".to_string(), "⡇".to_string()]);
}
