//! The desktop's music player: the rules every desktop's player dresses. The playlist is the
//! tunes you own ([`Save::tunes`], in their saved order); the save keeps which one is current,
//! repeat and shuffle ([`PlayerState`]); where you are in it and whether it's playing is the
//! [`Transport`], which lives as long as the session. A player only sends [`Act`]s; [`apply`]
//! carries them out, [`tick`] moves the music on, and [`wanted`] says what the speaker should
//! play at home. The run's tune wins: at home only, so jacking in pauses the desktop's music
//! where it was and jacking out picks it up there. Nothing here touches a run or its RNG.

use serde::{Deserialize, Serialize};

use crate::data::{Data, TuneFile};
use crate::hub::{Save, Screen};
use crate::intro::TuneDef;
use crate::rng::Rng;

/// The player's own settings, kept on the save.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct PlayerState {
    /// The tune the player is on (an id in `data/music/`), if any.
    #[serde(default)]
    pub current: Option<String>,
    /// At the end of the playlist, start it again.
    #[serde(default)]
    pub repeat: bool,
    /// Play the playlist in the save's own shuffled order.
    #[serde(default)]
    pub shuffle: bool,
}

/// Whether the music is going.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Play {
    #[default]
    Stopped,
    Playing,
    Paused,
}

/// Where the player is in the current tune, for this session only.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Transport {
    pub play: Play,
    /// Seconds into the current tune.
    pub at: f32,
    /// Bumped whenever the music jumps (a new tune, a seek, a stop), so the speaker knows to
    /// start again from [`Transport::at`] rather than carry on.
    pub cue: u32,
    /// Playing, but something else has the speaker (a cracktro): the clock stands still and
    /// the status says so. Set by [`tick`].
    pub held: bool,
}

/// What a player can ask for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    /// Play if stopped or paused, pause if playing.
    Toggle,
    Play,
    Pause,
    /// Stop and go back to the start of the tune.
    Stop,
    Next,
    /// The start of this tune, or the one before if it's only just begun.
    Prev,
    /// Play the playlist's `n`th tune from its start.
    Pick(usize),
    /// Jump to this many milliseconds into the tune.
    Seek(u32),
    /// Turn the master level up or down a step ([`VOLUME_STEP`]).
    Louder,
    Softer,
    Repeat,
    Shuffle,
}

/// How far `+` and `-` move the master level, in percent.
pub const VOLUME_STEP: u8 = 10;

/// Seconds into a tune after which Prev starts it again rather than going back one.
pub const PREV_RESTARTS_AFTER: f32 = 3.0;

/// The playlist: the tunes you own that the data has, in the save's order.
pub fn playlist<'a>(save: &Save, data: &'a Data) -> Vec<&'a TuneFile> {
    save.tunes.iter().filter_map(|t| data.tune(t)).collect()
}

/// The order the playlist plays in, as indices into [`playlist`]: as listed, or shuffled
/// from the save's key, so the same save always shuffles the same way.
pub fn order(save: &Save, data: &Data) -> Vec<usize> {
    order_of(save, playlist(save, data).len())
}

/// [`order`] for a playlist of `n` tunes.
fn order_of(save: &Save, n: usize) -> Vec<usize> {
    let mut out: Vec<usize> = (0..n).collect();
    if save.player.shuffle {
        let mut rng = Rng::new(save.key ^ 0x5EED_7A9E);
        for i in (1..n).rev() {
            out.swap(i, rng.below(i + 1));
        }
    }
    out
}

/// Which playlist row is current, if the current tune is still yours.
pub fn current(save: &Save, data: &Data) -> Option<usize> {
    current_in(save, &playlist(save, data))
}

/// [`current`] in a playlist already built.
fn current_in(save: &Save, list: &[&TuneFile]) -> Option<usize> {
    let id = save.player.current.as_deref()?;
    list.iter().position(|t| t.id == id)
}

/// The current tune, if there is one.
pub fn current_tune<'a>(save: &Save, data: &'a Data) -> Option<&'a TuneFile> {
    let id = save.player.current.as_deref()?;
    save.has_tune(id).then(|| data.tune(id)).flatten()
}

