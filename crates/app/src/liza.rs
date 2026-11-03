//! Liza, the Sense/Net deck's desktop: after the first Mac and the Lisa. A 1-bit menu bar with
//! the tools as menus, Finder windows with striped title bars and a close box, icons down the
//! right, a Trash, and a happy deck at boot. Everything is two theme colours (ink on paper) and
//! patterns from `data/patterns.ron`; a front-end on `home`, like the other desktops.

use egui::{Align2, Color32, FontFamily, FontId, Id, Pos2, Rect, Sense, Stroke, StrokeKind, Ui};
use egui::{Vec2, pos2, vec2};
use ninsei_core::Data;
use ninsei_core::hub::{IconDef, Tool};

use crate::home::{self, Cx, Desk, DesktopView, Home, WbOut, font};
use crate::{hubtools, patterns};

/// Every size and timing the desktop uses.
mod look {
    pub const MENU_H: f32 = 22.0;
    pub const TITLE_H: f32 = 20.0;
    /// The 1-bit drop shadow under windows and menus.
    pub const SHADOW: f32 = 2.0;
    pub const CLOSE_BOX: f32 = 12.0;
    /// Gap kept between a window's bottom and the screen's.
    pub const WIN_MARGIN: f32 = 8.0;
    pub const WIN_MIN_H: f32 = 120.0;
    /// Screen points per icon pixel.
    pub const ICON_SCALE: f32 = 3.0;
    pub const ICON_PITCH: f32 = 82.0;
    pub const ICON_COL_W: f32 = 96.0;
    pub const BOOT_SCALE: f32 = 5.0;
    /// How long the boot card stays up, unless clicked away.
    pub const BOOT_SECS: f64 = 1.8;
    pub const DOUBLE_CLICK_SECS: f64 = 0.45;
    pub const MENU_ITEM_H: f32 = 22.0;
    pub const MENU_W: f32 = 190.0;
    /// The desktop's pattern (calm on a lit-ink screen; the 50% checker is the boot card's).
    pub const DESKTOP: &str = "p25";
    pub const BOOT: &str = "p50";
}

/// What a menu item does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Act {
    Open(Tool),
    /// Closes the front window.
    Close,
    /// Shows the boot card again.
    About,
}

/// The menu bar: the tools as menus.
const MENUS: &[(&str, &[(&str, Act)])] = &[
    (
        "File",
        &[
            ("Shell", Act::Open(Tool::Shell)),
            ("Close Window", Act::Close),
        ],
    ),
    ("Net", &[("Phonebook", Act::Open(Tool::Terminal))]),
    (
        "Deck",
        &[
            ("RIG", Act::Open(Tool::Rig)),
            ("Stash", Act::Open(Tool::Stash)),
            ("HexEd", Act::Open(Tool::HexEd)),
        ],
    ),
    (
        "Special",
        &[
            ("Prefs", Act::Open(Tool::Prefs)),
            ("About Liza", Act::About),
        ],
    ),
];

struct Win {
    tool: Tool,
    /// Top-left, relative to the screen.
    pos: Pos2,
    size: Vec2,
}

fn title(tool: Tool) -> &'static str {
    match tool {
        Tool::Shell => "Shell",
        Tool::Terminal => "Phonebook",
        Tool::Prefs => "Prefs",
        Tool::Rig => "RIG",
        Tool::Stash => "Stash",
        Tool::HexEd => "HexEd",
    }
}

/// The Liza desktop.
pub struct Liza {
    /// Back to front: the last window is on top.
    wins: Vec<Win>,
    selected: Option<Tool>,
    last_click: Option<(Tool, f64)>,
    /// The window on top last frame, so the Shell takes the keyboard when it comes forward.
    top: Option<Tool>,
    /// The menu dropped open, by index into `MENUS`.
    menu: Option<usize>,
    /// When the boot card went up (`None`: not yet shown); `done` once it's gone.
    boot: Option<f64>,
    booted: bool,
}

