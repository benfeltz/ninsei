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
        ones.extend(work(&mut one, &d, KEY, 1, u32::MAX, in_force));
    }
    let mut batch = vec![s];
    let all = work(&mut batch, &d, KEY, 50, u32::MAX, in_force);
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
    assert_eq!((s.version, s.tried, s.from), (v, 5, 5), "a new search");
    assert_eq!(s.starts.len(), 1, "the old version's starts were its");
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
    let out = work(&mut v, &d, KEY, 5, u32::MAX, |_| {
        Some((newer, "10 TRACE 1".into()))
    });
    assert!(out.is_empty());
    assert!(v[0].rest);
    assert_eq!(v[0].tried, 0);
}

// The save (step 3).

use ninsei_core::hub::Save;

fn home(key: u64) -> Save {
    let d = data();
    let mut s = Save::default();
    s.ensure_key(key);
    s.ensure_phonebook(&d, 0);
    s.ensure_hardware(&d);
    s
}

/// One run from `save`: each of `fights` in turn on the next node, won (the ICE worn down to 1
/// and picked at, after a turn of `program`) or lost, then, if it's still going, a loss to
/// Hunter to end it. Booked.
fn run(save: &mut Save, d: &Data, program: &str, fights: &[(&str, bool)]) -> Game {
    save.program = Some(program.to_string());
    let mut g = save.jack_in(d, 9);
    let mut fights = fights.to_vec();
    if fights.last().is_none_or(|(_, won)| *won) {
        fights.push(("hunter", false));
    }
    for (id, win) in fights {
        let target = g.net.nodes[g.at].links[0];
        g.net.nodes[target].kind = NodeKind::Ice;
        g.net.nodes[target].ice = Some(ix(d, id));
        g.net.nodes[target].cleared = false;
        g.apply(Action::Move(target));
        assert_eq!(g.phase, Phase::Combat, "{id}");
        if win {
            let now = g.combat.as_ref().unwrap().turn;
            g.apply(Action::Play);
            while g.pass_open() {
                g.apply(Action::Finish);
            }
            if g.phase == Phase::Combat && g.combat.as_ref().unwrap().turn == now {
                g.apply(Action::EndTurn);
            }
            if g.phase == Phase::Combat {
                let c = g.combat.as_mut().unwrap();
                (c.strength, c.strength_max) = (1, 1);
                play_out(&mut g);
            }
            assert!(g.bouts.last().unwrap().broken, "{id}");
        } else {
            g.deck.integrity = 1;
            while g.phase == Phase::Combat {
                g.apply(Action::EndTurn);
            }
            assert_eq!(g.phase, Phase::Flatlined);
            break;
        }
    }
    save.record(&g, &[], &d.rig).unwrap();
    g
}

fn study<'a>(s: &'a Save, id: &str) -> Option<&'a Study> {
    s.ice_study.iter().find(|x| x.id == id)
}

#[test]
fn beating_a_kind_wakes_its_sysops_and_a_kill_never_does() {
    let d = data();
    let mut s = home(0x9144);
    run(&mut s, &d, SAFE_PICK, &[("enigma", true)]);
    let e = study(&s, "enigma").expect("beaten: studied");
    assert!(!e.rest);
    assert_eq!(e.starts.len(), 1);
    assert_eq!(e.deck.script, SAFE_PICK);
    assert!(study(&s, "hunter").is_none(), "it only killed the deck");
    // Beaten, then killed by it: it did its job, and they rest.
    run(
        &mut s,
        &d,
        SAFE_PICK,
        &[("enigma", true), ("enigma", false)],
    );
    let e = study(&s, "enigma").unwrap();
    assert!(e.rest);
    assert_eq!(e.starts.len(), 2);
}

#[test]
fn a_changed_script_starts_a_new_search() {
    let d = data();
    let mut s = home(0x9144);
    run(&mut s, &d, SAFE_PICK, &[("enigma", true)]);
    s.study(&Arc::new(d.clone()), 2, u32::MAX);
    let tried = study(&s, "enigma").unwrap().tried;
    run(
        &mut s,
        &d,
        "10 RUN icepick\n20 GOTO 10",
        &[("enigma", true)],
    );
    let e = study(&s, "enigma").unwrap();
    assert_eq!((e.from, e.starts.len()), (tried, 1));
    assert!(e.deck.script.contains("GOTO 10"));
}

