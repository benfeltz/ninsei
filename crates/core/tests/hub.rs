use ninsei_core::hub::{self, GameCopy, RunEnd, Save, Screen, ShellEffect, Tool};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

#[test]
fn save_round_trips() {
    let s = Save {
        runs: 7,
        best_haul: 420,
        phosphor: "green".into(),
        ..Default::default()
    };
    assert_eq!(Save::load(&s.to_text()), Ok(s));
}

#[test]
fn bad_saves_are_refused_not_fatal() {
    assert!(Save::load("").is_err());
    assert!(Save::load("not ron at all {{{").is_err());
    let future = Save::default()
        .to_text()
        .replace("version: 1", "version: 99");
    assert!(Save::load(&future).unwrap_err().contains("version 99"));
    let corrupt = Save::default()
        .to_text()
        .replace("runs: 0", "runs: \"lots\"");
    assert!(Save::load(&corrupt).is_err());
}

#[test]
fn older_saves_missing_fields_still_load() {
    let s = Save::load("(version: 1, runs: 3)").unwrap();
    assert_eq!(
        (s.runs, s.exits, s.phosphor.as_str(), s.crt),
        (3, 0, "amber", true)
    );
}

#[test]
fn finished_runs_are_booked() {
    let mut save = Save::default();
    let mut g = Game::new(data(), 9);
    assert_eq!(
        save.record(&g, &[], &data().rig),
        None,
        "a live run isn't booked"
    );
    g.deck.credits = 150;
    g.phase = Phase::JackedOut;
    let r = save.record(&g, &[], &data().rig).unwrap();
    assert_eq!((r.end, r.credits, r.seed), (RunEnd::Exit, 150, 9));
    g.phase = Phase::Flatlined;
    g.deck.credits = 900;
    save.record(&g, &[], &data().rig);
    assert_eq!(
        (save.runs, save.exits, save.flatlines, save.best_haul),
        (2, 1, 1, 150)
    );
}

#[test]
fn screen_drags_in_and_out() {
    let mut s = Screen::JackIn { t: 0.0 };
    assert_eq!(s.drag(), 0.0);
    let mut last = 0.0;
    for _ in 0..200 {
        s = s.tick(0.01);
        assert!(s.drag() >= last, "drag only moves one way");
        last = s.drag();
    }
    assert_eq!(s, Screen::Run);
    assert_eq!(s.drag(), 1.0);
    let mut s = Screen::JackOut { t: 0.0 };
    for _ in 0..200 {
        s = s.tick(0.01);
    }
    assert_eq!(s, Screen::Hub);
    assert!(Screen::Hub.hub_live() && !Screen::JackIn { t: 0.1 }.hub_live());
    assert_eq!(
        Screen::Crash.tick(5.0),
        Screen::Crash,
        "the crash screen waits for a click"
    );
}

#[test]
fn every_tool_has_an_icon_and_works() {
    let d = data();
    for t in Tool::ALL {
        assert!(d.icons.iter().any(|i| i.tool == t));
    }
    assert!(
        Tool::ALL.iter().all(|t| t.installed()),
        "every tool works now"
    );
}

#[test]
fn icon_validation_catches_ragged_art() {
    let mut icons = data().icons;
    icons[0].pixels[2].push('k');
    assert!(
        hub::IconDef::validate(&icons)
            .unwrap_err()
            .contains("row 2")
    );
    let mut icons = data().icons;
    icons[0].pixels[0] = icons[0].pixels[0].replace('.', "z");
    assert!(hub::IconDef::validate(&icons).unwrap_err().contains('z'));
}