impl Liza {
    pub fn new() -> Self {
        Self {
            wins: Vec::new(),
            selected: None,
            last_click: None,
            top: None,
            menu: None,
            boot: None,
            booted: false,
        }
    }

    /// Opens (or brings forward) a tool's window.
    fn open(&mut self, tool: Tool) {
        if let Some(i) = self.wins.iter().position(|w| w.tool == tool) {
            let w = self.wins.remove(i);
            self.wins.push(w);
            return;
        }
        let (pos, size) = match tool {
            Tool::Shell => (pos2(40.0, 330.0), vec2(620.0, 290.0)),
            Tool::Terminal => (pos2(60.0, 44.0), vec2(720.0, 440.0)),
            Tool::Prefs => (pos2(140.0, 44.0), vec2(560.0, 600.0)),
            Tool::Rig => (pos2(80.0, 54.0), vec2(620.0, 330.0)),
            Tool::Stash => (pos2(110.0, 84.0), vec2(640.0, 380.0)),
            Tool::HexEd => (pos2(40.0, 40.0), vec2(800.0, 500.0)),
        };
        self.wins.push(Win { tool, pos, size });
    }

    fn act(&mut self, a: Act, now: f64) {
        match a {
            Act::Open(t) => self.open(t),
            Act::Close => {
                self.wins.pop();
            }
            Act::About => {
                self.boot = Some(now);
                self.booted = false;
            }
        }
    }
}

/// Liza's two colours: ink lit on a dark paper (lit ink), from the theme's text and its deepest
/// background.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Two {
    ink: Color32,
    paper: Color32,
}

fn colours(d: Desk) -> Two {
    Two {
        ink: d.ink,
        paper: d.shadow,
    }
}

