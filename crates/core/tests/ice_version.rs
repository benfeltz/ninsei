//! ICE that patch themselves (141.9): the ICE's machine on its row, the class grammar, versions
//! and the generator that writes the next one, and the versions a save carries into a fight.

use ninsei_core::hub::Save;
use ninsei_core::*;

/// The builtin data with `ice.ron` edited by `edit` (a find and its replacement, which must be
/// there).
fn ice_edited(find: &str, replace: &str) -> Result<Data, String> {
    let ice = include_str!("../../../data/ice.ron");
    assert!(ice.contains(find), "ice.ron has no `{find}`");
    let ice = ice.replacen(find, replace, 1);
    Data::parse(Sources {
        ice: &ice,
        ..Sources::builtin()
    })
}

#[test]
fn every_row_has_a_machine_its_program_fits() {
    let d = Data::builtin().unwrap();
    for ice in &d.ice {
        assert!(ice.hdd >= 1, "{}", ice.id);
        d.check_ice_program(ice, &ice.program)
            .unwrap_or_else(|e| panic!("{}: {e}", ice.id));
        for v in ice.verbs.iter().filter(|v| v.takes_number()) {
            assert!(ice.range(*v).is_some(), "{} {v:?}", ice.id);
        }
    }
}

#[test]
fn a_program_longer_than_its_hdd_is_rejected() {
    let err = ice_edited(
        "hdd: 5, ranges: [(Fortify, 3, 5), (Attack, 3, 5)]",
        "hdd: 2, ranges: [(Fortify, 3, 5), (Attack, 3, 5)]",
    )
    .unwrap_err();
    assert!(err.contains("icewall") && err.contains("hdd"), "{err}");
}

#[test]
fn rems_do_not_count_against_the_hdd() {
    // Ice Wall's three lines and a REM fit an HDD of three.
    ice_edited(
        "hdd: 5, ranges: [(Fortify, 3, 5), (Attack, 3, 5)],\n        program: \"",
        "hdd: 3, ranges: [(Fortify, 3, 5), (Attack, 3, 5)],\n        program: \"\n            5 REM a wall",
    )
    .unwrap();
}

#[test]
fn a_number_outside_its_range_is_rejected() {
    let err = ice_edited(
        "(Fortify, 3, 5), (Attack, 3, 5)]",
        "(Fortify, 3, 5), (Attack, 5, 6)]",
    )
    .unwrap_err();
    assert!(err.contains("icewall") && err.contains("ATTACK"), "{err}");
}

#[test]
fn a_bad_range_is_rejected() {
    let bad = [
        // Attack on Ice Wall's row with no range.
        (
            "(Fortify, 3, 5), (Attack, 3, 5)]",
            "(Fortify, 3, 5)]",
            "has no range",
        ),
        // Past the three-digit field.
        (
            "(Fortify, 3, 5), (Attack, 3, 5)]",
            "(Fortify, 3, 5), (Attack, 3, 1000)]",
            "at most",
        ),
        // Lowest last.
        (
            "(Fortify, 3, 5), (Attack, 3, 5)]",
            "(Fortify, 5, 3), (Attack, 3, 5)]",
            "lowest first",
        ),
        // A verb the row doesn't have.
        (
            "(Fortify, 3, 5), (Attack, 3, 5)]",
            "(Fortify, 3, 5), (Attack, 3, 5), (Trace, 1, 2)]",
            "isn't on its verbs",
        ),
        // Twice.
        (
            "(Fortify, 3, 5), (Attack, 3, 5)]",
            "(Fortify, 3, 5), (Attack, 3, 5), (Attack, 3, 5)]",
            "two ranges",
        ),
    ];
    for (find, replace, why) in bad {
        let err = ice_edited(find, replace).expect_err(why);
        assert!(err.contains("icewall") && err.contains(why), "{why}: {err}");
    }
    // SEGFAULT takes no number, so it has no range.
    let err = ice_edited(
        "(Trace, 4, 6), (Attack, 3, 5), (Fortify, 1, 3)]",
        "(Trace, 4, 6), (Attack, 3, 5), (Fortify, 1, 3), (Segfault, 0, 1)]",
    )
    .unwrap_err();
    assert!(
        err.contains("hydra") && err.contains("takes no number"),
        "{err}"
    );
}

