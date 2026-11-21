//! Demos: a drawn intro, the way an Amiga crew put one on the front of a release. Where a
//! cracktro (`intro.rs`) is pipe-coded text, a demo is pixels on a small virtual screen: a
//! first-person flight over wireframe grids with wireframe solids passing by, lines of text
//! in a bitmap font shaded row by row, and a tune. A demo is a file in `data/demos/<id>.ron`;
//! everything in it (the screen, the palette, the camera's path, the grids, the shapes and
//! where they sit, the font, the text and the tune) is data.
//!
//! A frame is a pure function of time: [`DemoDef::frame`] places everything at `t` (line
//! segments in screen pixels, glyphs at pixel positions) and [`DemoDef::pixels`] rasterises
//! that into palette indices, one byte a pixel. The app only shows those pixels. Every
//! motion is a whole number of cycles per loop (`secs`), so the frame at `secs` is the frame
//! at 0 and a demo can be left running for as long as it's up.

use serde::Deserialize;

use crate::data::Data;
use crate::intro::TuneDef;

/// The smallest and largest virtual screens a demo may ask for.
pub const MIN_SIZE: (u32, u32) = (160, 100);
pub const MAX_SIZE: (u32, u32) = (640, 512);
/// The largest a glyph may be scaled.
pub const MAX_SCALE: u32 = 4;
/// How far a tune's loop may be off a whole fraction of the demo's, in seconds.
pub const LOOP_SLACK: f32 = 0.01;

/// One drawn intro (`data/demos/<id>.ron`).
#[derive(Deserialize, Clone, Debug)]
pub struct DemoDef {
    /// From the file name.
    #[serde(skip)]
    pub id: String,
    /// The crew that signs it.
    pub group: String,
    /// The virtual screen, in pixels; the app scales it up by a whole number.
    pub width: u32,
    pub height: u32,
    /// One loop of everything, in seconds.
    pub secs: f32,
    /// The palette: a name and a 12-bit colour (`"0AF"`, a digit per channel), as the
    /// machine's colour registers held them. The first is the background.
    pub colours: Vec<(String, String)>,
    /// Named runs of colours (by name), for depth fades and text shades.
    pub ramps: Vec<(String, Vec<String>)>,
    pub camera: CameraDef,
    pub grid: GridDef,
    pub shapes: Vec<ShapeDef>,
    /// The solids along the track.
    pub solids: Vec<SolidDef>,
    pub font: FontDef,
    /// Named ways to print text.
    pub styles: Vec<(String, StyleDef)>,
    /// Fixed lines (the logo and what sits with it), centred.
    pub title: Vec<LineDef>,
    /// A clear band across the screen: the credits are printed in it, and nothing else is.
    pub band: BandDef,
    pub credits: CreditsDef,
    pub scroller: ScrollerDef,
    pub tune: TuneDef,
}

/// The eye: where it is, where it looks, and how it moves.
#[derive(Deserialize, Clone, Debug)]
pub struct CameraDef {
    /// Pixels from the centre to a point one unit off the axis, one unit away.
    pub focal: f32,
    /// The eye's height above the floor (world units).
    pub height: f32,
    /// The screen row the horizon sits on, looking level.
    pub horizon: f32,
    /// The nearest and farthest things drawn (world units).
    pub near: f32,
    pub far: f32,
    /// How far the eye travels in one loop (world units, forward): a whole number of grid
    /// cells.
    pub track: f32,
    /// Its drift, each a sine with a whole number of cycles a loop.
    #[serde(default)]
    pub sway: Vec<WaveDef>,
}

/// A sine on one of the camera's axes.
#[derive(Deserialize, Clone, Copy, Debug)]
pub struct WaveDef {
    pub axis: Axis,
    /// World units on `X` and `Y`, degrees on the turns.
    pub amp: f32,
    /// Whole cycles a loop.
    pub cycles: u32,
    /// Where it starts, 0-1 of a cycle.
    #[serde(default)]
    pub phase: f32,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// Sideways.
    X,
    /// Up.
    Y,
    /// Turning left and right.
    Yaw,
    /// Looking up and down.
    Pitch,
    /// Banking.
    Roll,
}

/// The grids the eye flies over (and under).
#[derive(Deserialize, Clone, Debug)]
pub struct GridDef {
    /// One cell, in world units.
    pub spacing: f32,
    /// Lines each side of the track's centre.
    pub lines: u32,
    pub planes: Vec<PlaneDef>,
}

/// A flat grid at height `y`.
#[derive(Deserialize, Clone, Debug)]
pub struct PlaneDef {
    pub y: f32,
    /// Its colours, nearest first, fading out to `far`.
    pub ramp: String,
    /// The soft glow under its lines, nearest first, out to as far as it has colours.
    #[serde(default)]
    pub glow: Option<String>,
}

