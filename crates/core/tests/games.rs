//! Games as data: `games.ron`, the copy you own, and the ways you come by one.

use ninsei_core::bbs::Line;
use ninsei_core::data::{BbsFileDef, Drop};
use ninsei_core::hub::{GameCopy, Save};
use ninsei_core::shopper::{self, Block, Item, Page, Section, Stock};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

#[test]
fn every_game_validates() {
    let d = data();
    for e in games::Engine::ALL {
        assert!(d.games.iter().any(|g| g.engine == *e), "a game runs {e:?}");
    }
    for g in &d.games {
        assert!(!g.name.is_empty() && !g.blurb.is_empty() && !g.version.is_empty());
        assert_eq!(
            g.publisher, "papertiger",
            "{}: Paper Tiger boxes the games",
            g.id
        );
    }
}

#[test]
fn every_game_has_a_crack_and_a_splash() {
    let d = data();
    let mut seen: Vec<&str> = Vec::new();
    for g in &d.games {
        let crack = d.intro(&g.crack).expect("its crack");
        assert!(!crack.is_splash() && crack.board.is_some() && !crack.greetz.is_empty());
        let splash = d.intro(&g.splash).expect("its splash");
        assert!(splash.is_splash() && splash.greetz.is_empty(), "{}", g.id);
        assert!(
            splash.board.is_none(),
            "{}: a publisher advertises no board",
            g.id
        );
        for id in [&g.crack, &g.splash] {
            assert!(!seen.contains(&id.as_str()), "{id} is used twice");
            seen.push(id);
        }
    }
}

/// `games.ron`'s first row, changed by `f`, checked against the rest of the data.
fn check(f: impl Fn(&mut data::GameDef)) -> String {
    let mut d = data();
    f(&mut d.games[0]);
    d.validate().unwrap_err()
}

#[test]
fn a_bad_game_is_rejected() {
    let splash = data().games[0].splash.clone();
    let crack = data().games[0].crack.clone();
    assert!(check(|g| g.crack = "nowhere".into()).contains("unknown crack intro"));
    assert!(check(|g| g.splash = "nowhere".into()).contains("unknown splash intro"));
    assert!(check(|g| g.crack = splash.clone()).contains("is a splash"));
    // A tool's cracktro has no tag; a game's crack must carry one.
    assert!(check(|g| g.crack = "hexed".into()).contains("has no crew tag"));
    assert!(check(|g| g.splash = crack.clone()).contains("is a crack"));
    assert!(check(|g| g.publisher = "kowloon".into()).contains("unknown publisher"));
    assert!(check(|g| g.version = "9.9".into()).contains("doesn't name `Nibbler 9.9`"));
    assert!(check(|g| g.cols = 0).contains("field"));
    assert!(check(|g| g.rows = 99).contains("field"));
    // NIBBLER needs room for its walls and a snake.
    assert!(check(|g| g.cols = 7).contains("Nibbler plays on 8-78"));
    assert!(check(|g| g.tick_ms = 0).contains("tick_ms"));
    let mut d = data();
    d.games.retain(|g| g.engine != games::Engine::Blox);
    assert!(
        d.validate()
            .unwrap_err()
            .contains("no game runs the Blox engine")
    );
    assert!(check(|g| g.tick_ms = 5000).contains("tick_ms"));
    assert!(check(|g| g.shop = Some(data::GameShop { price: 0 })).contains("for nothing"));
    let mut d = data();
    let twice = d.games[0].clone();
    d.games.push(twice);
    assert!(d.validate().unwrap_err().contains("listed twice"));
}

#[test]
fn both_copies_are_owned_side_by_side() {
    use hub::GameCopy::{Boxed, Cracked};
    let d = data();
    let mut s = Save::default();
    assert!(!s.has_game("blox") && s.copies("blox").is_empty());
    assert!(s.learn_game("blox", Cracked));
    assert!(
        !s.learn_game("blox", Cracked),
        "the same copy twice is one entry"
    );
    assert_eq!(s.games.len(), 1);
    assert!(s.learn_game("blox", Boxed), "the box goes beside the crack");
    assert_eq!(s.copies("blox"), vec![Cracked, Boxed]);
    assert_eq!(s.games.len(), 2, "one entry per game and copy");
    let labels: Vec<String> = s
        .copies("blox")
        .into_iter()
        .map(|c| d.game_label("blox", c))
        .collect();
    assert_eq!(labels, vec!["BLOX [PHS]".to_string(), "Blox".to_string()]);
    assert!(s.learn_game("nibbler", Boxed));
    assert!(
        s.learn_game("nibbler", Cracked),
        "a crack goes beside the box too"
    );
    assert_eq!(d.game_label("nibbler", Cracked), "NIBBLER [2K4]");
    assert_eq!(d.game_label("nibbler", Boxed), "Nibbler");
    assert_eq!(s.cracks(), vec!["blox".to_string(), "nibbler".to_string()]);
}