/// The builtin data with `ice_grammar.ron` edited (a find, which must be there, and its
/// replacement).
fn grammar_edited(find: &str, replace: &str) -> Result<Data, String> {
    let g = include_str!("../../../data/ice_grammar.ron");
    assert!(g.contains(find), "ice_grammar.ron has no `{find}`");
    let g = g.replacen(find, replace, 1);
    Data::parse(Sources {
        ice_grammar: &g,
        ..Sources::builtin()
    })
}

#[test]
fn the_shipped_grammar_loads() {
    let d = Data::builtin().unwrap();
    for class in [IceClass::Barrier, IceClass::CodeGate, IceClass::Sentry] {
        assert!(d.ice_grammar.reads(class).len() >= 2, "{class:?}");
    }
    assert!(!d.ice_grammar.program_attacks.is_empty());
}

#[test]
fn a_bad_grammar_is_rejected() {
    let bad = [
        (
            "\"TARGET RAN icepick\"",
            "\"TARGET RAN nothing\"",
            "no program is called",
        ),
        (
            "\"TARGET CPU < n\",    n: Some((1, 2))",
            "\"TARGET MOOD < n\",    n: Some((1, 2))",
            "unknown condition",
        ),
        (
            "\"TARGET CPU < n\",    n: Some((1, 2))",
            "\"TARGET CPU < n\",    n: None",
            "no range",
        ),
        (
            "\"TARGET RAN icepick\", n: None",
            "\"TARGET RAN icepick\", n: Some((1, 2))",
            "no `n`",
        ),
        (
            "\"TARGET CPU < n\",    n: Some((1, 2))",
            "\"TARGET CPU < n\",    n: Some((2, 1))",
            "lowest first",
        ),
        (
            "responses: [(Fortify, 3), (Attack, 1)]),",
            "responses: [(Segfault, 3)]),",
            "which no BARRIER has",
        ),
        (
            "responses: [(Fortify, 3), (Attack, 1)]),",
            "responses: []),",
            "no responses",
        ),
        (
            "weight: 3, responses: [(Fortify, 3), (Attack, 1)]),",
            "weight: 0, responses: [(Fortify, 3), (Attack, 1)]),",
            "weight 0",
        ),
        ("(tier: 1, most: 0),", "", "no tier 1"),
        (
            "(tier: 1, most: 0),",
            "(tier: 1, most: 0), (tier: 1, most: 1),",
            "twice",
        ),
    ];
    for (find, replace, why) in bad {
        let err = grammar_edited(find, replace).expect_err(why);
        assert!(
            err.contains("ice_grammar.ron") && err.contains(why),
            "{why}: {err}"
        );
    }
}

#[test]
fn a_class_needs_two_reads() {
    // The Sentry keeps its first read only: every line after it up to the end of its list goes.
    let g = include_str!("../../../data/ice_grammar.ron");
    let start = g.find("(class: Sentry, reads: [").unwrap();
    let lines: Vec<&str> = g[start..].lines().collect();
    let end = lines.iter().position(|l| l.trim() == "]),").unwrap();
    let kept = [&lines[..2], &lines[end..]].concat().join("\n");
    let g = format!("{}{kept}\n", &g[..start]);
    let err = Data::parse(Sources {
        ice_grammar: &g,
        ..Sources::builtin()
    })
    .unwrap_err();
    assert!(err.contains("SENTRY has fewer than two reads"), "{err}");
}

#[test]
fn a_row_may_not_attack_the_program_past_its_tier() {
    // Archer, tier 2, SPOOFs once: a tier 2 that may hold none refuses it.
    let err = grammar_edited("(tier: 2, most: 1),", "(tier: 2, most: 0),").unwrap_err();
    assert!(
        err.contains("archer") && err.contains("attack the program"),
        "{err}"
    );
}

