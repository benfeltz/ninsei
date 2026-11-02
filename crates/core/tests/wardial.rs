//! The wardialer: ranges, scans, traps, and hints that narrow ranges down.

use ninsei_core::bbs::{COLS, ROWS, Screen, Session};
use ninsei_core::hub::{self, Answer, Save, ShellEffect};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

fn save(bank: u32) -> Save {
    let d = data();
    let mut s = Save::default();
    s.ensure_key(0xFACE);
    s.ensure_phonebook(&d, 1);
    s.bank = bank;
    s
}

fn range<'a>(d: &'a Data, id: &str) -> &'a hub::RangeDef {
    d.exchanges.iter().find(|r| r.id == id).unwrap()
}

#[test]
fn ranges_describe_themselves() {
    let d = data();
    let local = range(&d, "local");
    assert_eq!(
        (local.pattern().as_str(), local.blocks()),
        ("555-xxxx", 100)
    );
    let core = range(&d, "brightwater_core");
    assert_eq!((core.pattern().as_str(), core.blocks()), ("555-77xx", 1));
    let nums = core.numbers(0);
    assert_eq!(
        (nums.len(), nums[0].as_str(), nums[99].as_str()),
        (100, "555-7700", "555-7799")
    );
}

#[test]
fn you_start_knowing_only_the_local_exchange() {
    let d = data();
    let mut s = save(1000);
    assert!(s.knows_range("local") && !s.knows_range("brightwater"));
    assert!(
        s.wardial(&d, "brightwater")
            .unwrap_err()
            .contains("don't know")
    );
    assert!(s.wardial(&d, "nowhere").is_err());
}

#[test]
fn a_scan_costs_the_phone_bill_and_dials_a_block() {
    let d = data();
    let mut s = save(100);
    let before = s.phonebook.len();
    let r = s.wardial(&d, "local").unwrap();
    assert_eq!(s.bank, 100 - range(&d, "local").cost);
    assert_eq!(r.cells.len(), tuning::WARDIAL_BLOCK as usize);
    assert!(
        r.cells
            .iter()
            .all(|(n, _)| n.starts_with("555-") && n.len() == 8)
    );
    let new = r
        .cells
        .iter()
        .filter(|(_, a)| matches!(a, Answer::Carrier { new: true, .. }))
        .count();
    assert_eq!(s.phonebook.len(), before + new);
    assert_eq!(s.scanned_in("local"), 1);

    let mut broke = save(0);
    assert!(
        broke
            .wardial(&d, "local")
            .unwrap_err()
            .contains("phone bill")
    );
}

#[test]
fn the_same_block_always_answers_the_same() {
    let d = data();
    let (mut a, mut b) = (save(500), save(500));
    assert_eq!(
        a.wardial(&d, "local").unwrap(),
        b.wardial(&d, "local").unwrap()
    );
}

#[test]
fn tight_ranges_are_rich_and_run_dry() {
    let d = data();
    let mut s = save(1000);
    s.learn_range("brightwater_core", "a test");
    let r = s.wardial(&d, "brightwater_core").unwrap();
    for (n, a) in &r.cells {
        assert!(n.starts_with("555-77"));
        if let Answer::Carrier { entry, new: true } = a {
            let e = s.entry(*entry).unwrap();
            assert_eq!(
                e.name, "Brightwater Mutual",
                "carriers in a corp range are that corp's"
            );
            assert_eq!(&e.number, n);
            assert!(e.tier >= 2, "a core range holds no LANs");
        }
    }
    assert!(
        s.wardial(&d, "brightwater_core")
            .unwrap_err()
            .contains("tapped out")
    );
}

#[test]
fn tighter_ranges_hit_more_often() {
    let d = data();
    let carriers = |id: &str| {
        let mut total = 0;
        for key in 1..30u64 {
            let mut s = Save::default();
            s.ensure_key(key);
            s.bank = 1000;
            s.learn_range(id, "test");
            let r = s.wardial(&d, id).unwrap();
            total += r
                .cells
                .iter()
                .filter(|(_, a)| matches!(a, Answer::Carrier { .. }))
                .count();
        }
        total
    };
    let (wide, tight) = (carriers("local"), carriers("brightwater_core"));
    assert!(tight > wide * 4, "tight {tight} vs wide {wide}");
}

