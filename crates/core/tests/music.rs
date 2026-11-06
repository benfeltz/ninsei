//! The run's music: how trace arranges a tune, and that a tune loops seamlessly.

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
