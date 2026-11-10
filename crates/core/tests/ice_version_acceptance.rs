//! 141.9.1 acceptance: the ICE's machine, the grammar, the generator and versions in the save,
//! checked from the outside, the way 141.9.2's ladder will use them.

use ninsei_core::hub::{IceVer, Save};
use ninsei_core::ice_version::{Bump, Version, next_version, seed};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

/// A save that can jack in.
fn deck(d: &Data) -> Save {
    let mut s = Save::default();
    s.ensure_key(0x9141);
    s.ensure_phonebook(d, 0);
    s.ensure_hardware(d);
    s
}

/// `ice.ron` with ICE `id`'s program replaced by `program`.
fn with_program(id: &str, program: &str) -> Result<Data, String> {
    let ice = include_str!("../../../data/ice.ron");
    let at = ice.find(&format!("(id: \"{id}\"")).unwrap();
    let start = at + ice[at..].find("program: \"").unwrap() + "program: \"".len();
    let end = start + ice[start..].find('"').unwrap();
    let ice = format!("{}{program}{}", &ice[..start], &ice[end..]);
    Data::parse(Sources {
        ice: &ice,
        ..Sources::builtin()
    })
}

/// A fight from `save`'s jack-in against ICE `id`, strong enough to last.
fn fight(save: &mut Save, d: &Data, id: &str) -> Game {
    let mut g = save.jack_in(d, 5);
    let target = g.net.nodes[g.at].links[0];
    let ix = g.data.ice.iter().position(|i| i.id == id).unwrap();
    g.net.nodes[target].kind = NodeKind::Ice;
    g.net.nodes[target].ice = Some(ix);
    g.net.nodes[target].cleared = false;
    g.apply(Action::Move(target));
    assert_eq!(g.phase, Phase::Combat);
    let c = g.combat.as_mut().unwrap();
    (c.strength, c.strength_max) = (500, 500);
    g
}

#[test]
fn a_long_line_of_versions_would_each_pass_as_the_rows_own() {
    // Whatever the sysops write, it could have shipped in ice.ron: Data::parse takes it.
    let d = data();
    let s = deck(&d);
    for row in &d.ice {
        let (mut v, mut text) = (Version::default(), row.program.clone());
        for step in 0..24 {
            let bump = [Bump::Patch, Bump::Patch, Bump::Minor, Bump::Major][step % 4];
            let next = next_version(&d, row, &text, bump, &[], seed(s.key, &row.id, v));
            with_program(&row.id, &next.program.replace('\n', "\n            "))
                .unwrap_or_else(|e| panic!("{} after {v}: {e}\n{}", row.id, next.program));
            if let Some(b) = next.bump {
                v = v.bumped(b);
            }
            text = next.program;
        }
    }
}

#[test]
fn the_same_save_writes_the_same_versions_and_another_save_others() {
    let d = data();
    let (a, b) = (deck(&d), {
        let mut s = Save::default();
        s.ensure_key(0x7777);
        s
    });
    let line = |s: &Save, id: &str| {
        let row = d.ice.iter().find(|i| i.id == id).unwrap();
        let mut v = Version::default();
        let mut text = row.program.clone();
        let mut out = Vec::new();
        for bump in [Bump::Patch, Bump::Minor, Bump::Major, Bump::Minor] {
            let next = next_version(&d, row, &text, bump, &[], seed(s.key, id, v));
            v = v.bumped(next.bump.unwrap());
            text = next.program.clone();
            out.push((v, text.clone()));
        }
        out
    };
    let ids = ["icewall", "enigma", "rototurret", "tollbooth"];
    for id in ids {
        assert_eq!(line(&a, id), line(&a, id), "{id}");
    }
    assert!(
        ids.iter().any(|id| line(&a, id) != line(&b, id)),
        "two saves' sysops write the same patches"
    );
}

