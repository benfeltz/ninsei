//! What every GUI desktop's scroll bar shares: where a view stands, where the thumb sits, what a
//! click on the track, a drag of the thumb and the wheel do, and a view that scrolls with the
//! desktop's own bar beside it. How the bar looks (System 1's arrows and gray track, Workbench's
//! proportional gadget) is the desktop kit's.

use egui::{Id, Painter, Rect, Sense, Ui, pos2, vec2};

/// Where a scrolling view stands, in points: how far down it is, how much shows, how much
/// there is, and how far a line (an arrow, the wheel's notch) moves it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scroll {
    pub offset: f32,
    pub view: f32,
    pub content: f32,
    pub line: f32,
}

impl Scroll {
    /// How far down it can go.
    pub fn max(&self) -> f32 {
        (self.content - self.view).max(0.0)
    }

    /// There's more than shows.
    pub fn active(&self) -> bool {
        self.max() > 0.5
    }

    /// `to`, kept in range on a whole point; `None` when that's where it already is.
    pub fn settle(&self, to: f32) -> Option<f32> {
        let to = to.clamp(0.0, self.max()).round();
        (to != self.offset).then_some(to)
    }
}

/// The thumb on `track` for `s`, `len` long, on a whole pixel; none when there's nothing to
/// scroll or no room to move it.
pub fn thumb(track: Rect, s: &Scroll, len: f32) -> Option<Rect> {
    let travel = (track.height() - len).max(0.0);
    (s.active() && travel > 0.0).then(|| {
        let y = (track.top() + travel * (s.offset / s.max()).clamp(0.0, 1.0)).round();
        Rect::from_min_size(pos2(track.left(), y), vec2(track.width(), len))
    })
}

/// What `track` does to `to` this frame: a click above or below the `thumb` scrolls a page, and
/// a drag that starts on the thumb moves the view with it.
pub fn track_input(ui: &Ui, track: Rect, thumb: Option<Rect>, id: Id, s: &Scroll, to: &mut f32) {
    let t = ui.interact(track, id.with("track"), Sense::click_and_drag());
    let grab = id.with("grab");
    // A drag holds the thumb only if it started on it (egui starts it a few points later).
    if t.drag_started()
        && let (Some(th), Some(p)) = (thumb, ui.input(|i| i.pointer.press_origin()))
        && th.contains(p)
    {
        ui.data_mut(|d| d.insert_temp(grab, (s.offset, p.y)));
    }
    match ui.data(|d| d.get_temp::<(f32, f32)>(grab)) {
        Some((from, y)) if t.dragged() => {
            let len = thumb.map_or(0.0, |th| th.height());
            let travel = (track.height() - len).max(1.0);
            if let Some(p) = t.interact_pointer_pos() {
                *to = from + (p.y - y) * s.max() / travel;
            }
        }
        Some(_) => ui.data_mut(|d| d.remove::<(f32, f32)>(grab)),
        None => {}
    }
    if t.clicked()
        && let (Some(th), Some(p)) = (thumb, t.interact_pointer_pos())
    {
        let page = (s.view - s.line).max(s.line);
        if p.y < th.top() {
            *to -= page;
        } else if p.y > th.bottom() {
            *to += page;
        }
    }
}

/// The wheel's travel over `r` this frame, taken so nothing under it scrolls too: only while
/// `ui` takes input, nothing is being dragged, and nothing drawn since (a window in front)
/// covers `r` where the pointer is. Register it after the widgets inside `r`: egui hovers a
/// hover-only widget only when no clickable or draggable one lies above it.
pub fn take_wheel(ui: &Ui, r: Rect, id: Id) -> Option<f32> {
    let over = ui.interact(r, id.with("wheel"), Sense::hover()).hovered();
    let free = ui.is_enabled() && ui.ctx().dragged_id().is_none();
    let wheel = ui.input(|i| i.smooth_scroll_delta.y);
    if !(free && wheel != 0.0 && over) {
        return None;
    }
    ui.ctx().input_mut(|i| i.smooth_scroll_delta.y = 0.0);
    Some(wheel)
}

