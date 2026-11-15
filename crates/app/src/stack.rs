//! The Stack chrome (the Sense/Net deck): the run screen as a HyperCard stack. The deck is a
//! field of meters that fill with darker patterns as they rise, programs are rounded buttons
//! with a hatched shadow, the terminal is the Message box under a scrolling log field, and a
//! fight puts the Script card under the ICE: the deck's program as a stack of operator blocks
//! with fields, after HyperCard 1.2's button script editor, under its CPU and RAM gauges, and a
//! hi-fi's transport (Step, Play, fast-forward, Pause). What the
//! card shows is core's (`ninsei_core::stack_fight`); this only draws it and routes edits. Two
//! theme colours (lit ink on dark paper); anything it doesn't draw is Classic.

use egui::{Align2, Color32, Id, Rect, RichText, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use ninsei_core::game::ForecastKey;
use ninsei_core::stack_fight::{self, Block, BlockKind, Field};
use ninsei_core::{Action, Game, LogKind, Phase, tuning};

use crate::chrome::{ChromeView, FrameCx, MenuPart, Menus, act};
use crate::kit::liza;
use crate::panes::{Panes, Request, Tab};
use crate::patterns;

/// Every size the Stack chrome uses.
mod look {
    pub const METER_W: f32 = 150.0;
    /// The narrowest a bar gets in a narrow column.
    pub const METER_MIN: f32 = 24.0;
    pub const METER_LABEL: f32 = 54.0;
    pub const METER_H: f32 = 14.0;
    pub const BUTTON_H: f32 = 40.0;
    pub const BUTTON_R: f32 = 9.0;
    /// How far a button's hatched shadow falls.
    pub const DROP: f32 = 4.0;
    /// The Script card: a block's height and the gap above it, the gutter the ▶ sits in, the
    /// widest a block grows, a comment's row, the room inside a block, a field's width for a
    /// number, the gap between fields, and a block's move and close boxes.
    pub const BLOCK_H: f32 = 30.0;
    pub const BLOCK_GAP: f32 = 8.0;
    /// A block's row (the box and the gap above it), and a comment's or the slot's.
    pub const BLOCK_ROW: f32 = BLOCK_H + BLOCK_GAP;
    pub const NOTE_ROW: f32 = 24.0;
    pub const GUTTER: f32 = 18.0;
    /// How far a block guarded by an IF sits in from the one above, per IF.
    pub const INDENT: f32 = 16.0;
    pub const BLOCK_W: f32 = 900.0;
    pub const BLOCK_PAD: f32 = 8.0;
    pub const NUMBER_W: f32 = 44.0;
    pub const FIELD_GAP: f32 = 6.0;
    /// The palette's blocks: their width and height.
    pub const CHIP_W: f32 = 56.0;
    /// The narrowest a chip gets: its word and a little paper either side.
    pub const CHIP_MIN: f32 = 44.0;
    pub const CHIP_H: f32 = 20.0;
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

/// A field: a 1-px frame round what `body` draws.
fn field<R>(ui: &mut Ui, ink: Color32, body: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .stroke(Stroke::new(1.0, ink))
        .inner_margin(egui::Margin::same(6))
        .show(ui, body)
        .inner
}

/// A meter: label, a framed bar `bar_w` wide filled with a pattern, and its reading.
fn meter(ui: &mut Ui, p: &Panes, bar_w: f32, (label, v, max, reading): &(&str, u32, u32, String)) {
    let (ink, paper) = (p.pal.fg, p.pal.bg);
    let (v, max) = (*v, *max);
    ui.horizontal(|ui| {
        ui.add_sized(
            [look::METER_LABEL, look::METER_H],
            egui::Label::new(RichText::new(*label).monospace().color(ink)),
        );
        let (r, _) = ui.allocate_exact_size(vec2(bar_w, look::METER_H), Sense::hover());
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
        ui.label(RichText::new(reading.as_str()).monospace().color(ink));
    });
}

/// The bars' width in a column: what's left after the label and the widest reading, the
/// same for every bar, never more than `METER_W`.
fn bar_width(ui: &Ui, rows: &[(&str, u32, u32, String)]) -> f32 {
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let widest = rows
        .iter()
        .map(|r| text_w(ui, &r.3, &font))
        .fold(0.0, f32::max);
    let gaps = 2.0 * ui.spacing().item_spacing.x;
    (ui.available_width() - look::METER_LABEL - widest - gaps)
        .floor()
        .clamp(look::METER_MIN, look::METER_W)
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

/// The alert card's narrowest width, in points: it grows to fit its words.
const ALERT_W: f32 = 380.0;

/// The Sense/Net's alert card: Liza's alert box (a shadowed box, a heavy-framed "!" mark, the
/// two lines and OK as the default button) over the run. Enter or OK puts it away. Returns
/// whether it was dismissed this frame.
fn alert_card(ctx: &egui::Context, cx: &FrameCx, lines: &[String; 2], enter: bool) -> bool {
    let mut ok = enter;
    egui::Area::new(Id::new("stack-ai-alert"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, vec2(0.0, -40.0))
        .show(ctx, |ui| {
            let look = liza::Look {
                ink: cx.pal.fg,
                paper: cx.pal.bg,
                font: egui::TextStyle::Monospace.resolve(ui.style()),
                stripes: None,
                gray: cx.game.data.pattern("p50"),
            };
            // Wide enough for its longest line: nothing is cut at the box's edge.
            let widest = lines
                .iter()
                .map(|l| liza::text_width(ui.ctx(), l, &look.font))
                .fold(0.0, f32::max);
            let w = (16.0 + 32.0 + 14.0 + widest + 24.0).max(ALERT_W).ceil();
            let (r, _) = ui.allocate_exact_size(vec2(w, 120.0), Sense::hover());
            let r = Rect::from_min_size(r.min.round(), r.size().round());
            liza::shadowed(ui.painter(), r, &look);
            // The alert's mark: a "!" in a heavy frame, left of the words.
            let mark = Rect::from_min_size(r.min + vec2(16.0, 16.0), vec2(32.0, 32.0));
            ui.painter()
                .rect_stroke(mark, 4.0, Stroke::new(3.0, look.ink), StrokeKind::Inside);
            ui.painter().text(
                mark.center(),
                Align2::CENTER_CENTER,
                "!",
                egui::FontId::new(22.0, egui::FontFamily::Monospace),
                look.ink,
            );
            for (i, l) in lines.iter().enumerate() {
                ui.painter().text(
                    pos2(mark.right() + 14.0, mark.top() + 4.0 + 18.0 * i as f32),
                    Align2::LEFT_TOP,
                    l,
                    look.font.clone(),
                    look.ink,
                );
            }
            let w = liza::button_width(ctx, "OK", &look);
            let face = Rect::from_min_size(
                pos2(r.right() - w - 20.0, r.bottom() - 40.0).round(),
                vec2(w, liza::size::BUTTON_H),
            );
            let id = Id::new("stack-ai-alert-ok");
            if liza::button(ui, ui.painter(), face, id, "OK", true, true, &look).clicked() {
                ok = true;
            }
        });
    ok
}

impl ChromeView for Stack {
    /// The alert card when the net's AI wakes; while it's up it owns the keyboard.
    fn overlay(&self, ctx: &egui::Context, cx: &mut FrameCx) -> Vec<Request> {
        // Enter only puts away a card that was already up: the Enter that sent the hop that
        // woke it mustn't close it in the same frame.
        // A run that's over drops it: home has no card to answer.
        if matches!(cx.game.phase, Phase::JackedOut | Phase::Flatlined) {
            cx.alerts.open = None;
        }
        let was_up = cx.alerts.open.is_some();
        if let Some(lines) = cx.alerts.watch.step(cx.game) {
            cx.alerts.open = Some(lines);
        }
        let Some(lines) = cx.alerts.open.clone() else {
            return Vec::new();
        };
        // It owns the keyboard: the command line lets go while it's up.
        ctx.memory_mut(|m| m.stop_text_input());
        // Another modal (a board, a conversation) has the keys: its Enter isn't ours.
        let enter = was_up && cx.keys_free && ctx.input(|i| i.key_pressed(egui::Key::Enter));
        if alert_card(ctx, cx, &lines, enter) {
            cx.alerts.open = None;
        }
        Vec::new()
    }

    fn deck(&self, p: &mut Panes, ui: &mut Ui) {
        let g = p.game;
        let d = &g.deck;
        let ink = p.pal.fg;
        ui.label(RichText::new(p.deck.clone()).strong().color(ink));
        ui.add_space(4.0);
        field(ui, ink, |ui| {
            // The limits in the deck's words: CPU and RAM fill with what's left this turn.
            let b = stack_fight::budgets(g);
            let mut rows = vec![
                ("CPU", b.cpu.0, b.cpu.1, b.cpu_words()),
                ("RAM", b.ifs.0, b.ifs.1, b.ifs_words()),
                ("HDD", b.hdd.0, b.hdd.1, b.hdd_words()),
                (
                    "INTEG",
                    d.integrity,
                    d.integrity_max,
                    format!("{}/{}", d.integrity, d.integrity_max),
                ),
                ("TRACE", d.trace, tuning::TRACE_MAX, format!("{}%", d.trace)),
            ];
            if let Some(c) = &g.combat
                && c.shield > 0
            {
                rows.push(("SHIELD", c.shield, c.shield.max(10), c.shield.to_string()));
            }
            let bar_w = bar_width(ui, &rows);
            for row in &rows {
                meter(ui, p, bar_w, row);
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
                if button(ui, p, id, &w.file(), &sub, on, false) {
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
            Tab::Script => "Script",
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

    /// Three columns over the Message box: the deck over the programs on the left (narrow),
    /// the network with ICE and Hex as its other cards in the centre (widest), the Script in
    /// front of To Do on the right.
    fn default_layout(&self, extra: &[Tab]) -> egui_dock::DockState<Tab> {
        crate::chrome::three_columns(
            extra,
            crate::chrome::Shares {
                over_bottom: 0.74,
                left: 0.2,
                deck: 0.42,
                centre_of_rest: 0.66,
                terminal_of_bottom: 0.6,
            },
        )
    }

    fn network(&self, p: &mut Panes, ui: &mut Ui) {
        round(ui);
        p.network(ui);
    }

    /// The ICE card: the ICE, its hotbar and End Turn. Its rule is ink, as everything on
    /// the card is.
    fn target(&self, p: &mut Panes, ui: &mut Ui) {
        round(ui);
        ui.visuals_mut().widgets.noninteractive.bg_stroke.color = p.pal.fg;
        p.target_with(ui, &stack_fight::ICE_WORDS);
    }

    /// The Script card, the whole run: its transport (Step, Run, fast-forward, Pause) presses only
    /// with ICE in front.
    fn script(&self, p: &mut Panes, ui: &mut Ui) {
        round(ui);
        script_card(p, ui);
    }

    /// The Script card lays itself out and scrolls its own blocks.
    fn scroll_bars(&self, tab: Tab) -> Option<[bool; 2]> {
        (tab == Tab::Script).then_some([false, false])
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

    /// In a fight, while nothing else has the keyboard: Enter presses Run, the default, and Esc
    /// pauses a running program. Esc never closes the card.
    fn keys(&self, ctx: &egui::Context, cx: &mut FrameCx) -> Vec<Request> {
        // Free this frame and the last: egui lets go of a text field on the same Esc that
        // would reach us, and that Esc only meant to leave the field.
        // The alert card has the keyboard while it's up.
        let free = cx.keys_free && cx.alerts.open.is_none() && !ctx.egui_wants_keyboard_input();
        let was = Id::new("stack-keys-free");
        let last = ctx.data_mut(|d| std::mem::replace(d.get_temp_mut_or(was, false), free));
        if !(free && last) {
            return Vec::new();
        }
        let b = stack_fight::buttons(cx.game);
        let mut out = Vec::new();
        if b.run && fresh_key(ctx, egui::Key::Enter) {
            out.push(act(Action::Play));
        }
        if b.pause && fresh_key(ctx, egui::Key::Escape) {
            out.push(act(Action::Pause));
        }
        out
    }
}

/// A fresh press of `key` with no modifiers (a held key goes once), taken so nothing after
/// sees it.
fn fresh_key(ctx: &egui::Context, key: egui::Key) -> bool {
    let fresh = ctx.input(|i| {
        i.events.iter().any(|e| {
            matches!(e, egui::Event::Key { key: k, pressed: true, repeat: false, modifiers, .. }
                if *k == key && modifiers.is_none())
        })
    });
    fresh && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key))
}

/// The Script card's look: Liza's two colours and type.
fn card_look<'a>(p: &Panes<'a>, ui: &Ui) -> liza::Look<'a> {
    liza::Look {
        ink: p.pal.fg,
        paper: p.pal.bg,
        // The run's own type, as the panels round it set theirs.
        font: egui::TextStyle::Monospace.resolve(ui.style()),
        stripes: None,
        gray: p.game.data.pattern("p50"),
    }
}

/// A string's width in `font`, on a whole pixel.
fn text_w(ui: &Ui, text: &str, font: &egui::FontId) -> f32 {
    liza::text_width(ui.ctx(), text, font).ceil()
}

/// Which block is selected: a click on one selects it, and the palette moves or clears it,
/// as HyperCard's Objects did. View state only: the line number, dropped once it's gone.
fn selected(ui: &Ui, g: &Game) -> Option<u32> {
    ui.data(|d| d.get_temp::<Option<u32>>(Id::new("stack-selected")))
        .flatten()
        .filter(|&no| g.program.line(no).is_some())
}

fn select(ui: &Ui, no: Option<u32>) {
    ui.data_mut(|d| d.insert_temp(Id::new("stack-selected"), no));
}

/// How the palette lays out in `width` with `gap` between its pieces: each chip's width, and
/// whether the move and clear boxes go on a row of their own under the chips. Chips shrink to
/// fit beside the boxes; when even the narrowest won't, the boxes wrap. Nothing overlaps.
fn palette_fit(width: f32, gap: f32) -> (f32, bool) {
    let n = stack_fight::PALETTE.len() as f32;
    // The chips' shadows reach past their last one.
    let room = width - liza::size::SHADOW;
    let boxes = 3.0 * look::CHIP_H + 2.0 * gap;
    let beside = ((room - boxes - n * gap) / n).floor();
    if beside >= look::CHIP_MIN {
        return (beside.min(look::CHIP_W), false);
    }
    let alone = ((room - (n - 1.0) * gap) / n).floor();
    (alone.clamp(look::CHIP_MIN, look::CHIP_W), true)
}

/// The turn's forecast of starved lines (`Game::starved`), kept in egui memory and taken
/// again only when what it reads has changed (`Game::forecast_key`): the card is drawn every
/// frame, the forecast plays a pass on a copy of the game.
fn forecast(ui: &Ui, g: &Game) -> Vec<u32> {
    let id = Id::new("stack-forecast");
    let key = g.forecast_key();
    let kept = ui.data(|d| d.get_temp::<(ForecastKey, Vec<u32>)>(id));
    if let Some((k, starved)) = kept
        && k == key
    {
        return starved;
    }
    let starved = g.starved();
    ui.data_mut(|d| d.insert_temp(id, (key, starved.clone())));
    starved
}

/// The least room between a hang and the gauges on the Script card's title line, and between
/// the two gauges.
const BESIDE_GAP: f32 = 12.0;

/// The title line's gauges: a block's side (the largest that fits the column), the gap between
/// blocks, and between the label and its blocks.
const GAUGE_BLOCKS: [f32; 3] = [8.0, 6.0, 4.0];
const GAUGE_GAP: f32 = 2.0;
const GAUGE_LABEL_GAP: f32 = 6.0;

/// A gauge's width: its label and `most` blocks of side `side`.
fn gauge_width(label_w: f32, most: u32, side: f32) -> f32 {
    let n = most as f32;
    (label_w + GAUGE_LABEL_GAP + n * side + (n - 1.0).max(0.0) * GAUGE_GAP).ceil()
}

/// How the title line lays out in `full`: the gauges' block side, whether they stack on two
/// rows (when side by side won't fit), and whether a hang `broke_w` wide sits beside them at
/// the left (else on a row of its own above). Nothing overlaps and nothing runs past `full`.
fn title_fit(
    full: f32,
    labels: [f32; 2],
    most: [u32; 2],
    broke_w: Option<f32>,
) -> (f32, bool, bool) {
    let widths = |side| [0, 1].map(|i| gauge_width(labels[i], most[i], side));
    let side_by = |w: [f32; 2]| w[0] + BESIDE_GAP + w[1];
    let fits = |need: f32| need <= full;
    let (side, stacked) = GAUGE_BLOCKS
        .iter()
        .find_map(|&s| fits(side_by(widths(s))).then_some((s, false)))
        .or_else(|| {
            GAUGE_BLOCKS
                .iter()
                .find_map(|&s| fits(widths(s)[0].max(widths(s)[1])).then_some((s, true)))
        })
        .unwrap_or((GAUGE_BLOCKS[GAUGE_BLOCKS.len() - 1], true));
    let w = widths(side);
    // Stacked, both gauges start under the wider one.
    let row = if stacked { w[0].max(w[1]) } else { side_by(w) };
    let beside = broke_w.is_none_or(|b| b + BESIDE_GAP + row <= full);
    (side, stacked, beside)
}

/// One gauge at `left` on the row centred at `cy`: its label, then a block per unit, filled
/// ink for what's left and an ink outline for what's spent. No number is drawn.
fn gauge(
    ui: &Ui,
    left: f32,
    cy: f32,
    label: &str,
    (have, most): (u32, u32),
    side: f32,
    look: &liza::Look,
) {
    let label_w = text_w(ui, label, &look.font);
    let painter = ui.painter();
    painter.text(
        pos2(left, cy).round(),
        Align2::LEFT_CENTER,
        label,
        look.font.clone(),
        look.ink,
    );
    let mut x = (left + label_w + GAUGE_LABEL_GAP).round();
    let top = (cy - side / 2.0).round();
    for i in 0..most {
        let b = Rect::from_min_size(pos2(x, top), vec2(side, side));
        if i < have {
            painter.rect_filled(b, 0.0, look.ink);
        } else {
            painter.rect_stroke(b, 0.0, Stroke::new(1.0, look.ink), StrokeKind::Inside);
        }
        x += side + GAUGE_GAP;
    }
}

/// The Script card's title line: the CPU and RAM gauges at the right (stacked on two rows in a
/// narrow column), and a hang's `?BREAK IN n` at the left while the ▶ is held, or on a row of
/// its own above when the two don't fit. The pane's tab already names the card.
fn title_line(ui: &mut Ui, card: &stack_fight::Card, look: &liza::Look) {
    let line_h = ui.fonts_mut(|f| f.row_height(&look.font)).ceil() + 4.0;
    let full = ui.available_width().floor();
    let gauges = card.budgets.gauges();
    let labels = gauges.map(|(l, ..)| text_w(ui, l, &look.font));
    let most = gauges.map(|(_, _, m)| m);
    let broke = card.broke_words();
    let broke_w = broke.as_deref().map(|b| text_w(ui, b, &look.font) + 2.0);
    let (side, stacked, beside) = title_fit(full, labels, most, broke_w);
    let widths = [0, 1].map(|i| gauge_width(labels[i], most[i], side));
    let broke_at = |ui: &Ui, row: Rect, broke: &str| {
        ui.painter().text(
            pos2(row.left() + 1.0, row.center().y).round(),
            Align2::LEFT_CENTER,
            broke,
            look.font.clone(),
            look.ink,
        );
    };
    if let Some(b) = broke.as_deref().filter(|_| !beside) {
        let (row, _) = ui.allocate_exact_size(vec2(full, line_h), Sense::hover());
        broke_at(ui, row, b);
    }
    let rows = if stacked { 2 } else { 1 };
    let (area, _) = ui.allocate_exact_size(vec2(full, line_h * rows as f32), Sense::hover());
    let area = Rect::from_min_size(area.min.round(), area.size());
    let right = area.right() - 1.0;
    let first = Rect::from_min_size(area.min, vec2(full, line_h));
    if stacked {
        // Labels in line, the wider gauge flush right.
        let left = right - widths[0].max(widths[1]);
        for (i, (label, have, most)) in gauges.into_iter().enumerate() {
            let cy = first.center().y + i as f32 * line_h;
            gauge(ui, left, cy, label, (have, most), side, look);
        }
    } else {
        let mut left = right - widths[0] - BESIDE_GAP - widths[1];
        for (i, (label, have, most)) in gauges.into_iter().enumerate() {
            gauge(ui, left, first.center().y, label, (have, most), side, look);
            left += widths[i] + BESIDE_GAP;
        }
    }
    if let Some(b) = broke.as_deref().filter(|_| beside) {
        broke_at(ui, first, b);
    }
}

/// The Script card, top to bottom: the title line (the CPU and RAM gauges), the palette row
/// (the blocks to add, and moving or clearing the selected one), the program's blocks
/// (scrolling when they run past the card), then the transport alone at the foot: Step, Play,
/// fast-forward and Pause as a hi-fi's icon buttons, the one held in drawn down. Edits and
/// presses become actions; nothing here changes the game.
fn script_card(p: &mut Panes, ui: &mut Ui) {
    let g = p.game;
    let card = stack_fight::card_with(g, &forecast(ui, g));
    let look = card_look(p, ui);
    let ink = look.ink;
    let mut acts: Vec<Action> = Vec::new();
    title_line(ui, &card, &look);
    let mut sel = selected(ui, g);
    let gap = ui.spacing().item_spacing.x;
    let (chip_w, wrap) = palette_fit(ui.available_width().floor(), gap);
    // The palette: the blocks to add, then the selected block's move and clear boxes, beside
    // them or, in a narrow column, on a row of their own.
    let boxes = |ui: &mut Ui, sel: &mut Option<u32>, acts: &mut Vec<Action>| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            for (glyph, what) in [("X", "clear"), ("▼", "move down"), ("▲", "move up")] {
                let up = what == "move up";
                let to = sel.and_then(|no| stack_fight::moved_to(g, no, up));
                let on = match what {
                    "clear" => sel.is_some(),
                    _ => to.is_some(),
                };
                if palette_box(ui, glyph, what, on, &look)
                    && let Some(no) = *sel
                {
                    if what == "clear" {
                        acts.push(stack_fight::remove(g, no));
                        *sel = None;
                    } else if let Some(a) = stack_fight::swap(g, no, up) {
                        acts.push(a);
                        *sel = to;
                    }
                }
            }
        });
    };
    ui.horizontal(|ui| {
        for kind in stack_fight::PALETTE {
            if let Some(a) = palette_chip(ui, g, kind, chip_w, &look) {
                acts.push(a);
            }
        }
        if !wrap {
            boxes(ui, &mut sel, &mut acts);
        }
    });
    if wrap {
        ui.horizontal(|ui| boxes(ui, &mut sel, &mut acts));
    }
    ui.add_space(4.0);
    blocks(ui, g, &card, &look, &mut sel, &mut acts);
    ui.add_space(4.0);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let b = card.buttons;
        use stack_fight::Transport;
        // Right to left, so they read Step, Play, fast-forward, Pause. Play is the default
        // (Enter) while it presses.
        for t in Transport::ALL.into_iter().rev() {
            let (name, icon) = match t {
                Transport::Step => ("Step", liza::Icon::Step),
                Transport::Play => ("Run", liza::Icon::Play),
                Transport::Fast => ("Fast forward", liza::Icon::FastForward),
                Transport::Pause => ("Pause", liza::Icon::Pause),
            };
            let on = b.enabled(t);
            let default = t == Transport::Play && on;
            let held = b.latched == Some(t);
            if liza::icon_button(ui, name, icon, default, on, held, &look).clicked() {
                acts.push(t.action());
            }
        }
    });
    select(ui, sel);
    // A block dragged from the palette rides the pointer.
    if let Some(kind) = egui::DragAndDrop::payload::<BlockKind>(ui.ctx())
        && let Some(at) = ui.ctx().pointer_interact_pos()
    {
        let painter = ui.ctx().layer_painter(egui::LayerId::new(
            egui::Order::Tooltip,
            Id::new("stack-drag"),
        ));
        let r = Rect::from_min_size(at + vec2(8.0, 8.0), vec2(look::CHIP_W, look::CHIP_H));
        liza::shadowed(&painter, r, &look);
        painter.text(
            r.center(),
            Align2::CENTER_CENTER,
            kind.label(),
            look.font.clone(),
            ink,
        );
    }
    for a in acts {
        p.act(a);
    }
}

/// One of the palette's boxes for the selected block (move up, move down, clear): greyed
/// while it has nothing to do.
fn palette_box(ui: &mut Ui, glyph: &str, what: &str, on: bool, look: &liza::Look) -> bool {
    let (r, _) = ui.allocate_exact_size(vec2(look::CHIP_H, look::CHIP_H), Sense::hover());
    let r = Rect::from_min_size(r.min.round(), r.size());
    let resp = ui.interact(r, Id::new(("stack-palette-box", what)), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, on, what));
    let painter = ui.painter();
    // Greyed as Liza greys a button: the same box, the ink at half strength.
    let ink = if on {
        look.ink
    } else {
        look.ink.gamma_multiply(0.5)
    };
    let (fg, bg) = if on && resp.is_pointer_button_down_on() {
        (look.paper, ink)
    } else {
        (ink, look.paper)
    };
    painter.rect_filled(r, 0.0, bg);
    painter.rect_stroke(r, 0.0, Stroke::new(1.0, ink), StrokeKind::Inside);
    painter.text(
        r.center(),
        Align2::CENTER_CENTER,
        glyph,
        look.font.clone(),
        fg,
    );
    on && resp.clicked()
}