#[test]
fn time_at_home_ships_a_version_the_next_run_carries() {
    let d = data();
    let arc = Arc::new(d.clone());
    let mut s = home(0x9144);
    run(
        &mut s,
        &d,
        SAFE_PICK,
        &[("enigma", true), ("rototurret", true), ("icewall", true)],
    );
    let mut shipped = 0;
    for _ in 0..STUDY_PATIENCE {
        shipped += s.study(&arc, 1, u32::MAX);
    }
    assert!(shipped > 0, "{:?}", s.ice_study);
    assert!(
        s.ice_study.iter().all(|x| x.rest),
        "each shipped once or gave up"
    );
    assert_eq!(s.study(&arc, 50, u32::MAX), 0, "resting until beaten again");
    let v = s.ice_versions[0].clone();
    assert_eq!(v.version, Version::FIRST.bumped(v.bump.unwrap()));
    let kit = s.kit(&d);
    assert!(
        kit.ice_programs
            .iter()
            .any(|(id, ver, ..)| *id == v.id && *ver == v.version)
    );
    let g = s.jack_in(&d, 4);
    assert_eq!(g.ice_version(ix(&d, &v.id)), v.version);
}

#[test]
fn the_same_save_ships_the_same_versions() {
    let d = data();
    let arc = Arc::new(d.clone());
    let play = |key: u64, batch: u32| {
        let mut s = home(key);
        for _ in 0..4 {
            run(
                &mut s,
                &d,
                SAFE_PICK,
                &[("enigma", true), ("rototurret", true), ("icewall", true)],
            );
            let mut left = STUDY_PATIENCE;
            while left > 0 {
                s.study(&arc, batch.min(left), u32::MAX);
                left = left.saturating_sub(batch);
            }
        }
        s.ice_versions
    };
    let a = play(5, 1);
    assert!(!a.is_empty());
    assert_eq!(a, play(5, 7), "however the time was sliced");
    assert_ne!(a, play(6, 1));
}

#[test]
fn a_save_from_the_ladder_loads() {
    let s = home(0x9144);
    let text = s.to_text().replace(
        "ice_study: [],",
        "ice_ladder: [(id: \"enigma\", wins: 3, rung: 1, program: None, tells: [])],",
    );
    assert!(text.contains("ice_ladder"));
    let back = Save::load(&text).unwrap();
    assert!(back.ice_study.is_empty());
}

// In the run (step 4).

/// Moves `g` into a fight against `id` on the next node over.
fn rez(g: &mut Game, id: &str) {
    let target = g.net.nodes[g.at].links[0];
    let ice = ix(&g.data, id);
    g.net.nodes[target].kind = NodeKind::Ice;
    g.net.nodes[target].ice = Some(ice);
    g.net.nodes[target].cleared = false;
    g.apply(Action::Move(target));
    assert_eq!(g.phase, Phase::Combat, "{id}");
}

/// A run on `seed` whose sysops are at work on Enigma 1.0.0 (a study that ships).
fn studied_run(seed: u64) -> Game {
    let (_, s) = study_of(&[1, 2, 3]);
    let mut g = Game::new(data(), seed);
    g.apply(Action::Program(SAFE_PICK.into()));
    g.ice_study = vec![s];
    g.study_key = KEY;
    g
}

