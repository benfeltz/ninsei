//! 141.9.4: the sysops' simulator. A duel staged in the real game, the study that drafts
//! versions and fights them against the deck's script, and how it runs in a run and at home.

use std::sync::Arc;

use ninsei_core::game::{Duel, Start, duel};
use ninsei_core::probe::SAFE_PICK;
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
