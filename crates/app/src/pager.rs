//! One page control for every book in the game: Welcome to the Net's cards and CPU Shopper's
//! pages. A book is a fixed-size page with a bar under it: ◀ Home ▶, where you are, and a
//! status on the right. The page never scrolls or changes size, and the bar's buttons never
//! move: at either end they grey out instead of going away. Keys (← → PgUp PgDn Home) work
//! while the pointer is over the book and nothing else has the keyboard.
//!
//! The rules for turning live in core (`ninsei_core::book::turn`); this only draws.

use egui::{Align2, FontId, Rect, Sense, Stroke, Ui, UiBuilder, pos2, vec2};
use ninsei_core::book::{self, Turn};

use crate::home::{ink, muted};

/// Every book's page, in points.
pub const PAGE: egui::Vec2 = vec2(720.0, 520.0);
/// The control bar under the page.
pub const BAR: f32 = 30.0;
const GAP: f32 = 6.0;
/// Room a book takes: the page and its bar. Windows that hold a book are sized to fit it.
pub const SIZE: egui::Vec2 = vec2(PAGE.x, PAGE.y + GAP + BAR);

/// Button sizes and spots on the bar, from its left edge.
const ARROW: f32 = 36.0;
const HOME: f32 = 60.0;
const PAD: f32 = 4.0;

/// What the bar says about the page on show.
pub struct Book<'a> {
    pub at: usize,
    pub count: usize,
    /// Where you are, e.g. "card 3 of 7" or "Decks · p.12 of 880".
    pub label: &'a str,
    /// Right of the bar, e.g. a section's name or the bank.
    pub status: &'a str,
}

/// Draws a book: `page` paints into the page's fixed rectangle (clipped) and may ask for a turn
/// (a link on the page). Returns the page to show from now on.
pub fn show(ui: &mut Ui, b: Book, page: impl FnOnce(&mut Ui, Rect) -> Option<Turn>) -> usize {
    let (rect, resp) = ui.allocate_exact_size(SIZE, Sense::hover());
    let page_rect = Rect::from_min_size(rect.min, PAGE);
    let bar = Rect::from_min_size(pos2(rect.min.x, page_rect.max.y + GAP), vec2(PAGE.x, BAR));

    let mut turn = None;
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(page_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    child.set_clip_rect(page_rect.intersect(ui.clip_rect()));
    if let Some(t) = page(&mut child, page_rect) {
        turn = Some(t);
    }

    // The bar: every control has a fixed spot.
    let h = BAR - 2.0;
    let slot =
        |x: f32, w: f32| Rect::from_min_size(pos2(bar.min.x + x, bar.min.y + 1.0), vec2(w, h));
    let first = b.at == 0;
    let last = b.at + 1 >= b.count;
    let button = |ui: &mut Ui, r: Rect, enabled: bool, text: &str, hint: &str| {
        ui.put(r, |ui: &mut Ui| {
            ui.add_enabled(enabled, egui::Button::new(text).min_size(r.size()))
                .on_hover_text(hint)
        })
        .clicked()
    };
    if button(ui, slot(0.0, ARROW), !first, "◀", "previous (← or PgUp)") {
        turn = Some(Turn::Prev);
    }
    if button(ui, slot(ARROW + PAD, HOME), !first, "Home", "home (Home)") {
        turn = Some(Turn::Home);
    }
    if button(
        ui,
        slot(ARROW + PAD + HOME + PAD, ARROW),
        !last,
        "▶",
        "next (→ or PgDn)",
    ) {
        turn = Some(Turn::Next);
    }
    let font = FontId::monospace(13.0);
    let x = bar.min.x + 2.0 * ARROW + HOME + 4.0 * PAD;
    let p = ui.painter_at(bar);
    p.text(
        pos2(x, bar.center().y),
        Align2::LEFT_CENTER,
        b.label,
        font.clone(),
        muted(),
    );
    p.text(
        bar.right_center(),
        Align2::RIGHT_CENTER,
        b.status,
        font,
        ink(),
    );
    p.hline(
        bar.min.x..=bar.max.x,
        bar.min.y - GAP / 2.0,
        Stroke::new(1.0, muted()),
    );

    // Keys, while the pointer is on the book and nothing else is typing.
    if resp.contains_pointer() && ui.memory(|m| m.focused().is_none()) {
        ui.input(|i| {
            use egui::Key;
            if i.key_pressed(Key::ArrowLeft) || i.key_pressed(Key::PageUp) {
                turn = Some(Turn::Prev);
            } else if i.key_pressed(Key::ArrowRight) || i.key_pressed(Key::PageDown) {
                turn = Some(Turn::Next);
            } else if i.key_pressed(Key::Home) {
                turn = Some(Turn::Home);
            }
        });
    }
    match turn {
        Some(t) => book::turn(b.at, b.count, t),
        None => book::turn(b.at, b.count, Turn::To(b.at)),
    }
}

/// A child ui over part of the page, clipped to it: what each block on a page draws into.
pub fn area(ui: &mut Ui, r: Rect) -> Ui {
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(r)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    child.set_clip_rect(r.intersect(ui.clip_rect()));
    child
}
