//! 141.11.2: the boards sell hacked ICE. Each board with a `sources` block has one more file
//! area, generated: a file per kind of its ICE tiers, the newest version the scene has cracked,
//! priced by tier. Bought through the boards' one buy path, in a run and from home.

use ninsei_core::bbs::{Line, Screen, Session, source_file};
use ninsei_core::data::{BbsFileDef, Drop};
use ninsei_core::hub::{IceVer, Save};
use ninsei_core::ice_version::{Bump, Version};
use ninsei_core::tuning::{SOURCE_CRACK_STEPS, SOURCE_PRICE, SOURCE_PRICE_FIRST};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

fn board(d: &Data, id: &str) -> usize {
    d.board(id).unwrap()
}

/// A run on board `b`'s network, standing on its BBS with `credits`.
fn at_board(d: &Data, kit: &Kit, b: usize, credits: u32) -> Game {
    let kit = Kit {
        board: b,
        ..kit.clone()
    };
    let mut g = Game::start(d.clone(), 11, &kit);
    let bbs = g.net.bbs().expect("every network has a bbs");
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
    g.deck.credits = credits;
    g.deck.trace = 0;
    g
}

/// The source area's files on a line, if its board has one: the last area.
fn sources(line: &dyn Line) -> Vec<BbsFileDef> {
    let b = line.board();
    match b.areas.last() {
        Some(a) if b.sources.is_some() => a.files.clone(),
        _ => Vec::new(),
    }
}

fn listed(files: &[BbsFileDef]) -> Vec<(String, Version, u32)> {
    files
        .iter()
        .map(|f| match &f.item {
            Drop::Source { ice, version } => (ice.clone(), *version, f.price),
            other => panic!("not a source: {other:?}"),
        })
        .collect()
}

fn home(d: &Data) -> Save {
    let mut s = Save::default();
    s.ensure_key(0x0001_4112);
    s.ensure_phonebook(d, 0);
    s.ensure_hardware(d);
    for b in &d.boards {
        s.learn_board(&b.id, "test");
    }
    s.bank = 10_000;
    s
}

fn enigma_patch(d: &Data) -> IceVer {
    let row = d.ice.iter().find(|i| i.id == "enigma").unwrap();
    IceVer {
        id: "enigma".into(),
        version: Version::FIRST.bumped(Bump::Patch),
        program: row.program.replace("TRACE 10", "TRACE 9"),
        bump: Some(Bump::Patch),
        crack: SOURCE_CRACK_STEPS,
    }
}

#[test]
fn each_board_lists_its_own_tiers_at_1_0_0_from_the_start() {
    let d = data();
    let s = home(&d);
    let first = Version::FIRST;
    let kinds = |tiers: &[u32]| -> Vec<(String, Version, u32)> {
        d.ice
            .iter()
            .filter(|i| tiers.contains(&i.tier))
            .map(|i| (i.id.clone(), first, SOURCE_PRICE_FIRST[i.tier as usize]))
            .collect()
    };
    let call = |id: &str| s.home_call(&d, &d.rig, board(&d, id));
    assert_eq!(listed(&sources(&call("gutter"))), kinds(&[1]));
    assert_eq!(listed(&sources(&call("deadsector"))), kinds(&[2, 0]));
    assert_eq!(listed(&sources(&call("corplink"))), kinds(&[2]));
    assert!(
        sources(&call("sensenet")).is_empty(),
        "the customer line sells none"
    );
    // The same in a run, on each board's network.
    for b in 0..d.boards.len() {
        let g = at_board(&d, &Kit::default(), b, 0);
        assert_eq!(
            listed(&sources(&g)),
            listed(&sources(&s.home_call(&d, &d.rig, b))),
            "{}",
            d.boards[b].id
        );
    }
    // 8.3 names, one per kind, the version in the extension.
    let files = sources(&call("gutter"));
    assert!(files.iter().any(|f| f.file == "ENIGMA.100"), "{files:?}");
    assert!(files.iter().all(|f| {
        let (stem, ext) = f.file.split_once('.').unwrap();
        stem.len() <= 8 && ext.len() == 3
    }));
    assert_eq!(
        source_file("Ice Wall", Version::FIRST.bumped(Bump::Minor)),
        "ICEWALL.110"
    );
}

