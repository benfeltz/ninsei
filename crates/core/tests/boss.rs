//! 138.1: the nets' AIs. Models and versions in `ai.ron`, picked by a net's tier and heat, and
//! SEGFAULT as their verb only.

use ninsei_core::basic::Verb;
use ninsei_core::tuning::HEAT_MAX;
use ninsei_core::*;

/// The builtin data with `ai.ron` edited: `find` (which must be there) replaced by `replace`.
fn ai_edited(find: &str, replace: &str) -> Result<Data, String> {
    let ai = include_str!("../../../data/ai.ron");
    assert!(ai.contains(find), "ai.ron has no `{find}`");
    let ai = ai.replacen(find, replace, 1);
    Data::parse(Sources {
        ai: &ai,
        ..Sources::builtin()
    })
}

#[test]
fn every_tier_and_heat_runs_a_version_of_a_model() {
    let d = Data::builtin().unwrap();
    for t in &d.networks.tiers {
        for heat in 0..=HEAT_MAX {
            let ix = d
                .boss_for(t.tier, heat)
                .unwrap_or_else(|| panic!("tier {} heat {heat}", t.tier));
            let row = &d.ice[ix];
            assert!(row.boss && row.tier == data::BOSS_TIER, "{}", row.id);
            let model = d.boss_model(ix).unwrap();
            // Its name is the product's, with the version: `WATCHMAN 1.2`.
            assert!(row.name.starts_with(&model.name), "{}", row.name);
            assert_eq!(
                (row.cpu, row.ram, row.hdd),
                (model.cpu, model.ram, model.hdd)
            );
            d.check_ice_program(row, &row.program)
                .unwrap_or_else(|e| panic!("{}: {e}", row.id));
        }
    }
    // Heat moves a net up its line: a hot tier-1 net runs a later build than a cold one.
    assert_ne!(d.boss_for(1, 0), d.boss_for(1, HEAT_MAX));
    // A bigger tier runs a bigger machine.
    let cpu = |t| d.ice[d.boss_for(t, 0).unwrap()].cpu;
    assert!(cpu(3) >= cpu(1));
}

#[test]
fn segfault_is_on_no_ordinary_ice_and_the_sysops_never_write_it() {
    let d = Data::builtin().unwrap();
    for i in d.ice.iter().filter(|i| !i.boss) {
        assert!(!i.verbs.contains(&Verb::Segfault), "{}", i.id);
    }
    assert!(
        d.ice
            .iter()
            .any(|i| i.boss && i.verbs.contains(&Verb::Segfault))
    );
    for c in &d.ice_grammar.classes {
        for r in &c.reads {
            assert!(
                r.responses.iter().all(|(v, _)| *v != Verb::Segfault),
                "{}",
                r.read
            );
        }
    }
}

#[test]
fn validate_refuses_a_bad_ai() {
    let bad = [
        (
            r#"(model: "watchman", version: "1.2""#,
            r#"(model: "nobody", version: "1.2""#,
            "unknown model",
        ),
        (
            r#"(model: "watchman", version: "1.4""#,
            r#"(model: "watchman", version: "1.2""#,
            "listed twice",
        ),
        (
            r#"(tier: 1, heat: 3, model: "watchman", version: "1.4")"#,
            r#"(tier: 1, heat: 3, model: "watchman", version: "9.9")"#,
            "isn't a version",
        ),
        (
            r#"(tier: 1, heat: 0, model: "watchman", version: "1.2")"#,
            r#"(tier: 1, heat: 1, model: "watchman", version: "1.2")"#,
            "tier 1 runs no AI at heat 0",
        ),
        (
            r#"(tier: 1, heat: 3, model: "watchman", version: "1.4")"#,
            r#"(tier: 1, heat: 0, model: "watchman", version: "1.4")"#,
            "tier 1 at heat 0 is listed twice",
        ),
        (
            r#"(tier: 1, heat: 3, model: "watchman", version: "1.4")"#,
            r#"(tier: 1, heat: 99, model: "watchman", version: "1.4")"#,
            "heat tops out",
        ),
        (
            r#"(tier: 1, heat: 3, model: "watchman", version: "1.4")"#,
            r#"(tier: 9, heat: 3, model: "watchman", version: "1.4")"#,
            "no network has",
        ),
        // A verb outside its model's vocabulary is a bad line.
        (
            "60 ATTACK 5\n            70 ATTACK 4",
            "60 FORTIFY 5\n            70 ATTACK 4",
            "unknown keyword",
        ),
        // A number outside its model's range.
        (
            "60 ATTACK 5\n            70 ATTACK 4",
            "60 ATTACK 50\n            70 ATTACK 4",
            "ATTACK takes 3 to 7",
        ),
        // More lines than its model's HDD.
        (
            "cpu: 1, ram: 2, hdd: 10,",
            "cpu: 1, ram: 2, hdd: 3,",
            "more than its hdd",
        ),
        (
            "models: [",
            r#"models: [(id: "watchman", company: "Maas-Neotek", name: "WATCHMAN", class: Sentry,
                cpu: 1, ram: 2, hdd: 10, verbs: [Attack], ranges: [(Attack, 3, 7)]),"#,
            "model `watchman` is listed twice",
        ),
        (r#"strength: 22,"#, r#"strength: 0,"#, "strength 0"),
    ];
    for (find, replace, why) in bad {
        let err = ai_edited(find, replace).expect_err(why);
        assert!(err.contains(why), "{why}: {err}");
    }
}

#[test]
fn an_ordinary_row_may_not_segfault_and_ice_ron_cant_write_an_ai() {
    let ice = include_str!("../../../data/ice.ron");
    let find = "verbs: [Attack, Trace, Fortify, Spoof],";
    assert!(ice.contains(find));
    let err = Data::parse(Sources {
        ice: &ice.replacen(find, "verbs: [Attack, Trace, Fortify, Spoof, Segfault],", 1),
        ..Sources::builtin()
    })
    .unwrap_err();
    assert!(err.contains("rival") && err.contains("SEGFAULT"), "{err}");
    let err = Data::parse(Sources {
        ice: &ice.replacen("tier: 0, strength: 20", "tier: 3, strength: 20", 1),
        ..Sources::builtin()
    })
    .unwrap_err();
    assert!(err.contains("rival") && err.contains("tier 3"), "{err}");
}
