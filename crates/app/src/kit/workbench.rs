//! The Workbench kit: the widgets of a desktop after the Amiga's Workbench 1.3. The screen's
//! title bar, windows with their close and depth gadgets, the proportional gadget a window
//! scrolls with, the 1.3 bevel, push buttons and toggle gadgets, and the requester. It draws
//! and reports clicks; what a window shows and what a gadget does are the caller's. The numbers are the spec sheet's (`docs/refs/workbench.md`).
//! Nothing here names a tool or a deck.

use egui::{Align2, Color32, FontId, Id, Painter, Rect, Response, Sense, Stroke, StrokeKind, Ui};
use egui::{pos2, vec2};

use super::scrollbar::{self, Scroll};

/// Sizes, in one place (the spec sheet's).
pub mod size {
    pub const SCREEN_BAR_H: f32 = 22.0;
    pub const TITLE_H: f32 = 20.0;
    pub const BORDER: f32 = 2.0;
    /// A window's content, in from its border at the sides and bottom.
    pub const PAD: f32 = 4.0;
    /// Under the title bar.
    pub const PAD_TOP: f32 = 4.0;
    /// The bevel's lines.
    pub const BEVEL: f32 = 2.0;
    /// A requester's buttons.
    pub const BUTTON: [f32; 2] = [84.0, 22.0];
    /// A requester's lines, apart.
    pub const LINE: f32 = 18.0;
    /// A toggle gadget's type, and its room around the label.
    pub const GADGET_FONT: f32 = 13.0;
    pub const GADGET_PAD: [f32; 2] = [18.0, 10.0];
    /// The proportional gadget's width, and its knob's shortest.
    pub const PROP_W: f32 = 16.0;
    pub const KNOB_MIN: f32 = 12.0;
    /// The screen bar's name, in from the left; what's on its right, in from the right edge.
    pub const SCREEN_TEXT_IN: f32 = 8.0;
    pub const SCREEN_RIGHT_IN: f32 = 36.0;
    /// A depth gadget's size, and how far from the bar's right edge and top it sits (the
    /// screen's, a window's).
    pub const DEPTH: [f32; 2] = [22.0, 16.0];
    pub const DEPTH_IN: f32 = 26.0;
    pub const SCREEN_DEPTH_DOWN: f32 = 3.0;
    pub const WINDOW_DEPTH_DOWN: f32 = 2.0;
    /// A close gadget's size and place in the title bar, and its dot.
    pub const CLOSE: [f32; 2] = [16.0, 14.0];
    pub const CLOSE_AT: [f32; 2] = [4.0, 3.0];
    pub const CLOSE_DOT: f32 = 4.0;
    /// The title's backing, in from the bar's left, and its room either side of the text.
    pub const TITLE_IN: f32 = 26.0;
    pub const TITLE_PAD: f32 = 5.0;
    /// The active title bar's pinstripes: in from the left and right, from the top, apart.
    pub const STRIPES_IN: [f32; 2] = [24.0, 30.0];
    pub const STRIPES_TOP: f32 = 4.0;
    pub const STRIPES_STEP: f32 = 3.0;
    /// A requester's margins: its first line down from the top, its buttons in from the sides.
    pub const REQUESTER_PAD: f32 = 6.0;
    /// The dithered backdrop: a dot (one hires pixel, 1 across and 2 down), every `DITHER_STEP`
    /// across and down, alternate rows offset by half a step.
    pub const DITHER_DOT: [f32; 2] = [1.0, 2.0];
    pub const DITHER_STEP: [f32; 2] = [6.0, 3.0];
}

/// The desktop's colours and type, from the theme: 1.3's white, blue, black and orange, and
/// the gadgets' faces.
#[derive(Clone)]
pub struct Look {
    pub ink: Color32,
    pub backdrop: Color32,
    pub shadow: Color32,
    pub accent: Color32,
    pub fill: Color32,
    pub font: FontId,
}

