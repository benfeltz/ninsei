//! A demo on screen: the frame core drew (`ninsei_core::demo`), shown as a picture of its
//! virtual screen scaled up by a whole number and centred on black, each virtual pixel a
//! square of physical pixels. It knows nothing of the scene: core says what every pixel is
//! and which colour each index is.

use egui::{Color32, ColorImage, Rect, TextureHandle, TextureOptions, Ui, pos2, vec2};
use ninsei_core::demo::DemoDef;

/// What the view keeps between frames: the picture it updates in place.
#[derive(Default)]
pub struct DemoView {
    tex: Option<TextureHandle>,
    /// The demo's palette, read from its file once.
    palette: Vec<Color32>,
    /// The rect it was last drawn in, for tests.
    pub drawn: Option<Rect>,
}

/// Where a `w` by `h` virtual screen goes on `screen` (in points, at `ppp` physical pixels a
/// point): the largest whole multiple that fits, centred, on whole physical pixels. A window
/// smaller than the virtual screen shrinks it to fit whole, never cutting it.
pub fn place(screen: Rect, ppp: f32, w: u32, h: u32) -> Rect {
    let (sw, sh) = (screen.width() * ppp, screen.height() * ppp);
    let (w, h) = (w as f32, h as f32);
    let fit = (sw / w).min(sh / h);
    let k = if fit >= 1.0 {
        fit.floor()
    } else {
        fit.max(0.0)
    };
    let (pw, ph) = ((w * k).floor(), (h * k).floor());
    let x = (screen.min.x * ppp + ((sw - pw) / 2.0).floor()).round();
    let y = (screen.min.y * ppp + ((sh - ph) / 2.0).floor()).round();
    Rect::from_min_size(pos2(x / ppp, y / ppp), vec2(pw / ppp, ph / ppp))
}

impl DemoView {
    /// Draws `def` `t` seconds in over the whole of `ui`'s screen.
    pub fn paint(&mut self, ui: &mut Ui, def: &DemoDef, t: f64) {
        let ctx = ui.ctx().clone();
        let screen = ctx.content_rect();
        let painter = ui.painter_at(screen);
        painter.rect_filled(screen, 0.0, Color32::BLACK);
        if self.palette.is_empty() {
            self.palette = def
                .rgb()
                .into_iter()
                .map(|[r, g, b]| Color32::from_rgb(r, g, b))
                .collect();
        }
        let palette = &self.palette;
        let px = def.pixels(&def.frame(t));
        let image = ColorImage::new(
            [def.width as usize, def.height as usize],
            px.iter()
                .map(|&i| {
                    palette
                        .get(usize::from(i))
                        .copied()
                        .unwrap_or(Color32::BLACK)
                })
                .collect(),
        );
        match &mut self.tex {
            Some(tex) => tex.set(image, TextureOptions::NEAREST),
            None => {
                self.tex = Some(ctx.load_texture("demo", image, TextureOptions::NEAREST));
            }
        }
        let rect = place(screen, ctx.pixels_per_point(), def.width, def.height);
        if let Some(tex) = &self.tex {
            painter.image(
                tex.id(),
                rect,
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        self.drawn = Some(rect);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::uitest::Harness;
    use ninsei_core::Data;

    fn whole(v: f32) -> bool {
        (v - v.round()).abs() < 1e-3
    }

    #[test]
    fn it_scales_by_a_whole_number_centred_on_whole_pixels() {
        for (sw, sh, ppp, k) in [
            (1280.0, 800.0, 1.0, 4.0),
            (1280.0, 800.0, 2.0, 8.0),
            (1000.0, 700.0, 1.0, 3.0),
            (641.0, 401.0, 1.0, 2.0),
            (1920.0, 1080.0, 1.5, 8.0),
        ] {
            let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(sw, sh));
            let r = place(screen, ppp, 320, 200);
            assert_eq!(r.width() * ppp, 320.0 * k, "{sw}x{sh} at {ppp}");
            assert_eq!(r.height() * ppp, 200.0 * k);
            assert!(whole(r.min.x * ppp) && whole(r.min.y * ppp), "{r:?}");
            assert!(screen.contains_rect(r));
            assert!((r.center().x - screen.center().x).abs() <= 1.0);
        }
    }

    #[test]
    fn a_window_smaller_than_the_screen_shrinks_it_whole() {
        let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(240.0, 180.0));
        let r = place(screen, 1.0, 320, 200);
        assert!(screen.contains_rect(r), "{r:?}");
        assert!((r.width() / r.height() - 1.6).abs() < 0.02, "{r:?}");
        assert!(r.width() >= 239.0, "as big as fits");
    }

    #[test]
    fn a_frame_draws_at_every_size() {
        let data = Data::builtin().unwrap();
        let def = data.demo("ninsei").unwrap();
        for (w, h) in [
            (1280.0, 800.0),
            (800.0, 600.0),
            (330.0, 210.0),
            (200.0, 120.0),
        ] {
            let mut h_ = Harness::new(w, h);
            let mut view = DemoView::default();
            for n in 0..3 {
                h_.frame(vec![], |ui| view.paint(ui, def, f64::from(n) * 1.7));
            }
            let r = view.drawn.unwrap();
            let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(w, h));
            assert!(screen.contains_rect(r), "{w}x{h}: {r:?}");
            assert!(r.width() > 0.0 && r.height() > 0.0);
        }
    }
}