impl DesktopView for Liza {
    fn show(&mut self, ui: &mut Ui, home: &mut Home, cx: &Cx, out: &mut Vec<WbOut>) {
        let (data, save, offset, live) = (cx.data, cx.save, cx.offset, cx.live);
        home.prompt = "Liza> ";
        for t in home.take_opens() {
            self.open(t);
        }
        let theme = home::desk();
        let c = colours(theme);
        let (ink, paper) = (c.ink, c.paper);
        // The tools draw through the desktop's colour registers: load a strict two-colour set
        // while Liza draws, and put the theme's back after.
        home::set_desk(Desk {
            screen: paper,
            ink,
            shadow: paper,
            accent: ink,
            muted: theme.muted,
            fill: paper,
        });
        let now = ui.input(|i| i.time);
        let screen = ui.ctx().content_rect().translate(vec2(0.0, offset));
        let painter = ui.painter_at(screen);
        let at = |p: Pos2| screen.min + p.to_vec2();
        let sense = |s: Sense| if live { s } else { Sense::hover() };

        // The desktop: a pattern under everything.
        if let Some(p) = data.pattern(look::DESKTOP) {
            patterns::fill(&painter, p, screen, ink, paper);
        }
        let bg = ui.interact(screen, Id::new("liza-bg"), sense(Sense::click()));
        if bg.clicked() {
            self.selected = None;
            self.menu = None;
        }

        // Icons down the right, then the Trash in the corner.
        let col = screen.right() - look::ICON_COL_W;
        for (n, icon) in data.icons.iter().enumerate() {
            let cell = Rect::from_min_size(
                pos2(
                    col,
                    screen.top() + look::MENU_H + 14.0 + n as f32 * look::ICON_PITCH,
                ),
                vec2(look::ICON_COL_W, look::ICON_PITCH - 6.0),
            );
            let picked = self.selected == Some(icon.tool);
            let hit = draw_icon(&painter, icon, cell, picked, c);
            let r = ui.interact(hit, Id::new(("liza-icon", n)), sense(Sense::click()));
            if r.clicked() || r.double_clicked() {
                let double = r.double_clicked()
                    || matches!(self.last_click, Some((t, at)) if t == icon.tool && now - at < look::DOUBLE_CLICK_SECS);
                self.selected = Some(icon.tool);
                self.last_click = Some((icon.tool, now));
                self.menu = None;
                if double {
                    self.last_click = None;
                    self.open(icon.tool);
                }
            }
        }
        let trash = &data.liza.trash;
        let size = patterns::bitmap_size(trash, look::ICON_SCALE);
        let spot = pos2(
            screen.right() - look::ICON_COL_W / 2.0 - size.x / 2.0,
            screen.bottom() - size.y - 34.0,
        );
        patterns::bitmap(&painter, trash, spot, look::ICON_SCALE, ink, paper);
        label(
            &painter,
            "Trash",
            pos2(spot.x + size.x / 2.0, spot.y + size.y + 10.0),
            false,
            c,
        );

        // Windows, back to front.
        let press = ui.input(|i| {
            i.pointer
                .primary_pressed()
                .then(|| i.pointer.interact_pos())
                .flatten()
        });
        let release = ui.input(|i| {
            i.pointer
                .primary_released()
                .then(|| i.pointer.interact_pos())
                .flatten()
        });
        let (mut raise, mut close) = (None, None);
        let n = self.wins.len();
        let mut rects = Vec::with_capacity(n);
        for i in 0..n {
            let front = i + 1 == n;
            let (tool, pos, size) = (self.wins[i].tool, self.wins[i].pos, self.wins[i].size);
            let room = (screen.height() - pos.y - look::WIN_MARGIN).max(look::WIN_MIN_H);
            let rect = Rect::from_min_size(at(pos), vec2(size.x, size.y.min(room)));
            let wid = Id::new(("liza-win", format!("{tool:?}")));
            let body = ui.interact(rect, wid.with("body"), sense(Sense::click()));
            if body.clicked() || body.drag_started() {
                raise = Some(i);
            }
            rects.push(rect);
            let close_r = window_frame(&painter, data, rect, title(tool), front, c);
            let bar = Rect::from_min_size(rect.min, vec2(rect.width(), look::TITLE_H));
            let drag = ui.interact(bar, wid.with("bar"), sense(Sense::click_and_drag()));
            if drag.dragged() {
                let p = &mut self.wins[i].pos;
                *p += drag.drag_delta();
                p.x = p.x.clamp(-size.x + 60.0, screen.width() - 60.0);
                p.y = p.y.clamp(look::MENU_H, screen.height() - look::TITLE_H);
            }
            if drag.drag_started() || drag.clicked() {
                raise = Some(i);
            }
            if ui
                .interact(close_r, wid.with("close"), sense(Sense::click()))
                .clicked()
            {
                close = Some(i);
            }
            let content = Rect::from_min_max(
                bar.left_bottom() + vec2(6.0, 4.0),
                rect.max - vec2(6.0, 4.0),
            );
            if tool == Tool::Shell {
                home.shell(ui, content, data, save, live && front, out);
            } else {
                let mut child = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(content)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                );
                child.set_clip_rect(content);
                hubtools::wb_visuals(child.visuals_mut());
                if !live {
                    child.disable();
                }
                home::scrolled(&mut child, ("liza-scroll", tool), |ui| {
                    home.tool_body(ui, tool, data, save, out)
                });
            }
        }
        if raise.is_none()
            && live
            && self.menu.is_none()
            && let Some(p) = press
        {
            raise = rects.iter().rposition(|r| r.contains(p));
        }
        if let Some(i) = close {
            if i < self.wins.len() {
                self.wins.remove(i);
            }
        } else if let Some(i) = raise
            && i + 1 < self.wins.len()
        {
            let w = self.wins.remove(i);
            self.wins.push(w);
        }

        // The menu bar, over everything but the boot card.
        // The menu bar and the boot card sit on their own layer, over anything a window draws.
        let mut top_ui = on_top(ui, screen, egui::Order::Foreground, "liza-menus");
        let top_painter = top_ui.painter_at(screen);
        if let Some(a) = self.menu_bar(&mut top_ui, &top_painter, screen, cx, c) {
            self.act(a, now);
        }

