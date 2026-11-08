//! The Ono-Sendai's `ice.bas` tile: what the listing shows and what its keys do, played
//! through the real save and run.

use ninsei_core::btop_fight::{self, Gutter};
use ninsei_core::game::ExecState;
use ninsei_core::hub::Save;
use ninsei_core::*;

fn save() -> (Data, Save) {
    let d = Data::builtin().unwrap();
    let mut s = Save::default();
    s.ensure_key(0x5AFE);
    s.ensure_phonebook(&d, 0);
    s.ensure_hardware(&d);
    (d, s)
}

/// A fresh deck's run (it ships SAFE), in a fight with ICE too strong to fall.
fn fight() -> Game {
    let (d, mut s) = save();
    let mut g = s.jack_in(&d, 21);
    let target = g.net.nodes[g.at].links[0];
    g.net.nodes[target].kind = NodeKind::Ice;
    g.net.nodes[target].ice = Some(0);
    g.net.nodes[target].cleared = false;
    g.apply(Action::Move(target));
    let c = g.combat.as_mut().unwrap();
    (c.strength, c.strength_max) = (500, 500);
    g
}

fn texts(g: &Game) -> Vec<String> {
    g.program
        .lines
        .iter()
        .map(|l| format!("{} {}", l.no, l.text))
        .collect()
}

#[test]
fn the_listing_marks_the_current_line_the_stops_and_the_faults() {
    let mut g = fight();
    let rows = btop_fight::rows(&g);
    assert_eq!(rows.len(), g.program.lines.len(), "one row a line of SAFE");
    assert_eq!(
        rows[0].gutter,
        Gutter::Current,
        "the cursor on the first line"
    );
    assert_eq!(rows[0].no, g.exec.pc);
    assert!(
        rows.iter().all(|r| !r.dim && r.fault.is_none()),
        "SAFE reads"
    );

    g.apply(Action::Program(
        "10 RUN icepick\n20 STOP\n30 JUMP 10\n40 GOTO 10\nfoo".into(),
    ));
    let rows = btop_fight::rows(&g);
    assert_eq!(rows[1].gutter, Gutter::Stop);
    assert_eq!(rows[1].no, Some(20));
    let bad = &rows[2];
    assert!(bad.dim && bad.fault.is_some(), "{bad:?}");
    assert_eq!(bad.text, "JUMP 10", "as typed");
    let stray = rows.last().unwrap();
    assert_eq!(stray.no, None);
    assert!(
        stray.dim && stray.fault.is_some(),
        "text with no number: {stray:?}"
    );
    assert_eq!(
        rows.iter().filter(|r| r.gutter == Gutter::Current).count(),
        1
    );
}

#[test]
fn outside_a_fight_nothing_is_current_and_the_edge_shows_the_deck() {
    let (d, mut s) = save();
    let g = s.jack_in(&d, 21);
    assert_ne!(g.phase, Phase::Combat);
    let rows = btop_fight::rows(&g);
    assert!(!rows.is_empty(), "the program is there between fights");
    assert!(rows.iter().all(|r| r.gutter != Gutter::Current));
    let (cpu, ram) = (g.deck.cpu_max, g.deck.ram_max);
    assert_eq!(
        btop_fight::note(&g),
        format!("cpu {cpu}/{cpu}  ram {ram}/{ram}")
    );
    assert_eq!(btop_fight::title(&g), "ice.bas");
    assert_eq!(
        btop_fight::keys(&g),
        btop_fight::Keys::default(),
        "nothing presses"
    );
}

#[test]
fn the_edge_counts_down_this_turn() {
    let mut g = fight();
    g.apply(Action::Program(
        "10 IF INTEG > 0\n20 RUN icepick\n30 STOP".into(),
    ));
    g.apply(Action::Step);
    let c = g.combat.as_ref().unwrap();
    assert!(
        c.cpu < g.deck.cpu_max && c.ifs < g.deck.ram_max,
        "spent some"
    );
    assert_eq!(
        btop_fight::note(&g),
        format!(
            "cpu {}/{}  ram {}/{}",
            c.cpu, g.deck.cpu_max, c.ifs, g.deck.ram_max
        )
    );
}

#[test]
fn the_keys_follow_the_debugger() {
    let g = fight();
    let k = btop_fight::keys(&g);
    assert!(k.run && k.step && !k.cont && !k.pause, "paused: {k:?}");
    let mut g = fight();
    g.apply(Action::Program(
        "10 STOP\n20 RUN icepick\n30 GOTO 10".into(),
    ));
    g.apply(Action::Step);
    assert!(
        matches!(g.exec.state, ExecState::Stopped(10)),
        "{:?}",
        g.exec.state
    );
    let k = btop_fight::keys(&g);
    assert!(k.cont && k.step && !k.run && !k.pause, "stopped: {k:?}");
}

