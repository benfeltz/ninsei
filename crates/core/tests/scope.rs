//! The vis pane's maths: a tone peaks in its own bar, silence is empty, levels are right.

use ninsei_core::scope::{
    FALL_PER_SEC, FLOOR_DB, ballistics, bar_of, bar_range, db, height, level, peak, spectrum,
};

const RATE: u32 = 48_000;

fn tone(hz: f32, amp: f32, n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| amp * (std::f32::consts::TAU * hz * i as f32 / RATE as f32).sin())
        .collect()
}

#[test]
fn a_tone_peaks_in_its_own_bar() {
    for hz in [110.0, 440.0, 1000.0, 3000.0, 8000.0] {
        let bars = spectrum(&tone(hz, 0.5, 2048), RATE, 24);
        let at = bar_of(hz, 24, RATE).expect("in range");
        let top = (0..24)
            .max_by(|&a, &b| bars[a].total_cmp(&bars[b]))
            .unwrap();
        assert!(
            top.abs_diff(at) <= 1,
            "{hz} Hz: bar {top}, expected {at}: {bars:?}"
        );
        // Half scale is about -6 dB: near the top of the meter.
        assert!(
            bars[top] > 0.85 && bars[top] <= 1.0,
            "{hz} Hz: {}",
            bars[top]
        );
        // Far from the tone, the bars are low.
        let far = if at > 12 { 0 } else { 23 };
        assert!(bars[far] < 0.4, "{hz} Hz far bar {far}: {}", bars[far]);
    }
}

#[test]
fn silence_is_empty_and_short_windows_are_safe() {
    assert_eq!(spectrum(&[0.0; 2048], RATE, 16), vec![0.0; 16]);
    assert_eq!(spectrum(&[], RATE, 8), vec![0.0; 8]);
    assert_eq!(spectrum(&[0.3], RATE, 8), vec![0.0; 8]);
    assert!(
        spectrum(&tone(440.0, 0.5, 1000), RATE, 8)
            .iter()
            .all(|v| (0.0..=1.0).contains(v))
    );
    assert!(spectrum(&tone(440.0, 0.5, 2048), RATE, 0).is_empty());
}

#[test]
fn bars_tile_the_range_evenly_in_pitch() {
    let n = 20;
    for b in 0..n {
        let (lo, hi) = bar_range(b, n, RATE);
        assert!(lo < hi);
        if b > 0 {
            assert_eq!(bar_range(b - 1, n, RATE).1, lo, "no gaps");
        }
        assert_eq!(bar_of((lo + hi) / 2.0, n, RATE), Some(b));
    }
    assert_eq!(bar_of(20.0, n, RATE), None, "under the lowest");
    // At 8 kHz the top stops at Nyquist.
    assert!(bar_range(n - 1, n, 8000).1 <= 4000.5);
}

#[test]
fn levels_in_rms_peak_and_db() {
    assert_eq!(level(&[]), 0.0);
    assert!(
        (level(&[0.5, -0.5, 0.5, -0.5]) - 0.5).abs() < 1e-6,
        "a square"
    );
    let sine = tone(1000.0, 0.4, 4800);
    assert!((level(&sine) - 0.4 / 2f32.sqrt()).abs() < 1e-3);
    assert!((peak(&sine) - 0.4).abs() < 1e-3);
    assert_eq!(db(0.0), FLOOR_DB);
    assert!((db(1.0)).abs() < 1e-6);
    assert!((db(0.5) + 6.02).abs() < 0.01);
    assert_eq!(
        (height(FLOOR_DB), height(0.0), height(-30.0)),
        (0.0, 1.0, 0.5)
    );
}

#[test]
fn bars_jump_up_and_fall_slowly() {
    let shown = ballistics(&[0.0, 0.8], &[0.9, 0.0], 0.1);
    assert_eq!(shown[0], 0.9, "up at once");
    assert!(
        (shown[1] - (0.8 - FALL_PER_SEC * 0.1)).abs() < 1e-6,
        "down slowly"
    );
    // Never below what's playing, and it settles.
    let mut s = vec![1.0];
    for _ in 0..100 {
        s = ballistics(&s, &[0.2], 0.05);
    }
    assert_eq!(s, [0.2]);
    assert_eq!(ballistics(&[], &[0.5], 0.1), [0.5]);
}