#[test]
fn versions_count_and_print_as_semver() {
    use ice_version::{Bump, Version};
    let v = Version::default();
    assert_eq!(v.to_string(), "1.0.0");
    let p = v.bumped(Bump::Patch);
    assert_eq!(p.to_string(), "1.0.1");
    let m = p.bumped(Bump::Minor);
    assert_eq!(m.to_string(), "1.1.0");
    assert_eq!(
        m.bumped(Bump::Patch).bumped(Bump::Major).to_string(),
        "2.0.0"
    );
    assert!(m > p && p > v);
    assert_eq!(Bump::Major.smaller(), Some(Bump::Minor));
    assert_eq!(Bump::Patch.smaller(), None);
}

#[test]
fn straight_line_ice_round_trip_through_blocks() {
    let d = Data::builtin().unwrap();
    let mut straight = 0;
    for ice in &d.ice {
        let vocab = basic::Vocab::Ice(&ice.verbs);
        let p = basic::parse_with(&ice.program, vocab);
        let Some(blocks) = ice_version::blocks(&p) else {
            // Only the ICE that jump aren't in block form.
            assert!(
                p.lines
                    .iter()
                    .any(|l| matches!(l.stmt, basic::Stmt::Goto(_))),
                "{}",
                ice.id
            );
            continue;
        };
        straight += 1;
        let again = basic::parse_with(&ice_version::render(&blocks), vocab);
        let stmts = |p: &basic::Program| p.lines.iter().map(|l| l.stmt.clone()).collect::<Vec<_>>();
        assert_eq!(stmts(&again), stmts(&p), "{}", ice.id);
        assert_eq!(ice_version::blocks(&again), Some(blocks), "{}", ice.id);
    }
    assert_eq!(straight, 7);
    for id in ["archer", "hydra"] {
        let ice = d.ice.iter().find(|i| i.id == id).unwrap();
        let p = basic::parse_with(&ice.program, basic::Vocab::Ice(&ice.verbs));
        assert_eq!(ice_version::blocks(&p), None, "{id}");
    }
}

#[test]
fn what_is_not_in_block_form() {
    let verbs = [basic::Verb::Attack];
    let vocab = basic::Vocab::Ice(&verbs);
    for text in [
        "10 ATTACK 2\n20 IF TURN > 1",
        "10 IF TURN > 1\n20 IF TURN > 2\n30 ATTACK 2",
        "10 ATTACK 2\n20 STOP",
        "10 REM x\n20 ATTACK 2",
        "10 ATTACK 2\n20 GOTO 10",
    ] {
        assert_eq!(
            ice_version::blocks(&basic::parse_with(text, vocab)),
            None,
            "{text}"
        );
    }
}

/// Every builtin ICE whose program is in block form.
fn block_form(d: &Data) -> Vec<&IceDef> {
    d.ice
        .iter()
        .filter(|i| {
            ice_version::blocks(&basic::parse_with(&i.program, basic::Vocab::Ice(&i.verbs)))
                .is_some()
        })
        .collect()
}

fn blocks_of(ice: &IceDef, text: &str) -> Vec<ice_version::Block> {
    ice_version::blocks(&basic::parse_with(text, basic::Vocab::Ice(&ice.verbs)))
        .expect("a version stays in block form")
}

/// The statements of `blocks`, as a sorted list: a moved block keeps them.
fn statements(blocks: &[ice_version::Block]) -> Vec<String> {
    let mut out: Vec<String> = blocks
        .iter()
        .flat_map(|b| {
            b.read
                .iter()
                .map(|r| format!("IF {r}"))
                .chain([b.line.clone()])
        })
        .collect();
    out.sort();
    out
}

