//! Owning tunes: music disks on the boards, a compilation in the magazine, the kit's tunes.

use ninsei_core::bbs::Line;
use ninsei_core::data::{Data, Drop};
use ninsei_core::hub::Save;
use ninsei_core::shopper::{Item, Section, Stock};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

/// A run whose network's BBS is `board`, walked onto the board with the way cleared.
fn run_on_board(d: &Data, board: usize, tunes: Vec<String>) -> Game {
    let kit = Kit {
        board,
        tunes,
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

/// Where `tune`'s file sits in today's listing.
fn tune_file(line: &dyn Line, tune: &str) -> (usize, usize) {
    for (a, files) in line.stock().iter().enumerate() {
        for (f, s) in files.iter().enumerate() {
            if line.board().areas[a].files[s.def].item == Drop::Tune(tune.into()) {
                return (a, f);
            }
        }
    }
    panic!("no {tune} on {}", line.board().id)
}

#[test]
fn a_fresh_save_owns_the_kits_tunes() {
    let d = data();
    let mut s = Save::default();
    s.ensure_phonebook(&d, 1);
    let kit = d.starts.kit("");
    assert!(!kit.tunes.is_empty() && kit.tunes.len() <= 2);
    assert_eq!(s.tunes, kit.tunes);
    // ...and nothing else: the player itself is found on a board.
    assert!(!s.has_tool("player"));
}

#[test]
fn a_save_from_before_tunes_loads_with_none() {
    let mut s = Save::default();
    s.learn_tune("chroma");
    let text = s.to_text();
    let old = text.replace("tunes:", "_gone:");
    assert_ne!(old, text);
    let back = Save::load(&old).expect("a save without tunes still loads");
    assert!(back.tunes.is_empty());
    assert_eq!(Save::load(&text).unwrap().tunes, vec!["chroma".to_string()]);
}

#[test]
fn a_kit_naming_no_tune_is_refused() {
    let mut d = data();
    d.starts.kits[0].tunes.push("nope".into());
    assert!(d.validate().unwrap_err().contains("unknown tune `nope`"));
}

#[test]
fn a_board_file_naming_no_tune_is_refused() {
    let mut d = data();
    let b = d.board("gutter").unwrap();
    d.boards[b].areas[0].files[0].item = Drop::Tune("nope".into());
    assert!(d.validate().unwrap_err().contains("unknown tune `nope`"));
}

#[test]
fn a_tune_never_drops_from_loot() {
    let mut d = data();
    d.loot.ice.entries[0].drop = Drop::Tune("chroma".into());
    assert!(d.validate().unwrap_err().contains("drops a tune"));
}

#[test]
fn the_gutter_and_dead_sector_post_music_disks_without_a_roll() {
    let d = data();
    for (board, tune) in [("gutter", "ninsei-nights"), ("deadsector", "trap-line")] {
        let b = &d.boards[d.board(board).unwrap()];
        let area = b
            .areas
            .iter()
            .find(|a| a.files.iter().any(|f| f.item == Drop::Tune(tune.into())))
            .unwrap();
        assert!(area.always, "{board}: music never touches the run's RNG");
    }
}

#[test]
fn a_tune_downloaded_in_a_run_comes_home_once_even_on_a_flatline() {
    let d = data();
    let board = d.board("gutter").unwrap();
    let mut g = run_on_board(&d, board, Vec::new());
    let (area, file) = tune_file(&g, "chroma");
    assert!(g.buy_blocker(area, file).is_none());
    g.apply(Action::Buy { area, file });
    assert!(g.has_tune("chroma"));
    g.phase = Phase::Flatlined;
    let mut save = Save::default();
    let report = save.record(&g, &[], &d.rig).unwrap();
    assert_eq!(save.tunes, vec!["chroma".to_string()]);
    assert_eq!(report.tunes, vec!["CHROMA".to_string()]);
    // The next run carries it, so the board lists it as yours.
    let kit = save.kit(&d);
    let mut g = run_on_board(&d, board, kit.tunes);
    let (area, file) = tune_file(&g, "chroma");
    assert!(g.buy_blocker(area, file).is_some());
    g.phase = Phase::JackedOut;
    let report = save.record(&g, &[], &d.rig).unwrap();
    assert!(report.tunes.is_empty());
    assert_eq!(save.tunes, vec!["chroma".to_string()], "once");
}

#[test]
fn a_tune_downloaded_at_home_goes_on_the_end_of_the_playlist_once() {
    let d = data();
    let board = d.board("deadsector").unwrap();
    let mut s = Save::default();
    s.ensure_key(0xD15C);
    s.learn_tune("welcome");
    s.learn_board("deadsector", "a test");
    s.bank = 100;
    let call = s.home_call(&d, &d.rig, board);
    let (area, file) = tune_file(&call, "trap-line");
    let line = s.home_buy(&d, &d.rig, board, area, file).unwrap();
    assert!(line.contains("trap line"), "{line}");
    assert_eq!(
        s.tunes,
        vec!["welcome".to_string(), "trap-line".to_string()]
    );
    let call = s.home_call(&d, &d.rig, board);
    let (area, file) = tune_file(&call, "trap-line");
    assert!(call.buy_blocker(area, file).is_some(), "once");
}

#[test]
fn paper_tigers_compilation_is_in_the_games_section_and_grants_its_tunes() {
    let d = data();
    let jukebox = Item::Compilation("jukebox".into());
    let mut s = Save {
        bank: 100,
        ..Save::default()
    };
    let cat = s.catalog(&d);
    let l = cat.iter().find(|l| l.item == jukebox).unwrap();
    assert_eq!(l.section, Section::Games);
    assert_eq!(l.vendor.as_deref(), Some("papertiger"));
    let c = d.compilation("jukebox").unwrap();
    assert_eq!(l.stock, Stock::Price(c.price), "priced in its own file");
    s.buy_item(&d, &jukebox).unwrap();
    assert_eq!(s.bank, 100 - c.price);
    assert_eq!(s.tunes, c.tunes);
    // Owned now: listed greyed, and it won't sell twice.
    let cat = s.catalog(&d);
    let l = cat.iter().find(|l| l.item == jukebox).unwrap();
    assert_eq!(l.stock, Stock::Owned(c.price));
    assert!(s.buy_item(&d, &jukebox).is_err());
    assert_eq!(s.bank, 100 - c.price);
}

#[test]
fn a_compilation_adds_only_the_tunes_you_lack() {
    let d = data();
    let c = d.compilation("jukebox").unwrap();
    let mut s = Save {
        bank: 100,
        ..Save::default()
    };
    s.learn_tune(&c.tunes[1]);
    s.buy_compilation(&d, "jukebox").unwrap();
    assert_eq!(s.tunes, vec![c.tunes[1].clone(), c.tunes[0].clone()]);
}

#[test]
fn a_bad_compilation_is_refused() {
    let mut d = data();
    d.compilations[0].tunes.push("nope".into());
    assert!(d.validate().unwrap_err().contains("unknown tune"));
    let mut d = data();
    d.compilations[0].publisher = "kr4ken".into();
    assert!(d.validate().unwrap_err().contains("unknown publisher"));
    let mut d = data();
    d.compilations[0].price = 0;
    assert!(d.validate().unwrap_err().contains("price"));
    let mut d = data();
    let t = d.compilations[0].tunes[0].clone();
    d.compilations[0].tunes.push(t);
    assert!(d.validate().unwrap_err().contains("twice"));
}
