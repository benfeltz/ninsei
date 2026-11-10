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
/// node, won or lost, then (if it's still going) a loss to Hunter to end it. Booked. A fight
/// won falls in the deck's first pass, before its ICE has a turn to log, so the sysops have no
/// logs to judge a draft by and ship the first: these tests are about the ladder, not the
/// drafts.
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
    let rung = |s: &Save| {
        s.ice_ladder
            .iter()
            .find(|r| r.id == "enigma")
            .map_or(0, |r| r.rung)
    };
    let mut shipped = Vec::new();
    let mut rungs = Vec::new();
    for _ in 0..p + m + j + j {
        run(&mut s, &d, PLAY, &[("enigma", true)]);
        shipped.push(s.ice_program("enigma", &d).unwrap().0);
        rungs.push(rung(&s));
    }
    let at = |n: u32| (shipped[n as usize - 1], rungs[n as usize - 1]);
    let v = |t: &str| t.to_string();
    // Nothing until the patch's wins are in, then a patch.
    assert_eq!(at(p - 1).0.to_string(), v("1.0.0"), "{shipped:?}");
    assert_eq!(at(p).0.to_string(), v("1.0.1"), "{shipped:?}");
    // The same play beat it: the next is a minor, then majors while it keeps winning. (A size
    // that can't be written falls back to a smaller one: a version always ships newer.)
    assert_eq!(at(p).1, 1);
    assert!(
        at(p + m).0 > at(p).0 && at(p + m).0.patch == 0,
        "{shipped:?}"
    );
    assert_eq!(at(p + m).1, 2);
    assert!(at(p + m + j).0 > at(p + m).0, "{shipped:?}");
    assert!(at(p + m + j + j).0 > at(p + m + j).0, "{shipped:?}");
    assert_eq!(at(p + m + j + j).1, 2);
    // Between rungs, nothing ships.
    assert_eq!(at(p + m - 1).0, at(p).0);
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
fn a_new_program_starts_the_count_over() {
    // Wins and logs earned by one program don't ship a patch against another.
    let mut r = hub::IceRung::default();
    let [p, ..] = tuning::LADDER_WINS;
    for _ in 0..p - 1 {
        r.climb(PLAY, 1, false, &[vec![tell(0, "Firewall")]]);
    }
    assert_eq!(r.climb(OTHER, 1, false, &[vec![tell(0, "Patch")]]), None);
    assert_eq!((r.wins, r.rung), (1, 0));
    assert_eq!(r.tells, vec![vec![tell(0, "Patch")]]);
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

/// A tell of a deck with this shield up, having run this program.
fn tell(shield: u32, ran: &str) -> ice_version::Tell {
    ice_version::Tell {
        integ: 20,
        shield,
        cpu: 1,
        ran: vec![(ran.to_string(), ran.to_ascii_lowercase())],
        strength: 9,
        turn: 2,
    }
}

#[test]
fn a_bump_is_written_against_every_win_since_the_last() {
    use ice_version::Bump;
    let mut r = hub::IceRung::default();
    let [p, ..] = tuning::LADDER_WINS;
    for i in 0..p - 1 {
        assert_eq!(r.climb(PLAY, 1, false, &[vec![tell(i, "Firewall")]]), None);
    }
    assert_eq!(
        r.climb(PLAY, 1, false, &[vec![tell(9, "Patch")]]),
        Some(Bump::Patch)
    );
    // Every fight it lost since the last bump, oldest first, is what it's written against.
    assert_eq!(r.tells.len() as u32, p);
    assert_eq!(r.tells[0], vec![tell(0, "Firewall")]);
    assert_eq!(r.tells.last(), Some(&vec![tell(9, "Patch")]));
    // Nothing moves on until a version ships: the same bump is due after the next win.
    assert_eq!((r.wins, r.rung), (p, 0));
    assert_eq!(
        r.climb(PLAY, 1, false, &[vec![tell(1, "Patch")]]),
        Some(Bump::Patch)
    );
    r.shipped();
    assert_eq!((r.wins, r.rung), (0, 1));
    assert!(r.tells.is_empty());
    // The logs keep the latest fights that fit in TELLS_KEPT turns.
    let long: Vec<_> = (0..tuning::TELLS_KEPT as u32 - 2)
        .map(|i| tell(i, "Patch"))
        .collect();
    r.climb(
        PLAY,
        2,
        false,
        &[vec![tell(0, "Firewall"); 4], long.clone()],
    );
    assert_eq!(r.tells, vec![long]);
}

#[test]
fn a_bump_that_ships_nothing_leaves_the_ladder_where_it_was() {
    // Logs a draft can't beat: Rototurret already hits for its range's most on every turn of
    // a deck that never shields, so no rewrite of its single line hurts more.
    let d = data();
    let mut s = deck(&d);
    s.ice_versions.push(hub::IceVer {
        id: "rototurret".into(),
        version: ice_version::Version::default().bumped(ice_version::Bump::Patch),
        program: "10 ATTACK 8".into(),
        bump: Some(ice_version::Bump::Patch),
    });
    let [p, ..] = tuning::LADDER_WINS;
    s.ice_ladder.push(hub::IceRung {
        id: "rototurret".into(),
        wins: p - 1,
        program: Some(PLAY.into()),
        tells: vec![vec![tell(0, "Icepick"); 6]],
        ..Default::default()
    });
    run(&mut s, &d, PLAY, &[("rototurret", true)]);
    assert_eq!(version(&s, &d, "rototurret"), "1.0.1");
    let r = s.ice_ladder.iter().find(|r| r.id == "rototurret").unwrap();
    assert_eq!((r.wins, r.rung), (p, 0));
    assert!(!r.tells.is_empty());
}

#[test]
fn the_sysops_ship_the_draft_that_would_have_hurt_the_play_most() {
    use ice_version::{Bump, harm, next_version, written_against};
    let d = data();
    let roto = d.ice.iter().find(|i| i.id == "rototurret").unwrap();
    // A deck that always shields: hits into the firewall do nothing.
    let logs = vec![vec![tell(10, "Firewall"); 10]];
    assert_eq!(harm(roto, "10 ATTACK 5", &logs), 0);
    // The shield takes the first 10 of a hit of 12.
    assert_eq!(harm(roto, "10 ATTACK 12", &logs), 10 * 2 * tuning::HARM_HIT);
    // A trace has no shield to hit.
    let hunter = d.ice.iter().find(|i| i.id == "hunter").unwrap();
    assert_eq!(
        harm(hunter, "10 TRACE 4", &logs),
        10 * 4 * tuning::HARM_TRACE
    );
    for seed in 0..40 {
        for bump in [Bump::Patch, Bump::Minor, Bump::Major] {
            let next = written_against(&d, hunter, &hunter.program, bump, &logs, seed);
            // What ships never hurt the logged play less than what it replaces.
            if next.bump.is_some() {
                assert!(
                    harm(hunter, &next.program, &logs) > harm(hunter, &hunter.program, &logs),
                    "{bump:?} {seed}:\n{}",
                    next.program
                );
                d.check_ice_program(hunter, &next.program).unwrap();
            } else {
                assert_eq!(next.program, hunter.program);
            }
        }
    }
    // With no logs, the first draft ships.
    let plain = written_against(&d, hunter, &hunter.program, Bump::Patch, &[], 3);
    assert_eq!(plain.bump, Some(Bump::Patch));
    assert_eq!(
        plain,
        next_version(&d, hunter, &hunter.program, Bump::Patch, &[], {
            let mut r = Rng::new(3);
            r.next_u64()
        })
    );
}

#[test]
fn a_draft_is_judged_on_the_passes_the_ice_would_run() {
    use ice_version::harm;
    let d = data();
    let roto = d.ice.iter().find(|i| i.id == "rototurret").unwrap();
    // Two turns of a fight, no shield: a cpu-1 ICE hits with its first line, then its second.
    let fight = vec![vec![tell(0, "Icepick"), tell(0, "Icepick")]];
    let h = |p: &str| harm(roto, p, &fight);
    assert_eq!(h("10 ATTACK 5\n20 ATTACK 4"), (5 + 4) * tuning::HARM_HIT);
    // So a draft that changes only its second line is judged by it.
    assert!(h("10 ATTACK 5\n20 ATTACK 8") > h("10 ATTACK 5\n20 ATTACK 4"));
    // The end of the program ends a turn; the next starts at the top.
    let three = vec![vec![tell(0, "Icepick"); 3]];
    assert_eq!(
        harm(roto, "10 ATTACK 5\n20 ATTACK 4", &three),
        (5 + 4 + 5) * tuning::HARM_HIT
    );
    // A fight starts again at the top.
    assert_eq!(
        harm(
            roto,
            "10 ATTACK 5\n20 ATTACK 4",
            &[fight[0].clone(), fight[0].clone()]
        ),
        2 * (5 + 4) * tuning::HARM_HIT
    );
    // A hit uses the shield up, and the next condition reads it gone (Archer: cpu 2).
    let archer = d.ice.iter().find(|i| i.id == "archer").unwrap();
    let shielded = vec![vec![tell(4, "Firewall")]];
    assert_eq!(
        harm(
            archer,
            "10 ATTACK 4\n20 IF TARGET SHIELD > 0\n30 TRACE 5\n40 ATTACK 6",
            &shielded
        ),
        6 * tuning::HARM_HIT
    );
}
