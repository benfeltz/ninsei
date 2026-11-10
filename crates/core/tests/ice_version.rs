//! ICE that patch themselves (141.9): the ICE's machine on its row, the class grammar, versions
//! and the generator that writes the next one, and the versions a save carries into a fight.

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
