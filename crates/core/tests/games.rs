//! Games as data: `games.ron`, the copy you own, and the ways you come by one.

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
        assert_eq!(g.publisher, "papertiger", "{id}: Paper Tiger boxes the games");
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
        assert!(splash.board.is_none(), "{}: a publisher advertises no board", g.id);
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
