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
}

/// What a player can ask for.
#[derive(Clone, Copy, Debug, PartialEq)]
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
    /// Jump to this many seconds into the tune.
    Seek(f32),
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
    let n = playlist(save, data).len();
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
    let id = save.player.current.as_deref()?;
    playlist(save, data).iter().position(|t| t.id == id)
}

/// The current tune, if there is one.
pub fn current_tune<'a>(save: &Save, data: &'a Data) -> Option<&'a TuneFile> {
    current(save, data).map(|i| playlist(save, data)[i])
}

/// Moves to playlist row `row` from its start.
fn go(save: &mut Save, data: &Data, t: &mut Transport, row: usize) {
    save.player.current = playlist(save, data).get(row).map(|t| t.id.clone());
    t.at = 0.0;
    t.cue = t.cue.wrapping_add(1);
}

/// The row after (or before) the current one in play order, wrapping only with repeat.
/// `None` at an end without repeat, or with nothing to play.
fn step(save: &Save, data: &Data, forward: bool) -> Option<usize> {
    let order = order(save, data);
    if order.is_empty() {
        return None;
    }
    let Some(cur) = current(save, data).and_then(|c| order.iter().position(|&o| o == c)) else {
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
    let has = !playlist(save, data).is_empty();
    match act {
        Act::Toggle if t.play == Play::Playing => apply(save, data, t, Act::Pause),
        Act::Toggle => apply(save, data, t, Act::Play),
        Act::Play => {
            if !has {
                return false;
            }
            if current(save, data).is_none() {
                let first = order(save, data)[0];
                go(save, data, t, first);
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
            if act == Act::Prev && t.at >= PREV_RESTARTS_AFTER && current(save, data).is_some() {
                t.at = 0.0;
                t.cue = t.cue.wrapping_add(1);
                return true;
            }
            match step(save, data, act == Act::Next) {
                Some(row) => {
                    go(save, data, t, row);
                    true
                }
                None => false,
            }
        }
        Act::Pick(row) => {
            if row >= playlist(save, data).len() {
                return false;
            }
            go(save, data, t, row);
            t.play = Play::Playing;
            true
        }
        Act::Seek(secs) => {
            let Some(tune) = current_tune(save, data) else {
                return false;
            };
            t.at = secs.clamp(0.0, (tune.secs as f32 - 0.01).max(0.0));
            t.cue = t.cue.wrapping_add(1);
            true
        }
        Act::Louder => {
            let v = save.volume.saturating_add(VOLUME_STEP).min(100);
            let changed = v != save.volume;
            save.volume = v;
            changed
        }
        Act::Softer => {
            let v = save.volume.saturating_sub(VOLUME_STEP);
            let changed = v != save.volume;
            save.volume = v;
            changed
        }
        Act::Repeat => {
            save.player.repeat = !save.player.repeat;
            true
        }
        Act::Shuffle => {
            save.player.shuffle = !save.player.shuffle;
            true
        }
    }
}

/// Whether desktop music may play on this screen: at home, and on the way back up from a
/// run. Jacking in hands the speaker to the run's tune; the crash screen is silent.
pub fn at_home(screen: Screen) -> bool {
    matches!(screen, Screen::Hub | Screen::JackOut { .. })
}

/// Moves the music on by `dt` seconds while it's playing and `live` (at home, and not
/// displaced by a cracktro: the app knows). At the end of a tune it goes on to the next one
/// in play order, or, at the end of the list without repeat, stops on the first.
pub fn tick(save: &mut Save, data: &Data, t: &mut Transport, dt: f32, live: bool) {
    if !live || t.play != Play::Playing {
        return;
    }
    let Some(tune) = current_tune(save, data) else {
        t.play = Play::Stopped;
        return;
    };
    t.at += dt.max(0.0);
    let len = tune.secs as f32;
    if t.at < len {
        return;
    }
    let over = t.at - len;
    match step(save, data, true) {
        Some(row) => {
            go(save, data, t, row);
            t.at = over.min(1.0);
        }
        None => {
            let first = order(save, data).first().copied();
            if let Some(row) = first {
                go(save, data, t, row);
            }
            t.play = Play::Stopped;
        }
    }
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
