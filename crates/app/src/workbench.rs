//! The Workbench 1.3 desktop (the Hosaka's): pixel-art icons, draggable windows, and
//! "insert volume" requesters for tools that aren't installed yet. A front-end on `home`,
//! which holds the Shell and the tools' state. All drawn by hand into one area so the whole
//! screen can be dragged off a run.

use egui::{Align2, Id, Pos2, Rect, Sense, Ui, Vec2, pos2, vec2};
use ninsei_core::desktop::AddonEffect;
use ninsei_core::hardware::Desktop;
use ninsei_core::hub::{IconDef, Tool};

use crate::home::{Cx, DesktopView, Home, WbOut, accent, backdrop, fill, font, ink, shadow};
use crate::hubtools;
use crate::kit::workbench::{self as kit, Look};

/// Gap kept between a window's bottom and the screen's.
const WIN_MARGIN: f32 = 8.0;
/// Shortest a window gets when the screen is small.
const WIN_MIN_H: f32 = 120.0;
/// Screen pixels per icon pixel.
const ICON_SCALE: f32 = 3.0;
const ICON_PITCH: f32 = 84.0;
/// Width of a column of icons.
const ICON_COL_W: f32 = 92.0;

/// Where the `n`th icon sits on a screen `size`: down the left; once that column is full, the
/// rest go down the right edge, where Workbench keeps its disks, clear of the windows.
fn icon_pos(n: usize, size: Vec2) -> Pos2 {
    let top = kit::size::SCREEN_BAR_H + 16.0;
    let per_col = (((size.y - top) / ICON_PITCH).floor() as usize).max(1);
    let (col, row) = (n / per_col, n % per_col);
    let x = if col == 0 {
        18.0
    } else {
        size.x - 18.0 - 84.0 - (col - 1) as f32 * ICON_COL_W
    };
    pos2(x, top + row as f32 * ICON_PITCH)
}
/// Seconds between clicks that still count as a double-click.
const DOUBLE_CLICK_SECS: f64 = 0.45;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Shell,
    /// CarrierTerm: the phonebook.
    Terminal,
    System,
    Rig,
    HexEd,
    Qwk,
    IceDb,
    NightComm,
    TigerTeam,
    Guide,
    /// "Please insert volume X in any drive".
    Requester(Tool),
}

struct Win {
    kind: Kind,
    /// Top-left, relative to the screen.
    pos: Pos2,
    size: Vec2,
}

impl Win {
    fn title(&self) -> String {
        match self.kind {
            Kind::Shell => "NinShell".into(),
            Kind::System => "System".into(),
            Kind::Terminal => "CarrierTerm 1.0: Dialing Directory".into(),
            Kind::Rig => "CPU Shopper".into(),
            Kind::HexEd => "HexEd".into(),
            Kind::Qwk => "OLX QWK Reader".into(),
            Kind::IceDb => "ICE DB".into(),
            Kind::NightComm => "NiGHTCOMM 3.1".into(),
            Kind::TigerTeam => "Tiger Team Pro 3.0".into(),
            Kind::Guide => "Welcome to the Net".into(),
            Kind::Requester(_) => "System Request".into(),
        }
    }
}

/// Desktop's look, from the theme: the one place the theme maps onto the Workbench kit (System's
/// Preferences takes it too).
pub(crate) fn look() -> Look {
    Look {
        ink: ink(),
        backdrop: backdrop(),
        shadow: shadow(),
        accent: accent(),
        fill: fill(),
        font: font(),
    }
}

/// The Workbench 1.3 desktop: icons down the left, windows you drag, raise and close.
pub struct Workbench {
    /// Back to front: the last window is on top.
    wins: Vec<Win>,
    selected: Option<Tool>,
    last_click: Option<(Tool, f64)>,
    /// The window on top last frame, so the Shell takes the keyboard when it comes forward.
    top: Option<Kind>,
}

impl Workbench {
    pub fn new() -> Self {
        Self {
            wins: Vec::new(),
            selected: None,
            last_click: None,
            top: None,
        }
    }

