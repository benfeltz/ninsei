//! Monitors and decks: owned hardware decides the CRT look, the themes you can use, and the UX.

use ninsei_core::hardware::Hardware;
use ninsei_core::hub::{self, Save, ShellEffect};
use ninsei_core::netview::MapView;
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

#[test]
fn the_shelf() {
    let d = data();
    let hw = &d.hardware;
    let monitors: Vec<&str> = hw.monitors.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(monitors, ["amber", "green", "apple2", "c64", "vga"]);
    // Every CRT preset is a monitor you can own.
    for p in &d.displays {
        assert!(hw.monitors.iter().any(|m| m.display == p.id), "{}", p.id);
    }
    assert_eq!(hw.starter_monitor().id, "amber");
    assert_eq!(hw.starter_deck().title(), "Ono-Sendai Cyberspace VII");
    assert_eq!(hw.deck("hosaka").unwrap().firmware[0].map, MapView::Draft);
}

#[test]
fn old_saves_get_the_starters() {
    let d = data();
    let mut s =
        Save::load("(version: 1, phosphor: \"plasma\", display: \"c64\", map: Draft)").unwrap();
    assert!(s.ensure_hardware(&d));
    assert_eq!(
        (s.monitors.clone(), s.decks.clone()),
        (vec!["amber".to_string()], vec!["onosendai".to_string()])
    );
    assert_eq!(
        (s.monitor.as_str(), s.deck.as_str()),
        ("amber", "onosendai")
    );
    // The amber tube only shows amber.
    assert_eq!(s.phosphor, "amber");
    assert_eq!(s.firmware_def(&d).map, MapView::Iso);
    assert!(!s.ensure_hardware(&d), "nothing more to fix");
}

#[test]
fn a_mono_tube_locks_the_theme() {
    let d = data();
    let s = fresh(0);
    assert!(s.theme_blocker(&d, "amber").is_none());
    let why = s.theme_blocker(&d, "synthwave").unwrap();
    assert!(why.contains("mono"), "{why}");
    let (lines, fx) = hub::shell("theme synthwave", &s, &d);
    assert_eq!(fx, ShellEffect::None);
    assert!(lines[0].contains("mono"));
}

#[test]
fn a_colour_monitor_shows_any_theme() {
    let d = data();
    let mut s = fresh(1_000);
    s.buy_hardware(&d, &Hardware::Monitor("vga".into()))
        .unwrap();
    assert_eq!(s.monitor, "vga");
    assert_eq!(s.display_id(&d), "vga");
    for t in &d.themes {
        assert!(s.theme_blocker(&d, &t.id).is_none(), "{}", t.id);
    }
    // Back to the amber tube: back to amber.
    s.phosphor = "synthwave".into();
    s.plug_monitor(&d, "amber").unwrap();
    assert_eq!(s.phosphor, "amber");
}

#[test]
fn buying_hardware() {
    let d = data();
    let price = d.hardware.deck("hosaka").unwrap().price;
    let mut broke = fresh(price - 1);
    assert!(
        broke
            .buy_hardware(&d, &Hardware::Deck("hosaka".into()))
            .unwrap_err()
            .contains("bank")
    );
    let mut s = fresh(price);
    s.buy_hardware(&d, &Hardware::Deck("hosaka".into()))
        .unwrap();
    assert_eq!(s.bank, 0);
    assert_eq!((s.deck.as_str(), s.firmware.as_str()), ("hosaka", "4.0"));
    assert!(
        s.buy_hardware(&d, &Hardware::Deck("hosaka".into()))
            .is_err(),
        "once"
    );
    assert!(
        s.buy_hardware(&d, &Hardware::Monitor("nope".into()))
            .is_err()
    );
    // Swapping back and forth.
    s.use_deck(&d, "onosendai").unwrap();
    assert_eq!(s.firmware_def(&d).map, MapView::Iso);
    assert!(s.use_deck(&d, "nope").is_err());
    assert!(s.use_firmware(&d, "9.9").is_err());
}

#[test]
fn only_owned_hardware_plugs_in() {
    let d = data();
    let mut s = fresh(0);
    assert!(s.plug_monitor(&d, "vga").is_err());
    assert!(s.use_deck(&d, "hosaka").is_err());
    let run = |l: &str, s: &Save| hub::shell(l, s, &d).1;
    assert_eq!(run("monitor vga", &s), ShellEffect::None);
    assert_eq!(
        run("monitor amber", &s),
        ShellEffect::Monitor("amber".into())
    );
    assert_eq!(
        run("deck onosendai", &s),
        ShellEffect::Deck("onosendai".into())
    );
    s.monitors.push("vga".into());
    assert_eq!(run("display vga", &s), ShellEffect::Monitor("vga".into()));
}

#[test]
fn hardware_survives_a_save() {
    let d = data();
    let mut s = fresh(5_000);
    s.buy_hardware(&d, &Hardware::Monitor("c64".into()))
        .unwrap();
    s.buy_hardware(&d, &Hardware::Deck("hosaka".into()))
        .unwrap();
    let back = Save::load(&s.to_text()).unwrap();
    assert_eq!(
        (back.monitor.as_str(), back.deck.as_str()),
        ("c64", "hosaka")
    );
    assert_eq!(back.monitors.len(), 2);
}

#[test]
fn bad_hardware_is_caught() {
    let src = include_str!("../../../data/hub/hardware.ron");
    let parse = |hardware: &str| {
        Data::parse(Sources {
            hardware,
            ..Sources::builtin()
        })
    };
    let err = |s: String| parse(&s).unwrap_err();
    assert!(err(src.replace("display: \"vga\"", "display: \"nope\"")).contains("unknown display"));
    assert!(err(src.replacen("starter: true", "starter: false", 1)).contains("starter"));
    assert!(err(src.replace("map: Draft", "map: Blueprint")).contains("Blueprint"));
    assert!(err(src.replace("(id: \"green\",", "(id: \"amber\",")).contains("twice"));
}
