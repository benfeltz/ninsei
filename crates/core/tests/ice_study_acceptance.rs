//! 141.9.4 acceptance: the sysops' simulator, from the outside. Careers of real runs (dialed
//! from the save, played by the probe's bots, booked by `Save::record`) with the sysops' time
//! at home: what ships did better against the script that beat the last version, fits its
//! machine, and is the same however the time was sliced. In a run with the sysops at work
//! every action, a fight always runs the version in force when it opened.

use std::sync::Arc;

use ninsei_core::game::{Start, duel};
use ninsei_core::hub::{Entry, IceVer, Save};
use ninsei_core::ice_study::{DeckLog, Study};
use ninsei_core::probe::{self, Bot, SAFE_PICK};
use ninsei_core::tuning::STUDY_PATIENCE;
use ninsei_core::*;

fn data() -> Arc<Data> {
    Arc::new(Data::builtin().unwrap())
}

/// A save with one tier-`tier` number and SAFE_PICK as its program.
fn deck(key: u64, tier: u32) -> Save {
    let mut s = Save {
        program: Some(SAFE_PICK.into()),
        ..Save::default()
    };
    s.ensure_key(key);
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

/// A version that shipped: the one it replaced, the new one, and what its sysops fought.
struct Shipped {
    was: String,
    now: IceVer,
    study: Study,
}

/// Plays `runs` SAFE_PICK runs from `save`, each followed by the sysops' time at home in
/// slices of `slice` steps (a patience's worth in all). Returns what shipped at home.
fn career(save: &mut Save, d: &Arc<Data>, runs: u64, slice: u32) -> Vec<Shipped> {
    let mut out = Vec::new();
    for r in 0..runs {
        save.program = Some(SAFE_PICK.into());
        let Ok(g) = save.dial(d, 0, 0xACE ^ r) else {
            continue;
        };
        let (g, _) = probe::play(g, Bot::SafePick);
        save.record(&g, &[], &d.rig).unwrap();
        let before = save.ice_versions.clone();
        let programs: Vec<(String, String)> = d
            .ice
            .iter()
            .map(|i| {
                (
                    i.id.clone(),
                    save.ice_program(&i.id, d).unwrap().1.to_string(),
                )
            })
            .collect();
        let mut left = STUDY_PATIENCE;
        while left > 0 {
            let n = slice.min(left);
            save.study(d, n, u32::MAX);
            left -= n;
        }
        for v in save.ice_versions.iter().filter(|v| !before.contains(v)) {
            let was = programs
                .iter()
                .find(|(id, _)| *id == v.id)
                .unwrap()
                .1
                .clone();
            let study = save
                .ice_study
                .iter()
                .find(|s| s.id == v.id)
                .unwrap()
                .clone();
            out.push(Shipped {
                was,
                now: v.clone(),
                study,
            });
        }
    }
    out
}

/// What `program` costs the deck `deck` from `starts`, over `seeds` duels each: integrity
/// lost as a percent of its most, plus trace gained.
fn cost(d: &Arc<Data>, id: &str, program: &str, deck: &DeckLog, starts: &[Start]) -> u64 {
    let ice = d.ice_by_id(id).unwrap();
    let kit = deck.kit(d);
    let most = u64::from(deck.integrity.max(1));
    let mut total = 0;
    for start in starts {
        for seed in 0..16u64 {
            let r = duel(d, ice, program, &kit, *start, 0xD0E1 ^ seed);
            total += u64::from(r.integrity_lost) * 100 / most + u64::from(r.trace_gained);
        }
    }
    total
}

#[test]
fn what_ships_does_better_against_the_script_that_beat_the_last_version() {
    let d = data();
    let mut shipped = Vec::new();
    for tier in [1, 2] {
        let mut s = deck(0x9_1494 + u64::from(tier), tier);
        shipped.extend(career(&mut s, &d, 40, STUDY_PATIENCE));
    }
    assert!(shipped.len() >= 6, "{} shipped", shipped.len());
    let (mut old, mut new, mut better) = (0, 0, 0);
    for x in &shipped {
        let s = &x.study;
        assert!(s.rest, "{}: they rest once it ships", s.id);
        assert_eq!(s.version.bumped(x.now.bump.unwrap()), x.now.version);
        let a = cost(&d, &s.id, &x.was, &s.deck, &s.starts);
        let b = cost(&d, &s.id, &x.now.program, &s.deck, &s.starts);
        old += a;
        new += b;
        better += usize::from(b > a);
    }
    // Judged on fresh luck, not the duels it was chosen on.
    assert!(new > old, "versions cost the deck {new} vs {old} before");
    assert!(
        better * 3 >= shipped.len() * 2,
        "{better} of {} did better on fresh duels",
        shipped.len()
    );
}

#[test]
fn the_lazy_programs_careers_ship_a_version_of_every_tier_one_kind() {
    // Follow-up: Rototurret couldn't hurt a deck behind two firewalls, so it never shipped.
    let d = data();
    let mut kinds: Vec<String> = Vec::new();
    for key in 0..4u64 {
        let mut s = deck(0xF011 + key, 1);
        for x in career(&mut s, &d, 40, STUDY_PATIENCE) {
            if !kinds.contains(&x.now.id) {
                kinds.push(x.now.id.clone());
            }
        }
    }
    for row in d.ice.iter().filter(|i| i.tier == 1) {
        assert!(
            kinds.contains(&row.id),
            "{} never shipped: {kinds:?}",
            row.id
        );
    }
}

#[test]
fn every_version_fits_its_machine_and_can_be_patched_again() {
    let d = data();
    for tier in [1, 2] {
        let mut s = deck(0x5150 + u64::from(tier), tier);
        for x in career(&mut s, &d, 40, STUDY_PATIENCE) {
            let v = &x.now;
            let row = d.ice.iter().find(|i| i.id == v.id).unwrap();
            d.check_ice_program(row, &v.program)
                .unwrap_or_else(|e| panic!("{} {}: {e}\n{}", v.id, v.version, v.program));
            let p = basic::parse_with(&v.program, basic::Vocab::Ice(&row.verbs));
            assert!(ice_version::blocks(&p).is_some(), "{}", v.program);
        }
    }
}

#[test]
fn the_same_save_ships_the_same_versions_however_the_time_was_sliced() {
    let d = data();
    let play = |key: u64, slice: u32| {
        let mut s = deck(key, 1);
        career(&mut s, &d, 30, slice);
        s.ice_versions
    };
    let a = play(7, STUDY_PATIENCE);
    assert!(!a.is_empty());
    assert_eq!(a, play(7, 1), "a draft at a time");
    assert_eq!(a, play(7, 5));
    assert_ne!(a, play(8, STUDY_PATIENCE), "another save, other versions");
}

/// Plays a run as a bot with no hands, the sysops given a step of time before every action:
/// first route, each fight played by its script, every encounter left.
fn busy_run(mut g: Game) -> Game {
    let mut opened: Option<(usize, ice_version::Version)> = None;
    for _ in 0..3000 {
        g.apply(Action::Study {
            steps: 1,
            drafts: u32::MAX,
        });
        // A fight under way keeps the version it opened on, whatever lands.
        if let Some(c) = &g.combat {
            if let Some((node, v)) = opened
                && node == c.node
            {
                assert_eq!(c.ice_version, v);
            }
            opened = Some((c.node, c.ice_version));
        }
        match g.phase {
            Phase::Exploring => {
                let Some(&to) = g.reachable().first() else {
                    break;
                };
                g.apply(Action::Move(to));
                if let (Some(c), Some(b)) = (&g.combat, g.bouts.last()) {
                    // A fight opens on the version in force, and its rez line says which.
                    assert_eq!(c.ice_version, g.ice_version(c.ice));
                    assert_eq!(b.version, c.ice_version);
                    let label = format!("!! {} rezzed", g.ice_label(c.ice));
                    let rez = g.log.iter().rev().find(|l| l.text.contains(" rezzed"));
                    assert!(rez.is_some_and(|l| l.text.starts_with(&label)), "{label}");
                    opened = Some((c.node, c.ice_version));
                }
            }
            Phase::Combat => {
                let now = g.combat.as_ref().map(|c| c.turn);
                g.apply(Action::Play);
                while g.pass_open() {
                    g.apply(Action::Finish);
                }
                if g.phase == Phase::Combat && g.combat.as_ref().map(|c| c.turn) == now {
                    g.apply(Action::EndTurn);
                }
            }
            Phase::Encounter => {
                let Some(choice) = g.leave_choice() else {
                    break;
                };
                g.apply(Action::Choose { choice, slot: None });
                if g.encounter.as_ref().is_some_and(|e| e.done) {
                    g.apply(Action::EndEncounter);
                }
            }
            Phase::JackedOut | Phase::Flatlined => break,
        }
    }
    g
}

#[test]
fn in_a_run_with_the_sysops_at_work_a_fight_runs_what_it_opened_on() {
    let d = data();
    let mut s = deck(0x9143, 1);
    career(&mut s, &d, 20, STUDY_PATIENCE);
    let mut landed = 0;
    for r in 0..12 {
        // Wake every kind the save has studied on its version in force, so something can land
        // mid-run.
        let now: Vec<_> = s
            .ice_study
            .iter()
            .map(|st| s.ice_program(&st.id, &d).map(|(v, _)| v))
            .collect();
        for (st, v) in s.ice_study.iter_mut().zip(now) {
            if v == Some(st.version) && !st.starts.is_empty() {
                st.rest = false;
                st.from = st.tried;
            }
        }
        s.program = Some(SAFE_PICK.into());
        let Ok(g) = s.dial(&d, 0, 0xB0B ^ r) else {
            continue;
        };
        let g = busy_run(g);
        landed += g.shipped.len();
        if !matches!(g.phase, Phase::JackedOut | Phase::Flatlined) {
            continue;
        }
        s.record(&g, &[], &d.rig).unwrap();
        // What landed in the run is in force at home.
        for (id, was, next) in &g.shipped {
            let (v, _) = s.ice_program(id, &d).unwrap();
            assert!(v >= was.bumped(next.bump.unwrap()), "{id}");
        }
    }
    assert!(landed > 0, "nothing landed mid-run");
}
