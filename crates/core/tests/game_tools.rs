//! Games on the desktop: one `Tool::Game` per copy you own, named and drawn from `games.ron`,
//! and nowhere at all until you own it.

use ninsei_core::hub::{self, GameCopy, GameId, Save, ShellEffect, Tool};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

fn nibbler(copy: GameCopy) -> Tool {
    Tool::game("nibbler", copy).unwrap()
}

/// What `run`/`open` completes to on this save.
fn run_words(save: &Save, d: &Data) -> Vec<String> {
    hub::shell_complete("run ", save, d)
}

#[test]
fn a_game_you_dont_own_isnt_there() {
    let d = data();
    let save = Save::default();
    let cracked = nibbler(GameCopy::Cracked);
    assert!(!save.opens(cracked) && !save.opens(nibbler(GameCopy::Boxed)));
    assert!(Tool::games(&d, &save).is_empty());
    assert!(
        !save
            .icons(&d)
            .iter()
            .any(|i| matches!(i.tool, Tool::Game(..))),
        "no icon"
    );
    assert!(
        !run_words(&save, &d).iter().any(|w| w.contains("nibbler")),
        "no completion"
    );
    let (lines, effect) = hub::shell("run nibbler", &save, &d);
    assert_eq!(effect, ShellEffect::None);
    assert!(lines[0].contains("object not found"), "{lines:?}");
    let (dir, _) = hub::shell("dir", &save, &d);
    assert!(!dir.iter().any(|l| l.to_lowercase().contains("nibbler")));
}

#[test]
fn a_game_you_own_has_an_icon_a_label_and_a_command() {
    let d = data();
    let mut save = Save::default();
    save.learn_game("nibbler", GameCopy::Cracked);
    let cracked = nibbler(GameCopy::Cracked);
    assert!(save.opens(cracked));
    assert!(
        !save.opens(nibbler(GameCopy::Boxed)),
        "only the copy you have"
    );
    assert_eq!(Tool::games(&d, &save), vec![cracked]);
    let icon = save
        .icons(&d)
        .into_iter()
        .find(|i| i.tool == cracked)
        .expect("an icon");
    assert_eq!(icon.label, "NIBBLER [2K4]");
    assert_eq!(icon.pixels, &d.game("nibbler").unwrap().icon[..]);
    assert_eq!(cracked.name(&d), "NIBBLER [2K4]");
    assert!(run_words(&save, &d).contains(&"nibbler[2k4]".to_string()));
    // By the box's name, or the label as it's listed.
    for line in ["run nibbler", "run NIBBLER [2K4]", "open nibbler[2k4]"] {
        assert_eq!(
            hub::shell(line, &save, &d).1,
            ShellEffect::Open(cracked),
            "{line}"
        );
    }
    let (dir, _) = hub::shell("dir", &save, &d);
    assert!(
        dir.iter()
            .any(|l| l.contains("NIBBLER [2K4]") && l.contains("game")),
        "{dir:?}"
    );
}

#[test]
fn owning_both_copies_is_two_icons_with_two_labels() {
    let d = data();
    let mut save = Save::default();
    save.learn_game("nibbler", GameCopy::Cracked);
    save.learn_game("nibbler", GameCopy::Boxed);
    let labels: Vec<String> = save
        .icons(&d)
        .into_iter()
        .filter(|i| matches!(i.tool, Tool::Game(..)))
        .map(|i| i.label.into_owned())
        .collect();
    assert_eq!(labels, ["NIBBLER [2K4]", "Nibbler"]);
    // The plain name is the box's label; the tag picks the crack.
    assert_eq!(
        hub::shell("run nibbler", &save, &d).1,
        ShellEffect::Open(nibbler(GameCopy::Boxed))
    );
    assert_eq!(
        hub::shell("run nibbler [2k4]", &save, &d).1,
        ShellEffect::Open(nibbler(GameCopy::Cracked))
    );
    let words = run_words(&save, &d);
    assert!(words.contains(&"nibbler".to_string()) && words.contains(&"nibbler[2k4]".to_string()));
}

#[test]
fn a_game_tool_is_saved_by_its_id() {
    let mut save = Save::default();
    save.tiling.open(nibbler(GameCopy::Boxed));
    let text = save.to_text();
    assert!(text.contains("Game(\"nibbler\", Boxed)"), "{text}");
    let back = Save::load(&text).unwrap();
    assert!(back.tiling.tiles().contains(&nibbler(GameCopy::Boxed)));
    assert_eq!(
        format!("{:?}", nibbler(GameCopy::Boxed)),
        "Game(\"nibbler\", Boxed)"
    );
}

#[test]
fn a_game_id_fits_a_tool() {
    assert!(GameId::new("minefield").is_some());
    assert!(GameId::new("").is_none());
    assert!(GameId::new(&"x".repeat(hub::GAME_ID_MAX + 1)).is_none());
    let mut d = data();
    // Minefield: no kit starts with it, so only the id is wrong.
    let m = d.games.iter().position(|g| g.id == "minefield").unwrap();
    d.games[m].id = "a-much-too-long-game-id".into();
    assert!(d.validate().unwrap_err().contains("ASCII characters"));
}

#[test]
fn a_game_without_good_art_is_rejected() {
    let mut d = data();
    d.games[1].icon[3].push('k');
    assert!(d.validate().unwrap_err().contains("row 3"));
    let mut d = data();
    d.games[1].icon.clear();
    assert!(d.validate().unwrap_err().contains("empty"));
}

#[test]
fn a_game_icon_belongs_in_games_ron() {
    let mut icons = data().icons;
    icons[0].tool = nibbler(GameCopy::Cracked);
    // The tool that lost its icon is reported first; put one back to reach the game's.
    let mut with_all = icons.clone();
    with_all.extend(data().icons.into_iter().take(1));
    assert!(
        hub::IconDef::validate(&with_all)
            .unwrap_err()
            .contains("games.ron")
    );
}

#[test]
fn each_round_of_a_game_is_counted_on_the_save() {
    let mut save = Save::default();
    assert_eq!(save.game_round("blox"), 0);
    assert_eq!(save.start_game("blox"), 0);
    assert_eq!(save.start_game("blox"), 1);
    assert_eq!(save.game_round("blox"), 2);
    assert_eq!(save.game_round("nibbler"), 0, "each game counts its own");
    let back = Save::load(&save.to_text()).unwrap();
    assert_eq!(back.game_round("blox"), 2);
    // An older save has played none.
    assert_eq!(Save::load("(version: 2)").unwrap().game_round("blox"), 0);
}
