//! The AnarchyOS kit: what a tiling desktop after Omarchy/Hyprland draws with. Tiles with
//! their focus border and cut-in label, the bar with its workspace pills, the launcher and the
//! key help. It draws and reports clicks and picks; what to show, which keys do what and the
//! layout are the desktop's (`tiling.rs`) and core's (`desktop::Tiling`). The numbers are the
//! spec sheet's (`docs/refs/anarchyos.md`). Nothing here names a tool or a deck.

use egui::{
    Align2, Color32, Event, FontId, Id, Key, Painter, Rect, Sense, Stroke, StrokeKind, Ui, pos2,
    vec2,
};

use super::List;

/// Sizes, in one place (the spec sheet's).
pub mod size {
    pub const BAR_H: f32 = 26.0;
    pub const GAP: f32 = 8.0;
    pub const TITLE_H: f32 = 18.0;
    pub const ROUND: f32 = 6.0;
    pub const LAUNCHER: [f32; 2] = [460.0, 320.0];
    pub const LAUNCHER_ROW: f32 = 20.0;
    pub const PILL: [f32; 2] = [22.0, 18.0];
    pub const PILL_STEP: f32 = 26.0;
    pub const HELP_W: f32 = 580.0;
    pub const HELP_LINE: f32 = 22.0;
}

/// The desktop's colours and type, from the theme.
#[derive(Clone)]
pub struct Look {
    pub backdrop: Color32,
    pub shadow: Color32,
    pub ink: Color32,
    pub accent: Color32,
    pub muted: Color32,
    /// The body type (the launcher, the help).
    pub font: FontId,
    /// The bar's and the labels' type.
    pub small: FontId,
    /// How the launcher's search box is dressed.
    pub visuals: egui::Visuals,
}

/// A tile in `r`: its fill, its border (thicker, in the accent, when `focused`) and its
/// `label` cut into the top border.
pub fn tile(p: &Painter, r: Rect, label: &str, focused: bool, look: &Look) {
    p.rect_filled(r, size::ROUND, look.backdrop);
    let edge = if focused { look.accent } else { look.muted };
    p.rect_stroke(
        r,
        size::ROUND,
        Stroke::new(if focused { 2.0 } else { 1.0 }, edge),
        StrokeKind::Inside,
    );
    let g = p.layout_no_wrap(format!(" {label} "), look.small.clone(), edge);
    let tr = Rect::from_min_size(r.min + vec2(12.0, -7.0), g.size());
    p.rect_filled(tr, 0.0, look.shadow);
    p.galley(tr.min, g, look.ink);
}

/// Where a tile's content goes inside its rect `r`: under the label, inside the margins.
pub fn tile_content(r: Rect) -> Rect {
    r.shrink2(vec2(10.0, 8.0))
        .with_min_y(r.top() + size::TITLE_H - 4.0)
}

/// A workspace pill on the bar.
pub struct Workspace {
    /// It's the one shown.
    pub here: bool,
    /// It has tiles.
    pub used: bool,
}

/// What the bar shows.
pub struct Bar<'a> {
    pub workspaces: &'a [Workspace],
    /// The mode after the pills, and whether it's lit (the leader is down).
    pub mode: &'a str,
    pub mode_lit: bool,
    /// The focused tile's name (and any marker after it).
    pub focused: &'a str,
    /// The widgets on the right.
    pub right: &'a str,
}

