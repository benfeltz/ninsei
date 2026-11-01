use ninsei_core::bbs::{COLS, Outcome, ROWS, Screen, Session};
use ninsei_core::*;

fn game_at_bbs(seed: u64, credits: u32) -> Game {
    let mut g = Game::new(Data::builtin().unwrap(), seed);
    let bbs = g.net.bbs().expect("every network has a bbs");
    g.at = bbs;
    g.deck.credits = credits;
    g
}

/// Runs the modem clock until the session settles.
fn settle(s: &mut Session, g: &Game) -> Vec<Outcome> {
    let mut out = Vec::new();
    for _ in 0..1000 {
        let o = s.tick(g, 0.05);
        if o != Outcome::None {
            let closed = o == Outcome::Closed;
            out.push(o);
            if closed {
                break;
            }
        }
    }
    out
}

#[test]
fn every_network_has_one_bbs_on_its_layer() {
    for seed in 0..200 {
        let g = Game::new(Data::builtin().unwrap(), seed);
        let bbs: Vec<_> = g
            .net
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Bbs)
            .collect();
        assert_eq!(bbs.len(), 1, "seed {seed}");
        assert_eq!(bbs[0].layer, tuning::BBS_LAYER);
        assert!(bbs[0].ice.is_none());
    }
}

#[test]
fn bbs_stock_respects_area_sizes() {
    let g = Game::new(Data::builtin().unwrap(), 9);
    for (area, stock) in g.data.bbs.areas.iter().zip(&g.bbs_stock) {
        assert_eq!(stock.len(), (area.stock as usize).min(area.files.len()));
        let mut defs: Vec<_> = stock.iter().map(|f| f.def).collect();
        defs.dedup();
        assert_eq!(defs.len(), stock.len(), "no duplicate files");
    }
}

#[test]
fn dial_splash_menu_download_hangup() {
    let g = game_at_bbs(4, 10_000);
    let mut s = Session::new();
    settle(&mut s, &g);
    assert_eq!(s.screen, Screen::Splash);
    s.key(&g, '\n');
    assert_eq!(s.screen, Screen::Main);
    s.key(&g, 'f');
    assert_eq!(s.screen, Screen::Areas);
    s.key(&g, '1');
    assert_eq!(s.screen, Screen::Area(0));
    s.key(&g, '1');
    assert!(matches!(
        s.screen,
        Screen::Download {
            area: 0,
            file: 0,
            ..
        }
    ));
    let out = settle(&mut s, &g);
    assert_eq!(out, vec![Outcome::Act(Action::Buy { area: 0, file: 0 })]);
    assert_eq!(s.screen, Screen::Area(0));
    s.key(&g, 'q');
    s.key(&g, 'q');
    s.key(&g, 'g');
    assert_eq!(settle(&mut s, &g), vec![Outcome::Closed]);
}

#[test]
fn buying_spends_credits_and_grants_the_file() {
    let mut g = game_at_bbs(4, 10_000);
    let before = g.hdd.len();
    let price = g.data.bbs.areas[0].files[g.bbs_stock[0][0].def].price;
    g.apply(Action::Buy { area: 0, file: 0 });
    assert_eq!(g.deck.credits, 10_000 - price);
    assert_eq!(g.hdd.len(), before + 1);
    assert!(g.bbs_stock[0][0].bought);
    g.apply(Action::Buy { area: 0, file: 0 });
    assert_eq!(g.deck.credits, 10_000 - price, "a file sells once");
}

#[test]
fn broke_runner_cannot_start_a_download() {
    let g = game_at_bbs(4, 0);
    let mut s = Session::new();
    settle(&mut s, &g);
    for k in ['\n', 'f', '1', '1'] {
        s.key(&g, k);
    }
    assert_eq!(s.screen, Screen::Area(0));
    assert!(s.status.as_deref().unwrap_or("").contains("insufficient"));
}

#[test]
fn cannot_buy_away_from_the_bbs() {
    let mut g = Game::new(Data::builtin().unwrap(), 4);
    g.deck.credits = 10_000;
    g.apply(Action::Buy { area: 0, file: 0 });
    assert_eq!(g.deck.credits, 10_000);
}

#[test]
fn every_screen_fits_the_terminal() {
    let g = game_at_bbs(11, 500);
    let screens = [
        Screen::Splash,
        Screen::Main,
        Screen::Bulletins,
        Screen::Bulletin(0),
        Screen::Who,
        Screen::Areas,
        Screen::Area(0),
        Screen::Area(1),
        Screen::Area(2),
        Screen::Download {
            area: 0,
            file: 0,
            t: 0.3,
        },
    ];
    for sc in screens {
        let s = Session {
            screen: sc,
            status: Some("|12status".into()),
        };
        let lines = s.render(&g);
        assert!(lines.len() <= ROWS, "{sc:?} has {} rows", lines.len());
        for l in &lines {
            let w = ansi::visible_len(l);
            assert!(w <= COLS, "{sc:?} line is {w} wide: {}", ansi::plain(l));
        }
    }
}

#[test]
fn pipe_codes_and_sgr_parse() {
    let (mut f, mut b) = (ansi::DEFAULT_FG, ansi::DEFAULT_BG);
    let spans = ansi::parse_line("|14hi|17 \x1b[1;31mred\x1b[0mx|9", &mut f, &mut b);
    let got: Vec<_> = spans
        .iter()
        .map(|s| (s.fg, s.bg, s.text.as_str()))
        .collect();
    assert_eq!(
        got,
        vec![(14, 0, "hi"), (14, 1, " "), (9, 1, "red"), (7, 0, "x|9")]
    );
}
