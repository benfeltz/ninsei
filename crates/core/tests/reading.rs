use ninsei_core::bbs::{Outcome, Screen, Session};
use ninsei_core::game::Reading;
use ninsei_core::hub::{FIRST_BOARD, Save};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

fn fresh(d: &Data) -> Save {
    let mut s = Save::default();
    s.ensure_key(0x5E5E);
    s.ensure_phonebook(d, 1);
    s
}

/// Where the Gutter post is on Sense/Net: (base, post).
fn gutter_post(d: &Data) -> (usize, usize) {
    let b = &d.boards[d.board(FIRST_BOARD).unwrap()];
    for (base, m) in b.messages.iter().enumerate() {
        for (post, p) in m.posts.iter().enumerate() {
            if p.on_read == Some(data::ReadEffect::Board("gutter".into())) {
                return (base, post);
            }
        }
    }
    panic!("no post on Sense/Net leads to the Gutter")
}

#[test]
fn sense_net_hands_out_the_first_numbers_and_the_gutter() {
    let d = data();
    let mut s = fresh(&d);
    let board = d.board(FIRST_BOARD).unwrap();
    // Free to call with an empty bank.
    s.call_board(&d, &d.rig, board).unwrap();
    let def = &d.boards[board];
    let dir = (0..def.bulletins.len())
        .find(|&i| def.bulletins[i].on_read == Some(data::ReadEffect::Starters))
        .expect("a partner directory");
    let msg = s.home_read(&d, board, Reading::Bulletin(dir)).unwrap();
    assert!(msg.starts_with("New numbers in CarrierTerm"), "{msg}");
    assert_eq!(
        s.phonebook.len(),
        d.starts.always.len() + tuning::PHONEBOOK_START.len()
    );
    assert_eq!(
        s.home_read(&d, board, Reading::Bulletin(dir)).unwrap(),
        "",
        "once"
    );
    assert!(!s.knows_board("gutter"));
    let (base, post) = gutter_post(&d);
    let msg = s
        .home_read(&d, board, Reading::Post { base, post })
        .unwrap();
    assert!(msg.contains("THE NEON GUTTER"), "{msg}");
    assert!(s.knows_board("gutter"));
    assert!(
        s.home_read(&d, board, Reading::Bulletin(0)).is_err(),
        "a plain bulletin does nothing"
    );
}

#[test]
fn a_post_read_in_a_run_comes_home() {
    let d = data();
    let board = d.board(FIRST_BOARD).unwrap();
    let kit = Kit {
        board,
        ..Kit::default()
    };
    let mut g = Game::start(d.clone(), 11, &kit);
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
    let mut sess = Session::new();
    sess.screen = Screen::Main;
    let keys: Vec<char> = sess.keys(&g).iter().map(|(k, _)| *k).collect();
    assert!(keys.contains(&'M') && !keys.contains(&'J'), "{keys:?}");
    sess.key(&g, 'm');
    assert_eq!(sess.screen, Screen::Messages);
    let (base, post) = gutter_post(&d);
    sess.key(&g, char::from_digit(base as u32 + 1, 10).unwrap());
    let out = sess.key(&g, char::from_digit(post as u32 + 1, 10).unwrap());
    assert_eq!(
        out,
        Outcome::Act(Action::Read(Reading::Post { base, post }))
    );
    assert!(sess.render(&g).join("\n").contains("555-0199"));
    g.apply(Action::Read(Reading::Post { base, post }));
    assert_eq!(g.boards_read, vec!["gutter".to_string()]);
    let mut s = fresh(&d);
    g.phase = Phase::Flatlined;
    s.record(&g, &[], &d.rig).unwrap();
    assert!(
        s.knows_board("gutter"),
        "what you read comes home, even on a flatline"
    );
}

#[test]
fn boards_without_messages_have_no_messages_menu() {
    let d = data();
    let g = Game::new(d.clone(), 2);
    let mut sess = Session::new();
    sess.screen = Screen::Main;
    let keys: Vec<char> = sess.keys(&g).iter().map(|(k, _)| *k).collect();
    assert!(!keys.contains(&'M') && keys.contains(&'J'), "{keys:?}");
}

#[test]
fn the_shops_disk_plays_a_cracktro_that_teaches_the_gutter_once() {
    let d = data();
    let mut s = fresh(&d);
    assert!(!s.knows_board("gutter"));
    let crack = d.game("nibbler").unwrap().crack.clone();
    assert!(s.saw_intro(&d, &crack), "the first play books the Gutter");
    let k = s.boards.iter().find(|k| k.id == "gutter").unwrap();
    assert_eq!(k.source, "a cracktro");
    // A replay changes nothing.
    let before = s.clone();
    assert!(!s.saw_intro(&d, &crack));
    assert_eq!(s.to_text(), before.to_text());
    // A splash advertises no board, and an unknown intro is nobody's.
    let splash = d.game("nibbler").unwrap().splash.clone();
    assert!(!s.saw_intro(&d, &splash) && !s.saw_intro(&d, "no-such-intro"));
    assert_eq!(s.boards.len(), before.boards.len());
    // Sense/Net's post still leads there, and doesn't book it twice.
    let effect = data::ReadEffect::Board("gutter".into());
    assert!(s.read_effect(&d, &effect, "Sense/Net").is_none());
    assert_eq!(s.boards.iter().filter(|k| k.id == "gutter").count(), 1);
}

#[test]
fn the_sense_net_post_still_teaches_the_gutter_and_the_cracktro_then_adds_nothing() {
    let d = data();
    let mut s = fresh(&d);
    let (base, post) = gutter_post(&d);
    let p = &d.boards[d.board(FIRST_BOARD).unwrap()].messages[base].posts[post];
    assert!(
        s.read_effect(&d, p.on_read.as_ref().unwrap(), "Sense/Net")
            .is_some()
    );
    let crack = d.game("nibbler").unwrap().crack.clone();
    assert!(!s.saw_intro(&d, &crack), "already known from the post");
    assert_eq!(s.boards.iter().filter(|k| k.id == "gutter").count(), 1);
    assert!(
        s.boards
            .iter()
            .any(|k| k.id == "gutter" && k.source.contains("Sense/Net"))
    );
}
