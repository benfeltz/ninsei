//! The phonebook: tiers, traits, heat, and dialing networks.

use ninsei_core::hub::{self, Save, ShellEffect};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

fn book() -> (Data, Save) {
    let d = data();
    let mut s = Save::default();
    s.ensure_key(7);
    assert!(s.ensure_phonebook(&d, 99));
    assert_eq!(
        s.phonebook.len(),
        d.starts.always.len(),
        "a new deck knows one board and the networks everyone starts with"
    );
    s.add_starters(&d);
    (d, s)
}

/// An entry of `tier` with exactly `traits`.
fn entry(s: &mut Save, tier: u32, traits: &[&str]) -> u32 {
    let e = s.phonebook.last().unwrap().clone();
    let id = s.next_entry;
    s.next_entry += 1;
    s.phonebook.push(hub::Entry {
        id,
        number: format!("555-9{id:03}"),
        tier,
        traits: traits.iter().map(|t| t.to_string()).collect(),
        heat: 0,
        runs: 0,
        ..e
    });
    id
}

#[test]
fn the_partner_directory_gives_a_handful_of_numbers() {
    let (d, s) = book();
    let fixed = d.starts.always.len();
    assert_eq!(s.phonebook.len(), fixed + tuning::PHONEBOOK_START.len());
    let tiers: Vec<u32> = s.phonebook[fixed..].iter().map(|e| e.tier).collect();
    assert_eq!(tiers, tuning::PHONEBOOK_START);
    for (i, e) in s.phonebook.iter().enumerate().skip(fixed) {
        assert!(e.number.starts_with("555-") && e.number.len() == 8);
        assert!(
            !s.phonebook[..i]
                .iter()
                .any(|o| o.number == e.number || o.name == e.name)
        );
        let (lo, hi) = d.networks.tier(e.tier).unwrap().traits;
        assert!((lo..=hi).contains(&(e.traits.len() as u32)));
        assert!(e.traits.iter().all(|t| d.networks.trait_def(t).is_some()));
    }
    let mut again = s.clone();
    assert!(
        !again.ensure_phonebook(&d, 1),
        "an existing book is left alone"
    );
    assert!(
        again.add_starters(&d).is_empty(),
        "the directory hands them out once"
    );
    let mut twin = Save::default();
    twin.ensure_key(7);
    twin.ensure_phonebook(&d, 1);
    twin.add_starters(&d);
    assert_eq!(twin.phonebook, s.phonebook, "same save, same partners");
}

#[test]
fn tiers_set_size_strength_and_pay() {
    let (d, mut s) = book();
    let lan = entry(&mut s, 1, &[]);
    let mega = entry(&mut s, 3, &[]);
    let g1 = s.dial(&d, lan, 4).unwrap();
    let g3 = s.dial(&d, mega, 4).unwrap();
    let (t1, t3) = (d.networks.tier(1).unwrap(), d.networks.tier(3).unwrap());
    assert_eq!(g1.net.layers.len(), t1.mid_layers + 2);
    assert_eq!(g3.net.layers.len(), t3.mid_layers + 2);
    assert_eq!(g3.mods.ice_bonus, t3.ice_bonus);
    assert_eq!((g1.mods.pay_pct, g3.mods.pay_pct), (t1.pay_pct, t3.pay_pct));
    assert_eq!(g3.site.as_ref().unwrap().tier, 3);
}

#[test]
fn traits_change_the_network() {
    let (d, mut s) = book();
    let quiet = entry(&mut s, 2, &["no_bbs", "graveyard"]);
    let g = s.dial(&d, quiet, 11).unwrap();
    assert!(g.net.bbs().is_none());
    // Graveyard takes a quarter off the tier's own trace rate.
    let base = d.networks.tier(2).unwrap().trace_pct;
    assert_eq!(g.mods.trace_pct, base * 75 / 100);
    assert_eq!(
        g.site.as_ref().unwrap().traits,
        vec!["no BBS".to_string(), "graveyard shift".to_string()]
    );

    // A sentry-heavy network fields more sentries than a plain one.
    let sentries = |traits: &[&str], s: &mut Save| {
        let id = entry(s, 2, traits);
        (0..60)
            .map(|seed| {
                let g = s.dial(&d, id, seed).unwrap();
                g.net
                    .nodes
                    .iter()
                    .filter_map(|n| n.ice)
                    .filter(|&i| d.ice[i].class == IceClass::Sentry)
                    .count()
            })
            .sum::<usize>()
    };
    let plain = sentries(&[], &mut s);
    let heavy = sentries(&["sentry_heavy"], &mut s);
    assert!(
        heavy > plain * 3 / 2,
        "sentry-heavy {heavy} vs plain {plain}"
    );
}

