use ninsei_core::bbs::{COLS, ROWS, Screen, Session};
use ninsei_core::hub::Save;
use ninsei_core::qwk::{self, Item, Reader, View};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

fn fresh(d: &Data) -> Save {
    let mut s = Save::default();
    s.ensure_key(0x0C0C);
    s.ensure_phonebook(d, 3);
    s
}

#[test]
fn a_fresh_packet_holds_the_gutters_bulletins() {
    let d = data();
    let s = fresh(&d);
    let bs = qwk::boards(&s, &d);
    assert_eq!(bs.len(), 1);
    let gutter = &d.boards[bs[0]];
    assert_eq!(gutter.id, "gutter");
    let items = qwk::items(&s, &d, bs[0]);
    assert_eq!(items.len(), gutter.bulletins.len());
    assert!(items.iter().all(|i| matches!(i, Item::Bulletin(_))));
}

#[test]
fn kept_textfiles_join_the_packet_and_open() {
    let d = data();
    let mut s = fresh(&d);
    let g = qwk::boards(&s, &d)[0];
    assert!(s.keep_text("gutter", "primer"));
    assert!(!s.keep_text("gutter", "primer"), "once is enough");
    let items = qwk::items(&s, &d, g);
    assert_eq!(items.last(), Some(&Item::Text("primer".into())));
    let mut r = Reader::new();
    r.key(&s, &d, '1');
    assert_eq!(r.view, View::Board(g));
    let n = items.len();
    r.key(&s, &d, char::from_digit(n as u32, 10).unwrap());
    assert_eq!(
        r.view,
        View::Read {
            board: g,
            item: n - 1
        }
    );
    assert!(
        r.render(&s, &d)
            .join("\n")
            .contains("WARDIALING FOR THE CLUELESS")
    );
    r.key(&s, &d, 'p');
    assert_eq!(
        r.view,
        View::Read {
            board: g,
            item: n - 2
        }
    );
    r.key(&s, &d, 'q');
    assert_eq!(r.view, View::Board(g));
    r.key(&s, &d, '\x1b');
    assert_eq!(r.view, View::Boards);
}

#[test]
fn every_reader_screen_fits_the_terminal() {
    let d = data();
    let mut s = fresh(&d);
    for b in &d.boards {
        s.learn_board(&b.id, "test");
        for t in &b.texts {
            s.keep_text(&b.id, &t.id);
        }
    }
    let mut views = vec![View::Boards];
    for b in qwk::boards(&s, &d) {
        views.push(View::Board(b));
        for item in 0..qwk::items(&s, &d, b).len() {
            views.push(View::Read { board: b, item });
        }
    }
    for v in views {
        let r = Reader { view: v, page: 0 };
        let lines = r.render(&s, &d);
        assert!(lines.len() <= ROWS, "{v:?} has {} rows", lines.len());
        for l in &lines {
            let w = ansi::visible_len(l);
            assert!(w <= COLS, "{v:?} line is {w} wide: {}", ansi::plain(l));
        }
    }
}

#[test]
fn a_textfile_downloaded_in_a_run_comes_home() {
    let d = data();
    let mut s = fresh(&d);
    let e = s.phonebook[0].id;
    let mut g = s.dial(&d, e, 5).unwrap();
    // Walk onto the BBS the quick way, like the bbs tests do.
    let bbs = g.net.bbs().unwrap();
    for n in &mut g.net.nodes {
        if n.id != bbs {
            n.kind = NodeKind::Vault;
            n.cleared = true;
        }
    }
    while g.at != bbs {
        let reach = |n: usize| g.net.reachable_from(n)[bbs];
        let next = *g.net.nodes[g.at].links.iter().find(|&&n| reach(n)).unwrap();
        g.apply(Action::Move(next));
    }
    let gfiles = g
        .bbs()
        .areas
        .iter()
        .position(|a| a.name == "G-FILES")
        .expect("the Gutter has G-FILES");
    let mut sess = Session::new();
    sess.screen = Screen::Read {
        area: gfiles,
        file: 0,
    };
    assert!(sess.keys(&g).iter().any(|(k, _)| *k == 'D'));
    sess.key(&g, 'd');
    assert!(matches!(sess.screen, Screen::Download { .. }));
    g.apply(Action::Buy {
        area: gfiles,
        file: 0,
    });
    assert!(g.has_text("primer"));
    assert!(g.buy_blocker(gfiles, 0).is_some(), "already on disk");
    g.phase = Phase::Flatlined;
    s.record(&g, &[], &d.rig).unwrap();
    assert!(
        s.texts
            .iter()
            .any(|k| k.board == "gutter" && k.id == "primer")
    );
}

#[test]
fn a_textfile_downloads_from_home_too() {
    let d = data();
    let mut s = fresh(&d);
    let board = d.board("gutter").unwrap();
    let call = s.home_call(&d, &d.rig, board);
    let def = &d.boards[board];
    let gfiles = def.areas.iter().position(|a| a.name == "G-FILES").unwrap();
    use ninsei_core::bbs::Line;
    assert!(call.buy_blocker(gfiles, 1).is_none());
    s.home_buy(&d, &d.rig, board, gfiles, 1).unwrap();
    assert!(s.texts.iter().any(|k| k.id == "ranges"));
    let call = s.home_call(&d, &d.rig, board);
    assert!(call.buy_blocker(gfiles, 1).is_some());
}
