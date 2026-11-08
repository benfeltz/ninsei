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
    assert_eq!(
        monitors,
        ["white", "amber", "green", "apple2", "c64", "vga"]
    );
    // Every CRT preset is a monitor you can own.
    for p in &d.displays {
        assert!(hw.monitors.iter().any(|m| m.display == p.id), "{}", p.id);
    }
    assert_eq!(hw.starter_monitor().id, "white");
    assert_eq!(hw.starter_deck().title(), "Sense/Net Personal Deck");
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
        (vec!["white".to_string()], vec!["sensenet".to_string()])
    );
    assert_eq!((s.monitor.as_str(), s.deck.as_str()), ("white", "sensenet"));
    // The white tube only shows paper white.
    assert_eq!(s.phosphor, "paperwhite");
    assert_eq!(s.firmware_def(&d).map, MapView::Paint);
    assert!(!s.ensure_hardware(&d), "nothing more to fix");
}

#[test]
fn a_mono_tube_locks_the_theme() {
    let d = data();
    let s = fresh(0);
    assert!(s.theme_blocker(&d, "paperwhite").is_none());
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
    assert_eq!(s.monitor, "white", "buying a monitor doesn't plug it in");
    assert!(s.monitors.iter().any(|m| m == "vga"), "it's yours");
    s.plug_monitor(&d, "vga").unwrap();
    assert_eq!(s.monitor, "vga");
    assert_eq!(s.display_id(&d), "vga");
    for t in &d.themes {
        assert!(s.theme_blocker(&d, &t.id).is_none(), "{}", t.id);
    }
    // Back to the white tube: back to paper white.
    s.phosphor = "synthwave".into();
    s.plug_monitor(&d, "white").unwrap();
    assert_eq!(s.phosphor, "paperwhite");
}

#[test]
fn buying_hardware() {
    let d = data();
    let price = d.hardware.deck("onosendai").unwrap().price;
    let mut broke = fresh(price - 1);
    assert!(
        broke
            .buy_hardware(&d, &Hardware::Deck("onosendai".into()))
            .unwrap_err()
            .contains("bank")
    );
    let mut s = fresh(price);
    s.buy_hardware(&d, &Hardware::Deck("onosendai".into()))
        .unwrap();
    assert_eq!(s.bank, 0);
    assert!(s.decks.iter().any(|k| k == "onosendai"), "it's yours");
    assert_eq!(s.deck, "sensenet", "buying a deck doesn't switch to it");
    s.use_deck(&d, "onosendai").unwrap();
    assert_eq!((s.deck.as_str(), s.firmware.as_str()), ("onosendai", "7.0"));
    assert!(
        s.buy_hardware(&d, &Hardware::Deck("onosendai".into()))
            .is_err(),
        "once"
    );
    assert!(
        s.buy_hardware(&d, &Hardware::Monitor("nope".into()))
            .is_err()
    );
    // Swapping back and forth.
    s.use_deck(&d, "sensenet").unwrap();
    assert_eq!(s.firmware_def(&d).map, MapView::Paint);
    assert!(s.use_deck(&d, "nope").is_err());
    assert!(s.use_firmware(&d, "9.9").is_err());
}

#[test]
fn only_owned_hardware_plugs_in() {
    let d = data();
    let mut s = fresh(0);
    assert!(s.plug_monitor(&d, "vga").is_err());
    assert!(s.use_deck(&d, "onosendai").is_err());
    let run = |l: &str, s: &Save| hub::shell(l, s, &d).1;
    assert_eq!(run("monitor vga", &s), ShellEffect::None);
    assert_eq!(
        run("monitor white", &s),
        ShellEffect::Monitor("white".into())
    );
    assert_eq!(
        run("deck sensenet", &s),
        ShellEffect::Deck("sensenet".into())
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
    s.plug_monitor(&d, "c64").unwrap();
    s.buy_hardware(&d, &Hardware::Deck("onosendai".into()))
        .unwrap();
    s.use_deck(&d, "onosendai").unwrap();
    let back = Save::load(&s.to_text()).unwrap();
    assert_eq!(
        (back.monitor.as_str(), back.deck.as_str()),
        ("c64", "onosendai")
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

#[test]
fn stats_are_read() {
    let d = data();
    let cpu = d.hardware.deck("sensenet").unwrap().stat(hub::Upgrade::Cpu);
    assert_eq!(cpu.map(|s| (s.base, s.max)), Some((3, 6)));
    assert!(
        d.hardware
            .decks
            .iter()
            .all(|deck| deck.stat(hub::Upgrade::Stash).is_none())
    );
}

#[test]
fn bad_stats_are_caught() {
    let src = include_str!("../../../data/hub/hardware.ron");
    let parse = |hardware: &str| {
        Data::parse(Sources {
            hardware,
            ..Sources::builtin()
        })
    };
    let err = |s: String| parse(&s).unwrap_err();
    let cpu = "(line: Cpu, base: 3, max: 6)";
    assert!(err(src.replacen(cpu, "(line: Ram, base: 3, max: 6)", 1)).contains("twice"));
    assert!(err(src.replacen(cpu, "(line: Cpu, base: 7, max: 6)", 1)).contains("tops out"));
    assert!(err(src.replacen(cpu, "(line: Stash, base: 2, max: 6)", 1)).contains("home storage"));
    let missing = src.replacen(&format!("{cpu}, "), "", 1);
    assert!(err(missing).contains("disagree"));
}

#[test]
fn the_dev_key_fills_the_bank() {
    let mut s = fresh(5);
    let msg = s.dev_credits();
    assert_eq!(s.bank, 5 + tuning::DEV_CREDITS);
    assert!(msg.contains("dev"));
}

#[test]
fn layouts_are_kept_per_deck_firmware() {
    let d = data();
    let mut s = fresh(5_000);
    assert_eq!(
        s.layout(&d),
        None,
        "no layout yet: the app uses the default"
    );
    assert!(s.set_layout(&d, "hosaka layout".into()));
    assert!(!s.set_layout(&d, "hosaka layout".into()), "unchanged");
    assert_eq!(s.layout(&d), Some("hosaka layout"));
    // Another deck has its own slot.
    s.buy_hardware(&d, &Hardware::Deck("onosendai".into()))
        .unwrap();
    assert_eq!(s.layout(&d), Some("hosaka layout"), "still on the old deck");
    s.use_deck(&d, "onosendai").unwrap();
    assert_eq!(s.layout(&d), None);
    s.set_layout(&d, "ono layout".into());
    s.use_deck(&d, "sensenet").unwrap();
    assert_eq!(s.layout(&d), Some("hosaka layout"));
    // Kept in the save, and reset clears only this one.
    let mut back = Save::load(&s.to_text()).unwrap();
    assert_eq!(back.layouts.len(), 2);
    back.clear_layout(&d);
    assert_eq!(back.layout(&d), None);
    assert_eq!(back.layouts.len(), 1);
    // Old saves have none.
    assert!(Save::load("(version: 1)").unwrap().layouts.is_empty());
}
