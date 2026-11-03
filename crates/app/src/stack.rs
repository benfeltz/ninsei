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
    /// How far down the screen the fight's alert sits.
    pub const ALERT_TOP: f32 = 230.0;
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
            // Off to the right, below the panes' headers and the program row.
            .fixed_pos(egui::pos2(
                screen.right() - look::ALERT_W - 60.0,
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

#[cfg(test)]
mod tests {
    use super::*;
    use ninsei_core::Data;

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
