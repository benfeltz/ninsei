//! Cracktros: the ANSI intro a cracking crew put on the front of a release. A tool (or, later,
//! a door game) names one in `data/intros/<id>.ron`, and it plays every time the program opens:
//! a few frames of pipe-coded art with text effects, the board's ad, and greetz scrolling along
//! the bottom row. Everything here is a pure function of time; the app only paints it.

use serde::Deserialize;

use crate::ansi;
use crate::bbs::{COLS, ROWS};
use crate::data::Data;

/// Rows the art may use: the last two rows are the board's ad and the scroller.
pub const ART_ROWS: usize = ROWS - 2;

/// Tunables for every intro.
pub mod anim {
    /// Scroller speed in characters a second, and in calm mode (glitches reduced or off).
    pub const SCROLL_CPS: f32 = 16.0;
    pub const SCROLL_CALM_CPS: f32 = 8.0;
    /// Colour cycling: steps a second through [`super::RAMP`].
    pub const CYCLE_HZ: f32 = 10.0;
    /// Sine wobble: how far a line swings (columns) and how fast (radians a second).
    pub const SINE_COLS: f32 = 3.0;
    pub const SINE_RATE: f32 = 4.0;
    /// Phase between neighbouring lines (radians).
    pub const SINE_ROW: f32 = 0.6;
    /// Bounce: how far the block travels (columns) and its period (seconds).
    pub const BOUNCE_COLS: f32 = 8.0;
    pub const BOUNCE_SECS: f32 = 1.6;
}

/// The colours a cycling line steps through (CGA indices; the app folds them to the theme).
pub const RAMP: [u8; 6] = [1, 9, 11, 15, 11, 9];

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fx {
    /// Each line swings left and right on a sine, out of step with its neighbours.
    Sine,
    /// The lines' colours cycle through [`RAMP`].
    Cycle,
    /// The whole range slides left and right together.
    Bounce,
}

/// An effect on art rows `from..to` (counted from the top of the frame).
#[derive(Deserialize, Clone, Copy, Debug)]
pub struct FxDef {
    pub fx: Fx,
    pub from: usize,
    pub to: usize,
}

/// One cracktro (`data/intros/<id>.ron`).
#[derive(Deserialize, Clone, Debug)]
pub struct IntroDef {
    /// From the file name.
    #[serde(skip)]
    pub id: String,
    /// The board it advertises (an id in `data/bbs/`).
    pub board: String,
    /// The crew that cracked it.
    pub group: String,
    /// How long it plays before the program opens, in seconds.
    pub secs: f32,
    /// How long each frame shows.
    pub frame_secs: f32,
    /// Pipe-coded art, at most [`ART_ROWS`] lines of [`COLS`] each. Frames loop.
    pub frames: Vec<Vec<String>>,
    #[serde(default)]
    pub effects: Vec<FxDef>,
    /// Handles, crews and boards to greet, in order.
    pub greetz: Vec<String>,
}

impl IntroDef {
    pub fn validate(&self, data: &Data) -> Result<(), String> {
        let err = |m: String| Err(format!("intro `{}`: {m}", self.id));
        if data.board(&self.board).is_none() {
            return err(format!("unknown board `{}`", self.board));
        }
        if self.secs <= 0.0 || self.frame_secs <= 0.0 {
            return err("secs and frame_secs must be above 0".into());
        }
        if self.frames.is_empty() {
            return err("no frames".into());
        }
        for (n, f) in self.frames.iter().enumerate() {
            if f.len() > ART_ROWS {
                return err(format!("frame {n} has {} rows (max {ART_ROWS})", f.len()));
            }
            for l in f {
                let w = ansi::visible_len(l);
                if w > COLS {
                    return err(format!("frame {n} line is {w} wide: {}", ansi::plain(l)));
                }
            }
        }
        for e in &self.effects {
            if e.from >= e.to || e.to > ART_ROWS {
                return err(format!("effect rows {}..{} out of range", e.from, e.to));
            }
        }
        if self.greetz.is_empty() {
            return err("no greetz".into());
        }
        Ok(())
    }