/// The program's rows in a framed box, a whole row at a time: the blocks, any stray text, and
/// the dotted slot at the end, with Liza's scroll bar once they run past it. The window keeps
/// the line about to run in view.
fn blocks(
    ui: &mut Ui,
    g: &Game,
    card: &stack_fight::Card,
    look: &liza::Look,
    sel: &mut Option<u32>,
    acts: &mut Vec<Action>,
) {
    // Under the box: a gap and the transport's row, with egui's spacing round each.
    let foot = 4.0
        + liza::size::BUTTON_H
        + 2.0 * liza::size::RING_OUT
        + 2.0 * ui.spacing().item_spacing.y
        + 2.0;
    let box_h = (ui.available_height() - foot)
        .max(2.0 * look::BLOCK_ROW)
        .floor();
    let (rect, _) =
        ui.allocate_exact_size(vec2(ui.available_width().floor(), box_h), Sense::hover());
    let rect = Rect::from_min_size(rect.min.round(), rect.size());
    let view_h = rect.height() - look::BLOCK_GAP;
    // Each row's height: a comment is a line, a block a framed box; the dotted slot ends it.
    let n = card.blocks.len() + card.stray.len() + 1;
    let height = |i: usize| match card.blocks.get(i) {
        Some(b) if b.kind == BlockKind::Rem => look::NOTE_ROW,
        Some(_) => look::BLOCK_ROW,
        None => look::NOTE_ROW,
    };
    let heights: Vec<f32> = (0..n).map(height).collect();
    let fits = |top: usize| {
        let mut used = 0.0;
        heights[top..]
            .iter()
            .take_while(|h| {
                used += **h;
                used <= view_h
            })
            .count()
    };
    // The furthest down the window goes: the last rows filling it.
    let max_top = (0..n)
        .find(|&t| heights[t..].iter().sum::<f32>() <= view_h)
        .unwrap_or(n - 1);
    let id = Id::new("stack-script");
    let key = id.with("top");
    let mut top: usize = ui.data(|d| d.get_temp(key)).unwrap_or(0);
    // Keep the line about to run in view when it moves.
    let current = card.blocks.iter().position(|b| b.current);
    if ui.data(|d| d.get_temp::<Option<usize>>(key.with("cur"))) != Some(current)
        && let Some(c) = current
    {
        if c < top {
            top = c;
        }
        while c >= top + fits(top) && top < c {
            top += 1;
        }
    }
    top = top.min(max_top);
    let before = |t: usize| heights[..t].iter().sum::<f32>();
    let s = crate::kit::scrollbar::Scroll {
        offset: before(top),
        view: view_h,
        content: heights.iter().sum(),
        line: look::BLOCK_ROW,
    };
    let bar = Rect::from_min_max(
        pos2(rect.right() - liza::size::SCROLL_W, rect.top()),
        rect.max,
    );
    let painter = ui.painter().clone();
    painter.rect_filled(rect, 0.0, look.paper);
    if s.active()
        && let Some(o) = liza::scroll_bar(ui, &painter, bar, id.with("bar"), s, look)
    {
        // A whole row at a time: the row nearest the offset.
        top = (0..=max_top)
            .min_by(|a, b| (before(*a) - o).abs().total_cmp(&(before(*b) - o).abs()))
            .unwrap_or(0);
    }
    painter.rect_stroke(rect, 0.0, Stroke::new(1.0, look.ink), StrokeKind::Inside);
    let right = if s.active() {
        bar.left()
    } else {
        rect.right() - 1.0
    };
    let mut y = rect.top();
    for (i, h) in heights.iter().enumerate().skip(top).take(fits(top)) {
        let row = Rect::from_min_size(
            pos2(rect.left() + 1.0, y),
            vec2((right - rect.left() - 1.0).min(look::BLOCK_W), *h),
        );
        y += h;
        if let Some(b) = card.blocks.get(i) {
            block_row(ui, row, g, b, sel, look, acts);
        } else if let Some((text, why)) = card.stray.get(i - card.blocks.len()) {
            let at = pos2(
                row.left() + look::GUTTER + look::BLOCK_PAD,
                row.center().y + 3.0,
            );
            painter.text(
                at,
                Align2::LEFT_CENTER,
                format!("{text}   {why}"),
                look.font.clone(),
                look.ink,
            );
        } else {
            end_slot(ui, row, g, look, acts);
        }
    }
    if s.active()
        && let Some(wheel) = crate::kit::scrollbar::take_wheel(ui, rect.with_max_x(bar.left()), id)
    {
        let rows = (wheel / look::BLOCK_ROW).round() as isize;
        top = (top as isize - rows).clamp(0, max_top as isize) as usize;
        ui.ctx().request_repaint();
    }
    ui.data_mut(|d| {
        d.insert_temp(key, top);
        d.insert_temp(key.with("cur"), current);
    });
}

