//! The Liza kit: the widgets of a 1-bit desktop after the first Mac's System 1. Windows with a
//! striped title bar and a close box, the menu bar and its menus. It draws and reports clicks
//! and picks; what the menus hold and what a window shows are the caller's. The numbers are the
//! spec sheet's (`docs/refs/liza.md`). Nothing here names a tool or a deck.

use egui::{Align2, Color32, FontId, Id, Painter, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};
use egui::{pos2, vec2};
use ninsei_core::pattern::PatternDef;

use crate::patterns;

/// Sizes, in one place (the spec sheet's).
pub mod size {
    pub const MENU_H: f32 = 22.0;
    pub const MENU_ITEM_H: f32 = 22.0;
    pub const MENU_W: f32 = 190.0;
    /// Where the first menu title starts: the mark has the bar's left end.
    pub const MENU_LEFT: f32 = 40.0;
    /// A menu title's width past its text.
    pub const MENU_PAD: f32 = 20.0;
    pub const TITLE_H: f32 = 20.0;
    pub const CLOSE_BOX: f32 = 12.0;
    /// The 1-bit drop shadow under windows and menus.
    pub const SHADOW: f32 = 2.0;
}

/// Liza's two colours and her type, from the theme, and the patterns she fills with.
#[derive(Clone)]
pub struct Look<'a> {
    pub ink: Color32,
    pub paper: Color32,
    pub font: FontId,
    /// The front window's title bar.
    pub stripes: Option<&'a PatternDef>,
}

/// A box with the 1-bit drop shadow: paper in an ink frame.
pub fn shadowed(p: &Painter, r: Rect, look: &Look) {
    p.rect_filled(r.translate(Vec2::splat(size::SHADOW)), 0.0, look.ink);
    p.rect_filled(r, 0.0, look.paper);
    p.rect_stroke(r, 0.0, Stroke::new(1.0, look.ink), StrokeKind::Inside);
}

/// A window's frame in `rect`: the shadowed box, a title bar striped when it's in `front`, the
/// title on a solid backing, and a close box (drawn only in front). Returns the close box.
pub fn window_frame(p: &Painter, rect: Rect, title: &str, front: bool, look: &Look) -> Rect {
    let (ink, paper) = (look.ink, look.paper);
    shadowed(p, rect, look);
    let bar = title_bar(rect);
    p.line_segment(
        [bar.left_bottom(), bar.right_bottom()],
        Stroke::new(1.0, ink),
    );
    if front && let Some(s) = look.stripes {
        patterns::fill(p, s, bar.shrink2(vec2(3.0, 4.0)), ink, paper);
    }
    let close = Rect::from_min_size(
        bar.left_center() + vec2(10.0, -size::CLOSE_BOX / 2.0),
        Vec2::splat(size::CLOSE_BOX),
    );
    if front {
        p.rect_filled(close.expand(2.0), 0.0, paper);
        p.rect_stroke(close, 0.0, Stroke::new(1.0, ink), StrokeKind::Inside);
    }
    let galley = p.layout_no_wrap(title.to_string(), look.font.clone(), ink);
    let tbox = Rect::from_center_size(bar.center(), galley.size() + vec2(16.0, 2.0));
    p.rect_filled(tbox, 0.0, paper);
    p.galley(tbox.min + vec2(8.0, 1.0), galley, ink);
    close
}

/// A window's title bar: where it's dragged from.
pub fn title_bar(rect: Rect) -> Rect {
    Rect::from_min_size(rect.min, vec2(rect.width(), size::TITLE_H))
}

/// Where a window's contents go: inside its frame, under the title bar.
pub fn window_content(rect: Rect) -> Rect {
    Rect::from_min_max(
        title_bar(rect).left_bottom() + vec2(6.0, 4.0),
        rect.max - vec2(6.0, 4.0),
    )
}

/// A menu on the bar: its title and the items it shows.
pub struct Menu<'a> {
    pub title: &'a str,
    pub items: Vec<&'a str>,
}

/// The menu bar across the top of `screen`, and the menu `open` dropped from it. A click on a
/// title opens or closes its menu, sliding along the bar with one open opens the next, and a
/// click on an item closes the menu and returns `(menu, item)`. `right` sits at the bar's right
/// end. The bar's left end, before [`size::MENU_LEFT`], is left for the caller's mark. Clicks
/// count only when `live`.
#[allow(clippy::too_many_arguments)]
pub fn menu_bar(
    ui: &Ui,
    p: &Painter,
    screen: Rect,
    menus: &[Menu],
    right: &str,
    open: &mut Option<usize>,
    look: &Look,
    live: bool,
) -> Option<(usize, usize)> {
    let (ink, paper) = (look.ink, look.paper);
    let sense = if live { Sense::click() } else { Sense::hover() };
    let bar = menu_bar_rect(screen);
    p.rect_filled(bar, 0.0, paper);
    p.line_segment(
        [bar.left_bottom(), bar.right_bottom()],
        Stroke::new(1.0, ink),
    );
    let mut picked = None;
    let mut x = bar.left() + size::MENU_LEFT;
    for (m, menu) in menus.iter().enumerate() {
        let w = p
            .layout_no_wrap(menu.title.to_string(), look.font.clone(), ink)
            .size()
            .x
            + size::MENU_PAD;
        let r = Rect::from_min_size(pos2(x, bar.top()), vec2(w, size::MENU_H - 1.0));
        let is_open = *open == Some(m);
        if is_open {
            p.rect_filled(r, 0.0, ink);
        }
        p.text(
            r.center(),
            Align2::CENTER_CENTER,
            menu.title,
            look.font.clone(),
            if is_open { paper } else { ink },
        );
        let resp = ui.interact(r, Id::new(("liza-menu", m)), sense);
        if resp.clicked() {
            *open = if is_open { None } else { Some(m) };
        } else if open.is_some() && resp.hovered() {
            *open = Some(m);
        }
        if *open == Some(m) {
            let drop = Rect::from_min_size(
                pos2(r.left(), bar.bottom()),
                vec2(
                    size::MENU_W,
                    menu.items.len() as f32 * size::MENU_ITEM_H + 8.0,
                ),
            );
            shadowed(p, drop, look);
            for (k, label) in menu.items.iter().enumerate() {
                let row = Rect::from_min_size(
                    drop.min + vec2(1.0, 4.0 + k as f32 * size::MENU_ITEM_H),
                    vec2(drop.width() - 2.0, size::MENU_ITEM_H),
                );
                let hit = ui.interact(row, Id::new(("liza-item", m, k)), sense);
                let lit = hit.hovered();
                if lit {
                    p.rect_filled(row, 0.0, ink);
                }
                p.text(
                    row.left_center() + vec2(14.0, 0.0),
                    Align2::LEFT_CENTER,
                    *label,
                    look.font.clone(),
                    if lit { paper } else { ink },
                );
                if hit.clicked() {
                    picked = Some((m, k));
                    *open = None;
                }
            }
        }
        x += w;
    }
    p.text(
        bar.right_center() - vec2(12.0, 0.0),
        Align2::RIGHT_CENTER,
        right,
        look.font.clone(),
        ink,
    );
    picked
}

