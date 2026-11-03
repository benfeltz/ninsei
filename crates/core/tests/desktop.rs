//! Desktops at home: the tiling desktop's rules, the launcher's search, and add-ons.

use ninsei_core::desktop::{self, AddonEffect, BASE_WORKSPACES, Tiling};
use ninsei_core::hardware::Desktop;
use ninsei_core::hub::{Save, Tool};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

fn tiled() -> Tiling {
    let mut t = Tiling::default();
    t.ensure(BASE_WORKSPACES);
    t
}

#[test]
fn the_decks_have_their_desktops() {
    let d = data();
    let ono = &d.hardware.deck("onosendai").unwrap().firmware[0];
    let hosaka = &d.hardware.deck("hosaka").unwrap().firmware[0];
    assert_eq!(
        (ono.desktop, hosaka.desktop),
        (Desktop::Tiling, Desktop::Workbench)
    );
    // The classic firmware keeps the Workbench for anyone who wants it.
    let classic = &d.hardware.deck("onosendai").unwrap().firmware[1];
    assert_eq!(classic.desktop, Desktop::Workbench);
}

#[test]
fn a_new_save_starts_with_a_shell_tile() {
    let s = Save::load("(version: 1)").unwrap();
    assert_eq!(s.tiling.tiles(), &[Tool::Shell]);
    assert!(!s.tiling_help_seen);
    assert!(s.addons.is_empty());
}

#[test]
fn opening_tiles_and_finding_them_again() {
    let mut t = tiled();
    t.open(Tool::Terminal);
    t.open(Tool::Rig);
    assert_eq!(t.tiles(), &[Tool::Shell, Tool::Terminal, Tool::Rig]);
    assert_eq!(t.focused(), Some(Tool::Rig), "a new tile takes focus");
    // Opening one that's already tiled goes to it instead of tiling it twice.
    t.switch(1);
    t.open(Tool::Terminal);
    assert_eq!((t.current, t.focused()), (0, Some(Tool::Terminal)));
    assert_eq!(t.tiles().len(), 3);
}

#[test]
fn focus_swap_close_and_send() {
    let mut t = tiled();
    t.open(Tool::Terminal);
    t.open(Tool::Rig);
    t.step(1);
    assert_eq!(t.focused(), Some(Tool::Shell), "focus wraps");
    t.step(-1);
    assert_eq!(t.focused(), Some(Tool::Rig));
    t.swap_master();
    assert_eq!(t.tiles()[0], Tool::Rig);
    assert_eq!(t.focus, 0);
    t.send(2);
    assert!(!t.tiles().contains(&Tool::Rig));
    assert_eq!(t.workspaces[2], vec![Tool::Rig]);
    // Focus falls on the new master.
    assert_eq!(t.focused(), Some(Tool::Terminal));
    assert_eq!(t.close(), Some(Tool::Terminal));
    assert_eq!(t.tiles(), &[Tool::Shell]);
    t.close();
    assert!(t.close().is_none(), "nothing left to close");
    assert_eq!(t.focused(), None);
}

#[test]
fn losing_workspaces_keeps_their_tiles() {
    let mut t = tiled();
    t.ensure(5);
    t.switch(4);
    t.open(Tool::Stash);
    t.ensure(3);
    assert_eq!(t.workspaces.len(), 3);
    assert!(t.workspaces[2].contains(&Tool::Stash));
    assert!(t.current < 3);
}

#[test]
fn master_and_stack_layout() {
    let mut t = tiled();
    let area = [0.0, 0.0, 1000.0, 600.0];
    let one = t.layout(area, 10.0);
    assert_eq!(one, vec![(Tool::Shell, [10.0, 10.0, 980.0, 580.0])]);
    t.open(Tool::Terminal);
    t.open(Tool::Rig);
    let three = t.layout(area, 10.0);
    let [mx, my, mw, mh] = three[0].1;
    assert_eq!((mx, my, mh), (10.0, 10.0, 580.0));
    assert!(mw > 500.0 && mw < 600.0, "the master is the wide one");
    // The stack shares the right column, with a gap between, inside the area.
    let (a, b) = (three[1].1, three[2].1);
    assert_eq!(a[0], b[0]);
    assert!((b[1] - (a[1] + a[3]) - 10.0).abs() < 0.01);
    assert!((b[1] + b[3] - 590.0).abs() < 0.01);
    for (_, [x, y, w, h]) in &three {
        assert!(*x >= 0.0 && *y >= 0.0 && x + w <= 1000.0 && y + h <= 600.0);
    }
    t.switch(1);
    assert!(t.layout(area, 10.0).is_empty());
}

