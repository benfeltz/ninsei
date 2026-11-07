//! Sound cues: the right actions make the right sounds, and every sound renders cleanly.

use ninsei_core::cue::{Cue, SoundDef};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

fn take(g: &mut Game) -> Vec<Cue> {
    std::mem::take(&mut g.cues)
}

#[test]
fn every_cue_renders_short_and_clean() {
    let d = data();
    let ids: Vec<&str> = d.sounds.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(
        ids,
        ["hosaka", "ono", "sensenet"],
        "a pack per file in data/sounds/"
    );
    for (pack, c) in d.sounds.iter().flat_map(|p| Cue::ALL.map(|c| (p, c))) {
        let def = pack.sounds.iter().find(|s| s.cue == c).unwrap();
        let a = def.render(44_100);
        assert!(!a.is_empty(), "{c:?}");
        assert!(a.len() <= 44_100, "{c:?} runs under a second");
        assert!(
            a.iter().all(|s| (-1.0..=1.0).contains(s)),
            "{c:?} stays in range"
        );
        assert!(a.iter().any(|s| s.abs() > 0.01), "{c:?} makes a sound");
        assert_eq!(a, def.render(44_100), "{c:?} sounds the same every time");
    }
}

#[test]
fn bad_sounds_are_caught() {
    let src = include_str!("../../../data/sounds/ono.ron");
    let parse = |text: &str| {
        // The edited Ono-Sendai pack, beside every other shipped pack.
        let mut packs = vec![data::PackSrc {
            id: "ono",
            sounds: text,
        }];
        packs.extend(
            data::BUILTIN_SOUNDS
                .iter()
                .filter(|p| p.id != "ono")
                .copied(),
        );
        Data::parse(Sources {
            sounds: &packs,
            ..Sources::builtin()
        })
    };
    let missing: String = src
        .lines()
        .filter(|l| !l.contains("cue: Hop,"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(parse(&missing).unwrap_err().contains("Hop"));
    assert!(
        parse(&src.replacen("vol: 0.18", "vol: 1.8", 1))
            .unwrap_err()
            .contains("volume")
    );
    assert!(
        parse(&src.replacen("ms: 30,", "ms: 3000,", 1))
            .unwrap_err()
            .contains("ms")
    );
    for p in &data().sounds {
        assert!(SoundDef::validate(&p.sounds).is_ok(), "{}", p.id);
    }
}

/// The Ono-Sendai pack with `music` in place of its music section, parsed beside the others.
fn with_music(music: &str) -> Result<Data, String> {
    let src = include_str!("../../../data/sounds/ono.ron");
    let at = src
        .find("    music: [")
        .expect("ono.ron has a music section");
    let text = format!("{}    music: [{music}],\n)\n", &src[..at]);
    let mut packs = vec![data::PackSrc {
        id: "ono",
        sounds: &text,
    }];
    packs.extend(
        data::BUILTIN_SOUNDS
            .iter()
            .filter(|p| p.id != "ono")
            .copied(),
    );
    Data::parse(Sources {
        sounds: &packs,
        ..Sources::builtin()
    })
}

const TUNE: &str = "(bpm: 120, tracks: [(wave: Square, vol: 0.2, pattern: \"C4 . E4 G4\"), (wave: Noise, vol: 0.1, pattern: \"x . x .\")])";

#[test]
fn a_pack_without_music_is_silent() {
    // An empty music section parses and validates, and every tier is silent.
    let d = with_music("").unwrap();
    assert!(d.validate().is_ok());
    assert!((1..=3).all(|t| d.run_tune("ono", t).is_none()));
    assert!(data().run_tune("nope", 1).is_none());
    // A tier with no tune is fine too.
    let d = with_music(&format!("(tier: 2, tune: {TUNE})")).unwrap();
    assert!(d.run_tune("ono", 2).is_some());
    assert!(d.run_tune("ono", 1).is_none());
}

#[test]
fn bad_music_is_caught() {
    let bad = |music: String, says: &str| {
        let err = with_music(&music).unwrap_err();
        assert!(err.starts_with("sounds/ono.ron: "), "{err}");
        assert!(err.contains(says), "{music}: {err}");
    };
    bad(format!("(tier: 4, tune: {TUNE})"), "unknown tier 4");
    bad(format!("(tier: 0, tune: {TUNE})"), "unknown tier 0");
    // Tracks that only line up after thousands of steps (9009 here: 17 minutes).
    let primes = "(bpm: 132, tracks: [(wave: Square, vol: 0.2, pattern: \"C4 . . . . . .\"), \
        (wave: Square, vol: 0.2, pattern: \"E4 . . . . . . . .\"), \
        (wave: Saw, vol: 0.2, pattern: \"C2 . . . . . . . . . .\"), \
        (wave: Noise, vol: 0.1, pattern: \"x . . . . . . . . . . . .\")])";
    bad(format!("(tier: 1, tune: {primes})"), "loops after 1024 s");
    bad(
        format!("(tier: 1, tune: {TUNE}), (tier: 1, tune: {TUNE})"),
        "two tunes",
    );
    bad(
        format!("(tier: 1, drop: Some(9), tune: {TUNE})"),
        "drops track 9",
    );
    bad(
        format!("(tier: 1, tune: {})", TUNE.replace("E4", "H4")),
        "bad step `H4`",
    );
    bad(
        format!("(tier: 1, tune: {})", TUNE.replace("bpm: 120", "bpm: 9")),
        "bpm",
    );
    assert!(with_music(&format!("(tier: 1, drop: Some(1), tune: {TUNE})")).is_ok());
}

#[test]
fn a_hop_beeps_and_a_refused_move_is_silent() {
    let mut g = Game::new(data(), 1);
    for n in &mut g.net.nodes {
        n.kind = NodeKind::Vault;
        n.cleared = true;
    }
    g.apply(Action::Move(9999));
    assert!(take(&mut g).is_empty(), "illegal actions make no sound");
    let to = g.reachable()[0];
    g.apply(Action::Move(to));
    assert_eq!(take(&mut g), vec![Cue::Hop]);
}

#[test]
fn a_fight_sounds_like_a_fight() {
    let d = data();
    for seed in 0..80 {
        let mut g = Game::new(d.clone(), seed);
        let to = g.reachable()[0];
        g.net.nodes[to].kind = NodeKind::Ice;
        g.net.nodes[to].cleared = false;
        g.net.nodes[to].ice = g.net.nodes[to].ice.or(Some(0));
        g.apply(Action::Move(to));
        if g.phase != Phase::Combat {
            continue;
        }
        let rez = take(&mut g);
        assert!(
            rez.contains(&Cue::Hop) && rez.contains(&Cue::IceRez),
            "{rez:?}"
        );
        let mut heard: Vec<Cue> = Vec::new();
        for _ in 0..40 {
            if g.phase != Phase::Combat {
                break;
            }
            let slots: Vec<usize> = (0..g.hdd.len()).filter(|&i| g.hdd[i].loaded).collect();
            for i in slots {
                g.apply(Action::Run(i));
            }
            if g.phase == Phase::Combat {
                g.apply(Action::EndTurn);
            }
            heard.extend(take(&mut g));
        }
        assert!(heard.contains(&Cue::Hit), "breakers hit: {heard:?}");
        match g.phase {
            Phase::Flatlined => assert!(heard.contains(&Cue::Flatline)),
            _ if g.combat.is_none() => assert!(heard.contains(&Cue::Derez), "{heard:?}"),
            _ => {}
        }
        if heard.contains(&Cue::Damage)
            || heard.contains(&Cue::Block)
            || heard.contains(&Cue::Shield)
        {
            return;
        }
    }
    panic!("no fight made the full set of sounds");
}

#[test]
fn the_run_ending_has_a_sound() {
    let mut g = Game::new(data(), 2);
    let exit = g
        .net
        .nodes
        .iter()
        .position(|n| n.kind == NodeKind::Exit)
        .unwrap();
    // Walk straight there with nothing in the way.
    for n in &mut g.net.nodes {
        if n.id != exit && n.id != g.at {
            n.kind = NodeKind::Vault;
            n.cleared = true;
        }
    }
    while g.phase == Phase::Exploring {
        let reach = g.reachable().to_vec();
        let next = *reach
            .iter()
            .find(|&&n| g.net.reachable_from(n)[exit] || n == exit)
            .unwrap();
        g.apply(Action::Move(next));
    }
    assert_eq!(g.phase, Phase::JackedOut);
    assert!(take(&mut g).contains(&Cue::JackOut));
}

#[test]
fn nobody_listening_keeps_the_queue_small() {
    let mut g = Game::new(data(), 3);
    for n in &mut g.net.nodes {
        n.kind = NodeKind::Vault;
        n.cleared = true;
    }
    while g.phase == Phase::Exploring && !g.reachable().is_empty() {
        let to = g.reachable()[0];
        g.apply(Action::Move(to));
    }
    assert!(g.cues.len() <= cue::QUEUE_MAX);
}

#[test]
fn each_deck_sounds_like_itself() {
    let d = data();
    let ono = &d.hardware.deck("onosendai").unwrap().firmware[0];
    let hosaka = &d.hardware.deck("hosaka").unwrap().firmware[0];
    assert_eq!(
        (ono.sounds.as_str(), hosaka.sounds.as_str()),
        ("ono", "hosaka")
    );
    let hit = |pack: &str| {
        d.sound_pack(pack)
            .unwrap()
            .iter()
            .find(|s| s.cue == Cue::Hit)
            .unwrap()
            .render(22_050)
    };
    assert_ne!(hit("ono"), hit("hosaka"), "two packs, two sounds");
    // A firmware can't name a pack that isn't there.
    let hw = include_str!("../../../data/hub/hardware.ron")
        .replace("sounds: \"hosaka\"", "sounds: \"nope\"");
    let err = Data::parse(Sources {
        hardware: &hw,
        ..Sources::builtin()
    })
    .unwrap_err();
    assert!(err.contains("unknown sound pack `nope`"), "{err}");
}
