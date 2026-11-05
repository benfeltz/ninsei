use ninsei_core::bbs::{COLS, ROWS};
use ninsei_core::intro::{self, ART_ROWS};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

#[test]
fn every_tool_opens_with_its_own_intro() {
    let d = data();
    let mut seen: Vec<&str> = Vec::new();
    for t in &d.tools {
        let id = t.intro.as_deref().expect("every Gutter tool has an intro");
        assert!(d.intro(id).is_some(), "{}: no intro `{id}`", t.id);
        assert!(
            !seen.contains(&id),
            "{id} is used twice: each tool gets its own"
        );
        seen.push(id);
    }
}

#[test]
fn every_intro_screen_fits_the_terminal() {
    let d = data();
    for i in &d.intros {
        for step in 0..200 {
            let t = step as f32 * 0.05;
            for calm in [false, true] {
                let lines = i.screen(&d, t, calm);
                assert!(lines.len() <= ROWS, "{} at {t}: {} rows", i.id, lines.len());
                for l in &lines {
                    let w = ansi::visible_len(l);
                    assert!(w <= COLS, "{} at {t}: {w} wide: {}", i.id, ansi::plain(l));
                }
            }
        }
    }
}

#[test]
fn the_greetz_scroll_and_the_ad_names_the_board() {
    let d = data();
    let i = d.intro("wardial").unwrap();
    let a = i.screen(&d, 0.0, false);
    let b = i.screen(&d, 1.0, false);
    assert_ne!(a[ROWS - 1], b[ROWS - 1], "the scroller moved");
    assert!(a[ROWS - 2].contains("THE NEON GUTTER") && a[ROWS - 2].contains("555-0199"));
    let text = i.scroller(&d);
    assert!(text.contains("GREETZ") && text.contains("c4se"));
    // Calm scrolls at half speed.
    let fast = intro::window(&text, 2.0, false);
    let slow = intro::window(&text, 4.0, true);
    assert_eq!(fast, slow);
}

#[test]
fn calm_holds_the_first_frame_still() {
    let d = data();
    for i in &d.intros {
        let still = i.screen(&d, 0.0, true);
        for step in 1..40 {
            let t = step as f32 * 0.1;
            assert_eq!(
                i.screen(&d, t, true)[..ART_ROWS],
                still[..ART_ROWS],
                "{} moved at {t}",
                i.id
            );
        }
        assert!(i.done(i.secs) && !i.done(i.secs - 0.01));
    }
}

#[test]
fn a_bad_intro_is_caught() {
    let d = data();
    let mut i = d.intro("hexed").unwrap().clone();
    i.board = "nowhere".into();
    assert!(i.validate(&d).unwrap_err().contains("unknown board"));
    let mut i = d.intro("hexed").unwrap().clone();
    i.frames[0].push("x".repeat(COLS + 1));
    assert!(i.validate(&d).unwrap_err().contains("wide"));
    let mut i = d.intro("hexed").unwrap().clone();
    i.effects[0].to = ART_ROWS + 1;
    assert!(i.validate(&d).is_err());
}