#[test]
fn a_save_from_before_games_loads_with_none() {
    let mut s = Save::default();
    s.learn_game("blox", hub::GameCopy::Boxed);
    let text = s.to_text();
    assert!(text.contains("games"));
    let old = text.replace("games:", "_gone:");
    let back = Save::load(&old).expect("a save without games still loads");
    assert!(back.games.is_empty());
    let kept = Save::load(&text).unwrap();
    assert_eq!(kept.copies("blox"), vec![hub::GameCopy::Boxed]);
}

/// The builtin data with BLOX for free in the Gutter's first always-stocked area (no board
/// sells a game yet).
fn gutter_stocks_blox() -> (Data, usize, usize) {
    let mut d = data();
    let b = d.board("gutter").unwrap();
    let area = d.boards[b].areas.iter().position(|a| a.always).unwrap();
    d.boards[b].areas[area].files.push(BbsFileDef {
        file: "BLOX.ZIP".into(),
        item: Drop::Game("blox".into()),
        price: 0,
        desc: "BLOX cracked".into(),
    });
    d.validate().unwrap();
    (d, b, area)
}

/// A run whose network's BBS is `board`, walked onto the board with the way cleared.
fn run_on_board(d: &Data, board: usize, kit_games: Vec<String>) -> Game {
    let kit = Kit {
        board,
        games: kit_games,
        ..Kit::default()
    };
    let mut g = Game::start(d.clone(), 3, &kit);
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
    g.deck.trace = 0;
    g
}

/// Where the BLOX file sits in today's listing.
fn blox_file(line: &dyn Line) -> (usize, usize) {
    for (a, files) in line.stock().iter().enumerate() {
        for (f, s) in files.iter().enumerate() {
            if line.board().areas[a].files[s.def].item == Drop::Game("blox".into()) {
                return (a, f);
            }
        }
    }
    panic!("no BLOX on {}", line.board().id)
}

#[test]
fn a_game_downloads_and_comes_home_cracked() {
    let (d, board, _) = gutter_stocks_blox();
    let mut g = run_on_board(&d, board, Vec::new());
    let (area, file) = blox_file(&g);
    assert!(g.buy_blocker(area, file).is_none());
    g.apply(Action::Buy { area, file });
    assert!(g.has_crack("blox"));
    // Even on a flatline it's yours.
    g.phase = Phase::Flatlined;
    let mut save = Save::default();
    let report = save.record(&g, &[], &d.rig).unwrap();
    assert_eq!(save.copies("blox"), vec![GameCopy::Cracked]);
    assert_eq!(report.games, vec!["BLOX [PHS]".to_string()]);
    // A board won't sell the crack again; the run's kit carries it from the save.
    let kit = save.kit(&d, &d.rig);
    assert_eq!(kit.games, vec!["blox".to_string()]);
    let mut g = run_on_board(&d, board, kit.games);
    let (area, file) = blox_file(&g);
    assert_eq!(
        g.buy_blocker(area, file).unwrap(),
        "you already have BLOX [PHS]."
    );
    g.phase = Phase::JackedOut;
    let report = save.record(&g, &[], &d.rig).unwrap();
    assert!(report.games.is_empty(), "nothing new came home");
}

#[test]
fn a_board_still_sells_the_crack_to_someone_with_the_box() {
    let (d, board, _) = gutter_stocks_blox();
    let mut save = Save::default();
    save.learn_game("blox", GameCopy::Boxed);
    let kit = save.kit(&d, &d.rig);
    assert!(kit.games.is_empty(), "a box isn't a crack");
    let mut g = run_on_board(&d, board, kit.games);
    let (area, file) = blox_file(&g);
    assert!(g.buy_blocker(area, file).is_none());
    g.apply(Action::Buy { area, file });
    g.phase = Phase::JackedOut;
    save.record(&g, &[], &d.rig).unwrap();
    assert!(save.has_copy("blox", GameCopy::Cracked) && save.has_copy("blox", GameCopy::Boxed));
}

#[test]
fn a_game_downloaded_at_home_is_cracked() {
    let (d, board, _) = gutter_stocks_blox();
    let mut s = Save::default();
    s.ensure_key(0xB0A2D);
    s.learn_board("gutter", "a test");
    let call = s.home_call(&d, &d.rig, board);
    let (area, file) = blox_file(&call);
    let line = s.home_buy(&d, &d.rig, board, area, file).unwrap();
    assert!(line.contains("BLOX [PHS]"), "{line}");
    assert_eq!(s.copies("blox"), vec![GameCopy::Cracked]);
    let call = s.home_call(&d, &d.rig, board);
    let (area, file) = blox_file(&call);
    assert!(call.buy_blocker(area, file).is_some(), "once");
}

#[test]
fn a_board_file_naming_an_unknown_game_is_rejected() {
    let (mut d, board, area) = gutter_stocks_blox();
    let f = d.boards[board].areas[area].files.last_mut().unwrap();
    f.item = Drop::Game("pong".into());
    assert!(
        d.validate()
            .unwrap_err()
            .contains("file `BLOX.ZIP` names unknown game `pong`")
    );
}

