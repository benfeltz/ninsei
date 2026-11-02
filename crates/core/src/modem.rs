//! The modem handshake you hear when you dial: an old 1200/2400-baud call, squeezed down.
//! Dial tone, touch tones for the real number, ringback, the answer tone, then the
//! carrier hiss until CONNECT. No later-era "boing" probing.
//!
//! [`Handshake::new`] lays out the timeline (the dial screens print their lines from it, so text
//! and sound stay in step); [`Handshake::render`] synthesizes it as mono samples in -1..=1.
//! Pure maths, no audio device: the app plays the samples.

use std::f32::consts::TAU;

use crate::rng::Rng;
use crate::tuning::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tone {
    /// 350 + 440 Hz: the line is open.
    DialTone,
    /// A DTMF key.
    Digit(char),
    /// 440 + 480 Hz: the far end is ringing.
    Ringback,
    /// 2225 Hz: a Bell 212A modem picked up.
    Answer,
    /// Both carriers at once (1200 Hz originate, 2400 Hz answer), phase-hopping at 600 baud.
    Carrier,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Part {
    pub at: f32,
    pub secs: f32,
    pub tone: Tone,
}

/// One dial-up call, start to CONNECT.
#[derive(Clone, Debug, PartialEq)]
pub struct Handshake {
    /// When the modem goes off hook and dials (`ATDT`).
    pub offhook: f32,
    /// When each ring starts.
    pub rings: Vec<f32>,
    /// When the far modem answers.
    pub answer: f32,
    /// When the carriers lock: print `CONNECT`.
    pub connect: f32,
    /// When the call is done and the screen moves on.
    pub end: f32,
    pub parts: Vec<Part>,
}

impl Handshake {
    /// The call to `number`. Only DTMF keys are dialed; dashes and spaces are skipped.
    pub fn new(number: &str) -> Self {
        let mut parts = Vec::new();
        let mut t = MODEM_OFFHOOK_SECS;
        let offhook = t;
        let mut push = |t: &mut f32, secs: f32, gap: f32, tone: Tone| {
            parts.push(Part { at: *t, secs, tone });
            *t += secs + gap;
        };
        push(&mut t, MODEM_DIALTONE_SECS, 0.0, Tone::DialTone);
        for c in number.chars().filter(|&c| dtmf(c).is_some()) {
            push(
                &mut t,
                MODEM_DIGIT_SECS,
                MODEM_DIGIT_GAP_SECS,
                Tone::Digit(c),
            );
        }
        let mut rings = Vec::new();
        for _ in 0..MODEM_RINGS {
            rings.push(t);
            push(&mut t, MODEM_RING_SECS, MODEM_RING_GAP_SECS, Tone::Ringback);
        }
        let answer = t;
        push(&mut t, MODEM_ANSWER_SECS, 0.0, Tone::Answer);
        push(&mut t, MODEM_CARRIER_SECS, 0.0, Tone::Carrier);
        let connect = t;
        Self {
            offhook,
            rings,
            answer,
            connect,
            end: connect + MODEM_SETTLE_SECS,
            parts,
        }
    }

    /// The whole call as mono samples at `rate` Hz, `end` seconds long.
    pub fn render(&self, rate: u32) -> Vec<f32> {
        let rate_f = rate as f32;
        let mut out = vec![0.0; (self.end * rate_f) as usize];
        // Fixed seed: the same number always sounds the same.
        let mut rng = Rng::new(0x2400);
        for p in &self.parts {
            let start = (p.at * rate_f) as usize;
            let len = ((p.secs * rate_f) as usize).min(out.len().saturating_sub(start));
            let ramp = ((MODEM_RAMP_SECS * rate_f) as usize).max(1);
            // Carrier phase: hops by a multiple of 90° every symbol (scrambled data).
            let symbol = (rate_f / MODEM_BAUD) as usize;
            let (mut ph_o, mut ph_a) = (0.0f32, 0.0f32);
            for i in 0..len {
                let s = i as f32 / rate_f;
                let v = match p.tone {
                    Tone::DialTone => mix(&[350.0, 440.0], s),
                    Tone::Ringback => mix(&[440.0, 480.0], s),
                    Tone::Digit(c) => {
                        let (lo, hi) = dtmf(c).unwrap_or((0.0, 0.0));
                        mix(&[lo, hi], s)
                    }
                    Tone::Answer => (TAU * 2225.0 * s).sin(),
                    Tone::Carrier => {
                        if symbol > 0 && i % symbol == 0 {
                            ph_o += TAU / 4.0 * rng.below(4) as f32;
                            ph_a += TAU / 4.0 * rng.below(4) as f32;
                        }
                        let hiss = (rng.next_u64() as f32 / u64::MAX as f32) * 2.0 - 1.0;
                        0.45 * (TAU * 1200.0 * s + ph_o).sin()
                            + 0.45 * (TAU * 2400.0 * s + ph_a).sin()
                            + 0.1 * hiss
                    }
                };
                // Short fades so tones start and stop without clicks.
                let env = (i.min(len - 1 - i) as f32 / ramp as f32).min(1.0);
                out[start + i] = v * env * MODEM_VOLUME;
            }
        }
        out
    }
}

/// Equal mix of sines, peak 1.
fn mix(freqs: &[f32], s: f32) -> f32 {
    freqs.iter().map(|f| (TAU * f * s).sin()).sum::<f32>() / freqs.len() as f32
}

/// The (low, high) DTMF pair for a key, or `None` if it isn't one.
pub fn dtmf(c: char) -> Option<(f32, f32)> {
    const ROWS: [f32; 4] = [697.0, 770.0, 852.0, 941.0];
    const COLS: [f32; 3] = [1209.0, 1336.0, 1477.0];
    let (r, col) = match c {
        '1'..='9' => {
            let d = c as usize - '1' as usize;
            (d / 3, d % 3)
        }
        '*' => (3, 0),
        '0' => (3, 1),
        '#' => (3, 2),
        _ => return None,
    };
    Some((ROWS[r], COLS[col]))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Strength of `freq` in `samples` (Goertzel).
    fn power(samples: &[f32], rate: f32, freq: f32) -> f32 {
        let w = TAU * freq / rate;
        let (mut s1, mut s2) = (0.0f32, 0.0f32);
        for &x in samples {
            let s0 = x + 2.0 * w.cos() * s1 - s2;
            s2 = s1;
            s1 = s0;
        }
        (s1 * s1 + s2 * s2 - 2.0 * w.cos() * s1 * s2) / samples.len() as f32
    }

    #[test]
    fn the_timeline_runs_in_order() {
        let h = Handshake::new("555-0142");
        let digits = h
            .parts
            .iter()
            .filter(|p| matches!(p.tone, Tone::Digit(_)))
            .count();
        assert_eq!(digits, 7, "the dash isn't dialed");
        assert_eq!(h.rings.len(), MODEM_RINGS);
        assert!(h.offhook < h.rings[0] && h.rings[0] < h.answer);
        assert!(h.answer < h.connect && h.connect < h.end);
        for w in h.parts.windows(2) {
            assert!(w[0].at + w[0].secs <= w[1].at + 1e-4, "parts overlap");
        }
        assert!(
            Handshake::new("555-01").end < h.end,
            "fewer digits, shorter call"
        );
    }

    #[test]
    fn it_dials_the_real_digits() {
        let rate = 8000.0;
        let h = Handshake::new("5");
        let samples = h.render(rate as u32);
        assert_eq!(samples.len(), (h.end * rate) as usize);
        assert!(samples.iter().all(|s| s.abs() <= 1.0));
        let p = h.parts.iter().find(|p| p.tone == Tone::Digit('5')).unwrap();
        let (a, b) = ((p.at * rate) as usize, ((p.at + p.secs) * rate) as usize);
        let key = &samples[a..b];
        // '5' is 770 + 1336 Hz; '1' would be 697 + 1209.
        assert!(power(key, rate, 770.0) > 20.0 * power(key, rate, 697.0));
        assert!(power(key, rate, 1336.0) > 20.0 * power(key, rate, 1209.0));
    }

    #[test]
    fn it_answers_then_hisses_then_goes_quiet() {
        let rate = 8000.0;
        let h = Handshake::new("1");
        let samples = h.render(rate as u32);
        let at = |t: f32| (t * rate) as usize;
        let answer = &samples[at(h.answer + 0.05)..at(h.answer + 0.3)];
        assert!(power(answer, rate, 2225.0) > 20.0 * power(answer, rate, 1200.0));
        let carrier = &samples[at(h.connect - 0.5)..at(h.connect - 0.05)];
        assert!(power(carrier, rate, 1200.0) > 4.0 * power(carrier, rate, 2225.0));
        assert!(samples[at(h.connect) + 1..].iter().all(|&s| s == 0.0));
    }

    #[test]
    fn same_number_same_sound() {
        let h = Handshake::new("555-0199");
        assert_eq!(h.render(8000), h.render(8000));
    }
}
