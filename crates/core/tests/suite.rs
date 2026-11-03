//! The Hosaka's productivity suite and buyable panes: spreadsheet rules, formulas, report axes,
//! work orders and the dialogs, plus the pane catalog and what each firmware bundles.

use ninsei_core::hardware::Chrome;
use ninsei_core::history::TurnRow;
use ninsei_core::hub::Save;
use ninsei_core::panes::PaneDef;
use ninsei_core::suite::{self, SheetCol, Watch};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

#[test]
fn spreadsheet_columns_are_lettered() {
    let names: Vec<String> = [0, 1, 25, 26, 27, 51, 52, 701]
        .into_iter()
        .map(suite::col_name)
        .collect();
    assert_eq!(names, ["A", "B", "Z", "AA", "AB", "AZ", "BA", "ZZ"]);
}

#[test]
fn the_hosaka_runs_the_suite_and_the_ono_sendai_bundles_net() {
    let d = data();
    let hosaka = &d.hardware.deck("hosaka").unwrap().firmware[0];
    let ono = &d.hardware.deck("onosendai").unwrap().firmware[0];
    assert_eq!((hosaka.chrome, ono.chrome), (Chrome::Suite, Chrome::Btop));
    assert_eq!(ono.panes, ["net"]);
    assert!(hosaka.panes.is_empty());
}

#[test]
fn panes_come_with_firmware_or_are_bought() {
    let d = data();
    let mut s = Save::default();
    s.ensure_hardware(&d);
    // The starter Ono-Sendai 7.0 ships with the net pane.
    assert!(s.has_pane(&d, "net"));
    s.decks.push("hosaka".into());
    s.use_deck(&d, "hosaka").unwrap();
    assert!(!s.has_pane(&d, "net"), "the Hosaka doesn't bundle it");
    let price = d.panes.iter().find(|p| p.id == "net").unwrap().price;
    assert!(s.buy_pane(&d, "net").is_err(), "no credits yet");
    s.bank = price + 5;
    s.buy_pane(&d, "net").unwrap();
    assert_eq!(s.bank, 5);
    assert!(s.has_pane(&d, "net"));
    assert!(s.buy_pane(&d, "net").is_err(), "only once");
    assert!(s.buy_pane(&d, "nope").is_err());
    // Old saves load without the field.
    assert!(Save::load("(version: 1)").unwrap().panes.is_empty());
}

#[test]
fn bad_panes_are_caught() {
    let pane = |id: &str, price| PaneDef {
        id: id.into(),
        name: id.into(),
        price,
        blurb: String::new(),
    };
    assert!(PaneDef::validate(&[pane("a", 10), pane("b", 10)]).is_ok());
    assert!(PaneDef::validate(&[pane("a", 10), pane("a", 20)]).is_err());
    assert!(PaneDef::validate(&[pane("a", 0)]).is_err());
}

#[test]
fn the_sheet_sorts_and_filters() {
    let mut g = Game::new(data(), 7);
    assert!(g.hdd.len() >= 3);
    g.hdd[1].loaded = false;
    let all = suite::sheet_rows(&g.hdd, SheetCol::Slot, true, false);
    assert_eq!(all, (0..g.hdd.len()).collect::<Vec<_>>());
    let down = suite::sheet_rows(&g.hdd, SheetCol::Slot, false, false);
    assert_eq!(down.first(), Some(&(g.hdd.len() - 1)));
    let loaded = suite::sheet_rows(&g.hdd, SheetCol::Slot, true, true);
    assert!(!loaded.contains(&1) && loaded.iter().all(|&i| g.hdd[i].loaded));
    let by_ram = suite::sheet_rows(&g.hdd, SheetCol::Ram, true, false);
    assert!(
        by_ram
            .windows(2)
            .all(|w| g.hdd[w[0]].spec.ram <= g.hdd[w[1]].spec.ram)
    );
    let by_status = suite::sheet_rows(&g.hdd, SheetCol::Status, true, false);
    assert!(
        !g.hdd[*by_status.last().unwrap()].loaded,
        "loaded rows first"
    );
    assert_eq!(SheetCol::Status.cell(1, &g.hdd[1]), "STORED");
}

