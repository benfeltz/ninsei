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
    for c in Cue::ALL {
        let def = d.sounds.iter().find(|s| s.cue == c).unwrap();
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
    let src = include_str!("../../../data/sounds.ron");
    let parse = |sounds: &str| {
        Data::parse(Sources {
            sounds,
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
    assert!(SoundDef::validate(&data().sounds).is_ok());
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
