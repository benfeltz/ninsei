//! 138.2 acceptance: what the runner sees of the net's AI, hop by hop, through real dialed runs
//! (`Save::dial`, `Game::apply`), against the task's acceptance criteria.

use ninsei_core::cue::Cue;
use ninsei_core::hub::{Entry, Save};
use ninsei_core::netview::{ai_mark, route_tip};
use ninsei_core::*;

fn save(tier: u32) -> Save {
    let mut s = Save {
        program: Some(probe::CAREFUL.into()),
        ..Save::default()
    };
    s.ensure_key(0x1382);
    s.phonebook.push(Entry {
        id: 0,
        name: "probe".into(),
        business: "test".into(),
        number: "555-0000".into(),
        tier,
        traits: Vec::new(),
        heat: 0,
        runs: 0,
        last: Vec::new(),
    });
    s
}

/// Plays a fight or a conversation out, a bounded number of actions.
fn settle(g: &mut Game) {
    for _ in 0..80 {
        match g.phase {
            Phase::Combat => {
                g.apply(Action::Finish);
                if g.phase == Phase::Combat {
                    g.apply(Action::EndTurn);
                }
            }
            Phase::Encounter => {
                if let Some(c) = g.leave_choice() {
                    g.apply(Action::Choose {
                        choice: c,
                        slot: None,
                    });
                }
                g.apply(Action::EndEncounter);
            }
            _ => return,
        }
    }
}

#[test]
fn what_the_runner_sees_is_only_what_they_know_hop_by_hop() {
    let d = Data::builtin().unwrap();
    let (mut woke, mut marked, mut named) = (0, 0, 0);
    for tier in 1..=3 {
        for seed in 0..25 {
            let mut g = save(tier).dial(&d, 0, seed).unwrap();
            let mut wake_cues = 0;
            for _ in 0..40 {
                if g.phase != Phase::Exploring {
                    break;
                }
                // The map's mark is the runner's knowledge, and only that.
                match (ai_mark(&g), g.sighting()) {
                    (Some(m), Some(s)) => {
                        assert!(!g.in_subnet());
                        assert_eq!((m.node, m.now), (s.node(), s.now()), "seed {seed}");
                        marked += 1;
                    }
                    (None, s) => assert!(s.is_none() || g.in_subnet(), "seed {seed}"),
                    (Some(_), None) => panic!("seed {seed}: a mark from nothing"),
                }
                // The preview names it exactly where a step meets it, and that's where they
                // know it is now.
                let name = g
                    .boss
                    .as_ref()
                    .map(|b| g.data.ice[b.ice].name.clone())
                    .unwrap();
                for to in g.reachable() {
                    let meets = g.route(to).unwrap().meets;
                    let tip = route_tip(&g, to).unwrap();
                    assert_eq!(meets, tip.contains(&name), "seed {seed}: {tip}");
                    if meets {
                        let s = g.sighting().unwrap();
                        assert!(s.now() && s.node() == to);
                        named += 1;
                    }
                }
                let to = g.reachable()[0];
                g.cues.clear();
                g.apply(Action::Move(to));
                wake_cues += g.cues.iter().filter(|c| **c == Cue::AiWake).count();
                settle(&mut g);
            }
            // The wake: once, said once and sounded once, if it came.
            let line = g
                .ai_woke()
                .map(|(n, _)| format!("!! {n} is up on the net."));
            let said = line
                .as_ref()
                .map_or(0, |l| g.log.iter().filter(|x| &x.text == l).count());
            if g.ai_woke().is_some() {
                woke += 1;
                assert!(said <= 1 && wake_cues <= 1, "seed {seed}");
            } else {
                assert_eq!((said, wake_cues), (0, 0));
            }
        }
    }
    assert!(
        woke > 10 && marked > 10,
        "woke {woke} marked {marked} named {named}"
    );
}