#[test]
fn a_game_never_drops_from_loot_or_an_encounter() {
    let mut d = data();
    d.loot.vault.entries[0].drop = Drop::Game("blox".into());
    let e = d.validate().unwrap_err();
    assert!(e.contains("loot table `vault` drops a game"), "{e}");
    let mut d = data();
    let ev = d
        .events
        .iter()
        .position(|e| {
            e.choices
                .iter()
                .any(|c| matches!(c.effect, data::ChoiceEffect::Pay { .. }))
        })
        .expect("an event that sells something");
    for c in &mut d.events[ev].choices {
        if let data::ChoiceEffect::Pay { get, .. } = &mut c.effect {
            *get = Drop::Game("blox".into());
        }
    }
    assert!(d.validate().unwrap_err().contains("drops a game"));
}

/// The builtin data with every game boxed by mail order (none is for sale yet).
fn shop_sells_games() -> Data {
    let mut d = data();
    for (g, price) in d.games.iter_mut().zip([40, 60, 30]) {
        g.shop = Some(data::GameShop { price });
    }
    d.validate().unwrap();
    d
}

#[test]
fn every_game_is_sold_boxed_now_it_plays() {
    let d = data();
    assert!(d.games.iter().all(|g| g.shop.is_some()));
    let cat = Save::default().catalog(&d);
    assert_eq!(
        cat.iter().filter(|l| l.section == Section::Games).count(),
        d.games.len()
    );
    let pages = shopper::layout(&d.shopper, &cat);
    assert!(
        shopper::contents(&pages)
            .iter()
            .any(|(s, ..)| *s == Section::Games),
        "IN THIS ISSUE has a Games line"
    );
    let index = shopper::advertisers(&d.shopper, &cat, &pages);
    assert!(
        index.iter().any(|(v, _)| v == "papertiger"),
        "Paper Tiger in the index"
    );
}

#[test]
fn a_game_bought_by_mail_is_boxed() {
    let d = shop_sells_games();
    let mut s = Save {
        bank: 200,
        ..Save::default()
    };
    let item = Item::Game("nibbler".into());
    let price = d.game("nibbler").unwrap().shop.as_ref().unwrap().price;
    assert!(s.order_blocker(&d, &item).is_none());
    s.buy_item(&d, &item).unwrap();
    assert_eq!(s.copies("nibbler"), vec![GameCopy::Boxed]);
    assert_eq!(s.bank, 200 - price);
    assert!(s.order_blocker(&d, &item).is_some(), "one box is enough");
    assert!(s.buy_item(&d, &item).is_err());
    // A cracked copy still lets you buy the box, and the crack stays on the disk.
    s.learn_game("blox", GameCopy::Cracked);
    let blox = Item::Game("blox".into());
    assert!(s.order_blocker(&d, &blox).is_none());
    s.buy_item(&d, &blox).unwrap();
    assert!(s.has_copy("blox", GameCopy::Boxed) && s.has_copy("blox", GameCopy::Cracked));
}

#[test]
fn a_game_is_in_the_next_issue_without_an_ad() {
    let d = shop_sells_games();
    assert!(
        !d.shopper
            .ads
            .iter()
            .any(|a| matches!(a.item, Item::Game(_))),
        "no ad is written yet"
    );
    let cat = Save::default().catalog(&d);
    let pages = shopper::layout(&d.shopper, &cat);
    for g in d.games.iter().filter(|g| g.shop.is_some()) {
        let ix = cat
            .iter()
            .position(|l| l.item == Item::Game(g.id.clone()))
            .expect("listed");
        let l = &cat[ix];
        assert_eq!(l.section, Section::Games);
        assert_eq!(l.stock, Stock::Price(g.shop.as_ref().unwrap().price));
        assert_eq!(d.shopper.vendor_of(l), "papertiger");
        let in_list = pages.iter().any(|p| match p {
            Page::Section {
                sections, blocks, ..
            } => sections.contains(&Section::Games)
                && blocks.iter().any(
                    |(b, _)| matches!(b, Block::List { listings, .. } if listings.contains(&ix)),
                ),
            _ => false,
        });
        assert!(in_list, "{} is a price-list line in Games", g.id);
    }
}

