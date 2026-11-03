//! The Stack chrome (the Sense/Net deck): the run screen as a HyperCard stack. The deck is a
//! field of meters that fill with darker patterns as they rise, programs are rounded buttons
//! with a hatched shadow, the terminal is the Message box under a scrolling log field, and a
//! fight brings up a double-bordered Mac alert with the best move as its default button. Two
//! theme colours (lit ink on dark paper); anything it doesn't draw is Classic.

use egui::{Align2, Color32, Id, Rect, RichText, Sense, Stroke, StrokeKind, Ui, pos2 as pos, vec2};
use ninsei_core::data::Effect;
use ninsei_core::{Action, Game, LogKind, Phase, tuning};

use crate::chrome::{ChromeView, FrameCx, MenuPart, Menus, act};
use crate::panes::{Panes, Request, Tab};
use crate::patterns;

/// Every size the Stack chrome uses.
mod look {
    pub const METER_W: f32 = 150.0;
    pub const METER_H: f32 = 14.0;
    pub const BUTTON_H: f32 = 40.0;
    pub const BUTTON_R: f32 = 9.0;
    /// How far a button's hatched shadow falls.
    pub const DROP: f32 = 4.0;
    pub const ALERT_W: f32 = 460.0;
    /// How far down the screen the fight's alert sits, when the card hasn't placed it.
    pub const ALERT_TOP: f32 = 230.0;
    /// A window's title bar, and the gap between windows.
    pub const TITLE_H: f32 = 20.0;
    pub const GAP: f32 = 10.0;
    /// The 1-bit drop shadow under windows.
    pub const SHADOW: f32 = 2.0;
    /// The card's right column (the deck and the programs).
    pub const SIDE_W: f32 = 380.0;
    /// The card's navigation strip.
    pub const NAV_H: f32 = 40.0;
    /// The windows under the card: the log's height, and the Message box's.
    pub const LOG_H: f32 = 150.0;
    pub const MSG_H: f32 = 56.0;
    /// How round the small buttons are.
    pub const SMALL_R: u8 = 6;
}

/// The cards you page through, in order. The map is Home.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Card {
    Map,
    Ice,
    Hex,
}

impl Card {
    const ALL: [Card; 3] = [Card::Map, Card::Ice, Card::Hex];

    fn name(self) -> &'static str {
        match self {
            Card::Map => "Network",
            Card::Ice => "ICE",
            Card::Hex => "Hex",
        }
    }

    fn index(self) -> usize {
        Card::ALL.iter().position(|&c| c == self).unwrap_or(0)
    }

    /// The card `by` steps along, wrapping round.
    pub fn step(self, by: i32) -> Card {
        let n = Card::ALL.len() as i32;
        Card::ALL[((self.index() as i32 + by).rem_euclid(n)) as usize]
    }
}

/// What the card remembers between frames: which card is up, and whether the last frame was a
/// fight (a fight turns to the ICE card, and its end turns back to the map).
#[derive(Clone, Copy)]
struct CardState {
    card: Card,
    fighting: bool,
}

/// A HyperCard window: a drop shadow, a 1-px frame, and a title bar with pinstripes, a close
/// box and the title on a solid backing. Returns the area inside.
fn window(ui: &Ui, p: &Panes, rect: Rect, title: &str) -> Rect {
    let (ink, paper) = (p.pal.fg, p.pal.bg);
    let painter = ui.painter_at(rect.expand(look::SHADOW + 1.0));
    painter.rect_filled(rect.translate(vec2(look::SHADOW, look::SHADOW)), 0.0, ink);
    painter.rect_filled(rect, 0.0, paper);
    painter.rect_stroke(rect, 0.0, Stroke::new(1.0, ink), StrokeKind::Inside);
    let bar = Rect::from_min_size(rect.min, vec2(rect.width(), look::TITLE_H));
    painter.line_segment(
        [bar.left_bottom(), bar.right_bottom()],
        Stroke::new(1.0, ink),
    );
    if let Some(pat) = p.game.data.pattern("pinstripes") {
        patterns::fill(&painter, pat, bar.shrink2(vec2(3.0, 4.0)), ink, paper);
    }
    let close = Rect::from_center_size(bar.left_center() + vec2(16.0, 0.0), vec2(11.0, 11.0));
    painter.rect_filled(close.expand(2.0), 0.0, paper);
    painter.rect_stroke(close, 0.0, Stroke::new(1.0, ink), StrokeKind::Inside);
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let g = painter.layout_no_wrap(title.to_string(), font, ink);
    let tbox = Rect::from_center_size(bar.center(), g.size() + vec2(16.0, 2.0));
    painter.rect_filled(tbox, 0.0, paper);
    painter.galley(tbox.min + vec2(8.0, 1.0), g, ink);
    Rect::from_min_max(
        bar.left_bottom() + vec2(6.0, 5.0),
        rect.max - vec2(6.0, 5.0),
    )
}

