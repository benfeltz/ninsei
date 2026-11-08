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
    assert!(s.buy(&d, Upgrade::Ram).unwrap_err().contains("costs"));
    s.bank = 10_000;
    let first = d
        .rig
        .iter()
        .find(|u| u.kind == Upgrade::Ram)
        .unwrap()
        .cost(0)
        .unwrap();
    s.buy(&d, Upgrade::Ram).unwrap();
    assert_eq!((s.rig.ram, s.bank), (1, 10_000 - first));
    // On a chassis with room for every level, RIG's own maximum is what stops it.
    let roomy = starter_cpu(3, 100);
    let cpu_max = d.rig.iter().find(|u| u.kind == Upgrade::Cpu).unwrap().max;
    for _ in 0..cpu_max {
        s.buy(&roomy, Upgrade::Cpu).unwrap();
    }
    assert!(s.buy(&roomy, Upgrade::Cpu).unwrap_err().contains("maxed"));
}

#[test]
fn rig_bonuses_reach_the_deck() {
    // A chassis with room on CPU, so the level shows.
    let d = starter_cpu(3, 100);
    let mut s = save();
    s.rig.cpu = 1;
    s.rig.ram = 2;
    s.rig.integrity = 1;
    let base = |u| s.stat_base(&d, u);
    let (cpu, ram, integrity) = (
        base(Upgrade::Cpu),
        base(Upgrade::Ram),
        base(Upgrade::Integrity),
    );
    let g = s.jack_in(&d, 5);
    assert_eq!(g.deck.cpu_max, cpu + 1);
    assert_eq!(g.deck.ram_max, ram + 2);
    assert_eq!(g.deck.integrity, integrity + 3);
}

#[test]
fn a_run_starts_from_the_decks_own_base() {
    let d = starter_cpu(5, 8);
    let mut s = save();
    s.ensure_hardware(&d);
    s.rig.cpu = 1;
    let g = s.jack_in(&d, 5);
    assert_eq!(g.deck.cpu_max, 6, "the starter's base 5 plus one level");
    assert_eq!(g.deck.caps.cpu, Some(8));
}

#[test]
fn loadout_respects_the_hdd_and_never_goes_empty() {
    let d = data();
    let mut s = save();
    // Starters fill 4 of 6.
    assert_eq!(s.loadout_size(), tuning::START_WAREZ.len());
    s.toggle_starter(&d, "patch").unwrap();
    assert_eq!(s.loadout_size(), tuning::START_WAREZ.len() - 1);
    assert!(
        s.toggle_starter(&d, "torch").is_err(),
        "torch isn't a starter"
    );
    s.loadout.starters.clear();
    let g = s.jack_in(&d, 1);
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
    let mut g = s.jack_in(&d, 1);
    g.deck.fragments = 3;
    exit(&mut g, 200);
    let r = s.record(&g, &[], &d.rig).unwrap();
    assert_eq!((r.banked, s.bank, s.fragments), (200, 200, 3));

    let mut g = s.jack_in(&d, 2);
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
    s.toggle_stash(&d, item).unwrap();
    let mut g = s.jack_in(&d, 7);
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
    s.toggle_stash(&d, id).unwrap();
    let g = s.jack_in(&d, 8);
    assert!(s.stash.is_empty(), "programs in a run aren't in the stash");
    assert!(g.hdd.iter().any(|p| p.spec == tuned));
}

#[test]
fn flatlining_loses_programs_you_brought() {
    let d = data();
    let mut s = save();
    let mut g = s.jack_in(&d, 1);
    exit(&mut g, 0);
    s.record(&g, &[0], &d.rig);
    let id = s.stash[0].id;
    s.toggle_stash(&d, id).unwrap(); // the suite + 1 on a 6-slot HDD
    let mut g = s.jack_in(&d, 2);
    g.phase = Phase::Flatlined;
    s.record(&g, &[0, 1, 2], &d.rig);
    assert!(s.stash.is_empty(), "nothing comes home from a flatline");
}

#[test]
fn keeping_is_capped_by_stash_slots() {
    let d = data();
    let mut s = save();
    let mut g = s.jack_in(&d, 3);
    exit(&mut g, 0);
    let r = s.record(&g, &[0, 1, 2, 3, 0], &d.rig).unwrap();
    assert_eq!(r.kept.len(), tuning::STASH_BASE_SLOTS as usize);
    assert_eq!(s.stash.len() as u32, s.stash_cap(&d.rig));
}