#[test]
fn trace_and_pay_scale() {
    let m = RunMods {
        ice_bonus: 0,
        trace_pct: 75,
        pay_pct: 200,
    };
    assert_eq!(m.trace(4), 3);
    assert_eq!(m.trace(1), 1, "a gain never rounds away");
    assert_eq!(m.trace(0), 0);
    assert_eq!(m.pay(50), 100);

    let (d, mut s) = book();
    let id = entry(&mut s, 2, &["graveyard"]);
    let mut g = s.dial(&d, id, 3).unwrap();
    let to = g.net.nodes[g.at].links[0];
    g.apply(Action::Move(to));
    assert_eq!(g.deck.trace, g.mods.trace(tuning::TRACE_PER_HOP));
    assert!(g.deck.trace < tuning::TRACE_PER_HOP);
}

#[test]
fn heat_builds_on_a_network_and_cools_elsewhere() {
    let (d, mut s) = book();
    // Two of the generated numbers (the school net's quiet traits would muddy the trace).
    let fixed = d.starts.always.len();
    let a = s.phonebook[fixed].id;
    let b = s.phonebook[fixed + 1].id;
    for _ in 0..2 {
        let mut g = s.dial(&d, a, 1).unwrap();
        g.phase = Phase::JackedOut;
        s.record(&g, &[], &d.rig);
    }
    assert_eq!(s.entry(a).unwrap().heat, 2 * tuning::HEAT_PER_RUN);
    assert_eq!(s.entry(a).unwrap().runs, 2);
    let hot = s.dial(&d, a, 1).unwrap();
    let cold = s.dial(&d, b, 1).unwrap();
    assert!(
        hot.mods.ice_bonus > cold.mods.ice_bonus
            || s.entry(a).unwrap().tier < s.entry(b).unwrap().tier
    );
    assert!(hot.mods.trace_pct > 100);

    let mut g = s.dial(&d, b, 2).unwrap();
    g.phase = Phase::Flatlined;
    s.record(&g, &[], &d.rig);
    assert_eq!(
        s.entry(a).unwrap().heat,
        2 * tuning::HEAT_PER_RUN - tuning::HEAT_COOL
    );
    for _ in 0..10 {
        let mut g = s.dial(&d, a, 9).unwrap();
        g.phase = Phase::JackedOut;
        s.record(&g, &[], &d.rig);
    }
    assert_eq!(s.entry(a).unwrap().heat, tuning::HEAT_MAX);
}

#[test]
fn dialing_brings_the_loadout() {
    let (d, mut s) = book();
    s.rig.cpu = 1;
    let id = s.phonebook[0].id;
    let g = s.dial(&d, id, 5).unwrap();
    assert_eq!(g.deck.cpu_max, tuning::START_CPU + 1);
    assert_eq!(g.hdd.len(), tuning::START_WAREZ.len());
    assert!(s.dial(&d, 999, 5).is_err());
}

#[test]
fn shell_dials_by_position_or_number() {
    let (d, s) = book();
    let run = |l: &str| hub::shell(l, &s, &d);
    let second = &s.phonebook[1];
    assert_eq!(run("dial 2").1, ShellEffect::Dial(second.id, None));
    assert_eq!(
        run(&format!("dial {}", second.number)).1,
        ShellEffect::Dial(second.id, None)
    );
    assert_eq!(run("dial 2 ff").1, ShellEffect::Dial(second.id, Some(0xff)));
    assert_eq!(run("dial 99").1, ShellEffect::None);
    assert_eq!(run("pb").0.len(), s.phonebook.len() + s.boards.len());
}