/// A child ui drawing into `rect` only.
fn inside(ui: &mut Ui, rect: Rect, id: &str) -> Ui {
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .id_salt(id)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    child.set_clip_rect(rect);
    // HyperCard's buttons are rounded, whatever pane draws them.
    let w = &mut child.visuals_mut().widgets;
    for v in [&mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open] {
        v.corner_radius = egui::CornerRadius::same(look::SMALL_R);
    }
    child
}

/// A small rounded navigation button with a glyph. Returns whether it was clicked.
fn nav_button(ui: &mut Ui, p: &Panes, r: Rect, glyph: &str, label: &str) -> bool {
    let (ink, paper) = (p.pal.fg, p.pal.bg);
    let resp = ui.interact(r, ui.id().with(("stack-nav", label)), Sense::click());
    let pressed = resp.is_pointer_button_down_on();
    let painter = ui.painter_at(r.expand(2.0));
    painter.rect_filled(r, 7.0, if pressed { ink } else { paper });
    painter.rect_stroke(
        r,
        7.0,
        Stroke::new(if resp.hovered() { 2.0 } else { 1.0 }, ink),
        StrokeKind::Inside,
    );
    let fg = if pressed { paper } else { ink };
    if glyph == "home" {
        // The font has no house: draw one.
        let c = r.center();
        let s = Stroke::new(1.5, fg);
        painter.line_segment([c + vec2(-8.0, -1.0), c + vec2(0.0, -8.0)], s);
        painter.line_segment([c + vec2(0.0, -8.0), c + vec2(8.0, -1.0)], s);
        painter.rect_stroke(
            Rect::from_min_max(c + vec2(-6.0, -2.0), c + vec2(6.0, 7.0)),
            0.0,
            s,
            StrokeKind::Middle,
        );
        painter.rect_filled(
            Rect::from_min_max(c + vec2(-2.0, 2.0), c + vec2(2.0, 7.0)),
            0.0,
            fg,
        );
    } else {
        painter.text(
            r.center(),
            Align2::CENTER_CENTER,
            glyph,
            egui::TextStyle::Monospace.resolve(ui.style()),
            fg,
        );
    }
    resp.on_hover_text(label).clicked()
}

/// The log as a scrolling field.
fn log_field(p: &Panes, ui: &mut Ui) {
    let ink = p.pal.fg;
    egui::ScrollArea::vertical()
        .stick_to_bottom(true)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for l in &p.game.log {
                let t = RichText::new(p.garble(&l.text)).monospace().color(ink);
                // Bad news stands out by weight, not colour.
                let t = if matches!(l.kind, LogKind::Bad | LogKind::Loot) {
                    t.strong()
                } else {
                    t
                };
                ui.label(t);
            }
        });
}

const MENUS: Menus = &[
    ("File", &[MenuPart::Help]),
    ("Go", &[MenuPart::Turn]),
    (
        "View",
        &[
            MenuPart::Layout,
            MenuPart::Glitches,
            MenuPart::Panes,
            MenuPart::Toggles,
            MenuPart::Monitors,
            MenuPart::Decks,
        ],
    ),
    ("Style", &[MenuPart::Themes]),
];

