//! Tunes on music disks (`data/music/`): they load, and `Data::validate` refuses a bad one.

use ninsei_core::data::{Data, TUNE_TITLE_MAX};
use ninsei_core::music::MAX_LOOP_SECS;

#[test]
fn the_tunes_load_and_validate() {
    let d = Data::builtin().unwrap();
    assert!(d.tunes.len() >= 4, "a few tunes ship");
    for t in &d.tunes {
        assert!(d.signs(&t.by), "{} by {}", t.id, t.by);
        assert!(t.tune.loop_secs(0) <= MAX_LOOP_SECS, "{}", t.id);
        assert!(t.tune.loop_secs(0) <= t.secs as f32, "{}", t.id);
        assert_eq!(
            t.loop_steps,
            t.tune.loop_steps(),
            "worked out when it loads"
        );
    }
    assert!(d.tune("ninsei-nights").is_some());
    assert!(d.tune("nope").is_none());
}

#[test]
fn the_crews_sign_their_tunes() {
    let d = Data::builtin().unwrap();
    // Groups joined with ` & ` sign as each name.
    assert!(d.signs("the NiGHTMOTHS"));
    assert!(d.signs("m0th"));
    assert!(d.signs("PHOSPHOR"));
    assert!(!d.signs("NiGHTMOTHS & m0th"));
    assert!(!d.signs("kr4ken"));
}

#[test]
fn an_unknown_author_is_refused() {
    let mut d = Data::builtin().unwrap();
    d.tunes[0].by = "kr4ken".into();
    let e = d.validate().unwrap_err();
    assert!(e.contains("signs no cracktro"), "{e}");
}

#[test]
fn a_loop_past_the_limit_is_refused() {
    let mut d = Data::builtin().unwrap();
    let t = &mut d.tunes[0];
    // Tracks of 61, 59 and 53 steps line up only after thousands of steps.
    t.tune.tracks.truncate(1);
    let track = t.tune.tracks[0].clone();
    for n in [61, 59, 53] {
        let mut k = track.clone();
        k.pattern = vec!["C4"; n].join(" ");
        t.tune.tracks.push(k);
    }
    t.tune.tracks.remove(0);
    t.secs = 600;
    let e = d.validate().unwrap_err();
    assert!(e.contains("loops after"), "{e}");
}

#[test]
fn a_tune_shorter_than_its_loop_or_too_long_is_refused() {
    let mut d = Data::builtin().unwrap();
    d.tunes[0].secs = 1;
    assert!(d.validate().unwrap_err().contains("at least one loop"));
    d.tunes[0].secs = 601;
    assert!(d.validate().unwrap_err().contains("at most"));
}

#[test]
fn a_duplicate_or_a_long_title_is_refused() {
    let mut d = Data::builtin().unwrap();
    let copy = d.tunes[0].clone();
    d.tunes.push(copy);
    assert!(d.validate().unwrap_err().contains("listed twice"));
    let mut d = Data::builtin().unwrap();
    d.tunes[0].title = "x".repeat(TUNE_TITLE_MAX + 1);
    assert!(d.validate().unwrap_err().contains("title"));
}
