//! Damage glitches: the screen breaks down as integrity drops, and every hit lands with a
//! burst. With the CRT shader this scales its own effects (fringing, static, shake, wobble,
//! flicker) and adds tearing; without it, a lighter overlay. Text corruption happens in the
//! panes, through `ninsei_core::glitch::corrupt`. All visual: clicks never follow the glitch.

use egui::{Context, Id, LayerId, Order, Rect, pos2, vec2};
use ninsei_core::Game;
use ninsei_core::display::Effects;
use ninsei_core::glitch::Glitches;

use crate::theme::Palette;

/// Strengths and timings, in one place. Display numbers, not balance.
pub mod tune {
    /// How long a hit's burst takes to fade.
    pub const HIT_SECS: f64 = 0.5;
    /// A hit this big (integrity lost) is a full-strength burst.
    pub const HIT_FULL: f32 = 6.0;
    /// A shield soaking the hit gives this much of a burst.
    pub const SOAK_SCALE: f32 = 0.35;
    /// How much each shader effect grows at full glitch.
    pub const RGB: f32 = 0.9;
    pub const NOISE: f32 = 0.6;
    pub const JITTER: f32 = 0.9;
    pub const HSYNC: f32 = 0.9;
    pub const FLICKER: f32 = 0.3;
    /// Flicker never goes past this: the shader's flicker is 24 times a second, so it stays a
    /// small brightness change (under 10%), never a flash.
    pub const FLICKER_MAX: f32 = 0.6;
    /// Glitch amount where tearing starts.
    pub const TEAR_FROM: f32 = 0.35;
    /// Glitch amount where text starts to corrupt, and the most it corrupts.
    pub const CORRUPT_FROM: f32 = 0.6;
    pub const CORRUPT_MAX: f32 = 0.06;
    /// Corrupted characters change this often, so each one comes right again quickly.
    pub const CORRUPT_STEP_SECS: f64 = 0.12;
    /// Dropouts (a patch of screen blanking) from this amount: a chance per slot, briefly.
    pub const DROPOUT_FROM: f32 = 0.85;
    pub const DROPOUT_SLOT_SECS: f64 = 0.6;
    pub const DROPOUT_SECS: f64 = 0.08;
    pub const DROPOUT_CHANCE: u64 = 30;
    /// The hit flash at full strength: one per hit, so never more than a few a second.
    pub const FLASH_ALPHA: f32 = 0.12;
}

/// This frame's glitching.
#[derive(Clone, Copy, Debug, Default)]
pub struct Glitch {
    mode: Glitches,
    /// The steady level from integrity, 0..1.
    level: f32,
    /// The current hit burst, 0..1.
    hit: f32,
    /// The burst is a shield soaking a hit: smaller and cleaner.
    soft: bool,
    t: f64,
}

#[derive(Clone, Default)]
struct Memory {
    seed: u64,
    tick: Option<u32>,
    at: f64,
    size: f32,
    soft: bool,
}

/// Small, even-ish hash for picking bands and slots.
fn hash(a: u64, b: u64) -> u64 {
    let mut x = a.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.wrapping_mul(0xD1B5_4A32_D192_ED03);
    x ^= x >> 31;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^ (x >> 29)
}

impl Glitch {
    /// Reads the run for this frame. `live` is false at home and on the crash screen.
    pub fn frame(ctx: &Context, game: &Game, mode: Glitches, live: bool) -> Glitch {
        let t = ctx.input(|i| i.time);
        if mode == Glitches::Off || !live {
            return Glitch {
                mode,
                t,
                ..Glitch::default()
            };
        }
        let last = game.history.last_hit;
        let mem = ctx.data_mut(|d| {
            let m: &mut Memory = d.get_temp_mut_or_default(Id::new("glitch"));
            if m.seed != game.seed {
                // A new run: whatever hit last run is old news.
                *m = Memory {
                    seed: game.seed,
                    tick: last.map(|h| h.tick),
                    ..Memory::default()
                };
            }
            if let Some(h) = last
                && m.tick != Some(h.tick)
            {
                m.tick = Some(h.tick);
                m.at = t;
                let scale = if h.soaked { tune::SOAK_SCALE } else { 1.0 };
                m.size = (h.amount as f32 / tune::HIT_FULL).clamp(0.2, 1.0) * scale;
                m.soft = h.soaked;
            }
            m.clone()
        });
        let since = t - mem.at;
        let hit = if mem.tick.is_some() && since < tune::HIT_SECS {
            mem.size * (-(since / (tune::HIT_SECS / 3.0)) as f32).exp()
        } else {
            0.0
        };
        Glitch {
            mode,
            level: game.glitch(),
            // A big hit lands harder the worse off you are.
            hit: (hit * (0.6 + game.glitch())).min(1.0),
            soft: mem.soft,
            t,
        }
    }

    /// How glitched the screen is right now, 0..1.
    pub fn amount(&self) -> f32 {
        (self.level + self.hit).min(1.0)
    }

