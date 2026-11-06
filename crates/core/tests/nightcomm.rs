//! NiGHTCOMM: heat words, the iNTEL list, the wardial map, ranges and run results.

use ninsei_core::hub::{Found, RunEnd, Save};
use ninsei_core::nightcomm::{self, Block, Sort, Tab};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

/// A started deck with NiGHTCOMM and money for the phone bill.
fn deck(d: &Data) -> Save {
    let mut s = Save::default();
    s.ensure_key(0x5EED);
    s.ensure_phonebook(d, 1);
    s.add_starters(d);
    s.learn_tool("nightcomm");
    s.bank = 1_000_000;
    s
}

#[test]
fn heat_reads_cold_warm_and_hot() {
    assert_eq!(nightcomm::heat_word(0), "cold");
    assert_eq!(nightcomm::heat_word(1), "warm");
    assert_eq!(nightcomm::heat_word(nightcomm::HOT - 1), "warm");
    assert_eq!(nightcomm::heat_word(nightcomm::HOT), "HOT");
    assert_eq!(nightcomm::heat_word(tuning::HEAT_MAX), "HOT");
}

#[test]
fn keys_pick_tabs_and_sorts_cycle() {
    assert_eq!(Tab::from_key('1'), Some(Tab::Intel));
    assert_eq!(Tab::from_key('3'), Some(Tab::Ranges));
    assert_eq!(Tab::from_key('4'), Some(Tab::Term));
    assert_eq!(Tab::from_key('0'), None);
    assert_eq!(Tab::from_key('9'), None);
    let mut s = Sort::default();
    for _ in 0..Sort::ALL.len() {
        s = s.next();
    }
    assert_eq!(s, Sort::default(), "the sort key comes back round");
}

#[test]
fn intel_sorts_by_heat_and_filters() {
    let d = data();
    let mut s = deck(&d);
    assert!(s.phonebook.len() >= 3);
    s.phonebook[1].heat = 5;
    s.phonebook[2].heat = 2;
    let hot: Vec<u32> = nightcomm::intel(&s, "", Sort::Heat)
        .iter()
        .map(|e| e.heat)
        .collect();
    assert!(hot.windows(2).all(|w| w[0] >= w[1]), "{hot:?}");
    assert_eq!(hot[0], 5);
    let by_number = nightcomm::intel(&s, "", Sort::Number);
    assert!(by_number.windows(2).all(|w| w[0].number <= w[1].number));
    let school = nightcomm::intel(&s, "COLLEGE", Sort::Heat);
    assert_eq!(school.len(), 1);
    assert_eq!(nightcomm::intel(&s, "555 0420", Sort::Heat).len(), 1);
    assert_eq!(
        nightcomm::intel(&s, "-", Sort::Heat).len(),
        0,
        "a lone dash isn't a number"
    );
}

#[test]
fn runs_remember_how_they_ended() {
    let d = data();
    let mut s = deck(&d);
    let id = s.phonebook[0].id;
    for i in 0..(tuning::RESULTS_KEPT + 2) {
        let mut g = s.dial(&d, id, i as u64).unwrap();
        g.phase = if i % 2 == 0 {
            Phase::JackedOut
        } else {
            Phase::Flatlined
        };
        s.record(&g, &[], &d.rig);
    }
    let e = s.entry(id).unwrap();
    assert_eq!(e.last.len(), tuning::RESULTS_KEPT, "only the last few");
    assert_eq!(e.last.last(), Some(&RunEnd::Exit), "newest last");
    assert_eq!(
        s.phonebook[1].last,
        Vec::new(),
        "other numbers are untouched"
    );
    // A save from before kept no results.
    let text = s.to_text().replace("last:", "unused_last:");
    assert!(Save::load(&text).is_ok());
}

#[test]
fn nightcomm_dials_without_wardial() {
    let d = data();
    let mut s = deck(&d);
    assert!(!s.has_tool("wardial"));
    assert!(s.can_wardial());
    assert!(s.wardial(&d, tuning::LOCAL_RANGE).is_ok());
    let mut bare = Save::default();
    bare.ensure_key(1);
    bare.ensure_phonebook(&d, 1);
    bare.tools.clear();
    bare.bank = 1000;
    assert!(!bare.can_wardial());
    assert!(bare.wardial(&d, tuning::LOCAL_RANGE).is_err());
}

#[test]
fn scans_are_mapped_block_by_block() {
    let d = data();
    let mut s = deck(&d);
    let local = d
        .exchanges
        .iter()
        .find(|r| r.id == tuning::LOCAL_RANGE)
        .unwrap();
    let before = nightcomm::block_map(&s, local);
    assert_eq!(before.len(), local.blocks() as usize);
    assert!(before.iter().all(|b| *b == Block::Open));
    let mut found = 0;
    for _ in 0..20 {
        let r = s.wardial(&d, tuning::LOCAL_RANGE).unwrap();
        let worth = r
            .cells
            .iter()
            .filter(|(_, a)| {
                matches!(
                    a,
                    hub::Answer::Carrier { .. } | hub::Answer::Board { .. } | hub::Answer::Trap
                )
            })
            .count();
        assert_eq!(
            nightcomm::finds(&s, &r.range, r.block).count(),
            worth,
            "every carrier, board and trap is kept"
        );
        found += worth;
    }
    assert_eq!(s.finds.len(), found);
    let map = nightcomm::block_map(&s, local);
    assert_eq!(map.iter().filter(|b| **b != Block::Open).count(), 20);
    for (b, state) in map.iter().enumerate() {
        let n = nightcomm::finds(&s, &local.id, b as u32).count();
        match state {
            Block::Open | Block::Dry => assert_eq!(n, 0),
            Block::Watched => assert!(n > 0),
            Block::Hits { found, .. } => assert!(*found > 0 && n >= *found as usize),
        }
    }
    for f in nightcomm::watched(&s) {
        assert_eq!(f.what, Found::Watched);
        assert!(nightcomm::is_watched(&s, &f.number));
    }
    assert!(!nightcomm::is_watched(&s, "555-ZZZZ"));
}

#[test]
fn ranges_nest_and_hide_what_you_dont_know() {
    let d = data();
    let mut s = deck(&d);
    let ids = |s: &Save| -> Vec<(String, usize)> {
        nightcomm::ranges(s, &d)
            .iter()
            .map(|r| (r.def.id.clone(), r.depth))
            .collect()
    };
    assert_eq!(ids(&s), vec![(tuning::LOCAL_RANGE.to_string(), 0)]);
    // The core range learned first still lands under its company's lines once both are known.
    s.learn_range("yamatai_core", "a data vault");
    s.learn_range("yamatai", "a G-file");
    let rows = ids(&s);
    let lines = rows.iter().position(|(id, _)| id == "yamatai").unwrap();
    assert_eq!(
        rows[lines + 1],
        ("yamatai_core".to_string(), lines_depth(&rows, lines) + 1)
    );
    assert!(
        !rows.iter().any(|(id, _)| id == "brightwater"),
        "unknown ranges stay hidden"
    );
    let r = nightcomm::ranges(&s, &d);
    let core = r.iter().find(|r| r.def.id == "yamatai_core").unwrap();
    assert_eq!(
        (core.source, core.scanned, core.blocks),
        ("a data vault", 0, 1)
    );
}

fn lines_depth(rows: &[(String, usize)], at: usize) -> usize {
    rows[at].1
}