    /// Opens (or raises) a tool's window. Tools not installed yet get a requester.
    fn open(&mut self, tool: Tool) {
        let kind = match tool {
            t if !t.installed() => Kind::Requester(t),
            Tool::Terminal => Kind::Terminal,
            Tool::System => Kind::System,
            Tool::Shell => Kind::Shell,
            Tool::Rig => Kind::Rig,
            Tool::HexEd => Kind::HexEd,
            Tool::Qwk => Kind::Qwk,
            Tool::IceDb => Kind::IceDb,
            Tool::NightComm => Kind::NightComm,
            Tool::TigerTeam => Kind::TigerTeam,
            Tool::Guide => Kind::Guide,
        };
        if let Some(i) = self.wins.iter().position(|w| w.kind == kind) {
            let w = self.wins.remove(i);
            self.wins.push(w);
        } else {
            let (pos, size) = match kind {
                Kind::Shell => (pos2(150.0, 300.0), vec2(600.0, 280.0)),
                Kind::Terminal => (pos2(170.0, 40.0), vec2(720.0, 440.0)),
                Kind::System => (pos2(200.0, 40.0), vec2(700.0, 500.0)),
                Kind::Rig => (pos2(170.0, 30.0), book_window()),
                Kind::HexEd => (pos2(130.0, 36.0), vec2(800.0, 500.0)),
                Kind::Qwk => (pos2(190.0, 46.0), vec2(700.0, 470.0)),
                Kind::IceDb => (pos2(220.0, 60.0), vec2(700.0, 470.0)),
                Kind::NightComm => (pos2(150.0, 50.0), vec2(880.0, 500.0)),
                Kind::TigerTeam => (pos2(140.0, 44.0), vec2(900.0, 540.0)),
                Kind::Guide => (pos2(200.0, 30.0), book_window()),
                Kind::Requester(_) => (pos2(260.0, 150.0), vec2(330.0, 150.0)),
            };
            // A tool's window has the proportional gadget down its right side, beside the
            // tool's room (not the Shell's, which scrolls its own lines, or a requester's).
            let size = match kind {
                Kind::Shell | Kind::Requester(_) => size,
                _ => size + vec2(kit::size::PROP_W, 0.0),
            };
            self.wins.push(Win { kind, pos, size });
        }
    }
}

impl DesktopView for Workbench {
    /// Draws the whole screen into `ui`, dragged down by `offset` pixels.
    /// Only takes input when `live`.
    fn show(&mut self, ui: &mut Ui, home: &mut Home, cx: &Cx, out: &mut Vec<WbOut>) {
        let (data, save, offset, live) = (cx.data, cx.save, cx.offset, cx.live);
        home.prompt = "1.Desktop:> ";
        for t in home.take_opens() {
            self.open(t);
        }
        // The Welcome stack is the Sense/Net's: this desktop reaches it with `help net`.
        let icons: Vec<_> = data
            .icons
            .iter()
            .filter(|i| save.opens(i.tool) && i.tool != Tool::Guide)
            .collect();
        let screen = ui.ctx().content_rect().translate(vec2(0.0, offset));
        let painter = ui.painter_at(screen);
        let look = look();
        let at = |p: Pos2| screen.min + p.to_vec2();

        // Screen and its title bar.
        painter.rect_filled(screen, 0.0, backdrop());
        let addon = |e| save.has_effect(data, Desktop::Workbench, e);
        if addon(AddonEffect::Pattern) {
            // The classic dithered backdrop: every other pixel, every other row.
            let mut y = screen.top() + kit::size::SCREEN_BAR_H;
            let mut row = 0;
            while y < screen.bottom() {
                let mut x = screen.left() + if row % 2 == 0 { 0.0 } else { 3.0 };
                while x < screen.right() {
                    painter.rect_filled(
                        Rect::from_min_size(pos2(x, y), vec2(1.5, 1.5)),
                        0.0,
                        fill(),
                    );
                    x += 6.0;
                }
                y += 3.0;
                row += 1;
            }
        }
        let title = format!(
            "ninsei Desktop 1.3   runs {}  exits {}  flatlines {}   best {}¢",
            save.runs, save.exits, save.flatlines, save.best_haul
        );
        let clock = addon(AddonEffect::Clock).then(|| {
            let up = ui.input(|i| i.time) as u64;
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_secs(1));
            format!(
                "bank {}¢   {:02}:{:02}:{:02}",
                save.bank,
                up / 3600,
                up / 60 % 60,
                up % 60
            )
        });
        kit::screen_bar(&painter, screen, &title, clock.as_deref(), &look);

