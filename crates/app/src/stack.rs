//! The Stack chrome (the Sense/Net deck): the run screen as a HyperCard stack. The deck is a
//! field of meters that fill with darker patterns as they rise, programs are rounded buttons
//! with a hatched shadow, the terminal is the Message box under a scrolling log field, and a
//! fight brings up a double-bordered Mac alert with the best move as its default button. Two
//! theme colours (lit ink on dark paper); anything it doesn't draw is Classic.

use egui::{Align2, Color32, Id, Rect, RichText, Sense, Stroke, StrokeKind, Ui, vec2};
use ninsei_core::data::Effect;
use ninsei_core::{Action, Game, LogKind, Phase, tuning};

use crate::chrome::{ChromeView, FrameCx, MenuPart, Menus, act};
use crate::kit::liza;
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
    /// Where the fight's alert sits on the screen.
    pub const ALERT_LEFT: f32 = 60.0;
    pub const ALERT_TOP: f32 = 300.0;
    /// How round the small buttons are.
    pub const SMALL_R: u8 = 6;
    /// The Control Panel window, and where its slider and check box sit inside it.
    pub const PANEL_W: f32 = 280.0;
    pub const PANEL_H: f32 = 136.0;
    pub const PANEL_PAD: f32 = 20.0;
    pub const PANEL_TOP: f32 = 32.0;
    pub const TRACK_DOWN: f32 = 32.0;
    pub const NOTCH_GAP: f32 = 30.0;
    pub const KNOB_W: f32 = 10.0;
    pub const KNOB_H: f32 = 16.0;
    pub const BOX_DOWN: f32 = 76.0;
    pub const BOX: f32 = 12.0;
}