/// The screen's title bar across the top of `screen`: the screen's name at the left, `right`
/// (if any) towards the right, and the screen's depth gadget at the right end.
pub fn screen_bar(p: &Painter, screen: Rect, title: &str, right: Option<&str>, look: &Look) {
    let screen = snap(screen);
    let bar = Rect::from_min_size(screen.min, vec2(screen.width(), size::SCREEN_BAR_H));
    p.rect_filled(bar, 0.0, look.ink);
    p.line_segment(
        [bar.left_bottom(), bar.right_bottom()],
        Stroke::new(1.0, look.shadow),
    );
    p.text(
        bar.left_center() + vec2(size::SCREEN_TEXT_IN, 0.0),
        Align2::LEFT_CENTER,
        title,
        look.font.clone(),
        look.backdrop,
    );
    if let Some(right) = right {
        p.text(
            bar.right_center() - vec2(size::SCREEN_RIGHT_IN, 0.0),
            Align2::RIGHT_CENTER,
            right,
            look.font.clone(),
            look.backdrop,
        );
    }
    let depth = bar.right_top() + vec2(-size::DEPTH_IN, size::SCREEN_DEPTH_DOWN);
    depth_gadget(p, Rect::from_min_size(depth, size::DEPTH.into()), look);
}

/// The dithered backdrop over `screen` under its title bar, in the gadgets' face, on whole
/// pixels.
pub fn dither(p: &Painter, screen: Rect, look: &Look) {
    let screen = snap(screen);
    let [dx, dy] = size::DITHER_STEP;
    let dot = egui::Vec2::from(size::DITHER_DOT);
    let mut y = screen.top() + size::SCREEN_BAR_H;
    let mut row = 0;
    while y < screen.bottom() {
        let mut x = screen.left() + if row % 2 == 0 { 0.0 } else { dx / 2.0 };
        while x < screen.right() {
            p.rect_filled(Rect::from_min_size(pos2(x, y), dot), 0.0, look.fill);
            x += dx;
        }
        y += dy;
        row += 1;
    }
}

/// A window's gadgets: where its title bar, close gadget and depth gadget are, so the hit tests
/// use the frame's own geometry.
#[derive(Clone, Copy, Debug)]
pub struct Gadgets {
    pub bar: Rect,
    pub close: Rect,
    pub depth: Rect,
}

/// `r` on whole pixels: thin lines on half pixels blur.
fn snap(r: Rect) -> Rect {
    Rect::from_min_size(r.min.round(), r.size().round())
}

/// Where a window's gadgets are in `rect`.
pub fn gadgets(rect: Rect) -> Gadgets {
    let rect = snap(rect);
    let bar = Rect::from_min_size(rect.min, vec2(rect.width(), size::TITLE_H));
    Gadgets {
        bar,
        close: Rect::from_min_size(
            bar.min + egui::Vec2::from(size::CLOSE_AT),
            size::CLOSE.into(),
        ),
        depth: Rect::from_min_size(
            bar.right_top() + vec2(-size::DEPTH_IN, size::WINDOW_DEPTH_DOWN),
            size::DEPTH.into(),
        ),
    }
}

/// Where a window's contents go: inside its border, under the title bar.
pub fn window_content(rect: Rect) -> Rect {
    let g = gadgets(rect);
    let side = size::BORDER + size::PAD;
    Rect::from_min_max(
        g.bar.left_bottom() + vec2(side, size::PAD_TOP),
        snap(rect).max - vec2(side, side),
    )
}

/// Where a window's scrolled body goes ([`scrolled`]): from the content's left, under the title
/// bar, to inside the border at the bottom right, so the proportional gadget sits flush on it.
pub fn window_scrolled(rect: Rect) -> Rect {
    let g = gadgets(rect);
    Rect::from_min_max(
        g.bar.left_bottom() + vec2(size::BORDER + size::PAD, 0.0),
        snap(rect).max - vec2(size::BORDER, size::BORDER),
    )
}