        // Background click deselects icons.
        let bg = ui.interact(
            screen,
            Id::new("wb-bg"),
            if live { Sense::click() } else { Sense::hover() },
        );
        if bg.clicked() {
            self.selected = None;
        }

        // Icons down the left, like disks on a desk.
        let now = ui.input(|i| i.time);
        let screen_size = screen.size();
        for (n, icon) in icons.iter().enumerate() {
            let w = icon.width() as f32 * ICON_SCALE;
            let h = icon.pixels.len() as f32 * ICON_SCALE;
            let cell =
                Rect::from_min_size(at(icon_pos(n, screen_size)), vec2(84.0, ICON_PITCH - 6.0));
            let art = Rect::from_min_size(pos2(cell.center().x - w / 2.0, cell.top()), vec2(w, h));
            let selected = self.selected == Some(icon.tool);
            draw_icon(&painter, icon, art.min, selected);
            let label = Rect::from_center_size(
                pos2(cell.center().x, art.bottom() + 10.0),
                vec2(cell.width(), 16.0),
            );
            if selected {
                // Highlight exactly as wide as the name, however long it is.
                let w = painter
                    .layout_no_wrap(icon.label.clone(), font(), backdrop())
                    .size()
                    .x;
                painter.rect_filled(
                    Rect::from_center_size(label.center(), vec2(w + 8.0, label.height())),
                    0.0,
                    ink(),
                );
            }
            painter.text(
                label.center(),
                Align2::CENTER_CENTER,
                &icon.label,
                font(),
                if selected { backdrop() } else { ink() },
            );
            let hit = art.union(label);
            let r = ui.interact(
                hit,
                Id::new(("wb-icon", n)),
                if live { Sense::click() } else { Sense::hover() },
            );
            if r.clicked() || r.double_clicked() {
                // Two clicks on the same icon in quick succession (ours), or egui's double-click
                // (both clicks can land in one frame on a slow machine).
                let double = r.double_clicked()
                    || matches!(self.last_click, Some((t, at)) if t == icon.tool && now - at < DOUBLE_CLICK_SECS);
                self.selected = Some(icon.tool);
                self.last_click = Some((icon.tool, now));
                if double {
                    self.last_click = None;
                    self.open(icon.tool);
                }
            }
        }

        // A tool taller than its window scrolls with the proportional gadget.
        let show =
            |ui: &mut Ui, id: Id, body: &mut dyn FnMut(&mut Ui)| kit::scrolled(ui, id, &look, body);
        let scroller = crate::home::Scroller {
            width: kit::SCROLLED_W,
            show: &show,
        };