        // A Shell window that comes to the front takes the keyboard.
        let top = self.wins.last().map(|w| w.tool);
        let clicked_shell = top == Some(Tool::Shell)
            && release.is_some_and(|p| rects.last().is_some_and(|r| r.contains(p)));
        if (top != self.top && top == Some(Tool::Shell)) || (live && clicked_shell) {
            home.focus();
        }
        self.top = top;

        let mut boot_ui = on_top(ui, screen, egui::Order::Tooltip, "liza-boot-card");
        let boot_painter = boot_ui.painter_at(screen);
        self.boot_card(&mut boot_ui, &boot_painter, screen, data, live, c);
        home::set_desk(theme);
    }
}

impl Liza {
    /// Draws the menu bar and any menu dropped open; returns the item picked.
    fn menu_bar(
        &mut self,
        ui: &mut Ui,
        painter: &egui::Painter,
        screen: Rect,
        cx: &Cx,
        c: Two,
    ) -> Option<Act> {
        let (ink, paper) = (c.ink, c.paper);
        let sense = |s: Sense| if cx.live { s } else { Sense::hover() };
        let bar = Rect::from_min_size(screen.min, vec2(screen.width(), look::MENU_H));
        painter.rect_filled(bar, 0.0, paper);
        painter.line_segment(
            [bar.left_bottom(), bar.right_bottom()],
            Stroke::new(1.0, ink),
        );
        // Sense/Net's mark: an eye.
        let eye = bar.left_center() + vec2(18.0, 0.0);
        painter.circle_stroke(eye, 6.0, Stroke::new(1.5, ink));
        painter.circle_filled(eye, 2.5, ink);
        let mut x = bar.left() + 40.0;
        let mut picked = None;
        for (m, (name, items)) in MENUS.iter().enumerate() {
            let w = painter
                .layout_no_wrap(name.to_string(), font(), ink)
                .size()
                .x
                + 20.0;
            let r = Rect::from_min_size(pos2(x, bar.top()), vec2(w, look::MENU_H - 1.0));
            let open = self.menu == Some(m);
            if open {
                painter.rect_filled(r, 0.0, ink);
            }
            painter.text(
                r.center(),
                Align2::CENTER_CENTER,
                *name,
                font(),
                if open { paper } else { ink },
            );
            let resp = ui.interact(r, Id::new(("liza-menu", m)), sense(Sense::click()));
            if resp.clicked() {
                self.menu = if open { None } else { Some(m) };
            } else if self.menu.is_some() && resp.hovered() {
                // Sliding along the bar with a menu open opens the next one.
                self.menu = Some(m);
            }
            if self.menu == Some(m) {
                let drop = Rect::from_min_size(
                    pos2(r.left(), bar.bottom()),
                    vec2(look::MENU_W, items.len() as f32 * look::MENU_ITEM_H + 8.0),
                );
                painter.rect_filled(drop.translate(Vec2::splat(look::SHADOW)), 0.0, ink);
                painter.rect_filled(drop, 0.0, paper);
                painter.rect_stroke(drop, 0.0, Stroke::new(1.0, ink), StrokeKind::Inside);
                for (k, (label, act)) in items.iter().enumerate() {
                    let row = Rect::from_min_size(
                        drop.min + vec2(1.0, 4.0 + k as f32 * look::MENU_ITEM_H),
                        vec2(drop.width() - 2.0, look::MENU_ITEM_H),
                    );
                    let hit = ui.interact(row, Id::new(("liza-item", m, k)), sense(Sense::click()));
                    let lit = hit.hovered();
                    if lit {
                        painter.rect_filled(row, 0.0, ink);
                    }
                    painter.text(
                        row.left_center() + vec2(14.0, 0.0),
                        Align2::LEFT_CENTER,
                        *label,
                        font(),
                        if lit { paper } else { ink },
                    );
                    if hit.clicked() {
                        picked = Some(*act);
                        self.menu = None;
                    }
                }
            }
            x += w;
        }
        // The bank and the time on the right.
        let up = ui.input(|i| i.time) as u64;
        painter.text(
            bar.right_center() - vec2(12.0, 0.0),
            Align2::RIGHT_CENTER,
            format!(
                "{}¢    {:02}:{:02}",
                cx.save.bank,
                up / 3600 % 24,
                up / 60 % 60
            ),
            font(),
            ink,
        );
        picked
    }

