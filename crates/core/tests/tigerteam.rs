//! Tiger Team Pro: network profiles, findings, exposure and the engagements table.

use ninsei_core::hub::{self, Save, Severity};
use ninsei_core::tigerteam::{self, Exposure, Order, Page};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

fn deck(d: &Data) -> Save {
    let mut s = Save::default();
    s.ensure_key(0x7167);
    s.ensure_phonebook(d, 1);
    s.add_starters(d);
    s.learn_tool("tigerteam");
    s
}

/// A phonebook entry of `tier` with `traits`.
fn entry(s: &mut Save, tier: u32, traits: &[&str]) -> u32 {
    let e = s.phonebook[0].clone();
    let id = s.next_entry;
    s.next_entry += 1;
    s.phonebook.push(hub::Entry {
        id,
        number: format!("555-8{id:03}"),
        tier,
        traits: traits.iter().map(|t| t.to_string()).collect(),
        heat: 0,
        runs: 0,
        last: Vec::new(),
        ..e
    });
    id
}

#[test]
fn every_trait_has_a_finding() {
    let d = data();
    for t in &d.networks.traits {
        assert!(!t.finding.is_empty(), "{}", t.id);
        assert!(t.finding.len() <= 60, "{} reads long in the report", t.id);
    }
}

#[test]
fn a_profile_reads_the_network() {
    let d = data();
    let mut s = deck(&d);
    let id = entry(&mut s, 2, &["graveyard", "black_ice", "no_bbs"]);
    let e = s.entry(id).unwrap();
    let p = tigerteam::profile(&d, e);
    let tier = d.networks.tier(2).unwrap();
    assert_eq!(p.tier, tier.label);
    assert_eq!(p.layers, tier.mid_layers + 2, "entry and exit count");
    assert_eq!(p.board, None, "no BBS on this one");
    let (_, m) = Save::site_params(&d, e);
    assert_eq!(
        (p.ice_bonus, p.trace_pct, p.pay_pct),
        (m.ice_bonus, m.trace_pct, m.pay_pct)
    );
    // Worst first, numbered in that order.
    let sev: Vec<Severity> = p.findings.iter().map(|f| f.severity).collect();
    assert_eq!(sev, vec![Severity::High, Severity::Low, Severity::Info]);
    assert_eq!(p.findings[0].id, "F-01");
    assert!(p.findings[0].text.contains("black ICE"));
    let plain = entry(&mut s, 2, &[]);
    assert!(
        tigerteam::profile(&d, s.entry(plain).unwrap())
            .board
            .is_some()
    );
}

#[test]
fn exposure_climbs_with_tier_and_heat() {
    let d = data();
    let mut s = deck(&d);
    let lan = entry(&mut s, 1, &[]);
    let mega = entry(&mut s, 3, &["black_ice", "sentry_heavy"]);
    let rate = |s: &Save, id| tigerteam::profile(&d, s.entry(id).unwrap()).exposure;
    assert_eq!(rate(&s, lan), Exposure::Low);
    assert_eq!(rate(&s, mega), Exposure::High);
    s.phonebook.iter_mut().find(|e| e.id == lan).unwrap().heat = tuning::HEAT_MAX;
    assert!(
        rate(&s, lan) > Exposure::Low,
        "a hot number hits back harder"
    );
}

#[test]
fn engagements_sort_and_targets_rank_by_exposure() {
    let d = data();
    let mut s = deck(&d);
    entry(&mut s, 3, &["black_ice"]);
    entry(&mut s, 1, &["skint"]);
    let pay: Vec<u32> = tigerteam::engagements(&s, &d, Order::Value)
        .iter()
        .map(|p| p.pay_pct)
        .collect();
    assert!(pay.windows(2).all(|w| w[0] >= w[1]), "{pay:?}");
    let names: Vec<String> = tigerteam::engagements(&s, &d, Order::Name)
        .iter()
        .map(|p| p.entry.name.to_lowercase())
        .collect();
    assert!(names.windows(2).all(|w| w[0] <= w[1]));
    let t = tigerteam::targets(&s, &d);
    assert_eq!(t.len(), s.phonebook.len());
    assert!(t.windows(2).all(|w| w[0].exposure >= w[1].exposure));
    let mut o = Order::default();
    for _ in 0..Order::ALL.len() {
        o = o.next();
    }
    assert_eq!(o, Order::default());
}

#[test]
fn sections_and_the_pages_they_share() {
    assert_eq!(Page::from_key('1'), Some(Page::Assessment));
    assert_eq!(Page::from_key('5'), Some(Page::Console));
    assert_eq!(Page::from_key('6'), None);
    assert_eq!(Page::Recon.shared(), Some(nightcomm::Tab::Wardial));
    assert_eq!(Page::Address.shared(), Some(nightcomm::Tab::Ranges));
    assert_eq!(Page::Console.shared(), Some(nightcomm::Tab::Term));
    assert_eq!(Page::Assessment.shared(), None);
}

#[test]
fn it_does_everything_nightcomm_does() {
    let d = data();
    let mut s = deck(&d);
    assert!(!s.has_tool("nightcomm"));
    assert!(s.has_dialer() && s.can_wardial());
    s.bank = 1000;
    assert!(s.wardial(&d, tuning::LOCAL_RANGE).is_ok());
    assert!(s.atdt(&d, "555-0042").is_ok());
    let (lines, _) = hub::shell("phonebook", &s, &d);
    let school = lines.iter().find(|l| l.contains("555-0420")).unwrap();
    assert!(school.contains("T1") && school.contains("heat"), "{school}");
    assert!(school.contains("graveyard shift"), "traits show: {school}");
}

#[test]
fn corplink_sells_it() {
    let d = data();
    let b = &d.boards[d.board("corplink").unwrap()];
    assert!(b.areas.iter().any(|a| {
        a.always
            && a.files
                .iter()
                .any(|f| f.item == data::Drop::Tool("tigerteam".into()))
    }));
}
