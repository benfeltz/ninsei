//! The modem handshake you hear when you dial: an old 1200/2400-baud call, squeezed down.
//! Dial tone, touch tones for the real number, ringback, the answer tone, then the
//! carrier hiss until CONNECT. No later-era "boing" probing.
//!
//! [`Handshake::new`] lays out the timeline (the dial screens print their lines from it, so text
//! and sound stay in step); [`Handshake::render`] synthesizes it as mono samples in -1..=1.
//! [`Handshake::x25`] is the other call on the same timeline: a board reached in a run, through
//! the net's PAD, one short burst of packet data and no telephone at all.
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
    /// Packets on a data line: scrambled bits in short bursts. No tone, no carrier.
    Data,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Part {
    pub at: f32,
    pub secs: f32,
    pub tone: Tone,
}

/// One call, start to CONNECT: a dial-up handshake ([`Handshake::new`]) or an X.25 call
/// ([`Handshake::x25`], which has no off-hook, rings or answer: they're all at 0).
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

    /// An X.25 call through the net's PAD: one burst of packet data, then the far end accepts
    /// (`connect`, the PAD's `COM`) and a short settle. Under a second; the same every time.
    pub fn x25() -> Self {
        let connect = X25_BURST_SECS;
        Self {
            offhook: 0.0,
            rings: Vec::new(),
            answer: 0.0,
            connect,
            end: connect + X25_SETTLE_SECS,
            parts: vec![Part {
                at: 0.0,
                secs: X25_BURST_SECS,
                tone: Tone::Data,
            }],
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
            // Packet data: a new bit every `bit` samples, in packets with gaps between.
            let bit = ((rate_f / X25_BITS) as usize).max(1);
            let packet = X25_PACKET_SECS + X25_PACKET_GAP_SECS;
            let mut level = 1.0f32;
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
                    Tone::Data => {
                        if i % bit == 0 {
                            level = if rng.below(2) == 0 { -1.0 } else { 1.0 };
                        }
                        // Silent between packets, with the same short fades as a tone.
                        let into = s % packet;
                        let edge = into.min(X25_PACKET_SECS - into) * rate_f / ramp as f32;
                        if into < X25_PACKET_SECS {
                            level * edge.clamp(0.0, 1.0)
                        } else {
                            0.0
                        }
                    }
                };
                // Short fades so tones start and stop without clicks.
                let env = (i.min(len - 1 - i) as f32 / ramp as f32).min(1.0);
                let volume = if p.tone == Tone::Data {
                    X25_VOLUME
                } else {
                    MODEM_VOLUME
                };
                out[start + i] = v * env * volume;
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

    /// FNV-1a over the samples' bits: what the call sounds like, to the sample.
    fn sound_hash(samples: &[f32]) -> u64 {
        samples.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, s| {
            (h ^ u64::from(s.to_bits())).wrapping_mul(0x0100_0000_01b3)
        })
    }

    #[test]
    fn the_home_call_is_what_it_was() {
        // Pinned on main before the X.25 call came (140): the dial-up handshake, its timeline
        // and every sample of it, unchanged.
        for (n, hash, answer, connect, end) in [
            ("555-0199", 0xb044_9e53_2b3f_9216u64, 2.74, 4.34, 4.69),
            ("555-0142", 0x0518_3d7a_c6c5_4a54, 2.74, 4.34, 4.69),
            ("1-555-867-5309", 0x736a_0ba4_1b96_901c, 3.22, 4.82, 5.17),
        ] {
            let h = Handshake::new(n);
            assert_eq!(sound_hash(&h.render(8000)), hash, "{n} sounds as it did");
            assert!((h.offhook - 0.3).abs() < 1e-4, "{n}");
            assert!((h.answer - answer).abs() < 1e-4, "{n}");
            assert!((h.connect - connect).abs() < 1e-4, "{n}");
            assert!((h.end - end).abs() < 1e-4, "{n}");
            assert_eq!(h.rings.len(), 2, "{n}");
        }
    }

    #[test]
    fn the_x25_call_is_one_short_burst_and_no_telephone() {
        let h = Handshake::x25();
        assert!(h.end < 1.0, "well under a second: {}", h.end);
        assert!((h.end - (X25_BURST_SECS + X25_SETTLE_SECS)).abs() < 1e-6);
        assert!((h.connect - X25_BURST_SECS).abs() < 1e-6);
        assert!(h.rings.is_empty());
        assert_eq!(
            h.parts,
            vec![Part {
                at: 0.0,
                secs: X25_BURST_SECS,
                tone: Tone::Data
            }],
            "no dial tone, digits, ringback, answer tone or carrier"
        );
        assert_eq!(h, Handshake::x25(), "the same every time");
    }

    #[test]
    fn the_burst_hisses_in_packets_then_goes_quiet() {
        let rate = 8000.0;
        let h = Handshake::x25();
        let samples = h.render(rate as u32);
        assert_eq!(samples.len(), (h.end * rate) as usize);
        assert!(samples.iter().all(|s| s.abs() <= 1.0));
        let at = |t: f32| (t * rate) as usize;
        let first = &samples[at(0.01)..at(X25_PACKET_SECS - 0.01)];
        assert!(first.iter().any(|s| s.abs() > 0.05), "a packet is heard");
        // Noise, not a tone: no telephone frequency stands out of it.
        let band: Vec<f32> = (4..76)
            .map(|i| power(first, rate, i as f32 * 50.0))
            .collect();
        let flat = band.iter().sum::<f32>() / band.len() as f32;
        for f in [350.0, 440.0, 480.0, 1200.0, 2225.0, 2400.0] {
            assert!(power(first, rate, f) < 12.0 * flat, "{f} Hz");
        }
        let gap = X25_PACKET_SECS + X25_PACKET_GAP_SECS / 2.0;
        assert!(
            samples[at(gap) - 2..at(gap) + 2].iter().all(|&s| s == 0.0),
            "a gap between packets"
        );
        assert!(samples[at(h.connect) + 1..].iter().all(|&s| s == 0.0));
        assert_eq!(samples, h.render(rate as u32), "the same every time");
    }

    #[test]
    fn same_number_same_sound() {
        let h = Handshake::new("555-0199");
        assert_eq!(h.render(8000), h.render(8000));
    }
}
