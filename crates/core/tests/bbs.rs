use ninsei_core::bbs::{COLS, Outcome, ROWS, Screen, Session};
use ninsei_core::*;

/// Routes the runner onto the BBS the honest way: hop by hop from the entry.
fn game_at_bbs(seed: u64, credits: u32) -> Game {
    game_at_board(seed, credits, 0)
}

/// The same, on a network whose BBS is board `board`.
fn game_at_board(seed: u64, credits: u32, board: usize) -> Game {
    let kit = Kit {
        board,
        ..Kit::default()
    };
    let mut g = Game::start(Data::builtin().unwrap(), seed, &kit);
    let bbs = g.net.bbs().expect("every network has a bbs");
    // Clear the way so the walk doesn't start fights or conversations.
    for n in &mut g.net.nodes {
        if n.id != bbs {
            n.kind = NodeKind::Vault;
            n.cleared = true;
        }
    }
    while g.at != bbs {
        let reach_bbs = |n: usize| g.net.reachable_from(n)[bbs];
        let next = *g.net.nodes[g.at]
            .links
            .iter()
            .find(|&&n| reach_bbs(n))
            .unwrap();
        g.apply(Action::Move(next));
    }
    g.deck.credits = credits;
    g.deck.trace = 0;
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
    for (area, stock) in g.bbs().areas.iter().zip(&g.bbs_stock) {
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
    let price = g.bbs().areas[0].files[g.bbs_stock[0][0].def].price;
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
    let boards = Data::builtin().unwrap().boards.len();
    for board in 0..boards {
        let g = game_at_board(11, 500, board);
        let mut screens = vec![
            Screen::Dialing { t: 9.0 },
            Screen::Splash,
            Screen::Main,
            Screen::Bulletins,
            Screen::Who,
            Screen::Jobs,
            Screen::Areas,
            Screen::Download {
                area: 0,
                file: 0,
                t: 0.3,
            },
            Screen::Hangup { t: 9.0 },
        ];
        screens.extend((0..g.bbs().bulletins.len()).map(Screen::Bulletin));
        screens.extend((0..g.bbs().areas.len()).map(Screen::Area));
        for (area, files) in g.bbs_stock.iter().enumerate() {
            for (file, f) in files.iter().enumerate() {
                if matches!(g.bbs().areas[area].files[f.def].item, Drop::Text(_)) {
                    screens.push(Screen::Read { area, file });
                }
            }
        }
        for sc in screens {
            let s = Session {
                screen: sc,
                status: Some("|12status".into()),
            };
            let lines = s.render(&g);
            let id = &g.bbs().id;
            assert!(lines.len() <= ROWS, "{id} {sc:?} has {} rows", lines.len());
            for l in &lines {
                let w = ansi::visible_len(l);
                assert!(
                    w <= COLS,
                    "{id} {sc:?} line is {w} wide: {}",
                    ansi::plain(l)
                );
            }
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

#[test]
fn the_board_speaks_l33t() {
    use ninsei_core::leet::translate as l33t;
    assert_eq!(l33t("Rules of the Gutter"), "Rul3z 0f th3 Gutt3r");
    assert_eq!(
        l33t("   |09[|15F|09] |11File Areas"),
        "   |09[|15F|09] |11F1l3 4r34z"
    );
    assert_eq!(l33t("Download [#] or [Q]uit"), "D0wnl04d [#] 0r [Q]u1t");
    assert_eq!(
        l33t("|10Transfer complete. DECODR22.ZIP received."),
        "|10Tr4nsf3r c0mpl3t3. DECODR22.ZIP r3c31v3d."
    );
    assert_eq!(
        l33t("Press [ENTER] to continue"),
        "Pr3zz [ENTER] t0 c0nt1nu3"
    );
    assert_eq!(l33t("70¢ 555-0199"), "70¢ 555-0199");
    let line = "|07Read which? |09[|15#|09] |07or |09[|15Q|09]|07uit |09» |15_";
    assert_eq!(
        ansi::visible_len(&l33t(line)),
        ansi::visible_len(line),
        "same width"
    );
}

#[test]
fn bbs_screens_are_l33t_but_the_modem_and_splash_are_not() {
    let g = game_at_bbs(4, 100);
    let main = Session {
        screen: Screen::Main,
        status: None,
    }
    .render(&g);
    assert!(main.iter().any(|l| l.contains("F1l3 4r34z")), "{main:?}");
    let splash = Session {
        screen: Screen::Splash,
        status: None,
    }
    .render(&g);
    assert!(splash.iter().any(|l| ansi::plain(l).contains("SysOp")));
    let dial = Session {
        screen: Screen::Dialing { t: 9.0 },
        status: None,
    }
    .render(&g);
    assert!(dial.iter().any(|l| l.contains("CONNECT")));
}

#[test]
fn the_translator_is_for_everyone() {
    let d = Data::builtin().unwrap();
    let s = ninsei_core::hub::Save::default();
    let (out, _) = ninsei_core::hub::shell("leet hello world", &s, &d);
    assert_eq!(out, vec!["h3ll0 w0rld".to_string()]);

    // An event with `leet: true` speaks it, placeholders filled first.
    let mut g = Game::new(d.clone(), 3);
    let to = g.net.nodes[g.at].links[0];
    g.net.nodes[to].kind = NodeKind::Contact;
    g.net.nodes[to].cleared = false;
    g.apply(Action::Move(to));
    let ev = d.events.iter().position(|e| e.id == "lineman").unwrap();
    g.encounter.as_mut().unwrap().event = ev;
    assert!(d.events[ev].leet);
    let label = g.fill(&d.events[ev].choices[0].label, Some(0));
    assert_eq!(label, "P4y 40¢ f0r 4 r4ng3");
    assert_eq!(g.encounter_title(), "4 ph0n3 phr34k");
    // Events without the flag are left alone.
    let plain = d.events.iter().position(|e| !e.leet).unwrap();
    g.encounter.as_mut().unwrap().event = plain;
    assert_eq!(g.encounter_title(), d.events[plain].title);
}

#[test]
fn dialing_lasts_as_long_as_the_handshake() {
    let g = Game::new(Data::builtin().unwrap(), 1);
    let h = ninsei_core::modem::Handshake::new(&g.bbs().phone);
    let mut s = Session::new();
    s.tick(&g, h.end - 0.05);
    assert!(matches!(s.screen, Screen::Dialing { .. }), "still dialing");
    let lines = s.render(&g).join("\n");
    assert_eq!(lines.matches("RING").count(), h.rings.len());
    assert!(lines.contains("CONNECT"));
    s.tick(&g, 0.1);
    assert_eq!(s.screen, Screen::Splash);
}

/// The Gutter's always-stocked area holding a tool.
fn tool_file(g: &Game, id: &str) -> (usize, usize) {
    for (a, files) in g.bbs_stock.iter().enumerate() {
        for (f, s) in files.iter().enumerate() {
            if matches!(&g.bbs().areas[a].files[s.def].item, Drop::Tool(t) if t == id) {
                return (a, f);
            }
        }
    }
    panic!("no {id} on {}", g.bbs().id)
}

#[test]
fn the_gutter_always_stocks_the_starter_kit() {
    for seed in 0..20 {
        let g = game_at_bbs(seed, 0);
        assert_eq!(g.bbs().id, "gutter");
        tool_file(&g, "wardial");
        tool_file(&g, "hexed");
    }
}

#[test]
fn a_tool_downloads_free_once_and_comes_home() {
    let mut g = game_at_bbs(3, 0);
    g.tools.clear();
    let (area, file) = tool_file(&g, "wardial");
    assert!(g.buy_blocker(area, file).is_none());
    g.apply(Action::Buy { area, file });
    assert!(g.has_tool("wardial"));
    assert!(g.buy_blocker(area, file).is_some());
    let mut save = ninsei_core::hub::Save::default();
    assert!(!save.has_tool("wardial"));
    g.phase = Phase::Flatlined;
    let report = save.record(&g, &[], &g.data.rig.clone()).unwrap();
    assert!(save.has_tool("wardial"));
    assert_eq!(report.tools, vec!["WarDial".to_string()]);
}

#[test]
fn textfiles_are_read_not_bought() {
    let g = game_at_bbs(5, 0);
    let (area, _) = tool_file(&g, "wardial");
    let gfiles = area + 1;
    assert!(g.bbs().areas[gfiles].always);
    let mut s = Session::new();
    s.screen = Screen::Area(gfiles);
    assert_eq!(s.key(&g, '1'), Outcome::None);
    assert_eq!(s.screen, Screen::Read { area: gfiles, file: 0 });
    let text = s.render(&g).join("\n");
    assert!(text.contains("WARDIAL.ZIP"), "the primer names the dialer");
    s.key(&g, 'q');
    assert_eq!(s.screen, Screen::Area(gfiles));
}

#[test]
fn without_hexed_there_is_no_compiler() {
    let mut g = game_at_bbs(7, 0);
    g.tools.push("hexed".into());
    assert!(g.compile_blocker().is_none());
    g.tools.retain(|t| t != "hexed");
    assert!(g.compile_blocker().is_some());
}
