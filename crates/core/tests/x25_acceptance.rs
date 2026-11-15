//! Task 140's acceptance, end to end in core: every board a run can reach is an X.25 call
//! through the net's PAD, quick, skippable, cleared at once, in the PAD's words only, at the
//! longest number a network can be dialed on; and a call from home is still a modem call.

use ninsei_core::bbs::{COLS, Call, Line, Outcome, ROWS, Screen, Session};
use ninsei_core::hub::Save;
use ninsei_core::*;

/// What a modem call says, which a run's board never does.
const MODEM_WORDS: [&str; 7] = [
    "ATZ",
    "ATDT",
    "RING",
    "CONNECT",
    "+++",
    "ATH0",
    "NO CARRIER",
];

/// A run dialed on `number`, whose board is `board`, stood on its BBS node the honest way.
fn run_on(d: &Data, board: &str, number: &str) -> Game {
    let kit = Kit {
        board: d.boards.iter().position(|b| b.id == board).unwrap(),
        site: Some(Site {
            entry: 0,
            number: number.into(),
            name: "test".into(),
            tier: 1,
            traits: Vec::new(),
        }),
        ..Kit::default()
    };
    let mut g = Game::start(d.clone(), 11, &kit);
    let bbs = g.net.bbs().unwrap();
    for n in &mut g.net.nodes {
        if n.id != bbs {
            n.kind = NodeKind::Vault;
            n.cleared = true;
        }
    }
    while g.at != bbs {
        let next = *g.net.nodes[g.at]
            .links
            .iter()
            .find(|&&n| g.net.reachable_from(n)[bbs])
            .unwrap();
        g.apply(Action::Move(next));
    }
    g
}

fn plain(lines: &[String]) -> Vec<String> {
    lines.iter().map(|l| ansi::plain(l)).collect()
}

#[test]
fn every_board_in_a_run_is_a_quick_x25_call_in_the_pad_s_words() {
    let d = Data::builtin().unwrap();
    let x25 = &d.networks.x25;
    for b in &d.boards {
        for number in ["555-0142", "1-999-999-9999"] {
            let g = run_on(&d, &b.id, number);
            let addr = g.bbs_address();
            assert!(addr.len() <= data::LONG_X25_ADDRESS.len(), "{addr}");
            assert!(matches!(g.connect(), Call::Packet { .. }), "{}", b.id);
            let end = g.connect().timeline().end;
            assert!(end < 1.0, "{}: {end}", b.id);

            // The connect, frame by frame: the call, COM, the board; nothing a modem says.
            let mut s = Session::new();
            let mut frames = 0;
            while s.connecting() {
                let lines = s.render(&g);
                assert!(lines.len() <= ROWS);
                for l in plain(&lines) {
                    assert!(l.chars().count() <= COLS, "{}: {l}", b.id);
                    for w in MODEM_WORDS {
                        assert!(!l.contains(w), "{}: {w} in {l}", b.id);
                    }
                }
                assert_eq!(plain(&lines)[0], x25.call.replace("{address}", &addr));
                s.tick(&g, 1.0 / 60.0);
                frames += 1;
            }
            assert_eq!(s.screen, Screen::Splash);
            assert!(frames <= 60, "{}: {frames} frames at 60 fps", b.id);

            // The address on the screen is the address in the run's one log line.
            let logged: Vec<_> = g.log.iter().filter(|l| l.text.contains(&addr)).collect();
            assert_eq!(logged.len(), 1, "{}", b.id);
            assert_eq!(logged[0].text, x25.log_line(&addr, &b.name));
            assert!(Session::title(&g).contains(&addr));

            // Skipped at once by any key; the splash waits for its own.
            let mut s = Session::new();
            assert_eq!(s.key(&g, 'q'), Outcome::Cut);
            assert_eq!(s.screen, Screen::Splash);
            s.key(&g, '\n');
            assert_eq!(s.screen, Screen::Main);

            // Leaving: the goodbye and the PAD's clear, in a beat or on a key.
            s.key(&g, 'G');
            let bye = plain(&s.render(&g));
            assert_eq!(bye.len(), 2, "{}: {bye:?}", b.id);
            assert_eq!(bye[1], x25.clear.replace("{address}", &addr));
            for l in &bye {
                assert!(l.chars().count() <= COLS, "{}: {l}", b.id);
                for w in MODEM_WORDS {
                    assert!(!l.contains(w), "{}: {w} in {l}", b.id);
                }
            }
            let mut later = s.clone();
            assert_eq!(
                later.tick(&g, tuning::X25_CLEAR_SECS + 0.01),
                Outcome::Closed
            );
            assert_eq!(s.key(&g, ' '), Outcome::Closed);
            assert_eq!(g.connect().dropped(), bye[1]);
        }
    }
}

#[test]
fn every_board_called_from_home_is_still_the_modem_call() {
    let d = Data::builtin().unwrap();
    let mut s = Save::default();
    s.ensure_key(5);
    for b in &d.boards {
        s.learn_board(&b.id, "a test");
    }
    for i in 0..d.boards.len() {
        let call = s.home_call(&d, &d.rig, i);
        let b = call.board();
        let h = modem::Handshake::new(&b.phone);
        assert_eq!(call.connect(), Call::Modem(h.clone()));
        assert!(h.end > 4.0, "{}: the full theatre", b.id);
        let mut sess = Session::new();
        assert_eq!(sess.key(&call, 'x'), Outcome::None, "not hurried");
        sess.tick(&call, h.end + 0.01);
        assert_eq!(sess.screen, Screen::Splash);
        sess.key(&call, '\n');
        sess.key(&call, 'G');
        sess.tick(&call, 1.0);
        let bye = plain(&sess.render(&call));
        assert_eq!(
            bye.last().map(String::as_str),
            Some("NO CARRIER"),
            "{bye:?}"
        );
        assert_eq!(call.connect().dropped(), "NO CARRIER");
    }
}

#[test]
fn the_address_is_the_run_s_and_draws_nothing_from_it() {
    let d = Data::builtin().unwrap();
    let a = run_on(&d, "gutter", "555-0142");
    let b = run_on(&d, "gutter", "555-0142");
    assert_eq!(a.bbs_address(), b.bbs_address(), "same run, same address");
    assert_ne!(
        a.bbs_address(),
        run_on(&d, "gutter", "555-0143").bbs_address(),
        "another network, another address"
    );
    // Asking for it a thousand times leaves the run exactly as it was.
    let asked = run_on(&d, "gutter", "555-0142");
    for _ in 0..1000 {
        let _ = asked.connect();
        let _ = asked.bbs_address();
    }
    assert_eq!(asked.log, b.log);
    assert_eq!(asked.deck, b.deck);
    assert_eq!(asked.bbs_stock, b.bbs_stock);
}