/// Moves to playlist row `row` from its start.
fn go(save: &mut Save, list: &[&TuneFile], t: &mut Transport, row: usize) {
    save.player.current = list.get(row).map(|t| t.id.clone());
    t.at = 0.0;
    t.cue = t.cue.wrapping_add(1);
}

/// The row after (or before) the current one in play order, wrapping only with repeat.
/// `None` at an end without repeat, or with nothing to play.
fn step(save: &Save, list: &[&TuneFile], forward: bool) -> Option<usize> {
    let order = order_of(save, list.len());
    if order.is_empty() {
        return None;
    }
    let Some(cur) = current_in(save, list).and_then(|c| order.iter().position(|&o| o == c)) else {
        return Some(order[0]);
    };
    let n = order.len();
    let next = if forward {
        cur + 1
    } else {
        cur.wrapping_sub(1)
    };
    if next < n {
        Some(order[next])
    } else if save.player.repeat {
        Some(order[if forward { 0 } else { n - 1 }])
    } else {
        None
    }
}

/// Carries out one act. Returns false if it did nothing (nothing to play, an end of the list).
pub fn apply(save: &mut Save, data: &Data, t: &mut Transport, act: Act) -> bool {
    // The level and the modes need no playlist.
    match act {
        Act::Louder | Act::Softer => {
            let v = if act == Act::Louder {
                save.volume.saturating_add(VOLUME_STEP).min(100)
            } else {
                save.volume.saturating_sub(VOLUME_STEP)
            };
            let changed = v != save.volume;
            save.volume = v;
            return changed;
        }
        Act::Repeat => {
            save.player.repeat = !save.player.repeat;
            return true;
        }
        Act::Shuffle => {
            save.player.shuffle = !save.player.shuffle;
            return true;
        }
        Act::Toggle if t.play == Play::Playing => return apply(save, data, t, Act::Pause),
        Act::Toggle => return apply(save, data, t, Act::Play),
        _ => {}
    }
    let list = playlist(save, data);
    match act {
        Act::Play => {
            if list.is_empty() {
                return false;
            }
            if current_in(save, &list).is_none() {
                let first = order_of(save, list.len())[0];
                go(save, &list, t, first);
            }
            t.play = Play::Playing;
            true
        }
        Act::Pause => {
            let was = t.play == Play::Playing;
            if was {
                t.play = Play::Paused;
            }
            was
        }
        Act::Stop => {
            let was = t.play != Play::Stopped || t.at > 0.0;
            t.play = Play::Stopped;
            t.at = 0.0;
            t.cue = t.cue.wrapping_add(1);
            was
        }
        Act::Next | Act::Prev => {
            if act == Act::Prev && t.at >= PREV_RESTARTS_AFTER && current_in(save, &list).is_some()
            {
                t.at = 0.0;
                t.cue = t.cue.wrapping_add(1);
                return true;
            }
            match step(save, &list, act == Act::Next) {
                Some(row) => {
                    go(save, &list, t, row);
                    true
                }
                None => false,
            }
        }
        Act::Pick(row) => {
            if row >= list.len() {
                return false;
            }
            go(save, &list, t, row);
            t.play = Play::Playing;
            true
        }
        Act::Seek(ms) => {
            let Some(cur) = current_in(save, &list) else {
                return false;
            };
            let secs = ms as f32 / 1000.0;
            t.at = secs.clamp(0.0, (list[cur].secs as f32 - 0.01).max(0.0));
            t.cue = t.cue.wrapping_add(1);
            true
        }
        Act::Louder | Act::Softer | Act::Repeat | Act::Shuffle | Act::Toggle => false,
    }
}

/// Whether desktop music may play on this screen: at home, and on the way back up from a
/// run. Jacking in hands the speaker to the run's tune; the crash screen is silent.
pub fn at_home(screen: Screen) -> bool {
    matches!(screen, Screen::Hub | Screen::JackOut { .. })
}

