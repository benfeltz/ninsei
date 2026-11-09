//! The desktop's music player in core: the playlist, the transport, and what the speaker plays.

use ninsei_core::data::Data;
use ninsei_core::hub::{Save, Screen};
use ninsei_core::player::{self, Act, Play, Transport};

fn data() -> Data {
    Data::builtin().unwrap()
}

/// A save owning `tunes`, in that order.
fn owning(tunes: &[&str]) -> Save {
    let mut s = Save {
        key: 0xC0FFEE,
        ..Save::default()
    };
    for t in tunes {
        s.learn_tune(t);
    }
    s
}

const THREE: [&str; 3] = ["welcome", "chroma", "trap-line"];

fn title(s: &Save, d: &Data) -> String {
    player::current_tune(s, d).map_or(String::new(), |t| t.id.clone())
}

#[test]
fn the_playlist_is_what_you_own_in_the_saves_order() {
    let d = data();
    let mut s = owning(&["chroma", "welcome"]);
    s.tunes.push("gone-from-the-data".into());
    let ids: Vec<&str> = player::playlist(&s, &d)
        .iter()
        .map(|t| t.id.as_str())
        .collect();
    assert_eq!(ids, ["chroma", "welcome"]);
}

#[test]
fn space_plays_and_pauses_and_nothing_plays_with_no_tunes() {
    let d = data();
    let mut t = Transport::default();
    let mut empty = Save::default();
    assert!(!player::apply(&mut empty, &d, &mut t, Act::Toggle));
    assert_eq!(t.play, Play::Stopped);

    let mut s = owning(&THREE);
    assert!(player::apply(&mut s, &d, &mut t, Act::Toggle));
    assert_eq!(t.play, Play::Playing);
    assert_eq!(title(&s, &d), "welcome", "starts at the top");
    player::tick(&mut s, &d, &mut t, 5.0, true);
    player::apply(&mut s, &d, &mut t, Act::Toggle);
    assert_eq!(t.play, Play::Paused);
    let at = t.at;
    player::tick(&mut s, &d, &mut t, 5.0, true);
    assert_eq!(t.at, at, "paused holds its place");
    player::apply(&mut s, &d, &mut t, Act::Toggle);
    assert_eq!((t.play, t.at), (Play::Playing, at), "and plays on from it");
    player::apply(&mut s, &d, &mut t, Act::Stop);
    assert_eq!((t.play, t.at), (Play::Stopped, 0.0));
}

#[test]
fn next_and_prev_move_through_the_list_and_stop_at_its_ends_without_repeat() {
    let d = data();
    let mut s = owning(&THREE);
    let mut t = Transport::default();
    player::apply(&mut s, &d, &mut t, Act::Play);
    assert!(player::apply(&mut s, &d, &mut t, Act::Next));
    assert_eq!(title(&s, &d), "chroma");
    player::apply(&mut s, &d, &mut t, Act::Next);
    assert!(!player::apply(&mut s, &d, &mut t, Act::Next), "the end");
    assert_eq!(title(&s, &d), "trap-line");
    assert!(player::apply(&mut s, &d, &mut t, Act::Prev));
    assert_eq!(title(&s, &d), "chroma");
    // Well into a tune, prev starts it again.
    player::tick(&mut s, &d, &mut t, 10.0, true);
    let cue = t.cue;
    player::apply(&mut s, &d, &mut t, Act::Prev);
    assert_eq!((title(&s, &d).as_str(), t.at), ("chroma", 0.0));
    assert_ne!(t.cue, cue, "a jump the speaker hears");
    player::apply(&mut s, &d, &mut t, Act::Prev);
    assert!(!player::apply(&mut s, &d, &mut t, Act::Prev), "the top");
    // Repeat wraps both ways.
    player::apply(&mut s, &d, &mut t, Act::Repeat);
    assert!(s.player.repeat);
    player::apply(&mut s, &d, &mut t, Act::Prev);
    assert_eq!(title(&s, &d), "trap-line");
    player::apply(&mut s, &d, &mut t, Act::Next);
    assert_eq!(title(&s, &d), "welcome");
}

