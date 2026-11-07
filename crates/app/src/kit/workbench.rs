//! The Workbench kit: the widgets of a desktop after the Amiga's Workbench 1.3. The screen's
//! title bar, windows with their close and depth gadgets, the 1.3 bevel, push buttons and
//! toggle gadgets, and the requester. It draws and reports clicks; what a window shows and what
//! a gadget does are the caller's. The numbers are the spec sheet's (`docs/refs/workbench.md`).
//! Nothing here names a tool or a deck.

use egui::{Align2, Color32, FontId, Painter, Rect, Stroke, StrokeKind, pos2, vec2};

/// Sizes, in one place (the spec sheet's).
pub mod size {
    pub const SCREEN_BAR_H: f32 = 22.0;
    pub const TITLE_H: f32 = 20.0;
    pub const BORDER: f32 = 2.0;
    /// A window's content, in from its border at the sides and bottom.
    pub const PAD: f32 = 4.0;
    /// Under the title bar.
    pub const PAD_TOP: f32 = 4.0;
}

/// The desktop's colours and type, from the theme: 1.3's white, blue, black and orange.
#[derive(Clone)]
pub struct Look {
    pub ink: Color32,
    pub backdrop: Color32,
    pub shadow: Color32,
    pub accent: Color32,
    pub font: FontId,
}

/// The screen's title bar across the top of `screen`: the screen's name at the left, `right`
/// (if any) towards the right, and the screen's depth gadget at the right end.
pub fn screen_bar(p: &Painter, screen: Rect, title: &str, right: Option<&str>, look: &Look) {
    let bar = Rect::from_min_size(screen.min, vec2(screen.width(), size::SCREEN_BAR_H));
    p.rect_filled(bar, 0.0, look.ink);
    p.line_segment(
        [bar.left_bottom(), bar.right_bottom()],
        Stroke::new(1.0, look.shadow),
    );
    p.text(
        bar.left_center() + vec2(8.0, 0.0),
        Align2::LEFT_CENTER,
        title,
        look.font.clone(),
        look.backdrop,
    );
    if let Some(right) = right {
        p.text(
            bar.right_center() - vec2(36.0, 0.0),
            Align2::RIGHT_CENTER,
            right,
            look.font.clone(),
            look.backdrop,
        );
    }
    depth_gadget(
        p,
        Rect::from_min_size(bar.right_top() + vec2(-26.0, 3.0), vec2(22.0, 16.0)),
        look,
    );
}

/// A window's gadgets: where its title bar, close gadget and depth gadget are, so the hit tests
/// use the frame's own geometry.
#[derive(Clone, Copy, Debug)]
pub struct Gadgets {
    pub bar: Rect,
    pub close: Rect,
    pub depth: Rect,
}

/// Where a window's gadgets are in `rect`.
pub fn gadgets(rect: Rect) -> Gadgets {
    let bar = Rect::from_min_size(rect.min, vec2(rect.width(), size::TITLE_H));
    Gadgets {
        bar,
        close: Rect::from_min_size(bar.min + vec2(4.0, 3.0), vec2(16.0, 14.0)),
        depth: Rect::from_min_size(bar.right_top() + vec2(-26.0, 2.0), vec2(22.0, 16.0)),
    }
}

/// Where a window's contents go: inside its border, under the title bar.
pub fn window_content(rect: Rect) -> Rect {
    let g = gadgets(rect);
    let side = size::BORDER + size::PAD;
    Rect::from_min_max(
        g.bar.left_bottom() + vec2(side, size::PAD_TOP),
        rect.max - vec2(side, side),
    )
}

/// A window's frame in `rect`: its border, a title bar (pinstriped when `active`), the title on
/// a solid backing, and the close and depth gadgets. Returns where the gadgets are.
pub fn window_frame(p: &Painter, rect: Rect, title: &str, active: bool, look: &Look) -> Gadgets {
    let g = gadgets(rect);
    let (ink, backdrop) = (look.ink, look.backdrop);
    p.rect_filled(rect, 0.0, backdrop);
    p.rect_stroke(
        rect,
        0.0,
        Stroke::new(size::BORDER, ink),
        StrokeKind::Inside,
    );
    let bar = g.bar;
    p.rect_filled(bar, 0.0, if active { ink } else { backdrop });
    if active {
        // Horizontal pinstripes behind the title, the active-window tell.
        let mut y = bar.top() + 4.0;
        while y < bar.bottom() - 3.0 {
            p.line_segment(
                [pos2(bar.left() + 24.0, y), pos2(bar.right() - 30.0, y)],
                Stroke::new(1.0, backdrop),
            );
            y += 3.0;
        }
    }
    p.rect_stroke(bar, 0.0, Stroke::new(size::BORDER, ink), StrokeKind::Inside);
    let galley = p.layout_no_wrap(
        title.to_string(),
        look.font.clone(),
        if active { backdrop } else { ink },
    );
    let tw = galley.size().x + 10.0;
    let tbox = Rect::from_min_size(bar.min + vec2(26.0, 2.0), vec2(tw, size::TITLE_H - 4.0));
    p.rect_filled(tbox, 0.0, if active { ink } else { backdrop });
    p.galley(tbox.min + vec2(5.0, 1.0), galley, ink);
    // Close gadget: a box with a dot.
    let c = g.close;
    p.rect_filled(c, 0.0, ink);
    p.rect_stroke(c, 0.0, Stroke::new(1.0, look.shadow), StrokeKind::Inside);
    p.rect_filled(
        Rect::from_center_size(c.center(), vec2(4.0, 4.0)),
        0.0,
        look.shadow,
    );
    depth_gadget(p, g.depth, look);
    g
}

/// The depth gadget: two overlapping squares, the back one outlined, the front one filled.
pub fn depth_gadget(p: &Painter, r: Rect, look: &Look) {
    p.rect_filled(r, 0.0, look.ink);
    let back = Rect::from_min_size(r.min + vec2(3.0, 2.0), vec2(11.0, 8.0));
    let front = Rect::from_min_size(r.min + vec2(8.0, 6.0), vec2(11.0, 8.0));
    p.rect_stroke(back, 0.0, Stroke::new(1.0, look.shadow), StrokeKind::Inside);
    p.rect_filled(front, 0.0, look.accent);
    p.rect_stroke(
        front,
        0.0,
        Stroke::new(1.0, look.shadow),
        StrokeKind::Inside,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gadgets_sit_in_the_title_bar_and_the_content_under_it() {
        let win = Rect::from_min_size(pos2(100.0, 50.0), vec2(400.0, 300.0));
        let g = gadgets(win);
        assert!(g.bar.contains_rect(g.close) && g.bar.contains_rect(g.depth));
        assert!(
            g.close.right() < g.depth.left(),
            "close at the left, depth at the right"
        );
        let c = window_content(win);
        assert!(win.contains_rect(c) && c.top() >= g.bar.bottom());
        assert!(!c.intersects(g.close) && !c.intersects(g.depth));
    }
}