/// Moves the music on by `dt` seconds while it's playing and `live` (at home, and not
/// displaced by a cracktro: the app knows; [`Transport::held`] says when it isn't). At the
/// end of a tune it goes on to the next one in play order, as many as `dt` covers (no frames
/// come while the window is hidden, and the music has gone on); at the end of the list
/// without repeat, it stops on the first. True if it moved to another tune (the save
/// changed).
pub fn tick(save: &mut Save, data: &Data, t: &mut Transport, dt: f32, live: bool) -> bool {
    t.held = t.play == Play::Playing && !live;
    if !live || t.play != Play::Playing {
        return false;
    }
    let list = playlist(save, data);
    let before = save.player.current.clone();
    t.at += dt.max(0.0);
    loop {
        let Some(cur) = current_in(save, &list) else {
            t.play = Play::Stopped;
            break;
        };
        let len = (list[cur].secs as f32).max(1.0);
        if t.at < len {
            break;
        }
        let over = t.at - len;
        match step(save, &list, true) {
            Some(row) => {
                go(save, &list, t, row);
                t.at = over;
            }
            None => {
                let first = order_of(save, list.len()).first().copied();
                if let Some(row) = first {
                    go(save, &list, t, row);
                }
                t.play = Play::Stopped;
                break;
            }
        }
    }
    save.player.current != before
}

/// What the speaker should play at home: the current tune from where the player is.
#[derive(Clone, Copy, Debug)]
pub struct Want<'a> {
    /// The tune's id (in `data/music/`).
    pub id: &'a str,
    pub tune: &'a TuneDef,
    /// Where to start if it isn't already playing: seconds into the tune.
    pub at: f32,
    /// [`Transport::cue`]: when it changes, start again from `at`.
    pub cue: u32,
}

/// What the speaker should be playing for the desktop: nothing in a run, on the crash screen,
/// or while the player is stopped or paused; otherwise the current tune.
pub fn wanted<'a>(save: &Save, data: &'a Data, screen: Screen, t: &Transport) -> Option<Want<'a>> {
    if !at_home(screen) || t.play != Play::Playing {
        return None;
    }
    let tune = current_tune(save, data)?;
    Some(Want {
        id: &tune.id,
        tune: &tune.tune,
        at: t.at,
        cue: t.cue,
    })
}

/// `m:ss`, as a player prints a time.
pub fn clock(secs: f32) -> String {
    let s = secs.max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

/// A playlist row as a player lists it: the tune, its author, its length, and whether it's
/// the one the player is on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row<'a> {
    pub title: &'a str,
    pub by: &'a str,
    /// `m:ss`.
    pub length: String,
    pub current: bool,
}

/// The playlist's rows, in the save's order.
pub fn rows<'a>(save: &Save, data: &'a Data) -> Vec<Row<'a>> {
    let list = playlist(save, data);
    let cur = current_in(save, &list);
    list.into_iter()
        .enumerate()
        .map(|(i, t)| Row {
            title: &t.title,
            by: &t.by,
            length: clock(t.secs as f32),
            current: cur == Some(i),
        })
        .collect()
}

/// The status line's words, as a TUI player puts them: what the transport is doing, then
/// the modes that are on (`▶ playing · repeat · shuffle`).
pub fn status(save: &Save, t: &Transport) -> String {
    let mut s = match t.play {
        Play::Playing if t.held => "‖ waiting",
        Play::Playing => "▶ playing",
        Play::Paused => "‖ paused",
        Play::Stopped => "■ stopped",
    }
    .to_string();
    if save.player.repeat {
        s.push_str(" · repeat");
    }
    if save.player.shuffle {
        s.push_str(" · shuffle");
    }
    s
}

/// The master level as the status line shows it (`vol 70%`, or `vol off` with sound off).
pub fn volume(save: &Save) -> String {
    if save.sound {
        format!("vol {}%", save.volume.min(100))
    } else {
        "vol off".into()
    }
}

/// The current tune as the player heads it (`the NiGHTMOTHS - NiNSEi NiGHTS`), and where it
/// is: elapsed and total seconds. `None` with nothing current.
pub fn now(save: &Save, data: &Data, t: &Transport) -> Option<(String, f32, f32)> {
    let tune = current_tune(save, data)?;
    let total = tune.secs as f32;
    Some((
        format!("{} - {}", tune.by, tune.title),
        t.at.min(total),
        total,
    ))
}

/// The keys a player answers, as its legend lists them: key, then what it does.
pub const KEYS: [(&str, &str); 6] = [
    ("spc", "play"),
    ("n p", "next prev"),
    ("+ -", "vol"),
    ("r", "repeat"),
    ("s", "shuffle"),
    ("⏎", "pick"),
];
