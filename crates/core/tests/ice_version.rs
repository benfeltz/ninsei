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
