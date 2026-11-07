//! The Liza kit: the widgets of a 1-bit desktop after the first Mac's System 1. Windows with a
//! striped title bar and a close box, the menu bar and its menus, the scroll bar and the list
//! box, push buttons, check boxes and radio buttons, and the alert. It draws and reports clicks and picks; what the menus hold, what
//! a window shows and what a button does are the caller's. The numbers are the
//! spec sheet's (`docs/refs/liza.md`). Nothing here names a tool or a deck.

use egui::{Align2, Color32, FontId, Id, Key, Painter, Pos2, Rect, Response, Sense, Stroke};
use egui::{StrokeKind, Ui, Vec2, pos2, vec2};
use ninsei_core::pattern::PatternDef;

use super::List;
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
    /// A window's content, in from its frame and its title bar.
    pub const PAD_X: f32 = 6.0;
    pub const PAD_Y: f32 = 4.0;
    pub const BUTTON_H: f32 = 22.0;
    pub const BUTTON_R: f32 = 8.0;
    pub const BUTTON_MIN_W: f32 = 60.0;
    /// A button's room either side of its text.
    pub const BUTTON_PAD: f32 = 12.0;
    /// How far outside its button the default ring sits, and how thick it is.
    pub const RING_OUT: f32 = 4.0;
    pub const RING_W: f32 = 3.0;
    /// A check box's or radio button's square, and the gap to its label.
    pub const CHECK: f32 = 12.0;
    pub const CHECK_GAP: f32 = 5.0;
    /// The alert: its inner frame's inset and weight, the margin inside it, the icon's size,
    /// and the gap between buttons.
    pub const ALERT_INSET: f32 = 4.0;
    pub const ALERT_FRAME: f32 = 3.0;
    pub const ALERT_MARGIN: f32 = 12.0;
    pub const ALERT_ICON: f32 = 28.0;
    pub const BUTTON_GAP: f32 = 10.0;
    /// A scroll bar's width, and the side of its arrow boxes and its thumb.
    pub const SCROLL_W: f32 = 16.0;
}

/// Liza's two colours and her type, from the theme, and the patterns she fills with.
#[derive(Clone)]
pub struct Look<'a> {
    pub ink: Color32,
    pub paper: Color32,
    pub font: FontId,
    /// The front window's title bar.
    pub stripes: Option<&'a PatternDef>,
    /// The 50% gray: a scroll bar's track.
    pub gray: Option<&'a PatternDef>,
}

/// `r` on whole pixels: 1px ink lines on half pixels blur.
fn snap(r: Rect) -> Rect {
    Rect::from_min_size(r.min.round(), r.size().round())
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
    let rect = snap(rect);
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
    let tbox = snap(Rect::from_center_size(
        bar.center(),
        galley.size() + vec2(16.0, 2.0),
    ));
    p.rect_filled(tbox, 0.0, paper);
    p.galley(tbox.min + vec2(8.0, 1.0), galley, ink);
    close
}

/// A window's title bar: where it's dragged from.
pub fn title_bar(rect: Rect) -> Rect {
    let rect = snap(rect);
    Rect::from_min_size(rect.min, vec2(rect.width(), size::TITLE_H))
}

/// Where a window's contents go: inside its frame, under the title bar.
pub fn window_content(rect: Rect) -> Rect {
    Rect::from_min_max(
        title_bar(rect).left_bottom() + vec2(size::PAD_X, size::PAD_Y),
        snap(rect).max - vec2(size::PAD_X, size::PAD_Y),
    )
}

