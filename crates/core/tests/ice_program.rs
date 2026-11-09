//! ICE run their own programs: the data that holds them, the turn that runs them and the dry
//! run that telegraphs them.

use ninsei_core::*;

/// The builtin ICE with one more row, `probe`, that has this program, budgets and verbs.
fn with_row(program: &str, cpu: u32, ram: u32, verbs: &str) -> Result<Data, String> {
    let ice = include_str!("../../../data/ice.ron");
    let end = ice.rfind(']').unwrap();
    let row = format!(
        "    (id: \"probe\", name: \"Probe\", class: Sentry, tier: 1, strength: 10, \
         intents: [Attack(1)], cpu: {cpu}, ram: {ram}, verbs: [{verbs}], program: \"{program}\"),\n"
    );
    let ice = format!("{}{row}{}", &ice[..end], &ice[end..]);
    Data::parse(Sources {
        ice: &ice,
        ..Sources::builtin()
    })
}

#[test]
fn every_builtin_ice_has_a_program_that_parses() {
    let d = Data::builtin().unwrap();
    for ice in &d.ice {
        let p = basic::parse_with(&ice.program, basic::Vocab::Ice(&ice.verbs));
        assert!(!p.lines.is_empty(), "{}", ice.id);
        assert!(
            p.lines.iter().all(|l| p.fault(l.no).is_none()),
            "{}",
            ice.id
        );
        assert!(ice.cpu >= 1 && ice.ram >= 1, "{}", ice.id);
    }
}

#[test]
fn a_good_extra_row_validates() {
    with_row(
        r"10 IF TARGET SHIELD > 0\n20 TRACE 3\n30 ATTACK 2",
        1,
        1,
        "Attack, Trace",
    )
    .unwrap();
}

#[test]
fn a_bad_program_is_rejected() {
    let bad = [
        (
            r"10 ATTACK 2\n20 TRACE 3",
            1,
            1,
            "Attack",
            "TRACE isn't on its row",
        ),
        (
            r"10 RUN icepick\n20 ATTACK 2",
            1,
            1,
            "Attack",
            "the deck's word",
        ),
        (
            r"10 ATTACK 2\n20 GOTO 99",
            1,
            1,
            "Attack",
            "a GOTO to nowhere",
        ),
        (
            r"10 IF TARGET RAN nothing\n20 ATTACK 2",
            1,
            1,
            "Attack",
            "no such program",
        ),
        (r"10 ATTACK 2", 0, 1, "Attack", "no cycles"),
        (r"10 ATTACK 2", 1, 0, "Attack", "no RAM"),
        (r"10 REM idle\n20 GOTO 10", 1, 1, "Attack", "never acts"),
        (r"ATTACK 2", 1, 1, "Attack", "no line number"),
    ];
    for (program, cpu, ram, verbs, why) in bad {
        let err = with_row(program, cpu, ram, verbs).expect_err(why);
        assert!(err.contains("probe"), "{why}: {err}");
    }
}
