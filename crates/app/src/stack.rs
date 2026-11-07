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
    /// action, and closing it just tucks it away for this fight. The best move is its default
    /// button, so Enter runs it and Esc closes it while the Message box isn't being typed in.
    fn overlay(&self, ctx: &egui::Context, cx: &mut FrameCx) -> Vec<Request> {
        let g = cx.game;
        let mut out = Vec::new();
        let (Some(c), Some(closed)) = (&g.combat, alert_key(g)) else {
            return out;
        };
        if !alert_up(ctx, g) {
            return out;
        }
        let def = &g.data.ice[c.ice];
        let best = best_move(g);
        let shield = shield_move(g);
        // Left to right: Close, then a shield, then the action (the best move, else end turn).
        let mut buttons = vec!["Close".to_string()];
        let mut acts = vec![None];
        if let Some(s) = shield {
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
        let labels: Vec<&str> = buttons.iter().map(String::as_str).collect();
        let message = format!(
            "{}, strength {} of {}. It's holding the line.",
            def.class.label(),
            c.strength,
            c.strength_max
        );
        let title = format!("{} rezzed.", def.name);
        let alert = liza::Alert {
            icon: "⚠",
            title: &title,
            message: &message,
            buttons: &labels,
            default: best.map(|_| labels.len() - 1),
            cancel: Some(0),
            keys: cx.keys_free && !ctx.egui_wants_keyboard_input(),
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
        match liza::alert(
            ctx,
            Id::new("stack-alert"),
            at,
            look::ALERT_W,
            &alert,
            &look,
        ) {
            Some(0) => {
                ctx.data_mut(|d| d.insert_temp(closed, true));
            }
            Some(i) => out.extend(acts[i].map(act)),
            None => {}
        }
        out
    }

    fn takes_enter(&self, ctx: &egui::Context, game: &Game) -> bool {
        alert_up(ctx, game) && best_move(game).is_some()
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

    /// The first fight on the first seed whose opening route meets ICE.
    fn a_fight(d: &Data) -> Game {
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
            let mut cx = FrameCx {
                game: g,
                pal: crate::theme::Palette::from_def(&g.data.themes[0]),
                suite: &mut suite,
                keys_free,
            };
            out = Stack.overlay(ui.ctx(), &mut cx);
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
        let best = best_move(&g).expect("a breaker is loaded");
        let mut h = crate::uitest::Harness::new(1280.0, 800.0);
        overlay(&mut h, &g, true, vec![]);
        assert!(
            Stack.takes_enter(&h.ctx, &g),
            "Enter is the alert's, not the Message box's"
        );
        let out = overlay(&mut h, &g, true, crate::uitest::key(egui::Key::Enter));
        assert!(runs(&out, best), "Enter ran the best move");
        overlay(&mut h, &g, true, crate::uitest::key(egui::Key::Escape));
        assert!(!alert_up(&h.ctx, &g), "Esc closed it");
        assert!(!Stack.takes_enter(&h.ctx, &g), "and gave Enter back");
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
        let b = best_move(&g).expect("a breaker is loaded");
        let hit = g.preview(b).unwrap().hit();
        assert!(hit > 0);
        assert!((0..g.hdd.len()).all(|i| g.preview(i).map_or(0, |p| p.hit()) <= hit));
    }
}
