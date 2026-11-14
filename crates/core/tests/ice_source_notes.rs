//! 141.11.3: a source carries the header and remarks of the board it came from, shown with
//! its listing on every deck and in the ICE DB, never part of the ICE's program.

use ninsei_core::data::Drop;
use ninsei_core::hub::{IceSource, Save};
use ninsei_core::ice_version::Version;
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

fn fresh(d: &Data) -> Save {
    let mut s = Save::default();
    s.ensure_key(0x1411_3000);
    s.ensure_phonebook(d, 0);
    s.ensure_hardware(d);
    s
}

/// Enigma 1.0.0's source, as if bought on board `from` (empty: from no board).
fn owned(d: &Data, from: &str) -> Save {
    let mut s = fresh(d);
    s.ice_sources.push(IceSource {
        id: "enigma".into(),
        version: Version::FIRST,
        program: String::new(),
        from: from.into(),
    });
    s
}

/// A run from `s`, in a fight with Enigma, too strong to fall.
fn fight(s: &mut Save, d: &Data) -> Game {
    let mut g = s.jack_in(d, 11);
    let n = g.net.nodes[g.at].links[0];
    g.net.nodes[n].kind = NodeKind::Ice;
    g.net.nodes[n].ice = Some(d.ice_by_id("enigma").unwrap());
    g.net.nodes[n].cleared = false;
    g.apply(Action::Move(n));
    let c = g.combat.as_mut().unwrap();
    (c.strength, c.strength_max) = (500, 500);
    g.apply(Action::Program(String::new()));
    g
}

#[test]
fn a_source_from_a_board_lists_with_its_header_and_remarks() {
    let d = data();
    let mut s = owned(&d, "gutter");
    let g = fight(&mut s, &d);
    let rows = ice_listing::rows(&g);
    // The header first, then the program, a remark under the IF it knows.
    assert_eq!(
        rows.iter()
            .take_while(|r| r.rem)
            .map(|r| r.text.as_str())
            .collect::<Vec<_>>(),
        [
            "Enigma 1.0.0 // NiGHTMOTHS",
            "spread it free on THE NEON GUTTER",
            "greetz c4se ratz zero_kool + kr4ken"
        ]
    );
    let code: Vec<&str> = rows
        .iter()
        .filter(|r| !r.rem)
        .map(|r| r.text.as_str())
        .collect();
    assert_eq!(
        code,
        ["IF TARGET SHIELD > 0", "TRACE 10", "ATTACK 4", "TRACE 4"]
    );
    let at = rows
        .iter()
        .position(|r| r.text == "IF TARGET SHIELD > 0")
        .unwrap();
    assert!(rows[at + 1].rem && rows[at + 1].text.contains("firewall"));
    // A remark row never carries the cursor or NEXT, and has no number.
    assert!(
        rows.iter()
            .filter(|r| r.rem)
            .all(|r| !r.cursor && !r.next && r.no == 0)
    );
    assert!(rows.iter().any(|r| r.cursor) && rows.iter().any(|r| r.next));
}

#[test]
fn the_notes_are_never_the_ice_s_program() {
    let d = data();
    let (mut a, mut b) = (owned(&d, "gutter"), owned(&d, ""));
    let (mut g, mut h) = (fight(&mut a, &d), fight(&mut b, &d));
    assert_eq!(
        g.combat.as_ref().unwrap().ice_prog,
        h.combat.as_ref().unwrap().ice_prog
    );
    for _ in 0..6 {
        g.apply(Action::EndTurn);
        h.apply(Action::EndTurn);
    }
    let log = |g: &Game| g.log.iter().map(|l| l.text.clone()).collect::<Vec<_>>();
    assert_eq!(log(&g), log(&h), "the same fight, turn for turn");
    assert_eq!(
        (g.deck.integrity, g.deck.trace),
        (h.deck.integrity, h.deck.trace)
    );
    // A source from no board lists bare.
    assert!(ice_listing::rows(&h).iter().all(|r| !r.rem));
}

#[test]
fn each_deck_prints_a_remark_its_own_way() {
    let d = data();
    let mut s = owned(&d, "deadsector");
    let mut g = fight(&mut s, &d);
    g.apply(Action::EndTurn);
    let rows = ice_listing::rows(&g);
    let rem = rows.iter().find(|r| r.rem).unwrap();
    assert_eq!(rem.text, "RAZORWIRE // Enigma 1.0.0");
    assert_eq!(
        ice_listing::card_line(rem),
        "       REM RAZORWIRE // Enigma 1.0.0"
    );
    assert_eq!(
        btop_fight::ice_line(rem),
        "·     rem razorwire // enigma 1.0.0"
    );
    let p = suite::procedure(&rows);
    let first = &p[0];
    assert_eq!(
        (first.step.as_str(), first.statement.as_str()),
        ("", "REM RAZORWIRE // Enigma 1.0.0")
    );
    assert!(first.rem && !first.next);
    // The Ono's window starts at the line it carries on from, a code row.
    let w = btop_fight::ice_window(&rows, 3, true);
    assert!(w[0].cursor && !w[0].rem);
}

