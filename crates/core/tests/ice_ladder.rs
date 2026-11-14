//! 141.9.2: what a run books of how each kind was beaten, the bias the sysops lean their drafts
//! on, and seen lines per version. How a version is found is 141.9.4's (`ice_study.rs`).

use ninsei_core::hub::Save;
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

/// A run from `save` in a fight against `id`, strong enough to last, with the deck's program.
fn fight(save: &mut Save, d: &Data, id: &str, program: &str) -> Game {
    let mut g = save.jack_in(d, 3);
    let target = g.net.nodes[g.at].links[0];
    let ix = g.data.ice.iter().position(|i| i.id == id).unwrap();
    g.net.nodes[target].kind = NodeKind::Ice;
    g.net.nodes[target].ice = Some(ix);
    g.net.nodes[target].cleared = false;
    g.apply(Action::Move(target));
    assert_eq!(g.phase, Phase::Combat);
    let c = g.combat.as_mut().unwrap();
    (c.strength, c.strength_max) = (500, 500);
    g.apply(Action::Program(program.into()));
    g
}

fn deck(d: &Data) -> Save {
    let mut s = Save::default();
    s.ensure_key(0x9142);
    s.ensure_phonebook(d, 0);
    s.ensure_hardware(d);
    s
}

/// Plays one turn of the deck's program, then the ICE's: a pass that ends the program ends the
/// turn itself, and one that hands it over (a STOP, a hang) is ended here.
fn turn(g: &mut Game) {
    let now = g.combat.as_ref().map(|c| c.turn);
    g.apply(Action::Play);
    while g.pass_open() {
        g.apply(Action::Finish);
    }
    if g.phase == Phase::Combat && g.combat.as_ref().map(|c| c.turn) == now {
        g.apply(Action::EndTurn);
    }
}

#[test]
fn each_ice_turn_books_what_it_saw_of_the_deck() {
    let d = data();
    let mut s = deck(&d);
    let mut g = fight(&mut s, &d, "enigma", "10 RUN firewall\n20 RUN patch");
    turn(&mut g);
    turn(&mut g);
    let m = g.bouts.last().unwrap();
    assert_eq!(m.tells.len(), 2);
    let t = &m.tells[0];
    assert!(t.shield > 0, "{t:?}");
    assert_eq!(t.turn, 1);
    let ids: Vec<&str> = t.ran.iter().map(|(_, id)| id.as_str()).collect();
    assert_eq!(ids, ["firewall", "patch"]);
    assert_eq!(m.tells[1].turn, 2);
    assert_eq!(t.strength, 500);
}

#[test]
fn a_tell_reads_the_ice_s_conditions_as_the_fight_does() {
    // At the start of each ICE turn, every condition any ICE or the grammar can write reads the
    // same through the booked tell as through the fight.
    let d = data();
    let mut conds: Vec<basic::Cond> = Vec::new();
    for ice in &d.ice {
        let p = basic::parse_with(&ice.program, basic::Vocab::Ice(&ice.verbs));
        for l in &p.lines {
            if let basic::Stmt::If(c) = &l.stmt {
                conds.push(c.clone());
            }
        }
    }
    for class in IceClass::ALL {
        for r in d.ice_grammar.reads(class) {
            let (lo, hi) = r.n.unwrap_or((0, 0));
            for n in [lo, hi] {
                let basic::Stmt::If(c) = basic::parse_stmt_with(
                    &format!("IF {}", r.with(n)),
                    basic::Vocab::Ice(&basic::Verb::ALL),
                ) else {
                    panic!("{}", r.read)
                };
                conds.push(c);
            }
        }
    }
    let mut s = deck(&d);
    for program in ["10 RUN firewall\n20 RUN icepick", "10 RUN patch", ""] {
        let mut g = fight(&mut s, &d, "tollbooth", program);
        for _ in 0..4 {
            g.apply(Action::Play);
            while g.pass_open() {
                g.apply(Action::Finish);
            }
            let tell = g.tell();
            for c in &conds {
                assert_eq!(tell.holds(c), g.test(c), "{c:?} on {tell:?}");
            }
            g.apply(Action::EndTurn);
            assert_eq!(g.bouts.last().unwrap().tells.last(), Some(&tell));
        }
    }
}

