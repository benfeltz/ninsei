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

mod in_a_run {
    use ninsei_core::hub::{Save, Screen};
    use ninsei_core::music::{Arrangement, RunTune, wanted};
    use ninsei_core::tuning::MUSIC_DROP_AT;
    use ninsei_core::{Data, Game, Phase};

    /// The built-in data with a tune on every tier of the Ono-Sendai's pack.
    fn data() -> Data {
        let mut d = Data::builtin().unwrap();
        let tune: ninsei_core::intro::TuneDef = ron::from_str("(bpm: 120, tracks: [(wave: Square, vol: 0.2, pattern: \"C4 E4\"), (wave: Noise, vol: 0.1, pattern: \"x .\")])").unwrap();
        let ono = d.sounds.iter_mut().find(|p| p.id == "ono").unwrap();
        ono.music = (1..=3)
            .map(|tier| RunTune {
                tier,
                drop: Some(1),
                tune: tune.clone(),
            })
            .collect();
        d
    }

    /// A save on `deck`, dialed into its first number.
    fn dialed(d: &Data, deck: &str) -> (Save, Game) {
        let mut s = Save::default();
        s.ensure_key(1);
        s.ensure_phonebook(d, 1);
        s.ensure_hardware(d);
        s.deck = deck.into();
        s.firmware = s.deck_def(d).firmware[0].id.clone();
        let entry = s.phonebook[0].id;
        let g = s.dial(d, entry, 7).unwrap();
        (s, g)
    }

    #[test]
    fn the_run_plays_its_tier_until_it_ends() {
        let d = data();
        let (s, mut g) = dialed(&d, "onosendai");
        let tier = g.tier();
        let key = |screen, crashed, g: &Game| {
            wanted(screen, crashed, &s, g, &d).map(|w| (w.pack.to_string(), w.tier, w.arrangement))
        };
        assert_eq!(key(Screen::Hub, false, &g), None, "quiet at home");
        let playing = Some(("ono".to_string(), tier, Arrangement::PLAIN));
        assert_eq!(key(Screen::JackIn { t: 0.1 }, false, &g), playing);
        assert_eq!(key(Screen::Run, false, &g), playing);
        for phase in [Phase::Combat, Phase::Encounter] {
            g.phase = phase;
            assert_eq!(key(Screen::Run, false, &g), playing, "{phase:?}");
        }
        g.phase = Phase::Exploring;
        // The key changes once as trace crosses the drop step, then holds.
        g.deck.trace = MUSIC_DROP_AT - 1;
        assert_eq!(key(Screen::Run, false, &g), playing);
        let dropped = Some((
            "ono".to_string(),
            tier,
            Arrangement {
                drop: Some(1),
                tempo: 0,
            },
        ));
        for t in MUSIC_DROP_AT..MUSIC_DROP_AT + 5 {
            g.deck.trace = t;
            assert_eq!(key(Screen::Run, false, &g), dropped);
        }
        g.phase = Phase::Flatlined;
        assert_eq!(key(Screen::Run, false, &g), None, "a flatline stops it");
        assert_eq!(key(Screen::Crash, true, &g), None);
        g.phase = Phase::JackedOut;
        assert_eq!(key(Screen::Run, false, &g), None, "so does jacking out");
        assert_eq!(key(Screen::JackOut { t: 0.1 }, false, &g), None);
    }

    #[test]
    fn a_pack_without_music_is_valid_and_plays_nothing() {
        // Every deck's pack, its music taken out: still a valid pack, and the run is silent.
        let texts: Vec<String> = ninsei_core::data::BUILTIN_SOUNDS
            .iter()
            .map(|p| {
                let cues = &p.sounds[..p.sounds.rfind("music: [").expect("a music section")];
                format!("{cues}music: [],\n)")
            })
            .collect();
        let packs: Vec<_> = ninsei_core::data::BUILTIN_SOUNDS
            .iter()
            .zip(&texts)
            .map(|(p, t)| ninsei_core::data::PackSrc {
                id: p.id,
                sounds: t,
            })
            .collect();
        let d = Data::parse(ninsei_core::data::Sources {
            sounds: &packs,
            ..ninsei_core::data::Sources::builtin()
        })
        .unwrap();
        assert!(d.validate().is_ok());
        for deck in ["sensenet", "onosendai", "hosaka"] {
            let (s, g) = dialed(&d, deck);
            assert_eq!(g.phase, Phase::Exploring);
            assert!(wanted(Screen::Run, false, &s, &g, &d).is_none(), "{deck}");
        }
    }

