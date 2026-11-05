//! Between runs: banking, the RIG shop, the stash and loadout, and crafting at home.

use ninsei_core::binary::{self, OFF_CHECKSUM, OFF_OPT, OFF_POWER};
use ninsei_core::hub::{Save, Upgrade};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

fn save() -> Save {
    let mut s = Save::default();
    s.ensure_key(0xC0FFEE);
    s
}

/// Ends a run at the exit with `credits`.
fn exit(g: &mut Game, credits: u32) {
    g.deck.credits = credits;
    g.phase = Phase::JackedOut;
}

fn fix(b: &mut binary::Bin) {
    b[OFF_CHECKSUM] = binary::checksum(b);
}

#[test]
fn rig_prices_climb_and_cap() {
    let d = data();
    for def in &d.rig {
        let mut last = 0;
        for lvl in 0..def.max {
            let c = def.cost(lvl).unwrap();
            assert!(
                c > last,
                "{:?} level {lvl} should cost more than the last",
                def.kind
            );
            last = c;
        }
        assert_eq!(def.cost(def.max), None);
    }
}

#[test]
fn buying_spends_the_bank() {
    let d = data();
    let mut s = save();
    assert!(s.buy(&d.rig, Upgrade::Ram).unwrap_err().contains("costs"));
    s.bank = 10_000;
    let first = d
        .rig
        .iter()
        .find(|u| u.kind == Upgrade::Ram)
        .unwrap()
        .cost(0)
        .unwrap();
    s.buy(&d.rig, Upgrade::Ram).unwrap();
    assert_eq!((s.rig.ram, s.bank), (1, 10_000 - first));
    let cpu_max = d.rig.iter().find(|u| u.kind == Upgrade::Cpu).unwrap().max;
    for _ in 0..cpu_max {
        s.buy(&d.rig, Upgrade::Cpu).unwrap();
    }
    assert!(s.buy(&d.rig, Upgrade::Cpu).unwrap_err().contains("maxed"));
}

#[test]
fn rig_bonuses_reach_the_deck() {
    let d = data();
    let mut s = save();
    s.rig.cpu = 1;
    s.rig.ram = 2;
    s.rig.integrity = 1;
    let g = s.jack_in(&d, &d.rig, 5);
    assert_eq!(g.deck.cpu_max, tuning::START_CPU + 1);
    assert_eq!(g.deck.ram_max, tuning::START_RAM + 2);
    assert_eq!(g.deck.integrity, tuning::START_INTEGRITY + 3);
}

#[test]
fn loadout_respects_the_hdd_and_never_goes_empty() {
    let d = data();
    let mut s = save();
    // Starters fill 4 of 6.
    assert_eq!(s.loadout_size(), tuning::START_WAREZ.len());
    s.toggle_starter(&d, &d.rig, "patch").unwrap();
    assert_eq!(s.loadout_size(), tuning::START_WAREZ.len() - 1);
    assert!(
        s.toggle_starter(&d, &d.rig, "torch").is_err(),
        "torch isn't a starter"
    );
    s.loadout.starters.clear();
    let g = s.jack_in(&d, &d.rig, 1);
    assert_eq!(
        g.hdd.len(),
        tuning::START_WAREZ.len(),
        "an empty loadout falls back to the starters"
    );
}

#[test]
fn credits_and_fragments_bank_on_exit_only() {
    let d = data();
    let mut s = save();
    let mut g = s.jack_in(&d, &d.rig, 1);
    g.deck.fragments = 3;
    exit(&mut g, 200);
    let r = s.record(&g, &[], &d.rig).unwrap();
    assert_eq!((r.banked, s.bank, s.fragments), (200, 200, 3));

    let mut g = s.jack_in(&d, &d.rig, 2);
    g.deck.credits = 300;
    g.deck.fragments = 5;
    g.phase = Phase::Flatlined;
    let r = s.record(&g, &[], &d.rig).unwrap();
    assert_eq!(r.banked, 300 * tuning::DEADMAN_PCT / 100);
    assert_eq!(
        (s.bank, s.fragments),
        (200 + r.banked, 3),
        "fragments die with the deck"
    );
}

#[test]
fn a_hand_tuned_program_survives_the_trip_home_and_back() {
    let d = data();
    let mut s = save();
    s.learn_tool("hexed");
    // IcePick, downloaded from the Gutter's FREEWARE into the stash: a breaker to tune.
    let w = d.warez_ix("icepick").unwrap();
    let item = s.next_item;
    s.next_item += 1;
    s.stash.push(ninsei_core::hub::StashItem {
        id: item,
        origin: "icepick".into(),
        hex: binary::to_hex(&binary::encode(&d.warez[w], s.key)),
    });
    s.toggle_stash(&d.rig, item).unwrap();
    let mut g = s.jack_in(&d, &d.rig, 7);
    // Tune icepick at a BBS: optimize to O1, then spend the budget on power.
    let here = g.at;
    g.net.nodes[here].kind = NodeKind::Bbs;
    g.deck.fragments = 1;
    let i = g.hdd.iter().position(|p| p.spec.name == "IcePick").unwrap();
    g.apply(Action::Optimize { slot: i });
    let mut b = g.hdd[i].bin;
    b[OFF_POWER] = 6; // cpu 1, ram 2, opt 1: budget 3 + 1 + 2
    fix(&mut b);
    g.apply(Action::Patch { slot: i, bin: b });
    assert_eq!(g.hdd[i].spec.effect, Effect::Break { power: 6, vs: None });
    let tuned = g.hdd[i].spec.clone();

    exit(&mut g, 0);
    let r = s.record(&g, &[i], &d.rig).unwrap();
    assert_eq!(r.kept, vec!["icepick.exe".to_string()]);
    assert_eq!(s.stash.len(), 1);
    let stored = s.stash[0].bin().unwrap();
    assert_eq!(
        binary::check(&stored, s.key).unwrap(),
        tuned,
        "re-signed with the save's key"
    );

    // Take it into the next run: it leaves the stash and arrives intact.
    let id = s.stash[0].id;
    s.toggle_stash(&d.rig, id).unwrap();
    let g = s.jack_in(&d, &d.rig, 8);
    assert!(s.stash.is_empty(), "programs in a run aren't in the stash");
    assert!(g.hdd.iter().any(|p| p.spec == tuned));
}

