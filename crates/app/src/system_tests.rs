//! System, driven headlessly: clicks and keys on the real panels, checked against the
//! `WbOut`s they send.

use super::*;
use crate::uitest::Harness;
use ninsei_core::glitch::Glitches;

fn data() -> Data {
    Data::builtin().unwrap()
}

/// A save on the starter deck that owns every monitor and every tool.
fn save(data: &Data) -> Save {
    let mut s = Save {
        monitors: data
            .hardware
            .monitors
            .iter()
            .map(|m| m.id.clone())
            .collect(),
        tools: ninsei_core::data::TOOLS
            .iter()
            .map(|t| t.to_string())
            .collect(),
        ..Save::default()
    };
    s.ensure_hardware(data);
    s
}

/// Runs one frame of System on `panel` with `events`; returns what it sent.
fn frame(
    h: &mut Harness,
    data: &Data,
    save: &Save,
    panel: &mut Panel,
    events: Vec<egui::Event>,
) -> Vec<WbOut> {
    let mut out = Vec::new();
    h.frame(events, |ui| show(ui, data, save, panel, &mut out));
    out
}

/// A click on the widget that says `text`.
fn press(h: &Harness, text: &str) -> Vec<egui::Event> {
    let r = h
        .find(text)
        .unwrap_or_else(|| panic!("no {text:?}: {:?}", h.texts()));
    crate::uitest::click(r.center())
}

/// Opens `panel` and clicks the widget labelled `label` on it.
fn click(data: &Data, save: &Save, panel: Panel, label: &str) -> Vec<WbOut> {
    let mut h = Harness::new(900.0, 700.0);
    let mut p = panel;
    frame(&mut h, data, save, &mut p, Vec::new());
    let ev = press(&h, label);
    frame(&mut h, data, save, &mut p, ev)
}

#[test]
fn the_panel_list_switches_panels() {
    let (d, s) = (data(), save(&data()));
    let mut h = Harness::new(900.0, 700.0);
    let mut p = Panel::Overview;
    frame(&mut h, &d, &s, &mut p, Vec::new());
    for want in [Panel::Monitor, Panel::Theme, Panel::Overview] {
        let ev = press(&h, want.label());
        let out = frame(&mut h, &d, &s, &mut p, ev);
        assert_eq!(p, want);
        assert!(out.is_empty(), "switching panels changes nothing: {out:?}");
    }
}

#[test]
fn about_this_deck_shows_the_deck_and_only_owned_software() {
    let d = data();
    let mut s = save(&d);
    s.tools = vec!["wardial".into()];
    let mut h = Harness::new(900.0, 700.0);
    let mut p = Panel::Overview;
    frame(&mut h, &d, &s, &mut p, Vec::new());
    assert!(h.find(&s.deck_def(&d).title()).is_some(), "{:?}", h.texts());
    let wardial = d.tools.iter().find(|t| t.id == "wardial").unwrap();
    assert!(
        h.find(&format!("{} {}", wardial.name, wardial.version))
            .is_some()
    );
    for t in d.tools.iter().filter(|t| t.id != "wardial") {
        assert!(
            h.find(&format!("{} {}", t.name, t.version)).is_none(),
            "{} isn't owned",
            t.name
        );
    }
}

#[test]
fn the_switches_send_their_changes() {
    let d = data();
    let s = save(&d);
    assert_eq!(click(&d, &s, Panel::Overview, "Sound"), [WbOut::Sound]);
    assert_eq!(
        click(&d, &s, Panel::Overview, "Cracktros"),
        [WbOut::Intros(!s.intros)]
    );
    for g in Glitches::ALL.into_iter().filter(|&g| g != s.glitches) {
        assert_eq!(
            click(&d, &s, Panel::Overview, g.label()),
            [WbOut::Glitches(g)]
        );
    }
}

#[test]
fn the_monitor_panel_plugs_in_a_monitor_and_switches_the_crt() {
    let d = data();
    let s = save(&d);
    let other = d
        .hardware
        .monitors
        .iter()
        .find(|m| m.id != s.monitor)
        .unwrap();
    assert_eq!(
        click(&d, &s, Panel::Monitor, &other.title()),
        [WbOut::Monitor(other.id.clone())]
    );
    assert_eq!(click(&d, &s, Panel::Monitor, "CRT effects"), [WbOut::Crt]);
}

#[test]
fn a_mono_tube_locks_the_theme_panel() {
    let d = data();
    let mut s = save(&d);
    let colour = d.hardware.monitors.iter().find(|m| m.colour).unwrap();
    s.monitor = colour.id.clone();
    let other = d.themes.iter().find(|t| t.id != s.phosphor).unwrap();
    let label = other.label.clone();
    assert_eq!(
        click(&d, &s, Panel::Theme, &label),
        [WbOut::Theme(other.id.clone())]
    );

    let mono = d.hardware.monitors.iter().find(|m| !m.colour).unwrap();
    s.monitor = mono.id.clone();
    s.phosphor = mono.locked_theme(&d).unwrap().to_string();
    let other = d.themes.iter().find(|t| t.id != s.phosphor).unwrap();
    let label = other.label.clone();
    assert!(
        click(&d, &s, Panel::Theme, &label).is_empty(),
        "a mono tube shows its own theme only"
    );
}

#[test]
fn the_shell_opens_system_by_its_old_name_too() {
    let d = data();
    let s = save(&d);
    for line in ["open system", "open prefs"] {
        let mut home = crate::home::Home::new(Vec::new());
        home.take_opens();
        home.run(line, &d, &s, &mut Vec::new());
        assert_eq!(
            home.take_opens(),
            [ninsei_core::hub::Tool::System],
            "{line}"
        );
    }
}