#[test]
fn a_version_in_the_save_is_in_force_from_jack_in_on_every_net() {
    let d = data();
    let mut s = deck(&d);
    let row = d.ice.iter().find(|i| i.id == "tollbooth").unwrap();
    let next = next_version(
        &d,
        row,
        &row.program,
        Bump::Major,
        &[],
        seed(s.key, "tollbooth", Version::default()),
    );
    let v = Version::default().bumped(next.bump.unwrap());
    s.ice_versions.push(IceVer {
        id: "tollbooth".into(),
        version: v,
        program: next.program.clone(),
        bump: None,
    });
    let want = basic::parse_with(&next.program, basic::Vocab::Ice(&row.verbs));
    for site in 0..3 {
        let mut g = fight(&mut s, &d, "tollbooth");
        let c = g.combat.as_ref().unwrap();
        assert_eq!(c.ice_version, v, "site {site}");
        assert_eq!(c.ice_prog, want, "site {site}");
        // Fixed at jack-in: what the save says after it doesn't reach this run.
        s.ice_versions.clear();
        g.apply(Action::EndTurn);
        assert_eq!(g.combat.as_ref().unwrap().ice_version, v);
        s.ice_versions.push(IceVer {
            id: "tollbooth".into(),
            version: v,
            program: next.program.clone(),
            bump: None,
        });
    }
}

#[test]
fn nothing_patches_itself_yet() {
    // 141.9.1 bumps nothing: a won run and a lost run leave every kind on 1.0.0.
    let d = data();
    for lose in [false, true] {
        let mut s = deck(&d);
        let mut g = fight(&mut s, &d, "enigma");
        if lose {
            g.deck.integrity = 1;
        } else {
            let c = g.combat.as_mut().unwrap();
            (c.strength, c.strength_max) = (1, 1);
            g.apply(Action::Program("10 RUN icepick".into()));
        }
        for _ in 0..40 {
            if g.phase != Phase::Combat {
                break;
            }
            g.apply(Action::Play);
            while g.pass_open() {
                g.apply(Action::Finish);
            }
            if g.phase == Phase::Combat {
                g.apply(Action::EndTurn);
            }
        }
        // Won, the run goes on: flatline it here so it ends.
        if g.phase != Phase::Flatlined {
            g.deck.integrity = 1;
            let target = g.net.nodes[g.at].links[0];
            let ix = g
                .data
                .ice
                .iter()
                .position(|i| i.id == "rototurret")
                .unwrap();
            g.net.nodes[target].kind = NodeKind::Ice;
            g.net.nodes[target].ice = Some(ix);
            g.net.nodes[target].cleared = false;
            g.apply(Action::Move(target));
            while g.phase == Phase::Combat {
                g.apply(Action::EndTurn);
            }
        }
        assert_eq!(g.phase, Phase::Flatlined, "lose {lose}");
        assert_eq!(g.history.met.iter().any(|m| m.broken), !lose, "lose {lose}");
        s.record(&g, &[], &d.rig).unwrap();
        assert!(s.ice_versions.is_empty(), "lose {lose}");
        for row in &d.ice {
            assert_eq!(
                s.ice_program(&row.id, &d),
                Some((Version::default(), row.program.as_str()))
            );
        }
    }
}

#[test]
fn a_patched_kind_is_met_blind_and_learned_on_its_own_version() {
    let d = data();
    let mut s = deck(&d);
    s.tools.push("icedb".into());
    let row = d.ice.iter().find(|i| i.id == "enigma").unwrap();
    // A run on 1.0.0 teaches some of its lines.
    let mut g = fight(&mut s, &d, "enigma");
    g.apply(Action::Program(String::new()));
    g.apply(Action::EndTurn);
    g.apply(Action::EndTurn);
    g.deck.integrity = 1;
    while g.phase == Phase::Combat {
        g.apply(Action::EndTurn);
    }
    s.record(&g, &[], &d.rig).unwrap();
    assert!(!s.kit(&d).ice_known.is_empty());
    // The sysops patch it. The next fight shows none of its lines.
    let next = next_version(
        &d,
        row,
        &row.program,
        Bump::Patch,
        &[],
        seed(s.key, "enigma", Version::default()),
    );
    s.ice_versions.push(IceVer {
        id: "enigma".into(),
        version: Version::default().bumped(next.bump.unwrap()),
        program: next.program,
        bump: None,
    });
    assert!(s.kit(&d).ice_known.is_empty());
    let g = fight(&mut s, &d, "enigma");
    assert!(ice_listing::rows(&g).iter().all(|r| r.text.is_none()));
}
