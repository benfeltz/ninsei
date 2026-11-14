//! 141.11.1: the ICE sources you own, one per kind and version, in the save and in a run.

use ninsei_core::hub::{IceSource, IceVer, Save};
use ninsei_core::ice_version::{Bump, Version};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

fn fresh(d: &Data) -> Save {
    let mut s = Save::default();
    s.ensure_key(0x1432);
    s.ensure_phonebook(d, 0);
    s.ensure_hardware(d);
    s
}

fn ix(d: &Data, id: &str) -> usize {
    d.ice.iter().position(|i| i.id == id).unwrap()
}

/// Enigma's row with its first number changed: a version past 1.0.0 that passes its check.
fn patched(d: &Data) -> (Version, String) {
    let row = &d.ice[ix(d, "enigma")];
    let program = row.program.replace("TRACE 10", "TRACE 9");
    assert!(d.check_ice_program(row, &program).is_ok());
    (Version::FIRST.bumped(Bump::Patch), program)
}

fn ship(s: &mut Save, d: &Data) -> Version {
    let (v, program) = patched(d);
    s.ice_versions.push(IceVer {
        id: "enigma".into(),
        version: v,
        program,
        bump: Some(Bump::Patch),
        crack: 0,
    });
    v
}

/// A run from `s` in a fight with `ice` too strong to fall.
fn fight(s: &mut Save, d: &Data, ice: &str) -> Game {
    let mut g = s.jack_in(d, 11);
    let target = g.net.nodes[g.at].links[0];
    g.net.nodes[target].kind = NodeKind::Ice;
    g.net.nodes[target].ice = Some(ix(d, ice));
    g.net.nodes[target].cleared = false;
    g.apply(Action::Move(target));
    assert_eq!(g.phase, Phase::Combat);
    let c = g.combat.as_mut().unwrap();
    (c.strength, c.strength_max) = (500, 500);
    g
}

#[test]
fn a_source_is_learned_once_and_owned() {
    let d = data();
    let mut s = fresh(&d);
    assert!(
        !s.owns_source(&d, "enigma", Version::FIRST),
        "a new deck owns none"
    );
    assert!(s.learn_source(&d, "enigma", Version::FIRST));
    assert!(s.owns_source(&d, "enigma", Version::FIRST));
    assert!(
        !s.learn_source(&d, "enigma", Version::FIRST),
        "twice is once"
    );
    assert_eq!(s.ice_sources.len(), 1);
    // 1.0.0's program is the row's, kept as empty so a change to the row doesn't strand it.
    assert_eq!(s.ice_sources[0].program, "");
    // Owning 1.0.0 says nothing of the next.
    let v = ship(&mut s, &d);
    assert!(!s.owns_source(&d, "enigma", v));
    assert!(s.learn_source(&d, "enigma", v));
    assert!(s.owns_source(&d, "enigma", v));
    // A version nobody put out, or a kind that isn't, is nothing to learn.
    assert!(!s.learn_source(&d, "enigma", v.bumped(Bump::Major)));
    assert!(!s.learn_source(&d, "no-such-ice", Version::FIRST));
    assert_eq!(s.ice_sources.len(), 2);
}

#[test]
fn a_source_takes_no_hdd_slot_and_no_stash_room() {
    let d = data();
    let mut s = fresh(&d);
    let (stash, kit) = (s.stash.len(), s.kit(&d).programs.len());
    s.learn_source(&d, "enigma", Version::FIRST);
    assert_eq!(s.stash.len(), stash);
    assert_eq!(s.kit(&d).programs.len(), kit);
}

#[test]
fn a_save_from_before_sources_loads() {
    let d = data();
    let s = fresh(&d);
    let text = s.to_text();
    assert!(text.contains("ice_sources"));
    let old: String = text
        .lines()
        .filter(|l| !l.contains("ice_sources"))
        .collect::<Vec<_>>()
        .join("\n");
    let back = Save::load(&old).expect("loads");
    assert!(back.ice_sources.is_empty());
}