        // Windows, back to front.
        let mut raise = None;
        let mut close = None;
        let n = self.wins.len();
        // Where the mouse went down this frame: whichever window is on top there comes forward,
        // even if a widget inside it (the Shell's text, a button) took the click.
        let press = ui.input(|i| {
            i.pointer
                .primary_pressed()
                .then(|| i.pointer.interact_pos())
                .flatten()
        });
        let mut rects = Vec::with_capacity(n);
        // And where it came up: a click that ends on the Shell gives it the keyboard (a press
        // alone isn't enough, since releasing over empty space would take the focus away again).
        let release = ui.input(|i| {
            i.pointer
                .primary_released()
                .then(|| i.pointer.interact_pos())
                .flatten()
        });
        for i in 0..n {
            let top = i + 1 == n;
            let (kind, pos, size) = (self.wins[i].kind, self.wins[i].pos, self.wins[i].size);
            // A window never runs off the bottom of the screen: it gets shorter and scrolls.
            let room = (screen.height() - pos.y - WIN_MARGIN).max(WIN_MIN_H);
            let rect = Rect::from_min_size(at(pos), vec2(size.x, size.y.min(room)));
            let title = self.wins[i].title();
            let wid = Id::new(("wb-win", format!("{kind:?}")));
            // The whole window swallows clicks so icons under it don't fire.
            let body = ui.interact(
                rect,
                wid.with("body"),
                if live { Sense::click() } else { Sense::hover() },
            );
            if body.clicked() || body.drag_started() {
                raise = Some(i);
            }
            rects.push(rect);
            let g = kit::window_frame(&painter, rect, &title, top, &look);
            let (bar, close_r, depth_r) = (g.bar, g.close, g.depth);
            let sense = if live {
                Sense::click_and_drag()
            } else {
                Sense::hover()
            };
            let drag = ui.interact(bar, wid.with("bar"), sense);
            if drag.dragged() {
                let d = drag.drag_delta();
                let p = &mut self.wins[i].pos;
                *p += d;
                p.x = p.x.clamp(-size.x + 60.0, screen.width() - 60.0);
                p.y = p.y.clamp(
                    kit::size::SCREEN_BAR_H,
                    screen.height() - kit::size::TITLE_H,
                );
                // On whole pixels, where its frame is drawn.
                *p = p.round();
            }
            if drag.drag_started() || drag.clicked() {
                raise = Some(i);
            }
            if ui
                .interact(
                    close_r,
                    wid.with("close"),
                    if live { Sense::click() } else { Sense::hover() },
                )
                .clicked()
            {
                close = Some(i);
            }
            if ui
                .interact(
                    depth_r,
                    wid.with("depth"),
                    if live { Sense::click() } else { Sense::hover() },
                )
                .clicked()
            {
                // Depth gadget: top window goes to the back, anything else comes to the front.
                raise = if top { None } else { Some(i) };
                if top {
                    let w = self.wins.remove(i);
                    self.wins.insert(0, w);
                    break;
                }
            }
            let content = kit::window_content(rect);
            match kind {
                Kind::Shell => home.shell(ui, content, data, save, live && top, out),
                Kind::Rig
                | Kind::HexEd
                | Kind::Qwk
                | Kind::IceDb
                | Kind::NightComm
                | Kind::TigerTeam
                | Kind::Guide
                | Kind::Terminal
                | Kind::System => {
                    // To the border, so the proportional gadget sits on it.
                    let body = kit::window_scrolled(rect);
                    let mut child = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(body)
                            .layout(egui::Layout::top_down(egui::Align::Min)),
                    );
                    child.set_clip_rect(body);
                    hubtools::wb_visuals(child.visuals_mut());
                    if !live {
                        child.disable();
                    }
                    let tool = match kind {
                        Kind::Terminal => Tool::Terminal,
                        Kind::Rig => Tool::Rig,
                        Kind::System => Tool::System,
                        Kind::Qwk => Tool::Qwk,
                        Kind::IceDb => Tool::IceDb,
                        Kind::NightComm => Tool::NightComm,
                        Kind::TigerTeam => Tool::TigerTeam,
                        Kind::Guide => Tool::Guide,
                        _ => Tool::HexEd,
                    };
                    let hint = crate::home::drag_hint(pos.y, kit::size::SCREEN_BAR_H);
                    home.tool_in(
                        &mut child,
                        tool,
                        data,
                        save,
                        out,
                        hint,
                        Some(&scroller),
                        ("wb-scroll", tool),
                    );
                }
                Kind::Requester(tool) => {
                    let lines = [
                        ("Please insert volume", false),
                        (tool.volume(), true),
                        ("in any drive", false),
                    ];
                    let buttons = ["Retry", "Cancel"];
                    match kit::requester(ui, &painter, content, &lines, buttons, live, wid, &look) {
                        Some(0) => home.say(format!("{}: still not installed.", tool.volume())),
                        Some(_) => close = Some(i),
                        None => {}
                    }
                }
            }
        }
        if raise.is_none()
            && live
            && let Some(p) = press
        {
            raise = rects.iter().rposition(|r| r.contains(p));
        }
        if let Some(i) = close {
            if i < self.wins.len() {
                self.wins.remove(i);
            }
        } else if let Some(i) = raise
            && i < self.wins.len()
            && i + 1 != self.wins.len()
        {
            let w = self.wins.remove(i);
            self.wins.push(w);
        }
        // A Shell window that comes to the front (opened, clicked, raised) takes the keyboard.
        let top = self.wins.last().map(|w| w.kind);
        let clicked_shell = top == Some(Kind::Shell)
            && release.is_some_and(|p| rects.last().is_some_and(|r| r.contains(p)));
        if (top != self.top && top == Some(Kind::Shell)) || (live && clicked_shell) {
            home.focus();
        }
        self.top = top;
    }
}