/// A window's frame in `rect`: its border, a title bar (pinstriped when `active`), the title on
/// a solid backing, and the close and depth gadgets. Returns where the gadgets are.
pub fn window_frame(p: &Painter, rect: Rect, title: &str, active: bool, look: &Look) -> Gadgets {
    let rect = snap(rect);
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
        let [left, right] = size::STRIPES_IN;
        let mut y = bar.top() + size::STRIPES_TOP;
        while y < bar.bottom() - size::STRIPES_STEP {
            p.line_segment(
                [pos2(bar.left() + left, y), pos2(bar.right() - right, y)],
                Stroke::new(1.0, backdrop),
            );
            y += size::STRIPES_STEP;
        }
    }
    p.rect_stroke(bar, 0.0, Stroke::new(size::BORDER, ink), StrokeKind::Inside);
    let galley = p.layout_no_wrap(
        title.to_string(),
        look.font.clone(),
        if active { backdrop } else { ink },
    );
    let tw = galley.size().x + 2.0 * size::TITLE_PAD;
    let tbox = Rect::from_min_size(
        bar.min + vec2(size::TITLE_IN, 2.0),
        vec2(tw.ceil(), size::TITLE_H - 4.0),
    );
    p.rect_filled(tbox, 0.0, if active { ink } else { backdrop });
    p.galley(tbox.min + vec2(size::TITLE_PAD, 1.0), galley, ink);
    // Close gadget: a box with a dot.
    let c = g.close;
    p.rect_filled(c, 0.0, ink);
    p.rect_stroke(c, 0.0, Stroke::new(1.0, look.shadow), StrokeKind::Inside);
    p.rect_filled(
        Rect::from_center_size(c.center(), egui::Vec2::splat(size::CLOSE_DOT)),
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

/// The 1.3 bevel round `r`: light (`ink`) top and left and dark (`shadow`) bottom and right
/// when `raised`, the other way round when pressed in.
pub fn bevel(p: &Painter, r: Rect, raised: bool, look: &Look) {
    let (lt, rb) = if raised {
        (look.ink, look.shadow)
    } else {
        (look.shadow, look.ink)
    };
    let line = |c| Stroke::new(size::BEVEL, c);
    p.line_segment([r.left_top(), r.right_top()], line(lt));
    p.line_segment([r.left_top(), r.left_bottom()], line(lt));
    p.line_segment([r.left_bottom(), r.right_bottom()], line(rb));
    p.line_segment([r.right_top(), r.right_bottom()], line(rb));
}

/// A push button in `r`: a bevelled box with its label, inverted (an `accent` face) while
/// pressed. Clicks count only when `live`.
pub fn button(
    ui: &Ui,
    p: &Painter,
    r: Rect,
    id: Id,
    label: &str,
    live: bool,
    look: &Look,
) -> Response {
    let resp = ui.interact(r, id, if live { Sense::click() } else { Sense::hover() });
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, live, label));
    let pressed = resp.is_pointer_button_down_on();
    let (face, text) = if pressed {
        (look.accent, look.shadow)
    } else {
        (look.backdrop, look.ink)
    };
    p.rect_filled(r, 0.0, face);
    bevel(p, r, !pressed, look);
    p.text(
        r.center(),
        Align2::CENTER_CENTER,
        label,
        look.font.clone(),
        text,
    );
    resp
}

/// A requester in `rect`: its `lines` centred down from the top (each in `accent` when
/// flagged), and two buttons along the bottom, `buttons[0]` at the left and `buttons[1]` at the
/// right. Returns the button clicked.
#[allow(clippy::too_many_arguments)]
pub fn requester(
    ui: &Ui,
    p: &Painter,
    rect: Rect,
    lines: &[(&str, bool)],
    buttons: [&str; 2],
    live: bool,
    id: Id,
    look: &Look,
) -> Option<usize> {
    for (k, (l, lit)) in lines.iter().enumerate() {
        p.text(
            pos2(
                rect.center().x,
                rect.top() + size::REQUESTER_PAD + k as f32 * size::LINE,
            ),
            Align2::CENTER_TOP,
            *l,
            look.font.clone(),
            if *lit { look.accent } else { look.ink },
        );
    }
    let bw = vec2(size::BUTTON[0], size::BUTTON[1]);
    let pad = size::REQUESTER_PAD;
    let left = Rect::from_min_size(pos2(rect.left() + pad, rect.bottom() - bw.y), bw);
    let right = Rect::from_min_size(pos2(rect.right() - pad - bw.x, rect.bottom() - bw.y), bw);
    let mut out = None;
    for (k, r) in [left, right].into_iter().enumerate() {
        if button(ui, p, r, id.with(buttons[k]), buttons[k], live, look).clicked() {
            out = Some(k);
        }
    }
    out
}