#[test]
fn the_ice_that_ended_the_run_is_named() {
    let d = data();
    let mut s = deck(&d);
    let mut g = fight(&mut s, &d, "rototurret", "");
    let ix = g
        .data
        .ice
        .iter()
        .position(|i| i.id == "rototurret")
        .unwrap();
    assert_eq!(g.killed_by(), None);
    g.deck.integrity = 1;
    while g.phase == Phase::Combat {
        g.apply(Action::EndTurn);
    }
    assert_eq!(g.phase, Phase::Flatlined);
    assert_eq!(g.killed_by(), Some(ix));
    // A run still going names nobody.
    let g = s.jack_in(&d, 4);
    assert_eq!(g.killed_by(), None);
}

#[test]
fn an_encounter_fight_that_ends_the_run_names_its_fighter() {
    // The rival fights on a contact node, with no ICE of its own on the map.
    let d = data();
    let mut g = Game::new(d.clone(), 1);
    let to = g.net.nodes[g.at].links[0];
    g.net.nodes[to].kind = NodeKind::Contact;
    g.net.nodes[to].cleared = false;
    g.net.nodes[to].ice = None;
    g.apply(Action::Move(to));
    let ev = g.data.events.iter().position(|e| e.id == "rival").unwrap();
    g.encounter.as_mut().unwrap().event = ev;
    let c = g.data.events[ev]
        .choices
        .iter()
        .position(|c| matches!(c.effect, ChoiceEffect::Fight { .. }))
        .unwrap();
    g.apply(Action::Choose {
        choice: c,
        slot: None,
    });
    assert_eq!(g.phase, Phase::Combat);
    let rival = g.data.ice.iter().position(|i| i.id == "rival").unwrap();
    g.deck.integrity = 1;
    while g.phase == Phase::Combat {
        g.apply(Action::EndTurn);
    }
    assert_eq!(g.phase, Phase::Flatlined);
    assert_eq!(g.killed_by(), Some(rival));
    assert_eq!(
        g.bouts.last().map(|b| (b.ice, b.broken)),
        Some((rival, false))
    );
}

#[test]
fn a_trace_on_the_hop_is_no_ice_s_kill() {
    // Beat Rototurret, then hop onto another with the trace about to run out: the hop's trace
    // ends the run before the second fight begins.
    let d = data();
    let mut s = deck(&d);
    let mut g = fight(&mut s, &d, "rototurret", "");
    finish(&mut g, true);
    g.deck.trace = tuning::TRACE_MAX - 1;
    let target = g.net.nodes[g.at].links[0];
    let ix = g
        .data
        .ice
        .iter()
        .position(|i| i.id == "rototurret")
        .unwrap();
    g.net.nodes[target].kind = NodeKind::Ice;
    g.net.nodes[target].ice = Some(ix);
    g.net.nodes[target].cleared = false;
    g.apply(Action::Move(target));
    assert_eq!(g.phase, Phase::Flatlined);
    assert_eq!(g.killed_by(), None);
    assert_eq!(g.bouts.len(), 1);
    assert!(g.bouts[0].broken);
}