#[test]
fn a_version_that_lands_mid_fight_is_the_next_fight_s_and_met_unknown() {
    let mut g = studied_run(7);
    rez(&mut g, "enigma");
    let e = ix(&g.data, "enigma");
    let prog = g.combat.as_ref().unwrap().ice_prog.clone();
    g.apply(Action::Study {
        steps: STUDY_PATIENCE,
        drafts: u32::MAX,
    });
    let (id, was, next) = g.shipped.first().cloned().expect("it ships");
    assert_eq!((id.as_str(), was), ("enigma", Version::FIRST));
    let new = was.bumped(next.bump.unwrap());
    // The fight under way runs what it opened on, and says so.
    let c = g.combat.as_ref().unwrap();
    assert_eq!((c.ice_version, &c.ice_prog), (Version::FIRST, &prog));
    assert_eq!(g.ice_label(e), "Enigma 1.0.0");
    assert_eq!(g.ice_version(e), new, "in force");
    play_out(&mut g);
    assert_eq!(g.phase, Phase::Exploring, "SAFE_PICK beats it on this seed");
    // The next opens on it, knowing none of its lines, whatever was seen of 1.0.0.
    assert!(!g.ice_lines_of(e, Version::FIRST).is_empty());
    rez(&mut g, "enigma");
    let c = g.combat.as_ref().unwrap();
    assert_eq!(c.ice_version, new);
    assert_eq!(c.ice_prog.text(), next.program);
    assert_eq!(g.ice_label(e), format!("Enigma {new}"));
    assert!(g.ice_seen_lines(e).is_empty());
    assert!(ice_listing::rows(&g).iter().all(|r| r.text.is_none()));
    assert!(
        g.log
            .iter()
            .any(|l| l.text.starts_with(&format!("!! Enigma {new} rezzed")))
    );
    // Home books each version with its own lines: 1.0.0 as it was known, then the new one.
    let old_lines = g.ice_lines_of(e, Version::FIRST).to_vec();
    play_out(&mut g);
    let new_lines = g.ice_lines_of(e, new).to_vec();
    g.phase = Phase::JackedOut;
    let mut save = home(KEY);
    let rig = g.data.rig.clone();
    save.record(&g, &[], &rig).unwrap();
    let seen = save.ice_seen.iter().find(|x| x.id == "enigma").unwrap();
    assert_eq!((seen.version, &seen.lines), (new, &new_lines));
    assert_eq!(seen.past.len(), 1);
    assert_eq!(
        (seen.past[0].version, &seen.past[0].lines),
        (Version::FIRST, &old_lines)
    );
    assert_eq!(seen.met, 2);
}

#[test]
fn the_sysops_time_never_touches_what_the_run_plays_from() {
    let mut a = studied_run(7);
    rez(&mut a, "rototurret");
    let mut b = a.clone();
    b.apply(Action::Study {
        steps: STUDY_PATIENCE,
        drafts: u32::MAX,
    });
    assert!(!b.shipped.is_empty(), "something landed");
    assert_eq!(b.rng.clone().next_u64(), a.rng.clone().next_u64());
    assert_eq!(b.log, a.log);
    assert_eq!(b.cues, a.cues);
    assert_eq!(format!("{:?}", b.history), format!("{:?}", a.history));
    // The same play from here plays the same, while no fight meets what landed.
    for g in [&mut a, &mut b] {
        play_out(g);
        for _ in 0..3 {
            if g.phase != Phase::Exploring {
                break;
            }
            let to = g.reachable().iter().find(|&&n| {
                g.net.nodes[n]
                    .ice
                    .is_none_or(|i| g.data.ice[i].id != "enigma")
            });
            let Some(&to) = to else { break };
            g.apply(Action::Move(to));
            play_out(g);
        }
    }
    assert_eq!(b.rng.clone().next_u64(), a.rng.clone().next_u64());
    assert_eq!(
        (b.at, b.deck.integrity, b.deck.trace),
        (a.at, a.deck.integrity, a.deck.trace)
    );
}

#[test]
fn beating_a_kind_in_a_run_wakes_its_sysops_there() {
    let mut g = Game::new(data(), 2);
    g.apply(Action::Program(SAFE_PICK.into()));
    rez(&mut g, "rototurret");
    let c = g.combat.as_mut().unwrap();
    (c.strength, c.strength_max) = (1, 1);
    let start = g.bouts.last().unwrap().start;
    play_out(&mut g);
    assert!(g.bouts.last().unwrap().broken);
    let s = g
        .ice_study
        .iter()
        .find(|s| s.id == "rototurret")
        .expect("woken");
    assert!(!s.rest);
    assert_eq!(s.starts, vec![start]);
    assert_eq!(s.deck.script, SAFE_PICK);
}