/// A block from the palette: a click adds it at the end; dragged, it drops where it lands.
fn palette_chip(
    ui: &mut Ui,
    g: &Game,
    kind: BlockKind,
    w: f32,
    look: &liza::Look,
) -> Option<Action> {
    let (r, _) = ui.allocate_exact_size(vec2(w, look::CHIP_H), Sense::hover());
    let r = Rect::from_min_size(r.min.round(), r.size());
    let resp = ui.interact(r, Id::new(("stack-chip", kind)), Sense::click_and_drag());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, kind.label()));
    resp.dnd_set_drag_payload(kind);
    let painter = ui.painter();
    liza::shadowed(painter, r, look);
    let (fg, bg) = if resp.is_pointer_button_down_on() {
        (look.paper, look.ink)
    } else {
        (look.ink, look.paper)
    };
    painter.rect_filled(r.shrink(1.0), 0.0, bg);
    painter.text(
        r.center(),
        Align2::CENTER_CENTER,
        kind.label(),
        look.font.clone(),
        fg,
    );
    resp.clicked().then(|| stack_fight::add(g, kind)).flatten()
}

/// The dotted slot under the last block: a block dropped here goes at the end.
fn end_slot(ui: &mut Ui, row: Rect, g: &Game, look: &liza::Look, acts: &mut Vec<Action>) {
    let slot = Rect::from_min_max(
        pos2(row.left() + look::GUTTER, row.top() + look::BLOCK_GAP),
        row.right_bottom(),
    );
    let resp = ui.interact(slot, Id::new("stack-end-slot"), Sense::hover());
    if let Some(kind) = resp.dnd_release_payload::<BlockKind>() {
        acts.extend(stack_fight::add(g, *kind));
    }
    let painter = ui.painter();
    liza::dotted(painter, slot, look.ink);
    if resp.dnd_hover_payload::<BlockKind>().is_some() {
        liza::dotted(painter, slot.shrink(2.0), look.ink);
    }
}