/// The bar across the top of `screen`. Returns the workspace clicked, when `live`.
pub fn bar(ui: &Ui, p: &Painter, screen: Rect, b: &Bar, look: &Look, live: bool) -> Option<usize> {
    let rect = Rect::from_min_size(screen.min, vec2(screen.width(), size::BAR_H));
    p.rect_filled(rect, 0.0, look.shadow);
    p.line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(1.0, look.muted),
    );
    let mut x = rect.left() + 8.0;
    let mut clicked = None;
    for (w, ws) in b.workspaces.iter().enumerate() {
        let r = Rect::from_min_size(
            pos2(x, rect.top() + (size::BAR_H - size::PILL[1]) / 2.0),
            vec2(size::PILL[0], size::PILL[1]),
        );
        if ws.here {
            p.rect_filled(r, 4.0, look.accent);
        }
        let col = if ws.here {
            look.shadow
        } else if ws.used {
            look.ink
        } else {
            look.muted
        };
        p.text(
            r.center(),
            Align2::CENTER_CENTER,
            (w + 1).to_string(),
            look.small.clone(),
            col,
        );
        if live
            && ui
                .interact(r, Id::new(("tile-ws", w)), Sense::click())
                .clicked()
        {
            clicked = Some(w);
        }
        x += size::PILL_STEP;
    }
    p.text(
        pos2(x + 8.0, rect.center().y),
        Align2::LEFT_CENTER,
        b.mode,
        look.small.clone(),
        if b.mode_lit { look.accent } else { look.muted },
    );
    p.text(
        pos2(x + 110.0, rect.center().y),
        Align2::LEFT_CENTER,
        b.focused,
        look.small.clone(),
        look.ink,
    );
    p.text(
        rect.right_center() - vec2(8.0, 0.0),
        Align2::RIGHT_CENTER,
        b.right,
        look.small.clone(),
        look.muted,
    );
    clicked
}

/// The launcher between frames: what's typed, the pick, the list's window.
#[derive(Default)]
pub struct Launcher {
    pub query: String,
    pub sel: usize,
    pub list: List,
    focused: bool,
    /// The pick the window last went to: it follows a new pick, not the wheel's every turn.
    followed: Option<usize>,
}

/// The launcher over `screen`: a search box over `labels`, filtered by `search` (indices of
/// the labels that match the query, best first), ↑↓ to pick, Enter or a click to choose.
/// Returns the label chosen. `hint` is the empty search box's hint.
pub fn launcher(
    ctx: &egui::Context,
    screen: Rect,
    l: &mut Launcher,
    labels: &[String],
    search: impl Fn(&str) -> Vec<usize>,
    hint: &str,
    look: &Look,
) -> Option<usize> {
    // On whole pixels, so its border and rows stay sharp.
    let r = Rect::from_center_size(
        pos2(screen.center().x, screen.top() + screen.height() * 0.35),
        vec2(size::LAUNCHER[0], size::LAUNCHER[1]),
    );
    let r = Rect::from_min_size(r.min.round(), r.size());
    let mut chosen = None;
    // The box is painted inside the Area, on its own layer, so the widgets draw on top.
    egui::Area::new(Id::new("launcher-area"))
        .order(egui::Order::Foreground)
        .fade_in(false)
        .fixed_pos(r.min)
        .show(ctx, |ui| {
            let p = ui.painter();
            p.rect_filled(r.expand(3.0), size::ROUND + 2.0, look.shadow);
            p.rect_filled(r, size::ROUND, look.backdrop);
            p.rect_stroke(
                r,
                size::ROUND,
                Stroke::new(2.0, look.accent),
                StrokeKind::Inside,
            );
            ui.allocate_rect(r, Sense::hover());
            let inner = r.shrink2(vec2(12.0, 10.0));
            let mut ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(inner)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            let ui = &mut ui;
            ui.set_width(inner.width());
            *ui.visuals_mut() = look.visuals.clone();
            let edit = egui::TextEdit::singleline(&mut l.query)
                .hint_text(hint)
                .font(look.font.clone())
                .desired_width(f32::INFINITY)
                .id(Id::new("launcher-input"));
            let resp = ui.add(edit);
            if !l.focused {
                resp.request_focus();
                l.focused = true;
            }
            if resp.changed() {
                l.sel = 0;
            }
            // Filtered after the edit, so the list matches what was just typed.
            let hits = search(&l.query);
            let (down, up, enter) = ui.input(|i| {
                (
                    i.key_pressed(Key::ArrowDown),
                    i.key_pressed(Key::ArrowUp),
                    i.key_pressed(Key::Enter),
                )
            });
            if !hits.is_empty() {
                if down {
                    l.sel = (l.sel + 1) % hits.len();
                }
                if up {
                    l.sel = (l.sel + hits.len() - 1) % hits.len();
                }
                l.sel = l.sel.min(hits.len() - 1);
            }
            // The list starts on a whole pixel, under the box.
            let y = ui.cursor().top() + 6.0;
            ui.add_space(6.0 + y.ceil() - y);
            // As many whole rows as fit (the last one no longer runs past the box), the window
            // going to a new pick and otherwise staying where the wheel left it.
            let gap = ui.spacing().item_spacing.y;
            let pitch = size::LAUNCHER_ROW + gap;
            let rows = ((ui.available_height() + gap) / pitch).floor().max(1.0) as usize;
            let follow = (l.followed != Some(l.sel)).then_some(l.sel);
            l.followed = Some(l.sel);
            let top = ui.cursor().top();
            for k in l.list.view(hits.len(), follow, rows) {
                let i = hits[k];
                let on = k == l.sel;
                let txt = egui::RichText::new(&labels[i])
                    .font(look.font.clone())
                    .color(if on { look.shadow } else { look.ink });
                let b = egui::Button::new(txt)
                    .fill(if on { look.accent } else { look.backdrop })
                    .frame(on);
                if ui
                    .add_sized([ui.available_width(), size::LAUNCHER_ROW], b)
                    .clicked()
                {
                    chosen = Some(i);
                }
            }
            // The wheel over the rows scrolls them, a row at a time; the box keeps the wheel.
            let rows_rect = Rect::from_min_max(pos2(inner.left(), top), inner.max);
            if ui.rect_contains_pointer(rows_rect) {
                let delta = ui.input(|i| i.smooth_scroll_delta.y);
                if delta != 0.0 {
                    l.list.wheel(delta / pitch);
                    ui.ctx().input_mut(|i| i.smooth_scroll_delta.y = 0.0);
                }
            }
            // Where you are, when the list runs past the box.
            if let Some(at) =
                ninsei_core::scroll::position(hits.len(), Some(l.sel), l.list.top, rows)
            {
                ui.painter().text(
                    pos2(r.right() - 10.0, r.bottom() - 4.0),
                    Align2::RIGHT_BOTTOM,
                    at,
                    look.small.clone(),
                    look.muted,
                );
            }
            if hits.is_empty() {
                ui.label(
                    egui::RichText::new("nothing matches")
                        .font(look.font.clone())
                        .color(look.muted),
                );
            }
            if enter && let Some(&i) = hits.get(l.sel) {
                chosen = Some(i);
            }
        });
    chosen
}