/// A toggle gadget laid out in `ui`: a chunky bevelled box round `label`, pressed in (an
/// `accent` face) when `on`. Returns its response; clicked is the caller's to act on.
pub fn toggle_gadget(ui: &mut Ui, label: &str, on: bool, look: &Look) -> Response {
    let font = FontId::new(size::GADGET_FONT, look.font.family.clone());
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font, look.ink);
    let want = galley.size() + vec2(size::GADGET_PAD[0], size::GADGET_PAD[1]);
    let (r, resp) = ui.allocate_exact_size(want, Sense::click());
    resp.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, on, label)
    });
    let p = ui.painter();
    p.rect_filled(r, 0.0, if on { look.accent } else { look.fill });
    bevel(p, r, !on, look);
    let text = if on { look.shadow } else { look.ink };
    p.galley_with_override_text_color(r.center() - galley.size() / 2.0, galley, text);
    resp
}

/// The knob of a proportional gadget in `r` for `s`: as long as the view's share of the whole.
fn knob(r: Rect, s: &Scroll) -> Option<Rect> {
    let share = (s.view / s.content.max(1.0)).clamp(0.0, 1.0);
    let len = (r.height() * share).max(size::KNOB_MIN).round();
    scrollbar::thumb(r, s, len)
}

/// What the proportional gadget in `r` does to `s` this frame: a click above or below the knob
/// scrolls a page, the knob drags the view, and the wheel over it scrolls it. Returns the new
/// offset when it changed.
fn prop_input(ui: &Ui, r: Rect, id: Id, s: &Scroll) -> Option<f32> {
    let mut to = s.offset;
    scrollbar::track_input(ui, r, knob(r, s), id, s, &mut to);
    if let Some(wheel) = scrollbar::take_wheel(ui, r, id) {
        to -= wheel;
    }
    s.settle(to)
}

/// Paints the proportional gadget in `r` for `s`: the container in the gadgets' face with a dark
/// edge on its left, and the bevelled knob.
fn prop_paint(p: &Painter, r: Rect, s: &Scroll, look: &Look) {
    p.rect_filled(r, 0.0, look.fill);
    p.rect_filled(
        Rect::from_min_size(r.min, vec2(1.0, r.height())),
        0.0,
        look.shadow,
    );
    if let Some(k) = knob(r, s) {
        let k = k.shrink2(vec2(2.0, 1.0));
        p.rect_filled(k, 0.0, look.backdrop);
        bevel(p, k, true, look);
    }
}

/// `body` in a view that scrolls, filling `ui` (a window's [`window_scrolled`]), with the
/// proportional gadget flush on the right while there's more than shows.
pub fn scrolled(ui: &mut Ui, id: Id, look: &Look, body: impl FnOnce(&mut Ui)) {
    let line = ui.fonts_mut(|f| f.row_height(&look.font)).ceil();
    let pad = SCROLL_PAD;
    scrollbar::view(
        ui,
        id,
        line,
        size::PROP_W,
        pad,
        body,
        prop_input,
        |p, bar, s| prop_paint(p, bar, s, look),
    );
}