    /// The happy deck, for a moment at boot (or from Special > About Liza). A click skips it.
    fn boot_card(
        &mut self,
        ui: &mut Ui,
        painter: &egui::Painter,
        screen: Rect,
        data: &Data,
        live: bool,
        c: Two,
    ) {
        if self.booted {
            return;
        }
        let (ink, paper) = (c.ink, c.paper);
        let now = ui.input(|i| i.time);
        let start = *self.boot.get_or_insert(now);
        // Timed out: gone before it takes another click.
        if now - start >= look::BOOT_SECS {
            self.booted = true;
            return;
        }
        let click = ui.interact(
            screen,
            Id::new("liza-boot"),
            if live { Sense::click() } else { Sense::hover() },
        );
        if click.clicked() {
            self.booted = true;
            return;
        }
        ui.ctx().request_repaint();
        let p = data
            .pattern(look::BOOT)
            .or_else(|| data.pattern(look::DESKTOP));
        if let Some(p) = p {
            patterns::fill(painter, p, screen, ink, paper);
        }
        let art = &data.liza.boot;
        let size = patterns::bitmap_size(art, look::BOOT_SCALE);
        let c = screen.center();
        let top = c.y - size.y / 2.0 - 70.0;
        patterns::bitmap(
            painter,
            art,
            pos2(c.x - size.x / 2.0, top),
            look::BOOT_SCALE,
            ink,
            paper,
        );
        let boxr = Rect::from_center_size(pos2(c.x, top + size.y + 70.0), vec2(380.0, 84.0));
        painter.rect_filled(boxr.translate(Vec2::splat(look::SHADOW)), 0.0, ink);
        painter.rect_filled(boxr, 0.0, paper);
        painter.rect_stroke(boxr, 0.0, Stroke::new(1.0, ink), StrokeKind::Inside);
        painter.rect_stroke(
            boxr.shrink(4.0),
            0.0,
            Stroke::new(1.0, ink),
            StrokeKind::Inside,
        );
        painter.text(
            boxr.center() - vec2(0.0, 10.0),
            Align2::CENTER_CENTER,
            "Welcome to Liza.",
            FontId::new(home::FONT * 1.6, FontFamily::Monospace),
            ink,
        );
        painter.text(
            boxr.center() + vec2(0.0, 20.0),
            Align2::CENTER_CENTER,
            "Sense/Net Personal Deck",
            font(),
            ink,
        );
    }
}

/// A child of `ui` on a layer of its own (`order`), covering `rect`.
fn on_top(ui: &mut Ui, rect: Rect, order: egui::Order, name: &str) -> Ui {
    ui.new_child(
        egui::UiBuilder::new()
            .layer_id(egui::LayerId::new(order, Id::new(name)))
            .max_rect(rect),
    )
}

/// Window chrome: a 1-px frame with a drop shadow, a title bar striped when it's in front, the
/// title on a solid backing, and a close box. Returns the close box.
fn window_frame(
    painter: &egui::Painter,
    data: &Data,
    rect: Rect,
    title: &str,
    front: bool,
    c: Two,
) -> Rect {
    let (ink, paper) = (c.ink, c.paper);
    painter.rect_filled(rect.translate(Vec2::splat(look::SHADOW)), 0.0, ink);
    painter.rect_filled(rect, 0.0, paper);
    painter.rect_stroke(rect, 0.0, Stroke::new(1.0, ink), StrokeKind::Inside);
    let bar = Rect::from_min_size(rect.min, vec2(rect.width(), look::TITLE_H));
    painter.line_segment(
        [bar.left_bottom(), bar.right_bottom()],
        Stroke::new(1.0, ink),
    );
    if front && let Some(p) = data.pattern("pinstripes") {
        patterns::fill(painter, p, bar.shrink2(vec2(3.0, 4.0)), ink, paper);
    }
    let close = Rect::from_min_size(
        bar.left_center() + vec2(10.0, -look::CLOSE_BOX / 2.0),
        Vec2::splat(look::CLOSE_BOX),
    );
    if front {
        painter.rect_filled(close.expand(2.0), 0.0, paper);
        painter.rect_stroke(close, 0.0, Stroke::new(1.0, ink), StrokeKind::Inside);
    }
    let galley = painter.layout_no_wrap(title.to_string(), font(), ink);
    let tbox = Rect::from_center_size(bar.center(), galley.size() + vec2(16.0, 2.0));
    painter.rect_filled(tbox, 0.0, paper);
    painter.galley(tbox.min + vec2(8.0, 1.0), galley, ink);
    close
}