#[test]
fn a_tune_plays_its_length_then_the_next_and_the_list_ends_stopped() {
    let d = data();
    let mut s = owning(&["welcome", "chroma"]);
    let mut t = Transport::default();
    player::apply(&mut s, &d, &mut t, Act::Play);
    let len = d.tune("welcome").unwrap().secs as f32;
    player::tick(&mut s, &d, &mut t, len + 0.5, true);
    assert_eq!(title(&s, &d), "chroma");
    assert!(t.at < 1.0 && t.play == Play::Playing);
    let len = d.tune("chroma").unwrap().secs as f32;
    player::tick(&mut s, &d, &mut t, len, true);
    assert_eq!(t.play, Play::Stopped, "no repeat: it stops");
    assert_eq!(title(&s, &d), "welcome", "back at the top");
    // With repeat, it goes round.
    player::apply(&mut s, &d, &mut t, Act::Repeat);
    player::apply(&mut s, &d, &mut t, Act::Play);
    player::tick(&mut s, &d, &mut t, 60.0, true);
    player::tick(&mut s, &d, &mut t, 80.0, true);
    assert_eq!((title(&s, &d).as_str(), t.play), ("welcome", Play::Playing));
}

#[test]
fn pick_plays_a_row_and_seek_stays_inside_the_tune() {
    let d = data();
    let mut s = owning(&THREE);
    let mut t = Transport::default();
    assert!(!player::apply(&mut s, &d, &mut t, Act::Pick(3)));
    assert!(player::apply(&mut s, &d, &mut t, Act::Pick(2)));
    assert_eq!(
        (title(&s, &d).as_str(), t.play),
        ("trap-line", Play::Playing)
    );
    player::apply(&mut s, &d, &mut t, Act::Seek(30_000));
    assert_eq!(t.at, 30.0);
    player::apply(&mut s, &d, &mut t, Act::Seek(9_999_000));
    assert!(t.at < d.tune("trap-line").unwrap().secs as f32);
    player::apply(&mut s, &d, &mut t, Act::Seek(0));
    assert_eq!(t.at, 0.0);
}

#[test]
fn volume_steps_the_saves_master_level() {
    let d = data();
    let mut s = owning(&THREE);
    let mut t = Transport::default();
    assert!(
        !player::apply(&mut s, &d, &mut t, Act::Louder),
        "already full"
    );
    player::apply(&mut s, &d, &mut t, Act::Softer);
    assert_eq!(s.volume, 100 - player::VOLUME_STEP);
    for _ in 0..20 {
        player::apply(&mut s, &d, &mut t, Act::Softer);
    }
    assert_eq!(s.volume, 0);
    // A control that shows the level sets it outright.
    assert!(player::apply(&mut s, &d, &mut t, Act::Volume(40)));
    assert_eq!(s.volume, 40);
    assert!(
        !player::apply(&mut s, &d, &mut t, Act::Volume(40)),
        "no change"
    );
    player::apply(&mut s, &d, &mut t, Act::Volume(250));
    assert_eq!(s.volume, 100);
}

#[test]
fn shuffle_is_the_same_for_the_same_save_and_plays_every_tune() {
    let d = data();
    let all: Vec<&str> = d.tunes.iter().map(|t| t.id.as_str()).collect();
    let mut s = owning(&all);
    s.player.shuffle = true;
    let a = player::order(&s, &d);
    assert_eq!(a, player::order(&s.clone(), &d));
    let mut sorted = a.clone();
    sorted.sort();
    assert_eq!(sorted, (0..all.len()).collect::<Vec<_>>());
    assert_ne!(a, sorted, "it shuffles");
    // Next walks the shuffled order.
    let mut t = Transport::default();
    player::apply(&mut s, &d, &mut t, Act::Play);
    assert_eq!(player::current(&s, &d), Some(a[0]));
    player::apply(&mut s, &d, &mut t, Act::Next);
    assert_eq!(player::current(&s, &d), Some(a[1]));
    let mut other = s.clone();
    other.key ^= 1;
    assert_ne!(player::order(&other, &d), a, "another save, another order");
}

