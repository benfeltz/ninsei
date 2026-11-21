//! Demos: a drawn intro is data, and a frame is a pure function of time.

use ninsei_core::data::DemoSrc;
use ninsei_core::demo::DemoDef;
use ninsei_core::{Data, Sources};

/// A small demo: one floor, a cube passing on the left, a logo, two pages of credits in the
/// band and a scroller below it.
const SMALL: &str = r####"(
    group: "TESTCREW",
    width: 160, height: 100, secs: 4.0,
    colours: [("black", "000"), ("white", "FFF"), ("ice", "8CF"), ("blue", "05A"), ("dim", "023")],
    ramps: [
        ("floor", ["ice", "blue", "dim"]),
        ("glow", ["dim"]),
        ("type", ["white", "ice", "blue"]),
    ],
    camera: (focal: 80.0, height: 1.0, horizon: 40.0, near: 0.3, far: 12.0, track: 16.0,
        sway: [(axis: X, amp: 0.5, cycles: 1), (axis: Roll, amp: 4.0, cycles: 2, phase: 0.25)]),
    grid: (spacing: 1.0, lines: 12, planes: [(y: 0.0, ramp: "floor", glow: Some("glow"))]),
    shapes: [(id: "cube",
        points: [(-1, -1, -1), (1, -1, -1), (1, 1, -1), (-1, 1, -1),
                 (-1, -1, 1), (1, -1, 1), (1, 1, 1), (-1, 1, 1)],
        edges: [(0, 1), (1, 2), (2, 3), (3, 0), (4, 5), (5, 6), (6, 7), (7, 4),
                (0, 4), (1, 5), (2, 6), (3, 7)])],
    solids: [(shape: "cube", at: (-1.5, 1.0, 6.0), size: 0.4, spin: (1, 2, 0), ramp: "floor")],
    font: (width: 4, height: 3, glyphs: [
        ('A', [".#..", "#.#.", "#.#."]),
        ('B', ["##..", "###.", "##.."]),
        ('C', [".##.", "#...", ".##."]),
    ]),
    styles: [
        ("logo", (scale: 2, ramp: "type", shadow: Some("blue"))),
        ("text", (scale: 1, ramp: "type", cycles: 2, outline: Some("black"))),
    ],
    title: [(text: "ABC", y: 2, style: "logo")],
    band: (y: 50, h: 20, fill: Some("black"), edge: Some("blue")),
    credits: (style: "text", gap: 1, hold: 0.8, pages: [["ABC", "CBA"], ["A B C"]]),
    scroller: (style: "text", y: 84, text: "ABC ", greetz: [], sep: " ", passes: 2,
        amp: 3.0, wave: 40.0, cycles: 1),
    tune: (bpm: 120, tracks: [(wave: Square, vol: 0.2, pattern: "A4 - . . C5 - . . E5 - . . A4 - . .")]),
)"####;

fn parse(src: &str) -> Result<Data, String> {
    let mut demos = Sources::builtin().demos.to_vec();
    demos.push(DemoSrc {
        id: "small",
        demo: src,
    });
    Data::parse(Sources {
        demos: &demos,
        ..Sources::builtin()
    })
    .map(|mut d| {
        // Only `small` is under test here.
        d.demos.retain(|x| x.id == "small");
        d
    })
}

fn small() -> DemoDef {
    parse(SMALL).unwrap().demo("small").unwrap().clone()
}

/// The file with `find` replaced by `with`, which must be refused with a message naming the
/// file and containing `says`.
fn refused(find: &str, with: &str, says: &str) {
    assert!(SMALL.contains(find), "no `{find}` in the test demo");
    let Err(err) = parse(&SMALL.replacen(find, with, 1)) else {
        panic!("`{find}` -> `{with}` was taken");
    };
    assert!(
        err.contains("demos/small.ron") && err.contains(says),
        "`{find}` -> `{with}`: {err}"
    );
}