/// A wireframe: points and the edges between them.
#[derive(Deserialize, Clone, Debug)]
pub struct ShapeDef {
    pub id: String,
    pub points: Vec<[f32; 3]>,
    pub edges: Vec<[usize; 2]>,
}

/// A shape set down beside the track.
#[derive(Deserialize, Clone, Debug)]
pub struct SolidDef {
    pub shape: String,
    /// Sideways, up, and how far along the track (`0..track`).
    pub at: [f32; 3],
    /// How big (the shape's points are multiplied by it).
    pub size: f32,
    /// Whole turns a loop about each axis.
    #[serde(default)]
    pub spin: [i32; 3],
    pub ramp: String,
    #[serde(default)]
    pub glow: Option<String>,
}

/// A bitmap font: every glyph `width` by `height`, `#` for ink and `.` for none. The
/// advance is the width, so leave the gap in the glyph. A space is always blank.
#[derive(Deserialize, Clone, Debug)]
pub struct FontDef {
    pub width: u32,
    pub height: u32,
    pub glyphs: Vec<(char, Vec<String>)>,
}

/// How a line of text is printed.
#[derive(Deserialize, Clone, Debug)]
pub struct StyleDef {
    /// Each font pixel drawn as `scale` by `scale`.
    pub scale: u32,
    /// The shade, top row to bottom, stretched over the glyph's height.
    pub ramp: String,
    /// Whole times a loop the shade rolls down through the rows (0: it holds still).
    #[serde(default)]
    pub cycles: u32,
    /// A colour drawn one pixel round the ink.
    #[serde(default)]
    pub outline: Option<String>,
    /// A colour drawn under the ink, one font pixel down and right.
    #[serde(default)]
    pub shadow: Option<String>,
}

/// One centred line.
#[derive(Deserialize, Clone, Debug)]
pub struct LineDef {
    pub text: String,
    /// Its top row.
    pub y: i32,
    pub style: String,
}

/// The rows from `y` for `h`.
#[derive(Deserialize, Clone, Debug)]
pub struct BandDef {
    pub y: i32,
    pub h: i32,
    /// Painted over the flight across the band, if set.
    #[serde(default)]
    pub fill: Option<String>,
    /// A one-pixel rule along its top and bottom, if set.
    #[serde(default)]
    pub edge: Option<String>,
}

/// Pages of centred lines in the band, each up for an equal share of the loop.
#[derive(Deserialize, Clone, Debug)]
pub struct CreditsDef {
    pub style: String,
    /// Pixels between lines.
    pub gap: i32,
    /// The part of each page's share it's shown for (0-1); the rest is blank.
    pub hold: f32,
    pub pages: Vec<Vec<String>>,
}

/// One line of text running right to left across the screen on a sine.
#[derive(Deserialize, Clone, Debug)]
pub struct ScrollerDef {
    pub style: String,
    /// Its top row, before the sine.
    pub y: i32,
    pub text: String,
    /// Names greeted after the text, in order.
    #[serde(default)]
    pub greetz: Vec<String>,
    /// Between the greetz.
    #[serde(default = "dash")]
    pub sep: String,
    /// Whole times a loop it crosses the screen.
    pub passes: u32,
    /// How far it swings up and down, in pixels.
    #[serde(default)]
    pub amp: f32,
    /// The sine's length across the screen, in pixels.
    #[serde(default = "wave")]
    pub wave: f32,
    /// Whole cycles a loop the sine runs along the line.
    #[serde(default)]
    pub cycles: u32,
}

fn dash() -> String {
    " - ".into()
}

fn wave() -> f32 {
    160.0
}

/// A line on screen, in pixels, in palette colour `ink` over a glow in `glow`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seg {
    pub a: [f32; 2],
    pub b: [f32; 2],
    pub ink: u8,
    pub glow: Option<u8>,
}

/// A glyph on screen: its top left in pixels, the style it's printed in (an index into
/// `styles`) and the shade's roll, in rows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Print {
    pub x: i32,
    pub y: i32,
    pub ch: char,
    pub style: usize,
    pub roll: usize,
}

/// Everything on screen at one moment, in drawing order: the flight, the band, the text.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame {
    pub segs: Vec<Seg>,
    pub prints: Vec<Print>,
    /// The page of credits up (`None` between pages).
    pub page: Option<usize>,
}

/// The eye at one moment.
#[derive(Clone, Copy, Debug)]
struct Eye {
    pos: [f32; 3],
    /// Yaw, pitch, roll in radians, as sines and cosines.
    yaw: (f32, f32),
    pitch: (f32, f32),
    roll: (f32, f32),
}

impl DemoDef {
    /// Where `t` seconds falls in the loop, 0-1.
    pub fn phase(&self, t: f64) -> f64 {
        (t / f64::from(self.secs.max(f32::EPSILON))).rem_euclid(1.0)
    }