/// HyperCard's buttons are rounded, whatever pane draws them.
fn round(ui: &mut Ui) {
    let w = &mut ui.visuals_mut().widgets;
    for v in [&mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open] {
        v.corner_radius = egui::CornerRadius::same(look::SMALL_R);
    }
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
            MenuPart::Sound,
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

/// The program a fight's alert offers first: the program that breaks the most now.
pub fn best_move(g: &Game) -> Option<usize> {
    (0..g.hdd.len())
        .filter_map(|i| g.preview(i).ok().map(|p| (i, p.hit())))
        .filter(|&(_, hit)| hit > 0)
        .max_by_key(|&(i, hit)| (hit, std::cmp::Reverse(i)))
        .map(|(i, _)| i)
}

/// The first shield that can go up now.
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
    let painter = ui.painter_at(r.expand(liza::size::RING_OUT));
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
        // The default button's heavy outline, the same as the alert's.
        liza::default_ring(&painter, face, look::BUTTON_R, ink);
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
            let ifs = g.ifs_left();
            meter(
                ui,
                p,
                "RAM",
                ifs,
                d.ram_max,
                format!("{ifs}/{} GB", d.ram_max),
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
        ui.label(
            RichText::new(format!("On disk  {}/{}", g.hdd.len(), g.deck.hdd_max))
                .strong()
                .color(p.pal.fg),
        );
        for (i, s) in g.hdd.iter().enumerate() {
            let w = &s.spec;
            let on = fighting && g.preview(i).is_ok();
            let sub = format!("run · cpu {}", w.cpu);
            let id = ui.id().with(("stack-prog", s.uid));
            ui.horizontal(|ui| {
                if button(ui, p, id, &w.file(), &sub, on, best == Some(i)) {
                    p.act(Action::Run(i));
                }
                if p.game.has_tool("hexed") && ui.small_button("hex").clicked() {
                    p.out.push(Request::OpenHex(i));
                }
            });
        }
        if g.hdd.is_empty() {
            ui.label(RichText::new("(none)").color(p.pal.fg));
        }
        ui.add_space(6.0);
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
            Tab::Vis => "Scope",
            _ => return None,
        })
    }

    fn brand(&self) -> &'static str {
        "Liza"
    }

    fn menus(&self) -> Menus {
        MENUS
    }

    fn sound(&self, ctx: &egui::Context, cx: &mut FrameCx) -> Vec<Request> {
        control_panel(ctx, cx)
    }

    fn sound_item(&self) -> &'static str {
        "Control Panel..."
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

    /// The dock dressed as HyperCard windows: ink frames and splitters on paper, the front
    /// tab of each window inverted like a selected title.
    fn dock_style(&self, mut s: egui_dock::Style, p: &crate::theme::Palette) -> egui_dock::Style {
        let (ink, paper) = (p.fg, p.bg);
        s.main_surface_border_stroke = Stroke::new(1.0, ink);
        s.separator.width = 2.0;
        s.separator.color_idle = ink;
        s.separator.color_hovered = ink;
        s.separator.color_dragged = ink;
        s.tab_bar.bg_fill = paper;
        s.tab_bar.hline_color = ink;
        s.tab_bar.height = 22.0;
        s.tab.tab_body.bg_fill = paper;
        s.tab.tab_body.stroke = Stroke::new(1.0, ink);
        for (t, bg, text) in [
            (&mut s.tab.active, paper, ink),
            (&mut s.tab.focused, ink, paper),
            (&mut s.tab.active_with_kb_focus, paper, ink),
            (&mut s.tab.focused_with_kb_focus, ink, paper),
            (&mut s.tab.inactive, paper, ink),
            (&mut s.tab.inactive_with_kb_focus, paper, ink),
            (&mut s.tab.hovered, paper, ink),
        ] {
            t.bg_fill = bg;
            t.text_color = text;
            t.outline_color = ink;
            t.corner_radius = egui::CornerRadius {
                nw: look::SMALL_R,
                ne: look::SMALL_R,
                sw: 0,
                se: 0,
            };
        }
        s
    }

    /// The mockup's arrangement: the network (with ICE and Hex as its other cards) large on
    /// the left, the deck and the programs on the right, the Message box along the bottom.
    fn default_layout(&self, extra: &[Tab]) -> egui_dock::DockState<Tab> {
        use egui_dock::{DockState, NodeIndex};
        let mut dock = DockState::new(vec![Tab::Network, Tab::Target, Tab::Hex]);
        let tree = dock.main_surface_mut();
        let [card, bottom] = tree.split_below(NodeIndex::root(), 0.72, vec![Tab::Terminal]);
        let [_, side] = tree.split_right(card, 0.68, vec![Tab::Deck]);
        tree.split_below(side, 0.42, vec![Tab::Warez, Tab::Todo]);
        if !extra.is_empty() {
            tree.split_right(bottom, 0.6, extra.to_vec());
        }
        dock
    }

    fn network(&self, p: &mut Panes, ui: &mut Ui) {
        round(ui);
        p.network(ui);
    }

    fn target(&self, p: &mut Panes, ui: &mut Ui) {
        round(ui);
        p.target(ui);
    }

    fn hex(&self, p: &mut Panes, ui: &mut Ui) {
        round(ui);
        p.hex_pane(ui);
    }

    fn todo(&self, p: &mut Panes, ui: &mut Ui) {
        round(ui);
        p.todo(ui);
    }

    /// The Scope card: a 1-bit oscilloscope in a field, ink on paper.
    fn vis(&self, p: &mut Panes, ui: &mut Ui) {
        let ink = p.pal.fg;
        let samples = (p.hear)();
        crate::vispane::alive(ui, &samples);
        ui.label(RichText::new("Speaker").strong().color(ink));
        ui.add_space(4.0);
        egui::Frame::new()
            .stroke(Stroke::new(1.0, ink))
            .inner_margin(egui::Margin::same(4))
            .show(ui, |ui| {
                // Ink only: a dotted graticule, not a grey line.
                let centre = crate::vispane::Centre::Dots(ink);
                crate::vispane::trace(ui, &p.pal, &samples, centre)
            });
    }

    /// A fight brings up a Mac alert over the card. It's small and off to the side of the
    /// action, and closing it just tucks it away for this fight.
    fn overlay(&self, ctx: &egui::Context, cx: &mut FrameCx) -> Vec<Request> {
        let Some(f) = Fight::of(ctx, cx) else {
            return Vec::new();
        };
        let look = liza::Look {
            ink: cx.pal.fg,
            paper: cx.pal.bg,
            font: egui::TextStyle::Monospace.resolve(&ctx.global_style()),
            stripes: None,
            gray: None,
        };
        let screen = ctx.content_rect();
        // Over the lower left, below the ICE's numbers and clear of the programs (the Stack's
        // layout keeps them on the right).
        let at = egui::pos2(
            screen.left() + look::ALERT_LEFT,
            screen.top() + look::ALERT_TOP,
        );
        let id = Id::new("stack-alert");
        let pressed = f.with(|a| liza::alert(ctx, id, at, look::ALERT_W, a, &look));
        f.press(ctx, pressed)
    }

    /// The fight's alert takes Enter (its default, the best move) and Esc (Close) while the
    /// Message box isn't being typed in, before the run's own hotkeys see them.
    fn keys(&self, ctx: &egui::Context, cx: &mut FrameCx) -> Vec<Request> {
        let Some(f) = Fight::of(ctx, cx) else {
            return Vec::new();
        };
        let pressed = f.with(|a| liza::alert_keys(ctx, Id::new("stack-alert"), a));
        f.press(ctx, pressed)
    }
}