#[test]
fn the_same_moment_is_the_same_frame() {
    let d = small();
    for t in [0.0, 0.7, 1.9, 3.3] {
        assert_eq!(d.frame(t), d.frame(t));
        assert_eq!(d.pixels(&d.frame(t)), d.pixels(&d.frame(t)));
    }
    assert_ne!(d.frame(0.0), d.frame(0.5), "it moves");
}

#[test]
fn every_line_is_on_the_screen_all_the_way_round() {
    let d = small();
    let (w, h) = (d.width as f32 - 1.0, d.height as f32 - 1.0);
    let on = |p: [f32; 2]| (0.0..=w).contains(&p[0]) && (0.0..=h).contains(&p[1]);
    for n in 0..200 {
        let f = d.frame(f64::from(d.secs) * f64::from(n) / 200.0);
        assert!(!f.segs.is_empty(), "the grid is always there");
        for s in &f.segs {
            assert!(on(s.a) && on(s.b), "{s:?} at step {n}");
        }
    }
}

#[test]
fn nothing_behind_the_eye_is_drawn() {
    let d = small();
    // Looking level from the floor's height down a track: the floor is below the horizon, so
    // with no sway nothing of it is drawn above it.
    let mut level = d.clone();
    level.camera.sway.clear();
    level.solids.clear();
    for n in 0..20 {
        let f = level.frame(f64::from(n) * 0.2);
        for s in &f.segs {
            assert!(
                s.a[1] >= level.camera.horizon && s.b[1] >= level.camera.horizon,
                "{s:?}: a line from behind the eye wraps over the horizon"
            );
        }
    }
}

#[test]
fn the_end_of_the_loop_is_its_start() {
    let d = small();
    let secs = f64::from(d.secs);
    assert_eq!(d.frame(secs), d.frame(0.0));
    assert_eq!(d.frame(secs * 3.0 + 1.25), d.frame(1.25), "loops for ever");
    assert_eq!(d.pixels(&d.frame(secs)), d.pixels(&d.frame(0.0)));
}

#[test]
fn the_cube_passes_and_comes_round_again() {
    let d = small();
    let mut grid_only = d.clone();
    grid_only.solids.clear();
    let extra = |t: f64| d.frame(t).segs.len() - grid_only.frame(t).segs.len();
    let seen: Vec<bool> = (0..80)
        .map(|n| extra(f64::from(d.secs) * f64::from(n) / 80.0) > 0)
        .collect();
    assert!(seen.iter().any(|s| *s), "it shows");
    assert!(seen.iter().any(|s| !*s), "and it goes by");
}

#[test]
fn credits_page_through_the_band_and_rest_between() {
    let d = small();
    assert_eq!(d.frame(0.1).page, Some(0));
    assert_eq!(
        d.frame(1.9).page,
        None,
        "the rest of the first page's share"
    );
    assert_eq!(d.frame(2.1).page, Some(1));
    let top = d.band.y;
    let bottom = d.band.y + d.band.h;
    let f = d.frame(0.1);
    let credits: Vec<_> = f
        .prints
        .iter()
        .filter(|p| p.y >= top && p.y < bottom)
        .collect();
    assert_eq!(credits.len(), 6, "ABC and CBA");
}

#[test]
fn text_is_shaded_top_to_bottom_and_the_shade_rolls() {
    let d = small();
    let px = d.pixels(&d.frame(0.0));
    let at = |x: i32, y: i32| px[(y * d.width as i32 + x) as usize];
    // The logo's `A` at scale 2: its top ink row is white, its bottom blue.
    let w = 3 * 4 * 2;
    let x0 = (d.width as i32 - w) / 2;
    let (white, blue) = (1, 3);
    assert_eq!(at(x0 + 2, 2), white);
    assert_eq!(at(x0, 2 + 5), blue);
    // The scroller's style rolls: the same glyph row isn't the same colour a moment later.
    let roll = |t: f64| d.frame(t).prints.iter().find(|p| p.y >= 80).map(|p| p.roll);
    assert_eq!(roll(0.5), Some(0));
    assert_eq!(roll(1.2), Some(1));
}