#[test]
fn the_drafts_lean_on_the_reads_the_play_walked_into() {
    use ice_version::{Tell, against, fired};
    let d = data();
    let shielded = Tell {
        integ: 20,
        shield: 5,
        cpu: 0,
        ran: vec![("Firewall".into(), "firewall".into())],
        strength: 9,
        turn: 2,
    };
    let patched = Tell {
        integ: 5,
        shield: 0,
        cpu: 1,
        ran: vec![("Patch".into(), "patch".into())],
        strength: 9,
        turn: 3,
    };
    // A deck that always shields walks into a code gate's TARGET SHIELD read every turn.
    let f = fired(
        &d,
        IceClass::CodeGate,
        &[shielded.clone(), shielded.clone()],
    );
    let w = against(&f, 2);
    let weight = |w: &[(String, u32)], r: &str| w.iter().find(|(x, _)| x == r).map(|(_, n)| *n);
    assert_eq!(weight(&w, "TARGET SHIELD > n"), Some(tuning::BIAS_WEIGHT));
    assert_eq!(weight(&w, "TARGET RAN firewall"), Some(tuning::BIAS_WEIGHT));
    assert_eq!(weight(&w, "TARGET RAN patch"), None);
    // One that patched half its turns, low, is half as far into TARGET RAN patch.
    let f = fired(&d, IceClass::Sentry, &[shielded.clone(), patched.clone()]);
    let w = against(&f, 2);
    assert_eq!(
        weight(&w, "TARGET RAN patch"),
        Some(tuning::BIAS_WEIGHT / 2)
    );
    assert_eq!(
        weight(&w, "TARGET INTEG < n"),
        Some(tuning::BIAS_WEIGHT / 2)
    );
    // No tells, no bias; and it's the same every time.
    assert!(against(&fired(&d, IceClass::Barrier, &[]), 0).is_empty());
    assert_eq!(
        fired(&d, IceClass::Sentry, std::slice::from_ref(&patched)),
        fired(&d, IceClass::Sentry, &[patched])
    );
}

/// Fights the open fight out: a win if `win` (the ICE drops to 1 and the deck picks at it),
/// else a flatline.
fn finish(g: &mut Game, win: bool) {
    if win {
        let c = g.combat.as_mut().unwrap();
        (c.strength, c.strength_max) = (1, 1);
        g.apply(Action::Program("10 RUN icepick".into()));
        turn(g);
        assert_ne!(g.phase, Phase::Combat);
    } else {
        g.deck.integrity = 1;
        while g.phase == Phase::Combat {
            g.apply(Action::EndTurn);
        }
        assert_eq!(g.phase, Phase::Flatlined);
    }
}

/// Ends a run that's still going: the bot walks into Rototurret and loses.
fn end_run(g: &mut Game) {
    if matches!(g.phase, Phase::Flatlined | Phase::JackedOut) {
        return;
    }
    let target = g.net.nodes[g.at].links[0];
    let ix = g.data.ice.iter().position(|i| i.id == "hunter").unwrap();
    g.net.nodes[target].kind = NodeKind::Ice;
    g.net.nodes[target].ice = Some(ix);
    g.net.nodes[target].cleared = false;
    g.apply(Action::Move(target));
    finish(g, false);
}

#[test]
fn meeting_a_newer_version_keeps_the_last_one_as_it_was_known() {
    use ice_version::{Bump, Version};
    let d = data();
    let mut s = deck(&d);
    s.ice_seen.push(hub::IceSeen {
        id: "enigma".into(),
        met: 2,
        broken: 2,
        lo: 12,
        hi: 12,
        lines: vec![10, 30],
        won: 2,
        ..Default::default()
    });
    let patch = Version::default().bumped(Bump::Patch);
    let text = "10 ATTACK 4\n20 IF TARGET SHIELD > 0\n30 TRACE 10\n40 TRACE 4";
    s.ice_versions.push(hub::IceVer {
        id: "enigma".into(),
        version: patch,
        program: text.into(),
        bump: Some(Bump::Patch),
    });
    let mut g = fight(&mut s, &d, "enigma", "");
    turn(&mut g);
    finish(&mut g, true);
    end_run(&mut g);
    s.record(&g, &[], &d.rig).unwrap();
    let e = s.ice_seen.iter().find(|x| x.id == "enigma").unwrap();
    assert_eq!((e.version, e.bump), (patch, Some(Bump::Patch)));
    assert_eq!(e.program, text);
    assert_eq!((e.won, e.lost, e.met), (1, 0, 3));
    assert!(e.lines.contains(&10), "{e:?}");
    assert_eq!(
        e.past,
        vec![hub::SeenVersion {
            version: Version::default(),
            bump: None,
            program: String::new(),
            lines: vec![10, 30],
            won: 2,
            lost: 0,
        }]
    );
    // The ICE DB decompiles the version the lines were seen on.
    let entry = icedb::entries(&s, &d)
        .into_iter()
        .find(|x| x.def.id == "enigma")
        .unwrap();
    assert_eq!(entry.program, Some(text));
}