    /// Whether anything's moving that needs another frame.
    pub fn busy(&self) -> bool {
        self.amount() > 0.0
    }

    /// The CRT preset with the glitch on top, and how much tearing (0..1). Only ever
    /// passed to the shader: the mouse keeps using the preset's own geometry.
    pub fn effects(&self, mut e: Effects) -> (Effects, f32) {
        let a = self.amount();
        match self.mode {
            Glitches::Off => return (e, 0.0),
            Glitches::Reduced => {
                e.rgb_shift += a * tune::RGB;
                return (e, 0.0);
            }
            Glitches::On => {}
        }
        e.rgb_shift += a * tune::RGB;
        e.static_noise += a * tune::NOISE;
        e.jitter += a * tune::JITTER;
        e.horizontal_sync += a * tune::HSYNC;
        e.flickering = (e.flickering + a * tune::FLICKER).min(tune::FLICKER_MAX.max(e.flickering));
        let tear = if self.soft {
            0.0
        } else {
            ((a - tune::TEAR_FROM) / (1.0 - tune::TEAR_FROM)).clamp(0.0, 1.0)
        };
        (e, tear)
    }

    /// How much log text to corrupt, and the seed for this moment. Zero unless glitches are
    /// fully on and you're badly hurt.
    pub fn corruption(&self) -> (f32, u64) {
        let a = self.amount();
        if self.mode != Glitches::On || a <= tune::CORRUPT_FROM {
            return (0.0, 0);
        }
        let rate = (a - tune::CORRUPT_FROM) / (1.0 - tune::CORRUPT_FROM) * tune::CORRUPT_MAX;
        (rate, (self.t / tune::CORRUPT_STEP_SECS) as u64)
    }

    /// The overlay: the hit flash always; tearing bands and static when there's no shader to
    /// do them; dropouts at the very bottom. Drawn under the CRT pass so the glass covers it.
    pub fn overlay(&self, ctx: &Context, p: &Palette, shader: bool) {
        if !self.busy() {
            return;
        }
        let painter = ctx.layer_painter(LayerId::new(Order::Tooltip, Id::new("glitch")));
        let screen = ctx.content_rect();
        if self.hit > 0.0 {
            let c = if self.soft { p.fg } else { p.bright };
            painter.rect_filled(screen, 0.0, c.gamma_multiply(self.hit * tune::FLASH_ALPHA));
        }
        if self.mode != Glitches::On {
            return;
        }
        let a = self.amount();
        if !shader && a > tune::TEAR_FROM {
            // Torn bands: slices of screen that drop out and smear, re-picked a few times a second.
            let slot = (self.t * 9.0) as u64;
            for i in 0..3u64 {
                if (hash(slot, i) % 100) as f32 > (a - tune::TEAR_FROM) * 140.0 {
                    continue;
                }
                let y =
                    screen.top() + (hash(slot, i + 10) % 1000) as f32 / 1000.0 * screen.height();
                let h = 2.0 + (hash(slot, i + 20) % 14) as f32;
                let band = Rect::from_min_size(pos2(screen.left(), y), vec2(screen.width(), h));
                painter.rect_filled(band, 0.0, p.bg.gamma_multiply(0.75));
                painter.line_segment(
                    [band.left_top(), band.right_top()],
                    egui::Stroke::new(1.0, p.bright.gamma_multiply(0.5)),
                );
            }
            // Static: a scatter of lit dots.
            let n = (a * 160.0) as u64;
            let tick = (self.t * 24.0) as u64;
            for i in 0..n {
                let x = screen.left() + (hash(tick, i) % 10_000) as f32 / 10_000.0 * screen.width();
                let y = screen.top()
                    + (hash(tick, i + 7_777) % 10_000) as f32 / 10_000.0 * screen.height();
                painter.rect_filled(
                    Rect::from_min_size(pos2(x, y), vec2(1.5, 1.5)),
                    0.0,
                    p.fg.gamma_multiply(0.5),
                );
            }
        }
        if a > tune::DROPOUT_FROM {
            let slot = (self.t / tune::DROPOUT_SLOT_SECS) as u64;
            let into = self.t - slot as f64 * tune::DROPOUT_SLOT_SECS;
            if hash(slot, 99) % 100 < tune::DROPOUT_CHANCE && into < tune::DROPOUT_SECS {
                let w = screen.width() * (0.2 + (hash(slot, 1) % 20) as f32 / 100.0);
                let h = screen.height() * (0.2 + (hash(slot, 2) % 30) as f32 / 100.0);
                let x =
                    screen.left() + (hash(slot, 3) % 1000) as f32 / 1000.0 * (screen.width() - w);
                let y =
                    screen.top() + (hash(slot, 4) % 1000) as f32 / 1000.0 * (screen.height() - h);
                painter.rect_filled(Rect::from_min_size(pos2(x, y), vec2(w, h)), 0.0, p.bg);
            }
        }
    }
}