#[test]
fn a_band_with_a_fill_is_clear_of_the_flight() {
    let d = small();
    let f = d.frame(1.0);
    let px = d.pixels(&f);
    let w = d.width as usize;
    let row = |y: i32| &px[y as usize * w..(y as usize + 1) * w];
    assert!(row(d.band.y - 1).iter().all(|&c| c == 3), "its edge");
    assert!(row(d.band.y).iter().all(|&c| c == 0), "blank between pages");
}

#[test]
fn bad_files_are_refused() {
    refused("\"8CF\"", "\"8CFF\"", "not three hex digits");
    refused(
        "[\"ice\", \"blue\", \"dim\"]",
        "[\"ice\", \"red\"]",
        "unknown colour `red`",
    );
    refused("(3, 7)])", "(3, 9)])", "a point it doesn't have");
    refused("shape: \"cube\"", "shape: \"ball\"", "unknown shape `ball`");
    refused("track: 16.0", "track: 16.5", "whole number of grid cells");
    refused("track: 16.0", "track: 12.0", "too short");
    refused("cycles: 2, phase", "cycles: 0, phase", "never comes back");
    refused(
        "text: \"ABC\", y: 2",
        "text: \"ABD\", y: 2",
        "the font has no `D`",
    );
    refused(
        "text: \"ABC\", y: 2",
        "text: \"ABCABCABCABCABCABCABC\", y: 2",
        "doesn't fit",
    );
    refused("y: 2, style", "y: 48, style", "sits in the band");
    refused("y: 84", "y: 66", "swings into the band");
    refused("y: 84", "y: 96", "off the screen");
    refused(
        "[\"A B C\"]",
        "[\"A\", \"B\", \"C\", \"A\", \"B\", \"C\"]",
        "taller than the band",
    );
    refused("secs: 4.0", "secs: 5.0", "doesn't go into the loop");
    refused("greetz: []", "greetz: [\"ABBA\"]", "isn't a crew");
    refused("\"TESTCREW\"", "\"PHOSPHOR\"", "a demo's crew is its own");
    refused("\"TESTCREW\"", "\"Phosphor\"", "a demo's crew is its own");
    refused(
        "\"TESTCREW\"",
        "\"the nightmoths\"",
        "a demo's crew is its own",
    );
    refused("('C', [\".##.\"", "('C', [\".###.\"", "glyph `C`");
    refused("scale: 2", "scale: 9", "scale 9");
    refused("style: \"logo\"", "style: \"huge\"", "unknown style `huge`");
}

#[test]
fn greetz_name_the_game_s_crews_handles_and_boards_in_any_case() {
    for name in [
        "THE NIGHTMOTHS",
        "phosphor",
        "KR4KEN",
        "THE NEON GUTTER",
        "RAZORWIRE",
        "D1RGE",
    ] {
        let src = SMALL.replacen("greetz: []", &format!("greetz: [\"{name}\"]"), 1);
        // The font only draws A, B and C, so only the greet is on trial.
        let src = src.replacen("('A',", &font_for(name), 1);
        parse(&src).unwrap_or_else(|e| panic!("{name}: {e}"));
    }
}

/// Blank glyphs for every letter in `name` the test font doesn't draw, then its `A`.
fn font_for(name: &str) -> String {
    let mut out = String::new();
    let mut seen = vec!['A', 'B', 'C', ' '];
    for c in name.chars() {
        if !seen.contains(&c) {
            seen.push(c);
            out.push_str(&format!("('{c}', [\"....\", \"....\", \"....\"]), "));
        }
    }
    out.push_str("('A',");
    out
}

