//! Cracktros: the ANSI intro a cracking crew put on the front of a release. A tool (or, later,
//! a door game) names one in `data/intros/<id>.ron`, and it plays every time the program opens:
//! a few frames of pipe-coded art with text effects, the board's ad, and greetz scrolling along
//! the bottom row, over a chiptune synthesized from its tune (never sampled). Everything here
//! is a pure function of time; the app only paints and plays it.

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
    #[serde(default)]
    pub greetz: Vec<String>,
    /// A splash instead of a crack: the scroller shows this line, no crew and no greetz.
    #[serde(default)]
    pub tagline: Option<String>,
    /// The chiptune it plays, if any.
    #[serde(default)]
    pub tune: Option<TuneDef>,
}

/// A chiptune as a tiny tracker pattern: every track is a row of steps that loops for as long
/// as the intro plays. A step is a note (`C4`, `F#3`, `Bb2`), `x` (a hit, for noise drums),
/// `-` (hold the last note) or `.` (rest).
#[derive(Deserialize, Clone, Debug)]
pub struct TuneDef {
    pub bpm: u32,
    /// Steps per beat (4: sixteenth notes).
    #[serde(default = "four")]
    pub steps: u32,
    pub tracks: Vec<TrackDef>,
}

fn four() -> u32 {
    4
}

#[derive(Deserialize, Clone, Debug)]
pub struct TrackDef {
    pub wave: crate::cue::Wave,
    pub vol: f32,
    pub pattern: String,
}

/// One parsed step.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Step {
    Note(f32),
    Hit,
    Hold,
    Rest,
}

