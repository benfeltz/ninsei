//! 137.1 acceptance: a new deck's tunes, TRAX off the Gutter, a compilation by mail, and the
//! desktop's music giving way to a run and coming back after it.

use ninsei_core::data::{Data, Drop};
use ninsei_core::hub::{Save, Screen, Tool};
use ninsei_core::player::{self, Act, Play, Transport};
use ninsei_core::shopper::Item;

/// A new deck, booted once, with the Gutter's number.
fn new_deck(d: &Data) -> Save {
    let mut s = Save::default();
    s.ensure_key(0x1371);
    s.ensure_phonebook(d, 1);
    s.learn_board("gutter", "a test");
    s
}

#[test]
fn a_new_deck_has_tunes_but_no_player_until_trax_comes_off_the_gutter() {
    let d = data();
    let mut s = new_deck(&d);
    assert_eq!(s.tunes, d.starts.kit("").tunes);
    assert!(!s.opens(Tool::Player), "no player until it's found");
    assert!(!s.icons(&d).iter().any(|i| i.tool == Tool::Player));
    // TRAX is in the Gutter's always-stocked tools, free.
    let board = d.board("gutter").unwrap();
    let call = s.home_call(&d, &d.rig, board);
    let (area, file) = ninsei_core::bbs::Line::stock(&call)
        .iter()
        .enumerate()
        .find_map(|(a, files)| {
            files.iter().enumerate().find_map(|(f, st)| {
                let def = &d.boards[board].areas[a].files[st.def];
                (def.item == Drop::Tool("player".into())).then_some((a, f))
            })
        })
        .expect("TRAX on the Gutter");
    let bank = s.bank;
    let line = s.home_buy(&d, &d.rig, board, area, file).unwrap();
    assert!(line.contains("TRAX"), "{line}");
    assert_eq!(s.bank, bank, "free");
    assert!(s.opens(Tool::Player));
    assert!(s.icons(&d).iter().any(|i| i.tool == Tool::Player));
}

#[test]
fn the_jukebox_arrives_by_mail_and_nothing_plays() {
    let d = data();
    let mut s = new_deck(&d);
    s.bank = 500;
    let t = Transport::default();
    let before = s.tunes.len();
    s.buy_item(&d, &Item::Compilation("jukebox".into()))
        .unwrap();
    let jukebox = &d.compilation("jukebox").unwrap().tunes;
    assert_eq!(
        &s.tunes[before..],
        jukebox.as_slice(),
        "on the end of the playlist"
    );
    assert!(
        player::wanted(&s, &d, Screen::Hub, &t).is_none(),
        "buying plays nothing"
    );
    assert!(s.player.current.is_none());
}

#[test]
fn desktop_music_stops_for_a_run_and_comes_back_where_it_was() {
    let d = data();
    let mut s = new_deck(&d);
    s.learn_tool("player");
    let mut t = Transport::default();
    player::apply(&mut s, &d, &mut t, Act::Play);
    player::tick(&mut s, &d, &mut t, 9.5, player::at_home(Screen::Hub));
    let at_home = player::wanted(&s, &d, Screen::Hub, &t).unwrap();
    let (id, at, cue) = (at_home.id.to_string(), at_home.at, at_home.cue);
    // The dial plays on the modem's channel: the desk's music is still wanted.
    assert!(player::wanted(&s, &d, Screen::Hub, &t).is_some());
    for screen in [
        Screen::JackIn { t: 0.3 },
        Screen::Run,
        Screen::Run,
        Screen::Crash,
    ] {
        player::tick(&mut s, &d, &mut t, 20.0, player::at_home(screen));
        assert!(player::wanted(&s, &d, screen, &t).is_none(), "{screen:?}");
    }
    let back = player::wanted(&s, &d, Screen::JackOut { t: 0.2 }, &t).unwrap();
    assert_eq!((back.id, back.at, back.cue), (id.as_str(), at, cue));
    assert_eq!(t.play, Play::Playing);
    // The tune and the modes are kept on the save; where it was is not.
    let reloaded = Save::load(&s.to_text()).unwrap();
    assert_eq!(reloaded.player, s.player);
    assert_eq!(reloaded.tunes, s.tunes);
}

fn data() -> Data {
    Data::builtin().unwrap()
}