#[test]
fn the_sysops_come_home_with_the_run_on_an_exit_or_a_flatline() {
    let d = data();
    for flatline in [false, true] {
        let mut s = home(KEY);
        s.key = KEY;
        let (_, study) = study_of(&[1, 2, 3]);
        s.ice_study = vec![study];
        let mut g = s.jack_in(&d, 5);
        assert_eq!((g.ice_study.len(), g.study_key), (1, KEY));
        g.apply(Action::Study {
            steps: STUDY_PATIENCE,
            drafts: u32::MAX,
        });
        assert_eq!(g.shipped.len(), 1);
        let shipped = g.ice_versions.clone();
        if flatline {
            rez(&mut g, "enigma");
            g.deck.integrity = 1;
            while g.phase == Phase::Combat {
                g.apply(Action::EndTurn);
            }
            assert_eq!(g.phase, Phase::Flatlined);
        } else {
            g.phase = Phase::JackedOut;
        }
        // Over: the run's clock has stopped.
        let before = g.ice_study.clone();
        g.apply(Action::Study {
            steps: 10,
            drafts: u32::MAX,
        });
        assert_eq!(g.ice_study, before);
        s.record(&g, &[], &d.rig).unwrap();
        let (v, p) = s.ice_program("enigma", &d).unwrap();
        assert_eq!(v, shipped[0].1);
        assert_eq!(p, shipped[0].2.text());
        assert_eq!(s.ice_study.len(), 1);
        assert!(s.ice_study[0].rest);
        if flatline {
            // Killed by its new version, met unknown, booked as that version's.
            let e = s.ice_seen.iter().find(|x| x.id == "enigma").unwrap();
            assert_eq!((e.version, e.lost), (v, 1));
        }
    }
}

// Review fixes.

#[test]
fn a_study_from_a_broken_save_never_panics() {
    let (d, mut s) = study_of(&[1]);
    s.deck.integrity = 0;
    s.from = s.tried + 7;
    for st in &mut s.starts {
        st.integrity = 0;
    }
    let mut v = vec![s];
    let program = row_program(&d);
    let _ = work(&mut v, &d, KEY, STUDY_PATIENCE, u32::MAX, |_| {
        Some((Version::FIRST, program.clone()))
    });
    assert!(v[0].tried > 0);
}

#[test]
fn a_fight_the_sysops_stage_wakes_nobody() {
    let mut g = Game::new(data(), 2);
    g.watched = false;
    g.apply(Action::Program(SAFE_PICK.into()));
    rez(&mut g, "rototurret");
    let c = g.combat.as_mut().unwrap();
    (c.strength, c.strength_max) = (1, 1);
    play_out(&mut g);
    assert!(g.bouts.last().unwrap().broken);
    assert!(g.ice_study.is_empty());
}

#[test]
fn the_sysops_study_the_script_as_the_fight_opened_not_what_it_garbled() {
    let mut g = Game::new(data(), 2);
    g.apply(Action::Program(SAFE_PICK.into()));
    rez(&mut g, "rototurret");
    // A SEGFAULT during the fight: the line stays bad until it's retyped.
    g.program.garble(50, "#$%&@!?*");
    let c = g.combat.as_mut().unwrap();
    (c.strength, c.strength_max) = (1, 1);
    play_out(&mut g);
    assert!(g.bouts.last().unwrap().broken);
    assert_ne!(g.program.text(), SAFE_PICK);
    let s = g.ice_study.iter().find(|s| s.id == "rototurret").unwrap();
    assert_eq!(s.deck.script, SAFE_PICK);
}

/// Enigma's ICE DB entry for version 1.0.1 running `program`, its lines 10 and 20 seen.
fn seen_patch(program: &str) -> hub::IceSeen {
    hub::IceSeen {
        id: "enigma".into(),
        met: 1,
        lo: 12,
        hi: 12,
        lines: vec![10, 20],
        version: Version::FIRST.bumped(Bump::Patch),
        bump: Some(Bump::Patch),
        program: program.into(),
        won: 1,
        ..Default::default()
    }
}

