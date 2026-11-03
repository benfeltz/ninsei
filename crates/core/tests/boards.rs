//! Several BBSes as data: each network reaches one board, picked by tier, trait or number,
//! and each board has its own look.

use ninsei_core::bbs::{COLS, Screen, Session};
use ninsei_core::hub::{Entry, Save};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

fn entry(number: &str, tier: u32, traits: &[&str]) -> Entry {
    Entry {
        id: 0,
        name: "probe".into(),
        business: "test".into(),
        number: number.into(),
        tier,
        traits: traits.iter().map(|t| t.to_string()).collect(),
        heat: 0,
        runs: 0,
    }
}

fn on_board(board: usize) -> Game {
    let kit = Kit {
        board,
        ..Kit::default()
    };
    Game::start(data(), 3, &kit)
}

fn render(g: &Game, screen: Screen) -> Vec<String> {
    Session {
        screen,
        status: None,
    }
    .render(g)
}

#[test]
fn the_shipped_boards() {
    let d = data();
    let ids: Vec<&str> = d.boards.iter().map(|b| b.id.as_str()).collect();
    assert_eq!(ids, ["gutter", "corplink", "deadsector"], "default first");
    assert!(d.boards[0].default);
    assert_eq!(d.boards[0].name, "THE NEON GUTTER");
    assert!(d.boards.iter().all(|b| !b.splash.is_empty()));
}

#[test]
fn plain_runs_still_reach_the_gutter() {
    for seed in 0..20 {
        assert_eq!(Game::new(data(), seed).bbs().id, "gutter");
    }
}

#[test]
fn the_board_never_changes_the_network() {
    let d = data();
    let a = Game::new(d.clone(), 7);
    let c = Game::start(
        d.clone(),
        7,
        &Kit {
            board: d.board("corplink").unwrap(),
            ..Kit::default()
        },
    );
    assert_ne!(a.bbs().id, c.bbs().id);
    let shape = |g: &Game| {
        g.net
            .nodes
            .iter()
            .map(|n| (n.kind, n.layer, n.links.clone(), n.ice))
            .collect::<Vec<_>>()
    };
    assert_eq!(shape(&a), shape(&c));
}

#[test]
fn a_number_always_reaches_the_same_board() {
    let d = data();
    for n in 0..200 {
        let number = format!("555-{n:04}");
        for tier in 1..=3 {
            let e = entry(&number, tier, &[]);
            let b = Save::site_board(&d, &e);
            assert_eq!(b, Save::site_board(&d, &e));
            assert!(
                d.boards[b].tiers.contains(&tier),
                "{} isn't a tier {tier} board",
                d.boards[b].id
            );
        }
    }
}

#[test]
fn each_tier_has_its_board_and_better_stock() {
    let d = data();
    let id = |tier| Save::site_board(&d, &entry("555-0123", tier, &[])).to_owned();
    let ids: Vec<&str> = (1..=3).map(|t| d.boards[id(t)].id.as_str()).collect();
    assert_eq!(ids, ["gutter", "deadsector", "corplink"]);
    // A pirate can still run Dead Sector off any backbone.
    let pirate = Save::site_board(&d, &entry("555-0123", 1, &["pirate_board"]));
    assert_eq!(d.boards[pirate].id, "deadsector");

    // The best things only sell on the top board, and each board's dearest file costs more.
    let sells = |b: usize, f: &dyn Fn(&Drop) -> bool| {
        d.boards[b]
            .areas
            .iter()
            .flat_map(|a| &a.files)
            .any(|x| f(&x.item))
    };
    let top = |b: usize| {
        d.boards[b]
            .areas
            .iter()
            .flat_map(|a| &a.files)
            .map(|x| x.price)
            .max()
            .unwrap()
    };
    for t in 1..=2 {
        let b = id(t);
        assert!(
            !sells(b, &|i| matches!(i, Drop::Cpu(_))),
            "a CPU mod below tier 3"
        );
        assert!(
            !sells(b, &|i| *i == Drop::Warez("torch".into())),
            "torch below tier 3"
        );
        assert!(
            top(b) < top(id(t + 1)),
            "tier {t}'s board isn't cheaper than the next"
        );
    }
    assert!(!sells(
        id(1),
        &|i| matches!(i, Drop::Hint(r) if r.ends_with("_core"))
    ));
}

#[test]
fn a_trait_can_pick_the_board() {
    let d = data();
    let dead = d.board("deadsector").unwrap();
    for n in 0..50 {
        let e = entry(&format!("555-{n:04}"), 2, &["pirate_board"]);
        assert_eq!(Save::site_board(&d, &e), dead);
    }
}