#[test]
fn every_version_is_valid_and_the_change_it_says() {
    use ice_version::{Bump, next_version};
    let d = Data::builtin().unwrap();
    let ice = block_form(&d);
    assert_eq!(ice.len(), 7);
    for row in ice {
        let before = blocks_of(row, &row.program);
        for bump in [Bump::Patch, Bump::Minor, Bump::Major] {
            for seed in 0..200u64 {
                let next = next_version(&d, row, &row.program, bump, &[], seed);
                let why = format!("{} {bump:?} seed {seed}:\n{}", row.id, next.program);
                // Every builtin ICE can take every kind of change.
                assert_eq!(next.bump, Some(bump), "{why}");
                d.check_ice_program(row, &next.program)
                    .unwrap_or_else(|e| panic!("{why}\n{e}"));
                assert_eq!(
                    next,
                    next_version(&d, row, &row.program, bump, &[], seed),
                    "{why}"
                );
                let after = blocks_of(row, &next.program);
                assert_ne!(after, before, "{why}");
                match bump {
                    Bump::Patch => {
                        assert_eq!(after.len(), before.len(), "{why}");
                        // The same lines in a new order, or one line's number changed.
                        let (a, b) = (statements(&after), statements(&before));
                        let differ = a.iter().filter(|s| !b.contains(s)).count();
                        assert!(differ <= 1, "{why}");
                        if differ == 1 {
                            let pos = |bl: &[ice_version::Block]| {
                                bl.iter().map(|x| x.read.is_some()).collect::<Vec<_>>()
                            };
                            assert_eq!(pos(&after), pos(&before), "{why}");
                        }
                    }
                    Bump::Minor => {
                        assert_eq!(after.len().abs_diff(before.len()), 1, "{why}");
                        // A read and its line come or go; the lines on their own stay.
                        let bare = |bl: &[ice_version::Block]| {
                            let mut v: Vec<String> = bl
                                .iter()
                                .filter(|x| x.read.is_none())
                                .map(|x| x.line.clone())
                                .collect();
                            v.sort();
                            v
                        };
                        assert_eq!(bare(&after), bare(&before), "{why}");
                    }
                    Bump::Major => {
                        assert_eq!(after.len(), before.len(), "{why}");
                        // No read twice.
                        let mut r: Vec<String> =
                            after.iter().filter_map(|x| x.read.clone()).collect();
                        r.sort();
                        r.dedup();
                        assert_eq!(r.len(), after.iter().filter(|x| x.read.is_some()).count());
                        let reads = |bl: &[ice_version::Block]| {
                            bl.iter().filter_map(|x| x.read.clone()).collect::<Vec<_>>()
                        };
                        assert_ne!(reads(&after), reads(&before), "{why}");
                    }
                }
            }
        }
    }
}

#[test]
fn versions_climb_and_stay_on_the_machine() {
    use ice_version::{Bump, Version, next_version, seed};
    let d = Data::builtin().unwrap();
    for row in block_form(&d) {
        let (mut v, mut text) = (Version::default(), row.program.clone());
        for step in 0..60 {
            let bump = [Bump::Patch, Bump::Minor, Bump::Major][step % 3];
            let next = next_version(&d, row, &text, bump, &[], seed(7, &row.id, v));
            d.check_ice_program(row, &next.program)
                .unwrap_or_else(|e| panic!("{} at {v}: {e}\n{}", row.id, next.program));
            // No read twice, at any number.
            let mut reads: Vec<String> = blocks_of(row, &next.program)
                .iter()
                .filter_map(|b| b.read.clone())
                .map(|r| r.trim_end_matches(|c: char| c.is_ascii_digit()).to_string())
                .collect();
            let all = reads.len();
            reads.sort();
            reads.dedup();
            assert_eq!(reads.len(), all, "{} at {v}:\n{}", row.id, next.program);
            if let Some(b) = next.bump {
                v = v.bumped(b);
            }
            text = next.program;
        }
        assert!(v > Version::default(), "{}", row.id);
    }
}

#[test]
fn a_program_not_in_block_form_comes_back_unchanged() {
    use ice_version::{Bump, next_version};
    let d = Data::builtin().unwrap();
    let archer = d.ice.iter().find(|i| i.id == "archer").unwrap();
    for bump in [Bump::Patch, Bump::Minor, Bump::Major] {
        let next = next_version(&d, archer, &archer.program, bump, &[], 3);
        assert_eq!(next.bump, None);
        assert_eq!(next.program, archer.program);
    }
}