    #[test]
    fn the_sense_net_plays_its_own_tune() {
        let d = Data::builtin().unwrap();
        let (s, g) = dialed(&d, "sensenet");
        let fw = s.firmware_def(&d);
        assert_eq!(
            (fw.label.as_str(), fw.sounds.as_str()),
            ("Liza 1.0", "sensenet")
        );
        let w = wanted(Screen::Run, false, &s, &g, &d).expect("music in a Liza run");
        assert_eq!((w.pack, w.tier), ("sensenet", g.tier()));
        assert_eq!(w.arrangement, Arrangement::PLAIN);
    }
}

/// A loop's loudest sample and its RMS.
fn level(a: &[f32]) -> (f32, f32) {
    let peak = a.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    // Summed in f64: millions of squares in an f32 would lose the quiet samples.
    let sum: f64 = a.iter().map(|&s| (s as f64).powi(2)).sum();
    (peak, (sum / a.len().max(1) as f64).sqrt() as f32)
}

#[test]
fn every_scored_pack_has_a_tune_per_tier_that_sits_under_a_cracktro() {
    const HI: u32 = 48_000;
    let d = ninsei_core::Data::builtin().unwrap();
    let scored: Vec<&str> = d
        .sounds
        .iter()
        .filter(|p| !p.music.is_empty())
        .map(|p| p.id.as_str())
        .collect();
    for pack in ["ono", "sensenet"] {
        assert!(scored.contains(&pack), "{pack} has music: {scored:?}");
    }
    // The quietest cracktro tune: run music plays under every one of them.
    let cracktro = d
        .intros
        .iter()
        .filter_map(|i| Some(level(&i.tune.as_ref()?.render(HI, i.secs)).1))
        .fold(f32::MAX, f32::min);
    assert!(cracktro < 1.0, "a cracktro has a tune");
    for pack in scored {
        for tier in 1..=3 {
            let at = format!("{pack} tier {tier}");
            let t = d.run_tune(pack, tier).expect("a tune for every tier");
            let secs = t.tune.loop_secs(0);
            assert!((24.0..=64.0).contains(&secs), "{at} loops after {secs:.1}s");
            assert!(t.drop.is_some(), "{at} has a track to drop");
            let check = |pct: u32, a: &[f32]| {
                let (peak, rms) = level(a);
                assert!(peak < 0.5, "{at} at {pct}% peaks at {peak:.3}: no clipping");
                assert!(
                    (0.035..=0.08).contains(&rms) && rms < cracktro,
                    "{at} at {pct}%: RMS {rms:.4} (a cracktro is {cracktro:.4})"
                );
                assert!(
                    a[a.len() - 50..].iter().all(|s| s.abs() < 0.01),
                    "{at} at {pct}% ends quiet"
                );
                assert!(
                    (a[a.len() - 1] - a[0]).abs() < 0.05,
                    "{at} at {pct}% wraps without a click"
                );
            };
            // 0% trace is the tune as written.
            let plain = t.tune.render_loop(HI, &Arrangement::PLAIN);
            check(0, &plain);
            for pct in [MUSIC_DROP_AT, MUSIC_TEMPO_AT] {
                let arr = arrangement(pct, t.drop);
                let a = t.tune.render_loop(HI, &arr);
                check(pct, &a);
                // Against the whole tune at the same tempo, so only the drop can differ. Not
                // assert_ne: a failure would print millions of samples.
                let whole = Arrangement { drop: None, ..arr };
                let thins = if whole == Arrangement::PLAIN {
                    a != plain
                } else {
                    a != t.tune.render_loop(HI, &whole)
                };
                assert!(thins, "{at} at {pct}% thins out");
                if pct >= MUSIC_TEMPO_AT {
                    assert!(a.len() < plain.len(), "{at} speeds up");
                }
            }
        }
    }
}

#[test]
fn a_loop_renders_the_same_however_it_is_sliced() {
    let t = two_tracks();
    let whole = {
        let mut job = t.loop_render(RATE, 0, Some(1));
        assert!(job.work(usize::MAX));
        job.finish()
    };
    let mut job = t.loop_render(RATE, 0, Some(1));
    let mut frames = 1;
    while !job.work(777) {
        frames += 1;
    }
    assert!(frames > 5, "it took slices");
    assert_eq!(job.finish(), whole);
    // The bed is the tune without the stem; together they're the whole tune.
    let (bed, stem) = whole;
    assert_eq!(bed.len(), stem.len());
    let dropped = t.render_loop(
        RATE,
        &Arrangement {
            drop: Some(1),
            tempo: 0,
        },
    );
    assert_eq!(bed, dropped);
    let all = t.render_loop(RATE, &Arrangement::PLAIN);
    assert!(
        bed.iter()
            .zip(&stem)
            .zip(&all)
            .all(|((b, s), a)| (b + s - a).abs() < 1e-5)
    );
    // No stem, nothing to hold.
    let mut job = t.loop_render(RATE, 0, None);
    while !job.work(usize::MAX) {}
    assert!(job.finish().1.is_empty());
}
