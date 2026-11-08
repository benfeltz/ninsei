//! System: About This Deck reports the rules' numbers, and only the software you own.

use ninsei_core::hub::{Save, ShellEffect, Tool, Upgrade};
use ninsei_core::system::{self, Panel};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

fn fresh(bank: u32) -> Save {
    let mut s = Save {
        bank,
        ..Save::default()
    };
    s.ensure_hardware(&data());
    s
}

fn spec<'a>(o: &'a system::Overview, label: &str) -> &'a system::Spec {
    o.specs.iter().find(|s| s.label == label).unwrap()
}

#[test]
fn the_overview_reports_the_deck_and_its_specs() {
    let d = data();
    let s = fresh(0);
    let o = system::overview(&s, &d);
    let fw = s.firmware_def(&d);
    assert_eq!(o.deck, s.deck_def(&d).title());
    assert_eq!(o.os, fw.desktop.name());
    assert_eq!(o.firmware, fw.label);
    let labels: Vec<&str> = o.specs.iter().map(|s| s.label.as_str()).collect();
    assert_eq!(labels, ["CPU", "RAM", "HDD", "Integrity", "Stash"]);
    let cpu = spec(&o, "CPU");
    assert!(
        cpu.value
            .starts_with(&s.stat_base(&d, Upgrade::Cpu).to_string())
    );
    let max = d.rig.iter().find(|u| u.kind == Upgrade::Cpu).unwrap().max;
    assert_eq!(cpu.level, (0, max));
    assert!(cpu.next.is_some(), "a fresh deck has upgrades left");
}

#[test]
fn an_upgrade_shows_up_in_the_overview() {
    let d = data();
    let mut s = fresh(100_000);
    let before = system::overview(&s, &d);
    s.buy(&d.rig, Upgrade::Ram).unwrap();
    let after = system::overview(&s, &d);
    assert_eq!(
        spec(&after, "RAM").level.0,
        spec(&before, "RAM").level.0 + 1
    );
    assert_ne!(spec(&after, "RAM").value, spec(&before, "RAM").value);
    // Maxed: no next upgrade.
    while s.buy(&d.rig, Upgrade::Cpu).is_ok() {}
    assert_eq!(spec(&system::overview(&s, &d), "CPU").next, None);
}

#[test]
fn ram_and_hdd_say_what_the_loadout_uses() {
    let d = data();
    let s = fresh(0);
    let o = system::overview(&s, &d);
    let filled = s.kit(&d).programs.len();
    assert!(
        spec(&o, "HDD")
            .use_note
            .starts_with(&format!("{filled} of"))
    );
    assert!(spec(&o, "RAM").use_note.contains("free"));
}

#[test]
fn installed_software_is_only_what_you_own_with_versions() {
    let d = data();
    let mut s = fresh(0);
    s.tools.clear();
    assert!(system::overview(&s, &d).software.is_empty());
    s.tools.push("hexed".into());
    let o = system::overview(&s, &d);
    let hexed = d.tools.iter().find(|t| t.id == "hexed").unwrap();
    assert_eq!(
        o.software,
        vec![(hexed.name.clone(), hexed.version.clone())]
    );
}

#[test]
fn about_this_deck_lists_the_games_you_own_after_the_tools() {
    use hub::GameCopy::{Boxed, Cracked};
    let d = data();
    let mut s = fresh(0);
    s.tools = vec!["hexed".into()];
    s.learn_game("minefield", Cracked);
    s.learn_game("blox", Boxed);
    s.learn_game("blox", Cracked);
    let o = system::overview(&s, &d);
    let ver = |id: &str| d.game(id).unwrap().version.clone();
    let hexed = d.tools.iter().find(|t| t.id == "hexed").unwrap();
    assert_eq!(
        o.software,
        vec![
            (hexed.name.clone(), hexed.version.clone()),
            ("Blox".to_string(), ver("blox")),
            ("BLOX [PHS]".to_string(), ver("blox")),
            ("MINEFIELD [PHS]".to_string(), ver("minefield")),
        ],
        "each copy by its label, games in games.ron's order, after the tools"
    );
    assert!(
        !o.software.iter().any(|(n, _)| n.contains("ibbler")),
        "only what you own"
    );
}

#[test]
fn system_opens_by_its_name_and_by_prefs() {
    let d = data();
    let s = fresh(0);
    for line in ["open system", "run System", "open prefs"] {
        assert_eq!(
            hub::shell(line, &s, &d).1,
            ShellEffect::Open(Tool::System),
            "{line}"
        );
    }
    assert_eq!(Panel::default(), Panel::Overview);
}

#[test]
fn every_desktop_has_a_logo_that_fits() {
    use ninsei_core::hardware::Desktop;
    let d = data();
    for desk in [Desktop::Tiling, Desktop::Workbench, Desktop::Liza] {
        let logo = system::logo(&d, desk);
        assert!(!logo.is_empty(), "{desk:?}");
        assert!(logo.len() <= system::LOGO_MAX.1);
        assert!(logo.iter().all(|l| l.chars().count() <= system::LOGO_MAX.0));
    }
    // Too wide, or missing a desktop, is caught.
    let mut bad = d.fetch.clone();
    bad[0].logo[0] = "x".repeat(system::LOGO_MAX.0 + 1);
    assert!(system::FetchDef::validate(&bad).is_err());
    bad.remove(0);
    assert!(system::FetchDef::validate(&bad).is_err());
}

#[test]
fn every_panel_has_one_square_one_bit_icon() {
    let d = data();
    assert!(system::PanelIconDef::validate(&d.panel_icons).is_ok());
    for p in Panel::ALL {
        assert_eq!(system::panel_icon(&d, p).len(), system::PANEL_ICON, "{p:?}");
    }
    let mut missing = d.panel_icons.clone();
    missing.retain(|i| i.panel != Panel::Disk);
    assert!(
        system::PanelIconDef::validate(&missing).is_err(),
        "a panel with no icon"
    );
    let mut wide = d.panel_icons.clone();
    wide[0].pixels[3].push('k');
    assert!(
        system::PanelIconDef::validate(&wide).is_err(),
        "a row too wide"
    );
    let mut colour = d.panel_icons.clone();
    colour[0].pixels[3] = colour[0].pixels[3].replacen('k', "o", 1);
    assert!(
        system::PanelIconDef::validate(&colour).is_err(),
        "a colour pixel"
    );
}