#[test]
fn a_change_that_cannot_fit_falls_back_and_at_worst_changes_nothing() {
    use ice_version::{Bump, next_version};
    let d = Data::builtin().unwrap();
    let mut row = d.ice.iter().find(|i| i.id == "rototurret").unwrap().clone();
    // One line, its number pinned, on a full HDD: nothing can move, change, come or go.
    row.hdd = 1;
    row.ranges = vec![(basic::Verb::Attack, 5, 5)];
    let next = next_version(&d, &row, "10 ATTACK 5", Bump::Major, &[], 1);
    assert_eq!(next.bump, None);
    assert_eq!(next.program, "10 ATTACK 5");
    // With room for a number, a major that has no read to replace and no room to add one is a
    // patch.
    row.ranges = vec![(basic::Verb::Attack, 4, 6)];
    let next = next_version(&d, &row, "10 ATTACK 5", Bump::Major, &[], 1);
    assert_eq!(next.bump, Some(Bump::Patch));
    assert_ne!(next.program, "10 ATTACK 5");
}

#[test]
fn the_seed_is_the_saves_the_kinds_and_the_versions() {
    use ice_version::{Bump, Version, seed};
    let v = Version::default();
    assert_eq!(seed(1, "enigma", v), seed(1, "enigma", v));
    assert_ne!(seed(1, "enigma", v), seed(2, "enigma", v));
    assert_ne!(seed(1, "enigma", v), seed(1, "bulwark", v));
    assert_ne!(
        seed(1, "enigma", v),
        seed(1, "enigma", v.bumped(Bump::Patch))
    );
}

#[test]
fn a_bias_draws_its_read_more() {
    use ice_version::{Bump, next_version};
    let d = Data::builtin().unwrap();
    let wall = d.ice.iter().find(|i| i.id == "icewall").unwrap();
    let count = |bias: &[(String, u32)]| {
        (0..300u64)
            .filter(|&s| {
                next_version(&d, wall, &wall.program, Bump::Major, bias, s)
                    .program
                    .contains("TARGET RAN icepick")
            })
            .count()
    };
    let plain = count(&[]);
    let biased = count(&[("TARGET RAN icepick".to_string(), 40)]);
    assert!(biased > plain * 2, "{plain} -> {biased}");
}

/// A fight against ICE `id` from a run started with `kit`, strong enough to last.
fn fight_with(data: Data, kit: &Kit, id: &str) -> Game {
    let mut g = Game::start(data, 7, kit);
    let target = g.net.nodes[g.at].links[0];
    let ice = g.data.ice.iter().position(|i| i.id == id).unwrap();
    g.net.nodes[target].kind = NodeKind::Ice;
    g.net.nodes[target].ice = Some(ice);
    g.net.nodes[target].cleared = false;
    g.apply(Action::Move(target));
    assert_eq!(g.phase, Phase::Combat);
    let c = g.combat.as_mut().unwrap();
    c.strength = 500;
    c.strength_max = 500;
    g
}

/// A save whose Rototurret the sysops have patched to `program` at 1.0.1.
fn patched(program: &str) -> Save {
    use ice_version::{Bump, Version};
    Save {
        key: 99,
        ice_versions: vec![hub::IceVer {
            id: "rototurret".into(),
            version: Version::default().bumped(Bump::Patch),
            program: program.into(),
        }],
        ..Save::default()
    }
}

