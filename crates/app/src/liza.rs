//! Liza, the Sense/Net deck's desktop: after the first Mac and the Lisa. A 1-bit menu bar with
//! the tools as menus, Finder windows with striped title bars and a close box, icons down the
//! right, a Trash, and a happy deck at boot. Everything is two theme colours (ink on paper) and
//! patterns from `data/patterns.ron`; a front-end on `home`, like the other desktops.

use egui::{Align2, Color32, FontFamily, FontId, Id, Pos2, Rect, Sense, Stroke, StrokeKind, Ui};
use egui::{Vec2, pos2, vec2};
use ninsei_core::Data;
use ninsei_core::desktop::AddonEffect;
use ninsei_core::hardware::Desktop;
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
    /// Room kept clear at the foot of the icon column for the Trash.
    pub const TRASH_ROOM: f32 = 96.0;
    pub const BOOT_SCALE: f32 = 5.0;
    /// How long the boot card stays up, unless clicked away.
    pub const BOOT_SECS: f64 = 1.8;
    pub const DOUBLE_CLICK_SECS: f64 = 0.45;
    pub const MENU_ITEM_H: f32 = 22.0;
    pub const MENU_W: f32 = 190.0;
    /// The desktop's pattern (calm on a lit-ink screen; the 50% checker is the boot card's).
    pub const DESKTOP: &str = "p25";
    /// The desktop pattern add-on's pattern.
    pub const DESKTOP_ADDON: &str = "bricks";
    /// Windows open at once without MultiFinder.
    pub const WINDOW_LIMIT: usize = 3;
    /// The Calculator desk accessory's window.
    pub const CALC_POS: (f32, f32) = (24.0, 40.0);
    pub const CALC_SIZE: (f32, f32) = (330.0, 150.0);
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
    /// Opens or closes the Calculator desk accessory.
    Calculator,
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
    (
        "Net",
        &[
            ("Phonebook", Act::Open(Tool::Terminal)),
            ("QWK Mail", Act::Open(Tool::Qwk)),
            ("NiGHTCOMM", Act::Open(Tool::NightComm)),
            ("Tiger Team Pro", Act::Open(Tool::TigerTeam)),
        ],
    ),
    (
        "Deck",
        &[
            ("CPU Shopper", Act::Open(Tool::Rig)),
            ("HexEd", Act::Open(Tool::HexEd)),
            ("ICE DB", Act::Open(Tool::IceDb)),
        ],
    ),
    (
        "Special",
        &[
            ("Welcome to the Net", Act::Open(Tool::Guide)),
            ("System", Act::Open(Tool::System)),
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

/// Where the `n`th desktop icon sits: down the right edge, and when a column is full (the
/// Trash keeps the bottom corner) a new column to its left, the way the Finder lines them up.
fn icon_cell(screen: Rect, n: usize) -> Rect {
    let top = screen.top() + look::MENU_H + 14.0;
    let room = screen.bottom() - top - look::TRASH_ROOM;
    let per_col = ((room / look::ICON_PITCH).floor() as usize).max(1);
    let (col, row) = (n / per_col, n % per_col);
    Rect::from_min_size(
        pos2(
            screen.right() - look::ICON_COL_W * (col + 1) as f32,
            top + row as f32 * look::ICON_PITCH,
        ),
        vec2(look::ICON_COL_W, look::ICON_PITCH - 6.0),
    )
}

fn title(tool: Tool) -> &'static str {
    match tool {
        Tool::Qwk => "QWK Reader",
        Tool::NightComm => "NiGHTCOMM",
        Tool::TigerTeam => "Tiger Team Pro",
        Tool::IceDb => "ICE DB",
        Tool::Guide => "Welcome to the Net",
        Tool::Shell => "Shell",
        Tool::Terminal => "CarrierTerm",
        Tool::System => "System",
        Tool::Rig => "CPU Shopper",
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
    /// Every window stays open (the MultiFinder add-on).
    multifinder: bool,
    /// The Calculator desk accessory is open.
    calc: bool,
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
            multifinder: false,
            calc: false,
        }
    }

    /// Opens (or brings forward) a tool's window. A book (RIG, the Welcome stack) gets a window
    /// its fixed page fits exactly, so it never scrolls.
    fn open(&mut self, tool: Tool) {
        if let Some(i) = self.wins.iter().position(|w| w.tool == tool) {
            let w = self.wins.remove(i);
            self.wins.push(w);
            return;
        }
        let (pos, size) = match tool {
            Tool::Shell => (pos2(40.0, 330.0), vec2(620.0, 290.0)),
            Tool::Terminal => (pos2(60.0, 44.0), vec2(720.0, 440.0)),
            Tool::System => (pos2(100.0, 40.0), vec2(700.0, 500.0)),
            Tool::Rig => (pos2(80.0, 30.0), book_window()),
            Tool::HexEd => (pos2(40.0, 40.0), vec2(800.0, 500.0)),
            Tool::Qwk => (pos2(70.0, 50.0), vec2(700.0, 470.0)),
            Tool::NightComm => (pos2(60.0, 46.0), vec2(880.0, 500.0)),
            Tool::TigerTeam => (pos2(50.0, 40.0), vec2(900.0, 540.0)),
            Tool::IceDb => (pos2(100.0, 60.0), vec2(700.0, 470.0)),
            Tool::Guide => (pos2(120.0, 30.0), book_window()),
        };
        self.wins.push(Win { tool, pos, size });
        // Without MultiFinder, Liza keeps a few windows: the oldest goes.
        while !self.multifinder && self.wins.len() > look::WINDOW_LIMIT {
            self.wins.remove(0);
        }
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
            Act::Calculator => self.calc = !self.calc,
        }
    }
}

