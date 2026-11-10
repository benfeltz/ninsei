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
        // Its own bulletins and its ICE news, if it hears any.
        screens.extend((0..ninsei_core::bbs::Line::bulletins(&g).len()).map(Screen::Bulletin));
        screens.extend((0..g.bbs().areas.len()).map(Screen::Area));
        screens.push(Screen::Messages);
        for (base, m) in g.bbs().messages.iter().enumerate() {
            screens.push(Screen::Base(base));
            screens.extend((0..m.posts.len()).map(|post| Screen::Post { base, post }));
        }
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
    let gfiles = g
        .bbs()
        .areas
        .iter()
        .position(|a| a.name == "G-FILES")
        .expect("the Gutter has G-FILES");
    assert!(g.bbs().areas[gfiles].always);
    let mut s = Session::new();
    s.screen = Screen::Area(gfiles);
    assert_eq!(s.key(&g, '1'), Outcome::None);
    assert_eq!(
        s.screen,
        Screen::Read {
            area: gfiles,
            file: 0
        }
    );
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

/// A board with a deck mod in stock: the game sitting on it and where the file is.
fn deck_mod() -> (Game, usize, usize) {
    let d = Data::builtin().unwrap();
    for board in 0..d.boards.len() {
        for seed in 0..40 {
            let g = game_at_board(seed, 0, board);
            for (a, files) in g.bbs_stock.iter().enumerate() {
                for (f, s) in files.iter().enumerate() {
                    if matches!(
                        g.bbs().areas[a].files[s.def].item,
                        Drop::Cpu(_) | Drop::Ram(_) | Drop::Hdd(_)
                    ) {
                        return (g, a, f);
                    }
                }
            }
        }
    }
    panic!("no board stocks a deck mod")
}

#[test]
fn a_deck_mod_past_the_cap_is_refused_unpaid() {
    let (mut g, area, file) = deck_mod();
    g.deck.credits = 10_000;
    g.deck.caps = Caps {
        cpu: Some(g.deck.cpu_max),
        ram: Some(g.deck.ram_max),
        hdd: Some(g.deck.hdd_max),
    };
    let before = g.deck.clone();
    let why = g.buy_blocker(area, file).expect("refused");
    assert!(why.contains("can't take it"), "{why}");
    g.apply(Action::Buy { area, file });
    assert_eq!(g.deck, before, "nothing charged, nothing added");
    assert!(!g.bbs_stock[area][file].bought);
}

#[test]
fn loot_past_the_cap_is_wasted() {
    let src = include_str!("../../../data/loot.ron");
    let start = src.find("    vault: (").unwrap();
    let end = start + src[start..].find("]),").unwrap() + 3;
    let loot = format!(
        "{}    vault: (rolls: 1, entries: [(weight: 1, drop: Cpu(1))]),{}",
        &src[..start],
        &src[end..]
    );
    let d = Data::parse(Sources {
        loot: &loot,
        ..Sources::builtin()
    })
    .unwrap();
    let kit = Kit {
        caps: Caps {
            cpu: Some(Kit::default().cpu),
            ..Caps::default()
        },
        ..Kit::default()
    };
    let mut g = Game::start(d, 1, &kit);
    for n in &mut g.net.nodes {
        n.kind = NodeKind::Vault;
        n.cleared = true;
    }
    let to = g.reachable()[0];
    g.net.nodes[to].cleared = false;
    let cpu = g.deck.cpu_max;
    g.apply(Action::Move(to));
    assert_eq!(g.deck.cpu_max, cpu, "the chassis is full");
    let last = g.log.iter().rev().find(|l| l.text.contains("cpu")).unwrap();
    assert_eq!(last.kind, LogKind::Sys);
    assert!(last.text.contains("chassis is full"), "{}", last.text);
}

/// A save whose sysops have shipped, in this order, Rototurret 1.0.1 (a patch), Archer 1.1.0
/// (a minor) and Enigma 2.0.0 (a major).
fn shipped_save(d: &Data) -> hub::Save {
    use ice_version::{Bump, Version};
    let mut s = hub::Save::default();
    s.ensure_key(0x93);
    let v = Version::default();
    for (id, bump) in [
        ("rototurret", Bump::Patch),
        ("archer", Bump::Minor),
        ("enigma", Bump::Major),
    ] {
        let row = d.ice.iter().find(|i| i.id == id).unwrap();
        s.ice_versions.push(hub::IceVer {
            id: id.into(),
            version: v.bumped(bump),
            program: row.program.clone(),
            bump: Some(bump),
        });
    }
    s
}

fn board(d: &Data, id: &str) -> usize {
    d.boards.iter().position(|b| b.id == id).unwrap()
}

