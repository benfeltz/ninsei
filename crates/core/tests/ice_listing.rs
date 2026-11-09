//! 141.3.2: the deck shows the ICE's program as it has seen it run, in a fight and in the
//! ICE DB.

use ninsei_core::hub::Save;
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

fn fresh() -> Save {
    let d = data();
    let mut s = Save::default();
    s.ensure_key(0x1432);
    s.ensure_phonebook(&d, 0);
    s.ensure_hardware(&d);
    s
}

/// A run from `save`, in a fight with `ice` too strong to fall.
fn fight_from(save: &mut Save, ice: &str) -> Game {
    let d = data();
    let mut g = save.jack_in(&d, 11);
    let target = g.net.nodes[g.at].links[0];
    let ix = g.data.ice.iter().position(|i| i.id == ice).unwrap();
    g.net.nodes[target].kind = NodeKind::Ice;
    g.net.nodes[target].ice = Some(ix);
    g.net.nodes[target].cleared = false;
    g.apply(Action::Move(target));
    assert_eq!(g.phase, Phase::Combat);
    let c = g.combat.as_mut().unwrap();
    (c.strength, c.strength_max) = (500, 500);
    g.apply(Action::Program(String::new()));
    g
}

fn ix(g: &Game, id: &str) -> usize {
    g.data.ice.iter().position(|i| i.id == id).unwrap()
}

#[test]
fn the_deck_remembers_the_lines_it_saw_the_ice_run() {
    let mut s = fresh();
    let mut g = fight_from(&mut s, "enigma");
    let e = ix(&g, "enigma");
    assert!(g.ice_seen_lines(e).is_empty(), "nothing seen yet");
    // No shield: line 10's IF is false, it skips 20 and attacks at 30.
    g.apply(Action::EndTurn);
    assert_eq!(g.ice_seen_lines(e), [10, 30]);
    g.apply(Action::EndTurn);
    assert_eq!(g.ice_seen_lines(e), [10, 30, 40], "sorted, each once");
}

#[test]
fn what_it_saw_comes_home_and_back_into_the_next_run() {
    let d = data();
    let mut s = fresh();
    let mut g = fight_from(&mut s, "enigma");
    g.apply(Action::EndTurn);
    // A flatline still brings it home.
    g.deck.integrity = 1;
    while g.phase == Phase::Combat {
        g.apply(Action::EndTurn);
    }
    assert_eq!(g.phase, Phase::Flatlined);
    s.record(&g, &[], &d.rig).unwrap();
    let seen = s
        .ice_seen
        .iter()
        .find(|x| x.id == "enigma")
        .unwrap()
        .clone();
    assert!(
        seen.lines.contains(&10) && seen.lines.contains(&30),
        "{seen:?}"
    );
    // The bestiary fills in either way, but only the ICE DB brings it into a fight.
    assert!(s.kit(&d).ice_known.is_empty(), "no ICE DB, no knowledge in the fight");
    s.tools.push("icedb".into());
    let kit = s.kit(&d);
    assert!(
        kit.ice_known
            .iter()
            .any(|(id, l)| id == "enigma" && *l == seen.lines)
    );
    let g = fight_from(&mut s, "enigma");
    assert_eq!(g.ice_seen_lines(ix(&g, "enigma")), seen.lines.as_slice());
}

#[test]
fn a_save_from_before_lines_loads() {
    let mut s = fresh();
    s.ice_seen.push(hub::IceSeen {
        id: "enigma".into(),
        met: 1,
        broken: 0,
        lo: 12,
        hi: 12,
        lines: vec![10],
    });
    let old = s
        .to_text()
        .replace("lines: [\n                10,\n            ],", "");
    assert!(
        !old.contains("lines: ["),
        "the field is gone from the text: {old}"
    );
    let back = Save::load(&old).expect("loads");
    assert_eq!(back.ice_seen[0].lines, Vec::<u32>::new());
}

#[test]
fn an_ice_never_seen_decompiles_to_question_marks() {
    let d = data();
    let enigma = d.ice.iter().find(|i| i.id == "enigma").unwrap();
    let rows = ice_listing::decompile(&enigma.program, &enigma.verbs, &[]);
    assert_eq!(rows.len(), 4);
    assert!(rows.iter().all(|r| r.shown() == ice_listing::UNSEEN));
    let rows = ice_listing::decompile(&enigma.program, &enigma.verbs, &[10, 30]);
    let shown: Vec<&str> = rows.iter().map(|r| r.shown()).collect();
    assert_eq!(shown, ["IF TARGET SHIELD > 0", "????", "ATTACK 4", "????"]);
    assert_eq!(ice_listing::known(&rows), (2, 4));
}