    /// Everything on screen `t` seconds in.
    pub fn frame(&self, t: f64) -> Frame {
        let u = self.phase(t);
        let eye = self.eye(u);
        let mut f = Frame::default();
        self.grid(&eye, &mut f.segs);
        self.solids(&eye, u, &mut f.segs);
        for l in &self.title {
            if let Some(s) = self.style_index(&l.style) {
                self.centred(&l.text, l.y, s, u, &mut f.prints);
            }
        }
        f.page = self.page(u);
        if let (Some(p), Some(s)) = (f.page, self.style_index(&self.credits.style)) {
            let lines = &self.credits.pages[p];
            let mut y = self.credits_top(lines.len());
            for l in lines {
                self.centred(l, y, s, u, &mut f.prints);
                y += self.line_h(s) + self.credits.gap;
            }
        }
        self.scroll(u, &mut f.prints);
        f
    }

    /// The page of credits up at `u` (0-1 of the loop), if one is.
    fn page(&self, u: f64) -> Option<usize> {
        let n = self.credits.pages.len();
        if n == 0 {
            return None;
        }
        let at = u * n as f64;
        let p = (at.floor() as usize).min(n - 1);
        (at.fract() < f64::from(self.credits.hold)).then_some(p)
    }

    /// The camera at `u`.
    fn eye(&self, u: f64) -> Eye {
        let c = &self.camera;
        let mut pos = [0.0, c.height, (u * f64::from(c.track)) as f32];
        let (mut yaw, mut pitch, mut roll) = (0.0f32, 0.0f32, 0.0f32);
        for w in &c.sway {
            let a = std::f64::consts::TAU * (u * f64::from(w.cycles) + f64::from(w.phase));
            let v = w.amp * a.sin() as f32;
            match w.axis {
                Axis::X => pos[0] += v,
                Axis::Y => pos[1] += v,
                Axis::Yaw => yaw += v.to_radians(),
                Axis::Pitch => pitch += v.to_radians(),
                Axis::Roll => roll += v.to_radians(),
            }
        }
        Eye {
            pos,
            yaw: yaw.sin_cos(),
            pitch: pitch.sin_cos(),
            roll: roll.sin_cos(),
        }
    }

    /// A world point in the eye's space: x right, y up, z ahead.
    fn view(&self, e: &Eye, p: [f32; 3]) -> [f32; 3] {
        let (x, y, z) = (p[0] - e.pos[0], p[1] - e.pos[1], p[2] - e.pos[2]);
        let (s, c) = e.yaw;
        let (x, z) = (x * c - z * s, x * s + z * c);
        let (s, c) = e.pitch;
        let (y, z) = (y * c - z * s, y * s + z * c);
        let (s, c) = e.roll;
        let (x, y) = (x * c + y * s, y * c - x * s);
        [x, y, z]
    }

    /// A point in the eye's space on screen.
    fn project(&self, p: [f32; 3]) -> [f32; 2] {
        let f = self.camera.focal / p[2];
        [
            self.width as f32 / 2.0 + p[0] * f,
            self.camera.horizon - p[1] * f,
        ]
    }

    /// A world line on screen, clipped to the near plane and the screen's edges.
    fn line(&self, e: &Eye, a: [f32; 3], b: [f32; 3]) -> Option<([f32; 2], [f32; 2], f32)> {
        let (mut a, mut b) = (self.view(e, a), self.view(e, b));
        let near = self.camera.near;
        if a[2] < near && b[2] < near {
            return None;
        }
        // Cut what's behind the near plane.
        let cut = |p: [f32; 3], q: [f32; 3]| {
            let k = (near - p[2]) / (q[2] - p[2]);
            [p[0] + (q[0] - p[0]) * k, p[1] + (q[1] - p[1]) * k, near]
        };
        if a[2] < near {
            a = cut(a, b);
        } else if b[2] < near {
            b = cut(b, a);
        }
        let depth = (a[2] + b[2]) / 2.0;
        let (pa, pb) = clip(
            self.project(a),
            self.project(b),
            self.width as f32 - 1.0,
            self.height as f32 - 1.0,
        )?;
        Some((pa, pb, depth))
    }

    /// The colour at `depth` along ramp `ramp`, out to `far` (the glow's ramp runs out where
    /// its colours do, at the same spacing).
    fn faded(&self, ramp: &str, depth: f32, len_for: Option<usize>) -> Option<u8> {
        let r = self.ramp_at(ramp)?;
        let near = self.camera.near;
        let span = (self.camera.far - near).max(f32::EPSILON);
        let n = len_for.unwrap_or(r.len()).max(1);
        let i = (((depth - near) / span) * n as f32).floor().max(0.0) as usize;
        r.get(i).copied()
    }

