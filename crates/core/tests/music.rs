//! The run's music: how trace arranges a tune, and that a tune loops seamlessly.

use ninsei_core::intro::TuneDef;
use ninsei_core::music::{Arrangement, arrangement};
use ninsei_core::tuning::{MUSIC_DROP_AT, MUSIC_TEMPO_AT, MUSIC_TEMPO_BPM};

#[test]
fn trace_arranges_the_tune_in_two_coarse_steps() {
    let plain = Arrangement::PLAIN;
    let dropped = Arrangement {
        drop: Some(3),
        tempo: 0,
    };
    let urgent = Arrangement {
        drop: Some(3),
        tempo: MUSIC_TEMPO_BPM,
    };
    assert_eq!((MUSIC_DROP_AT, MUSIC_TEMPO_AT), (50, 75));
    assert_eq!(arrangement(0, Some(3)), plain);
    assert_eq!(arrangement(49, Some(3)), plain);
    assert_eq!(arrangement(50, Some(3)), dropped);
    assert_eq!(arrangement(74, Some(3)), dropped);
    assert_eq!(arrangement(75, Some(3)), urgent);
    assert_eq!(arrangement(100, Some(3)), urgent);
    // A tune with nothing to drop only speeds up.
    assert_eq!(arrangement(60, None), plain);
    assert_eq!(
        arrangement(100, None),
        Arrangement {
            drop: None,
            tempo: MUSIC_TEMPO_BPM
        }
    );
}

fn tune(text: &str) -> TuneDef {
    let t: TuneDef = ron::from_str(text).unwrap();
    t.validate().unwrap();
    t
}

const RATE: u32 = 8000;

fn two_tracks() -> TuneDef {
    // An 8-step lead over a 3-step drum: they line up every 24 steps.
    tune(
        "(bpm: 120, tracks: [(wave: Square, vol: 0.3, pattern: \"C4 . E4 G4 - . C5 .\"), \
         (wave: Noise, vol: 0.2, pattern: \". x .\")])",
    )
}

#[test]
fn a_loop_is_one_whole_cycle_and_seamless() {
    let t = two_tracks();
    assert_eq!(t.loop_steps(), 24);
    let a = t.render_loop(RATE, &Arrangement::PLAIN);
    let step = (t.step_secs() * RATE as f32) as usize;
    assert_eq!(a.len(), t.loop_steps() * step);
    assert_eq!(
        a,
        t.render_loop(RATE, &Arrangement::PLAIN),
        "same tune, same samples"
    );
    assert!(a.iter().all(|s| (-1.0..=1.0).contains(s)));
    assert!(a.iter().any(|s| s.abs() > 0.05), "it makes a sound");
    // The cycle starts on a fresh attack and ends on a release: no click where it wraps.
    assert!(a[0].abs() < 0.05 && a[1].abs() < 0.05, "{:?}", &a[..4]);
    assert!(a[a.len() - 50..].iter().all(|s| s.abs() < 0.05));
}

#[test]
fn a_note_held_over_the_end_rings_on_into_the_start() {
    // The note on the last step holds through the first: on repeat it never stops.
    let t = tune("(bpm: 120, tracks: [(wave: Triangle, vol: 0.5, pattern: \"- . C4 G4\")])");
    let a = t.render_loop(RATE, &Arrangement::PLAIN);
    let jump = |x: f32, y: f32| (x - y).abs();
    let inside = a.windows(2).map(|w| jump(w[0], w[1])).fold(0.0, f32::max);
    assert!(
        a[..10].iter().any(|s| s.abs() > 0.01),
        "the held note sounds at the start"
    );
    assert!(
        jump(a[a.len() - 1], a[0]) <= inside,
        "the seam is no worse than inside"
    );
}

#[test]
fn the_arrangement_changes_the_loop() {
    let t = two_tracks();
    let plain = t.render_loop(RATE, &Arrangement::PLAIN);
    let dropped = t.render_loop(
        RATE,
        &Arrangement {
            drop: Some(1),
            tempo: 0,
        },
    );
    assert_eq!(dropped.len(), plain.len());
    assert_ne!(dropped, plain, "the drums are gone");
    let faster = t.render_loop(
        RATE,
        &Arrangement {
            drop: None,
            tempo: MUSIC_TEMPO_BPM,
        },
    );
    assert!(
        faster.len() < plain.len(),
        "a faster tempo is a shorter loop"
    );
}
