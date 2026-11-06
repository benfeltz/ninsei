//! System, driven headlessly: clicks and keys on the real panels, checked against the
//! `WbOut`s they send.

use super::*;
use crate::uitest::{Harness, key};
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

/// `n` frames with no input, `dt` seconds apart.
fn idle(h: &mut Harness, n: usize, dt: f64, mut f: impl FnMut(&mut egui::Ui)) {
    for _ in 0..n {
        h.wait(dt - 0.1);
        h.frame(Vec::new(), &mut f);
    }
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
fn liza_arrows_walk_the_panel_list_once_it_has_been_clicked() {
    let (d, s) = (data(), save(&data()));
    let mut h = Harness::new(900.0, 700.0);
    let mut p = SystemUi::default();
    frame(&mut h, &d, &s, &mut p, Vec::new());
    // Nothing has the list yet: the arrows belong to whatever else is open.
    frame(&mut h, &d, &s, &mut p, key(Key::ArrowDown));
    assert_eq!(p.panel, Panel::Overview, "the list isn't focused yet");
    let ev = press(&h, Panel::Monitor.label());
    frame(&mut h, &d, &s, &mut p, ev);
    assert_eq!(p.panel, Panel::Monitor);
    for want in [Panel::Theme, Panel::Disk, Panel::Disk] {
        frame(&mut h, &d, &s, &mut p, key(Key::ArrowDown));
        assert_eq!(p.panel, want, "down, held at the end");
    }
    for want in [
        Panel::Theme,
        Panel::Monitor,
        Panel::Overview,
        Panel::Overview,
    ] {
        frame(&mut h, &d, &s, &mut p, key(Key::ArrowUp));
        assert_eq!(p.panel, want, "up, held at the top");
    }
}

#[test]
fn liza_panel_list_runs_the_window_height_with_the_version_under_it() {
    let (d, s) = (data(), save(&data()));
    let mut h = Harness::new(700.0, 480.0);
    let mut p = SystemUi::default();
    frame(&mut h, &d, &s, &mut p, Vec::new());
    let label = &s.firmware_def(&d).label;
    let version = h.find(label).expect("the version under the list");
    assert!(
        version.bottom() > 480.0 - 30.0,
        "the version sits at the bottom of the window, not mid-way: {version:?}"
    );
    let disk = h.find(Panel::Disk.label()).unwrap();
    assert!(disk.bottom() < version.top(), "every panel shows above it");
}

#[test]
fn the_monitor_panel_is_headed_monitor_and_names_the_tube_once() {
    let d = data();
    let s = save(&d);
    let mut h = Harness::new(900.0, 700.0);
    let mut p = SystemUi {
        panel: Panel::Monitor,
        ..SystemUi::default()
    };
    frame(&mut h, &d, &s, &mut p, Vec::new());
    // Where each text was drawn, once per place (the harness keeps both frames' widgets).
    let places = |t: &str| {
        let mut r: Vec<_> = h
            .all(t)
            .into_iter()
            .map(|r| (r.left() as i32, r.top() as i32))
            .collect();
        r.sort();
        r.dedup();
        r
    };
    let heading = places("Monitor")
        .into_iter()
        .filter(|&(x, _)| x as f32 > cp::LIST_W)
        .count();
    assert_eq!(heading, 1, "the panel says what it is, beside the list");
    let title = s.monitor_def(&d).title();
    assert_eq!(
        places(&title).len(),
        1,
        "the tube is named once, by its radio button"
    );
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

#[test]
fn defragment_plays_out_then_compacts_the_disk_once() {
    let (d, mut s, mut h, mut st) = on_disk();
    s.install(&d, &Disk::Stash(7)).unwrap();
    s.fragment(&d, 3);
    assert!(disk::fragmentation(&s, &d) > 0);
    frame(&mut h, &d, &s, &mut st, Vec::new());
    st.picked = Some(Disk::Stash(7));
    let ev = press(&h, "Defragment");
    assert!(
        frame(&mut h, &d, &s, &mut st, ev).is_empty(),
        "it plays first"
    );
    let steps = disk::defrag_steps(&s, &d).len();
    assert!(steps > 1, "a fragmented disk takes moves");
    assert!(st.defrag.is_some());
    assert_eq!(st.picked, None, "the pick is dropped while blocks move");
    assert!(h.find("Defragmenting…").is_some(), "{:?}", h.texts());
    // A click on the busy button does nothing.
    let ev = press(&h, "Defragmenting…");
    assert!(frame(&mut h, &d, &s, &mut st, ev).is_empty());

    // Run it out: one Defragment at the end, then it's idle again.
    let mut sent = Vec::new();
    let secs = anim::READ_SECS + anim::STEP_SECS * (steps + 2) as f64;
    let n = (secs / anim::STEP_SECS) as usize + 4;
    idle(&mut h, n, anim::STEP_SECS, |ui| {
        show(ui, &d, &s, &mut st, &mut sent)
    });
    assert_eq!(sent, [WbOut::Defragment]);
    assert!(st.defrag.is_none());
}

#[test]
fn defragment_takes_its_time() {
    let (d, mut s, mut h, mut st) = on_disk();
    s.install(&d, &Disk::Stash(7)).unwrap();
    s.fragment(&d, 3);
    frame(&mut h, &d, &s, &mut st, Vec::new());
    let ev = press(&h, "Defragment");
    frame(&mut h, &d, &s, &mut st, ev);
    // Still reading the disk a moment later: nothing's compacted yet.
    let mut sent = Vec::new();
    idle(&mut h, 2, anim::READ_SECS / 4.0, |ui| {
        show(ui, &d, &s, &mut st, &mut sent)
    });
    assert!(sent.is_empty());
    assert!(st.defrag.is_some());
}

/// `save` on another deck (its first firmware), owning all three decks.
fn on_deck(d: &Data, deck: &str) -> Save {
    let mut s = save(d);
    s.decks = d.hardware.decks.iter().map(|x| x.id.clone()).collect();
    s.deck = deck.into();
    s.firmware = String::new();
    s.ensure_hardware(d);
    s
}

/// AnarchyOS's System, focused (its keys live), on `panel`.
fn tui(d: &Data, s: &Save, panel: Panel) -> (Harness, SystemUi) {
    assert_eq!(
        s.firmware_def(d).desktop,
        ninsei_core::hardware::Desktop::Tiling
    );
    let mut h = Harness::new(900.0, 700.0);
    let mut st = SystemUi {
        panel,
        keys: true,
        ..SystemUi::default()
    };
    frame(&mut h, d, s, &mut st, Vec::new());
    (h, st)
}

#[test]
fn workbench_gadgets_pick_the_panel() {
    let d = data();
    let s = on_deck(&d, "hosaka");
    assert_eq!(
        s.firmware_def(&d).desktop,
        ninsei_core::hardware::Desktop::Workbench
    );
    let mut h = Harness::new(900.0, 700.0);
    let mut st = SystemUi::default();
    frame(&mut h, &d, &s, &mut st, Vec::new());
    for want in [Panel::Monitor, Panel::Theme, Panel::Disk, Panel::Overview] {
        let ev = press(&h, want.label());
        assert!(frame(&mut h, &d, &s, &mut st, ev).is_empty());
        assert_eq!(st.panel, want);
    }
    // The panels are the shared ones: About's switches still send.
    let ev = press(&h, "Sound");
    assert_eq!(frame(&mut h, &d, &s, &mut st, ev), [WbOut::Sound]);
}

#[test]
fn anarchy_keys_switch_tabs_and_tabs_click() {
    let d = data();
    let s = on_deck(&d, "onosendai");
    let (mut h, mut st) = tui(&d, &s, Panel::Overview);
    for (k, want) in [
        (Key::Num2, Panel::Monitor),
        (Key::Num3, Panel::Theme),
        (Key::Num4, Panel::Disk),
        (Key::Num1, Panel::Overview),
        (Key::CloseBracket, Panel::Monitor),
        (Key::OpenBracket, Panel::Overview),
        (Key::OpenBracket, Panel::Disk),
    ] {
        frame(&mut h, &d, &s, &mut st, key(k));
        assert_eq!(st.panel, want, "{k:?}");
    }
    let ev = press(&h, "2 monitor");
    frame(&mut h, &d, &s, &mut st, ev);
    assert_eq!(st.panel, Panel::Monitor);
    // Tab isn't System's: it moves the focus in egui and in the page.
    frame(&mut h, &d, &s, &mut st, key(Key::Tab));
    frame(&mut h, &d, &s, &mut st, key(Key::Num4));
    assert_eq!(st.panel, Panel::Disk, "Tab didn't take the keys away");
}

#[test]
fn anarchy_keys_are_quiet_unless_the_tile_is_focused() {
    let d = data();
    let s = on_deck(&d, "onosendai");
    let (mut h, mut st) = tui(&d, &s, Panel::Overview);
    st.keys = false;
    frame(&mut h, &d, &s, &mut st, key(Key::Num2));
    assert_eq!(st.panel, Panel::Overview);
}

#[test]
fn anarchy_settings_move_with_arrows_and_change_with_enter() {
    let d = data();
    let s = on_deck(&d, "onosendai");
    let (mut h, mut st) = tui(&d, &s, Panel::Overview);
    // Row 0 is sound.
    assert_eq!(
        frame(&mut h, &d, &s, &mut st, key(Key::Enter)),
        [WbOut::Sound]
    );
    // j (or ↓) to cracktros; Space works like Enter.
    frame(&mut h, &d, &s, &mut st, key(Key::J));
    assert_eq!(
        frame(&mut h, &d, &s, &mut st, key(Key::Space)),
        [WbOut::Intros(!s.intros)]
    );
    // Glitches: → and ← step through on / reduced / off.
    frame(&mut h, &d, &s, &mut st, key(Key::ArrowDown));
    let all = Glitches::ALL;
    let i = all.iter().position(|&g| g == s.glitches).unwrap();
    assert_eq!(
        frame(&mut h, &d, &s, &mut st, key(Key::ArrowRight)),
        [WbOut::Glitches(all[(i + 1) % all.len()])]
    );
    assert_eq!(
        frame(&mut h, &d, &s, &mut st, key(Key::ArrowLeft)),
        [WbOut::Glitches(all[(i + all.len() - 1) % all.len()])]
    );
    // Deck: → plugs in the next one you own.
    frame(&mut h, &d, &s, &mut st, key(Key::ArrowDown));
    let next =
        s.decks[(s.decks.iter().position(|x| *x == s.deck).unwrap() + 1) % s.decks.len()].clone();
    assert_eq!(
        frame(&mut h, &d, &s, &mut st, key(Key::ArrowRight)),
        [WbOut::Deck(next)]
    );
    // Clicking a row is Enter on it.
    let ev = press(&h, "cracktros");
    assert_eq!(
        frame(&mut h, &d, &s, &mut st, ev),
        [WbOut::Intros(!s.intros)]
    );
    assert_eq!(st.cursor, 1);
}

#[test]
fn anarchy_monitor_and_theme_lists() {
    let d = data();
    let s = on_deck(&d, "onosendai");
    let (mut h, mut st) = tui(&d, &s, Panel::Monitor);
    frame(&mut h, &d, &s, &mut st, key(Key::ArrowDown));
    assert_eq!(
        frame(&mut h, &d, &s, &mut st, key(Key::Enter)),
        [WbOut::Monitor(s.monitors[1].clone())]
    );
    // The last row is the CRT.
    for _ in 0..s.monitors.len() {
        frame(&mut h, &d, &s, &mut st, key(Key::ArrowDown));
    }
    assert_eq!(
        frame(&mut h, &d, &s, &mut st, key(Key::Enter)),
        [WbOut::Crt]
    );

    // On a mono tube, Enter on any other theme does nothing.
    let mut mono = s.clone();
    let m = d.hardware.monitors.iter().find(|m| !m.colour).unwrap();
    mono.monitor = m.id.clone();
    mono.phosphor = m.locked_theme(&d).unwrap().to_string();
    let (mut h, mut st) = tui(&d, &mono, Panel::Theme);
    for (i, t) in d.themes.iter().enumerate() {
        st.cursor = i;
        let out = frame(&mut h, &d, &mono, &mut st, key(Key::Enter));
        if t.id == mono.phosphor {
            assert_eq!(out, [WbOut::Theme(t.id.clone())]);
        } else {
            assert!(out.is_empty(), "{} is locked", t.id);
        }
    }
}

#[test]
fn anarchy_disk_picks_installs_scraps_and_defrags_by_key() {
    let d = data();
    let mut s = on_deck(&d, "onosendai");
    stash(&d, &mut s, "torch", 7);
    let (mut h, mut st) = tui(&d, &s, Panel::Disk);
    let on = disk::installed(&s, &d);
    // j picks the first installed program; Enter takes it off.
    frame(&mut h, &d, &s, &mut st, key(Key::J));
    assert_eq!(st.picked, Some(on[0].at.clone()));
    assert_eq!(
        frame(&mut h, &d, &s, &mut st, key(Key::Enter)),
        [WbOut::Remove(on[0].at.clone())]
    );
    // Down past the disk to the torch in the box: Enter installs it.
    for _ in 0..on.len() {
        frame(&mut h, &d, &s, &mut st, key(Key::J));
    }
    assert_eq!(st.picked, Some(Disk::Stash(7)));
    assert_eq!(
        frame(&mut h, &d, &s, &mut st, key(Key::Enter)),
        [WbOut::Install(Disk::Stash(7))]
    );
    // x asks, n cancels; x then y scraps.
    frame(&mut h, &d, &s, &mut st, key(Key::X));
    assert_eq!(st.confirm_scrap, Some(7));
    assert!(frame(&mut h, &d, &s, &mut st, key(Key::N)).is_empty());
    assert_eq!(st.confirm_scrap, None);
    frame(&mut h, &d, &s, &mut st, key(Key::X));
    assert_eq!(
        frame(&mut h, &d, &s, &mut st, key(Key::Y)),
        [WbOut::Scrap(7)]
    );
    // d defrags.
    frame(&mut h, &d, &s, &mut st, key(Key::D));
    assert!(st.defrag.is_some());
}

#[test]
fn anarchy_disk_clicks_pick_and_the_hints_click() {
    let d = data();
    let mut s = on_deck(&d, "onosendai");
    stash(&d, &mut s, "torch", 7);
    let (mut h, mut st) = tui(&d, &s, Panel::Disk);
    let ev = press(&h, &file_of(&d, &s, &Disk::Stash(7)));
    frame(&mut h, &d, &s, &mut st, ev);
    assert_eq!(st.picked, Some(Disk::Stash(7)));
    let ev = press(&h, "enter install");
    assert_eq!(
        frame(&mut h, &d, &s, &mut st, ev),
        [WbOut::Install(Disk::Stash(7))]
    );
}

#[test]
fn anarchy_fits_a_half_screen_tile() {
    // The tile next to the Shell: every key hint is still there and clickable.
    let d = data();
    let mut s = on_deck(&d, "onosendai");
    stash(&d, &mut s, "torch", 7);
    let mut h = Harness::new(460.0, 700.0);
    let mut st = SystemUi {
        panel: Panel::Disk,
        keys: true,
        picked: Some(Disk::Stash(7)),
        ..SystemUi::default()
    };
    frame(&mut h, &d, &s, &mut st, Vec::new());
    for hint in ["] next", "x scrap", "d defrag", "enter install"] {
        let r = h
            .find(hint)
            .unwrap_or_else(|| panic!("{hint}: {:?}", h.texts()));
        assert!(r.right() <= 460.0, "{hint} runs off the tile: {r:?}");
    }
    let ev = press(&h, "d defrag");
    frame(&mut h, &d, &s, &mut st, ev);
    assert!(st.defrag.is_some());
}

// Switching decks and firmware from the windowed System (Liza and Desktop).

#[test]
fn about_this_deck_switches_decks_on_liza() {
    let d = data();
    let s = on_deck(&d, "sensenet");
    for other in d.hardware.decks.iter().filter(|x| x.id != s.deck) {
        assert_eq!(
            click(&d, &s, Panel::Overview, &other.title()),
            [WbOut::Deck(other.id.clone())],
            "{}",
            other.id
        );
    }
}

/// The Ono-Sendai on its Workbench firmware (6.2), which has another firmware to switch to.
fn ono_on_workbench(d: &Data) -> Save {
    let mut s = on_deck(d, "onosendai");
    s.use_firmware(d, "6.2").unwrap();
    assert_eq!(
        s.firmware_def(d).desktop,
        ninsei_core::hardware::Desktop::Workbench
    );
    s
}

#[test]
fn about_this_deck_switches_firmware_on_desktop() {
    let d = data();
    let s = ono_on_workbench(&d);
    let other = s
        .deck_def(&d)
        .firmware
        .iter()
        .find(|f| f.id != s.firmware_def(&d).id)
        .unwrap();
    assert_eq!(
        click(&d, &s, Panel::Overview, &other.label),
        [WbOut::Firmware(other.id.clone())]
    );
}

// What System sends really changes the save: the click, then the app's own settings step.

/// Applies what a click sent, the way the app does.
fn apply_all(s: &mut Save, d: &Data, outs: Vec<WbOut>) -> Vec<crate::settings::Applied> {
    outs.into_iter()
        .map(|o| crate::settings::apply(s, d, o))
        .collect()
}

#[test]
fn system_clicks_change_the_save() {
    use crate::settings::Applied;
    let d = data();

    // A theme, on a colour tube.
    let mut s = save(&d);
    s.plug_monitor(
        &d,
        &d.hardware.monitors.iter().find(|m| m.colour).unwrap().id,
    )
    .unwrap();
    let theme = d.themes.iter().find(|t| t.id != s.phosphor).unwrap();
    let outs = click(&d, &s, Panel::Theme, &theme.label);
    assert_eq!(
        apply_all(&mut s, &d, outs),
        [Applied::Theme(theme.label.clone())]
    );
    assert_eq!(s.phosphor, theme.id);

    // A monitor.
    let tube = d
        .hardware
        .monitors
        .iter()
        .find(|m| m.id != s.monitor)
        .unwrap();
    let outs = click(&d, &s, Panel::Monitor, &tube.title());
    assert!(matches!(
        apply_all(&mut s, &d, outs)[..],
        [Applied::Hardware(_)]
    ));
    assert_eq!(s.monitor, tube.id);

    // The switches.
    let (glitches, intros) = (s.glitches, s.intros);
    let other = Glitches::ALL.into_iter().find(|&g| g != glitches).unwrap();
    let outs = click(&d, &s, Panel::Overview, other.label());
    apply_all(&mut s, &d, outs);
    assert_eq!(s.glitches, other);
    let outs = click(&d, &s, Panel::Overview, "Cracktros");
    apply_all(&mut s, &d, outs);
    assert_eq!(s.intros, !intros);

    // A deck: the desktop changes with it.
    let mut s = on_deck(&d, "sensenet");
    let ono = d.hardware.deck("onosendai").unwrap();
    let outs = click(&d, &s, Panel::Overview, &ono.title());
    assert!(matches!(
        apply_all(&mut s, &d, outs)[..],
        [Applied::Hardware(_)]
    ));
    assert_eq!(s.deck, "onosendai");
    assert_eq!(
        s.firmware_def(&d).desktop,
        ninsei_core::hardware::Desktop::Tiling
    );

    // Firmware: from Desktop back to AnarchyOS.
    let mut s = ono_on_workbench(&d);
    let outs = click(&d, &s, Panel::Overview, "Cyberspace 7.0");
    assert!(matches!(
        apply_all(&mut s, &d, outs)[..],
        [Applied::Hardware(_)]
    ));
    assert_eq!(
        s.firmware_def(&d).desktop,
        ninsei_core::hardware::Desktop::Tiling
    );
}

#[test]
fn refused_settings_leave_the_save_alone() {
    use crate::settings::{Applied, apply};
    let d = data();
    let mut s = save(&d);
    let mono = d.hardware.monitors.iter().find(|m| !m.colour).unwrap();
    s.plug_monitor(&d, &mono.id).unwrap();
    let locked = s.phosphor.clone();
    let other = d.themes.iter().find(|t| t.id != locked).unwrap();
    assert!(matches!(
        apply(&mut s, &d, WbOut::Theme(other.id.clone())),
        Applied::Refused(_)
    ));
    assert_eq!(s.phosphor, locked, "a mono tube keeps its theme");
    assert!(matches!(
        apply(&mut s, &d, WbOut::Deck("nope".into())),
        Applied::Refused(_)
    ));
    assert!(
        matches!(apply(&mut s, &d, WbOut::Crt), Applied::Other(WbOut::Crt)),
        "the app handles the CRT itself"
    );
}

// Desktop (Workbench): its Disk panel works like Liza's.

#[test]
fn desktop_disk_picks_then_installs() {
    let d = data();
    let mut s = on_deck(&d, "hosaka");
    assert_eq!(
        s.firmware_def(&d).desktop,
        ninsei_core::hardware::Desktop::Workbench
    );
    stash(&d, &mut s, "torch", 7);
    let mut h = Harness::new(900.0, 700.0);
    let mut st = SystemUi {
        panel: Panel::Disk,
        ..SystemUi::default()
    };
    frame(&mut h, &d, &s, &mut st, Vec::new());
    let torch = Disk::Stash(7);
    let ev = press(&h, &file_of(&d, &s, &torch));
    frame(&mut h, &d, &s, &mut st, ev);
    assert_eq!(st.picked, Some(torch.clone()));
    let ev = press(&h, "Install ▸");
    assert_eq!(frame(&mut h, &d, &s, &mut st, ev), [WbOut::Install(torch)]);
}

// Every panel fits its window: nothing reaches past what the window would clip.

/// A tool window's body on Liza and on Desktop, a little under both (700x500 windows).
const WINDOW_BODY: (f32, f32) = (680.0, 464.0);

/// A full save: every monitor, deck and tool, and a stash filled to the brim.
fn full(d: &Data, deck: &str) -> Save {
    let mut s = on_deck(d, deck);
    let room = s.stash_cap(&d.rig) as usize;
    for (i, w) in d.warez.iter().cycle().take(room).enumerate() {
        stash(d, &mut s, &w.id, 100 + i as u32);
    }
    s
}

#[test]
fn every_panel_fits_its_window_on_liza_and_desktop() {
    let d = data();
    for deck in ["sensenet", "hosaka"] {
        let s = full(&d, deck);
        for panel in Panel::ALL {
            let mut h = Harness::new(WINDOW_BODY.0, WINDOW_BODY.1);
            let mut st = SystemUi {
                panel,
                ..SystemUi::default()
            };
            frame(&mut h, &d, &s, &mut st, Vec::new());
            let area = egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(WINDOW_BODY.0, WINDOW_BODY.1),
            );
            let out = h.outside(area);
            assert!(
                out.is_empty(),
                "{deck} {panel:?}: past the window's edge: {out:?}"
            );
        }
    }
}