    /// The grids: lines along the track cut at every cell so each piece fades with its depth,
    /// and lines across it.
    fn grid(&self, e: &Eye, out: &mut Vec<Seg>) {
        let g = &self.grid;
        let sp = g.spacing;
        let n = g.lines as i32;
        let z0 = ((e.pos[2] - sp) / sp).floor() as i32;
        let cells = ((self.camera.far + sp) / sp).ceil() as i32 + 1;
        for plane in &g.planes {
            let Some(ramp) = self.ramp_at(&plane.ramp) else {
                continue;
            };
            let glow = |d: f32| {
                plane
                    .glow
                    .as_deref()
                    .and_then(|gl| self.faded(gl, d, Some(ramp.len())))
            };
            let mut seg = |a: [f32; 3], b: [f32; 3]| {
                if let Some((pa, pb, d)) = self.line(e, a, b)
                    && d <= self.camera.far
                    && let Some(ink) = self.faded(&plane.ramp, d, None)
                {
                    out.push(Seg {
                        a: pa,
                        b: pb,
                        ink,
                        glow: glow(d),
                    });
                }
            };
            for m in z0..z0 + cells {
                let (za, zb) = (m as f32 * sp, (m + 1) as f32 * sp);
                for k in -n..=n {
                    let x = k as f32 * sp;
                    seg([x, plane.y, za], [x, plane.y, zb]);
                }
                let (xa, xb) = (-n as f32 * sp, n as f32 * sp);
                seg([xa, plane.y, za], [xb, plane.y, za]);
            }
        }
    }

    /// The solids, each where the track puts it relative to the eye, turned by `u`.
    fn solids(&self, e: &Eye, u: f64, out: &mut Vec<Seg>) {
        let track = self.camera.track;
        for s in &self.solids {
            let Some(shape) = self.shapes.iter().find(|sh| sh.id == s.shape) else {
                continue;
            };
            // Shown from just behind the eye to a track's length ahead of it, so a solid
            // passes by and comes round again from beyond `far`.
            let back = s.size * 2.0;
            let ahead = (s.at[2] - e.pos[2] + back).rem_euclid(track) - back;
            let centre = [s.at[0], s.at[1], e.pos[2] + ahead];
            let turn = |k: usize| {
                let a = std::f64::consts::TAU * u * f64::from(s.spin[k]);
                (a as f32).sin_cos()
            };
            let (rx, ry, rz) = (turn(0), turn(1), turn(2));
            let place = |p: [f32; 3]| {
                let [x, y, z] = p.map(|v| v * s.size);
                let (y, z) = (y * rx.1 - z * rx.0, y * rx.0 + z * rx.1);
                let (x, z) = (x * ry.1 + z * ry.0, z * ry.1 - x * ry.0);
                let (x, y) = (x * rz.1 - y * rz.0, x * rz.0 + y * rz.1);
                [centre[0] + x, centre[1] + y, centre[2] + z]
            };
            let Some(ramp) = self.ramp_at(&s.ramp) else {
                continue;
            };
            for [i, j] in &shape.edges {
                let (Some(&a), Some(&b)) = (shape.points.get(*i), shape.points.get(*j)) else {
                    continue;
                };
                if let Some((pa, pb, d)) = self.line(e, place(a), place(b))
                    && d <= self.camera.far
                    && let Some(ink) = self.faded(&s.ramp, d, None)
                {
                    let glow = s
                        .glow
                        .as_deref()
                        .and_then(|g| self.faded(g, d, Some(ramp.len())));
                    out.push(Seg {
                        a: pa,
                        b: pb,
                        ink,
                        glow,
                    });
                }
            }
        }
    }

    /// The scroller's glyphs at `u`.
    fn scroll(&self, u: f64, out: &mut Vec<Print>) {
        let sc = &self.scroller;
        let Some(si) = self.style_index(&sc.style) else {
            return;
        };
        let s = &self.styles[si].1;
        let adv = (self.font.width * s.scale) as i32;
        let text = self.scroller_text();
        let len = text.chars().count() as i32 * adv;
        let run = f64::from(self.width as i32 + len);
        let along = (u * f64::from(sc.passes)).fract();
        let x0 = self.width as i32 - (along * run).floor() as i32;
        let roll = self.roll(s, u);
        let sway = std::f64::consts::TAU * u * f64::from(sc.cycles);
        for (n, ch) in text.chars().enumerate() {
            let x = x0 + n as i32 * adv;
            if x + adv <= 0 || x >= self.width as i32 || ch == ' ' {
                continue;
            }
            let a = std::f64::consts::TAU * f64::from(x) / f64::from(sc.wave.max(1.0)) + sway;
            let y = sc.y + (f64::from(sc.amp) * a.sin()).round() as i32;
            out.push(Print {
                x,
                y,
                ch,
                style: si,
                roll,
            });
        }
    }

