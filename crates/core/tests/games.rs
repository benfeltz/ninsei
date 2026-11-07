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
    assert_eq!(d.games.len(), data::GAMES.len());
    for id in data::GAMES {
        let g = d.game(id).expect("a row for every engine");
        assert!(!g.name.is_empty() && !g.blurb.is_empty() && !g.version.is_empty());
        assert_eq!(
            g.publisher, "papertiger",
            "{id}: Paper Tiger boxes the games"
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
    assert!(check(|g| g.splash = crack.clone()).contains("is a crack"));
    assert!(check(|g| g.publisher = "kowloon".into()).contains("unknown publisher"));
    assert!(check(|g| g.cols = 0).contains("field"));
    assert!(check(|g| g.rows = 99).contains("field"));
    assert!(check(|g| g.id = "pong".into()).contains("no engine"));
    assert!(check(|g| g.shop = Some(data::GameShop { price: 0 })).contains("for nothing"));
    let mut d = data();
    d.games.pop();
    assert!(d.validate().unwrap_err().contains("no row for game"));
    let mut d = data();
    let twice = d.games[0].clone();
    d.games.push(twice);
    assert!(d.validate().unwrap_err().contains("listed twice"));
}

#[test]
fn the_box_never_erases_the_crack() {
    use hub::GameCopy::{Boxed, Cracked};
    let mut s = Save::default();
    assert!(!s.has_game("blox") && s.game_copy("blox").is_none());
    assert!(s.learn_game("blox", Cracked));
    assert!(!s.learn_game("blox", Cracked), "once");
    assert_eq!(s.game_copy("blox"), Some(Cracked));
    assert!(s.learn_game("blox", Boxed), "the box goes beside the crack");
    assert!(s.has_copy("blox", Cracked) && s.has_copy("blox", Boxed));
    assert_eq!(s.game_copy("blox"), Some(Boxed), "it opens with the box you paid for");
    assert!(s.learn_game("nibbler", Boxed));
    assert!(!s.has_copy("nibbler", Cracked));
    assert!(s.learn_game("nibbler", Cracked), "a crack goes beside the box too");
    assert!(s.has_copy("nibbler", Boxed) && s.has_copy("nibbler", Cracked));
    assert_eq!(s.games.len(), 2, "one row a game");
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
    assert_eq!(kept.game_copy("blox"), Some(hub::GameCopy::Boxed));
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
    assert_eq!(save.game_copy("blox"), Some(GameCopy::Cracked));
    assert_eq!(report.games, vec!["BLOX".to_string()]);
    // A board won't sell the crack again; the run's kit carries it from the save.
    let kit = save.kit(&d, &d.rig);
    assert_eq!(kit.games, vec!["blox".to_string()]);
    let mut g = run_on_board(&d, board, kit.games);
    let (area, file) = blox_file(&g);
    assert!(g.buy_blocker(area, file).unwrap().contains("already have"));
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
    assert!(line.contains("BLOX"), "{line}");
    assert_eq!(s.game_copy("blox"), Some(GameCopy::Cracked));
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

#[test]
fn a_game_bought_by_mail_is_boxed() {
    let d = data();
    let mut s = Save {
        bank: 200,
        ..Save::default()
    };
    let item = Item::Game("nibbler".into());
    let price = d.game("nibbler").unwrap().shop.as_ref().unwrap().price;
    assert!(s.order_blocker(&d, &item).is_none());
    s.buy_item(&d, &item).unwrap();
    assert_eq!(s.game_copy("nibbler"), Some(GameCopy::Boxed));
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
    let d = data();
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
