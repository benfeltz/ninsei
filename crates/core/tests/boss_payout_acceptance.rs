//! 138.3 acceptance: beating the net's AI, through real dialed runs (`Save::dial`,
//! `Game::apply`, `Save::record`) on every tier, against the task's acceptance criteria: the
//! payout once, the net's sources from its model, booked home whatever the run's end, and the
//! AI's page in the ICE DB. And a run that doesn't beat it gets none of that.

use ninsei_core::boss::Hunter;
use ninsei_core::game::BossState;
use ninsei_core::hub::{Entry, Save};
use ninsei_core::*;

fn save(tier: u32) -> Save {
    let mut s = Save {
        program: Some(probe::CAREFUL.into()),
        ..Save::default()
    };
    s.ensure_key(0x1383);
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

/// Plays the run on to its end: hop and settle a bounded number of hops, then trace it out.
fn play_out(g: &mut Game) {
    for _ in 0..60 {
        if g.phase != Phase::Exploring {
            break;
        }
        let Some(&to) = g.reachable().first() else {
            break;
        };
        g.apply(Action::Move(to));
        settle(g);
    }
    // Still in after all that (on the net, in a long fight, still talking): the trace runs out,
    // and the next hop or turn is a flatline.
    if g.phase != Phase::JackedOut && g.phase != Phase::Flatlined {
        g.deck.trace = tuning::TRACE_MAX;
    }
    for _ in 0..10 {
        match g.phase {
            Phase::Exploring => {
                let to = g.reachable().first().copied();
                g.apply(Action::Move(to.unwrap_or(g.net.nodes[g.at].links[0])));
            }
            Phase::Combat => g.apply(Action::EndTurn),
            Phase::Encounter => settle(g),
            _ => break,
        }
    }
    assert!(
        matches!(g.phase, Phase::JackedOut | Phase::Flatlined),
        "{:?} trace {} log {:?}",
        g.phase,
        g.deck.trace,
        g.log
            .iter()
            .rev()
            .take(8)
            .map(|l| &l.text)
            .collect::<Vec<_>>()
    );
}

fn from_model(g: &Game) -> String {
    g.data
        .boss_model(g.boss.as_ref().unwrap().ice)
        .unwrap()
        .id
        .clone()
}

#[test]
fn beating_it_pays_once_and_gives_up_its_nets_sources_on_every_tier() {
    let d = Data::builtin().unwrap();
    let mut beaten = 0;
    for tier in 1..=3 {
        for seed in 0..15 {
            let mut s = save(tier);
            let mut g = s.dial(&d, 0, seed).unwrap();
            let Some(&to) = g.reachable().first() else {
                continue;
            };
            // The AI awake on the runner's next node, holding: stepping on meets it.
            let ai = g.boss.as_ref().unwrap().ice;
            let mut h = Hunter::wake(ai, &g.data.ice[ai], to, g.at);
            h.prog = basic::parse_with("10 HOLD", g.data.ice[ai].vocab());
            g.boss.as_mut().unwrap().state = BossState::Awake(h);
            g.apply(Action::Move(to));
            assert_eq!(g.phase, Phase::Combat, "tier {tier} seed {seed}");
            // Kinds on the net's main network, at their versions in force as the fight opens.
            let mut kinds: Vec<(String, ice_version::Version)> = g
                .root()
                .nodes
                .iter()
                .filter_map(|n| n.ice)
                .filter(|&i| !g.data.ice[i].boss)
                .chain([ai])
                .map(|i| (g.data.ice[i].id.clone(), g.ice_version(i)))
                .collect();
            kinds.sort();
            kinds.dedup();
            let before = g.deck.credits;
            g.combat.as_mut().unwrap().strength = 1;
            let pick = g.slot_named("icepick").unwrap();
            g.apply(Action::Run(pick));
            assert_eq!(g.boss.as_ref().unwrap().state, BossState::Down);
            beaten += 1;

            // The payout, once, scaled by the net's pay.
            let name = g.data.ice[ai].name.clone();
            let pay = g.mods.pay(tuning::AI_PAY[tier as usize - 1]);
            let line = format!("+ {pay}¢ from {name}'s accounts.");
            assert_eq!(g.log.iter().filter(|l| l.text == line).count(), 1);
            assert!(g.deck.credits >= before + pay);

            // Every kind on its net and its own, at the version in force, from its model.
            let model = from_model(&g);
            for (id, v) in &kinds {
                assert!(
                    g.sources_got
                        .iter()
                        .any(|x| x.id == *id && x.version == *v && x.from == model),
                    "tier {tier} seed {seed}: no {id} {v}"
                );
            }
            assert_eq!(g.sources_got.len(), kinds.len(), "one source is one source");

            // It stays down for the run; the run plays on to whatever end it meets, and what it
            // gave up comes home all the same.
            play_out(&mut g);
            assert_eq!(g.boss.as_ref().unwrap().state, BossState::Down);
            // Never paid twice (the run's log keeps only its last lines, so the first may be
            // gone by now).
            assert!(
                g.log
                    .iter()
                    .filter(|l| l.text.contains("'s accounts."))
                    .count()
                    <= 1
            );
            s.record(&g, &[], &d.rig).unwrap();
            for (id, v) in &kinds {
                assert!(
                    s.ice_sources
                        .iter()
                        .any(|x| x.id == *id && x.version == *v && x.from == model),
                    "tier {tier} seed {seed} ({:?}): {id} {v} not home",
                    g.phase
                );
            }
            // Its page in the ICE DB: its maker's network AI, its program on file.
            let entries = icedb::entries(&s, &d);
            let e = entries.iter().find(|e| e.def.id == d.ice[ai].id).unwrap();
            let page = e.lines(&s, &d).join("\n");
            assert!(page.contains("network AI"), "{page}");
            assert!(page.contains("source         on file"), "{page}");
        }
    }
    assert!(beaten >= 40, "beaten {beaten}");
}

#[test]
fn a_run_that_doesnt_beat_it_gets_none_of_it() {
    let d = Data::builtin().unwrap();
    let mut runs = 0;
    for tier in 1..=3 {
        for seed in 100..115 {
            let mut s = save(tier);
            let mut g = s.dial(&d, 0, seed).unwrap();
            play_out(&mut g);
            if g.boss.as_ref().unwrap().state == BossState::Down {
                continue;
            }
            runs += 1;
            let ai = g.boss.as_ref().unwrap().ice;
            let name = g.data.ice[ai].name.clone();
            assert!(
                !g.log.iter().any(|l| l.text.contains(&format!("{name}'s"))),
                "tier {tier} seed {seed}"
            );
            let model = from_model(&g);
            assert!(g.sources_got.iter().all(|x| x.from != model));
            s.record(&g, &[], &d.rig).unwrap();
            assert!(s.ice_sources.iter().all(|x| x.from != model));
            // An AI never met has no page; one met, but not beaten, lists no code.
            let entries = icedb::entries(&s, &d);
            match entries.iter().find(|e| e.def.id == d.ice[ai].id) {
                None => assert!(!s.ice_seen.iter().any(|x| x.id == d.ice[ai].id)),
                Some(e) => {
                    let page = e.lines(&s, &d).join("\n");
                    assert!(page.contains("source         none"), "{page}");
                }
            }
        }
    }
    assert!(runs >= 30, "runs {runs}");
}