#[test]
fn programs_read_as_formulas() {
    let g = Game::new(data(), 7);
    for s in &g.hdd {
        let f = suite::formula(&s.spec);
        assert!(f.starts_with('=') && f.ends_with(')'), "{f}");
    }
    let mut spec = g.hdd[0].spec.clone();
    spec.effect = Effect::Break {
        power: 6,
        vs: Some(IceClass::CodeGate),
    };
    assert_eq!(suite::formula(&spec), "=BREAK(CodeGate, 6)");
}

#[test]
fn report_axes_and_strength_per_turn() {
    assert_eq!(suite::ticks(100, 4), [0, 25, 50, 75, 100]);
    assert_eq!(suite::ticks(6, 3), [0, 2, 4, 6]);
    let row = |turn, dealt| TurnRow {
        turn,
        dealt,
        ..TurnRow::default()
    };
    assert_eq!(
        suite::strength_by_turn(10, &[row(1, 3), row(2, 0), row(3, 9)]),
        [7, 7, 0]
    );
}

#[test]
fn the_status_line_is_fixed_for_a_run() {
    assert_eq!(
        suite::doc_name(Some("Brightwater Mutual")),
        "BRIGHTWATER MUTUAL.NET"
    );
    assert_eq!(suite::doc_name(None), "LOCAL.NET");
    assert_eq!(suite::user_id(12_340_451), "GUEST-0451");
    for seed in [0, 1, 99, u64::MAX] {
        let d = suite::date(seed);
        assert_eq!(d, suite::date(seed));
        let parts: Vec<u32> = d.split('-').map(|p| p.parse().unwrap()).collect();
        assert!((1..=12).contains(&parts[0]) && (1..=28).contains(&parts[1]));
        assert!((90..=95).contains(&parts[2]));
    }
    assert_eq!(suite::log_ref(LogKind::Bad), "ERR");
}

fn contract(status: ContractStatus, reward: u32, ghost: Option<(u32, u32)>) -> Contract {
    Contract {
        client: "Maas".into(),
        brief: String::new(),
        node: 1,
        reward,
        ghost,
        status,
        paid: 0,
        betrayal: None,
    }
}

#[test]
fn work_orders_total_what_is_outstanding() {
    let ks = [
        contract(ContractStatus::Active, 100, Some((40, 57))),
        contract(ContractStatus::Active, 50, None),
        contract(ContractStatus::Done, 999, None),
        contract(ContractStatus::Failed, 999, None),
    ];
    assert_eq!(suite::outstanding(&ks), 207);
    assert_eq!(
        suite::ghost_cell(&ks[0], 37).as_deref(),
        Some("+57¢ if TRACE ≤ 40%")
    );
    assert!(suite::ghost_cell(&ks[0], 41).unwrap().contains("VOID"));
    assert!(suite::ghost_cell(&ks[1], 0).is_none());
}

#[test]
fn dialogs_pop_for_pay_and_exits() {
    let mut g = Game::new(data(), 9);
    let mut w = Watch::default();
    assert!(w.step(&g).is_empty(), "the first look is quiet");
    g.contracts.push(contract(ContractStatus::Active, 80, None));
    assert!(w.step(&g).is_empty());
    g.contracts[0].status = ContractStatus::Done;
    let n = w.step(&g);
    assert_eq!(n.len(), 1);
    assert_eq!(n[0].title, "WORK ORDER COMPLETE");
    assert!(w.step(&g).is_empty(), "only once");
    g.phase = Phase::JackedOut;
    assert_eq!(w.step(&g)[0].title, "SESSION ENDED");
    // A new run starts quiet, even if it begins somewhere odd.
    let mut g2 = Game::new(data(), 10);
    assert!(!w.watching(&g2));
    g2.phase = Phase::JackedOut;
    assert!(w.step(&g2).is_empty());
    assert!(w.watching(&g2));
    // A flatline has the crash screen, not a dialog.
    let mut g3 = Game::new(data(), 11);
    w.step(&g3);
    g3.phase = Phase::Flatlined;
    assert!(w.step(&g3).is_empty());
}