/// Liza's two colours: ink lit on a dark paper (lit ink), from the theme's text and its deepest
/// background.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Two {
    pub ink: Color32,
    pub paper: Color32,
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
        let addon = |e| save.has_effect(data, Desktop::Liza, e);
        self.multifinder = addon(AddonEffect::MultiFinder);
        for t in home.take_opens() {
            self.open(t);
        }
        // A new save's first boot opens the Welcome to the Net stack, once.
        if live && !save.guide_seen {
            self.open(Tool::Guide);
            out.push(WbOut::GuideSeen);
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
        let desktop = if addon(AddonEffect::Pattern) {
            look::DESKTOP_ADDON
        } else {
            look::DESKTOP
        };
        if let Some(p) = data.pattern(desktop) {
            patterns::fill(&painter, p, screen, ink, paper);
        }
        let bg = ui.interact(screen, Id::new("liza-bg"), sense(Sense::click()));
        if bg.clicked() {
            self.selected = None;
            self.menu = None;
        }

        // Icons down the right, then the Trash in the corner.
        for (n, icon) in data.icons.iter().filter(|i| save.opens(i.tool)).enumerate() {
            let cell = icon_cell(screen, n);
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
        let mut top_ui = on_top(ui, screen, egui::Order::Tooltip, "liza-menus");
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

        if self.calc && addon(AddonEffect::Calculator) {
            let mut da = on_top(ui, screen, egui::Order::Tooltip, "liza-calc");
            self.calculator(&mut da, cx, screen, c);
        }

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
        // Sense/Net's mark: an eye. With the Calculator it opens the desk accessory.
        let eye = bar.left_center() + vec2(18.0, 0.0);
        let calc = cx
            .save
            .has_effect(cx.data, Desktop::Liza, AddonEffect::Calculator);
        let mut picked = None;
        if calc {
            let r = Rect::from_center_size(eye, vec2(26.0, look::MENU_H - 2.0));
            let resp = ui
                .interact(r, Id::new("liza-eye"), sense(Sense::click()))
                .on_hover_text("Calculator");
            if self.calc {
                painter.rect_filled(r, 0.0, ink);
            }
            if resp.clicked() {
                picked = Some(Act::Calculator);
                self.menu = None;
            }
        }
        let (eye_ink, eye_paper) = if self.calc && calc {
            (paper, ink)
        } else {
            (ink, paper)
        };
        painter.circle_filled(eye, 6.0, eye_paper);
        painter.circle_stroke(eye, 6.0, Stroke::new(1.5, eye_ink));
        painter.circle_filled(eye, 2.5, eye_ink);
        let mut x = bar.left() + 40.0;
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
            // A tool you haven't downloaded isn't on the menu.
            let items: Vec<&(&str, Act)> = items
                .iter()
                .filter(|(_, a)| !matches!(a, Act::Open(t) if !cx.save.opens(*t)))
                .collect();
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
        // The bank on the right, and the time with the Alarm Clock.
        let up = ui.input(|i| i.time) as u64;
        let right = if cx
            .save
            .has_effect(cx.data, Desktop::Liza, AddonEffect::Clock)
        {
            format!(
                "{}¢    {:02}:{:02}",
                cx.save.bank,
                up / 3600 % 24,
                up / 60 % 60
            )
        } else {
            format!("{}¢", cx.save.bank)
        };
        painter.text(
            bar.right_center() - vec2(12.0, 0.0),
            Align2::RIGHT_CENTER,
            right,
            font(),
            ink,
        );
        picked
    }

    /// The Calculator desk accessory: a small window doing the sums you'd do on the back of an
    /// envelope before a run.
    fn calculator(&mut self, ui: &mut Ui, cx: &Cx, screen: Rect, c: Two) {
        let (save, data) = (cx.save, cx.data);
        let rect = Rect::from_min_size(
            screen.min + vec2(look::CALC_POS.0, look::MENU_H + look::CALC_POS.1),
            vec2(look::CALC_SIZE.0, look::CALC_SIZE.1),
        );
        let painter = ui.painter_at(rect.expand(look::SHADOW + 1.0));
        let close = window_frame(&painter, data, rect, "Calculator", true, c);
        if cx.live
            && ui
                .interact(close, Id::new("liza-calc-close"), Sense::click())
                .clicked()
        {
            self.calc = false;
        }
        let next = save.rig.next_upgrade(&data.rig);
        let heat: u32 = save.phonebook.iter().map(|e| e.heat).sum();
        let hottest = save.phonebook.iter().map(|e| e.heat).max().unwrap_or(0);
        let lines = [
            format!("Bank            {:>7}¢", save.bank),
            match next {
                Some((u, cost)) => {
                    let name = data
                        .rig
                        .iter()
                        .find(|d| d.kind == u)
                        .map_or("?", |d| d.label.as_str());
                    format!("Next: {name:<9} {cost:>7}¢")
                }
                None => "Next upgrade         none".to_string(),
            },
            match next {
                Some((_, cost)) if cost <= save.bank => {
                    format!("Left after      {:>7}¢", save.bank - cost)
                }
                Some((_, cost)) => format!("Short by        {:>7}¢", cost - save.bank),
                None => String::new(),
            },
            format!(
                "Heat  {heat} over {} numbers, hottest {hottest}",
                save.phonebook.len()
            ),
        ];
        let top = rect.top() + look::TITLE_H + 12.0;
        for (i, l) in lines.iter().enumerate() {
            painter.text(
                pos2(rect.left() + 14.0, top + i as f32 * 22.0),
                Align2::LEFT_TOP,
                l,
                font(),
                c.ink,
            );
        }
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
pub(crate) fn window_frame(
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

/// A window whose body holds one book (`pager::SIZE`): the body is the window less its title
/// bar and margins (see `content` in the window loop), plus a pixel of slack.
fn book_window() -> egui::Vec2 {
    crate::pager::SIZE + vec2(12.0 + 2.0, look::TITLE_H + 8.0 + 2.0)
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
        let save = Save {
            tools: ninsei_core::data::TOOLS
                .iter()
                .map(|t| t.to_string())
                .collect(),
            guide_seen: true,
            ..Save::default()
        };
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
        let screen = ctx.content_rect();
        for (n, icon) in data.icons.iter().enumerate() {
            let cell = icon_cell(screen, n);
            let at = pos2(cell.center().x, cell.top() + 20.0);
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
    fn liza_keeps_three_windows_unless_multifinder() {
        let mut liza = Liza::new();
        for t in [Tool::Shell, Tool::Terminal, Tool::Rig, Tool::HexEd] {
            liza.open(t);
        }
        assert_eq!(liza.wins.len(), look::WINDOW_LIMIT);
        assert!(
            liza.wins.iter().all(|w| w.tool != Tool::Shell),
            "the oldest went"
        );
        liza.multifinder = true;
        liza.open(Tool::System);
        liza.open(Tool::Qwk);
        assert_eq!(liza.wins.len(), 5);
    }

    #[test]
    fn clicking_the_eye_opens_the_calculator_once_owned() {
        let data = Data::builtin().unwrap();
        let ctx = egui::Context::default();
        let mut save = Save::default();
        save.addons.push("calculator".into());
        let (mut liza, mut home) = (Liza::new(), Home::new(Vec::new()));
        let run = |liza: &mut Liza, home: &mut Home, events, t| {
            let raw = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1280.0, 800.0))),
                time: Some(t),
                events,
                ..RawInput::default()
            };
            let mut o = ctx.run_ui(raw, |ui| {
                let cx = Cx {
                    data: &data,
                    save: &save,
                    offset: 0.0,
                    live: true,
                };
                liza.show(ui, home, &cx, &mut Vec::new());
            });
            o.textures_delta.clear();
        };
        run(&mut liza, &mut home, Vec::new(), 0.0);
        run(&mut liza, &mut home, Vec::new(), look::BOOT_SECS + 0.1);
        run(&mut liza, &mut home, Vec::new(), 3.0);
        run(
            &mut liza,
            &mut home,
            click(pos2(18.0, look::MENU_H / 2.0)),
            3.2,
        );
        assert!(liza.calc, "the eye opened it");
    }

    #[test]
    fn the_eye_opens_the_calculator() {
        let mut liza = Liza::new();
        liza.act(Act::Calculator, 0.0);
        assert!(liza.calc);
        liza.act(Act::Calculator, 1.0);
        assert!(!liza.calc);
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