#[test]
fn the_listing_marks_a_crack_you_have_as_yours() {
    use ninsei_core::bbs::{Screen, Session};
    let (d, board, area) = gutter_stocks_blox();
    let screens = |cracks: Vec<String>| {
        let g = run_on_board(&d, board, cracks);
        let mut s = Session::new();
        s.screen = Screen::Area(area);
        let files = ansi::plain(&s.render(&g).join("\n"));
        s.screen = Screen::Areas;
        let areas = ansi::plain(&s.render(&g).join("\n"));
        (files, areas)
    };
    // The area's "N file(s)" count on the FILE AREAS screen.
    let left = |areas: &str| -> usize {
        // The Gutter writes in l33t, so find the area by its translated name.
        let name = leet::translate(&d.boards[board].areas[area].name);
        let line = areas.lines().find(|l| l.contains(name.as_str())).unwrap();
        let words: Vec<&str> = line.split_whitespace().collect();
        words[words.len() - 2].parse().unwrap()
    };
    let (files, areas) = screens(Vec::new());
    let blox = files.lines().find(|l| l.contains("BLOX.ZIP")).unwrap();
    assert!(
        blox.contains("[EXE]") && !blox.contains("[yours]"),
        "{blox}"
    );
    let before = left(&areas);
    let (files, areas) = screens(vec!["blox".into()]);
    let blox = files.lines().find(|l| l.contains("BLOX.ZIP")).unwrap();
    assert!(
        blox.contains("[yours]") && !blox.contains("[EXE]"),
        "{blox}"
    );
    assert_eq!(
        left(&areas),
        before - 1,
        "an owned crack drops out of the count"
    );
}

#[test]
fn every_crack_is_signed_by_a_crew_that_cracks_games() {
    // docs/world.md: PHOSPHOR and THE 2400 CLUB crack games; the NiGHTMOTHS crack tools and
    // RAZORWIRE breakers. Games are the Gutter's, never Dead Sector's or CorpLink's.
    let d = data();
    for g in &d.games {
        let crack = d.intro(&g.crack).unwrap();
        assert!(
            ["PHOSPHOR", "THE 2400 CLUB"].contains(&crack.group.as_str()),
            "{}: cracked by {}",
            g.id,
            crack.group
        );
        assert_eq!(crack.board.as_deref(), Some("gutter"), "{}", g.id);
    }
}

mod nibbler {
    use ninsei_core::games::nibbler::Nibbler;
    use ninsei_core::games::{Arcade, Cell, Input};
    use ninsei_core::tuning;

    /// Steers at the food by the first safe square it can turn to, never back on itself.
    fn toward_food(n: &Nibbler) -> Input {
        let (hx, hy) = (n.head().0 as i32, n.head().1 as i32);
        let Some((fx, fy)) = n.food() else {
            return Input::None;
        };
        let (fx, fy) = (fx as i32, fy as i32);
        let cells = n.cells();
        let safe = |dx: i32, dy: i32| {
            let (x, y) = ((hx + dx) as usize, (hy + dy) as usize);
            matches!(cells[y * n.cols() + x], Cell::Empty | Cell::Food)
        };
        let mut moves = vec![
            ((fx - hx).signum(), 0),
            (0, (fy - hy).signum()),
            (1, 0),
            (-1, 0),
            (0, 1),
            (0, -1),
        ];
        moves.retain(|&(dx, dy)| (dx, dy) != (0, 0) && safe(dx, dy));
        match moves.first() {
            Some((1, 0)) => Input::Right,
            Some((-1, 0)) => Input::Left,
            Some((0, 1)) => Input::Down,
            Some((0, -1)) => Input::Up,
            _ => Input::None,
        }
    }

    #[test]
    fn a_straight_run_into_the_wall_ends_it() {
        let mut n = Nibbler::new(1, 32, 20);
        let mut ticks = 0;
        while !n.over() {
            n.step(Input::None, 1);
            ticks += 1;
            assert!(ticks < 1000, "it never hit the wall");
        }
        assert!(!n.won());
        assert!(n.status().contains("GAME OVER"));
        // Over is over: nothing moves it again.
        let still = n.cells();
        n.step(Input::Up, 50);
        assert_eq!(n.cells(), still);
    }

    #[test]
    fn it_eats_grows_and_scores() {
        let mut n = Nibbler::new(7, 32, 20);
        for _ in 0..5000 {
            if n.score() >= 3 * tuning::NIBBLER_FOOD_POINTS || n.over() {
                break;
            }
            let i = toward_food(&n);
            n.step(i, 1);
        }
        assert_eq!(n.score(), 3 * tuning::NIBBLER_FOOD_POINTS);
        assert_eq!(n.len(), tuning::NIBBLER_START_LEN + 3);
        assert!(n.status().starts_with("SCORE 30  LENGTH 6"));
    }

    #[test]
    fn it_speeds_up_every_few_meals() {
        let mut n = Nibbler::new(3, 40, 20);
        assert_eq!(n.period(), tuning::NIBBLER_START_PERIOD);
        while n.score() < tuning::NIBBLER_SPEEDUP_EVERY * tuning::NIBBLER_FOOD_POINTS && !n.over() {
            let i = toward_food(&n);
            n.step(i, 1);
        }
        assert_eq!(n.period(), tuning::NIBBLER_START_PERIOD - 1);
    }

    #[test]
    fn it_never_turns_back_on_itself() {
        let mut n = Nibbler::new(1, 32, 20);
        let (x, y) = n.head();
        n.step(Input::Left, tuning::NIBBLER_START_PERIOD);
        assert_eq!(n.head(), (x + 1, y), "still heading right");
        assert!(!n.over());
    }