/// The room [`scrolled`] keeps round its view: above and below it, and before the bar.
pub const SCROLL_PAD: scrollbar::Pad = scrollbar::Pad {
    top: size::PAD_TOP,
    gap: size::PAD,
    bottom: size::PAD,
};

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

    fn look() -> Look {
        Look {
            ink: Color32::WHITE,
            backdrop: Color32::BLUE,
            shadow: Color32::BLACK,
            accent: Color32::from_rgb(255, 136, 0),
            fill: Color32::GRAY,
            font: FontId::monospace(14.0),
        }
    }

    #[test]
    fn the_bevel_is_light_top_left_when_raised_and_swaps_when_pressed() {
        let mut h = crate::uitest::Harness::new(200.0, 100.0);
        let r = Rect::from_min_size(pos2(10.0, 10.0), vec2(80.0, 22.0));
        let lines = |h: &crate::uitest::Harness| -> Vec<Color32> {
            h.painted()
                .iter()
                .filter_map(|s| match s {
                    egui::Shape::LineSegment { stroke, .. } => Some(stroke.color),
                    _ => None,
                })
                .collect()
        };
        h.frame(vec![], |ui| bevel(ui.painter(), r, true, &look()));
        assert_eq!(
            lines(&h),
            [
                Color32::WHITE,
                Color32::WHITE,
                Color32::BLACK,
                Color32::BLACK
            ]
        );
        h.frame(vec![], |ui| bevel(ui.painter(), r, false, &look()));
        assert_eq!(
            lines(&h),
            [
                Color32::BLACK,
                Color32::BLACK,
                Color32::WHITE,
                Color32::WHITE
            ]
        );
    }

    #[test]
    fn a_requester_answers_its_buttons_only_when_live() {
        let mut h = crate::uitest::Harness::new(400.0, 300.0);
        let rect = Rect::from_min_size(pos2(20.0, 20.0), vec2(320.0, 120.0));
        let ask = |h: &mut crate::uitest::Harness, live, ev| {
            let mut got = None;
            h.frame(ev, |ui| {
                got = requester(
                    ui,
                    ui.painter(),
                    rect,
                    &[("Please insert volume", false), ("Tools:", true)],
                    ["Retry", "Cancel"],
                    live,
                    Id::new("req"),
                    &look(),
                );
            });
            got
        };
        ask(&mut h, true, vec![]);
        let (retry, cancel) = (h.find("Retry").unwrap(), h.find("Cancel").unwrap());
        assert!(retry.right() < cancel.left(), "Retry left, Cancel right");
        assert_eq!(
            ask(&mut h, true, crate::uitest::click(retry.center())),
            Some(0)
        );
        assert_eq!(
            ask(&mut h, true, crate::uitest::click(cancel.center())),
            Some(1)
        );
        assert_eq!(
            ask(&mut h, false, crate::uitest::click(cancel.center())),
            None
        );
    }

    #[test]
    fn a_toggle_gadget_is_pressed_in_when_on() {
        let mut h = crate::uitest::Harness::new(400.0, 100.0);
        let mut clicked = false;
        h.frame(vec![], |ui| {
            toggle_gadget(ui, "Monitor", true, &look());
        });
        let face = h.rects().into_iter().next().unwrap();
        assert_eq!(face.1, look().accent, "on: the accent face");
        let at = h.find("Monitor").unwrap().center();
        h.frame(crate::uitest::click(at), |ui| {
            clicked = toggle_gadget(ui, "Monitor", false, &look()).clicked();
        });
        assert!(clicked);
    }

    #[test]
    fn the_knob_is_the_views_share_and_never_too_small() {
        let r = Rect::from_min_size(pos2(0.0, 0.0), vec2(16.0, 200.0));
        let s = |view, content| Scroll {
            offset: 0.0,
            view,
            content,
            line: 17.0,
        };
        assert_eq!(knob(r, &s(100.0, 400.0)).unwrap().height(), 50.0);
        assert_eq!(
            knob(r, &s(100.0, 100_000.0)).unwrap().height(),
            size::KNOB_MIN
        );
        assert!(
            knob(r, &s(100.0, 90.0)).is_none(),
            "no knob when it all shows"
        );
    }

    #[test]
    fn a_window_dragged_to_half_a_pixel_lands_on_whole_pixels() {
        let mut h = crate::uitest::Harness::new(640.0, 400.0);
        let win = Rect::from_min_size(pos2(40.4, 60.6), vec2(300.3, 200.0));
        h.frame(vec![], |ui| {
            window_frame(ui.painter(), win, "Untitled", true, &look());
        });
        for (r, _, _) in h.rects() {
            assert_eq!(r.min, r.min.round(), "{r:?} is on a half pixel");
            assert_eq!(r.max, r.max.round(), "{r:?} is on a half pixel");
        }
    }

    #[test]
    fn the_screen_bar_and_the_dither_land_on_whole_pixels() {
        let mut h = crate::uitest::Harness::new(640.0, 400.0);
        // Dragged off a run by a fractional offset.
        let screen = Rect::from_min_size(pos2(0.0, 30.4), vec2(640.0, 300.0));
        h.frame(vec![], |ui| {
            screen_bar(ui.painter(), screen, "Workbench", Some("12:00"), &look());
            dither(ui.painter(), screen, &look());
        });
        for (r, _, _) in h.rects() {
            assert_eq!(r.min, r.min.round(), "{r:?} is on a half pixel");
            assert_eq!(r.max, r.max.round(), "{r:?} is on a half pixel");
        }
        for s in h.painted() {
            if let egui::Shape::LineSegment { points, .. } = s {
                assert_eq!(
                    points[0].y,
                    points[0].y.round(),
                    "the rule is on a half pixel"
                );
            }
        }
    }
}
