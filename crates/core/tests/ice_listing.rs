//! 141.3.2, made strict by 141.11.1: the deck shows an ICE's program only from the source it
//! owns, in a fight and in the ICE DB; fighting one shows none of it.

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

/// Owns the source of 1.0.0 of `ice`.
fn own(s: &mut Save, ice: &str) {
    assert!(s.learn_source(&data(), ice, ice_version::Version::FIRST));
}

/// Every statement of `ice`'s 1.0.0 program.
fn statements(ice: &str) -> Vec<String> {
    let d = data();
    let row = d.ice.iter().find(|i| i.id == ice).unwrap();
    basic::parse_with(&row.program, basic::Vocab::Ice(&row.verbs))
        .lines
        .iter()
        .map(|l| l.text.clone())
        .collect()
}

#[test]
fn fighting_an_ice_whose_source_you_dont_own_shows_none_of_its_program() {
    let d = data();
    let mut s = fresh();
    let mut g = fight_from(&mut s, "enigma");
    let lines = statements("enigma");
    // Long enough for it to run every line of its program, more than once.
    for _ in 0..8 {
        assert!(
            ice_listing::rows(&g).is_empty(),
            "no line, no count, no cursor"
        );
        g.apply(Action::EndTurn);
    }
    assert_eq!(g.phase, Phase::Combat);
    let log: Vec<&str> = g.log.iter().map(|l| l.text.as_str()).collect();
    assert!(
        log.iter().any(|l| l.starts_with("Enigma ")),
        "what it did is in the log: {log:#?}"
    );
    for l in &log {
        for stmt in &lines {
            assert!(!l.contains(stmt.as_str()), "{stmt:?} in the log: {l:?}");
        }
        assert!(
            !l.contains("Enigma 10") && !l.contains("Enigma 30"),
            "{l:?}"
        );
    }
    g.phase = Phase::JackedOut;
    s.record(&g, &[], &d.rig).unwrap();
    for e in icedb::entries(&s, &d) {
        for l in e.lines(&s, &d) {
            for stmt in &lines {
                assert!(!l.contains(stmt.as_str()), "{stmt:?} in the ICE DB: {l:?}");
            }
        }
    }
}