    #[test]
    fn a_quick_u_turn_keeps_both_keys() {
        // Heading right: Up then Left inside one move goes up one square, then left.
        let mut n = Nibbler::new(1, 32, 20);
        let (x, y) = n.head();
        n.step(Input::Up, 0);
        n.step(Input::Left, 0);
        n.step(Input::None, tuning::NIBBLER_START_PERIOD);
        assert_eq!(n.head(), (x, y - 1));
        n.step(Input::None, tuning::NIBBLER_START_PERIOD);
        assert_eq!(n.head(), (x - 1, y - 1));
        assert!(!n.over());
    }

    #[test]
    fn same_seed_same_food() {
        let foods = |seed| {
            let mut n = Nibbler::new(seed, 32, 20);
            let mut out = vec![n.food()];
            while out.len() < 4 && !n.over() {
                let i = toward_food(&n);
                n.step(i, 1);
                if n.food() != *out.last().unwrap() {
                    out.push(n.food());
                }
            }
            out
        };
        assert_eq!(foods(11), foods(11));
        assert_ne!(foods(11), foods(12));
    }

    #[test]
    fn the_field_is_walled_and_the_snake_is_on_it() {
        let n = Nibbler::new(1, 32, 20);
        let cells = n.cells();
        assert_eq!(cells.len(), 32 * 20);
        assert!(cells[..32].iter().all(|c| *c == Cell::Wall));
        assert_eq!(cells.iter().filter(|c| **c == Cell::Head).count(), 1);
        assert_eq!(cells.iter().filter(|c| **c == Cell::Food).count(), 1);
    }

    #[test]
    fn pause_holds_the_clock() {
        let mut n = Nibbler::new(1, 32, 20);
        n.step(Input::Pause, 0);
        let still = n.cells();
        n.step(Input::None, 100);
        assert!(n.paused() && n.cells() == still && n.status().contains("PAUSED"));
        n.step(Input::Pause, 0);
        n.step(Input::None, tuning::NIBBLER_START_PERIOD);
        assert_ne!(n.cells(), still);
    }
}

mod blox {
    use ninsei_core::games::blox::Blox;
    use ninsei_core::games::{Arcade, Cell, Input};
    use ninsei_core::tuning;

    #[test]
    fn a_filled_row_clears() {
        // Where the first piece lands on an empty board, dropped straight down.
        let mut probe = Blox::new(5, 10, 20);
        probe.step(Input::Fire, 0);
        let bottom = 19 * 10;
        let landed: Vec<usize> = (0..10)
            .filter(|&x| probe.cells()[bottom + x] != Cell::Empty)
            .collect();
        assert!(!landed.is_empty());
        // Fill the rest of the bottom row, then drop it in the gap.
        let rest: Vec<(usize, usize)> = (0..10)
            .filter(|x| !landed.contains(x))
            .map(|x| (x, 19))
            .collect();
        let mut b = Blox::with_board(5, 10, 20, &rest).unwrap();
        b.step(Input::Fire, 0);
        assert_eq!(b.lines(), 1, "the full row cleared");
        assert!(b.score() >= tuning::BLOX_LINE_POINTS[1]);
        assert!(b.status().contains("LINES 1"));
    }

    #[test]
    fn a_starting_board_must_be_one_a_game_could_reach() {
        assert!(Blox::with_board(5, 10, 20, &[(0, 19), (9, 19)]).is_some());
        assert!(
            Blox::with_board(5, 10, 20, &[(10, 19)]).is_none(),
            "off the field"
        );
        let full: Vec<(usize, usize)> = (0..10).map(|x| (x, 19)).collect();
        assert!(
            Blox::with_board(5, 10, 20, &full).is_none(),
            "a full row never clears"
        );
        let under = Blox::new(5, 10, 20).piece()[0];
        assert!(
            Blox::with_board(5, 10, 20, &[under]).is_none(),
            "under the piece"
        );
    }

    #[test]
    fn it_ends_when_a_piece_has_no_room() {
        let mut b = Blox::new(2, 10, 20);
        let mut ticks = 0;
        while !b.over() {
            b.step(Input::None, 1);
            ticks += 1;
            assert!(ticks < 100_000, "it never filled up");
        }
        assert!(b.status().contains("GAME OVER"));
        let still = b.cells();
        b.step(Input::Fire, 10);
        assert_eq!(b.cells(), still, "over is over");
    }

    #[test]
    fn turning_never_leaves_the_field() {
        for seed in 0..20 {
            let mut b = Blox::new(seed, 6, 8);
            for _ in 0..6 {
                b.step(Input::Left, 0);
            }
            for _ in 0..8 {
                b.step(Input::Up, 0);
                assert!(
                    b.piece().iter().all(|&(x, y)| x < 6 && y < 8),
                    "seed {seed}"
                );
            }
            for _ in 0..6 {
                b.step(Input::Right, 0);
                b.step(Input::Alt, 0);
                assert!(
                    b.piece().iter().all(|&(x, y)| x < 6 && y < 8),
                    "seed {seed}"
                );
            }
        }
    }

