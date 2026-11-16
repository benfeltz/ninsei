//! 141.11.1 acceptance: through a career of real runs (`Save::dial`, the probe's SAFE_PICK bot,
//! `Save::record`, the sysops' time at home), fighting reveals none of an ICE's program, in the
//! log or the ICE DB; a source owned lists its version's program and no other version's.

use ninsei_core::hub::{Entry, Save};
use ninsei_core::probe::{self, Bot, SAFE_PICK};
use ninsei_core::*;

fn deck() -> Save {
    let mut s = Save {
        program: Some(SAFE_PICK.into()),
        ..Save::default()
    };
    s.ensure_key(0x1411);
    s.tools.push("icedb".into());
    s.phonebook.push(Entry {
        id: 0,
        name: "probe".into(),
        business: "test".into(),
        number: "555-0000".into(),
        tier: 1,
        traits: Vec::new(),
        heat: 0,
        runs: 0,
        last: Vec::new(),
    });
    s
}

/// Whether a source came from a beaten net's AI (138.3), not a board: what fighting ICE gives
/// is the subject here, and a beaten AI gives up its net's sources.
fn from_ai(d: &Data, from: &str) -> bool {
    d.ai.models.iter().any(|m| m.id == from)
}

/// The log lines in which an ICE's step names a line of its program: `<name> <number> ...`,
/// but for the kinds whose source a beaten AI gave up this run.
fn lined<'a>(g: &'a Game, d: &Data) -> Vec<&'a str> {
    let given: Vec<&str> = g
        .sources_got
        .iter()
        .filter(|s| from_ai(d, &s.from))
        .filter_map(|s| d.ice.iter().find(|i| i.id == s.id).map(|i| i.name.as_str()))
        .collect();
    g.log
        .iter()
        .map(|l| l.text.as_str())
        .filter(|t| !given.iter().any(|n| t.starts_with(&format!("{n} "))))
        .filter(|t| {
            d.ice.iter().any(|i| {
                t.strip_prefix(&format!("{} ", i.name))
                    .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
            })
        })
        .collect()
}

/// `runs` runs from `s`, the sysops working a patience's worth at home after each; each run's
/// game handed to `check` before it's booked.
fn career(s: &mut Save, d: &Data, runs: u64, seed: u64, mut check: impl FnMut(&Save, &Game)) {
    let arc = std::sync::Arc::new(d.clone());
    for r in 0..runs {
        s.program = Some(SAFE_PICK.into());
        let Ok(g) = s.dial(d, 0, seed + r) else {
            continue;
        };
        let (g, _) = probe::play(g, Bot::SafePick);
        check(s, &g);
        s.record(&g, &[], &d.rig).unwrap();
        // What a beaten AI gave up isn't what fighting ICE gives: the career keeps only the rest.
        s.ice_sources.retain(|x| !from_ai(d, &x.from));
        s.study(&arc, tuning::STUDY_PATIENCE, u32::MAX);
    }
}

#[test]
fn a_career_without_sources_shows_none_of_any_program() {
    let d = Data::builtin().unwrap();
    let mut s = deck();
    let mut fights = 0;
    career(&mut s, &d, 30, 0x700, |_, g| {
        fights += g.bouts.len();
        let l = lined(g, &d);
        assert!(l.is_empty(), "a step named a line: {l:?}");
    });
    assert!(fights > 10, "the career fought: {fights}");
    assert!(s.ice_sources.is_empty(), "fighting got it no source");
    // Every version met, of every kind: none of its statements is on the ICE DB's pages.
    for e in icedb::entries(&s, &d) {
        let Some(seen) = e.seen else { continue };
        let page = e.lines(&s, &d).join("\n");
        let programs = std::iter::once(seen.program.as_str())
            .chain(seen.past.iter().map(|p| p.program.as_str()));
        for program in programs {
            let text = if program.is_empty() {
                e.def.program.as_str()
            } else {
                program
            };
            for l in &basic::parse_with(text, basic::Vocab::Ice(&e.def.verbs)).lines {
                assert!(
                    !page.contains(&l.text),
                    "{}: {:?} on\n{page}",
                    e.def.id,
                    l.text
                );
            }
        }
    }
}

#[test]
fn owning_1_0_0_lists_1_0_0_and_never_a_version_after_it() {
    let d = Data::builtin().unwrap();
    let mut s = deck();
    for i in &d.ice {
        assert!(s.learn_source(&d, &i.id, ice_version::Version::FIRST));
    }
    let (mut listed, mut blind) = (0, 0);
    career(&mut s, &d, 40, 0x900, |_, g| {
        // Each rez names its version; the log of a fight shows its lines only on 1.0.0.
        for b in &g.bouts {
            let name = &d.ice[b.ice].name;
            let steps: Vec<&str> = lined(g, &d)
                .into_iter()
                .filter(|t| t.starts_with(&format!("{name} ")))
                .collect();
            if b.version == ice_version::Version::FIRST {
                listed += usize::from(!steps.is_empty());
            } else if !g
                .bouts
                .iter()
                .any(|x| x.ice == b.ice && x.version == ice_version::Version::FIRST)
            {
                assert!(steps.is_empty(), "{name} {}: {steps:?}", b.version);
                blind += 1;
            }
        }
    });
    assert!(listed > 0, "a 1.0.0 fight listed its lines");
    assert!(
        blind > 0,
        "a version past 1.0.0 was met, and showed nothing"
    );
}