#[test]
fn wanted_is_nothing_in_a_run_and_resumes_the_same_place_after() {
    let d = data();
    let mut s = owning(&THREE);
    let mut t = Transport::default();
    assert!(player::wanted(&s, &d, Screen::Hub, &t).is_none(), "stopped");
    player::apply(&mut s, &d, &mut t, Act::Play);
    player::tick(&mut s, &d, &mut t, 12.0, player::at_home(Screen::Hub));
    let w = player::wanted(&s, &d, Screen::Hub, &t).unwrap();
    assert_eq!((w.id, w.at), ("welcome", 12.0));
    let cue = w.cue;
    // Jacking in: the run's tune has the speaker, and the player waits.
    for screen in [Screen::JackIn { t: 0.5 }, Screen::Run, Screen::Crash] {
        assert!(player::wanted(&s, &d, screen, &t).is_none(), "{screen:?}");
        player::tick(&mut s, &d, &mut t, 30.0, player::at_home(screen));
    }
    // Jacking out: the same tune from the same place, no jump.
    let w = player::wanted(&s, &d, Screen::JackOut { t: 0.1 }, &t).unwrap();
    assert_eq!((w.id, w.at, w.cue), ("welcome", 12.0, cue));
    assert_eq!(t.play, Play::Playing);
}

#[test]
fn the_players_settings_are_saved_and_its_place_is_not() {
    let d = data();
    let mut s = owning(&THREE);
    let mut t = Transport::default();
    player::apply(&mut s, &d, &mut t, Act::Pick(1));
    player::apply(&mut s, &d, &mut t, Act::Repeat);
    player::apply(&mut s, &d, &mut t, Act::Shuffle);
    let back = Save::load(&s.to_text()).unwrap();
    assert_eq!(back.player.current.as_deref(), Some("chroma"));
    assert!(back.player.repeat && back.player.shuffle);
    // An old save without them plays from the top, in order.
    let old = Save::load(&s.to_text().replace("player:", "_gone:")).unwrap();
    assert_eq!(old.player, player::PlayerState::default());
}

#[test]
fn a_clock_reads_like_a_players() {
    assert_eq!(player::clock(0.0), "0:00");
    assert_eq!(player::clock(65.9), "1:05");
    assert_eq!(player::clock(600.0), "10:00");
}

#[test]
fn the_rows_status_and_now_line_say_what_a_player_says() {
    let d = data();
    let mut s = owning(&["chroma", "welcome"]);
    let mut t = Transport::default();
    let rows = player::rows(&s, &d);
    assert_eq!(
        rows.iter()
            .map(|r| (r.title, r.by, r.length.as_str()))
            .collect::<Vec<_>>(),
        [
            ("CHROMA", "PHOSPHOR", "1:20"),
            ("Welcome to the Net", "Sense/Net", "1:00")
        ]
    );
    assert!(rows.iter().all(|r| !r.current));
    assert_eq!(player::status(&s, &t), "■ stopped");
    assert!(player::now(&s, &d, &t).is_none());
    player::apply(&mut s, &d, &mut t, Act::Pick(1));
    player::tick(&mut s, &d, &mut t, 23.4, true);
    assert!(player::rows(&s, &d)[1].current);
    assert_eq!(player::status(&s, &t), "▶ playing");
    let (head, at, total) = player::now(&s, &d, &t).unwrap();
    assert_eq!(head, "Sense/Net - Welcome to the Net");
    assert_eq!((player::clock(at), player::clock(total)).0, "0:23");
    assert_eq!(total, 60.0);
    player::apply(&mut s, &d, &mut t, Act::Repeat);
    player::apply(&mut s, &d, &mut t, Act::Shuffle);
    player::apply(&mut s, &d, &mut t, Act::Toggle);
    assert_eq!(player::status(&s, &t), "‖ paused · repeat · shuffle");
    s.volume = 70;
    assert_eq!(player::volume(&s), "vol 70%");
    assert_eq!(
        (player::level(&s), player::level_label(&s)),
        (0.7, "Volume")
    );
    s.sound = false;
    assert_eq!(player::volume(&s), "vol off");
    // Off, a level control still shows the level, and says it's muted.
    assert_eq!((player::level(&s), player::level_label(&s)), (0.7, "Muted"));
}