#[test]
fn flatlining_loses_programs_you_brought() {
    let d = data();
    let mut s = save();
    let mut g = s.jack_in(&d, &d.rig, 1);
    exit(&mut g, 0);
    s.record(&g, &[0], &d.rig);
    let id = s.stash[0].id;
    s.toggle_stash(&d.rig, id).unwrap(); // the suite + 1 on a 6-slot HDD
    let mut g = s.jack_in(&d, &d.rig, 2);
    g.phase = Phase::Flatlined;
    s.record(&g, &[0, 1, 2], &d.rig);
    assert!(s.stash.is_empty(), "nothing comes home from a flatline");
}

#[test]
fn keeping_is_capped_by_stash_slots() {
    let d = data();
    let mut s = save();
    let mut g = s.jack_in(&d, &d.rig, 3);
    exit(&mut g, 0);
    let r = s.record(&g, &[0, 1, 2, 3, 0], &d.rig).unwrap();
    assert_eq!(r.kept.len(), tuning::STASH_BASE_SLOTS as usize);
    assert_eq!(s.stash.len() as u32, s.stash_cap(&d.rig));
}

#[test]
fn crafting_at_home() {
    let d = data();
    let mut s = save();
    let mut g = s.jack_in(&d, &d.rig, 4);
    exit(&mut g, 0);
    let fw = g
        .hdd
        .iter()
        .position(|p| p.spec.name == "Firewall")
        .unwrap();
    s.record(&g, &[fw], &d.rig);
    let id = s.stash[0].id;

    let mut b = s.stash[0].bin().unwrap();
    b[OFF_POWER] = 6;
    fix(&mut b);
    s.patch(id, &b).unwrap();
    assert_eq!(s.stash[0].bin().unwrap()[OFF_POWER], 6);

    // Typing a bigger opt level doesn't sign it.
    let mut forged = b;
    forged[OFF_OPT] = 3;
    fix(&mut forged);
    assert!(s.patch(id, &forged).unwrap_err().contains("signature"));

    assert!(s.optimize(id).is_err(), "no fragments yet");
    s.fragments = 1;
    s.optimize(id).unwrap();
    assert_eq!((s.fragments, s.stash[0].bin().unwrap()[OFF_OPT]), (0, 1));

    s.scrap(id);
    assert!(s.stash.is_empty());
}

#[test]
fn in_a_run_you_compile_only_at_a_bbs() {
    let mut g = Game::new(data(), 6);
    let mut b = g.hdd[1].bin;
    b[OFF_POWER] += 1;
    fix(&mut b);
    g.apply(Action::Patch { slot: 1, bin: b });
    assert!(g.log.last().unwrap().text.contains("BBS"));
    assert!(g.compile_blocker().is_some());
}

#[test]
fn hex_round_trips() {
    let g = Game::new(data(), 1);
    let b = g.hdd[0].bin;
    assert_eq!(binary::from_hex(&binary::to_hex(&b)), Some(b));
    assert_eq!(binary::from_hex("abc"), None);
    assert_eq!(binary::from_hex(&"zz".repeat(32)), None);
}

#[test]
fn saves_from_before_the_stash_still_load() {
    let s = Save::load("(version: 1, runs: 2, exits: 1)").unwrap();
    assert_eq!((s.bank, s.fragments, s.key), (0, 0, 0));
    assert_eq!(s.loadout.starters.len(), tuning::START_WAREZ.len());
    let mut s = s;
    assert!(s.ensure_key(42));
    assert!(!s.ensure_key(99), "a key is set once");
    assert_eq!(Save::load(&s.to_text()).unwrap(), s);
}

#[test]
fn a_new_deck_ships_with_the_security_suite_only() {
    let d = data();
    let mut s = save();
    let g = s.jack_in(&d, &d.rig, 3);
    let mut names: Vec<String> = g.hdd.iter().map(|p| d.warez[p.warez].id.clone()).collect();
    names.sort();
    assert_eq!(names, vec!["firewall", "patch"], "no breakers in the box");
    assert!(s.toggle_starter(&d, &d.rig, "icepick").is_err());
    // An older save's breakers in the loadout don't come free any more.
    s.loadout.starters = tuning::CLASSIC_WAREZ
        .iter()
        .map(|w| w.to_string())
        .collect();
    assert_eq!(s.kit(&d, &d.rig).programs.len(), tuning::START_WAREZ.len());
}