#[test]
fn a_shipped_version_is_on_the_board_only_once_cracked() {
    let d = data();
    let mut s = home(&d);
    s.ice_versions.push(enigma_patch(&d));
    let v = s.ice_versions[0].version;
    let gutter = board(&d, "gutter");
    let enigma = |s: &Save| {
        listed(&sources(&s.home_call(&d, &d.rig, gutter)))
            .into_iter()
            .find(|(id, ..)| id == "enigma")
            .unwrap()
    };
    assert_eq!(
        enigma(&s),
        ("enigma".into(), Version::FIRST, SOURCE_PRICE_FIRST[1])
    );
    s.study(&std::sync::Arc::new(d.clone()), SOURCE_CRACK_STEPS, 0);
    assert_eq!(enigma(&s), ("enigma".into(), v, SOURCE_PRICE[1]));
    // A run jacked in before the crack lists 1.0.0; one after, the new version.
    s.ice_versions[0].crack = 1;
    let g = at_board(&d, &s.kit(&d), gutter, 0);
    assert!(listed(&sources(&g)).contains(&(
        "enigma".into(),
        Version::FIRST,
        SOURCE_PRICE_FIRST[1]
    )));
    s.ice_versions[0].crack = 0;
    let g = at_board(&d, &s.kit(&d), gutter, 0);
    assert!(listed(&sources(&g)).contains(&("enigma".into(), v, SOURCE_PRICE[1])));
}

/// The source area's index and Enigma's file in it, on line `l`.
fn enigma_file(l: &dyn Line) -> (usize, usize) {
    let a = l.board().areas.len() - 1;
    let f = sources(l)
        .iter()
        .position(|f| matches!(&f.item, Drop::Source { ice, .. } if ice == "enigma"))
        .unwrap();
    (a, f)
}

#[test]
fn bought_in_a_run_it_lists_in_the_next_fight_and_comes_home_on_a_flatline() {
    let d = data();
    let mut s = home(&d);
    let gutter = board(&d, "gutter");
    let mut kit = s.kit(&d);
    kit.hdd = 0; // a full HDD: a source takes no slot.
    let mut g = at_board(&d, &kit, gutter, 500);
    let (a, f) = enigma_file(&g);
    assert_eq!(g.buy_blocker(a, f), None, "no slot needed");
    g.apply(Action::Buy { area: a, file: f });
    assert_eq!(g.deck.credits, 500 - SOURCE_PRICE_FIRST[1]);
    let e = d.ice_by_id("enigma").unwrap();
    assert!(g.owns_source(e, Version::FIRST));
    assert!(
        g.log.iter().any(|l| l.text == "+ ICE source: Enigma 1.0.0"),
        "{:#?}",
        g.log
    );
    // It lists as yours now, and can't be bought twice.
    let sess = Session {
        screen: Screen::Area(a),
        status: None,
    };
    assert!(sess.render(&g).iter().any(|l| l.contains("[yours]")));
    assert!(g.buy_blocker(a, f).is_some());
    // The next fight with it lists its program.
    let n = g.net.nodes[g.at].links[0];
    g.net.nodes[n].kind = NodeKind::Ice;
    g.net.nodes[n].ice = Some(e);
    g.net.nodes[n].cleared = false;
    g.apply(Action::Move(n));
    assert_eq!(g.phase, Phase::Combat);
    assert_eq!(ice_listing::rows(&g).len(), 4);
    // Flatline: it comes home.
    g.deck.integrity = 1;
    let c = g.combat.as_mut().unwrap();
    (c.strength, c.strength_max) = (500, 500);
    while g.phase == Phase::Combat {
        g.apply(Action::EndTurn);
    }
    assert_eq!(g.phase, Phase::Flatlined);
    s.record(&g, &[], &d.rig).unwrap();
    assert!(s.owns_source(&d, "enigma", Version::FIRST));
}

#[test]
fn bought_from_home_it_is_yours_and_takes_no_stash_room() {
    let d = data();
    let mut s = home(&d);
    let dead = board(&d, "deadsector");
    let call = s.home_call(&d, &d.rig, dead);
    let a = call.board().areas.len() - 1;
    let f = sources(&call)
        .iter()
        .position(|f| matches!(&f.item, Drop::Source { ice, .. } if ice == "hydra"))
        .unwrap();
    let (stash, bank) = (s.stash.len(), s.bank);
    let said = s.home_buy(&d, &d.rig, dead, a, f).unwrap();
    assert!(said.contains("Hydra 1.0.0"), "{said}");
    assert_eq!(s.bank, bank - SOURCE_PRICE_FIRST[2]);
    assert_eq!(s.stash.len(), stash);
    assert!(s.owns_source(&d, "hydra", Version::FIRST));
    let call = s.home_call(&d, &d.rig, dead);
    assert!(call.buy_blocker(a, f).is_some(), "yours now");
    assert!(s.home_buy(&d, &d.rig, dead, a, f).is_err());
    assert_eq!(s.bank, bank - SOURCE_PRICE_FIRST[2]);
}