#[test]
fn the_fight_marks_the_ice_cursor_and_next_and_fills_in_as_it_runs() {
    let mut s = fresh();
    let mut g = fight_from(&mut s, "enigma");
    let rows = ice_listing::rows(&g);
    assert!(rows.iter().all(|r| r.text.is_none()), "nothing seen yet");
    let mark = |g: &Game, f: fn(&ice_listing::IceRow) -> bool| {
        ice_listing::rows(g).iter().find(|r| f(r)).map(|r| r.no)
    };
    assert_eq!(mark(&g, |r| r.cursor), Some(10));
    // NEXT is ATTACK 4 on line 30: marked before the deck has seen it.
    assert_eq!(mark(&g, |r| r.next), Some(30));
    let fw = g.slot_named("firewall").unwrap();
    g.apply(Action::Run(fw));
    assert_eq!(
        mark(&g, |r| r.next),
        Some(20),
        "the shield moved NEXT to the TRACE"
    );
    g.apply(Action::EndTurn);
    let rows = ice_listing::rows(&g);
    let shown: Vec<&str> = rows.iter().map(|r| r.shown()).collect();
    assert_eq!(shown, ["IF TARGET SHIELD > 0", "TRACE 10", "????", "????"]);
    assert_eq!(
        mark(&g, |r| r.cursor),
        Some(30),
        "it carries on after its TRACE"
    );
    // Lagged: no NEXT line.
    g.combat.as_mut().unwrap().stunned = true;
    assert_eq!(mark(&g, |r| r.next), None);
    // Outside a fight there's nothing to show.
    g.combat = None;
    g.phase = Phase::Exploring;
    assert!(ice_listing::rows(&g).is_empty());
}

#[test]
fn the_ice_db_prints_what_you_have_seen() {
    let d = data();
    let mut s = fresh();
    let mut g = fight_from(&mut s, "enigma");
    g.apply(Action::EndTurn);
    g.phase = Phase::JackedOut;
    s.record(&g, &[], &d.rig).unwrap();
    let e = icedb::entries(&s, &d)
        .into_iter()
        .find(|e| e.def.id == "enigma")
        .unwrap();
    let page = e.lines(&s, &d);
    assert!(
        page.iter()
            .any(|l| l.contains("decompiled, 2 of 4 lines seen")),
        "{page:#?}"
    );
    assert!(
        page.contains(&"   10 IF TARGET SHIELD > 0".to_string()),
        "{page:#?}"
    );
    assert!(page.contains(&"   20 ????".to_string()), "{page:#?}");
    assert!(!page.iter().any(|l| l.starts_with("moves")), "{page:#?}");
    // An unmet ICE has no listing.
    let unmet = icedb::entries(&s, &d)
        .into_iter()
        .find(|e| e.def.id == "hydra")
        .unwrap();
    assert!(unmet.listing().is_empty());
}

#[test]
fn the_ono_shows_a_window_of_the_listing_from_the_ice_cursor() {
    let mut s = fresh();
    let mut g = fight_from(&mut s, "enigma");
    g.apply(Action::EndTurn);
    // Enigma carries on from 40 after its first turn's ATTACK at 30.
    let w = btop_fight::ice_window(&g, 3);
    let lines: Vec<String> = w.iter().map(btop_fight::ice_line).collect();
    assert_eq!(
        lines,
        ["▶  40 ????", "·  10 if target shield > 0", "·  20 ????"],
        "from the cursor, wrapping to the top"
    );
    assert_eq!(btop_fight::ice_heading(&g), "PROG 2/4");
    // Never more rows than the program has.
    assert_eq!(btop_fight::ice_window(&g, 9).len(), 4);
}

#[test]
fn the_hosaka_lists_the_observed_procedure() {
    let mut s = fresh();
    let mut g = fight_from(&mut s, "enigma");
    assert_eq!(
        suite::procedure_title(&g),
        "OBSERVED PROCEDURE (0 OF 4 STEPS)"
    );
    g.apply(Action::EndTurn);
    let rows = suite::procedure(&g);
    let shown: Vec<(&str, &str, bool, bool)> = rows
        .iter()
        .map(|r| (r.step.as_str(), r.statement.as_str(), r.seen, r.next))
        .collect();
    assert_eq!(
        shown,
        [
            ("   10", "IF TARGET SHIELD > 0", true, false),
            ("   20", "????", false, false),
            ("   30", "ATTACK 4", true, false),
            (">  40", "????", false, true),
        ]
    );
    assert_eq!(suite::PROCEDURE_COLUMNS, ["STEP", "STATEMENT"]);
}

#[test]
fn a_fight_that_rezzes_on_the_node_that_just_fell_is_booked_and_its_lines_come_home() {
    let d = data();
    let mut s = fresh();
    let mut g = fight_from(&mut s, "enigma");
    // A client's setup: Enigma falls and Hunter rezzes on the same node, in one action.
    let was = history::Snap::of(&g);
    let hunter = ix(&g, "hunter");
    {
        let c = g.combat.as_mut().unwrap();
        c.ice = hunter;
        c.turn = 1;
        c.ice_prog = basic::parse_with(
            &d.ice[hunter].program,
            basic::Vocab::Ice(&d.ice[hunter].verbs),
        );
        c.ice_exec = Default::default();
    }
    let mut h = std::mem::take(&mut g.history);
    h.record(&was, &g, Action::Run(0), None);
    g.history = h;
    let met: Vec<(usize, bool)> = g.history.met.iter().map(|m| (m.ice, m.broken)).collect();
    assert_eq!(met, [(ix(&g, "enigma"), true), (hunter, false)]);
    // Its lines are booked to a new entry.
    g.apply(Action::EndTurn);
    assert!(!g.ice_seen_lines(hunter).is_empty());
    g.phase = Phase::JackedOut;
    s.record(&g, &[], &d.rig).unwrap();
    let seen = s
        .ice_seen
        .iter()
        .find(|x| x.id == "hunter")
        .expect("hunter booked");
    assert_eq!(seen.lines, g.ice_seen_lines(hunter));
}
