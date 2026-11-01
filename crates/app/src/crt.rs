//! CRT overlay: scanlines, a rolling refresh band and a dark vignette, painted over everything.

use egui::{Color32, Context, Id, LayerId, Mesh, Order, Pos2, Rect, pos2};

use crate::theme::Palette;

/// Pixels between scanlines.
const SCANLINE_PITCH: f32 = 3.0;
const SCANLINE_ALPHA: u8 = 55;
/// Height of the bright band that rolls down the screen, and its speed in px/s.
const BAND_HEIGHT: f32 = 120.0;
const BAND_SPEED: f32 = 90.0;
const BAND_ALPHA: f32 = 0.035;
/// How far in from the edges the vignette fades, as a fraction of the screen.
const VIGNETTE_INSET: f32 = 0.12;
const VIGNETTE_ALPHA: u8 = 150;

pub fn paint(ctx: &Context, p: &Palette) {
    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(LayerId::new(Order::Tooltip, Id::new("crt")));

    let line = Color32::from_black_alpha(SCANLINE_ALPHA);
    let mut y = screen.top();
    while y < screen.bottom() {
        painter.rect_filled(
            Rect::from_min_max(pos2(screen.left(), y), pos2(screen.right(), y + 1.0)),
            0.0,
            line,
        );
        y += SCANLINE_PITCH;
    }

    let t = ctx.input(|i| i.time) as f32;
    let travel = screen.height() + BAND_HEIGHT;
    let top = screen.top() - BAND_HEIGHT + (t * BAND_SPEED) % travel;
    let band = Rect::from_min_max(
        pos2(screen.left(), top),
        pos2(screen.right(), top + BAND_HEIGHT),
    );
    let glow = p.bright.gamma_multiply(BAND_ALPHA);
    painter.add(gradient_quad(band, Color32::TRANSPARENT, glow));

    painter.add(vignette(screen));
    ctx.request_repaint();
}

/// A rect fading from `top` colour to `bottom` colour.
fn gradient_quad(r: Rect, top: Color32, bottom: Color32) -> Mesh {
    let mut m = Mesh::default();
    m.colored_vertex(r.left_top(), top);
    m.colored_vertex(r.right_top(), top);
    m.colored_vertex(r.right_bottom(), bottom);
    m.colored_vertex(r.left_bottom(), bottom);
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    m
}

/// Dark frame: opaque-ish at the glass edge, clear toward the middle.
fn vignette(screen: Rect) -> Mesh {
    let inner = screen.shrink2(screen.size() * VIGNETTE_INSET);
    let dark = Color32::from_black_alpha(VIGNETTE_ALPHA);
    let outer: [Pos2; 4] = [
        screen.left_top(),
        screen.right_top(),
        screen.right_bottom(),
        screen.left_bottom(),
    ];
    let inn: [Pos2; 4] = [
        inner.left_top(),
        inner.right_top(),
        inner.right_bottom(),
        inner.left_bottom(),
    ];
    let mut m = Mesh::default();
    for p in outer {
        m.colored_vertex(p, dark);
    }
    for p in inn {
        m.colored_vertex(p, Color32::TRANSPARENT);
    }
    for i in 0..4u32 {
        let j = (i + 1) % 4;
        m.add_triangle(i, j, 4 + j);
        m.add_triangle(i, 4 + j, 4 + i);
    }
    m
}