#[test]
fn old_saves_keep_their_start() {
    let d = data();
    let mut s = Save::load("(version: 1, runs: 4)").unwrap();
    assert!(s.started, "a save from before kits has started");
    assert!(!s.ensure_phonebook(&d, 3), "no kit is applied to it");
    assert_eq!(Save::load(&s.to_text()).unwrap(), s);
}

#[test]
fn every_deck_starts_with_the_school_net() {
    let d = data();
    for kit in &d.starts.kits {
        let mut s = Save {
            kit: kit.id.clone(),
            ..Save::default()
        };
        s.ensure_key(5);
        assert!(s.ensure_phonebook(&d, 1));
        assert!(
            s.phonebook
                .iter()
                .any(|e| e.name == "Ninsei Community College"),
            "{}",
            kit.id
        );
        assert!(s.knows_board(ninsei_core::hub::FIRST_BOARD));
        assert_eq!(s.kit, kit.id);
        assert_eq!(s.free(&d), &kit.free[..]);
        assert_eq!(s.loadout.starters, kit.free);
        assert!(!s.ensure_phonebook(&d, 1), "once");
    }
}

#[test]
fn kits_hand_out_what_they_say() {
    let d = data();
    let mut s = Save {
        kit: "classic".into(),
        ..Save::default()
    };
    s.ensure_key(9);
    s.ensure_phonebook(&d, 1);
    assert!(s.knows_board("gutter") && s.has_tool("wardial") && s.has_tool("hexed"));
    assert_eq!(
        s.phonebook.len(),
        d.starts.always.len() + tuning::PHONEBOOK_START.len()
    );
    let e = s.phonebook[0].id;
    let g = s.dial(&d, e, 4).unwrap();
    assert!(
        g.hdd.iter().any(|p| p.spec.name == "IcePick"),
        "classic ships a breaker"
    );
    let mut p = Save {
        kit: "pick".into(),
        ..Save::default()
    };
    p.ensure_phonebook(&d, 1);
    assert!(p.free(&d).iter().any(|w| w == "icepick"));
    assert!(
        !p.free(&d).iter().any(|w| w == "decoder"),
        "specialists are found"
    );
    let unknown = Save {
        kit: "nope".into(),
        ..Save::default()
    };
    assert_eq!(
        unknown.free(&d),
        d.starts.kit("").free.as_slice(),
        "unknown means default"
    );
}

#[test]
fn the_school_nets_board_is_the_gutter() {
    let d = data();
    let mut s = Save::default();
    s.ensure_phonebook(&d, 1);
    let school = s
        .phonebook
        .iter()
        .find(|e| e.name == "Ninsei Community College")
        .unwrap()
        .clone();
    assert_eq!(d.boards[Save::site_board(&d, &school)].id, "gutter");
    let g = s.dial(&d, school.id, 3).unwrap();
    assert!(
        g.net.bbs().is_some(),
        "a way to the Gutter from the first network"
    );
}

#[test]
fn the_phonebook_is_a_directory_until_nightcomm() {
    let (d, mut s) = book();
    let (plain, _) = hub::shell("phonebook", &s, &d);
    assert!(plain.iter().all(|l| !l.contains("heat")), "{plain:?}");
    assert!(plain[0].contains(&s.phonebook[0].business));
    s.learn_tool("nightcomm");
    let (intel, _) = hub::shell("phonebook", &s, &d);
    assert!(intel[0].contains("heat"), "{intel:?}");
    assert!(
        !intel.iter().any(|l| l.contains(" T1") || l.contains(" T2")),
        "tier is Tiger Team Pro's business, not NiGHTCOMM's: {intel:?}"
    );
}

#[test]
fn nightcomm_comes_from_dead_sector() {
    let d = data();
    let b = &d.boards[d.board("deadsector").unwrap()];
    assert!(b.areas.iter().any(|a| {
        a.always
            && a.files
                .iter()
                .any(|f| f.item == ninsei_core::data::Drop::Tool("nightcomm".into()))
    }));
}
