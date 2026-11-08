//! The Stack chrome (the Sense/Net deck): the run screen as a HyperCard stack. The deck is a
//! field of meters that fill with darker patterns as they rise, programs are rounded buttons
//! with a hatched shadow, the terminal is the Message box under a scrolling log field, and a
//! fight puts the Script card under the ICE: the deck's program as a stack of operator blocks
//! with fields, after HyperCard 1.2's button script editor, and Step, Run and Pause. What the
//! card shows is core's (`ninsei_core::stack_fight`); this only draws it and routes edits. Two
//! theme colours (lit ink on dark paper); anything it doesn't draw is Classic.

use egui::{Align2, Color32, Id, Rect, RichText, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
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
    pub const BLOCK_W: f32 = 900.0;
    pub const BLOCK_PAD: f32 = 8.0;
    pub const NUMBER_W: f32 = 44.0;
    pub const FIELD_GAP: f32 = 6.0;
    /// The palette's blocks: their width and height.
    pub const CHIP_W: f32 = 56.0;
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

impl ChromeView for Stack {
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
        use egui_dock::{DockState, NodeIndex};
        let mut dock = DockState::new(vec![Tab::Network, Tab::Target, Tab::Hex]);
        let tree = dock.main_surface_mut();
        let [top, bottom] = tree.split_below(NodeIndex::root(), 0.74, vec![Tab::Terminal]);
        let [centre, left] = tree.split_left(top, 0.2, vec![Tab::Deck]);
        tree.split_below(left, 0.42, vec![Tab::Warez]);
        tree.split_right(centre, 0.66, vec![Tab::Script, Tab::Todo]);
        if !extra.is_empty() {
            tree.split_right(bottom, 0.6, extra.to_vec());
        }
        dock
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
        p.target(ui);
    }

    /// The Script card, the whole run: Step, Run and Pause press only with ICE in front.
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
        let free = cx.keys_free && !ctx.egui_wants_keyboard_input();
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

/// The Script card: a line with the budgets, the program's blocks (scrolling when they
/// run past the card), the palette row (the blocks to add, and moving or clearing the selected
/// one), then Step, Run and Pause on a row of their own. Edits and presses become actions;
/// nothing here changes the game.
fn script_card(p: &mut Panes, ui: &mut Ui) {
    let g = p.game;
    let card = stack_fight::card(g);
    let look = card_look(p, ui);
    let ink = look.ink;
    let mut acts: Vec<Action> = Vec::new();
    // The budgets line, at the right: the pane's tab already names the card.
    let line_h = ui.fonts_mut(|f| f.row_height(&look.font)).ceil() + 4.0;
    let (title, _) =
        ui.allocate_exact_size(vec2(ui.available_width().floor(), line_h), Sense::hover());
    ui.painter().text(
        pos2(title.right() - 1.0, title.center().y).round(),
        Align2::RIGHT_CENTER,
        card.budget_words(),
        look.font.clone(),
        ink,
    );
    let mut sel = selected(ui, g);
    blocks(ui, g, &card, &look, &mut sel, &mut acts);
    ui.add_space(4.0);
    // The palette: the blocks to add, then the selected block's move and clear boxes.
    ui.horizontal(|ui| {
        for kind in stack_fight::PALETTE {
            if let Some(a) = palette_chip(ui, g, kind, &look) {
                acts.push(a);
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            for (glyph, what) in [("X", "clear"), ("▼", "move down"), ("▲", "move up")] {
                let up = what == "move up";
                let to = sel.and_then(|no| stack_fight::moved_to(g, no, up));
                let on = match what {
                    "clear" => sel.is_some(),
                    _ => to.is_some(),
                };
                if palette_box(ui, glyph, what, on, &look)
                    && let Some(no) = sel
                {
                    if what == "clear" {
                        acts.push(stack_fight::remove(g, no));
                        sel = None;
                    } else if let Some(a) = stack_fight::swap(g, no, up) {
                        acts.push(a);
                        sel = to;
                    }
                }
            }
        });
    });
    ui.add_space(4.0);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let b = card.buttons;
        // Right to left: Pause, Run, Step, so they read Step, Run, Pause.
        for (label, on, default, a) in [
            ("Pause", b.pause, false, Action::Pause),
            ("Run", b.run, b.run, Action::Play),
            ("Step", b.step, false, Action::Step),
        ] {
            let pressed = ui
                .add_enabled_ui(on, |ui| liza::push_button(ui, label, default, &look))
                .inner
                .clicked();
            if pressed {
                acts.push(a);
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
    // Under the box: a gap, the palette row, a gap, the buttons' row, with egui's spacing
    // round each.
    let foot = 4.0
        + look::CHIP_H
        + 4.0
        + liza::size::BUTTON_H
        + 2.0 * liza::size::RING_OUT
        + 4.0 * ui.spacing().item_spacing.y
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
fn palette_chip(ui: &mut Ui, g: &Game, kind: BlockKind, look: &liza::Look) -> Option<Action> {
    let (r, _) = ui.allocate_exact_size(vec2(look::CHIP_W, look::CHIP_H), Sense::hover());
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

/// One line of the program: the ▶ in the gutter when it's next, then the block (a framed box
/// with its number on the top edge, its keyword, its fields, its fault); a comment is a plain
/// `--` line. A click on it selects it (a dotted marquee round it); a block dropped on it goes
/// in above it.
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
    let small = egui::FontId::new((font.size * 0.85).round(), font.family.clone());
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
    let body = Rect::from_min_max(
        pos2(row.left() + look::GUTTER, row.top() + look::BLOCK_GAP),
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
    painter.rect_filled(
        Rect::from_min_size(no_at - vec2(3.0, 6.0), vec2(no_w + 6.0, 12.0)),
        0.0,
        look.paper,
    );
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
    fn outside_a_fight_the_script_is_there_and_wont_play() {
        let d = Data::builtin().unwrap();
        let mut g = Game::new(d, 3);
        g.apply(Action::Program("10 RUN icepick\n20 GOTO 10".into()));
        assert_ne!(g.phase, Phase::Combat);
        let mut h = crate::uitest::Harness::new(1280.0, 800.0);
        script(&mut h, &g, PANE, vec![]);
        script(&mut h, &g, PANE, vec![]);
        for b in ["Step", "Run", "Pause"] {
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
    fn step_runs_a_pass_and_the_message_box_says_so() {
        let mut g = fight("10 RUN icepick\n20 IF INTEG > 99\n30 GOTO 10");
        let mut h = crate::uitest::Harness::new(1280.0, 800.0);
        script(&mut h, &g, PANE, vec![]);
        let turn = g.combat.as_ref().unwrap().turn;
        let before = g.log.len();
        let out = click_on(&mut h, &g, "Step");
        assert_eq!(apply(&mut g, out), [Action::Step]);
        if g.phase == Phase::Combat {
            assert_eq!(g.combat.as_ref().unwrap().turn, turn + 1);
        }
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
            for name in ["Step", "Run", "Pause"] {
                let r = h.find(name).unwrap();
                assert!(pane.contains_rect(r), "{size:?}: {name} at {r:?}");
                assert_eq!(r.min, r.min.round(), "{name} on whole pixels");
            }
        }
    }
}