/// The fight's alert: what it says and what each button does, left to right: Close, then a
/// shield, then the action (the best move, its default, else end turn).
struct Fight {
    title: String,
    message: String,
    buttons: Vec<String>,
    acts: Vec<Option<Action>>,
    default: Option<usize>,
    keys: bool,
    closed: Id,
}

impl Fight {
    /// The alert, while it's up.
    fn of(ctx: &egui::Context, cx: &FrameCx) -> Option<Fight> {
        let g = cx.game;
        let (Some(c), Some(closed)) = (&g.combat, alert_key(g)) else {
            return None;
        };
        if !alert_up(ctx, g) {
            return None;
        }
        let def = &g.data.ice[c.ice];
        let best = best_move(g);
        let mut buttons = vec!["Close".to_string()];
        let mut acts = vec![None];
        if let Some(s) = shield_move(g) {
            buttons.push(format!("Shield: {}", g.hdd[s].spec.name));
            acts.push(Some(Action::Run(s)));
        }
        match best {
            Some(b) => {
                buttons.push(format!("Run {}", g.hdd[b].spec.name));
                acts.push(Some(Action::Run(b)));
            }
            None => {
                buttons.push("End turn".to_string());
                acts.push(Some(Action::EndTurn));
            }
        }
        Some(Fight {
            title: format!("{} rezzed.", def.name),
            message: format!(
                "{}, strength {} of {}. It's holding the line.",
                def.class.label(),
                c.strength,
                c.strength_max
            ),
            default: best.map(|_| buttons.len() - 1),
            buttons,
            acts,
            keys: cx.keys_free && !ctx.egui_wants_keyboard_input(),
            closed,
        })
    }

    /// Runs `f` on the alert as the kit takes it.
    fn with<R>(&self, f: impl FnOnce(&liza::Alert) -> R) -> R {
        let labels: Vec<&str> = self.buttons.iter().map(String::as_str).collect();
        f(&liza::Alert {
            icon: "⚠",
            title: &self.title,
            message: &self.message,
            buttons: &labels,
            default: self.default,
            cancel: Some(0),
            keys: self.keys,
        })
    }

    /// What pressing button `i` does: Close tucks the alert away, the rest act.
    fn press(&self, ctx: &egui::Context, i: Option<usize>) -> Vec<Request> {
        match i {
            Some(0) => {
                ctx.data_mut(|d| d.insert_temp(self.closed, true));
                Vec::new()
            }
            Some(i) => self.acts[i].clone().map(act).into_iter().collect(),
            None => Vec::new(),
        }
    }
}

/// Where a fight's "closed" flag is kept: per run and per node.
fn alert_key(g: &Game) -> Option<Id> {
    g.combat
        .as_ref()
        .map(|c| Id::new(("stack-alert-closed", g.seed, c.node)))
}

/// The fight's alert is showing: a fight is on and it hasn't been closed.
fn alert_up(ctx: &egui::Context, g: &Game) -> bool {
    g.phase == Phase::Combat
        && alert_key(g).is_some_and(|k| !ctx.data(|d| d.get_temp::<bool>(k)).unwrap_or(false))
}

/// The Speaker Volume slider's notches: 0-7, as the Mac's Control Panel had.
const SPEAKER_NOTCHES: u32 = 7;