#[test]
fn dialing_takes_you_to_the_numbers_board() {
    let d = data();
    let mut s = Save::default();
    s.ensure_key(1);
    s.phonebook.push(entry("555-0001", 2, &["pirate_board"]));
    let g = s.dial(&d, 0, 5).unwrap();
    assert_eq!(g.bbs().id, "deadsector");
    // The stock comes from that board's areas.
    assert_eq!(g.bbs_stock.len(), g.bbs().areas.len());
}

#[test]
fn the_dial_log_names_the_board() {
    let mut g = on_board(data().board("corplink").unwrap());
    let to = g.net.nodes[g.at].links[0];
    g.net.nodes[to].kind = NodeKind::Bbs;
    g.apply(Action::Move(to));
    let line = &g
        .log
        .iter()
        .find(|l| l.text.contains("CONNECT"))
        .unwrap()
        .text;
    assert!(line.contains("CORPLINK EMPLOYEE SERVICES"), "{line}");
    assert!(line.contains("555-0100"), "{line}");
}

#[test]
fn every_board_looks_different() {
    let d = data();
    let mains: Vec<Vec<String>> = (0..d.boards.len())
        .map(|b| render(&on_board(b), Screen::Main))
        .collect();
    for (i, a) in mains.iter().enumerate() {
        for b in &mains[i + 1..] {
            assert_ne!(a[0], b[0], "headers differ");
            assert_ne!(a[2], b[2], "menus differ");
        }
    }
    // Every header style fills the full width.
    for m in &mains {
        assert_eq!(ansi::visible_len(&m[0]), COLS, "{}", ansi::plain(&m[0]));
    }
}

#[test]
fn leet_is_per_board() {
    let corp = on_board(data().board("corplink").unwrap());
    let main = render(&corp, Screen::Main).join("\n");
    assert!(main.contains("Software Library"), "{main}");
    let dead = on_board(data().board("deadsector").unwrap());
    let main = render(&dead, Screen::Main).join("\n");
    assert!(main.contains("W4r3z V4ult"), "{main}");
    let bye = render(&corp, Screen::Hangup { t: 0.0 });
    assert!(bye[0].contains("SESSION TERMINATED"), "{bye:?}");
}

fn parse_boards(boards: &[BoardSrc]) -> Result<Data, String> {
    Data::parse(Sources {
        boards,
        ..Sources::builtin()
    })
}

#[test]
fn bad_boards_are_caught() {
    let b = data::BUILTIN_BOARDS;
    let swap = |id: &str, from: &str, to: &str| -> Vec<(String, String, String)> {
        b.iter()
            .map(|s| {
                let board = if s.id == id {
                    assert!(s.board.contains(from), "{from}");
                    s.board.replacen(from, to, 1)
                } else {
                    s.board.to_string()
                };
                (s.id.to_string(), board, s.splash.to_string())
            })
            .collect()
    };
    let check = |owned: Vec<(String, String, String)>, want: &str| {
        let srcs: Vec<BoardSrc> = owned
            .iter()
            .map(|(id, board, splash)| BoardSrc { id, board, splash })
            .collect();
        let err = parse_boards(&srcs).unwrap_err();
        assert!(err.contains(want), "wanted `{want}` in: {err}");
    };
    // Cross-references in every board are checked, not just the default.
    check(
        swap("deadsector", "Warez(\"decoder\")", "Warez(\"nope\")"),
        "bbs/deadsector: file `DCDR-HOT.ZIP` names unknown warez `nope`",
    );
    check(
        swap("corplink", "Hint(\"saito_core\")", "Hint(\"atlantis\")"),
        "atlantis",
    );
    check(
        swap("corplink", "tiers: [3]", "tiers: [9]"),
        "unknown tier 9",
    );
    check(swap("corplink", "label: 11", "label: 16"), "look.label");
    check(
        swap("deadsector", "tiers: [2],", "tiers: [2], default: true,"),
        "exactly one",
    );
    // Only the boards that exist can be named by a trait.
    let nets = include_str!("../../../data/hub/networks.ron")
        .replace("Board(\"deadsector\")", "Board(\"atlantis\")");
    let err = Data::parse(Sources {
        networks: &nets,
        ..Sources::builtin()
    })
    .unwrap_err();
    assert!(err.contains("unknown board `atlantis`"), "{err}");
}