#[test]
fn running_shows_pause() {
    let mut g = fight();
    g.apply(Action::Program("10 RUN icepick\n20 GOTO 10".into()));
    g.apply(Action::Play);
    assert_eq!(g.exec.state, ExecState::Running);
    let k = btop_fight::keys(&g);
    assert!(k.pause && !k.run && !k.step && !k.cont, "{k:?}");
}

#[test]
fn the_cursor_moves_between_lines_and_stops_at_the_ends() {
    let mut g = fight();
    g.apply(Action::Program(
        "10 RUN icepick\n20 STOP\n30 GOTO 10".into(),
    ));
    assert_eq!(btop_fight::home(&g), Some(10));
    assert_eq!(btop_fight::moved(&g, Some(10), 1), Some(20));
    assert_eq!(btop_fight::moved(&g, Some(30), 1), Some(30), "the bottom");
    assert_eq!(btop_fight::moved(&g, Some(10), -1), Some(10), "the top");
    assert_eq!(btop_fight::moved(&g, Some(25), 1), Some(10), "gone: home");
}

#[test]
fn a_retyped_line_rewrites_that_line_only() {
    let mut g = fight();
    g.apply(Action::Program(
        "10 RUN icepick\n20 STOP\n30 GOTO 10".into(),
    ));
    let before = texts(&g);
    assert_eq!(btop_fight::editing_text(&g, 20), "STOP");
    assert_eq!(btop_fight::retype(&g, 20, "STOP"), None, "nothing changed");
    g.apply(btop_fight::retype(&g, 20, "RUN patch").unwrap());
    let after = texts(&g);
    let differ: Vec<_> = before.iter().zip(&after).filter(|(a, b)| a != b).collect();
    assert_eq!(differ.len(), 1, "{after:?}");
    assert_eq!(after[1], "20 RUN patch");
}

#[test]
fn open_takes_a_free_number_after_the_cursor_and_delete_takes_a_line_out() {
    let mut g = fight();
    g.apply(Action::Program(
        "10 RUN icepick\n20 STOP\n30 GOTO 10".into(),
    ));
    assert_eq!(btop_fight::open_after(&g, Some(20)), Some(25));
    assert_eq!(btop_fight::open_after(&g, Some(30)), Some(40));
    g.apply(btop_fight::retype(&g, 25, "RUN patch").unwrap());
    assert_eq!(g.program.line(25).unwrap().text, "RUN patch");
    g.apply(btop_fight::delete(&g, 20).unwrap());
    assert!(g.program.line(20).is_none());
    assert_eq!(g.program.lines.len(), 3);
    assert_eq!(btop_fight::delete(&g, 20), None, "already gone");
    g.apply(Action::Program(String::new()));
    assert_eq!(
        btop_fight::open_after(&g, None),
        Some(10),
        "an empty program"
    );
}

#[test]
fn an_edit_mid_fight_changes_what_the_next_pass_runs() {
    let mut g = fight();
    g.apply(Action::Program("10 STOP\n20 GOTO 10".into()));
    g.apply(btop_fight::retype(&g, 10, "RUN icepick").unwrap());
    let before = g.combat.as_ref().unwrap().strength;
    g.apply(Action::Step);
    assert!(
        g.combat.as_ref().is_none_or(|c| c.strength < before),
        "the retyped line ran"
    );
}

#[test]
fn the_tile_reads_as_the_program_view_says() {
    let mut g = fight();
    g.apply(Action::Program(
        "10 RUN icepick\n20 STOP\n30 JUMP 10\nfoo".into(),
    ));
    let v = g.program_view();
    let rows = btop_fight::rows(&g);
    for (r, l) in rows.iter().zip(&v.lines) {
        assert_eq!(r.no, Some(l.no));
        assert_eq!(r.gutter == Gutter::Current, l.current, "line {}", l.no);
        assert_eq!(r.gutter == Gutter::Stop, l.stop && !l.current);
        assert_eq!(r.fault, l.fault);
    }
    assert_eq!(rows.len(), v.lines.len() + v.stray.len());
    assert_eq!(
        btop_fight::note(&g),
        format!(
            "cpu {}/{}  ram {}/{}",
            v.cpu_left, g.deck.cpu_max, v.ifs_left, g.deck.ram_max
        )
    );
}
