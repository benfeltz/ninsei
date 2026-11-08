//! The Workbench kit: the widgets of a desktop after the Amiga's Workbench 1.3. The screen's
//! title bar, windows with their close and depth gadgets, the proportional gadget a window
//! scrolls with, the 1.3 bevel, push buttons and toggle gadgets, and the requester. It draws
//! and reports clicks; what a window shows and what a gadget does are the caller's. The numbers are the spec sheet's (`docs/refs/workbench.md`).
//! Nothing here names a tool or a deck.

use egui::{Align2, Color32, FontId, Id, Painter, Rect, Response, Sense, Stroke, StrokeKind, Ui};
use egui::{pos2, vec2};

use super::List;
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
    /// A horizontal proportional gadget's height and its knob's width.
    pub const HPROP_H: f32 = 14.0;
    pub const HPROP_KNOB: f32 = 24.0;
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
    /// A laid-out push button's room either side of its label.
    pub const BUTTON_PAD: f32 = 12.0;
    /// A check box: its box (a hires square, twice as wide as a pixel is tall) and the gap to
    /// its label.
    pub const CHECK: [f32; 2] = [16.0, 14.0];
    pub const CHECK_GAP: f32 = 6.0;
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

/// A push button laid out in `ui`: as wide as its label and its room, a requester button's
/// height. Disabled when `ui` is.
pub fn push_button(ui: &mut Ui, label: &str, look: &Look) -> Response {
    let text = ui
        .painter()
        .layout_no_wrap(label.to_string(), look.font.clone(), look.ink)
        .size()
        .x;
    let want = vec2((text + 2.0 * size::BUTTON_PAD).ceil(), size::BUTTON[1]);
    let (r, _) = ui.allocate_exact_size(want, Sense::hover());
    let id = ui.id().with(("push", label));
    let live = ui.is_enabled();
    button(ui, ui.painter(), r, id, label, live, look)
}

/// A check box laid out in `ui`: a bevelled box, pressed in with a check in it when on, and its
/// label after. A click flips `on`, and the response says it changed.
pub fn check_box(ui: &mut Ui, on: &mut bool, label: &str, look: &Look) -> Response {
    let g = ui
        .painter()
        .layout_no_wrap(label.to_string(), look.font.clone(), look.ink);
    let [bw, bh] = size::CHECK;
    let want = vec2(bw + size::CHECK_GAP + g.size().x, g.size().y.max(bh)).ceil();
    let (r, mut resp) = ui.allocate_exact_size(want, Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let (enabled, now) = (ui.is_enabled(), *on);
    resp.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, enabled, now, label)
    });
    let p = ui.painter();
    let b = Rect::from_min_size(
        pos2(r.left(), r.center().y - bh / 2.0).round(),
        vec2(bw, bh),
    );
    p.rect_filled(b, 0.0, if now { look.accent } else { look.fill });
    bevel(p, b, !now, look);
    if now {
        // The check: a short stroke down, a long one up.
        let c = |x: f32, y: f32| pos2(b.left() + x * bw, b.top() + y * bh);
        let tick = Stroke::new(2.0, look.shadow);
        p.line_segment([c(0.25, 0.5), c(0.45, 0.75)], tick);
        p.line_segment([c(0.45, 0.75), c(0.8, 0.25)], tick);
    }
    let at = pos2(b.right() + size::CHECK_GAP, r.center().y - g.size().y / 2.0).round();
    p.galley(at, g, look.ink);
    resp
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
    let enabled = ui.is_enabled();
    resp.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, enabled, on, label)
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
/// edge on its left, and the bevelled knob in the text colour.
fn prop_paint(p: &Painter, r: Rect, s: &Scroll, look: &Look) {
    p.rect_filled(r, 0.0, look.fill);
    p.rect_filled(
        Rect::from_min_size(r.min, vec2(1.0, r.height())),
        0.0,
        look.shadow,
    );
    if let Some(k) = knob(r, s) {
        // In the text colour, so it stands out from the container on every theme (on a mono
        // tube `backdrop` and `fill` can be all but one shade).
        let k = k.shrink2(vec2(2.0, 1.0));
        p.rect_filled(k, 0.0, look.ink);
        bevel(p, k, true, look);
    }
}

