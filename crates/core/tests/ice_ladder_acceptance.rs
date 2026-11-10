//! 141.9.2 acceptance: ICE evolve, from the outside. Careers of real runs (dialed from the
//! save, played by the probe's bots, booked by `Save::record`, with a patience's worth of the
//! sysops' time at home after each, 141.9.4) ship versions that fit their machines, the same
//! save ships the same ones, nothing changes inside a run, and the kind that ends a run isn't
//! patched by it.

use ninsei_core::hub::{Entry, Save};
use ninsei_core::probe::{self, Bot, SAFE_PICK};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
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

/// What a run checked along the way.
struct Seen {
    games: u32,
    shipped: u32,
}

/// Plays `runs` runs of SAFE_PICK from `save`, checking every run's end against the rules.
fn career(save: &mut Save, d: &Data, runs: u64) -> Seen {
    let mut seen = Seen {
        games: 0,
        shipped: 0,
    };
    for r in 0..runs {
        save.program = Some(SAFE_PICK.into());
        let before = save.ice_versions.clone();
        let Ok(g) = save.dial(d, 0, 0xACE ^ r) else {
            continue;
        };
        // Fixed at jack-in: the run's versions are the save's.
        for (ice, v, ..) in &g.ice_versions {
            let id = &d.ice[*ice].id;
            assert_eq!(save.ice_program(id, d).unwrap().0, *v);
        }
        let (g, _) = probe::play(g, Bot::SafePick);
        let during: Vec<String> = g
            .bouts
            .iter()
            .map(|b| g.ice_version(b.ice).to_string())
            .collect();
        save.record(&g, &[], &d.rig).unwrap();
        save.study(
            &std::sync::Arc::new(d.clone()),
            tuning::STUDY_PATIENCE,
            u32::MAX,
        );
        seen.games += 1;
        // Nothing changed inside the run: every fight of a kind ran the version it jacked in on.
        for (b, v) in g.bouts.iter().zip(&during) {
            let id = &d.ice[b.ice].id;
            let at_start = before
                .iter()
                .find(|x| &x.id == id)
                .map_or("1.0.0".to_string(), |x| x.version.to_string());
            assert_eq!(v, &at_start, "{id}");
        }
        // The kind that ended the run wasn't patched by it.
        if let Some(k) = g.killed_by() {
            let id = &d.ice[k].id;
            assert_eq!(
                save.ice_versions.iter().find(|v| &v.id == id),
                before.iter().find(|v| &v.id == id),
                "{id} killed the deck and evolved"
            );
        }
        for v in save.ice_versions.iter().filter(|v| !before.contains(v)) {
            seen.shipped += 1;
            let row = d.ice.iter().find(|i| i.id == v.id).unwrap();
            // It fits its machine and stays a program the sysops can patch again.
            d.check_ice_program(row, &v.program)
                .unwrap_or_else(|e| panic!("{} {}: {e}\n{}", v.id, v.version, v.program));
            let p = basic::parse_with(&v.program, basic::Vocab::Ice(&row.verbs));
            assert!(ice_version::blocks(&p).is_some(), "{}", v.program);
            let was = before
                .iter()
                .find(|x| x.id == v.id)
                .map_or(ice_version::Version::default(), |x| x.version);
            assert!(v.version > was, "{} {was} -> {}", v.id, v.version);
            assert_eq!(v.version, was.bumped(v.bump.unwrap()));
        }
    }
    seen
}

#[test]
fn a_career_of_real_runs_ships_versions_that_fit_their_machines() {
    let d = data();
    for tier in [1, 2] {
        let mut s = deck(0x9142 + u64::from(tier), tier);
        let seen = career(&mut s, &d, 60);
        assert!(seen.games > 50, "tier {tier}");
        assert!(seen.shipped > 0, "tier {tier}: nothing shipped");
    }
}

#[test]
fn the_same_save_ships_the_same_versions_and_another_save_others() {
    let d = data();
    let play = |key| {
        let mut s = deck(key, 1);
        career(&mut s, &d, 40);
        (s.ice_versions, s.ice_study)
    };
    let a = play(7);
    assert!(!a.0.is_empty());
    assert_eq!(a, play(7));
    assert_ne!(a.0, play(8).0);
}

#[test]
fn a_new_version_is_met_unknown_and_the_last_one_is_kept() {
    let d = data();
    let mut s = deck(0x51, 1);
    s.tools.push("icedb".into());
    career(&mut s, &d, 80);
    // Every kind past 1.0.0 that a run has met since: its seen lines are that version's, and
    // the versions it was met at before are kept, oldest first.
    for seen in &s.ice_seen {
        let (now, _) = s.ice_program(&seen.id, &d).unwrap();
        assert!(seen.version <= now, "{}", seen.id);
        let mut last = None;
        for p in &seen.past {
            assert!(p.version < seen.version, "{}", seen.id);
            assert!(last.is_none_or(|l| l < p.version), "{}", seen.id);
            last = Some(p.version);
        }
        let kit = s.kit(&d);
        let known = kit.ice_known.iter().any(|(id, _)| id == &seen.id);
        assert_eq!(
            known,
            seen.version == now && !seen.lines.is_empty(),
            "{}",
            seen.id
        );
    }
}
