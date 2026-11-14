//! 141.11.2 acceptance: through a career of real runs (`Save::dial`, the probe's SAFE_PICK bot,
//! `Save::record`, the sysops' time at home), the boards sell what the scene has cracked and
//! nothing sooner, and a source bought from home lists in the fights after it.

use ninsei_core::bbs::Line;
use ninsei_core::data::Drop;
use ninsei_core::hub::{Entry, Save};
use ninsei_core::probe::{self, Bot, SAFE_PICK};
use ninsei_core::*;

fn deck() -> Save {
    let mut s = Save {
        program: Some(SAFE_PICK.into()),
        ..Save::default()
    };
    s.ensure_key(0x1411_2002);
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
    s.learn_board("gutter", "test");
    s
}

/// The Gutter's source files as a call from home lists them today: (ICE id, version).
fn on_the_gutter(s: &Save, d: &Data) -> Vec<(String, ice_version::Version)> {
    let call = s.home_call(d, &d.rig, d.board("gutter").unwrap());
    call.board()
        .areas
        .last()
        .unwrap()
        .files
        .iter()
        .filter_map(|f| match &f.item {
            Drop::Source { ice, version } => Some((ice.clone(), *version)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_career_s_boards_sell_only_what_the_scene_has_cracked() {
    let d = Data::builtin().unwrap();
    let arc = std::sync::Arc::new(d.clone());
    let mut s = deck();
    let (mut shipped, mut cracked) = (0, 0);
    for r in 0..60 {
        s.program = Some(SAFE_PICK.into());
        let Ok(g) = s.dial(&d, 0, 0x5EED + r) else {
            continue;
        };
        let (g, _) = probe::play(g, Bot::SafePick);
        s.record(&g, &[], &d.rig).unwrap();
        // A patience's worth of the sysops' time at home, a step at a time.
        for _ in 0..tuning::STUDY_PATIENCE {
            s.study(&arc, 1, u32::MAX);
            for (id, v) in on_the_gutter(&s, &d) {
                // Every file is 1.0.0 or a version in force the scene has finished cracking.
                if v != ice_version::Version::FIRST {
                    let ver = s.ice_versions.iter().find(|x| x.id == id).unwrap();
                    assert_eq!((ver.version, ver.crack), (v, 0), "{id} {v} sold uncracked");
                }
            }
        }
        shipped = shipped.max(s.ice_versions.len());
        cracked = cracked.max(
            on_the_gutter(&s, &d)
                .iter()
                .filter(|(_, v)| *v != ice_version::Version::FIRST)
                .count(),
        );
    }
    assert!(shipped > 0, "the career shipped a version");
    assert!(cracked > 0, "and the scene cracked one in time to sell it");
}

#[test]
fn a_source_bought_from_home_lists_in_the_fights_after_it() {
    let d = Data::builtin().unwrap();
    let mut s = deck();
    s.bank = 5_000;
    let gutter = d.board("gutter").unwrap();
    let call = s.home_call(&d, &d.rig, gutter);
    let a = call.board().areas.len() - 1;
    for f in 0..call.board().areas[a].files.len() {
        let shown = s.home_call(&d, &d.rig, gutter);
        s.home_buy_shown(&d, &d.rig, &shown, a, f).unwrap();
    }
    assert_eq!(s.ice_sources.len(), 6, "every tier-1 kind's 1.0.0");
    let mut listed = 0;
    for r in 0..10 {
        s.program = Some(SAFE_PICK.into());
        let Ok(g) = s.dial(&d, 0, 0xB0B + r) else {
            continue;
        };
        let (g, _) = probe::play(g, Bot::SafePick);
        // An ICE step that names its line: only possible with its source.
        listed += g
            .log
            .iter()
            .filter(|l| {
                d.ice.iter().filter(|i| i.tier == 1).any(|i| {
                    l.text
                        .strip_prefix(&format!("{} ", i.name))
                        .is_some_and(|t| t.starts_with(|c: char| c.is_ascii_digit()))
                })
            })
            .count();
        s.record(&g, &[], &d.rig).unwrap();
    }
    assert!(listed > 0, "a fight logged its ICE's lines");
}