#[test]
fn shell_commands() {
    let d = data();
    let s = Save::default();
    let run = |l: &str| hub::shell(l, &s, &d);
    assert_eq!(
        run("jackin").1,
        ShellEffect::None,
        "jackin now points at dial"
    );
    assert_eq!(
        run("dial 1").1,
        ShellEffect::None,
        "an empty phonebook has nothing to dial"
    );
    assert_eq!(run("run shell").1, ShellEffect::Open(Tool::Shell));
    assert_eq!(run("run cpu shopper").1, ShellEffect::Open(Tool::Rig));
    assert_eq!(run("run CPUShopper").1, ShellEffect::Open(Tool::Rig));
    assert_eq!(
        run("RUN RIG").1,
        ShellEffect::Open(Tool::Rig),
        "the old name still works"
    );
    assert!(run("run nope").0[0].contains("not found"));
    // A fresh save has the white mono tube: paper white yes, green no.
    assert_eq!(
        run("theme paperwhite").1,
        ShellEffect::Theme("paperwhite".into())
    );
    assert_eq!(run("theme green").1, ShellEffect::None);
    assert_eq!(run("echo  hello there").0, vec!["hello there".to_string()]);
    assert!(run("dir").0.iter().any(|l| l.contains("CarrierTerm")));
    assert!(run("info").0.iter().any(|l| l.starts_with("runs")));
    assert!(run("frobnicate").0[0].contains("Unknown command"));
    assert_eq!(run("   ").1, ShellEffect::None);
}

#[test]
fn modem_sounds_default_on_and_toggle_from_the_shell() {
    let s = Save::load("(version: 1, runs: 4)").unwrap();
    assert!(s.sound, "old saves get sound on");
    let d = Data::builtin().unwrap();
    assert_eq!(hub::shell("sound", &s, &d).1, hub::ShellEffect::Sound);
}

#[test]
fn the_volume_starts_full_stays_in_range_and_comes_back() {
    let mut s = Save::load("(version: 1, runs: 4)").unwrap();
    assert_eq!(s.volume, 100, "old saves play at full volume");
    assert_eq!(Save::default().volume, 100);
    assert_eq!(s.volume_gain(), 1.0);
    s.set_volume(25);
    assert_eq!((s.volume, s.volume_gain()), (25, 0.25));
    s.set_volume(300);
    assert_eq!(s.volume, 100);
    // A control with notches: seven on a Mac slider, ten meter cells.
    assert_eq!(hub::notch_volume(5, 7), 71);
    assert_eq!(hub::volume_notch(71, 7), 5);
    assert_eq!(hub::volume_notch(71, 10), 7);
    assert_eq!(hub::notch_volume(9, 7), 100, "past the top is full");
    assert_eq!(hub::volume_notch(300, 7), 7);
    assert_eq!(hub::notch_volume(0, 10), 0);
    for steps in [7, 10, 20] {
        for n in 0..=steps {
            let pct = hub::notch_volume(n, steps);
            assert_eq!(
                hub::volume_notch(pct, steps),
                n,
                "notch {n} of {steps} round-trips"
            );
        }
    }
    // It comes back with the save.
    s.set_volume(60);
    assert_eq!(Save::load(&s.to_text()).unwrap().volume, 60);
}

#[test]
fn a_new_deck_comes_with_the_shops_disk_once() {
    let d = Data::builtin().unwrap();
    let mut s = Save::default();
    s.ensure_phonebook(&d, 0);
    assert_eq!(s.copies("nibbler"), vec![GameCopy::Cracked]);
    assert_eq!(s.games.len(), 1, "one disk");
    // Applied once: a later boot doesn't hand it over again, even if it went.
    s.games.clear();
    assert!(!s.ensure_phonebook(&d, 0));
    assert!(s.games.is_empty());
    // A save that booted before the disk existed never gains it: no migrations.
    let mut old = Save {
        started: true,
        ..Save::default()
    };
    old.ensure_phonebook(&d, 0);
    assert!(old.games.is_empty());
    // The bare kit starts with nothing on the desktop.
    let mut bare = Save {
        kit: "nothing".into(),
        ..Save::default()
    };
    bare.ensure_phonebook(&d, 0);
    assert!(bare.games.is_empty());
}

#[test]
fn a_kit_naming_an_unknown_game_is_rejected() {
    let mut d = Data::builtin().unwrap();
    d.starts.kits[0].games = vec![("pong".into(), GameCopy::Boxed)];
    assert!(d.validate().unwrap_err().contains("unknown game `pong`"));
}