    /// What the scroller says: its text, then the greetz.
    pub fn scroller_text(&self) -> String {
        let sc = &self.scroller;
        if sc.greetz.is_empty() {
            return sc.text.clone();
        }
        format!("{}{}", sc.text, sc.greetz.join(&sc.sep))
    }

    /// The shade's roll in rows for style `s` at `u`.
    fn roll(&self, s: &StyleDef, u: f64) -> usize {
        let rows = self.ramp_at(&s.ramp).map_or(1, |r| r.len()).max(1);
        ((u * f64::from(s.cycles) * rows as f64).floor() as usize) % rows
    }

    fn centred(&self, text: &str, y: i32, si: usize, u: f64, out: &mut Vec<Print>) {
        let s = &self.styles[si].1;
        let adv = (self.font.width * s.scale) as i32;
        let w = text.chars().count() as i32 * adv;
        let x0 = (self.width as i32 - w) / 2;
        let roll = self.roll(s, u);
        for (n, ch) in text.chars().enumerate() {
            if ch != ' ' {
                out.push(Print {
                    x: x0 + n as i32 * adv,
                    y,
                    ch,
                    style: si,
                    roll,
                });
            }
        }
    }

    fn line_h(&self, si: usize) -> i32 {
        (self.font.height * self.styles[si].1.scale) as i32
    }

    /// The top row of a page of `lines`, centred in the band.
    fn credits_top(&self, lines: usize) -> i32 {
        let Some(si) = self.style_index(&self.credits.style) else {
            return self.band.y;
        };
        let n = lines as i32;
        let h = n * self.line_h(si) + (n - 1).max(0) * self.credits.gap;
        self.band.y + (self.band.h - h) / 2
    }

    fn style_index(&self, name: &str) -> Option<usize> {
        self.styles.iter().position(|(n, _)| n == name)
    }

    fn colour(&self, name: &str) -> Option<u8> {
        self.colours
            .iter()
            .position(|(n, _)| n == name)
            .and_then(|i| u8::try_from(i).ok())
    }

    /// A ramp as palette indices.
    fn ramp_at(&self, name: &str) -> Option<Vec<u8>> {
        let (_, r) = self.ramps.iter().find(|(n, _)| n == name)?;
        r.iter().map(|c| self.colour(c)).collect()
    }

    fn glyph(&self, ch: char) -> Option<&[String]> {
        self.font
            .glyphs
            .iter()
            .find(|(c, _)| *c == ch)
            .map(|(_, rows)| rows.as_slice())
    }

    /// The palette as 8-bit RGB, in index order (a 12-bit digit `n` is `n * 17`).
    pub fn rgb(&self) -> Vec<[u8; 3]> {
        self.colours
            .iter()
            .map(|(_, hex)| rgb4(hex).unwrap_or([0, 0, 0]))
            .collect()
    }