    /// Whether it has finished playing at time `t` (seconds since it started).
    pub fn done(&self, t: f32) -> bool {
        t >= self.secs
    }

    /// The scroller's whole loop: who cracked it, the board, then the greetz.
    pub fn scroller(&self, data: &Data) -> String {
        let board = data
            .board(&self.board)
            .map(|b| &data.boards[b])
            .map_or(String::new(), |b| {
                format!("CALL {} {} ▪ {} BAUD ▪ ", b.name, b.phone, b.baud)
            });
        format!(
            "CRACKED BY {} ▪ {board}GREETZ TO {} ▪ ",
            self.group,
            self.greetz.join(" ▪ ")
        )
    }

    /// The whole screen at time `t`, at most [`ROWS`] lines of [`COLS`]. `calm` (glitches reduced
    /// or off) holds the art still, drops the colour cycling and slows the scroller.
    pub fn screen(&self, data: &Data, t: f32, calm: bool) -> Vec<String> {
        let frame = if calm {
            0
        } else {
            (t / self.frame_secs) as usize % self.frames.len()
        };
        let mut out: Vec<String> = self.frames[frame].clone();
        out.resize(ART_ROWS, String::new());
        if !calm {
            for e in &self.effects {
                for (row, line) in out.iter_mut().enumerate().take(e.to).skip(e.from) {
                    *line = apply(e.fx, line, row, t);
                }
            }
        }
        // The board's ad, centred above the scroller.
        let ad = data
            .board(&self.board)
            .map(|b| &data.boards[b])
            .map_or(String::new(), |b| {
                format!("{} ▪ {} ▪ SYSOP {}", b.name, b.phone, b.sysop)
            });
        let pad = COLS.saturating_sub(ad.chars().count()) / 2;
        out.push(format!("|14{}{ad}", " ".repeat(pad)));
        out.push(format!("|11{}", window(&self.scroller(data), t, calm)));
        out.into_iter().map(|l| ansi::clip(&l, COLS)).collect()
    }
}

/// The scroller's [`COLS`] visible characters at time `t`.
pub fn window(text: &str, t: f32, calm: bool) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return String::new();
    }
    let cps = if calm {
        anim::SCROLL_CALM_CPS
    } else {
        anim::SCROLL_CPS
    };
    let start = (t.max(0.0) * cps) as usize;
    (0..COLS)
        .map(|i| chars[(start + i) % chars.len()])
        .collect()
}

/// One effect on one line of art.
fn apply(fx: Fx, line: &str, row: usize, t: f32) -> String {
    match fx {
        Fx::Sine => {
            let s =
                ((t * anim::SINE_RATE + row as f32 * anim::SINE_ROW).sin() + 1.0) * anim::SINE_COLS;
            format!("{}{line}", " ".repeat(s.round() as usize))
        }
        Fx::Bounce => {
            let p = (t / anim::BOUNCE_SECS).fract();
            let tri = 1.0 - (2.0 * p - 1.0).abs();
            format!(
                "{}{line}",
                " ".repeat((tri * anim::BOUNCE_COLS).round() as usize)
            )
        }
        Fx::Cycle => {
            let i = (row + (t * anim::CYCLE_HZ) as usize) % RAMP.len();
            format!("|{:02}{}", RAMP[i], strip_fg(line))
        }
    }
}

/// Drops foreground codes (`|00`-`|15`), keeping backgrounds and the text.
fn strip_fg(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let code = (chars[i] == '|')
            .then(|| chars.get(i + 1..i + 3))
            .flatten()
            .filter(|d| d.iter().all(char::is_ascii_digit))
            .map(|d| (d[0] as u8 - b'0') * 10 + (d[1] as u8 - b'0'));
        match code {
            Some(c) if c < 16 => i += 3,
            _ => {
                out.push(chars[i]);
                i += 1;
            }
        }
    }
    out
}
