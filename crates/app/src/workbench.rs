//! The Workbench 1.3 desktop (the Hosaka's): pixel-art icons, draggable windows, and
//! "insert volume" requesters for tools that aren't installed yet. A front-end on `home`,
//! which holds the Shell and the tools' state. All drawn by hand into one area so the whole
//! screen can be dragged off a run.

use egui::{Align2, Id, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use ninsei_core::desktop::AddonEffect;
use ninsei_core::hardware::Desktop;
use ninsei_core::hub::{IconDef, Tool};

use crate::home::{Cx, DesktopView, Home, WbOut, accent, backdrop, fill, font, ink, shadow};
use crate::hubtools;

const TITLE_H: f32 = 20.0;
/// Gap kept between a window's bottom and the screen's.
const WIN_MARGIN: f32 = 8.0;
/// Shortest a window gets when the screen is small.
const WIN_MIN_H: f32 = 120.0;
const SCREEN_BAR_H: f32 = 22.0;
/// Screen pixels per icon pixel.
const ICON_SCALE: f32 = 3.0;
const ICON_PITCH: f32 = 84.0;
const BORDER: f32 = 2.0;
/// Seconds between clicks that still count as a double-click.
const DOUBLE_CLICK_SECS: f64 = 0.45;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Shell,
    /// CarrierTerm: the phonebook.
    Terminal,
    Prefs,
    Rig,
    Stash,
    HexEd,
    Qwk,
    IceDb,
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
            Kind::Prefs => "Prefs".into(),
            Kind::Terminal => "CarrierTerm 1.0: Dialing Directory".into(),
            Kind::Rig => "RIG: deck shop".into(),
            Kind::Stash => "STASH".into(),
            Kind::HexEd => "HexEd".into(),
            Kind::Qwk => "OLX QWK Reader".into(),
            Kind::IceDb => "ICE DB".into(),
            Kind::Guide => "Welcome to the Net".into(),
            Kind::Requester(_) => "System Request".into(),
        }
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
            Tool::Prefs => Kind::Prefs,
            Tool::Shell => Kind::Shell,
            Tool::Rig => Kind::Rig,
            Tool::Stash => Kind::Stash,
            Tool::HexEd => Kind::HexEd,
            Tool::Qwk => Kind::Qwk,
            Tool::IceDb => Kind::IceDb,
            Tool::Guide => Kind::Guide,
        };
        if let Some(i) = self.wins.iter().position(|w| w.kind == kind) {
            let w = self.wins.remove(i);
            self.wins.push(w);
        } else {
            let (pos, size) = match kind {
                Kind::Shell => (pos2(150.0, 300.0), vec2(600.0, 280.0)),
                Kind::Terminal => (pos2(170.0, 40.0), vec2(720.0, 440.0)),
                Kind::Prefs => (pos2(260.0, 40.0), vec2(560.0, 600.0)),
                Kind::Rig => (pos2(170.0, 50.0), vec2(620.0, 330.0)),
                Kind::Stash => (pos2(210.0, 80.0), vec2(640.0, 380.0)),
                Kind::HexEd => (pos2(130.0, 36.0), vec2(800.0, 500.0)),
                Kind::Qwk => (pos2(190.0, 46.0), vec2(700.0, 470.0)),
                Kind::IceDb => (pos2(220.0, 60.0), vec2(700.0, 470.0)),
                Kind::Guide => (pos2(240.0, 70.0), vec2(640.0, 440.0)),
                Kind::Requester(_) => (pos2(260.0, 150.0), vec2(330.0, 150.0)),
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
        let at = |p: Pos2| screen.min + p.to_vec2();

        // Screen and its title bar.
        painter.rect_filled(screen, 0.0, backdrop());
        let addon = |e| save.has_effect(data, Desktop::Workbench, e);
        if addon(AddonEffect::Pattern) {
            // The classic dithered backdrop: every other pixel, every other row.
            let mut y = screen.top() + SCREEN_BAR_H;
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
        let bar = Rect::from_min_size(screen.min, vec2(screen.width(), SCREEN_BAR_H));
        painter.rect_filled(bar, 0.0, ink());
        painter.line_segment(
            [bar.left_bottom(), bar.right_bottom()],
            Stroke::new(1.0, shadow()),
        );
        let title = format!(
            "ninsei Desktop 1.3   runs {}  exits {}  flatlines {}   best {}¢",
            save.runs, save.exits, save.flatlines, save.best_haul
        );
        painter.text(
            bar.left_center() + vec2(8.0, 0.0),
            Align2::LEFT_CENTER,
            title,
            font(),
            backdrop(),
        );
        if addon(AddonEffect::Clock) {
            let up = ui.input(|i| i.time) as u64;
            painter.text(
                bar.right_center() - vec2(36.0, 0.0),
                Align2::RIGHT_CENTER,
                format!(
                    "bank {}¢   {:02}:{:02}:{:02}",
                    save.bank,
                    up / 3600,
                    up / 60 % 60,
                    up % 60
                ),
                font(),
                backdrop(),
            );
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_secs(1));
        }
        depth_gadget(
            &painter,
            Rect::from_min_size(bar.right_top() + vec2(-26.0, 3.0), vec2(22.0, 16.0)),
        );

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
        for (n, icon) in icons.iter().enumerate() {
            let w = icon.width() as f32 * ICON_SCALE;
            let h = icon.pixels.len() as f32 * ICON_SCALE;
            let cell = Rect::from_min_size(
                at(pos2(18.0, SCREEN_BAR_H + 16.0 + n as f32 * ICON_PITCH)),
                vec2(84.0, ICON_PITCH - 6.0),
            );
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
            window_frame(&painter, rect, &title, top);
            let bar = Rect::from_min_size(rect.min, vec2(rect.width(), TITLE_H));
            let close_r = Rect::from_min_size(bar.min + vec2(4.0, 3.0), vec2(16.0, 14.0));
            let depth_r = Rect::from_min_size(bar.right_top() + vec2(-26.0, 2.0), vec2(22.0, 16.0));
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
                p.y = p.y.clamp(SCREEN_BAR_H, screen.height() - TITLE_H);
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
            let content = Rect::from_min_max(
                bar.left_bottom() + vec2(BORDER + 4.0, 4.0),
                rect.max - vec2(BORDER + 4.0, BORDER + 4.0),
            );
            match kind {
                Kind::Shell => home.shell(ui, content, data, save, live && top, out),
                Kind::Rig
                | Kind::Stash
                | Kind::HexEd
                | Kind::Qwk
                | Kind::IceDb
                | Kind::Guide
                | Kind::Terminal
                | Kind::Prefs => {
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
                    let tool = match kind {
                        Kind::Terminal => Tool::Terminal,
                        Kind::Rig => Tool::Rig,
                        Kind::Prefs => Tool::Prefs,
                        Kind::Stash => Tool::Stash,
                        Kind::Qwk => Tool::Qwk,
                        Kind::IceDb => Tool::IceDb,
                        Kind::Guide => Tool::Guide,
                        _ => Tool::HexEd,
                    };
                    crate::home::scrolled(&mut child, ("wb-scroll", tool), |ui| {
                        home.tool_body(ui, tool, data, save, out)
                    });
                }
                Kind::Requester(tool) => {
                    if let Some(action) = requester(ui, &painter, content, tool, live, wid) {
                        match action {
                            Req::Retry => {
                                home.say(format!("{}: still not installed.", tool.volume()))
                            }
                            Req::Cancel => close = Some(i),
                        }
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

enum Req {
    Retry,
    Cancel,
}

fn requester(
    ui: &mut Ui,
    painter: &egui::Painter,
    rect: Rect,
    tool: Tool,
    live: bool,
    wid: Id,
) -> Option<Req> {
    let lines = ["Please insert volume", tool.volume(), "in any drive"];
    for (k, l) in lines.iter().enumerate() {
        let col = if k == 1 { accent() } else { ink() };
        painter.text(
            pos2(rect.center().x, rect.top() + 6.0 + k as f32 * 18.0),
            Align2::CENTER_TOP,
            *l,
            font(),
            col,
        );
    }
    let bw = vec2(84.0, 22.0);
    let retry = Rect::from_min_size(pos2(rect.left() + 6.0, rect.bottom() - bw.y), bw);
    let cancel = Rect::from_min_size(pos2(rect.right() - 6.0 - bw.x, rect.bottom() - bw.y), bw);
    let mut out = None;
    for (r, label, what) in [(retry, "Retry", 0), (cancel, "Cancel", 1)] {
        let resp = ui.interact(
            r,
            wid.with(label),
            if live { Sense::click() } else { Sense::hover() },
        );
        let pressed = resp.is_pointer_button_down_on();
        button(painter, r, label, pressed);
        if resp.clicked() {
            out = Some(if what == 0 { Req::Retry } else { Req::Cancel });
        }
    }
    out
}

/// A 1.3 push button: white box, black shadow edges, inverted while pressed.
fn button(painter: &egui::Painter, r: Rect, label: &str, pressed: bool) {
    let (face, text) = if pressed {
        (accent(), shadow())
    } else {
        (backdrop(), ink())
    };
    painter.rect_filled(r, 0.0, face);
    painter.line_segment(
        [r.left_top(), r.right_top()],
        Stroke::new(2.0, if pressed { shadow() } else { ink() }),
    );
    painter.line_segment(
        [r.left_top(), r.left_bottom()],
        Stroke::new(2.0, if pressed { shadow() } else { ink() }),
    );
    painter.line_segment(
        [r.left_bottom(), r.right_bottom()],
        Stroke::new(2.0, if pressed { ink() } else { shadow() }),
    );
    painter.line_segment(
        [r.right_top(), r.right_bottom()],
        Stroke::new(2.0, if pressed { ink() } else { shadow() }),
    );
    painter.text(r.center(), Align2::CENTER_CENTER, label, font(), text);
}

/// Window chrome: white border, a title bar (striped when active), close and depth gadgets.
fn window_frame(painter: &egui::Painter, rect: Rect, title: &str, active: bool) {
    painter.rect_filled(rect, 0.0, backdrop());
    painter.rect_stroke(rect, 0.0, Stroke::new(BORDER, ink()), StrokeKind::Inside);
    let bar = Rect::from_min_size(rect.min, vec2(rect.width(), TITLE_H));
    painter.rect_filled(bar, 0.0, if active { ink() } else { backdrop() });
    if active {
        // Horizontal pinstripes behind the title, the active-window tell.
        let mut y = bar.top() + 4.0;
        while y < bar.bottom() - 3.0 {
            painter.line_segment(
                [pos2(bar.left() + 24.0, y), pos2(bar.right() - 30.0, y)],
                Stroke::new(1.0, backdrop()),
            );
            y += 3.0;
        }
    }
    painter.rect_stroke(bar, 0.0, Stroke::new(BORDER, ink()), StrokeKind::Inside);
    let galley = painter.layout_no_wrap(
        title.to_string(),
        font(),
        if active { backdrop() } else { ink() },
    );
    let tw = galley.size().x + 10.0;
    let tbox = Rect::from_min_size(bar.min + vec2(26.0, 2.0), vec2(tw, TITLE_H - 4.0));
    painter.rect_filled(tbox, 0.0, if active { ink() } else { backdrop() });
    painter.galley(tbox.min + vec2(5.0, 1.0), galley, ink());
    // Close gadget: a box with a dot.
    let c = Rect::from_min_size(bar.min + vec2(4.0, 3.0), vec2(16.0, 14.0));
    painter.rect_filled(c, 0.0, ink());
    painter.rect_stroke(c, 0.0, Stroke::new(1.0, shadow()), StrokeKind::Inside);
    painter.rect_filled(
        Rect::from_center_size(c.center(), vec2(4.0, 4.0)),
        0.0,
        shadow(),
    );
    depth_gadget(
        painter,
        Rect::from_min_size(bar.right_top() + vec2(-26.0, 2.0), vec2(22.0, 16.0)),
    );
}

/// Two overlapping squares: back one outlined, front one filled.
fn depth_gadget(painter: &egui::Painter, r: Rect) {
    painter.rect_filled(r, 0.0, ink());
    let back = Rect::from_min_size(r.min + vec2(3.0, 2.0), vec2(11.0, 8.0));
    let front = Rect::from_min_size(r.min + vec2(8.0, 6.0), vec2(11.0, 8.0));
    painter.rect_stroke(back, 0.0, Stroke::new(1.0, shadow()), StrokeKind::Inside);
    painter.rect_filled(front, 0.0, accent());
    painter.rect_stroke(front, 0.0, Stroke::new(1.0, shadow()), StrokeKind::Inside);
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
            let at = pos2(60.0, SCREEN_BAR_H + 16.0 + n as f32 * ICON_PITCH + 20.0);
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
        wb.open(Tool::Stash);
        frame(&ctx, &mut wb, &mut home, &data, &save, Vec::new(), 0.0);
        // The Shell's left strip, under its title bar, peeks out from behind STASH.
        let shell = wb.wins.iter().find(|w| w.kind == Kind::Shell).unwrap();
        let at = shell.pos + vec2(20.0, 150.0);
        frame(&ctx, &mut wb, &mut home, &data, &save, click(at), 0.5);
        frame(&ctx, &mut wb, &mut home, &data, &save, Vec::new(), 1.0);
        assert_eq!(wb.wins.last().map(|w| w.kind == Kind::Shell), Some(true));
    }

    fn wb_kind(t: Tool) -> Kind {
        match t {
            Tool::Terminal => Kind::Terminal,
            Tool::Prefs => Kind::Prefs,
            Tool::Shell => Kind::Shell,
            Tool::Rig => Kind::Rig,
            Tool::Stash => Kind::Stash,
            Tool::HexEd => Kind::HexEd,
            Tool::Qwk => Kind::Qwk,
            Tool::IceDb => Kind::IceDb,
            Tool::Guide => Kind::Guide,
        }
    }
}