/// The key help over `screen`: a key and what it does on each line. Returns true when it's
/// dismissed (a click, or any key but `?` and `/`, which open it).
pub fn help(ui: &Ui, screen: Rect, lines: &[(&str, &str)], look: &Look) -> bool {
    let r = Rect::from_center_size(
        screen.center(),
        vec2(size::HELP_W, size::HELP_LINE * lines.len() as f32 + 24.0),
    );
    let r = Rect::from_min_size(r.min.round(), r.size());
    let p = ui.ctx().layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        Id::new("tiling-help"),
    ));
    p.rect_filled(r.expand(3.0), size::ROUND + 2.0, look.shadow);
    p.rect_filled(r, size::ROUND, look.backdrop);
    p.rect_stroke(
        r,
        size::ROUND,
        Stroke::new(2.0, look.accent),
        StrokeKind::Inside,
    );
    for (i, (k, v)) in lines.iter().enumerate() {
        let y = r.top() + 14.0 + i as f32 * size::HELP_LINE;
        p.text(
            pos2(r.left() + 18.0, y),
            Align2::LEFT_TOP,
            *k,
            look.font.clone(),
            look.accent,
        );
        p.text(
            pos2(r.left() + 150.0, y),
            Align2::LEFT_TOP,
            *v,
            look.font.clone(),
            look.ink,
        );
    }
    ui.input(|i| {
        i.pointer.primary_clicked()
            || i.events.iter().any(|e| {
                matches!(e, Event::Key { pressed: true, key, .. } if *key != Key::Questionmark && *key != Key::Slash)
            })
    })
}