/// A horizontal proportional gadget's knob at `value` (0-1) in `r`.
fn hknob(r: Rect, value: f32) -> Rect {
    let travel = (r.width() - size::HPROP_KNOB).max(0.0);
    let x = (r.left() + travel * value.clamp(0.0, 1.0)).round();
    Rect::from_min_size(pos2(x, r.top()), vec2(size::HPROP_KNOB, r.height()))
}

/// A horizontal proportional gadget used to set a value, in `r` ([`size::HPROP_H`] tall): the
/// container in the gadgets' face with a dark edge along its top, and the bevelled knob at
/// `value` (0-1). A click puts the knob under the pointer; a drag carries it and lets go of it
/// where it's released. Returns the new value when it was set: on a click or a release, never
/// while the knob is still held.
pub fn hprop(ui: &Ui, r: Rect, id: Id, value: f32, look: &Look) -> Option<f32> {
    let r = Rect::from_min_size(r.min.round(), r.size().round());
    let t = ui.interact(r, id, Sense::click_and_drag());
    let under = t.interact_pointer_pos().map(|p| {
        let travel = (r.width() - size::HPROP_KNOB).max(1.0);
        ((p.x - r.left() - size::HPROP_KNOB / 2.0) / travel).clamp(0.0, 1.0)
    });
    let grab = id.with("held");
    let held = t.dragged().then_some(under).flatten();
    if let Some(v) = held {
        ui.data_mut(|d| d.insert_temp(grab, v));
    }
    let to = if t.drag_stopped() {
        ui.data_mut(|d| d.remove_temp::<f32>(grab)).or(under)
    } else if t.clicked() {
        under
    } else {
        None
    };
    let to = to.filter(|v| *v != value);
    let p = ui.painter();
    p.rect_filled(r, 0.0, look.fill);
    p.rect_filled(
        Rect::from_min_size(r.min, vec2(r.width(), 1.0)),
        0.0,
        look.shadow,
    );
    let k = hknob(r, held.or(to).unwrap_or(value)).shrink2(vec2(1.0, 2.0));
    p.rect_filled(k, 0.0, look.ink);
    bevel(p, k, true, look);
    to
}