/// The pattern a meter fills with: darker as it fills.
pub fn meter_pattern(frac: f32) -> &'static str {
    if frac < 0.34 {
        "p25"
    } else if frac < 0.67 {
        "p50"
    } else {
        "p75"
    }
}

/// The program a fight's alert offers first: the loaded program that breaks the most now.
pub fn best_move(g: &Game) -> Option<usize> {
    (0..g.hdd.len())
        .filter_map(|i| g.preview(i).ok().map(|p| (i, p.hit())))
        .filter(|&(_, hit)| hit > 0)
        .max_by_key(|&(i, hit)| (hit, std::cmp::Reverse(i)))
        .map(|(i, _)| i)
}

/// The first loaded shield that can go up now.
fn shield_move(g: &Game) -> Option<usize> {
    (0..g.hdd.len())
        .find(|&i| matches!(g.hdd[i].spec.effect, Effect::Shield(_)) && g.preview(i).is_ok())
}

/// A field: a 1-px frame round what `body` draws.
fn field<R>(ui: &mut Ui, ink: Color32, body: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .stroke(Stroke::new(1.0, ink))
        .inner_margin(egui::Margin::same(6))
        .show(ui, body)
        .inner
}

/// A meter: label, a framed bar filled with a pattern, and its reading.
fn meter(ui: &mut Ui, p: &Panes, label: &str, v: u32, max: u32, reading: String) {
    let (ink, paper) = (p.pal.fg, p.pal.bg);
    ui.horizontal(|ui| {
        ui.add_sized(
            [54.0, look::METER_H],
            egui::Label::new(RichText::new(label).monospace().color(ink)),
        );
        let (r, _) = ui.allocate_exact_size(vec2(look::METER_W, look::METER_H), Sense::hover());
        let painter = ui.painter_at(r.expand(1.0));
        painter.rect_filled(r, 0.0, paper);
        let frac = if max == 0 {
            0.0
        } else {
            (v as f32 / max as f32).clamp(0.0, 1.0)
        };
        let fill = Rect::from_min_size(r.min, vec2(r.width() * frac, r.height())).shrink(2.0);
        if fill.width() > 0.0
            && let Some(pat) = p.game.data.pattern(meter_pattern(frac))
        {
            patterns::fill(&painter, pat, fill, ink, paper);
        }
        painter.rect_stroke(r, 0.0, Stroke::new(1.0, ink), StrokeKind::Inside);
        ui.label(RichText::new(reading).monospace().color(ink));
    });
}

/// A HyperCard button: a rounded box with a hatched shadow. Returns whether it was clicked.
fn button(ui: &mut Ui, p: &Panes, id: Id, text: &str, sub: &str, on: bool, ring: bool) -> bool {
    let (ink, paper) = (p.pal.fg, p.pal.bg);
    let w = ui.available_width().min(240.0);
    let (r, _) = ui.allocate_exact_size(vec2(w, look::BUTTON_H + look::DROP), Sense::hover());
    let face = Rect::from_min_size(r.min, vec2(w - look::DROP, look::BUTTON_H));
    let resp = ui.interact(face, id, if on { Sense::click() } else { Sense::hover() });
    let painter = ui.painter_at(r.expand(4.0));
    if let Some(pat) = p.game.data.pattern("diagonal") {
        let shadow = face.translate(vec2(look::DROP, look::DROP));
        patterns::fill(&painter, pat, shadow, ink, paper);
    }
    let pressed = resp.is_pointer_button_down_on();
    let (fg, bg) = if pressed { (paper, ink) } else { (ink, paper) };
    painter.rect_filled(face, look::BUTTON_R, bg);
    painter.rect_stroke(
        face,
        look::BUTTON_R,
        Stroke::new(if resp.hovered() && on { 2.0 } else { 1.0 }, ink),
        StrokeKind::Inside,
    );
    if ring {
        // The default button's heavy outline.
        painter.rect_stroke(
            face.expand(3.0),
            look::BUTTON_R + 3.0,
            Stroke::new(2.0, ink),
            StrokeKind::Inside,
        );
    }
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let col = if on { fg } else { fg.gamma_multiply(0.5) };
    painter.text(
        face.center() - vec2(0.0, 7.0),
        Align2::CENTER_CENTER,
        text,
        font.clone(),
        col,
    );
    painter.text(
        face.center() + vec2(0.0, 9.0),
        Align2::CENTER_CENTER,
        sub,
        egui::FontId::new(font.size * 0.8, font.family),
        col,
    );
    resp.clicked()
}