#[test]
fn a_new_version_is_news_on_the_boards_that_would_hear_of_it() {
    use ninsei_core::bbs::Line;
    let d = Data::builtin().unwrap();
    let s = shipped_save(&d);
    let news = |id: &str| s.home_call(&d, &d.rig, board(&d, id)).news();
    // The Gutter hears tier 1 and black ICE, newest first, each in its bump's words.
    let g = news("gutter").expect("the Gutter hears news");
    assert_eq!(g.title, "ICE WATCH");
    assert_eq!(g.body.len(), 3, "{:?}", g.body);
    assert!(g.body[0].contains("Enigma 2.0.0") && g.body[0].contains("rewritten"));
    assert!(g.body[1].contains("Archer 1.1.0") && g.body[1].contains("trick"));
    assert!(g.body[2].contains("Rototurret 1.0.1") && g.body[2].contains("patched"));
    // Dead Sector hears only the black builds.
    let ds = news("deadsector").expect("Dead Sector hears news");
    assert_eq!(ds.body.len(), 1, "{:?}", ds.body);
    assert!(ds.body[0].contains("Archer 1.1.0"));
    // CorpLink posts a memo, and never says ICE.
    let c = news("corplink").expect("CorpLink posts its updates");
    assert_eq!(c.body.len(), 3);
    for l in std::iter::once(&c.title).chain(&c.body) {
        assert!(!l.contains("ICE"), "{l}");
    }
    // Sense/Net's customer line knows nothing of it.
    assert!(news("sensenet").is_none());
    // A rumour never tells what the new version does: no line of any program is quoted.
    for b in [&g, &ds, &c] {
        for l in &b.body {
            for row in &d.ice {
                for line in row.program.lines().map(str::trim).filter(|l| !l.is_empty()) {
                    let stmt = line.split_once(' ').map_or(line, |(_, t)| t);
                    assert!(!l.contains(stmt), "{l} quotes `{stmt}`");
                }
            }
        }
    }
}

#[test]
fn the_news_comes_after_the_boards_own_bulletins_and_says_so_when_quiet() {
    use ninsei_core::bbs::Line;
    let d = Data::builtin().unwrap();
    let gutter = board(&d, "gutter");
    let quiet = hub::Save::default().home_call(&d, &d.rig, gutter);
    let all = quiet.bulletins();
    let own = &d.boards[gutter].bulletins;
    assert_eq!(all.len(), own.len() + 1);
    for (a, b) in all.iter().zip(own) {
        assert_eq!((&a.title, &a.body), (&b.title, &b.body));
    }
    let news = all.last().unwrap();
    assert_eq!(news.body, d.boards[gutter].ice_news.as_ref().unwrap().none);
    assert!(news.on_read.is_none());
    // In a run the board reads the kit's versions, as at home.
    let s = shipped_save(&d);
    let mut kit = s.kit(&d);
    kit.board = gutter;
    let g = Game::start(d.clone(), 3, &kit);
    assert_eq!(
        g.news().unwrap().body,
        s.home_call(&d, &d.rig, gutter).news().unwrap().body
    );
}

#[test]
fn a_full_news_bulletin_fits_the_terminal() {
    use ice_version::{Bump, Version};
    use ninsei_core::bbs::Line;
    let d = Data::builtin().unwrap();
    // Every kind shipped as a major at a long version.
    let mut s = hub::Save::default();
    for row in &d.ice {
        s.ice_versions.push(hub::IceVer {
            id: row.id.clone(),
            version: Version {
                major: 99,
                minor: 99,
                patch: 99,
            },
            program: row.program.clone(),
            bump: Some(Bump::Major),
        });
    }
    for (i, b) in d.boards.iter().enumerate() {
        let call = s.home_call(&d, &d.rig, i);
        let n = call.bulletins().len();
        let sess = Session {
            screen: Screen::Bulletin(n - 1),
            status: None,
        };
        let lines = sess.render(&call);
        assert!(lines.len() <= ROWS, "{}", b.id);
        for l in &lines {
            assert!(ansi::visible_len(l) <= COLS, "{}: {}", b.id, ansi::plain(l));
        }
    }
}

#[test]
fn bad_ice_news_is_rejected() {
    let src = include_str!("../../../data/bbs/deadsector/board.ron");
    let parse = |find: &str, replace: &str| {
        assert!(src.contains(find), "{find}");
        let board = src.replacen(find, replace, 1);
        let boards: Vec<BoardSrc> = Sources::builtin()
            .boards
            .iter()
            .map(|b| {
                if b.id == "deadsector" {
                    BoardSrc {
                        board: Box::leak(board.clone().into_boxed_str()),
                        ..*b
                    }
                } else {
                    *b
                }
            })
            .collect();
        Data::parse(Sources {
            boards: Box::leak(boards.into_boxed_slice()),
            ..Sources::builtin()
        })
    };
    for (find, replace, why) in [
        ("count: 5,", "count: 0,", "count 0"),
        ("tiers: [2, 0],", "tiers: [7],", "no ICE of tier 7"),
        ("tiers: [2, 0],", "tiers: [],", "hears no tiers"),
        ("new trick. don't", "{who} new trick. don't", "can't fill"),
        (
            "patched. they read their logs.",
            "patched. they read their logs. they read their logs. they read their logs.",
            "wider than the terminal",
        ),
    ] {
        let err = parse(find, replace).expect_err(why);
        assert!(
            err.contains("deadsector") && err.contains(why),
            "{why}: {err}"
        );
    }
}
