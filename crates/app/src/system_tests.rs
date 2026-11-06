//! System, driven headlessly: clicks and keys on the real panels, checked against the
//! `WbOut`s they send.

use super::*;
use crate::uitest::Harness;
use egui::Key;
use ninsei_core::binary;
use ninsei_core::glitch::Glitches;
use ninsei_core::hub::StashItem;

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

/// Runs a frame of System with `events`, then one to settle; returns what it sent.
fn frame(
    h: &mut Harness,
    data: &Data,
    save: &Save,
    st: &mut SystemUi,
    events: Vec<egui::Event>,
) -> Vec<WbOut> {
    let mut out = Vec::new();
    h.frame(events, |ui| show(ui, data, save, st, &mut out));
    // One more frame so what the input changed is on screen.
    h.frame(Vec::new(), |ui| show(ui, data, save, st, &mut out));
    out
}

/// A key pressed with the pointer at `at`: Disk's keys work while the pointer is over it.
fn key_at(k: Key, at: egui::Pos2) -> Vec<egui::Event> {
    let mut ev = vec![egui::Event::PointerMoved(at)];
    ev.extend(crate::uitest::key(k));
    ev
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
    let mut p = SystemUi {
        panel,
        ..SystemUi::default()
    };
    frame(&mut h, data, save, &mut p, Vec::new());
    let ev = press(&h, label);
    frame(&mut h, data, save, &mut p, ev)
}

#[test]
fn the_panel_list_switches_panels() {
    let (d, s) = (data(), save(&data()));
    let mut h = Harness::new(900.0, 700.0);
    let mut p = SystemUi::default();
    frame(&mut h, &d, &s, &mut p, Vec::new());
    for want in [Panel::Monitor, Panel::Theme, Panel::Disk, Panel::Overview] {
        let ev = press(&h, want.label());
        let out = frame(&mut h, &d, &s, &mut p, ev);
        assert_eq!(p.panel, want);
        assert!(out.is_empty(), "switching panels changes nothing: {out:?}");
    }
}

