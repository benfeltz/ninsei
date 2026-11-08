//! Liza, the Sense/Net deck's desktop: after the first Mac and the Lisa. A 1-bit menu bar with
//! the tools as menus, Finder windows with striped title bars and a close box, icons down the
//! right, a Trash, and a happy deck at boot. Everything is two theme colours (ink on paper) and
//! patterns from `data/patterns.ron`; a front-end on `home`, like the other desktops.

use egui::{Align2, FontFamily, FontId, Id, Pos2, Rect, Sense, Stroke, StrokeKind, Ui};
use egui::{Vec2, pos2, vec2};
use ninsei_core::Data;
use ninsei_core::desktop::AddonEffect;
use ninsei_core::hardware::Desktop;
use ninsei_core::hub::{Icon, Tool};

use crate::home::{self, Cx, Desk, DesktopView, Home, WbOut, font};
use crate::kit::liza::{self as kit, Look, size};
use crate::{hubtools, patterns};

/// Every size and timing the desktop uses.
mod look {
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
            ("TRAX", Act::Open(Tool::Player)),
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
    let top = screen.top() + size::MENU_H + 14.0;
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

fn title(tool: Tool, data: &Data) -> String {
    let name = match tool {
        Tool::Qwk => "QWK Reader",
        Tool::NightComm => "NiGHTCOMM",
        Tool::TigerTeam => "Tiger Team Pro",
        Tool::IceDb => "ICE DB",
        Tool::Guide => "Welcome to the Net",
        Tool::Player => "TRAX",
        Tool::Shell => "Shell",
        Tool::Terminal => "CarrierTerm",
        Tool::System => "System",
        Tool::Rig => "CPU Shopper",
        Tool::HexEd => "HexEd",
        // A game by its copy's label (`Nibbler`, `NIBBLER [2K4]`).
        Tool::Game(..) => return tool.name(data),
    };
    name.to_string()
}

/// The Games menu's title: it's on the bar only while you own a game.
const GAMES_MENU: &str = "Games";

/// The menus as the bar shows them: a tool you haven't downloaded isn't on one, and the Games
/// menu (before Special) lists each copy of a game you own, by its label.
fn menus(data: &Data, save: &ninsei_core::hub::Save) -> Vec<(&'static str, Vec<(String, Act)>)> {
    let mut out: Vec<(&'static str, Vec<(String, Act)>)> = MENUS
        .iter()
        .map(|(title, items)| {
            let items = items
                .iter()
                .filter(|(_, a)| !matches!(a, Act::Open(t) if !save.opens(*t)))
                .map(|(l, a)| (l.to_string(), *a))
                .collect();
            (*title, items)
        })
        .collect();
    let games: Vec<(String, Act)> = Tool::games(data, save)
        .into_iter()
        .map(|t| (t.name(data), Act::Open(t)))
        .collect();
    if !games.is_empty() {
        let at = out.len().saturating_sub(1);
        out.insert(at, (GAMES_MENU, games));
    }
    out
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

    /// Opens (or brings forward) a tool's window: the tool's room, and the scroll bar beside it
    /// (not the Shell's, which scrolls its own lines). A book (RIG, the Welcome stack) gets a
    /// window its fixed page fits exactly, so it never scrolls.
    fn open(&mut self, tool: Tool) {
        if let Some(i) = self.wins.iter().position(|w| w.tool == tool) {
            let w = self.wins.remove(i);
            self.wins.push(w);
            return;
        }
        let (pos, size) = match tool {
            Tool::Shell => (pos2(40.0, 330.0), vec2(620.0, 290.0)),
            Tool::Terminal => (pos2(60.0, 44.0), vec2(720.0, 440.0)),
            Tool::System => (pos2(100.0, 40.0), vec2(760.0, 500.0)),
            Tool::Rig => (pos2(80.0, 30.0), book_window()),
            Tool::HexEd => (pos2(40.0, 40.0), vec2(800.0, 500.0)),
            Tool::Qwk => (pos2(70.0, 50.0), vec2(700.0, 470.0)),
            Tool::NightComm => (pos2(60.0, 46.0), vec2(880.0, 500.0)),
            Tool::TigerTeam => (pos2(50.0, 40.0), vec2(900.0, 540.0)),
            Tool::IceDb => (pos2(100.0, 60.0), vec2(700.0, 470.0)),
            Tool::Player => (pos2(140.0, 60.0), vec2(480.0, 452.0)),
            Tool::Guide => (pos2(120.0, 30.0), book_window()),
            // Sized when it's first drawn (`home::size_game_window`).
            Tool::Game(_, copy) => {
                let pos = home::game_spot(copy, pos2(90.0, 40.0));
                self.wins.push(Win {
                    tool,
                    pos,
                    size: Vec2::ZERO,
                });
                self.keep_window_limit();
                return;
            }
        };
        // A tool's window has the scroll bar down its right side, beside the tool's room.
        let size = if tool == Tool::Shell {
            size
        } else {
            size + vec2(kit::size::SCROLL_W, 0.0)
        };
        self.wins.push(Win { tool, pos, size });
        self.keep_window_limit();
    }

    /// Without MultiFinder, Liza keeps a few windows: the oldest goes.
    fn keep_window_limit(&mut self) {
        while !self.multifinder && self.wins.len() > look::WINDOW_LIMIT {
            self.wins.remove(0);
        }
    }

    /// Whether a window of `tool` is open (in front or not).
    pub(crate) fn has(&self, tool: Tool) -> bool {
        self.wins.iter().any(|w| w.tool == tool)
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

/// Liza's look: ink lit on a dark paper (lit ink), from the theme's text and its deepest
/// background, with the patterns she draws in.
fn look(d: Desk, data: &Data) -> Look<'_> {
    Look {
        ink: d.ink,
        paper: d.shadow,
        font: font(),
        stripes: data.pattern("pinstripes"),
        gray: data.pattern("p50"),
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
        let c = look(theme, data);
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
        for (n, icon) in save.icons(data).iter().enumerate() {
            let cell = icon_cell(screen, n);
            let picked = self.selected == Some(icon.tool);
            let hit = draw_icon(&painter, icon, cell, screen.right(), picked, &c);
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
            screen.right(),
            false,
            &c,
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
        // A tool taller than its window scrolls with Liza's own scroll bar.
        let show =
            |ui: &mut Ui, id: Id, body: &mut dyn FnMut(&mut Ui)| kit::scrolled(ui, id, &c, body);
        let scroller = home::Scroller {
            bar_w: kit::size::SCROLL_W,
            pad: kit::SCROLL_PAD,
            show: &show,
        };
        let (mut raise, mut close) = (None, None);
        let n = self.wins.len();
        let mut rects = Vec::with_capacity(n);
        for i in 0..n {
            let front = i + 1 == n;
            let (tool, size) = (self.wins[i].tool, &mut self.wins[i].size);
            let (bar, pad) = (kit::size::SCROLL_W, kit::SCROLL_PAD);
            home::size_game_window(size, ui, data, save, tool, kit::window_scrolled, bar, pad);
            let (tool, pos, size) = (self.wins[i].tool, self.wins[i].pos, self.wins[i].size);
            let room = (screen.height() - pos.y - look::WIN_MARGIN).max(look::WIN_MIN_H);
            let rect = Rect::from_min_size(at(pos), vec2(size.x, size.y.min(room)));
            let wid = Id::new(("liza-win", format!("{tool:?}")));
            let body = ui.interact(rect, wid.with("body"), sense(Sense::click()));
            if body.clicked() || body.drag_started() {
                raise = Some(i);
            }
            rects.push(rect);
            let close_r = kit::window_frame(&painter, rect, &title(tool, data), front, &c);
            let bar = kit::title_bar(rect);
            let drag = ui.interact(bar, wid.with("bar"), sense(Sense::click_and_drag()));
            if drag.dragged() {
                let p = &mut self.wins[i].pos;
                *p += drag.drag_delta();
                p.x = p.x.clamp(-size.x + 60.0, screen.width() - 60.0);
                p.y = p.y.clamp(size::MENU_H, screen.height() - size::TITLE_H);
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
            let content = kit::window_content(rect);
            if tool == Tool::Shell {
                home.shell(ui, content, data, save, live && front, out);
            } else {
                // From the content's left to the frame, so the scroll bar sits on it.
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
                // The front window has the keyboard, unless a menu is down.
                home.tool_in(
                    &mut child,
                    tool,
                    cx.keys && front && self.menu.is_none(),
                    data,
                    save,
                    out,
                    home::drag_hint(pos.y, size::MENU_H),
                    Some(&scroller),
                    ("liza-scroll", tool),
                );
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
        let top_ui = on_top(ui, screen, egui::Order::Tooltip, "liza-menus");
        let top_painter = top_ui.painter_at(screen);
        if let Some(a) = self.menu_bar(&top_ui, &top_painter, screen, cx, &c) {
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
            self.calculator(&mut da, cx, screen, &c);
        }

        let mut boot_ui = on_top(ui, screen, egui::Order::Tooltip, "liza-boot-card");
        let boot_painter = boot_ui.painter_at(screen);
        self.boot_card(&mut boot_ui, &boot_painter, screen, data, live, &c);
        home::set_desk(theme);
    }
}

impl Liza {
    /// Draws the menu bar and any menu dropped open, with Sense/Net's eye in its left end;
    /// returns the item picked.
    fn menu_bar(
        &mut self,
        ui: &Ui,
        painter: &egui::Painter,
        screen: Rect,
        cx: &Cx,
        c: &Look,
    ) -> Option<Act> {
        let (ink, paper) = (c.ink, c.paper);
        let shown = menus(cx.data, cx.save);
        let menus: Vec<kit::Menu> = shown
            .iter()
            .map(|(title, items)| kit::Menu {
                title,
                items: items.iter().map(|(l, _)| l.as_str()).collect(),
            })
            .collect();
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
        let mut picked = kit::menu_bar(
            ui,
            painter,
            screen,
            &menus,
            &right,
            &mut self.menu,
            c,
            cx.live,
        )
        .map(|(m, k)| shown[m].1[k].1);
        // Sense/Net's mark: an eye. With the Calculator it opens the desk accessory.
        let eye = kit::menu_bar_rect(screen).left_center() + vec2(18.0, 0.0);
        let calc = cx
            .save
            .has_effect(cx.data, Desktop::Liza, AddonEffect::Calculator);
        if calc {
            let r = Rect::from_center_size(eye, vec2(26.0, size::MENU_H - 2.0));
            let sense = if cx.live {
                Sense::click()
            } else {
                Sense::hover()
            };
            let resp = ui
                .interact(r, Id::new("liza-eye"), sense)
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
        picked
    }

    /// The Calculator desk accessory: a small window doing the sums you'd do on the back of an
    /// envelope before a run.
    fn calculator(&mut self, ui: &mut Ui, cx: &Cx, screen: Rect, c: &Look) {
        let (save, data) = (cx.save, cx.data);
        let rect = Rect::from_min_size(
            screen.min + vec2(look::CALC_POS.0, size::MENU_H + look::CALC_POS.1),
            vec2(look::CALC_SIZE.0, look::CALC_SIZE.1),
        );
        let painter = ui.painter_at(rect.expand(size::SHADOW + 1.0));
        let close = kit::window_frame(&painter, rect, "Calculator", true, c);
        if cx.live
            && ui
                .interact(close, Id::new("liza-calc-close"), Sense::click())
                .clicked()
        {
            self.calc = false;
        }
        let next = save.next_upgrade(data);
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
        let top = rect.top() + size::TITLE_H + 12.0;
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
        c: &Look,
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
        let mid = screen.center();
        let top = mid.y - size.y / 2.0 - 70.0;
        patterns::bitmap(
            painter,
            art,
            pos2(mid.x - size.x / 2.0, top),
            look::BOOT_SCALE,
            ink,
            paper,
        );
        let boxr = Rect::from_center_size(pos2(mid.x, top + size.y + 70.0), vec2(380.0, 84.0));
        kit::shadowed(painter, boxr, c);
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

/// An icon in 1-bit: the art's outlines in ink, its body in paper (inverted when selected),
/// with the name under it on a solid backing, kept left of `right` (the screen's edge).
/// Returns the area that takes clicks.
fn draw_icon(
    painter: &egui::Painter,
    icon: &Icon,
    cell: Rect,
    right: f32,
    picked: bool,
    c: &Look,
) -> Rect {
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
        right,
        picked,
        c,
    );
    art.union(lr)
}

/// A name under an icon, on paper (inverted when selected), centred under it but never past
/// `right` (the screen's edge, for a name wider than the icon column), so it's always whole.
fn label(
    painter: &egui::Painter,
    text: &str,
    center: Pos2,
    right: f32,
    picked: bool,
    c: &Look,
) -> Rect {
    let (fg, bg) = if picked {
        (c.paper, c.ink)
    } else {
        (c.ink, c.paper)
    };
    let galley = painter.layout_no_wrap(text.to_string(), font(), fg);
    let r = Rect::from_center_size(center, galley.size() + vec2(8.0, 2.0));
    let r = r.translate(vec2((right - r.right()).min(0.0).round(), 0.0));
    painter.rect_filled(r, 0.0, bg);
    painter.galley(r.min + vec2(4.0, 1.0), galley, fg);
    r
}

/// A window whose body holds one book (`pager::SIZE`): the body is the window less its title
/// bar and margins (see `content` in the window loop), plus a pixel of slack.
fn book_window() -> egui::Vec2 {
    crate::pager::SIZE + vec2(12.0 + 2.0, size::TITLE_H + 8.0 + 2.0)
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
        frame_with(ctx, liza, home, data, &save, events, t);
    }

    /// One frame of Liza on `save`.
    fn frame_with(
        ctx: &egui::Context,
        liza: &mut Liza,
        home: &mut Home,
        data: &Data,
        save: &Save,
        events: Vec<Event>,
        t: f64,
    ) {
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1280.0, 800.0))),
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
                keys: true,
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
        let net = pos2(40.0 + 60.0, size::MENU_H / 2.0);
        frame(&ctx, &mut liza, &mut home, &data, click(net), 3.0);
        assert_eq!(liza.menu, Some(1), "Net dropped open");
        let item = pos2(net.x, size::MENU_H + 4.0 + size::MENU_ITEM_H / 2.0);
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
                    keys: true,
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
            click(pos2(18.0, size::MENU_H / 2.0)),
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
        let c = look(before, &data);
        assert_eq!(
            (c.ink, c.paper),
            (before.ink, before.shadow),
            "lit ink on dark paper"
        );
    }

    #[test]
    fn every_tool_window_opens_big_enough_for_its_tool() {
        let data = Data::builtin().unwrap();
        let ctx = egui::Context::default();
        crate::uitest::themed(&ctx, &data);
        let (mut liza, mut home) = booted(&ctx, &data);
        let mut t = look::BOOT_SECS + 0.2;
        for tool in Tool::ALL {
            liza.open(tool);
            for _ in 0..2 {
                t += 0.1;
                frame(&ctx, &mut liza, &mut home, &data, Vec::new(), t);
            }
        }
        assert!(home.notices.is_empty(), "too small: {:?}", home.notices);
    }

    #[test]
    fn a_window_dragged_low_shows_the_notice_until_it_has_room_again() {
        let data = Data::builtin().unwrap();
        let ctx = egui::Context::default();
        crate::uitest::themed(&ctx, &data);
        let (mut liza, mut home) = booted(&ctx, &data);
        let mut t = look::BOOT_SECS + 0.2;
        let mut run = |liza: &mut Liza, home: &mut Home| {
            for _ in 0..2 {
                t += 0.1;
                frame(&ctx, liza, home, &data, Vec::new(), t);
            }
        };
        liza.open(Tool::NightComm);
        run(&mut liza, &mut home);
        assert!(home.notices.is_empty());
        // Near the bottom of the screen the window is only its minimum height.
        liza.wins.last_mut().unwrap().pos.y = 700.0;
        run(&mut liza, &mut home);
        assert!(home.notices.contains(&Tool::NightComm));
        home.notices.clear();
        liza.wins.last_mut().unwrap().pos.y = 46.0;
        run(&mut liza, &mut home);
        assert!(home.notices.is_empty(), "back up, it draws again");
    }

    #[test]
    fn a_short_window_scrolls_its_tool_with_the_scroll_bar() {
        let data = Data::builtin().unwrap();
        let ctx = egui::Context::default();
        crate::uitest::themed(&ctx, &data);
        let (mut liza, mut home) = booted(&ctx, &data);
        let mut t = look::BOOT_SECS + 0.2;
        liza.open(Tool::System);
        // Low on the screen, the window is only its minimum height: System runs past it.
        liza.wins.last_mut().unwrap().pos.y = 680.0;
        for _ in 0..3 {
            t += 0.1;
            frame(&ctx, &mut liza, &mut home, &data, Vec::new(), t);
        }
        let key = Id::new(("liza-scroll", Tool::System)).with("kit-scroll");
        let at = |ctx: &egui::Context| {
            ctx.data(|d| d.get_temp::<crate::kit::scrollbar::Scroll>(key))
                .unwrap()
        };
        let s = at(&ctx);
        assert!(s.active(), "it scrolls: {s:?}");
        let w = liza.wins.last().unwrap();
        let room = (800.0 - w.pos.y - look::WIN_MARGIN).max(look::WIN_MIN_H);
        let rect = Rect::from_min_size(w.pos, vec2(w.size.x, w.size.y.min(room)));
        let down = pos2(
            rect.right() - kit::size::SCROLL_W / 2.0,
            rect.bottom() - 8.0,
        );
        t += 0.1;
        frame(&ctx, &mut liza, &mut home, &data, click(down), t);
        assert_eq!(at(&ctx).offset, s.line, "the down arrow scrolled it a line");
    }

    #[test]
    fn every_tool_window_shows_its_tool_whole_beside_the_scroll_bar() {
        let data = Data::builtin().unwrap();
        let save = Save {
            tools: ninsei_core::data::TOOLS
                .iter()
                .map(|t| t.to_string())
                .collect(),
            guide_seen: true,
            ..Save::default()
        };
        for tool in Tool::ALL {
            let mut h = crate::uitest::Harness::new(1280.0, 800.0);
            crate::uitest::themed(&h.ctx, &data);
            let (mut liza, mut home) = (Liza::new(), Home::new(Vec::new()));
            liza.booted = true;
            liza.open(tool);
            for _ in 0..3 {
                h.frame(Vec::new(), |ui| {
                    let cx = Cx {
                        data: &data,
                        save: &save,
                        offset: 0.0,
                        live: true,
                        keys: true,
                    };
                    liza.show(ui, &mut home, &cx, &mut Vec::new());
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
    fn a_book_has_no_scroll_bar_at_its_own_size() {
        let data = Data::builtin().unwrap();
        let ctx = egui::Context::default();
        crate::uitest::themed(&ctx, &data);
        let (mut liza, mut home) = booted(&ctx, &data);
        let mut t = look::BOOT_SECS + 0.2;
        for tool in [Tool::Rig, Tool::Guide] {
            liza.open(tool);
            for _ in 0..3 {
                t += 0.1;
                frame(&ctx, &mut liza, &mut home, &data, Vec::new(), t);
            }
            let key = Id::new(("liza-scroll", tool)).with("kit-scroll");
            let s = ctx
                .data(|d| d.get_temp::<crate::kit::scrollbar::Scroll>(key))
                .unwrap();
            assert!(!s.active(), "{tool:?}'s page fits its window: {s:?}");
        }
    }

    #[test]
    fn the_wheel_over_a_window_in_front_doesnt_scroll_the_one_behind() {
        let data = Data::builtin().unwrap();
        let ctx = egui::Context::default();
        crate::uitest::themed(&ctx, &data);
        let (mut liza, mut home) = booted(&ctx, &data);
        liza.multifinder = true;
        let mut t = look::BOOT_SECS + 0.2;
        let mut run = |liza: &mut Liza, home: &mut Home, ev: Vec<egui::Event>| {
            t += 0.1;
            frame(&ctx, liza, home, &data, ev, t);
        };
        // System low on the screen, so it scrolls; ICE DB in front, over System's scroll bar.
        liza.open(Tool::System);
        liza.wins.last_mut().unwrap().pos = pos2(100.0, 680.0);
        liza.open(Tool::IceDb);
        liza.wins.last_mut().unwrap().pos = pos2(700.0, 600.0);
        for _ in 0..3 {
            run(&mut liza, &mut home, Vec::new());
        }
        let key = Id::new(("liza-scroll", Tool::System)).with("kit-scroll");
        let at = |ctx: &egui::Context| {
            ctx.data(|d| d.get_temp::<crate::kit::scrollbar::Scroll>(key))
                .unwrap()
        };
        assert!(at(&ctx).active());
        let sys = &liza.wins[liza.wins.len() - 2];
        let over_bar = pos2(sys.pos.x + sys.size.x - 8.0, 760.0);
        let wheel = vec![
            egui::Event::PointerMoved(over_bar),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: vec2(0.0, -40.0),
                modifiers: egui::Modifiers::NONE,
                phase: egui::TouchPhase::Move,
            },
        ];
        run(&mut liza, &mut home, wheel);
        for _ in 0..10 {
            run(&mut liza, &mut home, Vec::new());
        }
        assert_eq!(at(&ctx).offset, 0.0, "the window in front had the wheel");
        assert_eq!(liza.wins.last().unwrap().tool, Tool::IceDb);
    }

    #[test]
    fn the_wheel_over_a_covered_view_doesnt_scroll_it() {
        let data = Data::builtin().unwrap();
        let ctx = egui::Context::default();
        crate::uitest::themed(&ctx, &data);
        let (mut liza, mut home) = booted(&ctx, &data);
        liza.multifinder = true;
        let mut t = look::BOOT_SECS + 0.2;
        let mut run = |liza: &mut Liza, home: &mut Home, ev: Vec<egui::Event>| {
            t += 0.1;
            frame(&ctx, liza, home, &data, ev, t);
        };
        liza.open(Tool::System);
        liza.wins.last_mut().unwrap().pos = pos2(100.0, 680.0);
        liza.open(Tool::IceDb);
        liza.wins.last_mut().unwrap().pos = pos2(700.0, 600.0);
        for _ in 0..3 {
            run(&mut liza, &mut home, Vec::new());
        }
        let key = Id::new(("liza-scroll", Tool::System)).with("kit-scroll");
        let at = |ctx: &egui::Context| {
            ctx.data(|d| d.get_temp::<crate::kit::scrollbar::Scroll>(key))
                .unwrap()
        };
        let wheel = |p| {
            vec![
                egui::Event::PointerMoved(p),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: vec2(0.0, -40.0),
                    modifiers: egui::Modifiers::NONE,
                    phase: egui::TouchPhase::Move,
                },
            ]
        };
        // Over System's view, under ICE DB.
        run(&mut liza, &mut home, wheel(pos2(750.0, 760.0)));
        for _ in 0..10 {
            run(&mut liza, &mut home, Vec::new());
        }
        assert_eq!(at(&ctx).offset, 0.0, "ICE DB had the wheel");
        run(&mut liza, &mut home, wheel(pos2(300.0, 760.0)));
        for _ in 0..10 {
            run(&mut liza, &mut home, Vec::new());
        }
        assert!(
            at(&ctx).offset > 0.0,
            "uncovered, the wheel scrolls the view"
        );
    }

    #[test]
    fn a_slow_drag_moves_a_window_every_way() {
        let data = Data::builtin().unwrap();
        for step in [vec2(-0.5, -0.5), vec2(0.5, 0.5)] {
            let ctx = egui::Context::default();
            let (mut liza, mut home) = booted(&ctx, &data);
            liza.open(Tool::System);
            let mut t = look::BOOT_SECS + 0.2;
            let mut run = |liza: &mut Liza, home: &mut Home, ev: Vec<egui::Event>| {
                t += 0.05;
                frame(&ctx, liza, home, &data, ev, t);
            };
            run(&mut liza, &mut home, Vec::new());
            let from = liza.wins.last().unwrap().pos;
            let grab = from + vec2(300.0, 10.0);
            let button = |pos, pressed| egui::Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            run(
                &mut liza,
                &mut home,
                vec![egui::Event::PointerMoved(grab), button(grab, true)],
            );
            for k in 1..=60 {
                run(
                    &mut liza,
                    &mut home,
                    vec![egui::Event::PointerMoved(grab + step * k as f32)],
                );
            }
            run(
                &mut liza,
                &mut home,
                vec![button(grab + step * 60.0, false)],
            );
            let to = liza.wins.last().unwrap().pos;
            assert!(
                (to - from).length() > 10.0 && (to - from).x.signum() == step.x.signum(),
                "a drag of {step:?} a frame moved it {from:?} to {to:?}"
            );
        }
    }

    /// A save past the first boot with no intros to wait through, owning both copies of every
    /// game.
    fn gamer(data: &Data) -> Save {
        let mut s = Save {
            guide_seen: true,
            intros: false,
            ..Save::default()
        };
        for g in &data.games {
            s.learn_game(&g.id, ninsei_core::hub::GameCopy::Cracked);
            s.learn_game(&g.id, ninsei_core::hub::GameCopy::Boxed);
        }
        s
    }

    fn game(id: &str, copy: ninsei_core::hub::GameCopy) -> Tool {
        Tool::game(id, copy).unwrap()
    }

    #[test]
    fn the_games_menu_is_there_only_with_a_game_and_lists_each_copy() {
        use ninsei_core::hub::GameCopy::{Boxed, Cracked};
        let data = Data::builtin().unwrap();
        let none = menus(&data, &Save::default());
        assert!(none.iter().all(|(t, _)| *t != GAMES_MENU), "no Games menu");
        let mut save = Save::default();
        save.learn_game("nibbler", Cracked);
        save.learn_game("nibbler", Boxed);
        let all = menus(&data, &save);
        let at = all.iter().position(|(t, _)| *t == GAMES_MENU).unwrap();
        assert_eq!(all[at + 1].0, "Special", "Games comes before Special");
        let items: Vec<(&str, Act)> = all[at].1.iter().map(|(l, a)| (l.as_str(), *a)).collect();
        assert_eq!(
            items,
            [
                ("NIBBLER [2K4]", Act::Open(game("nibbler", Cracked))),
                ("Nibbler", Act::Open(game("nibbler", Boxed))),
            ]
        );
    }

    #[test]
    fn double_clicking_a_games_icon_opens_its_window_by_its_label() {
        let data = Data::builtin().unwrap();
        let save = gamer(&data);
        let ctx = egui::Context::default();
        let (mut liza, mut home) = (Liza::new(), Home::new(Vec::new()));
        let mut t = 0.0;
        let mut run = |liza: &mut Liza, home: &mut Home, ev| {
            t += 0.1;
            frame_with(&ctx, liza, home, &data, &save, ev, t);
        };
        run(&mut liza, &mut home, Vec::new());
        frame_with(
            &ctx,
            &mut liza,
            &mut home,
            &data,
            &save,
            Vec::new(),
            look::BOOT_SECS + 0.1,
        );
        let screen = ctx.content_rect();
        let icons = save.icons(&data);
        let n = icons
            .iter()
            .position(|i| i.tool == game("blox", ninsei_core::hub::GameCopy::Boxed))
            .expect("the box has an icon");
        let cell = icon_cell(screen, n);
        let at = pos2(cell.center().x, cell.top() + 20.0);
        let t0 = 20.0;
        frame_with(&ctx, &mut liza, &mut home, &data, &save, click(at), t0);
        frame_with(
            &ctx,
            &mut liza,
            &mut home,
            &data,
            &save,
            click(at),
            t0 + 0.1,
        );
        frame_with(
            &ctx,
            &mut liza,
            &mut home,
            &data,
            &save,
            Vec::new(),
            t0 + 0.2,
        );
        let top = liza.wins.last().unwrap();
        assert_eq!(top.tool, icons[n].tool);
        assert_eq!(title(top.tool, &data), "Blox");
    }

    #[test]
    fn every_games_window_opens_big_enough_and_plays_while_in_front() {
        let data = Data::builtin().unwrap();
        let save = gamer(&data);
        let ctx = egui::Context::default();
        crate::uitest::themed(&ctx, &data);
        let (mut liza, mut home) = (Liza::new(), Home::new(Vec::new()));
        liza.multifinder = true;
        frame_with(&ctx, &mut liza, &mut home, &data, &save, Vec::new(), 0.0);
        let mut t = look::BOOT_SECS + 0.2;
        for g in Tool::games(&data, &save) {
            liza.open(g);
            for _ in 0..2 {
                t += 0.1;
                frame_with(&ctx, &mut liza, &mut home, &data, &save, Vec::new(), t);
            }
            assert!(
                home.notices.is_empty(),
                "{g:?} too small: {:?}",
                home.notices
            );
            assert!(
                home.game(g).is_some_and(|g| !g.paused()),
                "{g:?} plays in front"
            );
        }
        // The one in front has the keys; the others wait, paused.
        let front = liza.wins.last().unwrap().tool;
        assert!(
            liza.wins[..liza.wins.len() - 1]
                .iter()
                .all(|w| home.game(w.tool).unwrap().paused())
        );
        let minefield = game("minefield", ninsei_core::hub::GameCopy::Boxed);
        assert_eq!(front, minefield);
        let at = home.game(minefield).unwrap().cursor().unwrap();
        frame_with(
            &ctx,
            &mut liza,
            &mut home,
            &data,
            &save,
            crate::uitest::key(egui::Key::ArrowRight),
            t + 0.1,
        );
        assert_eq!(
            home.game(minefield).unwrap().cursor(),
            Some((at.0 + 1, at.1))
        );
    }

    #[test]
    fn a_games_window_is_as_big_as_its_field_or_its_intro_when_one_plays() {
        let data = Data::builtin().unwrap();
        let ctx = egui::Context::default();
        crate::uitest::themed(&ctx, &data);
        let blox = game("blox", ninsei_core::hub::GameCopy::Boxed);
        let mut widths = Vec::new();
        for intros in [false, true] {
            let save = Save {
                intros,
                ..gamer(&data)
            };
            let (mut liza, mut home) = (Liza::new(), Home::new(Vec::new()));
            frame_with(&ctx, &mut liza, &mut home, &data, &save, Vec::new(), 0.0);
            liza.open(blox);
            let t = look::BOOT_SECS + 0.2;
            frame_with(&ctx, &mut liza, &mut home, &data, &save, Vec::new(), t);
            frame_with(
                &ctx,
                &mut liza,
                &mut home,
                &data,
                &save,
                Vec::new(),
                t + 0.1,
            );
            assert!(
                home.notices.is_empty(),
                "intros {intros}: {:?}",
                home.notices
            );
            widths.push(liza.wins.last().unwrap().size.x);
        }
        let mut intro = 0.0;
        let mut o = ctx.run_ui(egui::RawInput::default(), |ui| {
            intro = crate::bbs_view::screen_size(ui).x;
        });
        o.textures_delta.clear();
        assert!(widths[0] < intro / 2.0, "Blox alone is narrow: {widths:?}");
        assert!(widths[1] > intro, "its splash's screen fits: {widths:?}");
    }

    #[test]
    fn every_icons_name_is_painted_whole_and_under_its_icon() {
        // The longest names are the cracks' (`MINEFIELD [PHS]`), wider than the icon column.
        // A short screen puts the icons in several columns.
        let data = Data::builtin().unwrap();
        let mut save = Save {
            guide_seen: true,
            intros: false,
            ..Save::default()
        };
        save.ensure_phonebook(&data, 1);
        save.tools = ninsei_core::data::TOOLS
            .iter()
            .map(|t| t.to_string())
            .collect();
        for g in &data.games {
            save.learn_game(&g.id, ninsei_core::hub::GameCopy::Cracked);
            save.learn_game(&g.id, ninsei_core::hub::GameCopy::Boxed);
        }
        for size in [vec2(1280.0, 800.0), vec2(1280.0, 420.0)] {
            let (h, ..) = desk_sized(&data, &save, size);
            let screen = Rect::from_min_size(Pos2::ZERO, size);
            let painted = h.painted_text();
            let mut columns = 0;
            for (n, icon) in save.icons(&data).iter().enumerate() {
                let cell = icon_cell(screen, n);
                columns =
                    columns.max(((screen.right() - cell.right()) / look::ICON_COL_W) as u32 + 1);
                let (_, r) = painted
                    .iter()
                    .find(|(t, _)| *t == icon.label)
                    .unwrap_or_else(|| panic!("{} isn't painted", icon.label));
                assert!(screen.contains_rect(*r), "{} runs off at {r:?}", icon.label);
                // Centred under its icon, unless that would run off the screen's edge.
                let centred = (r.center().x - cell.center().x).abs() < 1.0;
                let at_edge = cell.center().x + r.width() / 2.0 + 4.0 > screen.right();
                assert!(
                    centred || at_edge,
                    "{} is off its icon: {r:?} in {cell:?}",
                    icon.label
                );
            }
            if size.y < 500.0 {
                assert!(columns > 2, "the short screen needs several columns");
            }
        }
    }

    /// Liza drawn on a harness for `save`, past its boot card.
    fn desk_on(data: &Data, save: &Save) -> (crate::uitest::Harness, Liza, Home) {
        desk_sized(data, save, vec2(1280.0, 800.0))
    }

    /// The same, on a screen of `size`.
    fn desk_sized(
        data: &Data,
        save: &Save,
        size: egui::Vec2,
    ) -> (crate::uitest::Harness, Liza, Home) {
        let mut h = crate::uitest::Harness::new(size.x, size.y);
        crate::uitest::themed(&h.ctx, data);
        let (mut liza, mut home) = (Liza::new(), Home::new(Vec::new()));
        drawn(&mut h, &mut liza, &mut home, data, save, Vec::new());
        h.wait(look::BOOT_SECS);
        drawn(&mut h, &mut liza, &mut home, data, save, Vec::new());
        drawn(&mut h, &mut liza, &mut home, data, save, Vec::new());
        (h, liza, home)
    }

    fn drawn(
        h: &mut crate::uitest::Harness,
        liza: &mut Liza,
        home: &mut Home,
        data: &Data,
        save: &Save,
        events: Vec<Event>,
    ) {
        h.frame(events, |ui| {
            let cx = Cx {
                data,
                save,
                offset: 0.0,
                live: true,
                keys: true,
            };
            liza.show(ui, home, &cx, &mut Vec::new());
        });
    }

    /// Clicks the painted text `label` (the last painted, so the one on top).
    fn click_text(
        h: &mut crate::uitest::Harness,
        liza: &mut Liza,
        home: &mut Home,
        data: &Data,
        save: &Save,
        label: &str,
    ) {
        let at = h
            .painted_text()
            .into_iter()
            .rev()
            .find(|(t, _)| t == label)
            .unwrap_or_else(|| panic!("no {label:?} painted"))
            .1
            .center();
        drawn(h, liza, home, data, save, crate::uitest::click(at));
        drawn(h, liza, home, data, save, Vec::new());
    }

    /// Every game copy's label but `but`'s.
    fn other_labels(data: &Data, but: &[Tool]) -> Vec<String> {
        data.games
            .iter()
            .flat_map(|g| {
                [
                    ninsei_core::hub::GameCopy::Cracked,
                    ninsei_core::hub::GameCopy::Boxed,
                ]
                .map(|c| game(&g.id, c))
            })
            .filter(|t| !but.contains(t))
            .map(|t| t.name(data))
            .collect()
    }

    #[test]
    fn a_game_you_dont_own_is_nowhere_on_liza_and_the_games_menu_opens_the_copy_picked() {
        use ninsei_core::hub::GameCopy::{Boxed, Cracked};
        let data = Data::builtin().unwrap();
        // No game: no Games menu on the bar, no game's icon.
        let none = Save {
            guide_seen: true,
            intros: false,
            ..Save::default()
        };
        let (h, ..) = desk_on(&data, &none);
        let texts: Vec<String> = h.painted_text().into_iter().map(|(t, _)| t).collect();
        assert!(!texts.iter().any(|t| t == GAMES_MENU), "no Games menu");
        for l in other_labels(&data, &[]) {
            assert!(!texts.contains(&l), "{l} painted");
        }
        // Both copies of Nibbler: two icons, two menu items, each opening its own copy.
        let mut save = none.clone();
        save.learn_game("nibbler", Cracked);
        save.learn_game("nibbler", Boxed);
        let owned = [game("nibbler", Cracked), game("nibbler", Boxed)];
        let (mut h, mut liza, mut home) = desk_on(&data, &save);
        let icons: Vec<String> = h.painted_text().into_iter().map(|(t, _)| t).collect();
        assert!(icons.contains(&"NIBBLER [2K4]".into()) && icons.contains(&"Nibbler".into()));
        click_text(&mut h, &mut liza, &mut home, &data, &save, GAMES_MENU);
        assert!(liza.menu.is_some(), "the Games menu dropped open");
        let shown: Vec<String> = h.painted_text().into_iter().map(|(t, _)| t).collect();
        for l in other_labels(&data, &owned) {
            assert!(!shown.contains(&l), "{l} painted");
        }
        click_text(&mut h, &mut liza, &mut home, &data, &save, "NIBBLER [2K4]");
        assert_eq!(liza.wins.last().map(|w| w.tool), Some(owned[0]));
        click_text(&mut h, &mut liza, &mut home, &data, &save, GAMES_MENU);
        click_text(&mut h, &mut liza, &mut home, &data, &save, "Nibbler");
        assert_eq!(liza.wins.last().map(|w| w.tool), Some(owned[1]));
    }
}