#[test]
fn crafting_at_home() {
    let d = data();
    let mut s = save();
    let mut g = s.jack_in(&d, 4);
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
fn a_new_deck_ships_with_the_suite_and_a_generic_breaker() {
    let d = data();
    let mut s = save();
    let g = s.jack_in(&d, 3);
    let mut names: Vec<String> = g.hdd.iter().map(|p| d.warez[p.warez].id.clone()).collect();
    names.sort();
    assert_eq!(
        names,
        vec!["firewall", "icepick", "patch"],
        "the default kit"
    );
    assert!(
        s.toggle_starter(&d, "decoder").is_err(),
        "specialists are found, not free"
    );
    // An older save's Decoder in the loadout doesn't come free any more.
    s.loadout.starters = tuning::CLASSIC_WAREZ
        .iter()
        .map(|w| w.to_string())
        .collect();
    assert_eq!(s.kit(&d).programs.len(), tuning::START_WAREZ.len());
}

#[test]
fn rig_stops_at_the_chassis() {
    let d = starter_cpu(3, 4);
    let mut s = save();
    s.ensure_hardware(&d);
    s.bank = 100_000;
    s.buy(&d, Upgrade::Cpu).unwrap();
    let bank = s.bank;
    let why = s.buy(&d, Upgrade::Cpu).unwrap_err();
    assert!(why.contains(&s.deck_def(&d).title()), "{why}");
    assert!(why.contains("4 cyc"), "{why}");
    assert_eq!((s.rig.cpu, s.bank), (1, bank), "nothing charged");
    assert_eq!(s.upgrade_blocker(&d, Upgrade::Cpu), Some(why));
    assert_eq!(s.upgrade_blocker(&d, Upgrade::Ram), None);
}

#[test]
fn order_blocker_says_why_before_buying() {
    use ninsei_core::shopper::{Item, Stock};
    let d = starter_cpu(3, 3);
    let mut s = save();
    s.ensure_hardware(&d);
    s.bank = 100_000;
    let cpu = Item::Upgrade(Upgrade::Cpu);
    let listing = s.catalog(&d).into_iter().find(|l| l.item == cpu).unwrap();
    assert!(
        matches!(listing.stock, Stock::Price(_)),
        "the magazine still lists it: it doesn't know your deck"
    );
    let why = s.order_blocker(&d, &cpu).expect("the order form refuses");
    assert!(why.contains("tops out at 3"), "{why}");
    assert_eq!(s.buy_item(&d, &cpu), Err(why));
    assert_eq!((s.rig.cpu, s.bank), (0, 100_000));
}

/// The shipped data with the starter deck's CPU row set to `base` and `max`.
fn starter_cpu(base: u32, max: u32) -> Data {
    let mut d = Data::builtin().unwrap();
    let starter = d.hardware.starter_deck().id.clone();
    let deck = d
        .hardware
        .decks
        .iter_mut()
        .find(|x| x.id == starter)
        .unwrap();
    let row = deck
        .stats
        .iter_mut()
        .find(|s| s.line == ninsei_core::hub::Upgrade::Cpu)
        .unwrap();
    (row.base, row.max) = (base, max);
    d
}

#[test]
fn an_old_save_past_its_decks_cap_keeps_its_levels_and_runs_at_the_cap() {
    let d = data();
    let mut s = save();
    s.ensure_hardware(&d);
    let cap = s.stat_cap(&d, Upgrade::Ram).expect("the starter caps RAM");
    // Bought on an older, roomier chassis: every RAM level RIG sells.
    let rig_max = d.rig.iter().find(|u| u.kind == Upgrade::Ram).unwrap().max;
    s.rig.ram = rig_max;
    assert!(
        s.stat_base(&d, Upgrade::Ram) + rig_max > cap,
        "past this case"
    );
    let text = s.to_text();
    let mut back = Save::load(&text).unwrap();
    back.ensure_hardware(&d);
    assert_eq!(back.rig.ram, rig_max, "nothing taken back");
    let g = back.jack_in(&d, 3);
    assert_eq!(g.deck.ram_max, cap, "runs at the cap");
    assert!(back.buy(&d, Upgrade::Ram).is_err(), "and sells no more");
}

#[test]
fn an_unknown_deck_falls_back_to_the_starters_chassis() {
    let d = data();
    let mut s = save();
    s.deck = "a deck from a newer build".into();
    let starter = d.hardware.starter_deck();
    for u in [Upgrade::Cpu, Upgrade::Ram, Upgrade::Hdd, Upgrade::Integrity] {
        let row = starter.stat(u).unwrap();
        assert_eq!(s.stat_base(&d, u), row.base, "{u:?}");
        assert_eq!(s.stat_cap(&d, u), Some(row.max), "{u:?}");
    }
    s.ensure_hardware(&d);
    assert_eq!(s.deck, starter.id);
}