    #[test]
    fn the_bag_deals_all_seven_before_repeating() {
        // Each hard drop deals the next piece; seven in a row are seven different pieces.
        let mut b = Blox::new(9, 10, 20);
        let mut kinds = Vec::new();
        for _ in 0..7 {
            let (x, y) = b.piece()[0];
            if let Cell::Block(k) = b.cells()[y * 10 + x] {
                kinds.push(k);
            }
            b.step(Input::Fire, 0);
        }
        kinds.sort_unstable();
        assert_eq!(kinds, vec![1, 2, 3, 4, 5, 6, 7]);
    }

    #[test]
    fn gravity_drops_a_row_a_period() {
        let mut b = Blox::new(1, 10, 20);
        let before = b.piece();
        b.step(Input::None, tuning::BLOX_START_PERIOD - 1);
        assert_eq!(b.piece(), before);
        b.step(Input::None, 1);
        let after = b.piece();
        assert!(after.iter().zip(&before).all(|(a, p)| a.1 == p.1 + 1));
    }

    #[test]
    fn same_seed_same_pieces() {
        let play = |seed| {
            let mut b = Blox::new(seed, 10, 20);
            let mut out = Vec::new();
            for i in 0..40 {
                b.step(if i % 3 == 0 { Input::Fire } else { Input::Left }, 2);
                out.push(b.cells());
            }
            out
        };
        assert_eq!(play(4), play(4));
        assert_ne!(play(4), play(5));
    }
}

mod minefield {
    use ninsei_core::games::minefield::Minefield;
    use ninsei_core::games::{Arcade, Cell, Input};

    #[test]
    fn the_first_square_is_never_a_mine() {
        for seed in 0..200 {
            let mut m = Minefield::new(seed, 16, 16, 100);
            let (x, y) = ((seed % 16) as usize, (seed / 16 % 16) as usize);
            m.step(Input::Click { col: x, row: y }, 0);
            assert!(!m.is_mine(x, y) && !m.over(), "seed {seed}");
        }
        // Even a field that's nearly all mines.
        let mut m = Minefield::new(3, 4, 4, 100);
        m.step(Input::Click { col: 0, row: 0 }, 0);
        assert!(!m.is_mine(0, 0));
    }

    #[test]
    fn an_empty_square_opens_its_region() {
        let mut m = Minefield::new(1, 16, 16, 100);
        m.step(Input::Click { col: 8, row: 8 }, 0);
        // The first square has no mine next to it, so it opened its neighbours too.
        let opened = (0..16)
            .flat_map(|y| (0..16).map(move |x| (x, y)))
            .filter(|&(x, y)| m.is_open(x, y))
            .count();
        assert!(opened > 9, "only {opened} opened");
        let cells = m.cells();
        assert_eq!(cells[8 * 16 + 8], Cell::Empty);
        // Every open square's number is the mines around it.
        for y in 0..16 {
            for x in 0..16 {
                if let Cell::Number(n) = cells[y * 16 + x] {
                    let around = (-1i32..=1)
                        .flat_map(|dy| (-1i32..=1).map(move |dx| (dx, dy)))
                        .filter(|&(dx, dy)| {
                            m.is_mine((x as i32 + dx) as usize, (y as i32 + dy) as usize)
                        })
                        .count();
                    assert_eq!(usize::from(n), around);
                }
            }
        }
    }

    #[test]
    fn opening_every_safe_square_wins() {
        let mut m = Minefield::new(4, 16, 16, 100);
        m.step(Input::Click { col: 0, row: 0 }, 0);
        for y in 0..16 {
            for x in 0..16 {
                if !m.is_mine(x, y) {
                    m.step(Input::Click { col: x, row: y }, 0);
                }
            }
        }
        assert!(m.over() && m.won());
        assert!(m.status().contains("FIELD CLEARED"));
        assert_eq!(m.score() as usize, 16 * 16 - m.mines());
    }

    #[test]
    fn a_mine_ends_it_and_shows_them_all() {
        let mut m = Minefield::new(4, 16, 16, 100);
        m.step(Input::Click { col: 0, row: 0 }, 0);
        let (x, y) = (0..256)
            .map(|i| (i % 16, i / 16))
            .find(|&(x, y)| m.is_mine(x, y))
            .unwrap();
        m.step(Input::Click { col: x, row: y }, 0);
        assert!(m.over() && !m.won() && m.status().contains("BOOM"));
        let shown = m.cells().iter().filter(|c| **c == Cell::Mine).count();
        assert_eq!(shown, m.mines(), "nothing was flagged, so every mine shows");
    }