/// Where a window's scrolled body goes ([`scrolled`]): from the content's left, under the title
/// bar, to the window's bottom right corner, so its scroll bar sits flush on the frame.
pub fn window_scrolled(rect: Rect) -> Rect {
    Rect::from_min_max(
        title_bar(rect).left_bottom() + vec2(size::PAD_X, 0.0),
        snap(rect).max,
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
    let bar = snap(menu_bar_rect(screen));
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
            .ceil()
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

/// Where a scrolling view stands, in points: how far down it is, how much shows, how much
/// there is, and how far an arrow moves it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scroll {
    pub offset: f32,
    pub view: f32,
    pub content: f32,
    pub line: f32,
}

impl Scroll {
    /// How far down it can go.
    fn max(&self) -> f32 {
        (self.content - self.view).max(0.0)
    }

    /// There's more than shows.
    pub fn active(&self) -> bool {
        self.max() > 0.5
    }
}

/// A scroll bar's parts in `r`: the up arrow's box, the down arrow's, the track between them
/// and, when there's anything to scroll, the thumb on it.
fn bar_parts(r: Rect, s: &Scroll) -> (Rect, Rect, Rect, Option<Rect>) {
    let w = size::SCROLL_W;
    let up = Rect::from_min_size(r.min, vec2(w, w));
    let down = Rect::from_min_max(pos2(r.left(), r.bottom() - w), r.max);
    let track = Rect::from_min_max(up.left_bottom(), down.right_top());
    let travel = (track.height() - w).max(0.0);
    let thumb = (s.active() && travel > 0.0).then(|| {
        let y = (track.top() + travel * (s.offset / s.max()).clamp(0.0, 1.0)).round();
        Rect::from_min_size(pos2(r.left(), y), vec2(w, w))
    });
    (up, down, track, thumb)
}

/// What the scroll bar in `r` does to `s` this frame: an arrow scrolls a line, a click on the
/// track above or below the thumb a page, the thumb drags the view, and the wheel over the bar
/// scrolls it. Returns the new offset when it changed.
fn bar_input(ui: &Ui, r: Rect, id: Id, s: &Scroll) -> Option<f32> {
    if !s.active() {
        return None;
    }
    let (up, down, track, thumb) = bar_parts(r, s);
    let mut to = s.offset;
    let (up_r, down_r) = (
        ui.interact(up, id.with("up"), Sense::click()),
        ui.interact(down, id.with("down"), Sense::click()),
    );
    if up_r.clicked() {
        to -= s.line;
    }
    if down_r.clicked() {
        to += s.line;
    }
    let t = ui.interact(track, id.with("track"), Sense::click_and_drag());
    let grab = id.with("grab");
    // A drag holds the thumb only if it started on it (egui starts it a few points later).
    if t.drag_started()
        && let (Some(th), Some(p)) = (thumb, ui.input(|i| i.pointer.press_origin()))
        && th.contains(p)
    {
        ui.data_mut(|d| d.insert_temp(grab, (s.offset, p.y)));
    }
    let held = ui.data(|d| d.get_temp::<(f32, f32)>(grab));
    match held {
        Some((from, y)) if t.dragged() => {
            let travel = (track.height() - size::SCROLL_W).max(1.0);
            if let Some(p) = t.interact_pointer_pos() {
                to = from + (p.y - y) * s.max() / travel;
            }
        }
        Some(_) if !t.dragged() => ui.data_mut(|d| d.remove::<(f32, f32)>(grab)),
        _ => {}
    }
    if t.clicked()
        && let (Some(th), Some(p)) = (thumb, t.interact_pointer_pos())
    {
        let page = (s.view - s.line).max(s.line);
        if p.y < th.top() {
            to -= page;
        } else if p.y > th.bottom() {
            to += page;
        }
    }
    if let Some(wheel) = take_wheel(ui, r, id) {
        to -= wheel;
    }
    let to = to.clamp(0.0, s.max()).round();
    (to != s.offset).then_some(to)
}

/// The wheel's travel over `r` this frame, taken so nothing under it scrolls too: only while
/// `ui` takes input, nothing is being dragged, and nothing drawn since (a window in front)
/// covers `r` where the pointer is. Register it after the widgets inside `r`: egui hovers a
/// hover-only widget only when no clickable or draggable one lies above it.
fn take_wheel(ui: &Ui, r: Rect, id: Id) -> Option<f32> {
    let over = ui.interact(r, id.with("wheel"), Sense::hover()).hovered();
    let free = ui.is_enabled() && ui.ctx().dragged_id().is_none();
    let wheel = ui.input(|i| i.smooth_scroll_delta.y);
    if !(free && wheel != 0.0 && over) {
        return None;
    }
    ui.ctx().input_mut(|i| i.smooth_scroll_delta.y = 0.0);
    Some(wheel)
}

/// Paints the scroll bar in `r` for `s`: its frame, the arrow boxes, and the gray track with the
/// thumb on it, or an empty track when there's nothing to scroll.
fn bar_paint(p: &Painter, r: Rect, s: &Scroll, look: &Look) {
    let (ink, paper) = (look.ink, look.paper);
    let (up, down, track, thumb) = bar_parts(r, s);
    p.rect_filled(r, 0.0, paper);
    if let (Some(_), Some(g)) = (thumb, look.gray) {
        patterns::fill(p, g, track, ink, paper);
    }
    let line = Stroke::new(1.0, ink);
    p.rect_stroke(r, 0.0, line, StrokeKind::Inside);
    for (b, dir) in [(up, -1.0), (down, 1.0)] {
        p.rect_stroke(b, 0.0, line, StrokeKind::Inside);
        arrow(p, b, dir, line);
    }
    if let Some(th) = thumb {
        p.rect_filled(th, 0.0, paper);
        p.rect_stroke(th, 0.0, line, StrokeKind::Inside);
    }
}

/// A scroll bar's arrow in its box, outlined: pointing up (`dir` -1) or down (1).
fn arrow(p: &Painter, b: Rect, dir: f32, line: Stroke) {
    let c = b.center();
    let (tip, base) = (c.y + dir * 4.0, c.y - dir * 3.0);
    let pts = vec![
        pos2(c.x, tip),
        pos2(c.x + 5.0, base),
        pos2(c.x - 5.0, base),
        pos2(c.x, tip),
    ];
    p.add(egui::Shape::line(pts, line));
}

/// A scroll bar in `r` ([`size::SCROLL_W`] wide) for `s`. Returns the new offset when it was
/// scrolled.
pub fn scroll_bar(ui: &Ui, p: &Painter, r: Rect, id: Id, s: Scroll, look: &Look) -> Option<f32> {
    let to = bar_input(ui, r, id, &s);
    bar_paint(
        p,
        r,
        &Scroll {
            offset: to.unwrap_or(s.offset),
            ..s
        },
        look,
    );
    to
}

/// `body` in a view that scrolls, filling `ui` (a window's [`window_scrolled`]): the view is in
/// from the top and bottom, and from the right by the scroll bar's room. The bar sits flush on
/// the right edge, only while there's more than shows (a tool that fits has no dead control
/// beside it). The wheel over either scrolls it.
pub fn scrolled(ui: &mut Ui, id: Id, look: &Look, body: impl FnOnce(&mut Ui)) {
    let rect = ui.max_rect();
    let bar = Rect::from_min_max(pos2(rect.right() - size::SCROLL_W, rect.top()), rect.max);
    let key = id.with("kit-scroll");
    let line = ui.fonts_mut(|f| f.row_height(&look.font)).ceil();
    // The bar acts on last frame's numbers, so this frame's view already shows the move.
    let last = ui.data(|d| d.get_temp::<Scroll>(key));
    let set = last.and_then(|s| bar_input(ui, bar, key, &s));
    let view_rect = Rect::from_min_max(
        rect.min + vec2(0.0, size::PAD_Y),
        pos2(bar.left() - size::PAD_X, rect.bottom() - size::PAD_Y),
    );
    let mut view = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(view_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    // A vertical scroll area clips only top and bottom; the view's sides are its edges too.
    view.set_clip_rect(view_rect.intersect(ui.clip_rect()));
    let mut area = egui::ScrollArea::vertical()
        .id_salt(key)
        .auto_shrink([false, false])
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden);
    if let Some(o) = set {
        area = area.vertical_scroll_offset(o);
    }
    let out = area.show(&mut view, body);
    let now = Scroll {
        offset: out.state.offset.y,
        view: out.inner_rect.height(),
        content: out.content_size.y,
        line,
    };
    ui.data_mut(|d| d.insert_temp(key, now));
    if now.active() {
        bar_paint(ui.painter(), bar, &now, look);
    }
    ui.advance_cursor_after_rect(rect);
}

/// The room [`scrolled`] keeps beside its view: the bar and the gap before it.
pub const SCROLLED_W: f32 = size::SCROLL_W + size::PAD_X;

/// A list box `size` big: a framed list of `n` cells `cell_h` tall from `top` down, the
/// `sel`ected one drawn by `cell` as on (it returns the cell's name, for the accessibility tree), and the scroll bar down its right side (active once the
/// cells run past the box, scrolling them a whole cell at a time). A click on a cell picks it and
/// gives the box the focus; with the focus ↑↓ walk the list. Returns the cell picked.
#[allow(clippy::too_many_arguments)]
pub fn list_box(
    ui: &mut Ui,
    size: Vec2,
    id: Id,
    n: usize,
    cell_h: f32,
    top: f32,
    sel: usize,
    look: &Look,
    mut cell: impl FnMut(&Painter, Rect, usize, bool) -> String,
) -> Option<usize> {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let rect = Rect::from_min_size(rect.min.round(), rect.size().round());
    // The box itself takes the focus (a click anywhere in it), so the arrows can walk it.
    let boxed = ui.interact(rect, id, Sense::click());
    if boxed.clicked() {
        boxed.request_focus();
    }
    let mut picked = None;
    if boxed.has_focus() {
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(
                id,
                egui::EventFilter {
                    vertical_arrows: true,
                    ..Default::default()
                },
            )
        });
        let (up, down) = ui.input(|i| (i.key_pressed(Key::ArrowUp), i.key_pressed(Key::ArrowDown)));
        if up && sel > 0 {
            picked = Some(sel - 1);
        }
        if down && sel + 1 < n {
            picked = Some(sel + 1);
        }
    }
    let at = picked.unwrap_or(sel);
    let bar = Rect::from_min_max(pos2(rect.right() - size::SCROLL_W, rect.top()), rect.max);
    let rows = (((rect.height() - top) / cell_h).floor() as usize).max(1);
    let key = id.with("list");
    let mut list: List = ui.data(|d| d.get_temp(key)).unwrap_or_default();
    let follow = (ui.data(|d| d.get_temp::<usize>(key.with("sel"))) != Some(at)).then_some(at);
    list.view(n, follow, rows);
    let s = Scroll {
        offset: list.top as f32 * cell_h,
        view: rows as f32 * cell_h,
        content: n as f32 * cell_h,
        line: cell_h,
    };
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 0.0, look.paper);
    if let Some(o) = scroll_bar(ui, &p, bar, id.with("bar"), s, look) {
        list.top = (o / cell_h).round() as usize;
    }
    let shown = list.view(n, None, rows);
    p.rect_stroke(rect, 0.0, Stroke::new(1.0, look.ink), StrokeKind::Inside);
    let cells_w = rect.width() - size::SCROLL_W;
    for i in shown {
        let r = Rect::from_min_size(
            rect.min + vec2(0.0, top + list.row(i) as f32 * cell_h),
            vec2(cells_w, cell_h),
        );
        let hit = ui.interact(r, id.with(i), Sense::click());
        if hit.clicked() {
            picked = Some(i);
            ui.memory_mut(|m| m.request_focus(id));
        }
        let name = cell(&p, r, i, i == at);
        hit.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, i == at, &name)
        });
    }
    // The wheel over the cells, for the next frame (the cells are registered, so it's known
    // whether anything covers them).
    if s.active()
        && let Some(wheel) = take_wheel(ui, rect.with_max_x(bar.left()), id)
    {
        list.wheel(wheel / cell_h);
        ui.ctx().request_repaint();
    }
    ui.data_mut(|d| {
        d.insert_temp(key, list);
        d.insert_temp(key.with("sel"), picked.unwrap_or(sel));
    });
    picked
}