/// Liza's Control Panel (View > Control Panel...): the Speaker Volume slider, 0 to 7, and a
/// Sound check box, in a window with a close box. ←/→ move the slider, Space checks the box,
/// and Enter, Esc or the close box put it away.
fn control_panel(ctx: &egui::Context, cx: &mut FrameCx) -> Vec<Request> {
    use egui::pos2;
    let (ink, paper) = (cx.pal.fg, cx.pal.bg);
    let now = cx.level;
    let (mut to, k) = crate::volume::start(ctx, now, egui::Key::Space, SPEAKER_NOTCHES);
    let mut done = k.ok || k.esc;
    let size = vec2(look::PANEL_W, look::PANEL_H);
    let screen = ctx.content_rect();
    let min = (screen.center() - size / 2.0).round();
    let data = &cx.game.data;
    egui::Area::new(Id::new("stack-control-panel"))
        .order(egui::Order::Foreground)
        .fixed_pos(min)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            let painter = ui.painter().clone();
            let look = liza::Look {
                ink,
                paper,
                font: crate::home::font(),
                stripes: data.pattern("pinstripes"),
                gray: None,
            };
            let close = liza::window_frame(&painter, rect, "Control Panel", true, &look);
            let hit = |ui: &mut Ui, r: Rect, label: &str, sense: Sense| {
                crate::volume::hit(ui, r, "stack-control-panel", label, sense)
            };
            if hit(ui, close, "close", Sense::click()).clicked() {
                done = true;
            }
            let font = crate::home::font();
            let left = rect.left() + look::PANEL_PAD;
            let top = rect.top() + look::PANEL_TOP;
            painter.text(
                pos2(left, top),
                Align2::LEFT_TOP,
                "Speaker Volume",
                font.clone(),
                ink,
            );
            // The slider: a track with a tick and a number at each notch, and a knob.
            let x0 = left + look::KNOB_W;
            let y = top + look::TRACK_DOWN;
            let at = |n: u32| x0 + n as f32 * look::NOTCH_GAP;
            let thin = Stroke::new(1.0, ink);
            painter.line_segment([pos2(x0, y), pos2(at(SPEAKER_NOTCHES), y)], thin);
            for n in 0..=SPEAKER_NOTCHES {
                let x = at(n);
                painter.line_segment([pos2(x, y + 3.0), pos2(x, y + 7.0)], thin);
                painter.text(
                    pos2(x, y + 9.0),
                    Align2::CENTER_TOP,
                    n.to_string(),
                    font.clone(),
                    ink,
                );
            }
            let knob = Rect::from_center_size(
                pos2(at(now.notch(SPEAKER_NOTCHES)), y),
                vec2(look::KNOB_W, look::KNOB_H),
            );
            painter.rect_filled(knob, 0.0, paper);
            painter.rect_stroke(knob, 0.0, thin, StrokeKind::Inside);
            let track = Rect::from_min_max(
                pos2(x0 - look::KNOB_W, y - look::KNOB_H / 2.0),
                pos2(at(SPEAKER_NOTCHES) + look::KNOB_W, y + look::KNOB_H / 2.0),
            );
            let slide = hit(ui, track, "Speaker Volume", Sense::click_and_drag());
            if (slide.clicked() || slide.dragged())
                && let Some(p) = slide.interact_pointer_pos().or(slide.hover_pos())
            {
                let n = ((p.x - x0) / look::NOTCH_GAP).round().clamp(0.0, 7.0) as u32;
                to.volume = ninsei_core::hub::notch_volume(n, SPEAKER_NOTCHES);
            }
            // The check box.
            let bx =
                Rect::from_min_size(pos2(left, top + look::BOX_DOWN), vec2(look::BOX, look::BOX));
            painter.rect_stroke(bx, 0.0, thin, StrokeKind::Inside);
            if now.on {
                painter.line_segment([bx.left_top(), bx.right_bottom()], thin);
                painter.line_segment([bx.right_top(), bx.left_bottom()], thin);
            }
            let words = painter.text(
                pos2(bx.right() + 6.0, bx.center().y),
                Align2::LEFT_CENTER,
                "Sound",
                font,
                ink,
            );
            if hit(ui, bx.union(words), "Sound", Sense::click()).clicked() {
                to.on = !to.on;
            }
        });
    if done {
        cx.popup.close();
    }
    now.to(to)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use ninsei_core::Data;

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

    /// The first fight on the first seed whose opening route meets ICE.
    pub(crate) fn a_fight(d: &Data) -> Game {
        (0..50)
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
            .expect("some run meets ICE")
    }

    /// A frame of the Stack's overlay; returns what it asked for.
    fn overlay(
        h: &mut crate::uitest::Harness,
        g: &Game,
        keys_free: bool,
        ev: Vec<egui::Event>,
    ) -> Vec<Request> {
        let mut suite = crate::suite::State::default();
        let mut out = Vec::new();
        h.frame(ev, |ui| {
            let mut popup = crate::volume::Popup::default();
            let mut cx = FrameCx {
                game: g,
                pal: crate::theme::Palette::from_def(&g.data.themes[0]),
                suite: &mut suite,
                keys_free,
                level: crate::volume::Level {
                    on: true,
                    volume: 100,
                },
                popup: &mut popup,
            };
            // As the run does: the chrome's keys first, then the overlay.
            out = Stack.keys(ui.ctx(), &mut cx);
            out.extend(Stack.overlay(ui.ctx(), &mut cx));
        });
        out
    }

    fn runs(out: &[Request], i: usize) -> bool {
        out.iter().any(
            |r| matches!(r, Request::Cmd(ninsei_core::Command::Game(Action::Run(n))) if *n == i),
        )
    }

    #[test]
    fn enter_runs_the_best_move_and_esc_tucks_the_alert_away() {
        let d = Data::builtin().unwrap();
        let g = a_fight(&d);
        let best = best_move(&g).expect("a breaker is on the disk");
        let mut h = crate::uitest::Harness::new(1280.0, 800.0);
        overlay(&mut h, &g, true, vec![]);
        let out = overlay(&mut h, &g, true, crate::uitest::key(egui::Key::Enter));
        assert!(runs(&out, best), "Enter ran the best move");
        overlay(&mut h, &g, true, crate::uitest::key(egui::Key::Escape));
        assert!(!alert_up(&h.ctx, &g), "Esc closed it");
    }

    #[test]
    fn the_alert_leaves_enter_alone_while_a_modal_has_the_keys() {
        let d = Data::builtin().unwrap();
        let g = a_fight(&d);
        let best = best_move(&g).unwrap();
        let mut h = crate::uitest::Harness::new(1280.0, 800.0);
        overlay(&mut h, &g, false, vec![]);
        let out = overlay(&mut h, &g, false, crate::uitest::key(egui::Key::Enter));
        assert!(!runs(&out, best));
        overlay(&mut h, &g, false, crate::uitest::key(egui::Key::Escape));
        assert!(alert_up(&h.ctx, &g));
    }

    #[test]
    fn the_alerts_buttons_click_through() {
        let d = Data::builtin().unwrap();
        let g = a_fight(&d);
        let best = best_move(&g).unwrap();
        let mut h = crate::uitest::Harness::new(1280.0, 800.0);
        overlay(&mut h, &g, true, vec![]);
        overlay(&mut h, &g, true, vec![]);
        let run = h
            .find(&format!("Run {}", g.hdd[best].spec.name))
            .unwrap_or_else(|| panic!("{:?}", h.texts()));
        let out = overlay(&mut h, &g, true, crate::uitest::click(run.center()));
        assert!(runs(&out, best));
        let close = h.find("Close").unwrap();
        overlay(&mut h, &g, true, crate::uitest::click(close.center()));
        assert!(!alert_up(&h.ctx, &g));
    }

    #[test]
    fn the_alert_offers_the_hardest_hit() {
        let d = Data::builtin().unwrap();
        assert_eq!(
            best_move(&Game::new(d.clone(), 3)),
            None,
            "nothing to hit out of a fight"
        );
        let g = a_fight(&d);
        let b = best_move(&g).expect("a breaker is on the disk");
        let hit = g.preview(b).unwrap().hit();
        assert!(hit > 0);
        assert!((0..g.hdd.len()).all(|i| g.preview(i).map_or(0, |p| p.hit()) <= hit));
    }

    #[test]
    fn the_esc_that_leaves_the_message_box_doesnt_close_the_alert() {
        let d = Data::builtin().unwrap();
        let g = a_fight(&d);
        let mut h = crate::uitest::Harness::new(1280.0, 800.0);
        let mut line = String::new();
        let mut suite = crate::suite::State::default();
        let mut frame = |h: &mut crate::uitest::Harness, ev, focus: bool| {
            h.frame(ev, |ui| {
                let mut popup = crate::volume::Popup::default();
                let mut cx = FrameCx {
                    game: &g,
                    pal: crate::theme::Palette::from_def(&d.themes[0]),
                    suite: &mut suite,
                    keys_free: true,
                    level: crate::volume::Level {
                        on: true,
                        volume: 100,
                    },
                    popup: &mut popup,
                };
                Stack.keys(ui.ctx(), &mut cx);
                // The Message box: a text field with the keyboard.
                let r = ui.text_edit_singleline(&mut line);
                if focus {
                    r.request_focus();
                }
                Stack.overlay(ui.ctx(), &mut cx);
            });
        };
        frame(&mut h, vec![], true);
        frame(&mut h, vec![], true);
        frame(&mut h, crate::uitest::key(egui::Key::Escape), false);
        assert!(alert_up(&h.ctx, &g), "Esc only let go of the Message box");
        frame(&mut h, crate::uitest::key(egui::Key::Escape), false);
        assert!(!alert_up(&h.ctx, &g), "the next Esc closes the alert");
    }
}