    /// `f` drawn: `width * height` palette indices, row by row. Lines go down a pixel at a
    /// time, every glow before every line so a glow never covers one; then the band, then the
    /// text (shadow, outline, ink).
    pub fn pixels(&self, f: &Frame) -> Vec<u8> {
        let (w, h) = (self.width as i32, self.height as i32);
        let mut px = vec![0u8; (w * h) as usize];
        let mut put = |x: i32, y: i32, c: u8| {
            if (0..w).contains(&x) && (0..h).contains(&y) {
                px[(y * w + x) as usize] = c;
            }
        };
        for s in &f.segs {
            if let Some(g) = s.glow {
                walk(s.a, s.b, |x, y| {
                    for (dx, dy) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
                        put(x + dx, y + dy, g);
                    }
                });
            }
        }
        for s in &f.segs {
            walk(s.a, s.b, |x, y| put(x, y, s.ink));
        }
        let b = &self.band;
        if let Some(c) = b.fill.as_deref().and_then(|c| self.colour(c)) {
            for y in b.y..b.y + b.h {
                for x in 0..w {
                    put(x, y, c);
                }
            }
        }
        if let Some(c) = b.edge.as_deref().and_then(|c| self.colour(c)) {
            for x in 0..w {
                put(x, b.y - 1, c);
                put(x, b.y + b.h, c);
            }
        }
        for p in &f.prints {
            let Some(rows) = self.glyph(p.ch) else {
                continue;
            };
            let st = &self.styles[p.style].1;
            let sc = st.scale as i32;
            let ink = |fx: usize, fy: usize| {
                rows.get(fy)
                    .is_some_and(|r| r.as_bytes().get(fx) == Some(&b'#'))
            };
            let cells = || {
                (0..rows.len()).flat_map(move |fy| {
                    (0..self.font.width as usize)
                        .filter(move |&fx| ink(fx, fy))
                        .map(move |fx| (fx as i32, fy as i32))
                })
            };
            let block = |put: &mut dyn FnMut(i32, i32, u8), x: i32, y: i32, c: u8| {
                for dy in 0..sc {
                    for dx in 0..sc {
                        put(x + dx, y + dy, c);
                    }
                }
            };
            if let Some(c) = st.shadow.as_deref().and_then(|c| self.colour(c)) {
                for (fx, fy) in cells() {
                    block(&mut put, p.x + (fx + 1) * sc, p.y + (fy + 1) * sc, c);
                }
            }
            if let Some(c) = st.outline.as_deref().and_then(|c| self.colour(c)) {
                for (fx, fy) in cells() {
                    for (ox, oy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                        block(&mut put, p.x + fx * sc + ox, p.y + fy * sc + oy, c);
                    }
                }
            }
            let Some(ramp) = self.ramp_at(&st.ramp) else {
                continue;
            };
            let tall = (self.font.height as i32 * sc).max(1);
            for (fx, fy) in cells() {
                for dy in 0..sc {
                    let row = fy * sc + dy;
                    let i = (row as usize * ramp.len() / tall as usize + ramp.len() - p.roll)
                        % ramp.len();
                    for dx in 0..sc {
                        put(p.x + fx * sc + dx, p.y + row, ramp[i]);
                    }
                }
            }
        }
        px
    }

    /// Checks the file against itself and the game's data: every name it uses exists, every
    /// line fits, the loop closes (and the tune loops with it), and its greetz name only
    /// crews, handles and boards the game's world knows.
    pub fn validate(&self, data: &Data) -> Result<(), String> {
        let err = |m: String| Err(format!("demos/{}.ron: {m}", self.id));
        if let Err(m) = self.check() {
            return err(m);
        }
        let known = known_names(data);
        for g in &self.scroller.greetz {
            if !known.contains(&plain(g)) {
                return err(format!(
                    "greets `{g}`, who isn't a crew, a handle or a board in the game"
                ));
            }
        }
        if data.signs(&self.group) {
            return err(format!(
                "`{}` signs something in the game; a demo's crew is its own",
                self.group
            ));
        }
        Ok(())
    }

    /// Everything [`Self::validate`] checks that needs only the file.
    pub fn check(&self) -> Result<(), String> {
        if self.group.trim().is_empty() {
            return Err("no crew signs it".into());
        }
        let (w, h) = (self.width, self.height);
        if !(MIN_SIZE.0..=MAX_SIZE.0).contains(&w) || !(MIN_SIZE.1..=MAX_SIZE.1).contains(&h) {
            return Err(format!(
                "a {w}x{h} screen (from {MIN_SIZE:?} to {MAX_SIZE:?})"
            ));
        }
        if self.colours.is_empty() || self.colours.len() > 256 {
            return Err("1 to 256 colours".into());
        }
        for (n, (name, hex)) in self.colours.iter().enumerate() {
            if rgb4(hex).is_none() {
                return Err(format!("colour `{name}` is `{hex}`, not three hex digits"));
            }
            if self.colours[..n].iter().any(|(o, _)| o == name) {
                return Err(format!("two colours called `{name}`"));
            }
        }
        for (name, r) in &self.ramps {
            if r.is_empty() {
                return Err(format!("ramp `{name}` has no colours"));
            }
            if let Some(c) = r.iter().find(|c| self.colour(c).is_none()) {
                return Err(format!("ramp `{name}` names unknown colour `{c}`"));
            }
        }
        let ramp = |r: &str, what: &str| match self.ramp_at(r) {
            Some(_) => Ok(()),
            None => Err(format!("{what} names unknown ramp `{r}`")),
        };
        let colour = |c: &Option<String>, what: &str| match c {
            Some(c) if self.colour(c).is_none() => {
                Err(format!("{what} names unknown colour `{c}`"))
            }
            _ => Ok(()),
        };
        self.check_flight(&ramp)?;
        self.check_font()?;
        for (name, s) in &self.styles {
            if !(1..=MAX_SCALE).contains(&s.scale) {
                return Err(format!(
                    "style `{name}` at scale {} (1-{MAX_SCALE})",
                    s.scale
                ));
            }
            ramp(&s.ramp, &format!("style `{name}`"))?;
            colour(&s.outline, &format!("style `{name}`"))?;
            colour(&s.shadow, &format!("style `{name}`"))?;
        }
        colour(&self.band.fill, "the band")?;
        colour(&self.band.edge, "the band")?;
        if self.band.h <= 0 || self.band.y < 1 || self.band.y + self.band.h >= h as i32 {
            return Err("the band must sit inside the screen".into());
        }
        self.check_text()?;
        self.check_loop()
    }

    fn check_flight(&self, ramp: &dyn Fn(&str, &str) -> Result<(), String>) -> Result<(), String> {
        let c = &self.camera;
        if !(c.near > 0.0 && c.far > c.near && c.focal > 0.0) {
            return Err("the camera needs 0 < near < far and a focal length".into());
        }
        let g = &self.grid;
        if g.spacing <= 0.0 || g.planes.is_empty() {
            return Err("the grid needs a spacing and a plane".into());
        }
        let cells = c.track / g.spacing;
        if c.track <= 0.0 || (cells - cells.round()).abs() > 1e-3 {
            return Err(format!(
                "the track ({}) must be a whole number of grid cells ({})",
                c.track, g.spacing
            ));
        }
        if let Some(w) = c.sway.iter().find(|w| w.cycles == 0) {
            return Err(format!(
                "a {:?} sway with no cycles never comes back",
                w.axis
            ));
        }
        for p in &g.planes {
            ramp(&p.ramp, "a plane")?;
            if let Some(gl) = &p.glow {
                ramp(gl, "a plane's glow")?;
            }
        }
        for (n, s) in self.shapes.iter().enumerate() {
            if self.shapes[..n].iter().any(|o| o.id == s.id) {
                return Err(format!("two shapes called `{}`", s.id));
            }
            if s.edges.is_empty() {
                return Err(format!("shape `{}` has no edges", s.id));
            }
            if let Some(e) = s
                .edges
                .iter()
                .find(|e| e.iter().any(|&i| i >= s.points.len()))
            {
                return Err(format!(
                    "shape `{}` has an edge {e:?} to a point it doesn't have",
                    s.id
                ));
            }
        }
        for s in &self.solids {
            if !self.shapes.iter().any(|sh| sh.id == s.shape) {
                return Err(format!("a solid names unknown shape `{}`", s.shape));
            }
            ramp(&s.ramp, &format!("solid `{}`", s.shape))?;
            if let Some(g) = &s.glow {
                ramp(g, &format!("solid `{}`'s glow", s.shape))?;
            }
            if !(0.0..c.track).contains(&s.at[2]) || s.size <= 0.0 {
                return Err(format!(
                    "solid `{}` must sit on the track (0 to {}) and have a size",
                    s.shape, c.track
                ));
            }
            // Coming round again, it must reappear beyond what's drawn.
            if c.track < c.far + s.size * 2.0 {
                return Err(format!(
                    "the track ({}) is too short to bring `{}` round from beyond far ({})",
                    c.track, s.shape, c.far
                ));
            }
        }
        Ok(())
    }

    fn check_font(&self) -> Result<(), String> {
        let f = &self.font;
        if f.width == 0 || f.height == 0 {
            return Err("the font has no size".into());
        }
        for (n, (ch, rows)) in f.glyphs.iter().enumerate() {
            if f.glyphs[..n].iter().any(|(o, _)| o == ch) {
                return Err(format!("the font draws `{ch}` twice"));
            }
            if rows.len() != f.height as usize
                || rows.iter().any(|r| {
                    r.chars().count() != f.width as usize || r.chars().any(|c| c != '#' && c != '.')
                })
            {
                return Err(format!(
                    "glyph `{ch}` must be {} rows of {} `#` or `.`",
                    f.height, f.width
                ));
            }
        }
        Ok(())
    }

    /// Every glyph exists, and every line fits where it's put.
    fn check_text(&self) -> Result<(), String> {
        let (w, h) = (self.width as i32, self.height as i32);
        let style = |name: &str| {
            self.style_index(name)
                .ok_or(format!("unknown style `{name}`"))
        };
        let glyphs = |t: &str| match t.chars().find(|&c| c != ' ' && self.glyph(c).is_none()) {
            Some(c) => Err(format!("the font has no `{c}` (in \"{t}\")")),
            None => Ok(()),
        };
        let wide = |t: &str, si: usize| {
            t.chars().count() as i32 * (self.font.width * self.styles[si].1.scale) as i32
        };
        let (band_top, band_end) = (self.band.y - 1, self.band.y + self.band.h + 1);
        let clear = |top: i32, bottom: i32| bottom <= band_top || top >= band_end;
        for l in &self.title {
            let si = style(&l.style)?;
            glyphs(&l.text)?;
            let bottom = l.y + self.line_h(si) + self.styles[si].1.scale as i32;
            if wide(&l.text, si) > w || l.y < 0 || bottom > h {
                return Err(format!("\"{}\" doesn't fit on the screen", l.text));
            }
            if !clear(l.y - 1, bottom) {
                return Err(format!("\"{}\" sits in the band", l.text));
            }
        }
        let cr = &self.credits;
        let si = style(&cr.style)?;
        if cr.pages.iter().any(|p| p.is_empty()) || !(0.0..=1.0).contains(&cr.hold) {
            return Err("every page of credits needs a line, and `hold` is 0-1".into());
        }
        for page in &cr.pages {
            let n = page.len() as i32;
            let tall = n * self.line_h(si) + (n - 1) * cr.gap;
            for l in page {
                glyphs(l)?;
                if wide(l, si) > w {
                    return Err(format!("credit \"{l}\" is wider than the screen"));
                }
            }
            if tall > self.band.h {
                return Err(format!("a page of {n} lines is taller than the band"));
            }
        }
        let sc = &self.scroller;
        let si = style(&sc.style)?;
        glyphs(&self.scroller_text())?;
        if sc.passes == 0 {
            return Err("the scroller must cross at least once a loop".into());
        }
        let sway = sc.amp.abs().ceil() as i32;
        let (top, bottom) = (
            sc.y - sway - 1,
            sc.y + self.line_h(si) + self.styles[si].1.scale as i32 + sway,
        );
        if top < 0 || bottom > h {
            return Err("the scroller swings off the screen".into());
        }
        if !clear(top, bottom) {
            return Err("the scroller swings into the band".into());
        }
        Ok(())
    }

    /// The tune is good and loops a whole number of times in one loop of the flight.
    fn check_loop(&self) -> Result<(), String> {
        if !(self.secs > 0.0 && self.secs.is_finite()) {
            return Err("a loop needs a length".into());
        }
        self.tune.validate().map_err(|e| format!("the tune: {e}"))?;
        let t = self.tune.loop_secs(0);
        if t > crate::music::MAX_LOOP_SECS {
            return Err(format!(
                "the tune loops every {t:.1}s, past {}s",
                crate::music::MAX_LOOP_SECS
            ));
        }
        let n = (self.secs / t).round();
        if n < 1.0 || (self.secs - n * t).abs() > LOOP_SLACK {
            return Err(format!(
                "the tune loops every {t:.2}s, which doesn't go into the loop's {}s",
                self.secs
            ));
        }
        Ok(())
    }
}

