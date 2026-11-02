//! Themes are data (`data/themes.ron`), and one theme colours the desktop and runs alike.

use ninsei_core::data::rgb;
use ninsei_core::hub::{self, DEFAULT_RUN_THEME, Save, ShellEffect};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

fn parse_with(themes: &str) -> Result<Data, String> {
    Data::parse(Sources {
        warez: include_str!("../../../data/warez.ron"),
        ice: include_str!("../../../data/ice.ron"),
        loot: include_str!("../../../data/loot.ron"),
        bbs: include_str!("../../../data/bbs/board.ron"),
        splash: include_str!("../../../data/bbs/splash.txt"),
        events: include_str!("../../../data/events.ron"),
        contracts: include_str!("../../../data/contracts.ron"),
        icons: include_str!("../../../data/hub/icons.ron"),
        rig: include_str!("../../../data/hub/rig.ron"),
        networks: include_str!("../../../data/hub/networks.ron"),
        exchanges: include_str!("../../../data/hub/exchanges.ron"),
        themes,
        displays: include_str!("../../../data/displays.ron"),
    })
}

#[test]
fn the_shipped_themes() {
    let d = data();
    for id in ["amber", "green", "ice", "plasma", "workbench"] {
        let t = d.theme(id).unwrap_or_else(|| panic!("missing {id}"));
        assert!(t.colors.all().iter().all(|c| rgb(c).is_some()));
    }
    assert!(d.theme(DEFAULT_RUN_THEME).is_some());
}

#[test]
fn rgb_parses_hex() {
    assert_eq!(rgb("#ff8800"), Some([0xff, 0x88, 0x00]));
    assert_eq!(rgb("ff8800"), None);
    assert_eq!(rgb("#ff88"), None);
    assert_eq!(rgb("#gg8800"), None);
}

#[test]
fn one_setting_for_everything() {
    let s = Save::default();
    assert_eq!(s.phosphor, DEFAULT_RUN_THEME);
    let old = Save::load("(version: 1, phosphor: \"ice\")").unwrap();
    assert_eq!(old.phosphor, "ice", "older saves keep their theme");
}

#[test]
fn shell_theme_command_knows_the_data() {
    let d = data();
    let s = Save::default();
    let run = |l: &str| hub::shell(l, &s, &d);
    assert_eq!(run("theme plasma").1, ShellEffect::Theme("plasma".into()));
    assert_eq!(
        run("theme workbench").1,
        ShellEffect::Theme("workbench".into()),
        "every theme works everywhere"
    );
    assert_eq!(run("theme nope").1, ShellEffect::None);
    assert!(run("theme").0[0].contains("workbench"));
    assert_eq!(
        run("desktop workbench").1,
        ShellEffect::None,
        "there's no separate desktop setting"
    );
}

#[test]
fn a_new_theme_is_one_entry() {
    let extra = include_str!("../../../data/themes.ron")
        .trim_end()
        .trim_end_matches(']')
        .to_string()
        + r##"    (id: "paper", label: "paper white",
     colors: (bg: "#101010", panel: "#181818", faint: "#404040", dim: "#909090", fg: "#e8e8e0", bright: "#ffffff")),
]"##;
    let d = parse_with(&extra).unwrap();
    assert!(d.theme("paper").is_some());
}

#[test]
fn bad_themes_are_caught() {
    let base = include_str!("../../../data/themes.ron");
    let bad_colour = base.replacen("#ffb000", "orange", 1);
    let no_default = base.replacen("id: \"amber\"", "id: \"umber\"", 1);
    assert!(parse_with(&no_default).unwrap_err().contains("default"));
    assert!(parse_with(&bad_colour).unwrap_err().contains("bad colour"));
    let dupe = base.replacen("id: \"green\"", "id: \"amber\"", 1);
    assert!(parse_with(&dupe).unwrap_err().contains("twice"));
}