/// A push button's width for `label`: its text and the room either side, at least the minimum,
/// on a whole pixel.
pub fn button_width(ctx: &egui::Context, label: &str, look: &Look) -> f32 {
    let text = ctx.fonts_mut(|f| {
        f.layout_no_wrap(label.to_string(), look.font.clone(), look.ink)
            .size()
            .x
    });
    (text + 2.0 * size::BUTTON_PAD)
        .max(size::BUTTON_MIN_W)
        .ceil()
}

/// A push button in `r`: a round-rect with its label centred, inverted while it's pressed, and
/// the heavy ring outside it when it's the `default`. Disabled, its label is at half strength and
/// it takes no clicks.
#[allow(clippy::too_many_arguments)]
pub fn button(
    ui: &Ui,
    p: &Painter,
    r: Rect,
    id: Id,
    label: &str,
    default: bool,
    enabled: bool,
    look: &Look,
) -> Response {
    let r = Rect::from_min_size(r.min.round(), r.size().round());
    let resp = ui.interact(
        r,
        id,
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    let pressed = enabled && resp.is_pointer_button_down_on();
    let (fg, bg) = if pressed {
        (look.paper, look.ink)
    } else {
        (look.ink, look.paper)
    };
    p.rect_filled(r, size::BUTTON_R, bg);
    p.rect_stroke(
        r,
        size::BUTTON_R,
        Stroke::new(1.0, look.ink),
        StrokeKind::Inside,
    );
    if default {
        default_ring(p, r, size::BUTTON_R, look.ink);
    }
    // A disabled button in a live Ui has its label at half strength (a disabled Ui already
    // paints everything so).
    let dim = !enabled && ui.is_enabled();
    p.text(
        r.center(),
        Align2::CENTER_CENTER,
        label,
        look.font.clone(),
        if dim { fg.gamma_multiply(0.5) } else { fg },
    );
    resp
}

/// The default button's heavy ring around a button `face` with corners `radius`: the spec's
/// ring, 4px out and 3px thick. Every default button draws it here.
pub fn default_ring(p: &Painter, face: Rect, radius: f32, ink: Color32) {
    p.rect_stroke(
        face.expand(size::RING_OUT),
        radius + size::RING_OUT,
        Stroke::new(size::RING_W, ink),
        StrokeKind::Inside,
    );
}

/// A push button laid out in `ui`: its width from its label, the room for the default ring
/// around it. Disabled when `ui` is.
pub fn push_button(ui: &mut Ui, label: &str, default: bool, look: &Look) -> Response {
    let w = button_width(ui.ctx(), label, look);
    let ring = size::RING_OUT;
    let (r, _) = ui.allocate_exact_size(
        vec2(w + 2.0 * ring, size::BUTTON_H + 2.0 * ring),
        Sense::hover(),
    );
    let face = Rect::from_min_size(r.min + Vec2::splat(ring), vec2(w, size::BUTTON_H));
    let id = ui.id().with(("push", label));
    let enabled = ui.is_enabled();
    button(ui, ui.painter(), face, id, label, default, enabled, look)
}

/// A check box (`round` false) or radio button (true) with its label, laid out in `ui`: the
/// square or circle, an X or a dot when `on`, thicker while pressed. Returns its response;
/// clicked is the caller's to act on.
fn toggle(ui: &mut Ui, on: bool, label: &str, round: bool, look: &Look) -> Response {
    // Disabled, egui paints the Ui at half strength: a dimmed control, as the Mac's was.
    let ink = look.ink;
    let g = ui
        .painter()
        .layout_no_wrap(label.to_string(), look.font.clone(), ink);
    let want = vec2(
        size::CHECK + size::CHECK_GAP + g.size().x,
        g.size().y.max(size::CHECK),
    )
    .ceil();
    let (r, resp) = ui.allocate_exact_size(want, Sense::click());
    let kind = if round {
        egui::WidgetType::RadioButton
    } else {
        egui::WidgetType::Checkbox
    };
    resp.widget_info(|| egui::WidgetInfo::selected(kind, ui.is_enabled(), on, label));
    let p = ui.painter();
    let b = Rect::from_min_size(
        pos2(r.left(), r.center().y - size::CHECK / 2.0).round(),
        Vec2::splat(size::CHECK),
    );
    let width = if resp.is_pointer_button_down_on() {
        2.0
    } else {
        1.0
    };
    let line = Stroke::new(width, ink);
    if round {
        let c = b.center();
        p.circle_filled(c, size::CHECK / 2.0, look.paper);
        p.circle_stroke(c, size::CHECK / 2.0 - 0.5, line);
        if on {
            p.circle_filled(c, size::CHECK / 4.0, ink);
        }
    } else {
        p.rect_filled(b, 0.0, look.paper);
        p.rect_stroke(b, 0.0, line, StrokeKind::Inside);
        if on {
            let x = Stroke::new(1.0, ink);
            let i = b.shrink(1.0);
            p.line_segment([i.left_top(), i.right_bottom()], x);
            p.line_segment([i.right_top(), i.left_bottom()], x);
        }
    }
    let at = pos2(b.right() + size::CHECK_GAP, r.center().y - g.size().y / 2.0).round();
    p.galley(at, g, ink);
    resp
}

/// A check box: a click flips `on`, and the response says it changed.
pub fn check_box(ui: &mut Ui, on: &mut bool, label: &str, look: &Look) -> Response {
    let mut resp = toggle(ui, *on, label, false, look);
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    resp
}

/// A radio button, `on` when it's the one chosen; clicked chooses it.
pub fn radio(ui: &mut Ui, on: bool, label: &str, look: &Look) -> Response {
    toggle(ui, on, label, true, look)
}

/// What an alert says and offers.
pub struct Alert<'a> {
    /// The icon's glyph, top left.
    pub icon: &'a str,
    pub title: &'a str,
    pub message: &'a str,
    /// Left to right; the action goes last.
    pub buttons: &'a [&'a str],
    /// The button Enter presses, ringed.
    pub default: Option<usize>,
    /// The button Esc presses.
    pub cancel: Option<usize>,
    /// Enter and Esc answer ([`alert_keys`]) only while this holds (nothing else is taking the
    /// keyboard).
    pub keys: bool,
}