/// An icon in 1-bit: the art's outlines in ink, its body in paper (inverted when selected),
/// with the name under it on a solid backing. Returns the area that takes clicks.
fn draw_icon(painter: &egui::Painter, icon: &IconDef, cell: Rect, picked: bool, c: Two) -> Rect {
    let (ink, paper) = (c.ink, c.paper);
    let w = icon.width() as f32 * look::ICON_SCALE;
    let h = icon.pixels.len() as f32 * look::ICON_SCALE;
    let art = Rect::from_min_size(pos2(cell.center().x - w / 2.0, cell.top()), vec2(w, h));
    let (fg, bg) = if picked { (paper, ink) } else { (ink, paper) };
    for (y, row) in icon.pixels.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            // The Workbench palette folds into two tones: dark strokes are ink, light fills paper.
            let col = match ch {
                'k' | 'o' => fg,
                'w' | 'b' => bg,
                _ => continue,
            };
            let p = art.min + vec2(x as f32, y as f32) * look::ICON_SCALE;
            painter.rect_filled(
                Rect::from_min_size(p, Vec2::splat(look::ICON_SCALE)),
                0.0,
                col,
            );
        }
    }
    let lr = label(
        painter,
        &icon.label,
        pos2(cell.center().x, art.bottom() + 10.0),
        picked,
        c,
    );
    art.union(lr)
}