#[test]
fn about_this_deck_shows_the_deck_and_only_owned_software() {
    let d = data();
    let mut s = save(&d);
    s.tools = vec!["wardial".into()];
    let mut h = Harness::new(900.0, 700.0);
    let mut p = SystemUi::default();
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

/// Adds a factory `warez` to the stash as `id`, sealed with the save's key.
fn stash(d: &Data, s: &mut Save, warez: &str, id: u32) {
    let w = d.warez_ix(warez).unwrap();
    let bin = binary::encode(&d.warez[w], s.key);
    s.stash.push(StashItem {
        id,
        origin: warez.into(),
        hex: binary::to_hex(&bin),
    });
}

/// A save with a torch in the box, and System open on Disk.
fn on_disk() -> (Data, Save, Harness, SystemUi) {
    let d = data();
    let mut s = save(&d);
    stash(&d, &mut s, "torch", 7);
    let mut h = Harness::new(900.0, 700.0);
    let mut st = SystemUi {
        panel: Panel::Disk,
        ..SystemUi::default()
    };
    frame(&mut h, &d, &s, &mut st, Vec::new());
    (d, s, h, st)
}

fn file_of(d: &Data, s: &Save, at: &Disk) -> String {
    disk::installed(s, d)
        .iter()
        .chain(&disk::shelf(s, d))
        .find(|p| &p.at == at)
        .unwrap()
        .spec
        .file()
}

#[test]
fn a_program_in_the_box_is_picked_then_installed() {
    let (d, s, mut h, mut st) = on_disk();
    let torch = Disk::Stash(7);
    let ev = press(&h, &file_of(&d, &s, &torch));
    assert!(frame(&mut h, &d, &s, &mut st, ev).is_empty());
    assert_eq!(st.picked, Some(torch.clone()));
    let ev = press(&h, "Install ▸");
    assert_eq!(frame(&mut h, &d, &s, &mut st, ev), [WbOut::Install(torch)]);
}

#[test]
fn clicking_a_block_on_the_map_picks_its_program() {
    let (d, s, mut h, mut st) = on_disk();
    let map = h.find("Disk map").expect("the map is labelled");
    let first = disk::installed(&s, &d)[0].at.clone();
    let ev = crate::uitest::click(map.min + egui::vec2(BLOCK / 2.0, BLOCK / 2.0));
    frame(&mut h, &d, &s, &mut st, ev);
    assert_eq!(st.picked, Some(first.clone()));
    // An installed program offers Remove, not Install.
    let ev = press(&h, "◂ Remove");
    assert_eq!(frame(&mut h, &d, &s, &mut st, ev), [WbOut::Remove(first)]);
}

#[test]
fn arrows_walk_the_disk_then_the_box_and_enter_moves_the_pick() {
    let (d, s, mut h, mut st) = on_disk();
    let at = egui::pos2(450.0, 350.0);
    let on = disk::installed(&s, &d);
    let shelf = disk::shelf(&s, &d);
    let order: Vec<Disk> = on.iter().chain(&shelf).map(|p| p.at.clone()).collect();
    for want in &order {
        frame(&mut h, &d, &s, &mut st, key_at(Key::ArrowDown, at));
        assert_eq!(st.picked.as_ref(), Some(want));
    }
    // The last is the torch in the box: Enter installs it.
    assert_eq!(
        frame(&mut h, &d, &s, &mut st, key_at(Key::Enter, at)),
        [WbOut::Install(Disk::Stash(7))]
    );
    // Back up to an installed one: Enter removes it.
    frame(&mut h, &d, &s, &mut st, key_at(Key::ArrowUp, at));
    let prev = order[order.len() - 2].clone();
    assert_eq!(
        frame(&mut h, &d, &s, &mut st, key_at(Key::Enter, at)),
        [WbOut::Remove(prev)]
    );
}

#[test]
fn a_full_disk_greys_out_install() {
    let d = data();
    let mut s = save(&d);
    stash(&d, &mut s, "torch", 7);
    let cap = s.hdd_cap(&d.rig) as usize;
    let mut n = 100;
    while disk::installed(&s, &d).len() < cap {
        stash(&d, &mut s, "icepick", n);
        s.install(&d, &Disk::Stash(n)).unwrap();
        n += 1;
    }
    let mut h = Harness::new(900.0, 700.0);
    let mut st = SystemUi {
        panel: Panel::Disk,
        picked: Some(Disk::Stash(7)),
        ..SystemUi::default()
    };
    frame(&mut h, &d, &s, &mut st, Vec::new());
    let ev = press(&h, "Install ▸");
    assert!(frame(&mut h, &d, &s, &mut st, ev).is_empty());
    let at = egui::pos2(450.0, 350.0);
    assert!(
        frame(&mut h, &d, &s, &mut st, key_at(Key::Enter, at)).is_empty(),
        "Enter can't install onto a full disk either"
    );
}

#[test]
fn scrap_asks_first_and_can_be_cancelled() {
    let (d, s, mut h, mut st) = on_disk();
    st.picked = Some(Disk::Stash(7));
    frame(&mut h, &d, &s, &mut st, Vec::new());
    let ev = press(&h, "Scrap…");
    assert!(frame(&mut h, &d, &s, &mut st, ev).is_empty());
    assert_eq!(st.confirm_scrap, Some(7));
    let ev = press(&h, "Cancel");
    assert!(frame(&mut h, &d, &s, &mut st, ev).is_empty());
    assert_eq!(st.confirm_scrap, None);
    // Delete asks too; this time, scrap it.
    let at = egui::pos2(450.0, 350.0);
    frame(&mut h, &d, &s, &mut st, key_at(Key::Delete, at));
    assert_eq!(st.confirm_scrap, Some(7));
    let ev = press(&h, "Scrap");
    assert_eq!(frame(&mut h, &d, &s, &mut st, ev), [WbOut::Scrap(7)]);
    assert_eq!(st.picked, None);
}

#[test]
fn free_programs_cant_be_scrapped() {
    let (d, s, mut h, mut st) = on_disk();
    let free = disk::installed(&s, &d)
        .into_iter()
        .find(|p| matches!(p.at, Disk::Free(_)))
        .unwrap();
    st.picked = Some(free.at);
    frame(&mut h, &d, &s, &mut st, Vec::new());
    assert!(h.find("Scrap…").is_none());
    let at = egui::pos2(450.0, 350.0);
    frame(&mut h, &d, &s, &mut st, key_at(Key::Delete, at));
    assert_eq!(st.confirm_scrap, None);
}

#[test]
fn the_shell_opens_disk_by_stash() {
    let d = data();
    let s = save(&d);
    let mut home = crate::home::Home::new(Vec::new());
    home.take_opens();
    home.run("open stash", &d, &s, &mut Vec::new());
    assert_eq!(home.take_opens(), [ninsei_core::hub::Tool::System]);
    assert_eq!(home.system.panel, Panel::Disk);
}