#[test]
fn the_game_starts_from_straylight_s_intro_and_its_font_draws_every_word() {
    let data = Data::builtin().unwrap();
    let d = data.demo("ninsei").expect("data/demos/ninsei.ron");
    assert_eq!(d.group, "STRAYLIGHT");
    let mut words: Vec<String> = d.title.iter().map(|l| l.text.clone()).collect();
    words.extend(d.credits.pages.iter().flatten().cloned());
    words.push(d.scroller_text());
    for w in &words {
        for c in w.chars().filter(|&c| c != ' ') {
            assert!(
                d.font.glyphs.iter().any(|(g, _)| *g == c),
                "no `{c}` in the font, for \"{w}\""
            );
        }
    }
    assert!(!d.scroller.greetz.is_empty(), "it greets the scene");
    assert!(
        !d.scroller_text().contains("555"),
        "a launch screen gives no number away"
    );
}

#[test]
fn nothing_in_the_game_names_the_crew_that_cracked_it() {
    let src = Sources::builtin();
    let mut texts: Vec<(String, &str)> = vec![
        ("warez.ron".into(), src.warez),
        ("ice.ron".into(), src.ice),
        ("ai.ron".into(), src.ai),
        ("loot.ron".into(), src.loot),
        ("events.ron".into(), src.events),
        ("contracts.ron".into(), src.contracts),
        ("hub/tools.ron".into(), src.tools),
        ("hub/games.ron".into(), src.games),
        ("hub/compilations.ron".into(), src.compilations),
        ("hub/shopper.ron".into(), src.shopper),
        ("hub/hardware.ron".into(), src.hardware),
        ("hub/networks.ron".into(), src.networks),
        ("stacks/welcome.ron".into(), src.welcome),
    ];
    for b in src.boards {
        texts.push((format!("bbs/{}/board.ron", b.id), b.board));
        texts.push((format!("bbs/{}/splash.txt", b.id), b.splash));
    }
    for i in src.intros {
        texts.push((format!("intros/{}.ron", i.id), i.intro));
    }
    for t in src.tunes {
        texts.push((format!("music/{}.ron", t.id), t.tune));
    }
    for (file, text) in texts {
        assert!(
            !text.to_lowercase().contains("straylight"),
            "{file} names STRAYLIGHT, who exists only in the game's own intro"
        );
    }
}

#[test]
fn a_launch_opens_on_the_intro_unless_intros_are_off() {
    use ninsei_core::hub::{Save, Screen};
    let data = Data::builtin().unwrap();
    let mut save = Save::default();
    let before = save.to_text();
    assert_eq!(Screen::first(&save, &data), Screen::Intro);
    assert_eq!(save.to_text(), before, "it books nothing");
    save.intros = false;
    assert_eq!(Screen::first(&save, &data), Screen::Hub);
    // On the intro, nothing of the run or the desktop is up, and the desktop's music holds.
    assert!(!Screen::Intro.shows_run() && !Screen::Intro.hub_live());
    assert!(!ninsei_core::player::at_home(Screen::Intro));
    assert_eq!(Screen::Intro.tick(1000.0), Screen::Intro, "it stays up");
}

#[test]
fn the_game_needs_the_intro_it_starts_from() {
    let err = Data::parse(Sources {
        demos: &[],
        ..Sources::builtin()
    })
    .unwrap_err();
    assert!(err.contains("demos/ninsei.ron"), "{err}");
}

#[test]
fn only_the_credits_print_in_the_shipped_band() {
    // The band is where the game's menu will go: the flight runs behind it (it has no fill,
    // as signed off), and no text but the credits is ever printed in it.
    let data = Data::builtin().unwrap();
    let d = data.demo("ninsei").unwrap();
    let (top, bottom) = (d.band.y, d.band.y + d.band.h);
    for n in 0..320 {
        let f = d.frame(f64::from(d.secs) * f64::from(n) / 320.0);
        let credits: usize = f.page.map_or(0, |p| {
            d.credits.pages[p]
                .iter()
                .map(|l| l.chars().filter(|&c| c != ' ').count())
                .sum()
        });
        let inside = f
            .prints
            .iter()
            .filter(|p| p.y + 8 > top && p.y < bottom)
            .count();
        assert_eq!(inside, credits, "step {n}");
    }
}