/// An alert at `at`, `width` wide, over everything: the double frame, the icon, the title and
/// the message wrapped beside it, and the buttons along the bottom right. Returns the button
/// clicked; its keys are [`alert_keys`].
pub fn alert(
    ctx: &egui::Context,
    id: Id,
    at: Pos2,
    width: f32,
    a: &Alert,
    look: &Look,
) -> Option<usize> {
    let (ink, paper) = (look.ink, look.paper);
    let pad = size::ALERT_INSET + size::ALERT_FRAME + size::ALERT_MARGIN;
    let text_x = pad + size::ALERT_ICON + size::ALERT_MARGIN;
    let wrap = width - text_x - pad;
    let (title, message) = ctx.fonts_mut(|f| {
        (
            f.layout(a.title.to_string(), look.font.clone(), ink, wrap),
            f.layout(a.message.to_string(), look.font.clone(), ink, wrap),
        )
    });
    let text_h = title.size().y + 4.0 + message.size().y;
    let body_h = text_h.max(size::ALERT_ICON).ceil();
    let height = pad + body_h + size::ALERT_MARGIN + size::BUTTON_H + pad;
    let r = Rect::from_min_size(at.round(), vec2(width, height).round());
    let mut pressed = None;
    egui::Area::new(id)
        .order(egui::Order::Foreground)
        .fade_in(false)
        .fixed_pos(r.min)
        .show(ctx, |ui| {
            ui.allocate_rect(r, Sense::hover());
            let p = ui.painter();
            p.rect_filled(r, 0.0, paper);
            p.rect_stroke(r, 0.0, Stroke::new(1.0, ink), StrokeKind::Inside);
            p.rect_stroke(
                r.shrink(size::ALERT_INSET),
                0.0,
                Stroke::new(size::ALERT_FRAME, ink),
                StrokeKind::Inside,
            );
            p.text(
                r.min + Vec2::splat(pad),
                Align2::LEFT_TOP,
                a.icon,
                FontId::proportional(size::ALERT_ICON),
                ink,
            );
            let tx = pos2(r.left() + text_x, r.top() + pad);
            let below = tx + vec2(0.0, (title.size().y + 4.0).round());
            p.galley(tx, title, ink);
            p.galley(below, message, ink);
            // The buttons, right to left from the corner.
            let mut x = r.right() - pad;
            let y = r.bottom() - pad - size::BUTTON_H;
            for (i, label) in a.buttons.iter().enumerate().rev() {
                let w = button_width(ctx, label, look);
                x -= w;
                let b = Rect::from_min_size(pos2(x, y), vec2(w, size::BUTTON_H));
                let def = a.default == Some(i);
                if button(ui, p, b, id.with(i), label, def, true, look).clicked() {
                    pressed = Some(i);
                }
                x -= size::BUTTON_GAP;
            }
        });
    pressed
}