/// The room a scrolled view keeps: above it, between it and the bar, and below it.
#[derive(Clone, Copy, Debug)]
pub struct Pad {
    pub top: f32,
    pub gap: f32,
    pub bottom: f32,
}

/// Where a scrolled view goes in `rect`: in from the top and bottom by `pad`, and from the
/// right by a bar `bar_w` wide and the gap before it.
pub fn view_rect(rect: Rect, bar_w: f32, pad: Pad) -> Rect {
    Rect::from_min_max(
        rect.min + vec2(0.0, pad.top),
        pos2(rect.right() - bar_w - pad.gap, rect.bottom() - pad.bottom),
    )
}

/// `body` in a view that scrolls, filling `ui`, with a bar `bar_w` wide flush on the right
/// edge. The bar is the desktop's: `input` says what it does to the view this frame (given its
/// rect, its id and where the view stood last frame) and `paint` draws it, only while there's
/// more than shows. The view clips at its own edges. The wheel over the view scrolls it (the
/// next frame), unless a window in front covers it.
#[allow(clippy::too_many_arguments)]
pub fn view(
    ui: &mut Ui,
    id: Id,
    line: f32,
    bar_w: f32,
    pad: Pad,
    body: impl FnOnce(&mut Ui),
    input: impl FnOnce(&Ui, Rect, Id, &Scroll) -> Option<f32>,
    paint: impl FnOnce(&Painter, Rect, &Scroll),
) {
    let rect = ui.max_rect();
    let bar = Rect::from_min_max(pos2(rect.right() - bar_w, rect.top()), rect.max);
    let key = id.with("kit-scroll");
    // The bar acts on last frame's numbers, so this frame's view already shows the move.
    let last = ui.data(|d| d.get_temp::<Scroll>(key));
    // Last frame's wheel over the view, then this frame's bar.
    let wheeled = ui.data_mut(|d| d.remove_temp::<f32>(key.with("wheeled")));
    let set = last
        .filter(Scroll::active)
        .and_then(|s| input(ui, bar, key, &s))
        .or(wheeled);
    let view_rect = view_rect(rect, bar_w, pad);
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
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        // The wheel goes through take_wheel below, so a window in front keeps it.
        .scroll_source(egui::scroll_area::ScrollSource {
            mouse_wheel: false,
            ..Default::default()
        });
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
    // After the body, so a list inside it that scrolls takes its own wheel first.
    if now.active()
        && let Some(wheel) = take_wheel(ui, view_rect, key.with("view"))
        && let Some(to) = now.settle(now.offset - wheel)
    {
        ui.data_mut(|d| d.insert_temp(key.with("wheeled"), to));
        ui.ctx().request_repaint();
    }
    if now.active() {
        paint(ui.painter(), bar, &now);
    }
    ui.advance_cursor_after_rect(rect);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_thumb_runs_the_track_end_to_end_on_whole_pixels() {
        let track = Rect::from_min_size(pos2(0.0, 10.0), vec2(16.0, 101.0));
        let at = |offset| Scroll {
            offset,
            view: 100.0,
            content: 300.0,
            line: 10.0,
        };
        let top = thumb(track, &at(0.0), 20.0).unwrap();
        let end = thumb(track, &at(200.0), 20.0).unwrap();
        let mid = thumb(track, &at(77.0), 20.0).unwrap();
        assert_eq!((top.top(), end.bottom()), (track.top(), track.bottom()));
        assert_eq!(mid.top(), mid.top().round());
        let fits = Scroll {
            content: 90.0,
            ..at(0.0)
        };
        assert!(thumb(track, &fits, 20.0).is_none());
    }

    #[test]
    fn settle_keeps_the_view_in_range() {
        let s = Scroll {
            offset: 0.0,
            view: 100.0,
            content: 300.0,
            line: 10.0,
        };
        assert_eq!(s.settle(-40.0), None, "already at the top");
        assert_eq!(s.settle(999.0), Some(200.0));
        assert_eq!(s.settle(10.4), Some(10.0));
    }
}
