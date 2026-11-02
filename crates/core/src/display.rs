//! CRT display presets (`data/displays.ron`) and the screen geometry they imply.
//!
//! The app draws the effects in a shader; the geometry lives here, pure and tested, because the
//! mouse has to follow it too: a click on the curved glass must land on what you see there.

use serde::{Deserialize, Serialize};

use crate::data::rgb;

/// How pixels are rasterized on the glass.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Raster {
    #[default]
    None,
    Scanline,
    Pixel,
}

/// Effect strengths as written in the data file (cool-retro-term's parameter names).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct EffectsDef {
    pub flickering: f32,
    pub horizontal_sync: f32,
    pub static_noise: f32,
    pub jitter: f32,
    pub screen_curvature: f32,
    pub screen_radius: f32,
    pub glowing_line: f32,
    pub bloom: f32,
    pub burn_in: f32,
    pub rgb_shift: f32,
    pub rasterization: Raster,
    pub brightness: f32,
    pub contrast: f32,
    pub ambient_light: f32,
    pub frame_size: f32,
    pub frame_color: String,
    pub frame_shininess: f32,
}

/// A display preset.
#[derive(Deserialize, Clone, Debug)]
pub struct DisplayDef {
    pub id: String,
    pub label: String,
    /// The colour theme this preset switches to.
    pub theme: Option<String>,
    pub effects: EffectsDef,
}

/// Effects ready for drawing: plain numbers, `Copy`, frame colour resolved.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Effects {
    pub flickering: f32,
    pub horizontal_sync: f32,
    pub static_noise: f32,
    pub jitter: f32,
    pub screen_curvature: f32,
    pub screen_radius: f32,
    pub glowing_line: f32,
    pub bloom: f32,
    pub burn_in: f32,
    pub rgb_shift: f32,
    pub rasterization: Raster,
    pub brightness: f32,
    pub contrast: f32,
    pub ambient_light: f32,
    pub frame_size: f32,
    /// Linear 0..1 RGB.
    pub frame_color: [f32; 3],
    pub frame_shininess: f32,
}

/// Fraction of each side of the window the bezel takes at `frame_size` 1.0.
pub const FRAME_SCALE: f32 = 0.07;
/// Barrel distortion at `screen_curvature` 1.0.
pub const CURVE_SCALE: f32 = 0.25;
/// Tube corner radius (as a fraction of the screen) at `screen_radius` 1.0.
pub const RADIUS_SCALE: f32 = 0.12;

impl Effects {
    pub fn from_def(d: &EffectsDef) -> Self {
        let [r, g, b] = rgb(&d.frame_color).unwrap_or([0x80, 0x80, 0x80]);
        Self {
            flickering: d.flickering,
            horizontal_sync: d.horizontal_sync,
            static_noise: d.static_noise,
            jitter: d.jitter,
            screen_curvature: d.screen_curvature,
            screen_radius: d.screen_radius,
            glowing_line: d.glowing_line,
            bloom: d.bloom,
            burn_in: d.burn_in,
            rgb_shift: d.rgb_shift,
            rasterization: d.rasterization,
            brightness: d.brightness,
            contrast: d.contrast,
            ambient_light: d.ambient_light,
            frame_size: d.frame_size,
            frame_color: [
                f32::from(r) / 255.0,
                f32::from(g) / 255.0,
                f32::from(b) / 255.0,
            ],
            frame_shininess: d.frame_shininess,
        }
    }

    /// The bezel's width as a fraction of the window, per side.
    pub fn frame(&self) -> f32 {
        self.frame_size * FRAME_SCALE
    }

    pub fn curve(&self) -> f32 {
        self.screen_curvature * CURVE_SCALE
    }

    pub fn radius(&self) -> f32 {
        self.screen_radius * RADIUS_SCALE
    }

    /// Whether the image is warped at all (and the mouse needs warping with it).
    pub fn warps(&self) -> bool {
        self.frame() > 0.0 || self.curve() > 0.0
    }
}

impl DisplayDef {
    pub fn validate(defs: &[DisplayDef], themes: &[crate::data::ThemeDef]) -> Result<(), String> {
        if defs.is_empty() {
            return Err("no display presets".into());
        }
        for (i, d) in defs.iter().enumerate() {
            if defs[..i].iter().any(|o| o.id == d.id) {
                return Err(format!("display `{}` is listed twice", d.id));
            }
            if let Some(t) = &d.theme
                && !themes.iter().any(|x| &x.id == t)
            {
                return Err(format!("display `{}` names unknown theme `{t}`", d.id));
            }
            let e = &d.effects;
            let values = [
                e.flickering,
                e.horizontal_sync,
                e.static_noise,
                e.jitter,
                e.screen_curvature,
                e.screen_radius,
                e.glowing_line,
                e.bloom,
                e.burn_in,
                e.rgb_shift,
                e.brightness,
                e.contrast,
                e.ambient_light,
                e.frame_size,
                e.frame_shininess,
            ];
            if values.iter().any(|v| !(0.0..=1.0).contains(v)) {
                return Err(format!("display `{}` has a value outside 0.0-1.0", d.id));
            }
            if rgb(&e.frame_color).is_none() {
                return Err(format!("display `{}` has a bad frame colour", d.id));
            }
        }
        Ok(())
    }
}

/// Maps a point on the window (0..1 each way, y down) to the point of the picture shown there,
/// or `None` if it's the bezel or outside the rounded tube. The shader does the same maths.
pub fn screen_to_content(uv: [f32; 2], e: &Effects) -> Option<[f32; 2]> {
    let f = e.frame();
    let inner = [(uv[0] - f) / (1.0 - 2.0 * f), (uv[1] - f) / (1.0 - 2.0 * f)];
    // Rounded tube: signed distance to a rounded rectangle filling the inner area.
    let r = e.radius();
    let qx = (inner[0] - 0.5).abs() - (0.5 - r);
    let qy = (inner[1] - 0.5).abs() - (0.5 - r);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() - r;
    if outside > 0.0 {
        return None;
    }
    // Barrel distortion: further from the centre, the picture is pushed outward.
    let (cx, cy) = (inner[0] - 0.5, inner[1] - 0.5);
    let k = 1.0 + (cx * cx + cy * cy) * 4.0 * e.curve();
    let c = [0.5 + cx * k, 0.5 + cy * k];
    if !(0.0..=1.0).contains(&c[0]) || !(0.0..=1.0).contains(&c[1]) {
        return None;
    }
    Some(c)
}