#[test]
fn traps_warm_up_the_next_call() {
    let d = data();
    let mut s = save(10_000);
    s.learn_range("kestrel_core", "a test");
    while s.telco_heat == 0 {
        s.learn_range("kestrel", "a test");
        let range = if s.scanned_in("kestrel_core") == 0 {
            "kestrel_core"
        } else {
            "kestrel"
        };
        s.wardial(&d, range).unwrap();
    }
    let heat = s.telco_heat;
    let id = s.phonebook[0].id;
    let (_, base) = Save::site_params(&d, s.entry(id).unwrap());
    let g = s.dial(&d, id, 3).unwrap();
    assert_eq!(
        g.mods.trace_pct,
        base.trace_pct + tuning::TRAP_TRACE_PCT * heat
    );
    assert_eq!(s.telco_heat, 0, "one call pays for it");
}

#[test]
fn hints_found_in_a_run_come_home_even_on_a_flatline() {
    let d = data();
    for end in [Phase::JackedOut, Phase::Flatlined] {
        let mut s = save(0);
        let id = s.phonebook[0].id;
        let mut g = s.dial(&d, id, 4).unwrap();
        g.hints.push("saito".into());
        g.hints.push("local".into());
        g.phase = end;
        let r = s.record(&g, &[], &d.rig).unwrap();
        assert_eq!(
            r.learned,
            vec!["saito".to_string()],
            "already-known ranges aren't news"
        );
        assert!(s.knows_range("saito"));
        let k = s.ranges.iter().find(|k| k.id == "saito").unwrap();
        assert!(k.source.contains(&s.phonebook[0].name));
    }
}

#[test]
fn bbs_textfiles_sell_ranges() {
    let d = data();
    let mut g = Game::new(d.clone(), 2);
    let here = g.at;
    g.net.nodes[here].kind = NodeKind::Bbs;
    g.deck.credits = 1_000;
    let area = d
        .bbs
        .areas
        .iter()
        .position(|a| a.name == "TEXTFILES")
        .unwrap();
    g.apply(Action::Buy { area, file: 0 });
    assert_eq!(g.hints.len(), 1);
    // Every file area fits the BBS screen, TEXTFILES included.
    for a in 0..d.bbs.areas.len() {
        let lines = Session {
            screen: Screen::Area(a),
            status: None,
        }
        .render(&g);
        assert!(lines.len() <= ROWS);
        assert!(
            lines.iter().all(|l| ansi::visible_len(l) <= COLS),
            "area {a} overflows"
        );
    }
}

#[test]
fn a_hint_at_a_missing_range_is_caught() {
    let loot =
        include_str!("../../../data/loot.ron").replace("Hint(\"saito\")", "Hint(\"atlantis\")");
    let err = Data::parse(Sources {
        warez: include_str!("../../../data/warez.ron"),
        ice: include_str!("../../../data/ice.ron"),
        loot: &loot,
        bbs: include_str!("../../../data/bbs/board.ron"),
        splash: include_str!("../../../data/bbs/splash.txt"),
        events: include_str!("../../../data/events.ron"),
        contracts: include_str!("../../../data/contracts.ron"),
        icons: include_str!("../../../data/hub/icons.ron"),
        rig: include_str!("../../../data/hub/rig.ron"),
        networks: include_str!("../../../data/hub/networks.ron"),
        exchanges: include_str!("../../../data/hub/exchanges.ron"),
        themes: include_str!("../../../data/themes.ron"),
        displays: include_str!("../../../data/displays.ron"),
    })
    .unwrap_err();
    assert!(err.contains("atlantis"), "{err}");
}

#[test]
fn shell_ranges_and_wardial() {
    let d = data();
    let s = save(0);
    let run = |l: &str| hub::shell(l, &s, &d);
    assert_eq!(run("ranges").0.len(), 1);
    assert_eq!(run("wardial local").1, ShellEffect::Scan("local".into()));
    assert_eq!(run("wardial kestrel").1, ShellEffect::None);
}

#[test]
fn old_saves_know_the_local_exchange() {
    let s = Save::load("(version: 1, runs: 1)").unwrap();
    assert!(s.knows_range("local"));
    assert_eq!(s.telco_heat, 0);
}