#[test]
fn the_source_area_never_touches_the_run_s_dice() {
    let d = data();
    let mut bare = data();
    for b in &mut bare.boards {
        b.sources = None;
    }
    for b in 0..d.boards.len() {
        let kit = Kit {
            board: b,
            ..Kit::default()
        };
        let g = Game::start(d.clone(), 77, &kit);
        let h = Game::start(bare.clone(), 77, &kit);
        let own = h.bbs_stock.len();
        assert_eq!(g.bbs_stock[..own], h.bbs_stock[..], "{}", d.boards[b].id);
        assert_eq!(g.net.nodes.len(), h.net.nodes.len());
        let mut a = g.clone();
        let mut c = h.clone();
        for _ in 0..5 {
            a.apply(Action::EndTurn);
            c.apply(Action::EndTurn);
        }
        let log = |g: &Game| g.log.iter().map(|l| l.text.clone()).collect::<Vec<_>>();
        assert_eq!(log(&a), log(&c));
    }
}

#[test]
fn the_tenth_area_is_picked_with_zero() {
    let d = data();
    let g = at_board(&d, &Kit::default(), board(&d, "gutter"), 0);
    assert_eq!(
        g.bbs().areas.len(),
        10,
        "nine of its own and the source area"
    );
    let mut sess = Session {
        screen: Screen::Areas,
        status: None,
    };
    let rows = sess.render(&g).iter().filter(|l| l.contains("(s)")).count();
    assert_eq!(rows, 10);
    assert_eq!(ninsei_core::bbs::area_key(9), '0');
    assert!(sess.keys(&g).iter().any(|(k, _)| *k == '0'));
    sess.key(&g, '0');
    assert_eq!(sess.screen, Screen::Area(9));
}

#[test]
fn a_bad_sources_block_is_refused() {
    let mut d = data();
    let g = d.board("gutter").unwrap();
    d.boards[g].sources.as_mut().unwrap().patched = "{ice} {version} {price}".into();
    assert!(d.validate().is_err());
    let mut d = data();
    d.boards[g].sources.as_mut().unwrap().tiers = vec![7];
    assert!(d.validate().is_err());
    let mut d = data();
    d.boards[g].sources.as_mut().unwrap().first =
        "{ice} {version}, the very classic of classics".into();
    assert!(d.validate().is_err(), "too wide");
    let mut d = data();
    d.boards[g].areas[0].files[0].item = Drop::Source {
        ice: "enigma".into(),
        version: Version::FIRST,
    };
    assert!(d.validate().is_err(), "sources are generated");
}

#[test]
fn a_source_gone_from_the_nets_mid_run_cant_be_bought_and_costs_nothing() {
    let d = data();
    let mut s = home(&d);
    let mut v = enigma_patch(&d);
    v.crack = 0;
    s.ice_versions.push(v.clone());
    let gutter = board(&d, "gutter");
    let mut g = at_board(&d, &s.kit(&d), gutter, 500);
    let (a, f) = enigma_file(&g);
    assert!(listed(&sources(&g)).contains(&("enigma".into(), v.version, SOURCE_PRICE[1])));
    // The sysops ship over it mid-run: the listing, fixed at jack-in, still offers 1.0.1.
    let e = d.ice_by_id("enigma").unwrap();
    let newer = v.version.bumped(Bump::Patch);
    let prog = g.ice_versions.iter().find(|x| x.0 == e).unwrap().2.clone();
    g.ice_versions.retain(|x| x.0 != e);
    g.ice_versions.push((e, newer, prog, Some(Bump::Patch)));
    let why = g.buy_blocker(a, f).expect("refused");
    assert!(why.contains("out of date"), "{why}");
    g.apply(Action::Buy { area: a, file: f });
    assert_eq!(g.deck.credits, 500, "nothing charged");
    assert!(g.sources_got.is_empty());
}
