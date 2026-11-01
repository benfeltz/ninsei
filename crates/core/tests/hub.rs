use ninsei_core::hub::{self, RunEnd, Save, Screen, ShellEffect, Tool};
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
    assert_eq!(save.record(&g), None, "a live run isn't booked");
    g.deck.credits = 150;
    g.phase = Phase::JackedOut;
    let r = save.record(&g).unwrap();
    assert_eq!((r.end, r.credits, r.seed), (RunEnd::Exit, 150, 9));
    g.phase = Phase::Flatlined;
    g.deck.credits = 900;
    save.record(&g);
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
fn every_tool_has_an_icon_and_only_some_are_installed() {
    let d = data();
    for t in Tool::ALL {
        assert!(d.icons.iter().any(|i| i.tool == t));
    }
    assert!(Tool::Shell.installed() && Tool::JackIn.installed());
    assert!(!Tool::Rig.installed());
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
    let run = |l: &str| hub::shell(l, &s, &d.icons);
    assert_eq!(run("jackin").1, ShellEffect::JackIn(None));
    assert_eq!(run("jackin 1a2b").1, ShellEffect::JackIn(Some(0x1a2b)));
    assert_eq!(run("jackin zz").1, ShellEffect::None);
    assert_eq!(run("run shell").1, ShellEffect::Open(Tool::Shell));
    assert_eq!(run("RUN RIG").1, ShellEffect::Open(Tool::Rig));
    assert!(run("run nope").0[0].contains("not found"));
    assert_eq!(run("theme green").1, ShellEffect::Theme("green".into()));
    assert_eq!(run("echo  hello there").0, vec!["hello there".to_string()]);
    assert!(
        run("dir")
            .0
            .iter()
            .any(|l| l.contains("RIG") && l.contains("not installed"))
    );
    assert!(run("info").0.iter().any(|l| l.starts_with("runs")));
    assert!(run("frobnicate").0[0].contains("Unknown command"));
    assert_eq!(run("   ").1, ShellEffect::None);
}