/// Paints an icon's pixels. Selected icons draw in complement colours, as on the real thing.
fn draw_icon(painter: &egui::Painter, icon: &IconDef, at: Pos2, selected: bool) {
    for (y, row) in icon.pixels.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            let col = match (ch, selected) {
                ('w', false) | ('k', true) => ink(),
                ('k', false) | ('w', true) => shadow(),
                ('o', false) | ('b', true) => accent(),
                ('b', false) | ('o', true) => fill(),
                _ => continue,
            };
            let p = at + vec2(x as f32, y as f32) * ICON_SCALE;
            painter.rect_filled(Rect::from_min_size(p, Vec2::splat(ICON_SCALE)), 0.0, col);
        }
    }
}

/// A window whose body holds one book (`pager::SIZE`): the body is the window less its title
/// bar, border and margins (`kit::window_content`), plus a pixel of slack. The proportional
/// gadget's room is added in `open`, as for every tool window.
fn book_window() -> egui::Vec2 {
    let side = kit::size::BORDER + kit::size::PAD;
    crate::pager::SIZE
        + vec2(
            2.0 * side + 2.0,
            kit::size::TITLE_H + kit::size::PAD_TOP + side + 2.0,
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::home::{Cx, Home};
    use egui::{Event, PointerButton, RawInput, Rect};
    use ninsei_core::Data;
    use ninsei_core::hub::Save;

    /// Runs one frame of the Workbench with these input events at time `t`.
    fn frame(
        ctx: &egui::Context,
        wb: &mut Workbench,
        home: &mut Home,
        data: &Data,
        save: &Save,
        events: Vec<Event>,
        t: f64,
    ) {
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1280.0, 800.0))),
            time: Some(t),
            events,
            ..RawInput::default()
        };
        let mut output = ctx.run_ui(raw, |ui| {
            let cx = Cx {
                data,
                save,
                offset: 0.0,
                live: true,
            };
            let mut out = Vec::new();
            wb.show(ui, home, &cx, &mut out);
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

    #[test]
    fn double_clicking_an_icon_raises_its_window() {
        let data = Data::builtin().unwrap();
        let save = Save {
            tools: ninsei_core::data::TOOLS
                .iter()
                .map(|t| t.to_string())
                .collect(),
            ..Save::default()
        };
        let ctx = egui::Context::default();
        let mut wb = Workbench::new();
        let mut home = Home::new(Vec::new());
        // Every icon this desktop shows, with another window opened over its own.
        let icons: Vec<_> = data
            .icons
            .iter()
            .filter(|i| i.tool != Tool::Guide)
            .collect();
        for (n, icon) in icons.iter().enumerate() {
            wb.open(icon.tool);
            let other = icons[(n + 1) % icons.len()].tool;
            wb.open(other);
            let at = icon_pos(n, ctx.content_rect().size()) + vec2(42.0, 20.0);
            let t = n as f64 * 10.0;
            frame(&ctx, &mut wb, &mut home, &data, &save, Vec::new(), t);
            if n % 2 == 0 {
                frame(&ctx, &mut wb, &mut home, &data, &save, click(at), t + 0.1);
                frame(&ctx, &mut wb, &mut home, &data, &save, click(at), t + 0.2);
            } else {
                // A slow frame: both clicks arrive at once.
                let mut both = click(at);
                both.extend(click(at));
                frame(&ctx, &mut wb, &mut home, &data, &save, both, t + 0.1);
            }
            frame(&ctx, &mut wb, &mut home, &data, &save, Vec::new(), t + 0.3);
            let top = wb.wins.last().map(|w| w.kind);
            assert_eq!(
                top.map(|k| format!("{k:?}")),
                Some(format!("{:?}", wb_kind(icon.tool))),
                "{} didn't come to the front",
                icon.label
            );
        }
    }

    #[test]
    fn clicking_inside_a_window_raises_it() {
        let data = Data::builtin().unwrap();
        let save = Save::default();
        let ctx = egui::Context::default();
        let mut wb = Workbench::new();
        let mut home = Home::new(Vec::new());
        wb.open(Tool::Shell);
        wb.open(Tool::System);
        frame(&ctx, &mut wb, &mut home, &data, &save, Vec::new(), 0.0);
        // The Shell's left strip, under its title bar, peeks out from behind System.
        let shell = wb.wins.iter().find(|w| w.kind == Kind::Shell).unwrap();
        let at = shell.pos + vec2(20.0, 150.0);
        frame(&ctx, &mut wb, &mut home, &data, &save, click(at), 0.5);
        frame(&ctx, &mut wb, &mut home, &data, &save, Vec::new(), 1.0);
        assert_eq!(wb.wins.last().map(|w| w.kind == Kind::Shell), Some(true));
    }

    fn wb_kind(t: Tool) -> Kind {
        match t {
            Tool::Terminal => Kind::Terminal,
            Tool::System => Kind::System,
            Tool::Shell => Kind::Shell,
            Tool::Rig => Kind::Rig,
            Tool::HexEd => Kind::HexEd,
            Tool::Qwk => Kind::Qwk,
            Tool::IceDb => Kind::IceDb,
            Tool::NightComm => Kind::NightComm,
            Tool::TigerTeam => Kind::TigerTeam,
            Tool::Guide => Kind::Guide,
        }
    }

    #[test]
    fn every_tool_window_opens_big_enough_for_its_tool() {
        let data = Data::builtin().unwrap();
        let save = Save {
            tools: ninsei_core::data::TOOLS
                .iter()
                .map(|t| t.to_string())
                .collect(),
            guide_seen: true,
            intros: false,
            ..Save::default()
        };
        let ctx = egui::Context::default();
        crate::uitest::themed(&ctx, &data);
        let (mut wb, mut home) = (Workbench::new(), Home::new(Vec::new()));
        let mut t = 0.0;
        for tool in Tool::ALL {
            wb.open(tool);
            for _ in 0..2 {
                t += 0.1;
                frame(&ctx, &mut wb, &mut home, &data, &save, Vec::new(), t);
            }
        }
        assert!(home.notices.is_empty(), "too small: {:?}", home.notices);
    }

    #[test]
    fn the_close_and_depth_gadgets_work_where_the_frame_draws_them() {
        let data = Data::builtin().unwrap();
        let save = Save::default();
        let ctx = egui::Context::default();
        let (mut wb, mut home) = (Workbench::new(), Home::new(Vec::new()));
        wb.open(Tool::Shell);
        wb.open(Tool::System);
        let mut t = 0.0;
        let mut run = |wb: &mut Workbench, home: &mut Home, ev| {
            t += 0.5;
            frame(&ctx, wb, home, &data, &save, ev, t);
        };
        run(&mut wb, &mut home, Vec::new());
        let at = |wb: &Workbench, k| {
            let w = wb.wins.iter().find(|w| w.kind == k).unwrap();
            kit::gadgets(Rect::from_min_size(w.pos, w.size))
        };
        // The front window's depth gadget sends it to the back.
        let front = wb.wins.last().unwrap().kind;
        let back = wb.wins[0].kind;
        let depth = at(&wb, front).depth.center();
        run(&mut wb, &mut home, click(depth));
        run(&mut wb, &mut home, Vec::new());
        assert_eq!(wb.wins.last().map(|w| w.kind), Some(back));
        assert_eq!(wb.wins[0].kind, front);
        // Its close gadget closes it.
        let close = at(&wb, back).close.center();
        run(&mut wb, &mut home, click(close));
        run(&mut wb, &mut home, Vec::new());
        assert!(wb.wins.iter().all(|w| w.kind != back));
    }

    fn all_tools() -> Save {
        Save {
            tools: ninsei_core::data::TOOLS
                .iter()
                .map(|t| t.to_string())
                .collect(),
            guide_seen: true,
            intros: false,
            ..Save::default()
        }
    }

    fn scroll_of(ctx: &egui::Context, tool: Tool) -> crate::kit::scrollbar::Scroll {
        let key = Id::new(("wb-scroll", tool)).with("kit-scroll");
        ctx.data(|d| d.get_temp(key)).unwrap()
    }

    #[test]
    fn a_short_window_scrolls_with_the_proportional_gadget() {
        let data = Data::builtin().unwrap();
        let save = all_tools();
        let ctx = egui::Context::default();
        crate::uitest::themed(&ctx, &data);
        let (mut wb, mut home) = (Workbench::new(), Home::new(Vec::new()));
        let mut t = 0.0;
        let mut run = |wb: &mut Workbench, home: &mut Home, ev| {
            t += 0.5;
            frame(&ctx, wb, home, &data, &save, ev, t);
        };
        run(&mut wb, &mut home, Vec::new());
        wb.open(Tool::System);
        // Low on the screen the window is its shortest: System runs past it.
        wb.wins.last_mut().unwrap().pos.y = 680.0;
        for _ in 0..3 {
            run(&mut wb, &mut home, Vec::new());
        }
        let s = scroll_of(&ctx, Tool::System);
        assert!(s.active(), "{s:?}");
        let w = wb.wins.last().unwrap();
        let room = (800.0 - w.pos.y - WIN_MARGIN).max(WIN_MIN_H);
        let rect = Rect::from_min_size(w.pos, vec2(w.size.x, w.size.y.min(room)));
        let body = kit::window_scrolled(rect);
        // A click on the container under the knob scrolls a page.
        let below = pos2(body.right() - kit::size::PROP_W / 2.0, body.bottom() - 3.0);
        run(&mut wb, &mut home, click(below));
        let paged = scroll_of(&ctx, Tool::System);
        assert!(paged.offset > s.offset, "{paged:?}");
    }

    #[test]
    fn a_book_has_no_scroll_gadget_at_its_own_size() {
        let data = Data::builtin().unwrap();
        let save = all_tools();
        let ctx = egui::Context::default();
        crate::uitest::themed(&ctx, &data);
        let (mut wb, mut home) = (Workbench::new(), Home::new(Vec::new()));
        let mut t = 0.0;
        for tool in [Tool::Rig, Tool::Guide] {
            wb.open(tool);
            for _ in 0..3 {
                t += 0.5;
                frame(&ctx, &mut wb, &mut home, &data, &save, Vec::new(), t);
            }
            let s = scroll_of(&ctx, tool);
            assert!(!s.active(), "{tool:?}'s page fits its window: {s:?}");
        }
    }

    #[test]
    fn every_tool_window_shows_its_tool_whole_beside_the_gadget() {
        let data = Data::builtin().unwrap();
        let save = all_tools();
        for tool in Tool::ALL {
            let mut h = crate::uitest::Harness::new(1280.0, 800.0);
            crate::uitest::themed(&h.ctx, &data);
            let (mut wb, mut home) = (Workbench::new(), Home::new(Vec::new()));
            wb.open(tool);
            for _ in 0..3 {
                h.frame(Vec::new(), |ui| {
                    let cx = Cx {
                        data: &data,
                        save: &save,
                        offset: 0.0,
                        live: true,
                    };
                    wb.show(ui, &mut home, &cx, &mut Vec::new());
                });
            }
            let cut: Vec<_> = h
                .cut_text()
                .into_iter()
                .filter(|(_, r)| r.bottom() < 780.0)
                .collect();
            assert!(
                cut.is_empty(),
                "{tool:?} is cut at its window's edge: {cut:?}"
            );
        }
    }

    #[test]
    fn the_wheel_over_a_window_in_front_doesnt_scroll_the_one_behind() {
        let data = Data::builtin().unwrap();
        let save = all_tools();
        let ctx = egui::Context::default();
        crate::uitest::themed(&ctx, &data);
        let (mut wb, mut home) = (Workbench::new(), Home::new(Vec::new()));
        let mut t = 0.0;
        let mut run = |wb: &mut Workbench, home: &mut Home, ev| {
            t += 0.5;
            frame(&ctx, wb, home, &data, &save, ev, t);
        };
        run(&mut wb, &mut home, Vec::new());
        // System low on the screen, so it scrolls; ICE DB in front, over System's gadget.
        wb.open(Tool::System);
        wb.wins.last_mut().unwrap().pos = pos2(100.0, 680.0);
        wb.open(Tool::IceDb);
        wb.wins.last_mut().unwrap().pos = pos2(700.0, 600.0);
        for _ in 0..3 {
            run(&mut wb, &mut home, Vec::new());
        }
        assert!(scroll_of(&ctx, Tool::System).active());
        let sys = wb.wins.iter().find(|w| w.kind == Kind::System).unwrap();
        let over = pos2(sys.pos.x + sys.size.x - 10.0, 760.0);
        let wheel = vec![
            Event::PointerMoved(over),
            Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: vec2(0.0, -40.0),
                modifiers: egui::Modifiers::NONE,
                phase: egui::TouchPhase::Move,
            },
        ];
        run(&mut wb, &mut home, wheel);
        for _ in 0..10 {
            run(&mut wb, &mut home, Vec::new());
        }
        assert_eq!(scroll_of(&ctx, Tool::System).offset, 0.0);
    }
}
