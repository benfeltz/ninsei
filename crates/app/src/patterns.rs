//! Draws the 1-bit patterns (`data/patterns.ron`) and 1-bit pixel art in two theme colours.
//! Each pattern and colour pair is one 8x8 texture, tiled with nearest sampling and anchored to
//! the screen's origin, so neighbouring shapes line up and nothing blurs. The textures live in
//! egui's memory, so every part that shades with patterns shares them.

use std::collections::HashMap;

use egui::{Color32, ColorImage, Context, Id, Painter, Pos2, Rect, TextureHandle, TextureOptions};
use egui::{pos2, vec2};
use ninsei_core::pattern::{PatternDef, SIZE};

/// Screen points per pattern pixel, before rounding to whole physical pixels.
pub const SCALE: f32 = 2.0;

#[derive(Clone, Default)]
struct Cache(HashMap<(String, Color32, Color32), TextureHandle>);

fn cache_id() -> Id {
    Id::new("pattern-textures")
}

/// One pattern pixel, in points, as a whole number of physical pixels.
pub fn pixel(ctx: &Context) -> f32 {
    let ppp = ctx.pixels_per_point();
    (SCALE * ppp).round().max(1.0) / ppp
}

fn texture(ctx: &Context, def: &PatternDef, ink: Color32, paper: Color32) -> TextureHandle {
    let key = (def.id.clone(), ink, paper);
    if let Some(t) = ctx.data(|d| {
        d.get_temp::<Cache>(cache_id())
            .and_then(|c| c.0.get(&key).cloned())
    }) {
        return t;
    }
    let pixels = (0..SIZE * SIZE)
        .map(|i| {
            if def.ink(i % SIZE, i / SIZE) {
                ink
            } else {
                paper
            }
        })
        .collect();
    let tex = ctx.load_texture(
        format!("pattern-{}", def.id),
        ColorImage::new([SIZE, SIZE], pixels),
        TextureOptions::NEAREST_REPEAT,
    );
    ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Cache>(cache_id())
            .0
            .insert(key, tex.clone());
    });
    tex
}

/// Fills `rect` with `def`, ink on paper.
pub fn fill(painter: &Painter, def: &PatternDef, rect: Rect, ink: Color32, paper: Color32) {
    let ctx = painter.ctx();
    let tex = texture(ctx, def, ink, paper);
    let tile = pixel(ctx) * SIZE as f32;
    // Anchored to the screen, so patterns in shapes side by side meet seamlessly.
    let uv = Rect::from_min_max(
        pos2(rect.min.x / tile, rect.min.y / tile),
        pos2(rect.max.x / tile, rect.max.y / tile),
    );
    painter.image(tex.id(), rect, uv, Color32::WHITE);
}

/// Paints 1-bit pixel art (rows of `#` ink and `.` paper) with its top-left at `at`, each
/// art pixel `scale` points wide.
pub fn bitmap(
    painter: &Painter,
    rows: &[String],
    at: Pos2,
    scale: f32,
    ink: Color32,
    paper: Color32,
) {
    let w = rows.first().map_or(0, |r| r.chars().count());
    painter.rect_filled(
        Rect::from_min_size(at, vec2(w as f32, rows.len() as f32) * scale),
        0.0,
        paper,
    );
    for (y, row) in rows.iter().enumerate() {
        // One rect per run of ink, so a picture is a handful of shapes, not one per pixel.
        let mut x = 0;
        let cells: Vec<char> = row.chars().collect();
        while x < cells.len() {
            if cells[x] != '#' {
                x += 1;
                continue;
            }
            let start = x;
            while x < cells.len() && cells[x] == '#' {
                x += 1;
            }
            painter.rect_filled(
                Rect::from_min_size(
                    at + vec2(start as f32, y as f32) * scale,
                    vec2((x - start) as f32, 1.0) * scale,
                ),
                0.0,
                ink,
            );
        }
    }
}

/// The size of `rows` drawn at `scale`.
pub fn bitmap_size(rows: &[String], scale: f32) -> egui::Vec2 {
    let w = rows.first().map_or(0, |r| r.chars().count());
    vec2(w as f32, rows.len() as f32) * scale
}
