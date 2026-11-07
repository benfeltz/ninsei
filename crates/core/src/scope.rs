//! The maths behind the vis pane: what the speaker is playing, as a spectrum of bars, a level
//! and the motion a meter shows. Display only: it reads samples the app hands it and never the
//! game, so the probe can't see it.

/// The lowest and highest frequencies the bars cover, in Hz (the top is also kept under the
/// rate's Nyquist).
pub const LOW_HZ: f32 = 60.0;
pub const HIGH_HZ: f32 = 12_000.0;
/// The quietest level a bar or meter shows, in dB below full scale: anything under it is empty.
pub const FLOOR_DB: f32 = -60.0;
/// How fast a bar falls back, in full heights per second. It rises at once and falls slowly,
/// the way a VU meter's needle does, so a frame-to-frame jump never flickers.
pub const FALL_PER_SEC: f32 = 1.5;

/// A peak at or above this many dB is close to clipping: a report flags it.
pub const CLIP_DB: f32 = -3.0;
/// The rate band labels are worked out at when there's no output to ask (sound off, no
/// device): the bands are the same ones either way.
pub const NOMINAL_RATE: u32 = 48_000;
/// How much a scope trace is magnified: run music peaks around 0.3.
pub const TRACE_GAIN: f32 = 3.0;

/// `x` in dB below full scale, floored at [`FLOOR_DB`].
pub fn db(x: f32) -> f32 {
    if x <= 0.0 {
        FLOOR_DB
    } else {
        (20.0 * x.log10()).max(FLOOR_DB)
    }
}

/// A dB value as a share of a meter's height: 0 at the floor, 1 at full scale.
pub fn height(db: f32) -> f32 {
    ((db - FLOOR_DB) / -FLOOR_DB).clamp(0.0, 1.0)
}

/// The RMS of `samples` (0 for none).
pub fn level(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f64 = samples.iter().map(|&s| f64::from(s) * f64::from(s)).sum();
    (sum / samples.len() as f64).sqrt() as f32
}

/// The loudest sample's size.
pub fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0f32, |m, s| m.max(s.abs()))
}

/// The frequency range `[lo, hi)` of bar `bar` of `bars`, spaced evenly in pitch from
/// [`LOW_HZ`] to the top.
pub fn bar_range(bar: usize, bars: usize, rate: u32) -> (f32, f32) {
    let top = HIGH_HZ.min(rate as f32 / 2.0);
    let k = (top / LOW_HZ).max(1.0);
    let at = |i: usize| LOW_HZ * k.powf(i as f32 / bars.max(1) as f32);
    (at(bar), at(bar + 1))
}

/// The bar a frequency falls in, if any.
pub fn bar_of(freq: f32, bars: usize, rate: u32) -> Option<usize> {
    (0..bars).find(|&b| {
        let (lo, hi) = bar_range(b, bars, rate);
        freq >= lo && freq < hi
    })
}

/// `bars` bars of `samples` (a window at `rate`), each 0-1 ([`height`] of its loudest
/// partial's amplitude). The window is the largest power of two that fits, Hann-weighted.
pub fn spectrum(samples: &[f32], rate: u32, bars: usize) -> Vec<f32> {
    let n = if samples.len() < 2 {
        0
    } else {
        1usize << (usize::BITS - 1 - samples.len().leading_zeros())
    };
    if n < 2 || bars == 0 || rate == 0 {
        return vec![0.0; bars];
    }
    let tail = &samples[samples.len() - n..];
    let mut re: Vec<f32> = tail
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let w = 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / n as f32).cos();
            s * w
        })
        .collect();
    let mut im = vec![0.0f32; n];
    fft(&mut re, &mut im);
    // A full-scale sine's peak, Hann-weighted, is n / 4: scale back to its amplitude.
    let amp = |k: usize| (re[k] * re[k] + im[k] * im[k]).sqrt() * 4.0 / n as f32;
    let hz = rate as f32 / n as f32;
    (0..bars)
        .map(|b| {
            let (lo, hi) = bar_range(b, bars, rate);
            let (a, z) = ((lo / hz).ceil() as usize, (hi / hz).ceil() as usize);
            // A narrow low bar may hold no bin of its own: it reads the one nearest its middle.
            let loudest = if z > a {
                (a..z.min(n / 2)).map(amp).fold(0.0, f32::max)
            } else {
                amp((((lo + hi) / 2.0 / hz).round() as usize).min(n / 2))
            };
            height(db(loudest))
        })
        .collect()
}

/// Whether a peak is close to clipping.
pub fn clipping(peak: f32) -> bool {
    db(peak) >= CLIP_DB
}

/// A level for a readout: `-12.0 dB`, or dashes in silence. Always 8 characters wide.
pub fn db_text(x: f32) -> String {
    let d = db(x);
    if d <= FLOOR_DB {
        "  --- dB".into()
    } else {
        format!("{d:>5.1} dB")
    }
}

/// A frequency for a band's label: `250`, `1.2k`.
pub fn hz_text(hz: f32) -> String {
    if hz >= 1000.0 {
        format!("{:.1}k", hz / 1000.0)
    } else {
        format!("{hz:.0}")
    }
}

/// Bar `bar`'s label: where its band starts, at `rate` (or [`NOMINAL_RATE`] with none).
pub fn band_label(bar: usize, bars: usize, rate: u32) -> String {
    let rate = if rate == 0 { NOMINAL_RATE } else { rate };
    hz_text(bar_range(bar, bars, rate).0)
}

/// How many cells each bar lights in a column `rows` tall: whole cells only.
pub fn cells_lit(bars: &[f32], rows: usize) -> Vec<usize> {
    bars.iter()
        .map(|b| (b.clamp(0.0, 1.0) * rows as f32).round() as usize)
        .collect()
}

/// The dB a meter height stands for (the inverse of [`height`]).
pub fn db_at(height: f32) -> f32 {
    FLOOR_DB * (1.0 - height.clamp(0.0, 1.0))
}

/// `samples` cut to `width` points for a scope trace, one per column (each the column's
/// loudest swing, so a narrow trace never hides a peak), magnified by [`TRACE_GAIN`].
pub fn trace(samples: &[f32], width: usize) -> Vec<f32> {
    if width == 0 || samples.is_empty() {
        return vec![0.0; width];
    }
    (0..width)
        .map(|x| {
            let a = x * samples.len() / width;
            let b = ((x + 1) * samples.len() / width)
                .max(a + 1)
                .min(samples.len());
            let s = &samples[a..b];
            let hi = s.iter().copied().fold(0.0f32, f32::max);
            let lo = s.iter().copied().fold(0.0f32, f32::min);
            (if hi >= -lo { hi } else { lo }) * TRACE_GAIN
        })
        .collect()
}

/// The bars a meter shows now: each jumps up to `now` at once, but falls toward it no faster
/// than [`FALL_PER_SEC`] over `dt` seconds.
pub fn ballistics(shown: &[f32], now: &[f32], dt: f32) -> Vec<f32> {
    let fall = FALL_PER_SEC * dt.max(0.0);
    now.iter()
        .enumerate()
        .map(|(i, &v)| {
            let was = shown.get(i).copied().unwrap_or(0.0);
            if v >= was { v } else { (was - fall).max(v) }
        })
        .collect()
}

/// An in-place radix-2 FFT (`re.len()` a power of two).
fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -std::f32::consts::TAU / len as f32;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (s, c) = (ang * k as f32).sin_cos();
                let (a, b) = (start + k, start + k + len / 2);
                let tr = re[b] * c - im[b] * s;
                let ti = re[b] * s + im[b] * c;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
            }
        }
        len <<= 1;
    }
}