/// The menu bar's rect across the top of `screen`.
pub fn menu_bar_rect(screen: Rect) -> Rect {
    Rect::from_min_size(screen.min, vec2(screen.width(), size::MENU_H))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::uitest::{Harness, click};
    use egui::Pos2;

    fn look() -> Look<'static> {
        Look {
            ink: Color32::WHITE,
            paper: Color32::BLACK,
            font: FontId::monospace(14.0),
            stripes: None,
        }
    }

    /// One frame of a bar with two menus; returns what was picked.
    fn bar(
        h: &mut Harness,
        open: &mut Option<usize>,
        live: bool,
        events: Vec<egui::Event>,
    ) -> Option<(usize, usize)> {
        let mut picked = None;
        h.frame(events, |ui| {
            let menus = [
                Menu {
                    title: "File",
                    items: vec!["Open", "Close"],
                },
                Menu {
                    title: "Edit",
                    items: vec!["Cut"],
                },
            ];
            let screen = ui.max_rect();
            picked = menu_bar(ui, ui.painter(), screen, &menus, "", open, &look(), live);
        });
        picked
    }

    /// The middle of the `m`th title (File is 4 letters wide, so Edit starts after it).
    fn title_at(h: &Harness, m: usize) -> Pos2 {
        let w = h.ctx.fonts_mut(|f| {
            f.layout_no_wrap("File".into(), FontId::monospace(14.0), Color32::WHITE)
                .size()
                .x
        }) + size::MENU_PAD;
        pos2(size::MENU_LEFT + w * m as f32 + 8.0, size::MENU_H / 2.0)
    }

    #[test]
    fn a_title_opens_its_menu_and_an_item_closes_it() {
        let mut h = Harness::new(640.0, 400.0);
        let mut open = None;
        bar(&mut h, &mut open, true, vec![]);
        let t0 = title_at(&h, 0);
        bar(&mut h, &mut open, true, click(t0));
        assert_eq!(open, Some(0));
        // The second item: 4px into the menu, a row down.
        let at = pos2(
            size::MENU_LEFT + 20.0,
            size::MENU_H + 4.0 + size::MENU_ITEM_H * 1.5,
        );
        bar(&mut h, &mut open, true, vec![egui::Event::PointerMoved(at)]);
        assert_eq!(bar(&mut h, &mut open, true, click(at)), Some((0, 1)));
        assert_eq!(open, None, "picking closes it");
        // A title clicked again closes its menu.
        bar(&mut h, &mut open, true, click(t0));
        bar(&mut h, &mut open, true, click(t0));
        assert_eq!(open, None);
    }

    #[test]
    fn sliding_along_the_bar_opens_the_next_menu() {
        let mut h = Harness::new(640.0, 400.0);
        let mut open = None;
        bar(&mut h, &mut open, true, vec![]);
        let (t0, t1) = (title_at(&h, 0), title_at(&h, 1));
        bar(&mut h, &mut open, true, click(t0));
        bar(&mut h, &mut open, true, vec![egui::Event::PointerMoved(t1)]);
        assert_eq!(open, Some(1));
    }

    #[test]
    fn a_bar_that_isnt_live_takes_no_clicks() {
        let mut h = Harness::new(640.0, 400.0);
        let mut open = None;
        bar(&mut h, &mut open, false, vec![]);
        let t0 = title_at(&h, 0);
        bar(&mut h, &mut open, false, click(t0));
        assert_eq!(open, None);
    }

    #[test]
    fn the_close_box_is_in_the_title_bar_on_whole_pixels() {
        let mut h = Harness::new(640.0, 400.0);
        let mut close = Rect::NOTHING;
        let win = Rect::from_min_size(pos2(40.0, 60.0), vec2(300.0, 200.0));
        h.frame(vec![], |ui| {
            close = window_frame(ui.painter(), win, "Untitled", true, &look())
        });
        assert!(title_bar(win).contains_rect(close));
        assert_eq!(close.size(), Vec2::splat(size::CLOSE_BOX));
        assert_eq!(close.min, close.min.round(), "on whole pixels");
        let content = window_content(win);
        assert!(content.top() > title_bar(win).bottom() && win.contains_rect(content));
    }
}