pub struct Stack;

impl ChromeView for Stack {
    fn deck(&self, p: &mut Panes, ui: &mut Ui) {
        let g = p.game;
        let d = &g.deck;
        let ink = p.pal.fg;
        ui.label(RichText::new(p.deck.clone()).strong().color(ink));
        ui.add_space(4.0);
        field(ui, ink, |ui| {
            let cpu = g.combat.as_ref().map_or(d.cpu_max, |c| c.cpu);
            meter(ui, p, "CPU", cpu, d.cpu_max, format!("{cpu}/{}", d.cpu_max));
            let ram = g.ram_used();
            meter(
                ui,
                p,
                "RAM",
                ram,
                d.ram_max,
                format!("{ram}/{} GB", d.ram_max),
            );
            let hdd = g.hdd.len() as u32;
            meter(ui, p, "HDD", hdd, d.hdd_max, format!("{hdd}/{}", d.hdd_max));
            meter(
                ui,
                p,
                "INTEG",
                d.integrity,
                d.integrity_max,
                format!("{}/{}", d.integrity, d.integrity_max),
            );
            meter(
                ui,
                p,
                "TRACE",
                d.trace,
                tuning::TRACE_MAX,
                format!("{}%", d.trace),
            );
            if let Some(c) = &g.combat
                && c.shield > 0
            {
                meter(
                    ui,
                    p,
                    "SHIELD",
                    c.shield,
                    c.shield.max(10),
                    c.shield.to_string(),
                );
            }
        });
        ui.add_space(4.0);
        ui.label(
            RichText::new(format!(
                "{}¢   {} fragments   {} hops",
                d.credits, d.fragments, g.hops
            ))
            .monospace()
            .color(ink),
        );
    }

    fn warez(&self, p: &mut Panes, ui: &mut Ui) {
        let g = p.game;
        let fighting = g.phase == Phase::Combat;
        let best = if fighting { best_move(g) } else { None };
        for loaded in [true, false] {
            ui.label(
                RichText::new(if loaded {
                    format!("Loaded  {}/{} GB", g.ram_used(), g.deck.ram_max)
                } else {
                    format!("On disk  {}/{}", g.hdd.len(), g.deck.hdd_max)
                })
                .strong()
                .color(p.pal.fg),
            );
            let mut any = false;
            for i in (0..g.hdd.len()).filter(|&i| g.hdd[i].loaded == loaded) {
                any = true;
                let s = &g.hdd[i];
                let w = &s.spec;
                let (what, on, a) = if !loaded {
                    let fits = g.ram_used() + w.ram <= g.deck.ram_max;
                    ("load", fits, Action::Load(i))
                } else if fighting {
                    ("run", g.preview(i).is_ok(), Action::Run(i))
                } else {
                    ("unload", true, Action::Unload(i))
                };
                let sub = format!("{what} · ram {} cpu {}", w.ram, w.cpu);
                let id = ui.id().with(("stack-prog", s.uid));
                ui.horizontal(|ui| {
                    if button(ui, p, id, &w.file(), &sub, on, best == Some(i)) {
                        p.act(a);
                    }
                    if ui.small_button("hex").clicked() {
                        p.out.push(Request::OpenHex(i));
                    }
                });
            }
            if !any {
                ui.label(RichText::new("(none)").color(p.pal.fg));
            }
            ui.add_space(6.0);
        }
    }