/// How wide a block's face is drawn from its left edge to the end of what it says: its
/// keyword, caption, a bad line's text, its fields and its fault, with the room round them; a
/// comment's line of words.
fn content_width(ui: &Ui, b: &Block, look: &liza::Look) -> f32 {
    let font = &look.font;
    if b.kind == BlockKind::Rem {
        return look::BLOCK_PAD + text_w(ui, &format!("{}  -- {}", b.no, b.text), font);
    }
    let mut w = look::BLOCK_PAD + text_w(ui, &format!("({})", b.kind.label()), font);
    w += look::BLOCK_PAD;
    if let Some(caption) = b.caption {
        w += text_w(ui, caption, font) + look::FIELD_GAP;
    }
    if b.kind == BlockKind::Bad {
        w += text_w(ui, &b.text, font) + look::FIELD_GAP;
    }
    for f in &b.fields {
        w += match f {
            Field::Menu { value, options } => liza::popup_width(ui.ctx(), options, value, look),
            Field::Number(_) => look::NUMBER_W,
        } + look::FIELD_GAP;
    }
    if let Some(why) = &b.fault {
        w += text_w(ui, why, &fault_font(font));
    }
    w
}

/// The smaller type a block's number and fault are set in.
fn fault_font(font: &egui::FontId) -> egui::FontId {
    egui::FontId::new((font.size * 0.85).round(), font.family.clone())
}