#[test]
fn a_loss_counts_against_the_version_that_won_it() {
    let d = data();
    let mut s = deck(&d);
    let mut g = fight(&mut s, &d, "bulwark", "");
    finish(&mut g, false);
    s.record(&g, &[], &d.rig).unwrap();
    let b = s.ice_seen.iter().find(|x| x.id == "bulwark").unwrap();
    assert_eq!((b.won, b.lost), (0, 1));
    assert_eq!(b.version.to_string(), "1.0.0");
    assert!(b.past.is_empty());
}

#[test]
fn a_save_from_before_versions_per_seen_line_loads() {
    let mut s = Save::default();
    s.ice_seen.push(hub::IceSeen {
        id: "enigma".into(),
        met: 1,
        lines: vec![10],
        ..Default::default()
    });
    let text = s.to_text();
    let old: String = text
        .lines()
        .filter(|l| {
            let l = l.trim_start();
            !["bump:", "program: \"\"", "won:", "lost:", "past:"]
                .iter()
                .any(|f| l.starts_with(f))
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert_ne!(old, text);
    let back = Save::load(&old).unwrap();
    assert_eq!(back.ice_seen, s.ice_seen);
}

const PLAY: &str = "10 RUN firewall\n20 RUN icepick";
const OTHER: &str = "10 RUN patch\n20 RUN icepick";

#[test]
fn a_garbled_line_is_the_same_play() {
    use hub::same_play;
    let garbled = "10 RUN firewall\n20 #$%&@!?*";
    assert!(same_play(PLAY, garbled));
    assert!(same_play(garbled, PLAY));
    assert!(!same_play(PLAY, OTHER));
    assert!(!same_play(PLAY, "10 RUN firewall"));
    assert!(!same_play(PLAY, "10 RUN firewall\n30 RUN icepick"));
}

#[test]
fn a_fight_names_its_ice_with_the_version_in_force() {
    let d = data();
    let mut s = deck(&d);
    let g = fight(&mut s, &d, "enigma", "");
    let e = g.data.ice.iter().position(|i| i.id == "enigma").unwrap();
    assert_eq!(g.ice_label(e), "Enigma 1.0.0");
    assert!(
        g.log
            .iter()
            .any(|l| l.text.starts_with("!! Enigma 1.0.0 rezzed"))
    );
    s.ice_versions.push(hub::IceVer {
        id: "enigma".into(),
        version: ice_version::Version::default().bumped(ice_version::Bump::Patch),
        program: "10 ATTACK 4\n20 TRACE 4".into(),
        bump: Some(ice_version::Bump::Patch),
    });
    let g = fight(&mut s, &d, "enigma", "");
    assert_eq!(g.ice_label(e), "Enigma 1.0.1");
    let w: std::collections::HashMap<_, _> = suite::watches(&g).into_iter().collect();
    assert_eq!(w["ICE"], "ENIGMA 1.0.1");
    // The steps keep the bare name.
    let mut g = g;
    g.apply(Action::EndTurn);
    assert!(
        g.log
            .iter()
            .any(|l| l.text.starts_with("Enigma 10 ATTACK 4")),
        "{:?}",
        g.log
    );
}