#[test]
fn an_owned_version_lists_whole_with_the_cursor_and_next() {
    let mut s = fresh();
    own(&mut s, "enigma");
    let mut g = fight_from(&mut s, "enigma");
    let shown = |g: &Game| -> Vec<String> {
        ice_listing::rows(g)
            .iter()
            .map(|r| r.text.clone())
            .collect()
    };
    let all = ["IF TARGET SHIELD > 0", "TRACE 10", "ATTACK 4", "TRACE 4"];
    assert_eq!(shown(&g), all, "whole from its first turn");
    let mark = |g: &Game, f: fn(&ice_listing::IceRow) -> bool| {
        ice_listing::rows(g).iter().find(|r| f(r)).map(|r| r.no)
    };
    assert_eq!(mark(&g, |r| r.cursor), Some(10));
    assert_eq!(mark(&g, |r| r.next), Some(30));
    let fw = g.slot_named("firewall").unwrap();
    g.apply(Action::Run(fw));
    assert_eq!(
        mark(&g, |r| r.next),
        Some(20),
        "the shield moved NEXT to the TRACE"
    );
    g.apply(Action::EndTurn);
    assert_eq!(shown(&g), all);
    assert_eq!(
        mark(&g, |r| r.cursor),
        Some(30),
        "it carries on after its TRACE"
    );
    // With its source the log names the line, as a debugger does.
    assert!(
        g.log
            .iter()
            .any(|l| l.text == "Enigma 20 TRACE 10: traces +10."),
        "{:#?}",
        g.log
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
fn owning_one_kind_says_nothing_of_another() {
    let mut s = fresh();
    own(&mut s, "rototurret");
    let g = fight_from(&mut s, "enigma");
    assert!(ice_listing::rows(&g).is_empty());
}

#[test]
fn the_telegraph_works_without_a_source() {
    let mut s = fresh();
    let mut g = fight_from(&mut s, "enigma");
    assert!(ice_listing::rows(&g).is_empty());
    assert_eq!(g.ice_next(), Some(Intent::Attack(4)));
    g.apply(Action::Program("10 IF NEXT ATTACK\n20 RUN firewall".into()));
    g.apply(Action::Step);
    g.apply(Action::Step);
    let log: Vec<&str> = g.log.iter().map(|l| l.text.as_str()).collect();
    assert!(log.contains(&"10 IF NEXT ATTACK: yes."), "{log:#?}");
    assert!(log.iter().any(|l| l.starts_with("Firewall up")), "{log:#?}");
    // It saw the shield and traced instead: the flip the telegraph is for, with no line shown.
    assert!(log.contains(&"Enigma traces +10."), "{log:#?}");
}

#[test]
fn a_save_from_before_sources_with_seen_lines_loads() {
    let mut s = fresh();
    s.ice_seen.push(hub::IceSeen {
        id: "enigma".into(),
        met: 1,
        lo: 12,
        hi: 12,
        ..Default::default()
    });
    let text = s.to_text();
    let old = text.replacen("id: \"enigma\",", "id: \"enigma\", lines: [10, 30],", 1);
    assert_ne!(old, text);
    let back = Save::load(&old).expect("loads");
    assert_eq!(back.ice_seen[0].met, 1);
    assert!(back.ice_sources.is_empty(), "what was seen is no source");
}

#[test]
fn the_ono_shows_a_window_of_the_listing_from_the_ice_cursor() {
    let mut s = fresh();
    own(&mut s, "enigma");
    let mut g = fight_from(&mut s, "enigma");
    g.apply(Action::EndTurn);
    // Enigma carries on from 40 after its first turn's ATTACK at 30.
    let rows = ice_listing::rows(&g);
    let w = btop_fight::ice_window(&rows, 3, true);
    let lines: Vec<String> = w.iter().map(btop_fight::ice_line).collect();
    assert_eq!(
        lines,
        [
            "▶  40 trace 4",
            "·  10 if target shield > 0",
            "·  20 trace 10"
        ],
        "from the cursor, wrapping to the top"
    );
    // Never more rows than the program has.
    assert_eq!(btop_fight::ice_window(&rows, 9, true).len(), 4);
}

#[test]
fn the_hosaka_lists_the_procedure_on_file() {
    let mut s = fresh();
    own(&mut s, "enigma");
    let mut g = fight_from(&mut s, "enigma");
    g.apply(Action::EndTurn);
    let rows = suite::procedure(&ice_listing::rows(&g));
    let shown: Vec<(&str, &str, bool)> = rows
        .iter()
        .map(|r| (r.step.as_str(), r.statement.as_str(), r.next))
        .collect();
    assert_eq!(
        shown,
        [
            ("   10", "IF TARGET SHIELD > 0", false),
            ("   20", "TRACE 10", false),
            ("   30", "ATTACK 4", false),
            (">  40", "TRACE 4", true),
        ]
    );
    assert_eq!(suite::PROCEDURE_COLUMNS, ["STEP", "STATEMENT"]);
}

#[test]
fn a_fight_that_rezzes_on_the_node_that_just_fell_is_booked() {
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
    // The rules book it as `start_combat` does.
    let mut bout = g.bouts.last().unwrap().clone();
    g.bouts.last_mut().unwrap().broken = true;
    bout.ice = hunter;
    g.bouts.push(bout);
    let mut h = std::mem::take(&mut g.history);
    h.record(&was, &g, Action::Run(0), None);
    g.history = h;
    let met: Vec<(usize, bool)> = g.history.met.iter().map(|m| (m.ice, m.broken)).collect();
    assert_eq!(met, [(ix(&g, "enigma"), true), (hunter, false)]);
    g.apply(Action::EndTurn);
    g.phase = Phase::JackedOut;
    s.record(&g, &[], &d.rig).unwrap();
    let seen = s
        .ice_seen
        .iter()
        .find(|x| x.id == "hunter")
        .expect("hunter booked");
    assert_eq!(seen.met, 1);
}

#[test]
fn each_deck_says_there_is_no_source_in_its_own_words() {
    use ice_listing::IceWords;
    let decks: [(&str, IceWords); 4] = [
        ("sense/net", stack_fight::ICE_WORDS),
        ("ono-sendai", btop_fight::ICE_WORDS),
        ("hosaka", suite::ICE_WORDS),
        ("classic", ice_listing::CLASSIC),
    ];
    assert_eq!(stack_fight::ICE_WORDS.none, "(none)");
    assert_eq!(btop_fight::ICE_WORDS.none, "[ no source available ]");
    assert_eq!(suite::ICE_WORDS.none, "PROGRAM HAS NO SYMBOL TABLE");
    for (deck, w) in decks {
        // What is there, never where to get one or that one can be got, never a count.
        let none = w.none.to_lowercase();
        for coach in [
            "download", "board", "bbs", "buy", "get", "find", "line", "seen", "disk",
        ] {
            assert!(!none.contains(coach), "{deck}: {:?}", w.none);
        }
        assert!(!none.chars().any(|c| c.is_ascii_digit()), "{deck}");
        assert!(!w.heading.chars().any(|c| c.is_ascii_digit()), "{deck}");
    }
    // Each its own.
    let all: std::collections::HashSet<&str> = decks.iter().map(|(_, w)| w.none).collect();
    assert_eq!(all.len(), decks.len());
}