/// A 12-bit colour (`"0AF"`) as 8-bit RGB.
fn rgb4(hex: &str) -> Option<[u8; 3]> {
    let d: Vec<u8> = hex
        .chars()
        .map(|c| c.to_digit(16).map(|v| v as u8 * 17))
        .collect::<Option<_>>()?;
    match d[..] {
        [r, g, b] => Some([r, g, b]),
        _ => None,
    }
}

/// A name as greetz compare it: lower case, without a leading "the " or an aside in
/// brackets.
fn plain(name: &str) -> String {
    let n = name
        .split(" (")
        .next()
        .unwrap_or(name)
        .trim()
        .to_lowercase();
    n.strip_prefix("the ").unwrap_or(&n).to_string()
}

/// Every crew, handle and board the game's world names: who signs its cracktros (each name
/// a group joins with ` & `), who they greet, and the boards.
fn known_names(data: &Data) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for i in &data.intros {
        names.extend(i.group.split(" & ").map(plain));
        names.extend(i.greetz.iter().map(|g| plain(g)));
    }
    names.extend(data.boards.iter().map(|b| plain(&b.name)));
    names
}

/// Clips the line `a`-`b` to `0..=w` by `0..=h` (Liang-Barsky); `None` if none of it shows.
fn clip(a: [f32; 2], b: [f32; 2], w: f32, h: f32) -> Option<([f32; 2], [f32; 2])> {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for (p, q) in [(-dx, a[0]), (dx, w - a[0]), (-dy, a[1]), (dy, h - a[1])] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                t0 = t0.max(r);
            } else {
                t1 = t1.min(r);
            }
            if t0 > t1 {
                return None;
            }
        }
    }
    // Clamped too, so rounding never leaves an end a hair off the edge.
    let at = |t: f32| [(a[0] + dx * t).clamp(0.0, w), (a[1] + dy * t).clamp(0.0, h)];
    Some((at(t0), at(t1)))
}

/// Every pixel on the line `a`-`b`, a step at a time (Bresenham).
fn walk(a: [f32; 2], b: [f32; 2], mut f: impl FnMut(i32, i32)) {
    let (mut x, mut y) = (a[0].round() as i32, a[1].round() as i32);
    let (x1, y1) = (b[0].round() as i32, b[1].round() as i32);
    let (dx, dy) = ((x1 - x).abs(), -(y1 - y).abs());
    let (sx, sy) = (if x < x1 { 1 } else { -1 }, if y < y1 { 1 } else { -1 });
    let mut e = dx + dy;
    loop {
        f(x, y);
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * e;
        if e2 >= dy {
            e += dy;
            x += sx;
        }
        if e2 <= dx {
            e += dx;
            y += sy;
        }
    }
}