/// The alert's keys: Enter presses the default button and Esc the cancel button, when `keys`
/// held last frame too (so the Esc that lets go of a text field, which egui drops before this
/// frame draws, doesn't also cancel). It takes the key it acts on, so nothing that reads keys
/// after it sees it too. Call it before anything else that reads Enter or Esc this frame.
/// Returns the button pressed.
pub fn alert_keys(ctx: &egui::Context, id: Id, a: &Alert) -> Option<usize> {
    let was = id.with("keys");
    let before = ctx.data(|d| d.get_temp::<bool>(was)).unwrap_or(false);
    ctx.data_mut(|d| d.insert_temp(was, a.keys));
    if !(a.keys && before) {
        return None;
    }
    // Only a fresh press: a held key repeats, and a Mac alert's button goes once. A repeat is
    // still taken, so it doesn't reach anything after.
    let take = |key, button: Option<usize>| {
        button.filter(|_| {
            let fresh = ctx.input(|i| {
                i.events.iter().any(|e| {
                    matches!(e, egui::Event::Key { key: k, pressed: true, repeat: false, modifiers, .. }
                        if *k == key && modifiers.is_none())
                })
            });
            let taken = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key));
            fresh && taken
        })
    };
    take(Key::Enter, a.default).or_else(|| take(Key::Escape, a.cancel))
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
            gray: None,
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

    #[test]
    fn a_check_box_flips_and_says_so_and_a_radio_leaves_it_to_the_caller() {
        let mut h = Harness::new(400.0, 200.0);
        // (on, changed, radio clicked)
        let mut st = (false, false, false);
        let frame = |h: &mut Harness, st: &mut (bool, bool, bool), ev| {
            h.frame(ev, |ui| {
                st.1 |= check_box(ui, &mut st.0, "Sound", &look()).changed();
                st.2 |= radio(ui, false, "Off", &look()).clicked();
            })
        };
        frame(&mut h, &mut st, vec![]);
        let at = h.find("Sound").expect("labelled");
        frame(&mut h, &mut st, click(at.center()));
        assert!(st.0 && st.1);
        let r = h.find("Off").unwrap();
        frame(&mut h, &mut st, click(r.center()));
        assert!(st.2);
        // The box is the spec's square, on whole pixels.
        let boxes: Vec<Rect> = h
            .rects()
            .into_iter()
            .map(|(r, _, _)| r)
            .filter(|r| r.size() == Vec2::splat(size::CHECK))
            .collect();
        assert!(!boxes.is_empty() && boxes.iter().all(|b| b.min == b.min.round()));
    }

    #[test]
    fn a_disabled_button_takes_no_click() {
        let mut h = Harness::new(400.0, 200.0);
        let r = Rect::from_min_size(pos2(20.0, 20.0), vec2(80.0, size::BUTTON_H));
        let mut clicked = false;
        for ev in [vec![], click(r.center())] {
            h.frame(ev, |ui| {
                clicked |= button(
                    ui,
                    ui.painter(),
                    r,
                    Id::new("b"),
                    "OK",
                    false,
                    false,
                    &look(),
                )
                .clicked();
            });
        }
        assert!(!clicked);
    }

    /// Frames of an alert offering Close and Run; returns the last frame's answer.
    fn ask(
        h: &mut Harness,
        keys: bool,
        default: Option<usize>,
        ev: Vec<egui::Event>,
    ) -> Option<usize> {
        let mut got = None;
        h.frame(ev, |ui| {
            let a = Alert {
                icon: "!",
                title: "Black ICE rezzed.",
                message: "Killer, strength 9 of 9. It's holding the line, and this message is long enough to wrap.",
                buttons: &["Close", "Run icebreaker.exe"],
                default,
                cancel: Some(0),
                keys,
            };
            got = alert_keys(ui.ctx(), Id::new("a"), &a)
                .or(alert(ui.ctx(), Id::new("a"), pos2(40.5, 30.3), 460.0, &a, &look()));
        });
        got
    }

    #[test]
    fn the_alert_answers_clicks_enter_and_esc() {
        let mut h = Harness::new(800.0, 600.0);
        ask(&mut h, true, Some(1), vec![]);
        ask(&mut h, true, Some(1), vec![]);
        let close = h.find("Close").unwrap();
        let run = h.find("Run icebreaker.exe").unwrap();
        assert!(
            close.right() < run.left(),
            "the action is at the right, Close left of it"
        );
        assert_eq!(close.min, close.min.round(), "on whole pixels");
        assert_eq!(ask(&mut h, true, Some(1), click(run.center())), Some(1));
        assert_eq!(
            ask(&mut h, true, Some(1), crate::uitest::key(Key::Enter)),
            Some(1)
        );
        assert_eq!(
            ask(&mut h, true, Some(1), crate::uitest::key(Key::Escape)),
            Some(0)
        );
        // No default: Enter does nothing.
        assert_eq!(
            ask(&mut h, true, None, crate::uitest::key(Key::Enter)),
            None
        );
    }

    #[test]
    fn the_alert_leaves_the_keys_alone_while_something_else_has_them() {
        let mut h = Harness::new(800.0, 600.0);
        ask(&mut h, false, Some(1), vec![]);
        assert_eq!(
            ask(&mut h, false, Some(1), crate::uitest::key(Key::Enter)),
            None
        );
        assert_eq!(
            ask(&mut h, false, Some(1), crate::uitest::key(Key::Escape)),
            None
        );
    }

    #[test]
    fn the_alert_wraps_its_message_inside_its_frame() {
        let mut h = Harness::new(800.0, 600.0);
        ask(&mut h, true, Some(1), vec![]);
        ask(&mut h, true, Some(1), vec![]);
        let frame = Rect::from_min_size(pos2(41.0, 30.0), vec2(460.0, 1000.0));
        for s in h.painted() {
            if let egui::Shape::Text(t) = s {
                let r = Rect::from_min_size(t.pos, t.galley.size());
                assert!(
                    r.right() <= frame.right(),
                    "{:?} runs past the alert",
                    t.galley.text()
                );
            }
        }
    }

    /// A frame of a 200x300 view of a body `tall` high; returns where it stands after.
    fn view(h: &mut Harness, tall: f32, ev: Vec<egui::Event>) -> Scroll {
        view_in(h, tall, true, ev)
    }

    fn view_in(h: &mut Harness, tall: f32, enabled: bool, ev: Vec<egui::Event>) -> Scroll {
        let id = Id::new("v");
        h.frame(ev, |ui| {
            let r = Rect::from_min_size(pos2(10.0, 10.0), vec2(200.0, 300.0));
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r));
            if !enabled {
                child.disable();
            }
            scrolled(&mut child, id, &look(), |ui| {
                ui.allocate_exact_size(vec2(100.0, tall), Sense::hover());
            });
        });
        h.ctx
            .data(|d| d.get_temp::<Scroll>(id.with("kit-scroll")))
            .unwrap()
    }

    /// The scroll bar's parts for the view above.
    fn parts(s: &Scroll) -> (Rect, Rect, Rect, Option<Rect>) {
        let bar = Rect::from_min_max(pos2(210.0 - size::SCROLL_W, 10.0), pos2(210.0, 310.0));
        bar_parts(bar, s)
    }

    #[test]
    fn the_arrows_scroll_a_line_and_the_track_a_page() {
        let mut h = Harness::new(400.0, 400.0);
        view(&mut h, 1000.0, vec![]);
        let s = view(&mut h, 1000.0, vec![]);
        assert!(s.active() && s.offset == 0.0);
        let (up, down, _, thumb) = parts(&s);
        let s = view(&mut h, 1000.0, click(down.center()));
        assert_eq!(s.offset, s.line, "a line down");
        let s = view(&mut h, 1000.0, click(up.center()));
        assert_eq!(s.offset, 0.0, "and back");
        let below = pos2(down.center().x, thumb.unwrap().bottom() + 40.0);
        let s = view(&mut h, 1000.0, click(below));
        assert_eq!(s.offset, (s.view - s.line).round(), "a page down");
    }

    #[test]
    fn the_thumb_drags_the_view_and_the_wheel_over_the_bar_scrolls_it() {
        let mut h = Harness::new(400.0, 400.0);
        view(&mut h, 1000.0, vec![]);
        let s = view(&mut h, 1000.0, vec![]);
        let th = parts(&s).3.unwrap();
        let (from, to) = (th.center(), th.center() + vec2(0.0, 60.0));
        let b = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        view(
            &mut h,
            1000.0,
            vec![egui::Event::PointerMoved(from), b(from, true)],
        );
        view(
            &mut h,
            1000.0,
            vec![egui::Event::PointerMoved(from + vec2(0.0, 30.0))],
        );
        view(&mut h, 1000.0, vec![egui::Event::PointerMoved(to)]);
        let s = view(&mut h, 1000.0, vec![b(to, false)]);
        assert!(s.offset > 0.0, "the drag moved it: {s:?}");
        let th2 = parts(&s).3.unwrap();
        assert!(
            (th2.center().y - to.y).abs() <= 1.5,
            "the thumb is under the pointer"
        );
        let before = s.offset;
        let wheel = vec![
            egui::Event::PointerMoved(th2.center()),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: vec2(0.0, -40.0),
                modifiers: egui::Modifiers::NONE,
                phase: egui::TouchPhase::Move,
            },
        ];
        view(&mut h, 1000.0, wheel);
        let mut s = view(&mut h, 1000.0, vec![]);
        for _ in 0..10 {
            s = view(&mut h, 1000.0, vec![]);
        }
        assert!(s.offset > before, "the wheel over the bar scrolled it down");
    }

    #[test]
    fn a_view_that_fits_has_no_thumb_and_its_arrows_do_nothing() {
        let mut h = Harness::new(400.0, 400.0);
        view(&mut h, 100.0, vec![]);
        let s = view(&mut h, 100.0, vec![]);
        assert!(!s.active());
        let (_, down, _, thumb) = parts(&s);
        assert!(thumb.is_none());
        assert_eq!(view(&mut h, 100.0, click(down.center())).offset, 0.0);
        let bar = Rect::from_min_max(down.min - vec2(0.0, 300.0), down.max);
        assert!(
            h.rects().iter().all(|(r, _, _)| !bar.intersects(*r)),
            "no dead scroll bar beside a view that fits"
        );
    }

    /// A frame of a list box of `n` cells 20 high in a box 100 high; returns the pick.
    fn list(h: &mut Harness, n: usize, sel: &mut usize, ev: Vec<egui::Event>) -> Option<usize> {
        let mut got = None;
        h.frame(ev, |ui| {
            let r = Rect::from_min_size(pos2(0.0, 0.0), vec2(120.0, 100.0));
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r));
            got = list_box(
                &mut child,
                r.size(),
                Id::new("l"),
                n,
                20.0,
                0.0,
                *sel,
                &look(),
                |_, _, i, _| format!("cell {i}"),
            );
        });
        if let Some(i) = got {
            *sel = i;
        }
        got
    }

    #[test]
    fn a_list_box_picks_by_click_and_walks_by_arrows_once_focused() {
        let mut h = Harness::new(400.0, 400.0);
        let mut sel = 0;
        list(&mut h, 4, &mut sel, vec![]);
        // Arrows do nothing until it has the focus.
        list(&mut h, 4, &mut sel, crate::uitest::key(Key::ArrowDown));
        assert_eq!(sel, 0);
        let c2 = h.find("cell 2").unwrap();
        list(&mut h, 4, &mut sel, click(c2.center()));
        assert_eq!(sel, 2);
        list(&mut h, 4, &mut sel, crate::uitest::key(Key::ArrowDown));
        assert_eq!(sel, 3);
        list(&mut h, 4, &mut sel, crate::uitest::key(Key::ArrowDown));
        assert_eq!(sel, 3, "stops at the end");
        list(&mut h, 4, &mut sel, crate::uitest::key(Key::ArrowUp));
        assert_eq!(sel, 2);
    }

    #[test]
    fn a_long_list_box_scrolls_with_the_pick_in_whole_cells() {
        let mut h = Harness::new(400.0, 400.0);
        let mut sel = 0;
        list(&mut h, 12, &mut sel, vec![]);
        assert!(
            h.find("cell 4").is_some() && h.find("cell 5").is_none(),
            "five cells show"
        );
        let c0 = h.find("cell 0").unwrap();
        list(&mut h, 12, &mut sel, click(c0.center()));
        for _ in 0..7 {
            list(&mut h, 12, &mut sel, crate::uitest::key(Key::ArrowDown));
        }
        list(&mut h, 12, &mut sel, vec![]);
        assert_eq!(sel, 7);
        let c7 = h.find("cell 7").expect("the pick is on screen");
        assert_eq!(c7.top() % 20.0, 0.0, "on a whole cell");
        assert!(h.find("cell 2").is_none());
    }

    #[test]
    fn the_alert_ignores_the_key_that_freed_the_keyboard() {
        let mut h = Harness::new(800.0, 600.0);
        // A text field had the keys; this frame's Esc let go of it.
        ask(&mut h, false, Some(1), vec![]);
        assert_eq!(
            ask(&mut h, true, Some(1), crate::uitest::key(Key::Escape)),
            None
        );
        // The next Esc is the alert's.
        assert_eq!(
            ask(&mut h, true, Some(1), crate::uitest::key(Key::Escape)),
            Some(0)
        );
    }

    #[test]
    fn a_held_enter_presses_the_default_once() {
        let mut h = Harness::new(800.0, 600.0);
        ask(&mut h, true, Some(1), vec![]);
        // Held down: the press, then the key's repeats.
        let down = |repeat| {
            vec![egui::Event::Key {
                key: Key::Enter,
                physical_key: None,
                pressed: true,
                repeat,
                modifiers: egui::Modifiers::NONE,
            }]
        };
        assert_eq!(
            ask(&mut h, true, Some(1), down(false)),
            Some(1),
            "the press"
        );
        assert_eq!(
            ask(&mut h, true, Some(1), down(true)),
            None,
            "a repeat isn't a press"
        );
    }

    /// The wheel turned down by `dy` points with the pointer at `at`.
    fn wheel_at(at: Pos2, dy: f32) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(at),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: vec2(0.0, -dy),
                modifiers: egui::Modifiers::NONE,
                phase: egui::TouchPhase::Move,
            },
        ]
    }

    #[test]
    fn a_disabled_view_doesnt_take_the_wheel() {
        let mut h = Harness::new(400.0, 400.0);
        view_in(&mut h, 1000.0, false, vec![]);
        let s = view_in(&mut h, 1000.0, false, vec![]);
        let th = parts(&s).3.unwrap();
        view_in(&mut h, 1000.0, false, wheel_at(th.center(), 40.0));
        let mut s = s;
        for _ in 0..10 {
            s = view_in(&mut h, 1000.0, false, vec![]);
        }
        assert_eq!(s.offset, 0.0);
    }

    #[test]
    fn the_wheel_scrolls_a_long_list_box_in_whole_cells() {
        let mut h = Harness::new(400.0, 400.0);
        let mut sel = 0;
        list(&mut h, 12, &mut sel, vec![]);
        list(&mut h, 12, &mut sel, vec![]);
        list(&mut h, 12, &mut sel, wheel_at(pos2(50.0, 50.0), 45.0));
        for _ in 0..10 {
            list(&mut h, 12, &mut sel, vec![]);
        }
        let first = (0..12)
            .find(|i| h.find(&format!("cell {i}")).is_some())
            .unwrap();
        assert!(first > 0, "it scrolled");
        let c = h.find(&format!("cell {first}")).unwrap();
        assert_eq!(c.top(), 0.0, "a whole cell at the top");
        assert_eq!(sel, 0, "the wheel doesn't move the pick");
    }

    #[test]
    fn the_menus_and_a_window_dragged_to_half_a_pixel_land_on_whole_pixels() {
        let mut h = Harness::new(640.0, 400.0);
        let mut open = Some(1);
        bar(&mut h, &mut open, true, vec![]);
        let win = Rect::from_min_size(pos2(40.4, 60.6), vec2(300.3, 200.0));
        h.frame(vec![], |ui| {
            let menus = [
                Menu {
                    title: "File",
                    items: vec!["Open"],
                },
                Menu {
                    title: "Edit",
                    items: vec!["Cut"],
                },
            ];
            menu_bar(
                ui,
                ui.painter(),
                ui.max_rect(),
                &menus,
                "",
                &mut open,
                &look(),
                true,
            );
            window_frame(ui.painter(), win, "Untitled", true, &look());
        });
        for (r, _, _) in h.rects() {
            assert_eq!(r.min, r.min.round(), "{r:?} is on a half pixel");
            assert_eq!(r.max, r.max.round(), "{r:?} is on a half pixel");
        }
    }

    #[test]
    fn the_default_ring_is_the_specs() {
        let mut h = Harness::new(200.0, 100.0);
        let face = Rect::from_min_size(pos2(20.0, 20.0), vec2(80.0, size::BUTTON_H));
        h.frame(vec![], |ui| {
            default_ring(ui.painter(), face, 9.0, Color32::WHITE)
        });
        let ring = h.painted().iter().find_map(|s| match s {
            egui::Shape::Rect(r) => Some(r.clone()),
            _ => None,
        });
        let ring = ring.expect("a ring");
        assert_eq!(ring.rect, face.expand(4.0));
        assert_eq!(ring.stroke.width, 3.0);
    }
}
