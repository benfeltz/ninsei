//! 144 acceptance: the game's own intro, against the task's acceptance criteria, on the shipped
//! data: a first-person flight over a glowing grid with wireframe ICE passing, a logo, centred
//! credits and a scroller shaded and cycling white to ice blue, with a tune; it stays up and
//! never plays with intros off; it changes nothing in the save or in play; and everything it
//! shows is its data file's.

use ninsei_core::data::DemoSrc;
use ninsei_core::hub::{Save, Screen};
use ninsei_core::player::{self, INTRO_TUNE, Transport};
use ninsei_core::{Data, Game, Sources};

/// The loop at `n` evenly spaced moments.
fn moments(secs: f32, n: u32) -> impl Iterator<Item = f64> {
    (0..n).map(move |k| f64::from(secs) * f64::from(k) / f64::from(n))
}

#[test]
fn ac1_the_flight_the_ice_the_type_and_the_tune_are_on_screen() {
    let data = Data::builtin().unwrap();
    let d = data.launch_demo();
    let mut flat = d.clone();
    flat.solids.clear();
    let (mut ice_seen, mut rolls, mut pages) = (0, Vec::new(), Vec::new());
    for t in moments(d.secs, 256) {
        let f = d.frame(t);
        let grid = flat.frame(t).segs.len();
        assert!(grid > 100, "{t}: the grid is always there");
        // A glowing grid: its near lines carry a halo.
        assert!(f.segs.iter().any(|s| s.glow.is_some()), "{t}: no glow");
        if f.segs.len() > grid {
            ice_seen += 1;
        }
        // The logo is always up, centred.
        assert!(f.prints.iter().any(|p| p.y < d.band.y), "{t}: no logo");
        if let Some(p) = f.page {
            pages.push(p);
        }
        let scroll: Vec<_> = f
            .prints
            .iter()
            .filter(|p| p.y > d.band.y + d.band.h)
            .collect();
        if let Some(p) = scroll.first() {
            rolls.push(p.roll);
        }
    }
    assert!(
        ice_seen > 64,
        "ICE passes for much of the loop ({ice_seen}/256)"
    );
    pages.dedup();
    assert!(
        pages.len() >= d.credits.pages.len(),
        "every page of credits shows: {pages:?}"
    );
    rolls.sort();
    rolls.dedup();
    assert!(rolls.len() > 2, "the scroller's shade cycles: {rolls:?}");
    // Shaded white at the top of the logo, ice blue lower down.
    let px = d.pixels(&d.frame(0.0));
    let rgb = d.rgb();
    let logo = d.title.iter().find(|l| l.style == "logo").unwrap();
    let row = |y: i32| -> Vec<[u8; 3]> {
        (0..d.width as i32)
            .map(|x| px[(y * d.width as i32 + x) as usize])
            .filter(|&c| c != 0)
            .map(|c| rgb[c as usize])
            .collect()
    };
    assert!(row(logo.y).contains(&[255, 255, 255]), "white on top");
    let low = row(logo.y + 28);
    assert!(
        low.iter().any(|c| c[2] >= c[0] + 100 && c[2] > 200),
        "ice blue below: {low:?}"
    );
    // The tune is there and loops with the picture.
    let loops = d.secs / d.tune.loop_secs(0);
    assert!((loops - loops.round()).abs() < 1e-3 && loops >= 1.0);
}

#[test]
fn ac2_it_stays_up_and_with_intros_off_never_plays() {
    let mut s = Save::default();
    let first = Screen::first(&s);
    assert!(first.is_intro());
    let mut screen = first;
    for _ in 0..10_000 {
        screen = screen.tick(0.1);
    }
    assert!(screen.is_intro(), "nothing ends it but the player");
    s.intros = false;
    assert_eq!(Screen::first(&s), Screen::Hub);
}

#[test]
fn ac3_it_changes_nothing_in_the_save_or_in_play() {
    let data = Data::builtin().unwrap();
    let save = Save::default();
    let before = save.to_text();
    let a = Game::new(std::sync::Arc::new(data.clone()), 0x144);
    // Everything the intro does at home: decide the screen, draw it, choose the music.
    let screen = Screen::first(&save);
    let d = data.launch_demo();
    for t in moments(d.secs, 32) {
        d.pixels(&d.frame(t));
        let w = player::wanted(&save, &data, Screen::Intro { t }, &Transport::default());
        assert_eq!(w.map(|w| w.id), Some(INTRO_TUNE));
    }
    assert!(screen.is_intro());
    assert_eq!(save.to_text(), before, "the save is untouched");
    let b = Game::new(std::sync::Arc::new(data.clone()), 0x144);
    assert_eq!(
        format!("{:?}", a.net),
        format!("{:?}", b.net),
        "same seed, same network"
    );
}

#[test]
fn ac4_what_it_shows_is_its_data_file_s() {
    let src = include_str!("../../../data/demos/ninsei.ron");
    let parse = |text: &str| {
        let demos = [DemoSrc {
            id: "ninsei",
            demo: text,
        }];
        Data::parse(Sources {
            demos: &demos,
            ..Sources::builtin()
        })
        .unwrap()
    };
    let base = parse(src);
    let d = base.launch_demo();
    // The palette.
    let warm = parse(&src.replacen("(\"grid0\", \"5CF\")", "(\"grid0\", \"F80\")", 1));
    assert_ne!(warm.launch_demo().rgb(), d.rgb());
    // The shapes and where they sit.
    let gone = parse(&src.replacen(
        "size: 0.6, spin: (0, 8, 3)",
        "size: 0.1, spin: (0, 8, 3)",
        1,
    ));
    let t = 0.5;
    assert_ne!(gone.launch_demo().frame(t).segs, d.frame(t).segs);
    // The font.
    let font = parse(&src.replacen("\"#######.\",", "\"######..\",", 1));
    assert_ne!(
        font.launch_demo().pixels(&font.launch_demo().frame(t)),
        d.pixels(&d.frame(t))
    );
    // The text.
    let text = parse(&src.replacen("ANOTHER FINE RELEASE", "ANOTHER GOOD RELEASE", 1));
    assert_ne!(text.launch_demo().frame(0.5).prints, d.frame(0.5).prints);
    // The tune.
    let tune = parse(&src.replacen("bpm: 120", "bpm: 240", 1));
    assert_eq!(tune.launch_demo().tune.bpm, 240);
}