#[test]
fn a_version_in_the_save_is_what_the_fight_runs() {
    use fight::NextMove;
    use ice_version::Version;
    let d = Data::builtin().unwrap();
    let save = patched("10 ATTACK 3\n20 ATTACK 8");
    let (v, program) = save.ice_program("rototurret", &d).unwrap();
    assert_eq!(v.to_string(), "1.0.1");
    assert_eq!(program, "10 ATTACK 3\n20 ATTACK 8");
    let kit = save.kit(&d);
    assert_eq!(
        kit.ice_programs,
        vec![("rototurret".to_string(), v, program.to_string())]
    );
    let mut g = fight_with(d.clone(), &kit, "rototurret");
    let c = g.combat.as_ref().unwrap();
    assert_eq!(c.ice_version, v);
    assert_eq!(c.ice_prog.text(), "10 ATTACK 3\n20 ATTACK 8");
    assert_eq!(g.next_move(), Some(NextMove::Act(Intent::Attack(3))));
    g.apply(Action::EndTurn);
    assert_eq!(g.next_move(), Some(NextMove::Act(Intent::Attack(8))));
    // Every other kind runs its row's, at 1.0.0.
    let g = fight_with(d.clone(), &kit, "hunter");
    let c = g.combat.as_ref().unwrap();
    assert_eq!(c.ice_version, Version::default());
    let hunter = d.ice.iter().find(|i| i.id == "hunter").unwrap();
    let row = basic::parse_with(&hunter.program, basic::Vocab::Ice(&hunter.verbs));
    assert_eq!(c.ice_prog, row);
}

#[test]
fn a_bare_kit_runs_the_rows_program() {
    let d = Data::builtin().unwrap();
    assert!(Save::default().kit(&d).ice_programs.is_empty());
    let g = fight_with(d.clone(), &Kit::default(), "rototurret");
    let c = g.combat.as_ref().unwrap();
    assert_eq!(c.ice_version.to_string(), "1.0.0");
    let row = d.ice.iter().find(|i| i.id == "rototurret").unwrap();
    assert_eq!(
        c.ice_prog,
        basic::parse_with(&row.program, basic::Vocab::Ice(&row.verbs))
    );
}

#[test]
fn a_saved_program_that_no_longer_fits_falls_back_to_the_rows() {
    let d = Data::builtin().unwrap();
    let row = d.ice.iter().find(|i| i.id == "rototurret").unwrap();
    // Past ATTACK's range, a verb it doesn't have, longer than its HDD, and nonsense.
    for bad in [
        "10 ATTACK 99",
        "10 TRACE 3",
        "10 ATTACK 3\n20 ATTACK 3\n30 ATTACK 3\n40 ATTACK 3\n50 ATTACK 3\n60 ATTACK 3\n70 ATTACK 3",
        "garbage",
    ] {
        let save = patched(bad);
        let (v, program) = save.ice_program("rototurret", &d).unwrap();
        assert_eq!(v.to_string(), "1.0.0", "{bad}");
        assert_eq!(program, row.program, "{bad}");
        assert!(save.kit(&d).ice_programs.is_empty(), "{bad}");
    }
    assert_eq!(Save::default().ice_program("nobody", &d), None);
}

#[test]
fn a_save_from_before_versions_loads() {
    let text = Save::default().to_text();
    let old: String = text
        .lines()
        .filter(|l| !l.trim_start().starts_with("ice_versions"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_ne!(old, text);
    let s = Save::load(&old).unwrap();
    assert!(s.ice_versions.is_empty());
    // And a save with a version round-trips.
    let p = patched("10 ATTACK 3\n20 ATTACK 8");
    assert_eq!(Save::load(&p.to_text()), Ok(p));
}

#[test]
fn the_same_save_gives_the_same_versions() {
    use ice_version::{Bump, next_version, seed};
    let d = Data::builtin().unwrap();
    let save = patched("10 ATTACK 3\n20 ATTACK 8");
    assert_eq!(save.kit(&d), save.kit(&d));
    let row = d.ice.iter().find(|i| i.id == "rototurret").unwrap();
    let (v, program) = save.ice_program("rototurret", &d).unwrap();
    let draw = |key| next_version(&d, row, program, Bump::Minor, &[], seed(key, &row.id, v));
    assert_eq!(draw(save.key), draw(save.key));
    // And a run with versions in force plays the same for the same seed.
    let kit = save.kit(&d);
    let mut a = fight_with(d.clone(), &kit, "rototurret");
    let mut b = fight_with(d.clone(), &kit, "rototurret");
    for _ in 0..3 {
        a.apply(Action::EndTurn);
        b.apply(Action::EndTurn);
    }
    assert_eq!(a.deck, b.deck);
    assert_eq!(a.combat, b.combat);
}
