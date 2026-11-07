//! The Stack chrome (the Sense/Net deck): the run screen as a HyperCard stack. The deck is a
//! field of meters that fill with darker patterns as they rise, programs are rounded buttons
//! with a hatched shadow, the terminal is the Message box under a scrolling log field, and a
//! fight brings up a double-bordered Mac alert with the best move as its default button. Two
//! theme colours (lit ink on dark paper); anything it doesn't draw is Classic.

use egui::{Align2, Color32, Id, Rect, RichText, Sense, Stroke, StrokeKind, Ui, vec2};
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
                    if p.game.has_tool("hexed") && ui.small_button("hex").clicked() {
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
            // Over the lower left, below the ICE's numbers and clear of the programs (the
            // Stack's layout keeps them on the right).
            .fixed_pos(egui::pos2(
                screen.left() + look::ALERT_LEFT,
                screen.top() + look::ALERT_TOP,
            ))
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
            let two = crate::liza::Two { ink, paper };
            let close = crate::liza::window_frame(&painter, data, rect, "Control Panel", true, two);
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
mod tests {
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