#[test]
fn a_long_gap_catches_up_whole_tunes() {
    // No frames came for a while (a hidden window): the playlist went on without them.
    let d = data();
    let mut s = owning(&THREE);
    let mut t = Transport::default();
    player::apply(&mut s, &d, &mut t, Act::Play);
    let welcome = d.tune("welcome").unwrap().secs as f32;
    let chroma = d.tune("chroma").unwrap().secs as f32;
    assert!(player::tick(
        &mut s,
        &d,
        &mut t,
        welcome + chroma + 7.0,
        true
    ));
    assert_eq!(
        (title(&s, &d).as_str(), t.play),
        ("trap-line", Play::Playing)
    );
    assert!((t.at - 7.0).abs() < 1e-3, "{}", t.at);
    // Past the end without repeat: stopped at the top.
    player::tick(&mut s, &d, &mut t, 10_000.0, true);
    assert_eq!((title(&s, &d).as_str(), t.play), ("welcome", Play::Stopped));
    assert!(
        !player::tick(&mut s, &d, &mut t, 5.0, true),
        "nothing moved"
    );
}

#[test]
fn held_by_a_cracktro_the_status_says_it_waits() {
    let d = data();
    let mut s = owning(&THREE);
    let mut t = Transport::default();
    player::apply(&mut s, &d, &mut t, Act::Play);
    player::tick(&mut s, &d, &mut t, 1.0, false);
    assert!(t.held);
    assert_eq!(player::status(&s, &t), "‖ waiting");
    assert_eq!(t.at, 0.0, "the clock waits too");
    player::tick(&mut s, &d, &mut t, 1.0, true);
    assert_eq!(player::status(&s, &t), "▶ playing");
}

#[test]
fn the_readout_counts_a_tune_s_passes_and_rows() {
    let d = data();
    let mut s = owning(&THREE);
    let mut t = Transport::default();
    let r = player::readout(&s, &d, &t, 0);
    assert_eq!((r.pos, r.length, r.pattern, r.row, r.time), (0, 3, 0, 0, 0));
    player::apply(&mut s, &d, &mut t, Act::Pick(1));
    let chroma = &d.tune("chroma").unwrap().tune;
    let (rows, step) = (chroma.loop_steps(), chroma.step_secs());
    player::tick(&mut s, &d, &mut t, 10.0, true);
    let r = player::readout(&s, &d, &t, 0);
    let steps = (10.0 / step) as usize;
    assert_eq!((r.pos, r.length, r.rows), (2, 3, rows));
    assert_eq!((r.pattern, r.row, r.time), (steps / rows, steps % rows, 10));
    // Past one loop the pattern counts up and the row starts again.
    let lap = chroma.loop_secs(0);
    player::apply(
        &mut s,
        &d,
        &mut t,
        Act::Seek(((lap + step * 0.5) * 1000.0) as u32),
    );
    let r = player::readout(&s, &d, &t, 0);
    assert_eq!((r.pattern, r.row), (1, 0));
}

#[test]
fn the_readout_keeps_time_with_the_samples_the_speaker_renders() {
    // A step is a whole number of samples when it's rendered: late in a tune the rows follow
    // those, not the exact step.
    let d = data();
    let mut s = owning(&THREE);
    let mut t = Transport::default();
    player::apply(&mut s, &d, &mut t, Act::Pick(1));
    let chroma = &d.tune("chroma").unwrap().tune;
    let rate = 48_000;
    let samples = (chroma.step_secs() * rate as f32).floor();
    let rows = chroma.loop_steps();
    // Just past the 500th rendered step.
    t.at = (500.0 * samples + 1.0) / rate as f32;
    let r = player::readout(&s, &d, &t, rate);
    assert_eq!((r.pattern, r.row), (500 / rows, 500 % rows));
}