#[test]
fn a_saved_version_its_row_no_longer_takes_still_books_the_fights_that_fall_back() {
    let d = data();
    let mut s = home(0x9144);
    // 1.0.1 was saved, then the row changed under it: tier 1 may not SEGFAULT.
    let bad = "10 SEGFAULT\n20 TRACE 4";
    s.ice_seen.push(seen_patch(bad));
    s.ice_versions.push(hub::IceVer {
        id: "enigma".into(),
        version: Version::FIRST.bumped(Bump::Patch),
        program: bad.into(),
        bump: Some(Bump::Patch),
    });
    assert_eq!(s.ice_program("enigma", &d).unwrap().0, Version::FIRST);
    run(&mut s, &d, SAFE_PICK, &[("enigma", true)]);
    let e = s.ice_seen.iter().find(|x| x.id == "enigma").unwrap();
    assert_eq!((e.version, e.won), (Version::FIRST, 1), "{e:?}");
    assert_eq!(e.past.len(), 1, "1.0.1 kept as it was known");
    assert_eq!(e.past[0].lines, [10, 20]);
}

#[test]
fn a_version_number_met_before_with_another_program_is_met_unknown() {
    let d = data();
    let mut s = home(0x9144);
    s.tools.push("icedb".into());
    s.ice_seen.push(seen_patch("10 TRACE 4\n20 ATTACK 3"));
    let now = "10 ATTACK 4\n20 TRACE 4";
    s.ice_versions.push(hub::IceVer {
        id: "enigma".into(),
        version: Version::FIRST.bumped(Bump::Patch),
        program: now.into(),
        bump: Some(Bump::Patch),
    });
    assert!(
        s.kit(&d).ice_known.iter().all(|(id, _)| id != "enigma"),
        "those lines were another program's"
    );
    run(&mut s, &d, SAFE_PICK, &[("enigma", true)]);
    let e = s.ice_seen.iter().find(|x| x.id == "enigma").unwrap();
    assert_eq!(e.program, now);
    assert_eq!(e.past.len(), 1);
    assert_eq!(e.past[0].program, "10 TRACE 4\n20 ATTACK 3");
    // The same number and program is the same version: its lines come back in.
    let mut known = e.clone();
    known.lines = vec![10];
    s.ice_seen = vec![known];
    assert!(
        s.kit(&d)
            .ice_known
            .iter()
            .any(|(id, l)| id == "enigma" && l == &[10])
    );
}

#[test]
fn a_run_shares_the_content_it_was_dialed_with() {
    let arc = Arc::new(data());
    let mut s = home(0x9144);
    let g = s.jack_in(&arc, 1);
    assert!(Arc::ptr_eq(&g.data, &arc));
    let entry = s.phonebook[0].id;
    let g = s.dial(&arc, entry, 2).unwrap();
    assert!(Arc::ptr_eq(&g.data, &arc));
    // A plain `Data` is copied, as before.
    let d = data();
    let g = s.jack_in(&d, 3);
    assert_eq!(Arc::strong_count(&g.data), 1);
}

#[test]
fn drafts_spread_over_frames_ship_what_working_them_at_once_does() {
    let (d, s) = study_of(&[1, 2, 3]);
    let mut other = s.clone();
    other.id = "tollbooth".into();
    let programs: std::collections::HashMap<String, String> = ["enigma", "tollbooth"]
        .iter()
        .map(|id| (id.to_string(), d.ice[ix(&d, id)].program.clone()))
        .collect();
    let in_force = |id: &str| Some((Version::FIRST, programs[id].clone()));
    let mut at_once = vec![s.clone(), other.clone()];
    let all = work(&mut at_once, &d, KEY, STUDY_PATIENCE, u32::MAX, in_force);
    assert!(at_once.iter().all(|s| s.owed == 0));
    // A frame's budget at a time: two drafts, whoever is owed them.
    let mut framed = vec![s, other];
    let mut out = work(&mut framed, &d, KEY, STUDY_PATIENCE, 2, in_force);
    assert_eq!(framed.iter().map(|s| s.tried).sum::<u32>(), 2);
    while ice_study::owed(&framed) {
        out.extend(work(&mut framed, &d, KEY, 0, 2, in_force));
    }
    let key = |v: &Vec<(String, Version, ice_version::Next)>| {
        let mut v = v.clone();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    };
    assert_eq!(key(&out), key(&all));
    assert_eq!(framed, at_once);
}