    #[test]
    fn a_lost_field_keeps_right_flags_and_marks_wrong_ones() {
        let mut m = Minefield::new(4, 16, 16, 100);
        m.step(Input::Click { col: 0, row: 0 }, 0);
        let squares: Vec<(usize, usize)> = (0..256).map(|i| (i % 16, i / 16)).collect();
        let mines: Vec<_> = squares.iter().filter(|&&(x, y)| m.is_mine(x, y)).collect();
        let safe = squares
            .iter()
            .find(|&&(x, y)| !m.is_mine(x, y) && !m.is_open(x, y))
            .unwrap();
        let (right, boom) = (mines[0], mines[1]);
        m.step(
            Input::Flag {
                col: right.0,
                row: right.1,
            },
            0,
        );
        m.step(
            Input::Flag {
                col: safe.0,
                row: safe.1,
            },
            0,
        );
        m.step(
            Input::Click {
                col: boom.0,
                row: boom.1,
            },
            0,
        );
        let cells = m.cells();
        let at = |(x, y): (usize, usize)| cells[y * 16 + x];
        assert_eq!(at(*right), Cell::Flag, "a right flag stays a flag");
        assert_eq!(at(*safe), Cell::WrongFlag);
        assert_eq!(at(*boom), Cell::Mine);
        let shown = cells.iter().filter(|c| **c == Cell::Mine).count();
        assert_eq!(shown, m.mines() - 1, "every mine but the flagged one");
    }

    #[test]
    fn a_cleared_field_flags_every_mine() {
        let mut m = Minefield::new(4, 16, 16, 100);
        m.step(Input::Click { col: 0, row: 0 }, 0);
        for i in 0..256 {
            if !m.is_mine(i % 16, i / 16) {
                m.step(
                    Input::Click {
                        col: i % 16,
                        row: i / 16,
                    },
                    0,
                );
            }
        }
        assert!(m.won());
        let flags = m.cells().iter().filter(|c| **c == Cell::Flag).count();
        assert_eq!(flags, m.mines());
        assert_eq!(m.mines_left(), 0);
        assert!(!m.cells().contains(&Cell::Covered));
    }

    #[test]
    fn flags_guard_a_square_and_count_down() {
        let mut m = Minefield::new(2, 16, 16, 100);
        let all = m.mines();
        m.step(Input::Flag { col: 3, row: 3 }, 0);
        assert_eq!(m.mines_left(), all - 1);
        assert_eq!(m.cells()[3 * 16 + 3], Cell::Flag);
        m.step(Input::Click { col: 3, row: 3 }, 0);
        assert!(!m.is_open(3, 3), "a flagged square doesn't open");
        m.step(Input::Flag { col: 3, row: 3 }, 0);
        assert_eq!(m.mines_left(), all);
    }

    #[test]
    fn a_flag_click_moves_the_cursor_like_a_click() {
        let mut m = Minefield::new(2, 16, 16, 100);
        m.step(Input::Flag { col: 3, row: 3 }, 0);
        assert_eq!(m.cursor(), Some((3, 3)));
        // Fire acts on the square just touched: here it's flagged, so it stays shut.
        m.step(Input::Fire, 0);
        assert!(!m.is_open(3, 3) && !m.is_open(8, 8));
        m.step(Input::Flag { col: 99, row: 3 }, 0);
        assert_eq!(
            m.cursor(),
            Some((3, 3)),
            "a click off the field moves nothing"
        );
    }

    #[test]
    fn the_keyboard_moves_a_cursor_that_stays_on_the_field() {
        let mut m = Minefield::new(2, 16, 16, 100);
        for _ in 0..40 {
            m.step(Input::Left, 0);
            m.step(Input::Up, 0);
        }
        assert_eq!(m.cursor(), Some((0, 0)));
        m.step(Input::Alt, 0);
        assert_eq!(m.cells()[0], Cell::Flag);
        m.step(Input::Alt, 0);
        m.step(Input::Fire, 0);
        assert!(m.is_open(0, 0));
    }

    #[test]
    fn the_clock_runs_from_the_first_open_and_holds_on_pause() {
        let mut m = Minefield::new(2, 16, 16, 100);
        m.step(Input::None, 50);
        assert_eq!(m.seconds(), 0, "not before the first square");
        m.step(Input::Click { col: 8, row: 8 }, 30);
        assert_eq!(m.seconds(), 3);
        m.step(Input::Pause, 0);
        m.step(Input::None, 100);
        assert_eq!(m.seconds(), 3, "paused");
        assert!(m.status().contains("TIME 3") && m.status().contains("PAUSED"));
        m.step(Input::Pause, 0);
        m.step(Input::None, 10);
        assert_eq!(m.seconds(), 4);
    }

    #[test]
    fn same_seed_same_field() {
        let field = |seed| {
            let mut m = Minefield::new(seed, 16, 16, 100);
            m.step(Input::Click { col: 5, row: 5 }, 0);
            (0..256)
                .map(|i| m.is_mine(i % 16, i / 16))
                .collect::<Vec<_>>()
        };
        assert_eq!(field(8), field(8));
        assert_ne!(field(8), field(9));
    }
}