/// One line of the program: the ▶ in the gutter when it's next, then the block (a framed box
/// with its number on the top edge, its keyword, its fields, its fault), in from the gutter by
/// one indent for each IF that guards it; a comment is a plain `--` line. A block the turn
/// won't execute is greyed, as the Mac greyed a disabled item, and nothing says why. A click on
/// it selects it (a dotted marquee round it); a block dropped on it goes in above it.
fn block_row(
    ui: &mut Ui,
    row: Rect,
    g: &Game,
    b: &Block,
    sel: &mut Option<u32>,
    look: &liza::Look,
    acts: &mut Vec<Action>,
) {
    let ink = look.ink;
    let font = look.font.clone();
    let small = fault_font(&font);
    let note = b.kind == BlockKind::Rem;
    let row = Rect::from_min_size(row.min.round(), row.size().round());
    let drop = ui.interact(row, Id::new(("stack-row", b.no)), Sense::hover());
    if let Some(kind) = drop.dnd_release_payload::<BlockKind>() {
        acts.extend(stack_fight::insert_before(g, b.no, *kind));
    }
    let painter = ui.painter().clone();
    if drop.dnd_hover_payload::<BlockKind>().is_some() {
        // Where it would land: a line above the block.
        let y = row.top() + (look::BLOCK_GAP / 2.0).floor();
        painter.line_segment(
            [pos2(row.left() + look::GUTTER, y), pos2(row.right(), y)],
            Stroke::new(2.0, ink),
        );
    }
    // In by its IFs, as far as the row has room to spare beside the block's own words and
    // fields: in a narrow column the indent gives way before anything runs off the card.
    let spare = row.width() - look::GUTTER - look::BLOCK_PAD - content_width(ui, b, look);
    let indent = (b.depth as f32 * look::INDENT).min(spare.max(0.0)).floor();
    let body = Rect::from_min_max(
        pos2(
            row.left() + look::GUTTER + indent,
            row.top() + look::BLOCK_GAP,
        ),
        pos2(row.right() - look::BLOCK_PAD, row.bottom()),
    );
    if b.current {
        painter.text(
            pos2(row.left() + 2.0, body.center().y),
            Align2::LEFT_CENTER,
            "▶",
            font.clone(),
            ink,
        );
    }
    // Selecting: the block's own face, under its fields (they're drawn after, so they win).
    let pick = ui.interact(body, Id::new(("stack-block", b.no)), Sense::click());
    pick.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("line {}", b.no))
    });
    if pick.clicked() {
        *sel = Some(b.no);
    }
    if *sel == Some(b.no) {
        liza::dotted(&painter, body.expand(2.0), ink);
    }
    if note {
        painter.text(
            pos2(body.left() + look::BLOCK_PAD, body.center().y),
            Align2::LEFT_CENTER,
            format!("{}  -- {}", b.no, b.text),
            font.clone(),
            ink,
        );
        return;
    }
    let frame = Rect::from_min_max(body.min.round(), body.max.round());
    painter.rect_filled(frame, 0.0, look.paper);
    painter.rect_stroke(frame, 0.0, Stroke::new(1.0, ink), StrokeKind::Inside);
    // The line's number, set into the top edge.
    let no = b.no.to_string();
    let no_at = pos2(frame.left() + look::BLOCK_PAD, frame.top());
    let no_w = text_w(ui, &no, &small);
    let no_box = Rect::from_min_size(no_at - vec2(3.0, 6.0), vec2(no_w + 6.0, 12.0));
    painter.rect_filled(no_box, 0.0, look.paper);
    painter.text(no_at, Align2::LEFT_CENTER, &no, small.clone(), ink);
    // The keyword: STOP stands out inverted, the breakpoint.
    let mut x = frame.left() + look::BLOCK_PAD;
    let cy = frame.center().y + 2.0;
    let word = format!("({})", b.kind.label());
    let word_w = text_w(ui, &word, &font);
    if b.kind == BlockKind::Stop {
        painter.rect_filled(
            Rect::from_min_size(pos2(x - 2.0, cy - 9.0), vec2(word_w + 4.0, 18.0)),
            0.0,
            ink,
        );
        painter.text(
            pos2(x, cy),
            Align2::LEFT_CENTER,
            &word,
            font.clone(),
            look.paper,
        );
    } else {
        painter.text(pos2(x, cy), Align2::LEFT_CENTER, &word, font.clone(), ink);
    }
    x += word_w + look::BLOCK_PAD;
    if let Some(caption) = b.caption {
        painter.text(pos2(x, cy), Align2::LEFT_CENTER, caption, font.clone(), ink);
        x += text_w(ui, caption, &font) + look::FIELD_GAP;
    }
    if b.kind == BlockKind::Bad {
        painter.text(pos2(x, cy), Align2::LEFT_CENTER, &b.text, font.clone(), ink);
        x += text_w(ui, &b.text, &font) + look::FIELD_GAP;
    }
    let field_h = liza::size::FIELD_H;
    for (ix, f) in b.fields.iter().enumerate() {
        let id = Id::new(("stack-field", b.no, ix));
        match f {
            Field::Menu { value, options } => {
                let fw = liza::popup_width(ui.ctx(), options, value, look);
                let r = Rect::from_min_size(pos2(x, cy - field_h / 2.0), vec2(fw, field_h));
                if let Some(k) = liza::popup_menu(ui, r, id, value, options, look)
                    && let Some(a) = stack_fight::set_field(g, b.no, ix, &options[k])
                {
                    acts.push(a);
                }
                x += fw + look::FIELD_GAP;
            }
            Field::Number(n) => {
                let r =
                    Rect::from_min_size(pos2(x, cy - field_h / 2.0), vec2(look::NUMBER_W, field_h));
                if let Some(t) = liza::text_field(ui, r, id, &n.to_string(), look)
                    && let Some(a) = stack_fight::set_field(g, b.no, ix, &t)
                {
                    acts.push(a);
                }
                x += look::NUMBER_W + look::FIELD_GAP;
            }
        }
    }
    if let Some(why) = &b.fault {
        painter.text(pos2(x, cy), Align2::LEFT_CENTER, why, small, ink);
    }
    if b.starved {
        liza::grey_out(&painter, frame.union(no_box), look);
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

    /// A frame of the ICE card in a `w`x`h` pane, the Stack's way; returns what it asked for.
    pub(crate) fn ice_card(
        hn: &mut crate::uitest::Harness,
        g: &Game,
        size: (f32, f32),
        ev: Vec<egui::Event>,
    ) -> Vec<Request> {
        pane(hn, g, size, ev, Tab::Target)
    }

    /// A frame of the Script card in a `w`x`h` pane; returns what it asked for.
    fn script(
        hn: &mut crate::uitest::Harness,
        g: &Game,
        size: (f32, f32),
        ev: Vec<egui::Event>,
    ) -> Vec<Request> {
        pane(hn, g, size, ev, Tab::Script)
    }

    /// A frame of one of the Stack's panes in a `w`x`h` rect.
    fn pane(
        hn: &mut crate::uitest::Harness,
        g: &Game,
        (w, h): (f32, f32),
        ev: Vec<egui::Event>,
        tab: Tab,
    ) -> Vec<Request> {
        let (mut term, mut focus, mut hex, mut keep) =
            (ninsei_term::LineEditor::default(), false, None, Vec::new());
        let mut suite = crate::suite::State::default();
        let mut out = Vec::new();
        hn.frame(ev, |ui| {
            let mut child = ui.new_child(
                egui::UiBuilder::new().max_rect(Rect::from_min_size(egui::Pos2::ZERO, vec2(w, h))),
            );
            let mut panes = Panes {
                game: g,
                pal: crate::theme::Palette::from_def(&g.data.themes[0]),
                map: ninsei_core::netview::MapView::Paint,
                deck: String::new(),
                chrome: ninsei_core::hardware::Chrome::Stack,
                corrupt: (0.0, 0),
                term: &mut term,
                focus_input: &mut focus,
                bbs_open: false,
                hex: &mut hex,
                keep: &mut keep,
                keep_room: 0,
                suite: &mut suite,
                hear: &Vec::new,
                scope_rate: 0,
                out: Vec::new(),
            };
            match tab {
                Tab::Target => Stack.target(&mut panes, &mut child),
                Tab::Deck => Stack.deck(&mut panes, &mut child),
                _ => Stack.script(&mut panes, &mut child),
            }
            out = panes.out;
        });
        out
    }

    /// A frame of the Stack's keys; returns what they asked for.
    fn keys(h: &mut crate::uitest::Harness, g: &Game, ev: Vec<egui::Event>) -> Vec<Request> {
        let mut suite = crate::suite::State::default();
        let mut out = Vec::new();
        h.frame(ev, |ui| {
            let mut popup = crate::volume::Popup::default();
            let mut cx = FrameCx {
                game: g,
                pal: crate::theme::Palette::from_def(&g.data.themes[0]),
                suite: &mut suite,
                keys_free: true,
                level: crate::volume::Level {
                    on: true,
                    volume: 100,
                },
                popup: &mut popup,
                alerts: &mut Default::default(),
            };
            out = Stack.keys(ui.ctx(), &mut cx);
        });
        out
    }

    /// The game actions in `out`, applied to `g`.
    fn apply(g: &mut Game, out: Vec<Request>) -> Vec<Action> {
        let acts: Vec<Action> = out
            .into_iter()
            .filter_map(|r| match r {
                Request::Cmd(ninsei_core::Command::Game(a)) => Some(a),
                _ => None,
            })
            .collect();
        for a in &acts {
            g.apply(a.clone());
        }
        acts
    }

    /// A fight on the first seed that meets ICE, running `program`.
    fn fight(program: &str) -> Game {
        let mut g = a_fight(&Data::builtin().unwrap());
        g.apply(Action::Program(program.into()));
        g
    }

    /// The Script column of the default layout, about.
    const PANE: (f32, f32) = (340.0, 520.0);

    fn press(h: &crate::uitest::Harness, text: &str) -> Vec<egui::Event> {
        let r = h
            .find(text)
            .unwrap_or_else(|| panic!("no {text:?} in {:?}", h.texts()));
        crate::uitest::click(r.center())
    }

    /// Clicks what says `text` in the Script card; returns what the card asked for.
    fn click_on(h: &mut crate::uitest::Harness, g: &Game, text: &str) -> Vec<Request> {
        let ev = press(h, text);
        script(h, g, PANE, ev)
    }

    /// Selects the block on line `no`: a click on its keyword, at the left of its face.
    fn select_line(h: &mut crate::uitest::Harness, g: &Game, no: u32) -> Vec<Request> {
        let r = h
            .find(&format!("line {no}"))
            .unwrap_or_else(|| panic!("no line {no} in {:?}", h.texts()));
        script(
            h,
            g,
            PANE,
            crate::uitest::click(pos2(r.left() + 12.0, r.center().y)),
        )
    }

    /// One frame as the run draws it: the Stack's keys first, then the Script card; returns
    /// what both asked for.
    fn run_frame(hn: &mut crate::uitest::Harness, g: &Game, ev: Vec<egui::Event>) -> Vec<Request> {
        let (mut term, mut focus, mut hex, mut keep) =
            (ninsei_term::LineEditor::default(), false, None, Vec::new());
        let (mut suite, mut suite2) = (
            crate::suite::State::default(),
            crate::suite::State::default(),
        );
        let mut out = Vec::new();
        hn.frame(ev, |ui| {
            let mut popup = crate::volume::Popup::default();
            let mut cx = FrameCx {
                game: g,
                pal: crate::theme::Palette::from_def(&g.data.themes[0]),
                suite: &mut suite2,
                keys_free: true,
                level: crate::volume::Level {
                    on: true,
                    volume: 100,
                },
                popup: &mut popup,
                alerts: &mut Default::default(),
            };
            out = Stack.keys(ui.ctx(), &mut cx);
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(Rect::from_min_size(egui::Pos2::ZERO, vec2(PANE.0, PANE.1))),
            );
            let mut panes = Panes {
                game: g,
                pal: crate::theme::Palette::from_def(&g.data.themes[0]),
                map: ninsei_core::netview::MapView::Paint,
                deck: String::new(),
                chrome: ninsei_core::hardware::Chrome::Stack,
                corrupt: (0.0, 0),
                term: &mut term,
                focus_input: &mut focus,
                bbs_open: false,
                hex: &mut hex,
                keep: &mut keep,
                keep_room: 0,
                suite: &mut suite,
                hear: &Vec::new,
                scope_rate: 0,
                out: Vec::new(),
            };
            Stack.script(&mut panes, &mut child);
            out.extend(panes.out);
        });
        out
    }

    #[test]
    fn an_open_menu_takes_enter_and_esc_from_the_card() {
        let mut g = fight("10 IF INTEG < 3\n20 RUN icepick");
        let mut h = crate::uitest::Harness::new(1280.0, 800.0);
        run_frame(&mut h, &g, vec![]);
        let menu = h.find("INTEG").unwrap();
        run_frame(&mut h, &g, crate::uitest::click(menu.center()));
        run_frame(&mut h, &g, vec![]);
        run_frame(&mut h, &g, crate::uitest::key(egui::Key::ArrowDown));
        let out = run_frame(&mut h, &g, crate::uitest::key(egui::Key::Enter));
        let acts = apply(&mut g, out);
        assert_eq!(acts.len(), 1, "{acts:?}");
        assert!(
            matches!(&acts[0], Action::Program(_)),
            "Enter picked, it didn't press Run"
        );
        assert_eq!(g.program.line(10).unwrap().text, "IF TRACE < 10");
        // While running, Esc on an open menu closes the menu and leaves the program running.
        g.apply(Action::Play);
        run_frame(&mut h, &g, vec![]);
        let menu = h.find("TRACE").unwrap();
        run_frame(&mut h, &g, crate::uitest::click(menu.center()));
        run_frame(&mut h, &g, vec![]);
        let out = run_frame(&mut h, &g, crate::uitest::key(egui::Key::Escape));
        assert!(apply(&mut g, out).is_empty());
        assert_eq!(g.exec.state, ninsei_core::game::ExecState::Running);
        run_frame(&mut h, &g, vec![]);
        assert!(h.find("CPU").is_none(), "the menu closed");
    }

    #[test]
    fn the_esc_that_leaves_the_message_box_doesnt_pause() {
        let mut g = fight("10 RUN icepick\n20 GOTO 10");
        let c = g.combat.as_mut().unwrap();
        (c.strength, c.strength_max) = (500, 500);
        g.apply(Action::Play);
        let mut h = crate::uitest::Harness::new(1280.0, 800.0);
        let mut line = String::new();
        let mut suite = crate::suite::State::default();
        let mut frame = |h: &mut crate::uitest::Harness, ev, focus: bool| {
            let mut out = Vec::new();
            h.frame(ev, |ui| {
                let mut popup = crate::volume::Popup::default();
                let mut cx = FrameCx {
                    game: &g,
                    pal: crate::theme::Palette::from_def(&g.data.themes[0]),
                    suite: &mut suite,
                    keys_free: true,
                    level: crate::volume::Level {
                        on: true,
                        volume: 100,
                    },
                    popup: &mut popup,
                    alerts: &mut Default::default(),
                };
                out = Stack.keys(ui.ctx(), &mut cx);
                // The Message box: a text field with the keyboard.
                let r = ui.text_edit_singleline(&mut line);
                if focus {
                    r.request_focus();
                }
            });
            out
        };
        frame(&mut h, vec![], true);
        frame(&mut h, vec![], true);
        let out = frame(&mut h, crate::uitest::key(egui::Key::Escape), false);
        assert!(
            out.is_empty(),
            "Esc only let go of the Message box: {}",
            out.len()
        );
        let out = frame(&mut h, crate::uitest::key(egui::Key::Escape), false);
        assert_eq!(out.len(), 1, "the next Esc pauses");
    }

    #[test]
    fn the_card_is_set_in_the_runs_type() {
        let g = fight("10 RUN icepick");
        let mut h = crate::uitest::Harness::new(1280.0, 800.0);
        crate::uitest::themed(&h.ctx, &g.data);
        script(&mut h, &g, PANE, vec![]);
        script(&mut h, &g, PANE, vec![]);
        let height = |t: &str| {
            h.painted_text()
                .into_iter()
                .find(|p| p.0 == t)
                .unwrap_or_else(|| panic!("no {t}"))
                .1
                .height()
        };
        assert_eq!(
            height("(RUN)"),
            height("RUN"),
            "the block's type is the palette's"
        );
    }

    #[test]
    fn the_ice_card_is_the_ice_and_the_script_is_its_own_pane() {
        let g = fight("10 RUN icepick\n20 STOP");
        let mut h = crate::uitest::Harness::new(1280.0, 800.0);
        ice_card(&mut h, &g, (600.0, 500.0), vec![]);
        ice_card(&mut h, &g, (600.0, 500.0), vec![]);
        assert!(h.find("END TURN").is_some(), "{:?}", h.texts());
        for word in ["Step", "Pause", "STOP"] {
            assert!(h.find(word).is_none(), "{word} isn't on the ICE card");
        }
        let mut h = crate::uitest::Harness::new(1280.0, 800.0);
        script(&mut h, &g, PANE, vec![]);
        script(&mut h, &g, PANE, vec![]);
        for word in ["Step", "Run", "Pause", "RUN", "IF", "GOTO", "STOP"] {
            assert!(h.find(word).is_some(), "{word} in {:?}", h.texts());
        }
        assert!(h.find("Close").is_none(), "no alert");
        assert_eq!(h.enabled("Pause"), Some(false), "nothing's running yet");
        assert_eq!(h.enabled("Run"), Some(true));
        let painted: Vec<String> = h.painted_text().into_iter().map(|t| t.0).collect();
        assert!(painted.iter().any(|t| t == "(RUN)") && painted.iter().any(|t| t == "(STOP)"));
        assert!(painted.iter().any(|t| t == "▶"), "the line about to run");
        assert!(!painted.iter().any(|t| t.contains("rezzed")));
    }

    #[test]
    fn the_ice_card_lists_the_ice_program_only_from_its_source() {
        let mut g = fight("");
        let ice = g.combat.as_ref().unwrap().ice;
        // No source: no listing, however long the fight.
        g.apply(Action::EndTurn);
        assert!(ninsei_core::ice_listing::rows(&g).is_empty());
        let mut h = crate::uitest::Harness::new(1280.0, 800.0);
        ice_card(&mut h, &g, (600.0, 500.0), vec![]);
        ice_card(&mut h, &g, (600.0, 500.0), vec![]);
        let row = &g.data.ice[ice];
        let prog = ninsei_core::ice_listing::decompile_def(&row.program, &row.verbs);
        for r in &prog {
            assert!(
                !h.texts().iter().any(|t| t.contains(&r.text)),
                "{:?} in {:?}",
                r.text,
                h.texts()
            );
        }
        // With its source, every line in clear.
        g.apply(ninsei_core::Action::GetSource {
            ice,
            version: ninsei_core::ice_version::Version::FIRST,
        });

        assert!(g.owns_source(ice, ninsei_core::ice_version::Version::FIRST));
        let rows = ninsei_core::ice_listing::rows(&g);
        assert_eq!(rows.len(), prog.len());
        ice_card(&mut h, &g, (600.0, 500.0), vec![]);
        ice_card(&mut h, &g, (600.0, 500.0), vec![]);
        for r in &rows {
            let line = ninsei_core::ice_listing::card_line(r);
            assert!(h.find(line.trim()).is_some(), "{line:?} in {:?}", h.texts());
        }
    }

    #[test]
    fn outside_a_fight_the_script_is_there_and_wont_play() {
        let d = Data::builtin().unwrap();
        let mut g = Game::new(d, 3);
        g.apply(Action::Program("10 RUN icepick\n20 GOTO 10".into()));
        assert_ne!(g.phase, Phase::Combat);
        let mut h = crate::uitest::Harness::new(1280.0, 800.0);
        script(&mut h, &g, PANE, vec![]);
        script(&mut h, &g, PANE, vec![]);
        for b in ["Step", "Run", "Fast forward", "Pause"] {
            assert_eq!(h.enabled(b), Some(false), "{b} with no ICE in front");
        }
        let painted: Vec<String> = h.painted_text().into_iter().map(|t| t.0).collect();
        assert!(painted.iter().any(|t| t == "(GOTO)"), "the program shows");
        assert!(!painted.iter().any(|t| t == "▶"), "nothing about to run");
        // Enter doesn't press a Run that can't be pressed.
        keys(&mut h, &g, vec![]);
        assert!(keys(&mut h, &g, crate::uitest::key(egui::Key::Enter)).is_empty());
        // And it still edits: the palette adds a line between fights.
        script(&mut h, &g, PANE, vec![]);
        let out = click_on(&mut h, &g, "STOP");
        apply(&mut g, out);
        assert_eq!(g.program.text(), "10 RUN icepick\n20 GOTO 10\n30 STOP");
    }

    #[test]
    fn a_selected_block_moves_and_clears_from_the_palette() {
        let mut g = fight("10 RUN icepick");
        let mut h = crate::uitest::Harness::new(1280.0, 800.0);
        script(&mut h, &g, PANE, vec![]);
        // The palette's GOTO (the first "GOTO" is the palette's: there's no GOTO block yet).
        let out = click_on(&mut h, &g, "GOTO");
        apply(&mut g, out);
        assert_eq!(g.program.text(), "10 RUN icepick\n20 GOTO 10");
        script(&mut h, &g, PANE, vec![]);
        for what in ["move up", "move down", "clear"] {
            assert_eq!(h.enabled(what), Some(false), "{what} with nothing selected");
        }
        // Select line 20: it can go up, not down.
        select_line(&mut h, &g, 20);
        script(&mut h, &g, PANE, vec![]);
        assert_eq!(h.enabled("move up"), Some(true));
        assert_eq!(h.enabled("move down"), Some(false), "the last block");
        let out = click_on(&mut h, &g, "move up");
        apply(&mut g, out);
        assert_eq!(g.program.text(), "10 GOTO 10\n20 RUN icepick");
        // The selection went with it, to line 10: now it can only go down.
        script(&mut h, &g, PANE, vec![]);
        assert_eq!(h.enabled("move up"), Some(false));
        assert_eq!(h.enabled("move down"), Some(true));
        let out = click_on(&mut h, &g, "clear");
        apply(&mut g, out);
        assert_eq!(g.program.text(), "20 RUN icepick");
        script(&mut h, &g, PANE, vec![]);
        assert_eq!(
            h.enabled("clear"),
            Some(false),
            "nothing selected once it's gone"
        );
    }

    #[test]
    fn the_deck_card_reads_whole_in_the_narrow_column() {
        let g = fight("10 RUN icepick");
        // The left column of the default layout at 1280 wide, and at 1150.
        for w in [250.0, 225.0] {
            let mut h = crate::uitest::Harness::new(1280.0, 800.0);
            pane(&mut h, &g, (w, 300.0), vec![], Tab::Deck);
            pane(&mut h, &g, (w, 300.0), vec![], Tab::Deck);
            assert!(h.cut_text().is_empty(), "{w}: {:?}", h.cut_text());
            for (t, r) in h.painted_text() {
                assert!(r.right() <= w + 0.5, "{w}: {t} at {r:?}");
            }
        }
    }

    #[test]
    fn the_palette_never_overlaps_in_a_narrow_column() {
        let g = fight("10 RUN icepick\n20 GOTO 10\n30 STOP");
        let parts = [
            "RUN",
            "IF",
            "GOTO",
            "STOP",
            "move up",
            "move down",
            "clear",
            "Step",
            "Run",
            "Fast forward",
            "Pause",
        ];
        for w in [340.0, 300.0, 260.0, 230.0] {
            let size = (w, 520.0);
            let mut h = crate::uitest::Harness::new(1280.0, 800.0);
            script(&mut h, &g, size, vec![]);
            script(&mut h, &g, size, vec![]);
            // A middle block selected, so the move and clear boxes are all live.
            let r = h.find("line 20").unwrap();
            script(
                &mut h,
                &g,
                size,
                crate::uitest::click(pos2(r.left() + 12.0, r.center().y)),
            );
            script(&mut h, &g, size, vec![]);
            let rects: Vec<(&str, Rect)> = parts
                .iter()
                .map(|p| (*p, h.find(p).unwrap_or_else(|| panic!("{w}: no {p}"))))
                .collect();
            for (i, (a, ra)) in rects.iter().enumerate() {
                assert!(
                    ra.right() <= w + 0.5,
                    "{w}: {a} at {ra:?} runs past the column"
                );
                for (b, rb) in &rects[i + 1..] {
                    assert!(!ra.intersects(*rb), "{w}: {a} {ra:?} overlaps {b} {rb:?}");
                }
            }
            assert!(h.cut_text().is_empty(), "{w}: {:?}", h.cut_text());
        }
    }

    #[test]
    fn a_hang_beside_stacked_gauges_clears_the_wider_one() {
        // A narrow column and a big RAM: the gauges stack, flush right under the wider (RAM),
        // so the hang only sits beside them if it clears that one, not just the CPU.
        let (side, stacked, beside) = title_fit(130.0, [24.0, 24.0], [3, 12], Some(60.0));
        assert!(stacked, "{side}");
        assert!(
            60.0 + BESIDE_GAP + gauge_width(24.0, 3, side) <= 130.0,
            "it clears the CPU"
        );
        assert!(!beside, "but not the RAM: the hang takes a row of its own");
        let (_, stacked, beside) = title_fit(400.0, [24.0, 24.0], [3, 12], Some(60.0));
        assert!(!stacked && beside);
    }

    #[test]
    fn the_palette_shrinks_its_chips_then_wraps_its_boxes() {
        assert_eq!(
            palette_fit(400.0, 8.0),
            (look::CHIP_W, false),
            "room to spare"
        );
        let (w, wrap) = palette_fit(300.0, 8.0);
        assert!(!wrap && (look::CHIP_MIN..look::CHIP_W).contains(&w), "{w}");
        let (w, wrap) = palette_fit(230.0, 8.0);
        assert!(wrap && w >= look::CHIP_MIN, "{w}");
    }

    #[test]
    fn a_block_has_no_boxes_of_its_own() {
        let g = fight("10 RUN icepick\n20 IF INTEG < 10\n30 GOTO 10");
        let mut h = crate::uitest::Harness::new(1280.0, 800.0);
        script(&mut h, &g, PANE, vec![]);
        script(&mut h, &g, PANE, vec![]);
        // With the middle block selected, one move-up, one move-down and one clear: the
        // palette's, however many blocks there are.
        select_line(&mut h, &g, 20);
        script(&mut h, &g, PANE, vec![]);
        for what in ["move up", "move down", "clear"] {
            assert_eq!(h.all(what).len(), 1, "{what}");
        }
    }

    #[test]
    fn a_field_edit_rewrites_the_program() {
        let mut g = fight("10 IF INTEG < 10\n20 RUN icepick");
        let mut h = crate::uitest::Harness::new(1280.0, 800.0);
        script(&mut h, &g, PANE, vec![]);
        // The comparison's pop-up: open it, then pick ">".
        click_on(&mut h, &g, "<");
        script(&mut h, &g, PANE, vec![]);
        let out = click_on(&mut h, &g, ">");
        assert_eq!(apply(&mut g, out).len(), 1);
        assert_eq!(g.program.line(10).unwrap().text, "IF INTEG > 10");
        // The RUN's program menu lists the disk; pick another.
        let other = g.hdd[1].spec.name.clone();
        script(&mut h, &g, PANE, vec![]);
        let shown = g.hdd[g.slot_named("icepick").unwrap()].spec.name.clone();
        click_on(&mut h, &g, &shown);
        script(&mut h, &g, PANE, vec![]);
        let out = click_on(&mut h, &g, &other);
        apply(&mut g, out);
        assert_eq!(g.program.line(20).unwrap().text, format!("RUN {other}"));
    }

    #[test]
    fn step_runs_a_line_and_the_message_box_says_so() {
        let mut g = fight("10 RUN icepick\n20 IF INTEG > 99\n30 GOTO 10");
        let mut h = crate::uitest::Harness::new(1280.0, 800.0);
        script(&mut h, &g, PANE, vec![]);
        let turn = g.combat.as_ref().unwrap().turn;
        let out = click_on(&mut h, &g, "Step");
        assert_eq!(apply(&mut g, out), [Action::Step]);
        assert_eq!(g.exec.pc, Some(20), "the ▶ moves to the next line");
        assert_eq!(g.combat.as_ref().unwrap().turn, turn, "the turn goes on");
        let before = g.log.len();
        script(&mut h, &g, PANE, vec![]);
        let out = click_on(&mut h, &g, "Step");
        assert_eq!(apply(&mut g, out), [Action::Step]);
        let said: Vec<&str> = g.log[before..].iter().map(|l| l.text.as_str()).collect();
        assert!(
            said.contains(&"20 IF INTEG > 99: no. skipping the next line."),
            "{said:?}"
        );
    }

    #[test]
    fn run_then_esc_pauses_and_the_hotbar_works() {
        let mut g = fight("10 RUN icepick\n20 GOTO 10");
        let c = g.combat.as_mut().unwrap();
        (c.strength, c.strength_max) = (500, 500);
        let mut h = crate::uitest::Harness::new(1280.0, 800.0);
        keys(&mut h, &g, vec![]);
        let out = keys(&mut h, &g, crate::uitest::key(egui::Key::Enter));
        assert_eq!(apply(&mut g, out), [Action::Play], "Enter presses Run");
        assert_eq!(g.exec.state, ninsei_core::game::ExecState::Running);
        let out = keys(&mut h, &g, crate::uitest::key(egui::Key::Escape));
        assert_eq!(apply(&mut g, out), [Action::Pause], "Esc pauses");
        // Esc again does nothing: there's nothing to close.
        assert!(keys(&mut h, &g, crate::uitest::key(egui::Key::Escape)).is_empty());
        ice_card(&mut h, &g, (600.0, 500.0), vec![]);
        ice_card(&mut h, &g, (600.0, 500.0), vec![]);
        let name = &g.hdd[0].spec.name;
        let slot = h
            .texts()
            .into_iter()
            .find(|t| t.starts_with("[0]") && t.contains(name.as_str()))
            .map(String::from)
            .unwrap_or_else(|| panic!("{:?}", h.texts()));
        let ev = press(&h, &slot);
        let out = ice_card(&mut h, &g, (600.0, 500.0), ev);
        assert_eq!(
            apply(&mut g, out),
            [Action::Run(0)],
            "the hotbar runs while paused"
        );
    }

    #[test]
    fn a_block_dragged_from_the_palette_lands_where_its_dropped() {
        let mut g = fight("10 RUN icepick\n20 RUN patch");
        let mut h = crate::uitest::Harness::new(1280.0, 800.0);
        script(&mut h, &g, PANE, vec![]);
        script(&mut h, &g, PANE, vec![]);
        let from = h.find("STOP").unwrap().center();
        // Over the block on line 20: it goes in above it.
        let to = h.find("line 20").unwrap().center();
        let at = |pos| egui::Event::PointerMoved(pos);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        script(&mut h, &g, PANE, vec![at(from), button(from, true)]);
        script(&mut h, &g, PANE, vec![at(from + vec2(20.0, -20.0))]);
        script(&mut h, &g, PANE, vec![at(to)]);
        let out = script(&mut h, &g, PANE, vec![button(to, false)]);
        apply(&mut g, out);
        assert_eq!(g.program.text(), "10 RUN icepick\n15 STOP\n20 RUN patch");
    }

    #[test]
    fn the_card_stays_whole_at_three_sizes() {
        let d = Data::builtin().unwrap();
        let mut s = ninsei_core::hub::Save::default();
        s.ensure_phonebook(&d, 0);
        let safe = s.program_text(&d);
        let g = fight(&safe);
        // The Script column at a small window, the default and a wide one.
        for size in [(300.0, 360.0), (340.0, 520.0), (480.0, 700.0)] {
            let mut h = crate::uitest::Harness::new(1280.0, 800.0);
            script(&mut h, &g, size, vec![]);
            script(&mut h, &g, size, vec![]);
            assert!(h.cut_text().is_empty(), "{size:?}: {:?}", h.cut_text());
            let pane = Rect::from_min_size(egui::Pos2::ZERO, vec2(size.0, size.1));
            for (t, r) in h.painted_text() {
                assert!(
                    r.min.x >= 0.0 && r.right() <= pane.right() + 0.5,
                    "{size:?}: {t} at {r:?}"
                );
            }
            for name in ["Step", "Run", "Fast forward", "Pause"] {
                let r = h.find(name).unwrap();
                assert!(pane.contains_rect(r), "{size:?}: {name} at {r:?}");
                assert_eq!(r.min, r.min.round(), "{name} on whole pixels");
            }
        }
    }

    /// The title line's gauges as last frame drew them: each label, the rect from it to its
    /// last block, and how many of its blocks are filled.
    fn gauges(h: &crate::uitest::Harness) -> Vec<(String, Rect, usize, usize)> {
        let painted = h.painted_text();
        let labels: Vec<(String, Rect)> = ["CPU", "RAM"]
            .iter()
            .filter_map(|l| painted.iter().find(|(t, _)| t == l).cloned())
            .collect();
        let squares: Vec<(Rect, bool)> = h
            .painted()
            .iter()
            .filter_map(|s| match s {
                egui::Shape::Rect(r)
                    if r.rect.width() == r.rect.height() && r.rect.width() <= 8.5 =>
                {
                    Some((r.rect, r.fill != Color32::TRANSPARENT))
                }
                _ => None,
            })
            .collect();
        labels
            .iter()
            .map(|(l, lr)| {
                // Its blocks: on its row, right of it, and left of any label after it there.
                let stop = labels
                    .iter()
                    .filter(|(_, o)| {
                        (o.center().y - lr.center().y).abs() < 2.0 && o.left() > lr.right()
                    })
                    .map(|(_, o)| o.left())
                    .fold(f32::INFINITY, f32::min);
                let mine: Vec<&(Rect, bool)> = squares
                    .iter()
                    .filter(|(r, _)| {
                        (r.center().y - lr.center().y).abs() < 3.0
                            && r.left() > lr.right()
                            && r.right() < stop
                    })
                    .collect();
                let all = mine.iter().fold(*lr, |a, (r, _)| a.union(*r));
                let filled = mine.iter().filter(|(_, f)| *f).count();
                (l.clone(), all, filled, mine.len())
            })
            .collect()
    }

    /// A hung fight: `10 GOTO 20` / `20 GOTO 10`, the ▶ held on line 10.
    fn hung() -> Game {
        let mut g = fight("10 GOTO 20\n20 GOTO 10");
        while g.exec.broke.is_none() {
            g.apply(Action::Step);
        }
        g
    }

    #[test]
    fn on_a_narrow_card_the_hang_takes_a_row_of_its_own() {
        let g = hung();
        for w in [340.0, 240.0, 200.0] {
            let mut h = crate::uitest::Harness::new(1280.0, 800.0);
            script(&mut h, &g, (w, 520.0), vec![]);
            script(&mut h, &g, (w, 520.0), vec![]);
            let painted = h.painted_text();
            let broke = painted
                .iter()
                .find(|(t, _)| t == "?BREAK IN 10")
                .unwrap_or_else(|| panic!("{w}: {painted:?}"));
            let gs = gauges(&h);
            assert_eq!(gs.len(), 2, "{w}: {gs:?}");
            for (l, r, ..) in &gs {
                assert!(
                    !broke.1.intersects(*r),
                    "{w}: they overlap: {broke:?} {l} {r:?}"
                );
                assert!(r.right() <= w + 0.5, "{w}: {l} at {r:?} runs past the card");
            }
            assert!(
                !gs[0].1.intersects(gs[1].1),
                "{w}: the gauges overlap: {gs:?}"
            );
        }
    }

    #[test]
    fn a_hang_is_said_on_the_cards_title_line_with_the_line_held() {
        let mut g = hung();
        assert_eq!(g.exec.broke, Some(10));
        let mut h = crate::uitest::Harness::new(1280.0, 800.0);
        script(&mut h, &g, PANE, vec![]);
        script(&mut h, &g, PANE, vec![]);
        let painted: Vec<(String, egui::Rect)> = h.painted_text();
        let broke = painted.iter().find(|(t, _)| t == "?BREAK IN 10");
        let gs = gauges(&h);
        let (Some(broke), Some(cpu)) = (broke, gs.first()) else {
            panic!("{painted:?}");
        };
        assert!(
            (broke.1.center().y - cpu.1.center().y).abs() < 1.5 && broke.1.right() < cpu.1.left(),
            "beside the gauges, clear of them: {broke:?} {cpu:?}"
        );
        assert!(painted.iter().any(|(t, _)| t == "▶"));
        g.apply(Action::Step);
        script(&mut h, &g, PANE, vec![]);
        assert!(!h.painted_text().iter().any(|(t, _)| t.contains("BREAK")));
    }

    #[test]
    fn the_gauges_are_blocks_with_no_numbers_and_drain_as_the_turn_runs() {
        let mut g = fight("10 IF INTEG > 0\n20 RUN icepick\n30 GOTO 10");
        let c = g.combat.as_mut().unwrap();
        (c.strength, c.strength_max) = (500, 500);
        let (cpu, ram) = (g.deck.cpu_max as usize, g.deck.ram_max as usize);
        let mut h = crate::uitest::Harness::new(1280.0, 800.0);
        script(&mut h, &g, PANE, vec![]);
        script(&mut h, &g, PANE, vec![]);
        let gs = gauges(&h);
        let counts: Vec<(&str, usize, usize)> =
            gs.iter().map(|(l, _, f, n)| (l.as_str(), *f, *n)).collect();
        assert_eq!(
            counts,
            [("CPU", cpu, cpu), ("RAM", ram, ram)],
            "full at the turn's start"
        );
        // No number anywhere on the title line.
        let title = gs[0].1.union(gs[1].1);
        for (t, r) in h.painted_text() {
            if r.intersects(title) {
                assert!(
                    !t.chars().any(|c| c.is_ascii_digit()),
                    "{t:?} on the title line"
                );
            }
        }
        // An IF and a RUN: a GB and a cycle gone.
        g.apply(Action::Step);
        g.apply(Action::Step);
        script(&mut h, &g, PANE, vec![]);
        let counts: Vec<(usize, usize)> = gauges(&h).iter().map(|(_, _, f, n)| (*f, *n)).collect();
        assert_eq!(counts, [(cpu - 1, cpu), (ram - 1, ram)]);
    }

    #[test]
    fn the_palette_is_on_top_and_the_transport_alone_at_the_foot() {
        let g = fight("10 RUN icepick\n20 GOTO 10");
        let mut h = crate::uitest::Harness::new(1280.0, 800.0);
        script(&mut h, &g, PANE, vec![]);
        script(&mut h, &g, PANE, vec![]);
        let first = h.find("line 10").unwrap();
        let last = h.find("line 20").unwrap();
        for chip in ["RUN", "IF", "GOTO", "STOP", "move up", "move down", "clear"] {
            let r = h.find(chip).unwrap();
            assert!(
                r.bottom() <= first.top(),
                "{chip} at {r:?} above the blocks {first:?}"
            );
        }
        let transport = ["Step", "Run", "Fast forward", "Pause"];
        let foot: Vec<Rect> = transport.iter().map(|t| h.find(t).unwrap()).collect();
        for (t, r) in transport.iter().zip(&foot) {
            assert!(r.top() >= last.bottom(), "{t} at {r:?} under the blocks");
            assert!(
                (r.center().y - foot[0].center().y).abs() < 0.5,
                "{t}: one row"
            );
        }
        // Left to right: Step, Run, Fast forward, Pause.
        assert!(
            foot.windows(2).all(|w| w[0].right() <= w[1].left()),
            "{foot:?}"
        );
        // Nothing else on the transport's row.
        let row = foot.iter().fold(foot[0], |a, r| a.union(*r));
        let others: Vec<(String, Rect)> = h
            .outside(Rect::NOTHING)
            .into_iter()
            .filter(|(t, r)| r.intersects(row) && !transport.contains(&t.trim()))
            .collect();
        assert!(others.is_empty(), "{others:?}");
        // The transport's icons are painted, not set in type.
        let painted: Vec<String> = h.painted_text().into_iter().map(|t| t.0).collect();
        for glyph in ["▶▶", "‖", "■", "Step", "Run", "Pause"] {
            assert!(!painted.iter().any(|t| t == glyph), "{glyph} set as text");
        }
    }

    #[test]
    fn fast_forward_latches_until_it_or_another_button_is_clicked() {
        let mut g = fight("10 RUN icepick\n20 GOTO 10");
        let c = g.combat.as_mut().unwrap();
        (c.strength, c.strength_max) = (500, 500);
        let mut h = crate::uitest::Harness::new(1280.0, 800.0);
        script(&mut h, &g, PANE, vec![]);
        script(&mut h, &g, PANE, vec![]);
        assert_eq!(h.ticked("Pause"), Some(true), "paused: Pause is down");
        let out = click_on(&mut h, &g, "Fast forward");
        assert_eq!(apply(&mut g, out), [Action::FastForward]);
        script(&mut h, &g, PANE, vec![]);
        assert_eq!(h.ticked("Fast forward"), Some(true), "it stays in");
        assert_eq!(h.ticked("Run"), Some(false));
        assert_eq!(h.ticked("Pause"), Some(false));
        // Clicked again it pops up, and the program plays on at pace.
        let out = click_on(&mut h, &g, "Fast forward");
        apply(&mut g, out);
        script(&mut h, &g, PANE, vec![]);
        assert_eq!(h.ticked("Fast forward"), Some(false));
        assert_eq!(h.ticked("Run"), Some(true), "Play is down");
        assert_eq!(
            h.enabled("Run"),
            Some(false),
            "and pressing it again does nothing"
        );
        // In again, then Run pops it up.
        let out = click_on(&mut h, &g, "Fast forward");
        apply(&mut g, out);
        script(&mut h, &g, PANE, vec![]);
        let out = click_on(&mut h, &g, "Run");
        assert_eq!(apply(&mut g, out), [Action::Play]);
        script(&mut h, &g, PANE, vec![]);
        assert_eq!(h.ticked("Fast forward"), Some(false));
        assert!(!g.fast_forward());
        // In again, then Pause pops it up.
        let out = click_on(&mut h, &g, "Fast forward");
        apply(&mut g, out);
        script(&mut h, &g, PANE, vec![]);
        let out = click_on(&mut h, &g, "Pause");
        assert_eq!(apply(&mut g, out), [Action::Pause]);
        script(&mut h, &g, PANE, vec![]);
        assert_eq!(h.ticked("Fast forward"), Some(false));
        assert_eq!(h.ticked("Pause"), Some(true));
        // And Enter still presses Run, Esc still pauses.
        keys(&mut h, &g, vec![]);
        let out = keys(&mut h, &g, crate::uitest::key(egui::Key::Enter));
        assert_eq!(apply(&mut g, out), [Action::Play]);
        let out = keys(&mut h, &g, crate::uitest::key(egui::Key::Escape));
        assert_eq!(apply(&mut g, out), [Action::Pause]);
    }

    /// The grey a block is drawn in: a pattern image laid over `r` (`liza::grey_out`).
    fn greyed(h: &crate::uitest::Harness, r: Rect) -> bool {
        h.painted().iter().any(|s| match s {
            egui::Shape::Mesh(m) => {
                let b = m.calc_bounds();
                b.contains_rect(r.shrink(1.0)) && b.height() < r.height() * 3.0
            }
            _ => false,
        })
    }

    #[test]
    fn a_starved_block_is_grey_and_still_selectable() {
        // A pick for each cycle, then one more and the torch (not on the disk).
        let n = a_fight(&Data::builtin().unwrap()).deck.cpu_max;
        let picks: String = (1..=n + 1)
            .map(|i| format!("{} RUN icepick\n", i * 10))
            .collect();
        let mut g = fight(&format!("{picks}{} RUN torch", (n + 2) * 10));
        let c = g.combat.as_mut().unwrap();
        (c.strength, c.strength_max) = (500, 500);
        let mut h = crate::uitest::Harness::new(1280.0, 800.0);
        script(&mut h, &g, (340.0, 700.0), vec![]);
        script(&mut h, &g, (340.0, 700.0), vec![]);
        for i in 1..=n + 2 {
            let r = h.find(&format!("line {}", i * 10)).unwrap();
            assert_eq!(greyed(&h, r), i > n, "line {}", i * 10);
        }
        // No words say why.
        let painted: Vec<String> = h.painted_text().into_iter().map(|t| t.0).collect();
        assert!(
            !painted
                .iter()
                .any(|t| t.contains("cpu") || t.contains("cycle")),
            "{painted:?}"
        );
        // Still selectable, and the palette moves it.
        let last = (n + 2) * 10;
        let r = h.find(&format!("line {last}")).unwrap();
        script(
            &mut h,
            &g,
            (340.0, 700.0),
            crate::uitest::click(pos2(r.left() + 12.0, r.center().y)),
        );
        script(&mut h, &g, (340.0, 700.0), vec![]);
        assert_eq!(h.enabled("move up"), Some(true));
        let ev = press(&h, "move up");
        let out = script(&mut h, &g, (340.0, 700.0), ev);
        apply(&mut g, out);
        assert_eq!(g.program.line(last - 10).unwrap().text, "RUN torch");
    }

    #[test]
    fn the_kept_forecast_follows_the_game() {
        let mut g = fight("10 IF INTEG > 0\n20 RUN icepick\n30 RUN icepick");
        let c = g.combat.as_mut().unwrap();
        (c.strength, c.strength_max) = (500, 500);
        let mut h = crate::uitest::Harness::new(1280.0, 800.0);
        script(&mut h, &g, PANE, vec![]);
        script(&mut h, &g, PANE, vec![]);
        let row = |h: &crate::uitest::Harness| h.find("line 30").unwrap();
        assert!(!greyed(&h, row(&h)), "two cycles for two picks");
        // The hotbar spends two cycles: the last pick starves, and the card says so.
        g.apply(Action::Step);
        let pick = g.slot_named("icepick").unwrap();
        g.apply(Action::Run(pick));
        g.apply(Action::Run(pick));
        script(&mut h, &g, PANE, vec![]);
        assert!(greyed(&h, row(&h)));
        // The next turn has the cycles again: the grey goes.
        g.apply(Action::EndTurn);
        script(&mut h, &g, PANE, vec![]);
        assert!(!greyed(&h, row(&h)));
    }

    #[test]
    fn a_guarded_block_is_indented_under_its_if() {
        let g = fight("10 IF INTEG < 10\n20 IF TURN > 1\n30 RUN icepick\n40 GOTO 10");
        let mut h = crate::uitest::Harness::new(1280.0, 800.0);
        script(&mut h, &g, PANE, vec![]);
        script(&mut h, &g, PANE, vec![]);
        let left =
            |h: &crate::uitest::Harness, no: u32| h.find(&format!("line {no}")).unwrap().left();
        assert!(left(&h, 20) > left(&h, 10), "the IF's guarded IF");
        assert!(left(&h, 30) > left(&h, 20), "two IFs in a row nest");
        assert_eq!(
            left(&h, 40),
            left(&h, 10),
            "an unguarded line is back at the left"
        );
        // Its fields move with it.
        let turn = h.find("TURN").unwrap().left();
        let integ = h.find("INTEG").unwrap().left();
        assert!(turn > integ, "{turn} {integ}");
        // The ▶ stays in the gutter.
        let mut g = g;
        g.apply(Action::Step);
        script(&mut h, &g, PANE, vec![]);
        let arrow = h.painted_text().into_iter().find(|t| t.0 == "▶").unwrap();
        assert!(arrow.1.right() <= left(&h, 10), "{arrow:?}");
    }

    #[test]
    fn a_guarded_fault_or_comment_stays_on_the_card() {
        // A guarded GOTO to nowhere says so after its fields; a guarded comment is a line of
        // its own words. The indent gives way for both.
        let g = fight(
            "10 IF INTEG < 10\n20 IF TURN > 1\n30 GOTO 999\n40 IF TURN > 1\n\
             50 REM patch, then the breaker, then go",
        );
        // The default column and a little wider (a GOTO's fault fills a 300px one unindented).
        for w in [340.0, 360.0] {
            let mut h = crate::uitest::Harness::new(1280.0, 800.0);
            script(&mut h, &g, (w, 520.0), vec![]);
            script(&mut h, &g, (w, 520.0), vec![]);
            assert!(h.cut_text().is_empty(), "{w}: {:?}", h.cut_text());
            for (t, r) in h.painted_text() {
                assert!(r.right() <= w + 0.5, "{w}: {t} at {r:?}");
            }
        }
    }

    #[test]
    fn the_new_card_is_whole_from_the_small_column_to_a_wide_one() {
        let g = fight(
            "10 IF INTEG < 10\n20 IF NEXT ATTACK\n30 RUN firewall\n40 IF HAVE icepick\n\
             50 RUN icepick\n60 RUN torch\n70 GOTO 10",
        );
        // The small window's column, the default's and a wide one: nested IFs and all, nothing
        // runs off the card (the indent gives way first).
        for size in [(300.0, 520.0), (340.0, 520.0), (480.0, 700.0)] {
            let mut h = crate::uitest::Harness::new(1280.0, 800.0);
            script(&mut h, &g, size, vec![]);
            script(&mut h, &g, size, vec![]);
            assert!(h.cut_text().is_empty(), "{size:?}: {:?}", h.cut_text());
            let pane = Rect::from_min_size(egui::Pos2::ZERO, vec2(size.0, size.1));
            assert!(
                h.outside(pane).is_empty(),
                "{size:?}: {:?}",
                h.outside(pane)
            );
            for (l, r, ..) in gauges(&h) {
                assert!(pane.contains_rect(r), "{size:?}: {l} at {r:?}");
            }
            for name in ["Step", "Run", "Fast forward", "Pause"] {
                let r = h.find(name).unwrap();
                assert!(pane.contains_rect(r), "{size:?}: {name} at {r:?}");
                assert_eq!(r.min, r.min.round(), "{name} on whole pixels");
            }
        }
    }
}
