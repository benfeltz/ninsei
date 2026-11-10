//! 141.9.2: ICE that evolve between runs. What a run books of how each kind was beaten, the
//! bias that writes the next version against it, seen lines per version, and the ladder.

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
    let m = g.history.met.last().unwrap();
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
            assert_eq!(g.history.met.last().unwrap().tells.last(), Some(&tell));
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
fn the_next_version_is_written_against_the_play_that_won() {
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

/// One run from `save` with the deck's program `program`: each of `fights` in turn on the next
/// node, won or lost, then (if it's still going) a loss to Hunter to end it. Booked.
fn run(save: &mut Save, d: &Data, program: &str, fights: &[(&str, bool)]) -> Game {
    save.program = Some(program.to_string());
    let mut g = save.jack_in(d, 9);
    for &(id, win) in fights {
        let target = g.net.nodes[g.at].links[0];
        let ix = g.data.ice.iter().position(|i| i.id == id).unwrap();
        g.net.nodes[target].kind = NodeKind::Ice;
        g.net.nodes[target].ice = Some(ix);
        g.net.nodes[target].cleared = false;
        g.apply(Action::Move(target));
        assert_eq!(g.phase, Phase::Combat, "{id}");
        let c = g.combat.as_mut().unwrap();
        (c.strength, c.strength_max) = (500, 500);
        g.apply(Action::Program(program.into()));
        turn(&mut g);
        finish(&mut g, win);
        if !win {
            break;
        }
    }
    end_run(&mut g);
    save.record(&g, &[], &d.rig).unwrap();
    g
}

fn version(save: &Save, d: &Data, id: &str) -> String {
    save.ice_program(id, d).unwrap().0.to_string()
}

const PLAY: &str = "10 RUN firewall\n20 RUN icepick";
const OTHER: &str = "10 RUN patch\n20 RUN icepick";

#[test]
fn the_same_play_climbs_the_ladder_patch_minor_major() {
    let d = data();
    let mut s = deck(&d);
    let [p, m, j] = tuning::LADDER_WINS;
    let mut shipped = Vec::new();
    for _ in 0..p + m + j + j {
        run(&mut s, &d, PLAY, &[("enigma", true)]);
        shipped.push(version(&s, &d, "enigma"));
    }
    let at = |n: u32| shipped[n as usize - 1].clone();
    assert_eq!(at(p - 1), "1.0.0", "{shipped:?}");
    assert_eq!(at(p), "1.0.1", "{shipped:?}");
    assert_eq!(at(p + m), "1.1.0", "{shipped:?}");
    assert_eq!(at(p + m + j), "2.0.0", "{shipped:?}");
    // And majors while the same play keeps winning.
    assert_eq!(at(p + m + j + j), "3.0.0", "{shipped:?}");
    let v = s.ice_versions.iter().find(|v| v.id == "enigma").unwrap();
    assert_eq!(v.bump, Some(ice_version::Bump::Major));
}

#[test]
fn a_new_program_sends_the_ladder_back_to_a_patch() {
    let d = data();
    let mut s = deck(&d);
    let [p, ..] = tuning::LADDER_WINS;
    for _ in 0..p {
        run(&mut s, &d, PLAY, &[("enigma", true)]);
    }
    assert_eq!(version(&s, &d, "enigma"), "1.0.1");
    for _ in 0..p {
        run(&mut s, &d, OTHER, &[("enigma", true)]);
    }
    assert_eq!(version(&s, &d, "enigma"), "1.0.2");
}

#[test]
fn an_ice_that_kills_you_does_not_evolve_but_what_you_beat_first_does() {
    let d = data();
    let mut s = deck(&d);
    let [p, ..] = tuning::LADDER_WINS;
    for _ in 0..p - 1 {
        run(&mut s, &d, PLAY, &[("enigma", true), ("bulwark", true)]);
    }
    // The run that would earn both their patches: Bulwark falls, then Enigma kills the deck
    // after losing to it once.
    run(
        &mut s,
        &d,
        PLAY,
        &[("bulwark", true), ("enigma", true), ("enigma", false)],
    );
    assert_eq!(version(&s, &d, "bulwark"), "1.0.1");
    assert_eq!(version(&s, &d, "enigma"), "1.0.0");
    // Its ladder starts again: it takes a full rung of wins to patch it.
    for _ in 0..p - 1 {
        run(&mut s, &d, PLAY, &[("enigma", true)]);
    }
    assert_eq!(version(&s, &d, "enigma"), "1.0.0");
    run(&mut s, &d, PLAY, &[("enigma", true)]);
    assert_eq!(version(&s, &d, "enigma"), "1.0.1");
}

#[test]
fn a_version_is_in_force_from_the_next_jack_in_and_met_unknown() {
    let d = data();
    let mut s = deck(&d);
    s.tools.push("icedb".into());
    let [p, ..] = tuning::LADDER_WINS;
    let mut last = None;
    for _ in 0..p {
        last = Some(run(&mut s, &d, PLAY, &[("enigma", true)]));
    }
    // The run that earned it ran 1.0.0 to the end.
    let g = last.unwrap();
    let e = g.data.ice.iter().position(|i| i.id == "enigma").unwrap();
    assert_eq!(g.ice_version(e).to_string(), "1.0.0");
    // The next one runs 1.0.1, and knows none of its lines.
    assert!(s.kit(&d).ice_known.iter().all(|(id, _)| id != "enigma"));
    let mut next = fight(&mut s, &d, "enigma", PLAY);
    assert_eq!(
        next.combat.as_ref().unwrap().ice_version.to_string(),
        "1.0.1"
    );
    assert!(ice_listing::rows(&next).iter().all(|r| r.text.is_none()));
    turn(&mut next);
    assert_eq!(next.ice_version(e).to_string(), "1.0.1");
}

#[test]
fn the_same_save_ships_the_same_versions() {
    let d = data();
    let play = |key: u64| {
        let mut s = deck(&d);
        s.key = key;
        for _ in 0..12 {
            run(&mut s, &d, PLAY, &[("enigma", true), ("rototurret", true)]);
        }
        s.ice_versions
    };
    let a = play(5);
    assert!(!a.is_empty());
    assert_eq!(a, play(5));
    assert_ne!(a, play(6));
}

#[test]
fn a_bump_is_written_against_every_win_since_the_last() {
    use ice_version::Bump;
    let mut r = hub::IceRung::default();
    let [p, ..] = tuning::LADDER_WINS;
    let shield = vec![("TARGET SHIELD > n".to_string(), 2)];
    let patch = vec![("TARGET RAN patch".to_string(), 2)];
    for i in 0..p - 1 {
        let f = if i == 0 { &shield } else { &patch };
        assert_eq!(r.climb(PLAY, 1, false, f, 2), None);
    }
    let (bump, bias) = r.climb(PLAY, 1, false, &patch, 2).unwrap();
    assert_eq!(bump, Bump::Patch);
    // Every turn it lost on, the shield's share and the patch's both counted.
    let turns = 2 * p;
    let w = |n: u32| tuning::BIAS_WEIGHT * n / turns;
    assert!(
        bias.contains(&("TARGET SHIELD > n".to_string(), w(2))),
        "{bias:?}"
    );
    assert!(
        bias.contains(&("TARGET RAN patch".to_string(), w(2 * (p - 1)))),
        "{bias:?}"
    );
    assert_eq!((r.wins, r.turns, r.rung), (0, 0, 1));
    assert!(r.fired.is_empty());
}