#[test]
fn an_engine_plays_on_exactly_the_field_validation_allows() {
    // The smallest field validation lets through is the field the engine plays on.
    for &e in games::Engine::ALL {
        let mut d = data();
        let g = d.games.iter_mut().find(|g| g.engine == e).unwrap();
        (g.cols, g.rows) = e.min_size();
        let g = g.clone();
        d.validate().unwrap();
        let a = games::new_game(&g, 1, 0);
        assert_eq!((a.cols() as u32, a.rows() as u32), e.min_size(), "{e:?}");
    }
}

#[test]
fn every_game_starts_a_playing_engine_on_its_own_field() {
    let d = data();
    for g in &d.games {
        let a = games::new_game(g, 1, 0);
        assert_eq!((a.cols(), a.rows()), (g.cols as usize, g.rows as usize));
        assert_eq!(a.cells().len(), a.cols() * a.rows(), "{}", g.id);
        assert!(!a.over() && !a.paused() && a.score() == 0, "{}", g.id);
        assert!(!a.status().is_empty());
    }
}

/// Any input at all, clicks off the field included.
fn random_input(rng: &mut Rng, cols: usize, rows: usize) -> games::Input {
    use games::Input::*;
    let (col, row) = (rng.below(cols + 4), rng.below(rows + 4));
    match rng.below(10) {
        0 => None,
        1 => Up,
        2 => Down,
        3 => Left,
        4 => Right,
        5 => Fire,
        6 => Alt,
        7 => Pause,
        8 => Click { col, row },
        _ => Flag { col, row },
    }
}

#[test]
fn random_input_never_breaks_a_game() {
    let d = data();
    for g in &d.games {
        for seed in 0..20 {
            let mut a = games::new_game(g, seed, 0);
            let mut rng = Rng::new(seed ^ 0xF422);
            let (cols, rows) = (a.cols(), a.rows());
            for _ in 0..10_000 {
                let input = random_input(&mut rng, cols, rows);
                a.step(input, rng.below(4) as u32);
                assert_eq!(a.cells().len(), cols * rows, "{} seed {seed}", g.id);
                if let Some((x, y)) = a.cursor() {
                    assert!(x < cols && y < rows, "{} seed {seed}", g.id);
                }
                assert!(!a.status().is_empty());
                if a.over() {
                    // A finished game stays finished, whatever you press.
                    let still = a.cells();
                    a.step(input, 5);
                    assert_eq!(a.cells(), still, "{} seed {seed}", g.id);
                    a = games::new_game(g, seed + 100, 0);
                }
            }
        }
    }
}

#[test]
fn every_game_always_ends() {
    // Fire, and a click on each square in turn: the snake runs into the wall, the blocks pile
    // up, and the field is opened square by square.
    let d = data();
    for g in &d.games {
        for seed in 0..20 {
            let mut a = games::new_game(g, seed, 0);
            let (cols, rows) = (a.cols(), a.rows());
            let mut n = 0;
            while !a.over() {
                let (col, row) = (n % cols, n / cols % rows);
                a.step(games::Input::Click { col, row }, 1);
                a.step(games::Input::Fire, 1);
                n += 1;
                assert!(n < 100_000, "{} seed {seed} never ended", g.id);
            }
        }
    }
}

#[test]
fn every_round_deals_a_new_game_and_no_two_games_share_a_stream() {
    let d = data();
    for g in &d.games {
        let field = |seed, round| {
            let mut a = games::new_game(g, seed, round);
            // Open the middle square (a click) and drop a piece (Fire), so the seed shows.
            let (col, row) = (a.cols() / 2, a.rows() / 2);
            a.step(games::Input::Click { col, row }, 0);
            a.step(games::Input::Fire, 0);
            a.cells()
        };
        assert_eq!(field(5, 2), field(5, 2), "{}: same seed and round", g.id);
        let rounds: Vec<_> = (0..4).map(|r| field(5, r)).collect();
        assert!(
            rounds.windows(2).any(|w| w[0] != w[1]),
            "{}: every round dealt the same field",
            g.id
        );
    }
    // The same save seed and round seed each game differently.
    let seeds: Vec<u64> = d
        .games
        .iter()
        .map(|g| games::round_seed(&g.id, 5, 0))
        .collect();
    assert!(
        seeds
            .iter()
            .enumerate()
            .all(|(i, s)| !seeds[..i].contains(s))
    );
}

#[test]
fn same_seed_same_game_for_every_engine() {
    let d = data();
    for g in &d.games {
        let play = |seed: u64| {
            let mut a = games::new_game(g, seed, 0);
            let mut rng = Rng::new(77);
            let mut frames = Vec::new();
            for _ in 0..300 {
                let input = random_input(&mut rng, a.cols(), a.rows());
                a.step(input, 1);
                frames.push((a.cells(), a.score(), a.status()));
            }
            frames
        };
        assert_eq!(play(3), play(3), "{}", g.id);
    }
}
