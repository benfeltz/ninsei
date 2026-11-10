//! 141.9.4: the sysops' simulator. A duel staged in the real game, the study that drafts
//! versions and fights them against the deck's script, and how it runs in a run and at home.

use std::sync::Arc;

use ninsei_core::game::{Duel, Start, duel};
use ninsei_core::ice_study::{DeckLog, Study, size, work};
use ninsei_core::ice_version::{Bump, Version};
use ninsei_core::probe::SAFE_PICK;
use ninsei_core::tuning::{STUDY_PATIENCE, STUDY_STARTS};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

fn ix(d: &Data, id: &str) -> usize {
    d.ice.iter().position(|i| i.id == id).unwrap()
}

/// A run with the classic four (breakers too) on `seed`, in a fight against `id` on the node
/// next to the entry, running `program`.
fn fight(seed: u64, id: &str, program: &str) -> Game {
    let mut g = Game::new(data(), seed);
    g.apply(Action::Program(program.into()));
    let target = g.net.nodes[g.at].links[0];
    let ice = ix(&g.data, id);
    g.net.nodes[target].kind = NodeKind::Ice;
    g.net.nodes[target].ice = Some(ice);
    g.net.nodes[target].cleared = false;
    g.apply(Action::Move(target));
    assert_eq!(g.phase, Phase::Combat);
    g
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

fn enigma_duel(g: &Game, seed: u64) -> Duel {
    let e = ix(&g.data, "enigma");
    let program = g.data.ice[e].program.clone();
    let start = g.bouts.last().unwrap().start;
    duel(&g.data, e, &program, &g.deck_kit(), start, seed)
}

#[test]
fn a_fight_books_how_it_opened() {
    let g = fight(7, "enigma", SAFE_PICK);
    let b = g.bouts.last().unwrap();
    let c = g.combat.as_ref().unwrap();
    assert_eq!(
        b.start,
        Start {
            integrity: g.deck.integrity,
            trace: g.deck.trace,
            strength: c.strength,
        }
    );
}

#[test]
fn the_same_duel_ends_the_same_way() {
    let g = fight(7, "enigma", SAFE_PICK);
    assert_eq!(enigma_duel(&g, 11), enigma_duel(&g, 11));
}

#[test]
fn a_duel_leaves_the_run_it_was_staged_from_alone() {
    let g = fight(7, "enigma", SAFE_PICK);
    let before = g.clone();
    let _ = enigma_duel(&g, 11);
    assert_eq!(g.log.len(), before.log.len());
    assert_eq!(g.cues, before.cues);
    assert_eq!(g.bouts, before.bouts);
    assert_eq!(g.rng.clone().next_u64(), before.rng.clone().next_u64());
    // It shares the run's content, never copies it.
    assert_eq!(Arc::strong_count(&g.data), 2);
}

#[test]
fn a_script_that_won_a_fight_wins_its_duel_from_that_start() {
    let mut won = 0;
    for seed in 1..=6 {
        let mut g = fight(seed, "enigma", SAFE_PICK);
        let kit = g.deck_kit();
        let start = g.bouts.last().unwrap().start;
        play_out(&mut g);
        if !g.bouts.last().unwrap().broken {
            continue;
        }
        won += 1;
        let e = ix(&g.data, "enigma");
        let program = g.data.ice[e].program.clone();
        let d = duel(&g.data, e, &program, &kit, start, seed);
        assert!(d.broken, "seed {seed}: {d:?}");
        assert!(d.turns >= 1 && !d.tells.is_empty(), "{d:?}");
    }
    assert!(won >= 3, "SAFE_PICK beats Enigma in most of these: {won}");
}

#[test]
fn a_duel_against_a_harder_program_costs_the_deck_more() {
    let g = fight(7, "enigma", SAFE_PICK);
    let e = ix(&g.data, "enigma");
    let start = g.bouts.last().unwrap().start;
    let kit = g.deck_kit();
    let soft = duel(&g.data, e, "10 TRACE 1", &kit, start, 3);
    let row = g.data.ice[e].program.clone();
    let real = duel(&g.data, e, &row, &kit, start, 3);
    assert!(
        real.integrity_lost + real.trace_gained > soft.integrity_lost + soft.trace_gained,
        "{real:?} vs {soft:?}"
    );
}

// The study (step 2).

const KEY: u64 = 0x9_1494;

/// A study of Enigma 1.0.0 woken by SAFE_PICK beating it from the starts of `seeds`' fights.
fn study_of(seeds: &[u64]) -> (Arc<Data>, Study) {
    let mut s = Study::new("enigma");
    let mut data = None;
    for &seed in seeds {
        let g = fight(seed, "enigma", SAFE_PICK);
        let deck = DeckLog::of(&g.deck_kit(), &g.data);
        s.beaten(Version::FIRST, deck, g.bouts.last().unwrap().start);
        data = Some(Arc::clone(&g.data));
    }
    (data.unwrap(), s)
}

fn row_program(d: &Data) -> String {
    d.ice[ix(d, "enigma")].program.clone()
}

/// Steps `s` until it rests: (the draft that shipped, if one did; steps taken).
fn until_rest(s: &mut Study, d: &Arc<Data>) -> (Option<ninsei_core::ice_version::Next>, u32) {
    let program = row_program(d);
    for n in 1..=STUDY_PATIENCE {
        if let Some(next) = s.step(d, &program, KEY) {
            return (Some(next), n);
        }
        if s.rest {
            return (None, n);
        }
    }
    panic!("never rested");
}

#[test]
fn draft_sizes_go_patch_minor_major() {
    let third = STUDY_PATIENCE / 3;
    assert_eq!(size(0), Bump::Patch);
    assert_eq!(size(third - 1), Bump::Patch);
    assert_eq!(size(third), Bump::Minor);
    assert_eq!(size(2 * third), Bump::Major);
    assert_eq!(size(STUDY_PATIENCE * 5), Bump::Major);
}

#[test]
fn beating_a_kind_wakes_its_study_with_that_start() {
    let (_, s) = study_of(&[1, 2]);
    assert!(!s.rest);
    assert_eq!(s.starts.len(), 2, "the same script adds its start");
    assert_eq!(s.version, Version::FIRST);
    assert!(s.deck.script.contains("RUN icepick"));
}

#[test]
fn it_ships_the_first_draft_that_clears_the_bar_and_rests() {
    let (d, mut s) = study_of(&[1, 2, 3]);
    let fresh = s.clone();
    let (shipped, n) = until_rest(&mut s, &d);
    let next = shipped.expect("Enigma finds a counter to SAFE_PICK");
    assert!(s.rest);
    assert_eq!(s.tried, n);
    assert_ne!(next.program, row_program(&d));
    // A resting study does nothing.
    assert_eq!(s.step(&d, &row_program(&d), KEY), None);
    assert_eq!(s.tried, n);
    // Every draft before it fell short: it is the first, not the best.
    let mut again = fresh;
    for _ in 1..n {
        assert_eq!(again.step(&d, &row_program(&d), KEY), None);
    }
    assert_eq!(again.step(&d, &row_program(&d), KEY), Some(next));
}

#[test]
fn the_same_study_ships_the_same_version_however_its_steps_are_batched() {
    let (d, s) = study_of(&[1, 2, 3]);
    let program = row_program(&d);
    let in_force = |_: &str| Some((Version::FIRST, program.clone()));
    let mut one = vec![s.clone()];
    let mut ones = Vec::new();
    for _ in 0..STUDY_PATIENCE {
        ones.extend(work(&mut one, &d, KEY, 1, in_force));
    }
    let mut batch = vec![s];
    let all = work(&mut batch, &d, KEY, 50, in_force);
    assert_eq!(ones, all);
    assert_eq!(one, batch);
    assert_eq!(all.len(), 1, "one version, then rest");
}

#[test]
fn a_deck_the_ice_never_touches_tires_them_out() {
    // Strength 1: the first breaker derezzes it before it acts, whatever it runs.
    let g = fight(1, "enigma", SAFE_PICK);
    let mut start = g.bouts.last().unwrap().start;
    start.strength = 1;
    let mut s = Study::new("enigma");
    s.beaten(Version::FIRST, DeckLog::of(&g.deck_kit(), &g.data), start);
    let d = Arc::clone(&g.data);
    let (shipped, n) = until_rest(&mut s, &d);
    assert_eq!(shipped, None);
    assert_eq!(n, STUDY_PATIENCE, "patience, then rest without shipping");
    // Beaten again by the same script: a fresh search, at patches, from where it left off.
    s.beaten(Version::FIRST, s.deck.clone(), start);
    assert!(!s.rest);
    assert_eq!((s.from, s.tried), (STUDY_PATIENCE, STUDY_PATIENCE));
}

#[test]
fn a_new_script_starts_a_new_search_and_a_new_version_a_new_study() {
    let (_, mut s) = study_of(&[1, 2]);
    s.tried = 5;
    let mut deck = s.deck.clone();
    deck.script = "10 RUN icepick\n20 GOTO 10".into();
    let start = s.starts[0];
    s.beaten(Version::FIRST, deck.clone(), start);
    assert_eq!(
        (s.from, s.starts.len()),
        (5, 1),
        "the old starts were another play's"
    );
    let v = Version::FIRST.bumped(Bump::Patch);
    s.beaten(v, deck, start);
    assert_eq!((s.version, s.tried, s.from), (v, 0, 0));
    for _ in 0..STUDY_STARTS + 3 {
        s.beaten(v, s.deck.clone(), start);
    }
    assert_eq!(s.starts.len(), STUDY_STARTS, "the latest are kept");
}

#[test]
fn a_study_of_a_version_no_longer_in_force_rests() {
    let (d, s) = study_of(&[1]);
    let mut v = vec![s];
    let newer = Version::FIRST.bumped(Bump::Patch);
    let out = work(&mut v, &d, KEY, 5, |_| Some((newer, "10 TRACE 1".into())));
    assert!(out.is_empty());
    assert!(v[0].rest);
    assert_eq!(v[0].tried, 0);
}