#[test]
fn a_source_whose_program_no_longer_reads_is_owned_by_nothing() {
    let d = data();
    let mut s = fresh(&d);
    let v = Version::FIRST.bumped(Bump::Patch);
    // The row changed under the save: its verbs no longer say this.
    s.ice_sources.push(IceSource {
        id: "enigma".into(),
        version: v,
        program: "10 SEGFAULT\n20 SEGFAULT".into(),
    });
    assert!(!s.ice_sources[0].reads(&d));
    // Never learned that way, and dropped as the save loads.
    assert!(!s.learn_source(&d, "enigma", v));
    assert!(s.ensure_sources(&d));
    assert!(s.ice_sources.is_empty());
    assert!(!s.owns_source(&d, "enigma", v));
    assert!(s.kit(&d).ice_sources.is_empty(), "never into a run");
    assert!(!s.ensure_sources(&d), "nothing more to drop");
}

#[test]
fn a_run_sees_what_the_save_owned_at_jack_in() {
    let d = data();
    let mut s = fresh(&d);
    s.learn_source(&d, "enigma", Version::FIRST);
    let e = ix(&d, "enigma");
    let g = fight(&mut s, &d, "enigma");
    assert!(g.owns_source(e, Version::FIRST));
    assert!(!g.owns_source(ix(&d, "rototurret"), Version::FIRST));
    // What the save gets after jack-in isn't this run's.
    let mut s2 = fresh(&d);
    let g = fight(&mut s2, &d, "enigma");
    s2.learn_source(&d, "enigma", Version::FIRST);
    assert!(!g.owns_source(e, Version::FIRST));
}

#[test]
fn a_run_owns_the_version_it_runs_and_no_other() {
    let d = data();
    let mut s = fresh(&d);
    s.learn_source(&d, "enigma", Version::FIRST);
    let v = ship(&mut s, &d);
    let e = ix(&d, "enigma");
    let mut g = fight(&mut s, &d, "enigma");
    assert_eq!(g.version_met(e), v);
    assert!(!g.owns_source(e, v), "1.0.0's source is not 1.0.1's");
    let tick = g.history.tick;
    g.apply(Action::GetSource { ice: e, version: v });
    assert!(g.owns_source(e, v));
    assert_eq!(g.history.tick, tick + 1, "an action history hears");
    let log = g.log.len();
    g.apply(Action::GetSource { ice: e, version: v });
    assert_eq!(g.sources_got.len(), 1, "twice is once");
    assert_eq!(g.log.len(), log + 1, "and says so: {:?}", g.log.last());
    g.apply(Action::GetSource {
        ice: e,
        version: v.bumped(Bump::Minor),
    });
    assert_eq!(g.sources_got.len(), 1, "not a version this run runs");
}

/// A run that got Enigma's 1.0.0 source, ended by `exit` (true) or a flatline.
fn got_one(exit: bool) -> Save {
    let d = data();
    let mut s = fresh(&d);
    let mut g = fight(&mut s, &d, "enigma");
    let e = ix(&d, "enigma");
    g.apply(Action::GetSource {
        ice: e,
        version: Version::FIRST,
    });

    assert!(g.owns_source(e, Version::FIRST));
    if exit {
        g.phase = Phase::JackedOut;
        g.combat = None;
    } else {
        g.deck.integrity = 1;
        while g.phase == Phase::Combat {
            g.apply(Action::EndTurn);
        }
        assert_eq!(g.phase, Phase::Flatlined);
    }
    s.record(&g, &[], &d.rig).unwrap();
    s
}

#[test]
fn a_source_got_in_a_run_comes_home_on_an_exit_and_on_a_flatline() {
    let d = data();
    for exit in [true, false] {
        let s = got_one(exit);
        assert!(s.owns_source(&d, "enigma", Version::FIRST), "exit {exit}");
        assert_eq!(s.ice_sources.len(), 1);
    }
}

#[test]
fn the_save_and_the_fight_agree_on_what_is_owned() {
    // A 1.0.1 source of another program than the 1.0.1 in force is owned by neither.
    let d = data();
    let mut s = fresh(&d);
    let v = ship(&mut s, &d);
    s.ice_sources.push(IceSource {
        id: "enigma".into(),
        version: v,
        program: d.ice[ix(&d, "enigma")]
            .program
            .replace("ATTACK 4", "ATTACK 5"),
    });
    assert!(s.ice_sources[0].reads(&d), "it reads");
    assert!(!s.owns_source(&d, "enigma", v));
    let g = fight(&mut s, &d, "enigma");
    assert!(!g.owns_source(ix(&d, "enigma"), v));
    // The one in force is.
    assert!(s.learn_source(&d, "enigma", v));
    assert!(s.owns_source(&d, "enigma", v));
    let g = fight(&mut s, &d, "enigma");
    assert!(g.owns_source(ix(&d, "enigma"), v));
}