/// A name under an icon, on paper (inverted when selected).
fn label(painter: &egui::Painter, text: &str, center: Pos2, picked: bool, c: Two) -> Rect {
    let (fg, bg) = if picked {
        (c.paper, c.ink)
    } else {
        (c.ink, c.paper)
    };
    let galley = painter.layout_no_wrap(text.to_string(), font(), fg);
    let r = Rect::from_center_size(center, galley.size() + vec2(8.0, 2.0));
    painter.rect_filled(r, 0.0, bg);
    painter.galley(r.min + vec2(4.0, 1.0), galley, fg);
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::home::{Cx, Home};
    use egui::{Event, PointerButton, RawInput};
    use ninsei_core::hub::Save;

    fn frame(
        ctx: &egui::Context,
        liza: &mut Liza,
        home: &mut Home,
        data: &Data,
        events: Vec<Event>,
        t: f64,
    ) {
        let save = Save::default();
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1280.0, 800.0))),
            time: Some(t),
            events,
            ..RawInput::default()
        };
        let mut output = ctx.run_ui(raw, |ui| {
            let cx = Cx {
                data,
                save: &save,
                offset: 0.0,
                live: true,
            };
            liza.show(ui, home, &cx, &mut Vec::new());
        });
        output.textures_delta.clear();
    }

    fn click(at: Pos2) -> Vec<Event> {
        let b = |pressed| Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        vec![Event::PointerMoved(at), b(true), b(false)]
    }

    /// A desktop past its boot card.
    fn booted(ctx: &egui::Context, data: &Data) -> (Liza, Home) {
        let (mut liza, mut home) = (Liza::new(), Home::new(Vec::new()));
        frame(ctx, &mut liza, &mut home, data, Vec::new(), 0.0);
        frame(
            ctx,
            &mut liza,
            &mut home,
            data,
            Vec::new(),
            look::BOOT_SECS + 0.1,
        );
        (liza, home)
    }

    #[test]
    fn it_boots_to_the_happy_deck_and_a_click_skips_it() {
        let data = Data::builtin().unwrap();
        let ctx = egui::Context::default();
        let (mut liza, mut home) = (Liza::new(), Home::new(Vec::new()));
        frame(&ctx, &mut liza, &mut home, &data, Vec::new(), 0.0);
        assert!(!liza.booted, "the boot card is up");
        frame(
            &ctx,
            &mut liza,
            &mut home,
            &data,
            click(pos2(600.0, 400.0)),
            0.3,
        );
        assert!(liza.booted, "a click skips it");
        // Special > About Liza brings it back, and time takes it away.
        liza.act(Act::About, 5.0);
        frame(&ctx, &mut liza, &mut home, &data, Vec::new(), 5.1);
        assert!(!liza.booted);
        frame(
            &ctx,
            &mut liza,
            &mut home,
            &data,
            Vec::new(),
            5.0 + look::BOOT_SECS + 0.1,
        );
        assert!(liza.booted);
    }

    #[test]
    fn double_clicking_an_icon_opens_its_window() {
        let data = Data::builtin().unwrap();
        let ctx = egui::Context::default();
        let (mut liza, mut home) = booted(&ctx, &data);
        for (n, icon) in data.icons.iter().enumerate() {
            let at = pos2(
                1280.0 - look::ICON_COL_W / 2.0,
                look::MENU_H + 14.0 + n as f32 * look::ICON_PITCH + 20.0,
            );
            let t = 10.0 + n as f64 * 10.0;
            frame(&ctx, &mut liza, &mut home, &data, click(at), t);
            frame(&ctx, &mut liza, &mut home, &data, click(at), t + 0.1);
            frame(&ctx, &mut liza, &mut home, &data, Vec::new(), t + 0.2);
            assert_eq!(
                liza.wins.last().map(|w| w.tool),
                Some(icon.tool),
                "{} didn't open",
                icon.label
            );
        }
    }

    #[test]
    fn the_menus_open_the_tools() {
        let data = Data::builtin().unwrap();
        let ctx = egui::Context::default();
        let (mut liza, mut home) = booted(&ctx, &data);
        // Every item that opens a tool is on a menu.
        for t in Tool::ALL {
            assert!(
                MENUS
                    .iter()
                    .any(|(_, items)| items.iter().any(|(_, a)| *a == Act::Open(t))),
                "{t:?} has no menu item"
            );
        }
        // Net > Phonebook, by clicking.
        let net = pos2(40.0 + 60.0, look::MENU_H / 2.0);
        frame(&ctx, &mut liza, &mut home, &data, click(net), 3.0);
        assert_eq!(liza.menu, Some(1), "Net dropped open");
        let item = pos2(net.x, look::MENU_H + 4.0 + look::MENU_ITEM_H / 2.0);
        frame(
            &ctx,
            &mut liza,
            &mut home,
            &data,
            vec![Event::PointerMoved(item)],
            3.1,
        );
        frame(&ctx, &mut liza, &mut home, &data, click(item), 3.2);
        assert_eq!(liza.menu, None);
        assert_eq!(liza.wins.last().map(|w| w.tool), Some(Tool::Terminal));
        // File > Close Window closes the front one.
        liza.act(Act::Close, 4.0);
        assert!(liza.wins.iter().all(|w| w.tool != Tool::Terminal));
    }

    #[test]
    fn it_draws_in_two_colours_and_gives_the_theme_back() {
        let data = Data::builtin().unwrap();
        let ctx = egui::Context::default();
        let before = home::desk();
        let (mut liza, mut home) = booted(&ctx, &data);
        frame(&ctx, &mut liza, &mut home, &data, Vec::new(), 4.0);
        assert_eq!(home::desk(), before, "the theme's registers come back");
        let c = colours(before);
        assert_eq!(
            (c.ink, c.paper),
            (before.ink, before.shadow),
            "lit ink on dark paper"
        );
    }
}
