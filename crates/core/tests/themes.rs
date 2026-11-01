//! Themes are data: runs and the desktop pick from `data/themes.ron`.

use ninsei_core::data::rgb;
use ninsei_core::hub::{self, DEFAULT_RUN_THEME, MATCH_RUN_THEME, Save, ShellEffect};
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
    })
}

#[test]
fn the_shipped_themes() {
    let d = data();
    for id in ["amber", "green", "ice", "plasma", "workbench"] {
        let t = d.theme(id).unwrap_or_else(|| panic!("missing {id}"));
        assert!(t.colors.all().iter().all(|c| rgb(c).is_some()));
    }
    assert!(d.theme("workbench").unwrap().desktop && !d.theme("workbench").unwrap().run);
    assert!(d.theme(DEFAULT_RUN_THEME).unwrap().run);
}

#[test]
fn rgb_parses_hex() {
    assert_eq!(rgb("#ff8800"), Some([0xff, 0x88, 0x00]));
    assert_eq!(rgb("ff8800"), None);
    assert_eq!(rgb("#ff88"), None);
    assert_eq!(rgb("#gg8800"), None);
}

#[test]
fn the_desktop_matches_the_run_theme_by_default() {
    let mut s = Save::default();
    assert_eq!(s.desktop, MATCH_RUN_THEME);
    assert_eq!(s.desktop_theme(), DEFAULT_RUN_THEME);
    s.phosphor = "green".into();
    assert_eq!(s.desktop_theme(), "green");
    s.desktop = "workbench".into();
    assert_eq!(s.desktop_theme(), "workbench");
    let old = Save::load("(version: 1, phosphor: \"ice\")").unwrap();
    assert_eq!(
        old.desktop_theme(),
        "ice",
        "old saves follow their run theme"
    );
}

#[test]
fn shell_theme_commands_know_the_data() {
    let d = data();
    let s = Save::default();
    let run = |l: &str| hub::shell(l, &s, &d);
    assert_eq!(run("theme plasma").1, ShellEffect::Theme("plasma".into()));
    assert_eq!(
        run("theme workbench").1,
        ShellEffect::None,
        "workbench is desktop-only"
    );
    assert_eq!(
        run("desktop workbench").1,
        ShellEffect::Desktop("workbench".into())
    );
    assert_eq!(
        run("desktop match").1,
        ShellEffect::Desktop(MATCH_RUN_THEME.into())
    );
    assert_eq!(run("desktop nope").1, ShellEffect::None);
    assert!(run("desktop").0[0].contains("workbench"));
}

#[test]
fn a_new_theme_is_one_entry() {
    let extra = include_str!("../../../data/themes.ron")
        .trim_end()
        .trim_end_matches(']')
        .to_string()
        + r##"    (id: "paper", label: "paper white", run: true, desktop: true,
     colors: (bg: "#101010", panel: "#181818", faint: "#404040", dim: "#909090", fg: "#e8e8e0", bright: "#ffffff")),
]"##;
    let d = parse_with(&extra).unwrap();
    assert!(d.theme("paper").is_some_and(|t| t.run && t.desktop));
}

#[test]
fn bad_themes_are_caught() {
    let base = include_str!("../../../data/themes.ron");
    let bad_colour = base.replacen("#ffb000", "orange", 1);
    assert!(parse_with(&bad_colour).unwrap_err().contains("bad colour"));
    let nowhere = base.replacen("run: false, desktop: true", "run: false, desktop: false", 1);
    assert!(parse_with(&nowhere).unwrap_err().contains("usable"));
    let dupe = base.replacen("id: \"green\"", "id: \"amber\"", 1);
    assert!(parse_with(&dupe).unwrap_err().contains("twice"));
    let reserved = base.replacen("id: \"green\"", "id: \"match\"", 1);
    assert!(parse_with(&reserved).unwrap_err().contains("reserved"));
}