#[test]
fn the_ice_db_lists_the_notes_with_the_source() {
    let d = data();
    let mut s = owned(&d, "corplink");
    s.ice_seen.push(hub::IceSeen {
        id: "enigma".into(),
        met: 1,
        ..Default::default()
    });
    let e = icedb::entries(&s, &d)
        .into_iter()
        .find(|e| e.def.id == "enigma")
        .unwrap();
    let page = e.lines(&s, &d);
    let top = page.iter().position(|l| l.starts_with("source")).unwrap();
    assert_eq!(page[top + 1], "       REM DOCUMENT CONTROL: Enigma 1.0.0");
    assert!(
        page.iter()
            .any(|l| l == "       REM Reviewer: reacts to firewalls."),
        "{page:#?}"
    );
    let at = page
        .iter()
        .position(|l| l.ends_with("IF TARGET SHIELD > 0"))
        .unwrap();
    assert!(page[at + 1].contains("Reviewer"));
}

#[test]
fn the_office_s_notes_are_the_office_s() {
    let d = data();
    let c = d.boards[d.board("corplink").unwrap()]
        .sources
        .clone()
        .unwrap();
    let words: Vec<String> = c
        .header
        .iter()
        .cloned()
        .chain(c.remarks.iter().map(|r| r.text.clone()))
        .collect();
    let handles: Vec<String> = d
        .boards
        .iter()
        .filter(|b| b.id != "corplink")
        .flat_map(|b| b.users.iter().map(|u| u.handle.to_lowercase()))
        .collect();
    for w in &words {
        let low = w.to_lowercase();
        assert!(
            !low.contains("hack") && !low.contains("crack") && !low.contains("rip"),
            "{w}"
        );
        for h in &handles {
            assert!(!low.contains(h.as_str()), "{w} names {h}");
        }
    }
}

#[test]
fn a_source_bought_on_a_board_remembers_it() {
    let d = data();
    let mut s = fresh(&d);
    s.bank = 1_000;
    s.learn_board("deadsector", "test");
    let b = d.board("deadsector").unwrap();
    let call = s.home_call(&d, &d.rig, b);
    let a = ninsei_core::bbs::Line::board(&call).areas.len() - 1;
    let f = ninsei_core::bbs::Line::board(&call).areas[a]
        .files
        .iter()
        .position(|f| matches!(&f.item, Drop::Source { ice, .. } if ice == "hydra"))
        .unwrap();
    s.home_buy_shown(&d, &d.rig, &call, a, f).unwrap();
    assert_eq!(s.ice_sources[0].from, "deadsector");
    // A save from before sources had a board loads, the source bare.
    let text = s.to_text();
    let old: String = text
        .lines()
        .filter(|l| !l.trim_start().starts_with("from:"))
        .collect::<Vec<_>>()
        .join("\n");
    let back = Save::load(&old).unwrap();
    assert_eq!(back.ice_sources[0].from, "");
}

#[test]
fn a_window_counts_program_lines_and_never_shows_the_header() {
    let d = data();
    let mut s = owned(&d, "gutter");
    let mut g = fight(&mut s, &d);
    // Turn 1: the cursor on the IF. Three lines of program, the IF's remark after it.
    let rows = ice_listing::rows(&g);
    let w = btop_fight::ice_window(&rows, 3, true);
    let code: Vec<u32> = w.iter().filter(|r| !r.rem).map(|r| r.no).collect();
    assert_eq!(code, [10, 20, 30], "NEXT's line stays in view");
    assert!(w[1].rem && w[1].text.contains("firewall"));
    assert!(
        !w.iter().any(|r| r.text.contains("NiGHTMOTHS")),
        "no header"
    );
    // Without remarks, the program alone.
    assert!(
        btop_fight::ice_window(&rows, 3, false)
            .iter()
            .all(|r| !r.rem)
    );
    // The cursor on the last line: the window wraps to line 10, not into the header.
    g.apply(Action::EndTurn);
    let rows = ice_listing::rows(&g);
    let w = btop_fight::ice_window(&rows, 3, false);
    assert_eq!(w.iter().map(|r| r.no).collect::<Vec<_>>(), [40, 10, 20]);
    // Spoofed: no cursor marked, so it starts at the first line of the program.
    g.combat.as_mut().unwrap().spoofed = 2;
    let rows = ice_listing::rows(&g);
    assert!(!rows.iter().any(|r| r.cursor));
    let w = btop_fight::ice_window(&rows, 3, false);
    assert_eq!(w.iter().map(|r| r.no).collect::<Vec<_>>(), [10, 20, 30]);
}

#[test]
fn a_remark_too_wide_for_the_ono_s_window_is_refused() {
    let mut d = data();
    let g = d.board("gutter").unwrap();
    d.boards[g].sources.as_mut().unwrap().remarks[0].text = "x".repeat(data::REMARK_MAX + 1);
    let e = d.validate().unwrap_err();
    assert!(e.contains("wider"), "{e}");
}