#[test]
fn launcher_search() {
    let items: Vec<String> = [
        "CarrierTerm",
        "WarDial",
        "RIG",
        "STASH",
        "Shell",
        "$ phonebook",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    assert_eq!(desktop::search("", &items, false).len(), items.len());
    assert_eq!(desktop::search("ri", &items, false), vec![2]);
    assert_eq!(desktop::search("SH", &items, false), vec![4]);
    assert_eq!(
        desktop::search("phone", &items, false),
        vec![5],
        "a later word counts"
    );
    assert!(
        desktop::search("ctm", &items, false).is_empty(),
        "no fuzzy without the add-on"
    );
    assert_eq!(
        desktop::search("ctm", &items, true),
        vec![0],
        "letters in order"
    );
    // Prefix matches beat loose ones.
    let hits = desktop::search("st", &items, true);
    assert_eq!(hits[0], 3);
}

#[test]
fn add_ons_cost_and_do_things() {
    let d = data();
    let mut s = Save {
        bank: 1_000,
        ..Save::default()
    };
    assert_eq!(s.workspaces(&d), BASE_WORKSPACES);
    assert!(!s.has_effect(&d, Desktop::Tiling, AddonEffect::Fuzzy));
    let price = d
        .addons
        .iter()
        .find(|a| a.id == "workspaces")
        .unwrap()
        .price;
    s.buy_addon(&d, "workspaces").unwrap();
    assert_eq!(s.bank, 1_000 - price);
    assert_eq!(s.workspaces(&d), BASE_WORKSPACES + 2);
    assert!(s.buy_addon(&d, "workspaces").is_err(), "once");
    assert!(s.buy_addon(&d, "nope").is_err());
    s.buy_addon(&d, "fuzzy").unwrap();
    assert!(s.has_effect(&d, Desktop::Tiling, AddonEffect::Fuzzy));
    assert!(
        !s.has_effect(&d, Desktop::Workbench, AddonEffect::Fuzzy),
        "per desktop"
    );
    let mut broke = Save::default();
    assert!(broke.buy_addon(&d, "clock").unwrap_err().contains("bank"));
    // Kept in the save.
    let back = Save::load(&s.to_text()).unwrap();
    assert_eq!(back.addons.len(), 2);
}

#[test]
fn bad_add_ons_are_caught() {
    let src = include_str!("../../../data/hub/desktops.ron");
    let parse = |addons: &str| {
        Data::parse(Sources {
            addons,
            ..Sources::builtin()
        })
    };
    assert!(
        parse(&src.replace("(id: \"fuzzy\",", "(id: \"workspaces\","))
            .unwrap_err()
            .contains("twice")
    );
    assert!(
        parse(&src.replacen("price: 90", "price: 0", 1))
            .unwrap_err()
            .contains("free")
    );
    assert!(parse(&src.replacen("desktop: Tiling", "desktop: Amiga", 1)).is_err());
}

#[test]
fn each_os_has_a_name() {
    assert_eq!(Desktop::Tiling.name(), "AnarchyOS");
    assert_eq!(Desktop::Workbench.name(), "Desktop");
    assert_eq!(Desktop::Liza.name(), "Liza");
}

#[test]
fn the_starter_boots_liza() {
    let d = data();
    let mut s = Save::default();
    s.ensure_hardware(&d);
    assert_eq!(s.deck, "sensenet");
    assert_eq!(s.firmware_def(&d).desktop, Desktop::Liza);
    // Its pictures are 1-bit and rectangular.
    let src = include_str!("../../../data/hub/liza.ron");
    let parse = |liza: &str| {
        Data::parse(data::Sources {
            liza,
            ..data::Sources::builtin()
        })
    };
    assert!(parse(src).is_ok());
    let err =
        parse(&src.replacen("\"......######......\",", "\"......######.....\",", 1)).unwrap_err();
    assert!(err.contains("trash") && err.contains("wide"), "{err}");
    let err =
        parse(&src.replacen("\"......######......\",", "\"......######....x.\",", 1)).unwrap_err();
    assert!(err.contains("only `#` and `.`"), "{err}");
}

#[test]
fn the_shell_completes_its_verbs_and_arguments() {
    use ninsei_core::hub::{self, SHELL_VERBS};
    let d = data();
    let mut s = Save::default();
    s.ensure_hardware(&d);
    s.ensure_phonebook(&d, 7);
    assert_eq!(hub::shell_complete("", &s, &d).len(), SHELL_VERBS.len());
    // Every verb is one the Shell answers (not "Unknown command").
    for v in SHELL_VERBS {
        let (lines, _) = hub::shell(v, &s, &d);
        assert!(
            !lines.iter().any(|l| l.starts_with("Unknown command")),
            "{v}: {lines:?}"
        );
    }
    let dial = hub::shell_complete("dial ", &s, &d);
    assert_eq!(dial.len(), s.phonebook.len());
    assert_eq!(dial.first().map(String::as_str), Some("1"));
    assert!(hub::shell_complete("run ", &s, &d).contains(&"carrierterm".to_string()));
    assert!(hub::shell_complete("theme ", &s, &d).contains(&"amber".to_string()));
    assert_eq!(hub::shell_complete("deck ", &s, &d), s.decks);
    assert_eq!(
        hub::shell_complete("wardial ", &s, &d).len(),
        s.ranges.len()
    );
    assert!(
        hub::shell_complete("dial 1 ", &s, &d).is_empty(),
        "no third word"
    );
    // Old saves load without the history.
    assert!(Save::load("(version: 1)").unwrap().shell_history.is_empty());
}

#[test]
fn the_shell_runs_the_probe() {
    use ninsei_core::hub::{self, ShellEffect};
    let d = data();
    let s = Save::default();
    assert_eq!(
        hub::shell("probe", &s, &d).1,
        ShellEffect::Probe(hub::PROBE_CAREERS)
    );
    assert_eq!(hub::shell("probe 5", &s, &d).1, ShellEffect::Probe(5));
    assert_eq!(hub::shell("probe 9999", &s, &d).1, ShellEffect::Probe(500));
    assert_eq!(hub::shell("probe stop", &s, &d).1, ShellEffect::ProbeStop);
    let (lines, effect) = hub::shell("probe lots", &s, &d);
    assert_eq!(effect, ShellEffect::None);
    assert!(lines[0].contains("careers"));
}