/// A note name's frequency: `A4` is 440 Hz.
pub fn pitch(name: &str) -> Option<f32> {
    let mut c = name.chars();
    let base = match c.next()? {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let rest: String = c.collect();
    let (shift, oct) = match rest.strip_prefix('#') {
        Some(o) => (1, o),
        None => match rest.strip_prefix('b') {
            Some(o) => (-1, o),
            None => (0, rest.as_str()),
        },
    };
    let oct: i32 = oct.parse().ok().filter(|o| (0..=8).contains(o))?;
    let midi = 12 * (oct + 1) + base + shift;
    Some(440.0 * 2f32.powf((midi - 69) as f32 / 12.0))
}

fn steps(pattern: &str) -> Result<Vec<Step>, String> {
    pattern
        .split_whitespace()
        .map(|t| match t {
            "." => Ok(Step::Rest),
            "-" => Ok(Step::Hold),
            "x" => Ok(Step::Hit),
            n => pitch(n).map(Step::Note).ok_or(format!("bad step `{n}`")),
        })
        .collect()
}

impl TuneDef {
    pub fn validate(&self) -> Result<(), String> {
        if !(40..=300).contains(&self.bpm) || !(1..=8).contains(&self.steps) {
            return Err(format!(
                "bpm {} / steps {} out of range",
                self.bpm, self.steps
            ));
        }
        if self.tracks.is_empty() || self.tracks.len() > 4 {
            return Err("a tune has 1 to 4 tracks".into());
        }
        for t in &self.tracks {
            if !(0.0..=1.0).contains(&t.vol) {
                return Err(format!("track volume {} (0-1)", t.vol));
            }
            if steps(&t.pattern)?.is_empty() {
                return Err("an empty track".into());
            }
        }
        Ok(())
    }

    /// Seconds per step.
    pub fn step_secs(&self) -> f32 {
        60.0 / (self.bpm.max(1) * self.steps.max(1)) as f32
    }

    /// `secs` of the tune at `rate` samples a second, every track looped and mixed, fading
    /// out at the end. The same tune always renders the same samples.
    pub fn render(&self, rate: u32, secs: f32) -> Vec<f32> {
        let rate = rate.max(1) as f32;
        let len = (secs.max(0.0) * rate) as usize;
        let step = (self.step_secs() * rate).max(1.0) as usize;
        let mut out = vec![0.0f32; len];
        for t in &self.tracks {
            if let Some(mut synth) = Synth::new(t, step) {
                synth.fill(&mut out, 0, rate, len);
            }
        }
        let fade = (0.25 * rate) as usize;
        for (i, o) in out.iter_mut().enumerate() {
            let tail = len - i;
            let g = if tail < fade {
                tail as f32 / fade as f32
            } else {
                1.0
            };
            *o = (*o * g).clamp(-1.0, 1.0);
        }
        out
    }

    /// Steps before every track lines up again: the least common multiple of their lengths
    /// (`usize::MAX` if that's past counting, which no loop limit lets through).
    pub fn loop_steps(&self) -> usize {
        fn gcd(a: usize, b: usize) -> usize {
            if b == 0 { a } else { gcd(b, a % b) }
        }
        self.tracks
            .iter()
            .filter_map(|t| steps(&t.pattern).ok())
            .map(|p| p.len().max(1))
            .try_fold(1usize, |l, n| (l / gcd(l, n)).checked_mul(n))
            .unwrap_or(usize::MAX)
    }

    /// Seconds one cycle lasts at `bpm + tempo`.
    pub fn loop_secs(&self, tempo: u32) -> f32 {
        let per_step = 60.0 / ((self.bpm + tempo).max(1) * self.steps.max(1)) as f32;
        self.loop_steps() as f32 * per_step
    }

    /// Exactly one cycle of the tune ([`Self::loop_steps`] steps), played with `arr`'s track
    /// left out and its extra bpm, with no fade: played on repeat, it never seams. A note still
    /// ringing at the end of the cycle carries on into the start, as it would on the next pass.
    pub fn render_loop(&self, rate: u32, arr: &crate::music::Arrangement) -> Vec<f32> {
        let mut job = self.loop_render(rate, arr.tempo, arr.drop);
        while !job.work(usize::MAX) {}
        job.finish().0
    }

    /// One cycle at `bpm + tempo`, split in two: the bed (every track but `stem`) and the stem
    /// (that track alone, empty if none), the same length, so a player can mute the stem
    /// without losing its place. Rendered a slice at a time ([`LoopRender::work`]), so a long
    /// loop never holds up a frame; the samples are the same however it's sliced.
    pub fn loop_render(&self, rate: u32, tempo: u32, stem: Option<usize>) -> LoopRender {
        let rate = rate.max(1) as f32;
        let bpm = (self.bpm + tempo).max(1) * self.steps.max(1);
        let step = ((60.0 / bpm as f32) * rate).max(1.0) as usize;
        let len = self.loop_steps().saturating_mul(step);
        let synths: Vec<(Synth, bool)> = self
            .tracks
            .iter()
            .enumerate()
            .filter_map(|(n, t)| Some((Synth::new(t, step)?, stem == Some(n))))
            .collect();
        // Room for the last notes to ring out past the end, folded back to the start after.
        let tail = synths.iter().map(|(s, _)| s.longest()).max().unwrap_or(0) * step;
        let has_stem = synths.iter().any(|(_, st)| *st);
        LoopRender {
            bed: vec![0.0; len + tail],
            stem: if has_stem {
                vec![0.0; len + tail]
            } else {
                Vec::new()
            },
            len,
            rate,
            synths,
            track: 0,
            at: 0,
        }
    }
}

/// A tune's loop being rendered a slice at a time (see [`TuneDef::loop_render`]).
pub struct LoopRender {
    bed: Vec<f32>,
    stem: Vec<f32>,
    /// One cycle, in samples; the buffers run past it by the ringing tail.
    len: usize,
    rate: f32,
    /// Each track's synth, and whether it plays on the stem.
    synths: Vec<(Synth, bool)>,
    /// The track being rendered, and how far into the buffer it has got.
    track: usize,
    at: usize,
}

impl LoopRender {
    /// Renders about `budget` more samples (all tracks counted). True once it's all done.
    pub fn work(&mut self, budget: usize) -> bool {
        let mut left = budget.max(1);
        while self.track < self.synths.len() && left > 0 {
            let (synth, on_stem) = &mut self.synths[self.track];
            let out = if *on_stem {
                &mut self.stem
            } else {
                &mut self.bed
            };
            let to = self.at.saturating_add(left).min(out.len());
            synth.fill(&mut out[self.at..to], self.at, self.rate, self.len);
            left -= to - self.at;
            self.at = to;
            if self.at >= out.len() {
                self.track += 1;
                self.at = 0;
            }
        }
        self.track >= self.synths.len()
    }

    /// The bed and the stem, one cycle each, with what rang past the end
    /// folded back into the start. Call once [`Self::work`] says it's done.
    pub fn finish(mut self) -> (Vec<f32>, Vec<f32>) {
        let len = self.len;
        let close = |buf: &mut Vec<f32>| {
            if buf.is_empty() {
                return;
            }
            let tail = buf.split_off(len);
            for (o, t) in buf.iter_mut().zip(tail) {
                *o += t;
            }
            for o in buf.iter_mut() {
                *o = o.clamp(-1.0, 1.0);
            }
        };
        close(&mut self.bed);
        close(&mut self.stem);
        (self.bed, self.stem)
    }

    /// Samples one cycle lasts.
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// One track's synthesizer: its pattern, looping from the first sample, `step` samples a
/// step, and where it got to, so it can render in slices.
struct Synth {
    pat: Vec<Step>,
    wave: crate::cue::Wave,
    vol: f32,
    step: usize,
    seed: u32,
    freq: f32,
    start: usize,
    sounding: bool,
    phase: f32,
    held: f32,
    length: usize,
}

impl Synth {
    fn new(t: &TrackDef, step: usize) -> Option<Synth> {
        let pat = steps(&t.pattern).ok().filter(|p| !p.is_empty())?;
        Some(Synth {
            pat,
            wave: t.wave,
            vol: t.vol,
            step,
            seed: 0x9E37_79B9_u32,
            freq: 0.0,
            start: 0,
            sounding: false,
            phase: 0.0,
            held: 0.0,
            length: step,
        })
    }

    /// How long a note starting at `at` runs, in steps, counting the holds after it.
    fn run(&self, at: usize) -> usize {
        let pat = &self.pat;
        1 + (1..pat.len())
            .take_while(|k| pat[(at + k) % pat.len()] == Step::Hold)
            .count()
    }

    /// The longest any note or hit rings, in steps.
    fn longest(&self) -> usize {
        (0..self.pat.len())
            .filter(|&at| matches!(self.pat[at], Step::Note(_) | Step::Hit))
            .map(|at| self.run(at))
            .max()
            .unwrap_or(0)
    }

    /// Mixes the track into `out`, whose first sample is sample `base` of the whole. No note
    /// starts from sample `until` on: what's ringing then plays out.
    fn fill(&mut self, out: &mut [f32], base: usize, rate: f32, until: usize) {
        let step = self.step;
        for (j, o) in out.iter_mut().enumerate() {
            let i = base + j;
            if i.is_multiple_of(step) && i < until {
                let at = (i / step) % self.pat.len();
                match self.pat[at] {
                    Step::Note(f) => {
                        let length = self.run(at) * step;
                        (self.freq, self.start, self.sounding, self.length) = (f, i, true, length);
                    }
                    Step::Hit => {
                        (self.freq, self.start, self.sounding, self.length) = (0.0, i, true, step);
                    }
                    Step::Rest => self.sounding = false,
                    Step::Hold => {}
                }
            }
            if !self.sounding {
                continue;
            }
            let k = (i - self.start) as f32 / self.length.max(1) as f32;
            if k >= 1.0 {
                self.sounding = false;
                continue;
            }
            let s = if self.freq == 0.0 {
                // A drum hit: noise, changing every few samples.
                if i.is_multiple_of(3) {
                    let seed = &mut self.seed;
                    *seed ^= *seed << 13;
                    *seed ^= *seed >> 17;
                    *seed ^= *seed << 5;
                    self.held = (*seed % 2001) as f32 / 1000.0 - 1.0;
                }
                self.held * (1.0 - k).powi(4)
            } else {
                self.phase = (self.phase + self.freq / rate).fract();
                let w = crate::cue::Wave::sample(self.wave, self.phase, &mut self.seed);
                // A pluck: a quick attack, then down to a sustain, released at the end.
                let attack = ((i - self.start) as f32 / (0.003 * rate)).min(1.0);
                let body = 0.55 + 0.45 * (1.0 - k).powi(3);
                let release = ((1.0 - k) / 0.15).min(1.0);
                w * attack * body * release
            };
            *o += s * self.vol;
        }
    }
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
        if self.greetz.is_empty() && self.tagline.is_none() {
            return err("no greetz (or a tagline)".into());
        }
        if let Some(t) = &self.tune
            && let Err(e) = t.validate()
        {
            return err(format!("tune: {e}"));
        }
        Ok(())
    }

    /// Whether it has finished playing at time `t` (seconds since it started).
    pub fn done(&self, t: f32) -> bool {
        t >= self.secs
    }

    /// The scroller's whole loop: who cracked it, the board, then the greetz.
    pub fn scroller(&self, data: &Data) -> String {
        if let Some(t) = &self.tagline {
            return format!("{t} ▪ ");
        }
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