/// A list `size` big: `n` rows `row_h` tall in a recessed box, the `sel`ected one drawn by
/// `cell` as on (it returns the row's name, for the accessibility tree), and the proportional
/// gadget down its right side once the rows run past the box, scrolling a whole row at a time.
/// Only whole rows are drawn. A click on a row picks it; returns the row picked.
#[allow(clippy::too_many_arguments)]
pub fn list_box(
    ui: &mut Ui,
    size: egui::Vec2,
    id: Id,
    n: usize,
    row_h: f32,
    sel: usize,
    look: &Look,
    mut cell: impl FnMut(&Painter, Rect, usize, bool) -> String,
) -> Option<usize> {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let rect = Rect::from_min_size(rect.min.round(), rect.size().round());
    let inner = rect.shrink(size::BEVEL);
    let rows = ((inner.height() / row_h).floor() as usize).max(1);
    let key = id.with("list");
    let mut list: List = ui.data(|d| d.get_temp(key)).unwrap_or_default();
    let follow = (ui.data(|d| d.get_temp::<usize>(key.with("sel"))) != Some(sel)).then_some(sel);
    list.view(n, follow, rows);
    let s = Scroll {
        offset: list.top as f32 * row_h,
        view: rows as f32 * row_h,
        content: n as f32 * row_h,
        line: row_h,
    };
    let bar = Rect::from_min_max(pos2(inner.right() - size::PROP_W, inner.top()), inner.max);
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 0.0, look.backdrop);
    let cells_w = if s.active() {
        if let Some(o) = prop_input(ui, bar, id.with("bar"), &s) {
            list.top = (o / row_h).round() as usize;
        }
        prop_paint(
            &p,
            bar,
            &Scroll {
                offset: list.top as f32 * row_h,
                ..s
            },
            look,
        );
        inner.width() - size::PROP_W
    } else {
        inner.width()
    };
    // Every row's click first, so the frame a row is picked paints (and reports) one cursor.
    let shown: Vec<(usize, Rect, Response)> = list
        .view(n, None, rows)
        .map(|i| {
            let r = Rect::from_min_size(
                inner.min + vec2(0.0, list.row(i) as f32 * row_h),
                vec2(cells_w, row_h),
            );
            (i, r, ui.interact(r, id.with(i), Sense::click()))
        })
        .collect();
    let picked = shown
        .iter()
        .find(|(_, _, hit)| hit.clicked())
        .map(|(i, _, _)| *i);
    let at = picked.unwrap_or(sel);
    for (i, r, hit) in shown {
        let name = cell(&p, r, i, i == at);
        hit.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, i == at, &name)
        });
    }
    // The wheel over the rows scrolls them too, for the next frame.
    if s.active()
        && let Some(wheel) = scrollbar::take_wheel(ui, inner.with_max_x(bar.left()), id)
    {
        list.wheel(wheel / row_h);
        ui.ctx().request_repaint();
    }
    bevel(&p, rect, false, look);
    ui.data_mut(|d| {
        d.insert_temp(key, list);
        d.insert_temp(key.with("sel"), picked.unwrap_or(sel));
    });
    picked
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

    #[test]
    fn the_knob_stands_out_from_its_container() {
        let mut h = crate::uitest::Harness::new(100.0, 300.0);
        let r = Rect::from_min_size(pos2(10.0, 10.0), vec2(size::PROP_W, 200.0));
        let s = Scroll {
            offset: 0.0,
            view: 100.0,
            content: 400.0,
            line: 17.0,
        };
        h.frame(vec![], |ui| prop_paint(ui.painter(), r, &s, &look()));
        let fills: Vec<Color32> = h.rects().into_iter().map(|(_, f, _)| f).collect();
        assert_eq!(fills.first(), Some(&look().fill), "the container");
        assert_eq!(
            fills.last(),
            Some(&look().ink),
            "the knob, in the text colour"
        );
    }

    fn hp(h: &mut crate::uitest::Harness, value: f32, events: Vec<egui::Event>) -> Option<f32> {
        let mut out = None;
        h.frame(events, |ui| {
            let r = Rect::from_min_size(pos2(10.0, 10.0), vec2(224.0, 14.0));
            out = hprop(ui, r, Id::new("hp"), value, &look());
        });
        out
    }

    #[test]
    fn a_proportional_slider_sets_a_value_on_a_click_and_on_release() {
        use egui::{Event, PointerButton};
        let mut h = crate::uitest::Harness::new(400.0, 200.0);
        assert_eq!(hp(&mut h, 0.25, Vec::new()), None);
        // The knob is 24 wide, so it travels 200: its centre at 10 + 12 + 100 is the middle.
        let mid = egui::Pos2::new(122.0, 17.0);
        let got = hp(&mut h, 0.25, crate::uitest::click(mid)).unwrap();
        assert!((got - 0.5).abs() < 0.01, "{got}");
        // Held, nothing is set; let go, it is.
        let press = |pos, pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let a = egui::Pos2::new(40.0, 17.0);
        hp(&mut h, 0.0, vec![Event::PointerMoved(a)]);
        hp(&mut h, 0.0, vec![press(a, true)]);
        for x in [60.0, 100.0, 172.0, 172.0] {
            let got = hp(
                &mut h,
                0.0,
                vec![Event::PointerMoved(egui::Pos2::new(x, 17.0))],
            );
            assert_eq!(got, None, "held at {x}");
        }
        let got = hp(
            &mut h,
            0.0,
            vec![press(egui::Pos2::new(172.0, 17.0), false)],
        )
        .unwrap();
        assert!((got - 0.75).abs() < 0.01, "{got}");
    }

    /// One frame of a 300 x `h` list of `n` rows 20 tall; the row picked and the rows drawn.
    fn list(
        hn: &mut crate::uitest::Harness,
        n: usize,
        h: f32,
        sel: usize,
        events: Vec<egui::Event>,
    ) -> (Option<usize>, Vec<usize>) {
        let mut picked = None;
        let mut drawn = Vec::new();
        hn.frame(events, |ui| {
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(Rect::from_min_size(pos2(10.0, 10.0), vec2(300.0, h))),
            );
            picked = list_box(
                &mut child,
                vec2(300.0, h),
                Id::new("l"),
                n,
                20.0,
                sel,
                &look(),
                |_, _, i, _| {
                    drawn.push(i);
                    format!("row {i}")
                },
            );
        });
        (picked, drawn)
    }

    #[test]
    fn a_list_draws_whole_rows_and_a_click_picks_one() {
        let mut h = crate::uitest::Harness::new(400.0, 400.0);
        // 100 tall, 2px bevel: four whole rows of 20.
        let (_, drawn) = list(&mut h, 10, 100.0, 0, Vec::new());
        assert_eq!(drawn, [0, 1, 2, 3]);
        let at = h.find("row 2").unwrap().center();
        let (picked, _) = list(&mut h, 10, 100.0, 0, crate::uitest::click(at));
        assert_eq!(picked, Some(2));
        // Picking past the bottom brings the row into view.
        let (_, drawn) = list(&mut h, 10, 100.0, 7, Vec::new());
        assert!(drawn.contains(&7), "{drawn:?}");
        // A list that fits has no gadget: its rows run the box's whole width.
        let (_, drawn) = list(&mut h, 3, 100.0, 0, Vec::new());
        assert_eq!(drawn, [0, 1, 2]);
        let row = h.find("row 0").unwrap();
        assert_eq!(row.width(), 300.0 - 2.0 * size::BEVEL);
    }

    #[test]
    fn the_frame_a_row_is_clicked_shows_one_cursor() {
        let mut h = crate::uitest::Harness::new(400.0, 400.0);
        let mut on: Vec<usize> = Vec::new();
        let frame = |h: &mut crate::uitest::Harness, ev: Vec<egui::Event>, on: &mut Vec<usize>| {
            on.clear();
            h.frame(ev, |ui| {
                let mut child = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(Rect::from_min_size(pos2(10.0, 10.0), vec2(300.0, 100.0))),
                );
                list_box(
                    &mut child,
                    vec2(300.0, 100.0),
                    Id::new("l"),
                    4,
                    20.0,
                    1,
                    &look(),
                    |_, _, i, lit| {
                        if lit {
                            on.push(i);
                        }
                        format!("row {i}")
                    },
                );
            });
        };
        frame(&mut h, Vec::new(), &mut on);
        assert_eq!(on, [1]);
        let at = h.find("row 3").unwrap().center();
        frame(&mut h, crate::uitest::click(at), &mut on);
        assert_eq!(on, [3], "only the clicked row is lit");
    }

    #[test]
    fn the_wheel_over_the_rows_scrolls_the_list() {
        let mut h = crate::uitest::Harness::new(400.0, 400.0);
        let rows =
            |h: &mut crate::uitest::Harness, ev: Vec<egui::Event>| list(h, 20, 100.0, 0, ev).1;
        assert_eq!(rows(&mut h, Vec::new()), [0, 1, 2, 3]);
        let over = h.find("row 1").unwrap().center();
        let wheel = vec![
            egui::Event::PointerMoved(over),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: vec2(0.0, -40.0),
                modifiers: egui::Modifiers::NONE,
                phase: egui::TouchPhase::Move,
            },
        ];
        rows(&mut h, wheel);
        for _ in 0..3 {
            rows(&mut h, Vec::new());
        }
        let after = rows(&mut h, Vec::new());
        assert_ne!(after.first(), Some(&0), "scrolled: {after:?}");
    }
}