    fn terminal(&self, p: &mut Panes, ui: &mut Ui) {
        let ink = p.pal.fg;
        let input_h = ui.text_style_height(&egui::TextStyle::Monospace) + 22.0;
        // The log: a scrolling field.
        let h = (ui.available_height() - input_h).max(40.0);
        egui::Frame::new()
            .stroke(Stroke::new(1.0, ink))
            .inner_margin(egui::Margin::same(4))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .auto_shrink([false, false])
                    .max_height(h - 10.0)
                    .show(ui, |ui| {
                        for l in &p.game.log {
                            let line = p.garble(&l.text);
                            let t = RichText::new(line).monospace().color(ink);
                            // Bad news stands out by weight, not colour.
                            let t = if matches!(l.kind, LogKind::Bad | LogKind::Loot) {
                                t.strong()
                            } else {
                                t
                            };
                            ui.label(t);
                        }
                    });
            });
        ui.add_space(4.0);
        // The Message box.
        p.prompt_as(ui, "msg>", true);
    }

    fn title(&self, tab: Tab) -> Option<&'static str> {
        Some(match tab {
            Tab::Network => "Card",
            Tab::Deck => "Deck",
            Tab::Warez => "Programs",
            Tab::Terminal => "Message",
            Tab::Target => "ICE",
            Tab::Todo => "To Do",
            Tab::Hex => "Hex",
            _ => return None,
        })
    }

    fn brand(&self) -> &'static str {
        "Liza"
    }

    fn menus(&self) -> Menus {
        MENUS
    }

    fn one_bit(&self) -> bool {
        true
    }

    /// Ink on paper: everything folds onto the theme's text and its deepest background, with
    /// only the faint tone kept for hovers and selections.
    fn palette(&self, p: crate::theme::Palette) -> crate::theme::Palette {
        crate::theme::Palette {
            bg: p.bg,
            panel: p.bg,
            faint: p.faint,
            dim: p.fg,
            fg: p.fg,
            bright: p.fg,
        }
    }

    fn status_fill(&self, p: &crate::theme::Palette) -> Color32 {
        p.bg
    }

    /// A plain line of facts along the bottom, ink on paper.
    fn status(&self, ui: &mut Ui, pal: &crate::theme::Palette, game: &Game) -> bool {
        let node = &game.net.nodes[game.at];
        let state = match game.phase {
            Phase::Exploring => "linked",
            Phase::Combat => "ICE contact",
            Phase::Encounter => "incoming message",
            Phase::Flatlined => "flatline",
            Phase::JackedOut => "offline",
        };
        let site = game.site.as_ref().map_or("a local net".to_string(), |s| {
            format!("{} (tier {})", s.name, s.tier)
        });
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!(
                    "{state}  ·  {site}  ·  {}  ·  layer {} of {}  ·  trace {}%",
                    node.name,
                    node.layer,
                    game.net.layers.len() - 1,
                    game.deck.trace
                ))
                .color(pal.fg),
            );
        });
        true
    }

    /// The mockup's screen: one card window (the card on the left, the deck and programs on
    /// the right, ◀ ⌂ ▶ along its bottom), and under it the Message box and the log.
    fn run_screen(&self, p: &mut Panes, ui: &mut Ui) -> bool {
        let g = p.game;
        let full = ui.max_rect();
        let id = Id::new("stack-card");
        // A fight turns to the ICE card; its end turns back to the map.
        let fighting = g.phase == Phase::Combat;
        let mut st = ui
            .ctx()
            .data(|d| d.get_temp::<CardState>(id))
            .unwrap_or(CardState {
                card: Card::Map,
                fighting,
            });
        if st.fighting != fighting {
            st = CardState {
                card: if fighting { Card::Ice } else { Card::Map },
                fighting,
            };
        }

        let gap = look::GAP;
        let bottom_h = look::LOG_H;
        let card = Rect::from_min_max(
            full.min + vec2(gap, gap),
            full.max - vec2(gap + look::SHADOW, bottom_h + gap * 2.0),
        );
        let site = g
            .site
            .as_ref()
            .map_or("LOCAL NET".to_string(), |s| s.name.to_uppercase());
        let title = format!(
            "{site}  ·  card {} of {}: {}",
            st.card.index() + 1,
            Card::ALL.len(),
            st.card.name()
        );
        let body = window(ui, p, card, &title);
        let side = Rect::from_min_max(pos(body.right() - look::SIDE_W, body.top()), body.max);
        let left = Rect::from_min_max(
            body.min,
            pos(side.left() - gap, body.bottom() - look::NAV_H),
        );
        // The card itself.
        {
            let mut c = inside(ui, left, "stack-card-body");
            match st.card {
                Card::Map => p.network(&mut c),
                Card::Ice => p.target(&mut c),
                Card::Hex => p.hex_pane(&mut c),
            }
        }
        // A line between the card and its right column.
        ui.painter().line_segment(
            [
                pos(side.left() - gap / 2.0, body.top()),
                pos(side.left() - gap / 2.0, body.bottom()),
            ],
            Stroke::new(1.0, p.pal.fg),
        );
        {
            let mut c = inside(ui, side, "stack-card-side");
            egui::ScrollArea::vertical()
                .id_salt("stack-side-scroll")
                .auto_shrink([false, false])
                .show(&mut c, |ui| {
                    self.deck(p, ui);
                    ui.add_space(8.0);
                    self.warez(p, ui);
                    ui.add_space(4.0);
                    ui.label(RichText::new("To Do").strong().color(p.pal.fg));
                    p.todo(ui);
                });
        }
        // ◀ ⌂ ▶ along the bottom of the card.
        let nav_y = body.bottom() - look::NAV_H / 2.0;
        let mid = left.center().x;
        let b = |x: f32| Rect::from_center_size(pos(x, nav_y), vec2(42.0, 28.0));
        if nav_button(ui, p, b(mid - 56.0), "◀", "previous card") {
            st.card = st.card.step(-1);
        }
        if nav_button(ui, p, b(mid), "home", "home: the network") {
            st.card = Card::Map;
        }
        if nav_button(ui, p, b(mid + 56.0), "▶", "next card") {
            st.card = st.card.step(1);
        }
        ui.ctx().data_mut(|d| d.insert_temp(id, st));
        // The fight's alert sits over the card's lower left, clear of the ICE's numbers.
        let alert_at = pos(left.left() + gap, left.bottom() - 150.0);
        ui.ctx()
            .data_mut(|d| d.insert_temp(Id::new("stack-alert-at"), alert_at));

        // Under the card: the Message box on the left, the log on the right.
        let row_top = card.bottom() + gap + look::SHADOW;
        let split = full.left() + full.width() * 0.6;
        let msg = Rect::from_min_size(
            pos(full.left() + gap, row_top),
            vec2(split - full.left() - gap * 1.5, look::MSG_H),
        );
        let log = Rect::from_min_max(
            pos(split + gap / 2.0, row_top),
            pos(full.right() - gap - look::SHADOW, full.bottom() - gap),
        );
        let msg_body = window(ui, p, msg, "Message");
        {
            let mut c = inside(ui, msg_body, "stack-message");
            p.prompt_as(&mut c, "", false);
        }
        let log_body = window(ui, p, log, "Log");
        {
            let mut c = inside(ui, log_body, "stack-log");
            log_field(p, &mut c);
        }
        true
    }

    /// A fight brings up a Mac alert over the card. It's small and off to the side of the
    /// action, and closing it just tucks it away for this fight.
    fn overlay(&self, ctx: &egui::Context, cx: &mut FrameCx) -> Vec<Request> {
        let g = cx.game;
        let mut out = Vec::new();
        let Some(c) = &g.combat else {
            return out;
        };
        if g.phase != Phase::Combat {
            return out;
        }
        let key = Id::new(("stack-alert-closed", g.seed, c.node));
        if ctx.data(|d| d.get_temp::<bool>(key)).unwrap_or(false) {
            return out;
        }
        let (ink, paper) = (cx.pal.fg, cx.pal.bg);
        let def = &g.data.ice[c.ice];
        let best = best_move(g);
        let shield = shield_move(g);
        let screen = ctx.content_rect();
        egui::Area::new(Id::new("stack-alert"))
            .order(egui::Order::Foreground)
            // Where the card put it, or off to the right below the panes' headers.
            .fixed_pos(
                ctx.data(|d| d.get_temp::<egui::Pos2>(Id::new("stack-alert-at")))
                    .unwrap_or(egui::pos2(
                        screen.right() - look::ALERT_W - 60.0,
                        screen.top() + look::ALERT_TOP,
                    )),
            )
            .show(ctx, |ui| {
                ui.set_width(look::ALERT_W);
                egui::Frame::new()
                    .fill(paper)
                    .stroke(Stroke::new(1.0, ink))
                    .inner_margin(egui::Margin::same(4))
                    .show(ui, |ui| {
                        egui::Frame::new()
                            .stroke(Stroke::new(3.0, ink))
                            .inner_margin(egui::Margin::same(12))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("⚠").size(28.0).color(ink));
                                    ui.vertical(|ui| {
                                        ui.label(
                                            RichText::new(format!("{} rezzed.", def.name))
                                                .strong()
                                                .color(ink),
                                        );
                                        ui.label(
                                            RichText::new(format!(
                                                "{}, strength {} of {}. It's holding the line.",
                                                def.class.label(),
                                                c.strength,
                                                c.strength_max
                                            ))
                                            .color(ink),
                                        );
                                    });
                                });
                                ui.add_space(8.0);
                                ui.horizontal(|ui| {
                                    if ui.button("Close").clicked() {
                                        ctx.data_mut(|d| d.insert_temp(key, true));
                                    }
                                    if let Some(s) = shield
                                        && ui
                                            .button(format!("Shield: {}", g.hdd[s].spec.name))
                                            .clicked()
                                    {
                                        out.push(act(Action::Run(s)));
                                    }
                                    if let Some(b) = best {
                                        let text =
                                            RichText::new(format!("Run {}", g.hdd[b].spec.name))
                                                .strong()
                                                .color(ink);
                                        let r = ui.button(text);
                                        // The default button's heavy ring.
                                        ui.painter().rect_stroke(
                                            r.rect.expand(3.0),
                                            4.0,
                                            Stroke::new(2.0, ink),
                                            StrokeKind::Outside,
                                        );
                                        if r.clicked() {
                                            out.push(act(Action::Run(b)));
                                        }
                                    } else if ui.button("End turn").clicked() {
                                        out.push(act(Action::EndTurn));
                                    }
                                });
                            });
                    });
            });
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ninsei_core::Data;

    #[test]
    fn the_cards_page_round() {
        assert_eq!(Card::Map.step(1), Card::Ice);
        assert_eq!(Card::Hex.step(1), Card::Map);
        assert_eq!(Card::Map.step(-1), Card::Hex);
    }

    #[test]
    fn the_stack_draws_in_ink_on_paper() {
        let d = Data::builtin().unwrap();
        let p = Stack.palette(crate::theme::Palette::from_def(&d.themes[0]));
        assert_eq!((p.panel, p.dim, p.bright), (p.bg, p.fg, p.fg));
    }

    #[test]
    fn meters_darken_as_they_fill() {
        let d = Data::builtin().unwrap();
        let cover = |f| d.pattern(meter_pattern(f)).unwrap().coverage_pct();
        assert!(cover(0.1) < cover(0.5));
        assert!(cover(0.5) < cover(0.9));
    }

    #[test]
    fn the_alert_offers_the_hardest_hit() {
        let d = Data::builtin().unwrap();
        assert_eq!(
            best_move(&Game::new(d.clone(), 3)),
            None,
            "nothing to hit out of a fight"
        );
        // The first ICE ahead, on the first seed whose opening route has some.
        let g = (0..50)
            .find_map(|seed| {
                let mut g = Game::new(d.clone(), seed);
                for _ in 0..20 {
                    let &to = g.reachable().first()?;
                    g.apply(Action::Move(to));
                    if g.phase == Phase::Combat {
                        return Some(g);
                    }
                }
                None
            })
            .expect("some run meets ICE");
        let b = best_move(&g).expect("a breaker is loaded");
        let hit = g.preview(b).unwrap().hit();
        assert!(hit > 0);
        assert!((0..g.hdd.len()).all(|i| g.preview(i).map_or(0, |p| p.hit()) <= hit));
    }
}
