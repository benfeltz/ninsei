//! Chromes: how the run screen is dressed. A chrome is a [`ChromeView`]: one method per pane,
//! plus its tab titles, menus, status line, an optional bar over the panes, keys, an overlay
//! and the layout it starts with. Every method has a Classic default, so a new chrome only
//! writes what it changes. The deck's firmware names the chrome (`hardware::Chrome`); [`view`]
//! hands back its renderer.

use egui::Ui;
use egui_dock::{DockState, NodeIndex};
use ninsei_core::hardware::Chrome;
use ninsei_core::{Action, Game};

use crate::panes::{Panes, Request, Tab};
use crate::theme::Palette;

/// One section of a menu. A chrome arranges these into its own menus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuPart {
    /// End turn.
    Turn,
    /// The `help` command.
    Help,
    /// Reset the layout.
    Layout,
    /// Add a secondary pane you own.
    Panes,
    /// Damage glitches on, reduced or off.
    Glitches,
    /// CRT effects on or off.
    Toggles,
    /// Opens the sound pop-out (the level and the switch).
    Sound,
    /// Monitors you own.
    Monitors,
    /// Decks you own.
    Decks,
    /// Themes.
    Themes,
}

pub type Menus = &'static [(&'static str, &'static [MenuPart])];

const CLASSIC_MENUS: Menus = &[
    ("run", &[MenuPart::Turn, MenuPart::Help]),
    (
        "view",
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
    ("theme", &[MenuPart::Themes]),
];

/// What a chrome's frame (bars, keys, overlay) can read and change.
pub struct FrameCx<'a> {
    pub game: &'a Game,
    pub pal: Palette,
    pub suite: &'a mut crate::suite::State,
    /// No modal (BBS, talk, the sound pop-out) has the keyboard.
    pub keys_free: bool,
    /// The sound switch and level, as the save has them.
    pub level: crate::volume::Level,
    /// The sound pop-out: open or not.
    pub popup: &'a mut crate::volume::Popup,
}

pub trait ChromeView {
    fn deck(&self, p: &mut Panes, ui: &mut Ui) {
        p.deck(ui);
    }
    fn warez(&self, p: &mut Panes, ui: &mut Ui) {
        p.warez(ui);
    }
    fn network(&self, p: &mut Panes, ui: &mut Ui) {
        p.network(ui);
    }
    fn target(&self, p: &mut Panes, ui: &mut Ui) {
        p.target(ui);
    }
    fn hex(&self, p: &mut Panes, ui: &mut Ui) {
        p.hex_pane(ui);
    }
    fn terminal(&self, p: &mut Panes, ui: &mut Ui) {
        p.terminal(ui);
    }
    fn todo(&self, p: &mut Panes, ui: &mut Ui) {
        p.todo(ui);
    }
    /// The deck's program, on screen for the whole run.
    fn script(&self, p: &mut Panes, ui: &mut Ui) {
        p.script(ui);
    }
    /// The net pane (secondary).
    fn net(&self, p: &mut Panes, ui: &mut Ui) {
        crate::netpane::live(ui, p.game, &p.pal);
    }
    /// The vis pane (secondary): what the speaker is playing.
    fn vis(&self, p: &mut Panes, ui: &mut Ui) {
        let samples = (p.hear)();
        crate::vispane::alive(ui, &samples);
        let centre = crate::vispane::Centre::Line(p.pal.faint);
        crate::vispane::trace(ui, &p.pal, &samples, centre);
    }

    /// The tab brought forward when ICE rezzes or the run ends.
    fn combat_tab(&self) -> Tab {
        Tab::Target
    }
    /// A tab's title, if this chrome names it differently.
    fn title(&self, _tab: Tab) -> Option<&'static str> {
        None
    }
    /// The dock's scroll bars for a tab, if this chrome wants them different.
    fn scroll_bars(&self, _tab: Tab) -> Option<[bool; 2]> {
        None
    }
    /// The name at the left of the menu bar.
    fn brand(&self) -> &'static str {
        "▌ninsei"
    }
    fn menus(&self) -> Menus {
        CLASSIC_MENUS
    }
    /// Draws its own status line; false keeps the classic one.
    fn status(&self, _ui: &mut Ui, _pal: &Palette, _game: &Game) -> bool {
        false
    }
    /// Extras on the classic status line (btop's sparklines and its speaker).
    fn status_extra(&self, _ui: &mut Ui, _cx: &mut FrameCx) -> Vec<Request> {
        Vec::new()
    }
    /// Whether it has a bar over the panes, under the menu bar (the suite's F-key legend).
    fn has_legend(&self) -> bool {
        false
    }
    /// Draws the bar over the panes.
    fn legend(&self, _ui: &mut Ui, _cx: &mut FrameCx) -> Vec<Request> {
        Vec::new()
    }
    /// Keys of its own, read every frame the run is on screen, before the shared hotkeys: a key
    /// it consumes (an alert's Enter) never reaches them.
    fn keys(&self, _ctx: &egui::Context, _cx: &mut FrameCx) -> Vec<Request> {
        Vec::new()
    }
    /// Whether the Script pane holds the keyboard as a pane, not as typing (btop's `ice.bas`
    /// tile with the focus): the run's hotkeys (Space, the digits) still work then.
    fn script_has_keys(&self, _ctx: &egui::Context) -> bool {
        false
    }
    /// Drawn over everything after the panes (the suite's dialogs).
    fn overlay(&self, _ctx: &egui::Context, _cx: &mut FrameCx) -> Vec<Request> {
        Vec::new()
    }
    /// The sound pop-out while it's open: the level and the sound switch (`volume`). It owns
    /// the keyboard.
    fn sound(&self, ctx: &egui::Context, cx: &mut FrameCx) -> Vec<Request> {
        crate::volume::classic(ctx, cx)
    }
    /// The menu item that opens the sound pop-out.
    fn sound_item(&self) -> &'static str {
        "volume..."
    }
    /// The layout a firmware starts with; `extra` are the secondary panes it has.
    fn default_layout(&self, extra: &[Tab]) -> DockState<Tab> {
        base_layout(extra)
    }
    /// A 1-bit chrome: damage glitches tear and smear, but never split colour.
    fn one_bit(&self) -> bool {
        false
    }
    /// The run's colours under this chrome, from the theme's (a 1-bit chrome folds them).
    fn palette(&self, p: Palette) -> Palette {
        p
    }
    /// How the dock (tabs, frames, splitters) is dressed, from the shared style.
    fn dock_style(&self, s: egui_dock::Style, _p: &Palette) -> egui_dock::Style {
        s
    }
    /// The status line's background.
    fn status_fill(&self, p: &Palette) -> egui::Color32 {
        p.faint
    }
}

/// The shared layout: map, target and hex in the middle, deck and warez on the left, the
/// terminal, todo and the script below, and any secondary panes beside the terminal.
pub fn base_layout(extra: &[Tab]) -> DockState<Tab> {
    let mut dock = DockState::new(vec![Tab::Network, Tab::Target, Tab::Hex]);
    let tree = dock.main_surface_mut();
    let [center, left] = tree.split_left(NodeIndex::root(), 0.27, vec![Tab::Deck]);
    tree.split_below(left, 0.42, vec![Tab::Warez]);
    let [_, bottom] = tree.split_below(center, 0.68, vec![Tab::Terminal, Tab::Todo, Tab::Script]);
    if !extra.is_empty() {
        tree.split_right(bottom, 0.55, extra.to_vec());
    }
    dock
}

pub struct Classic;
impl ChromeView for Classic {}

/// Where the three-column layout splits, as shares: the panes over the bottom row (the
/// terminal, the Message box, the activity log), the left column's share of them, the deck's
/// share of the left column, the centre's share of what the left leaves, and the bottom row's
/// share left to the terminal when secondary panes sit beside it.
pub struct Shares {
    pub over_bottom: f32,
    pub left: f32,
    pub deck: f32,
    pub centre_of_rest: f32,
    pub terminal_of_bottom: f32,
}

/// The three-column layout every deck with a Script pane starts in: the deck over the
/// programs on the left (narrow), the map with ICE and Hex as its other tabs in the centre
/// (widest), the Script in front of To Do on the right, the terminal along the bottom with any
/// secondary panes beside it. Each chrome only picks its `Shares`.
pub fn three_columns(extra: &[Tab], s: Shares) -> DockState<Tab> {
    let mut dock = DockState::new(vec![Tab::Network, Tab::Target, Tab::Hex]);
    let tree = dock.main_surface_mut();
    let [top, bottom] = tree.split_below(NodeIndex::root(), s.over_bottom, vec![Tab::Terminal]);
    let [centre, left] = tree.split_left(top, s.left, vec![Tab::Deck]);
    tree.split_below(left, s.deck, vec![Tab::Warez]);
    tree.split_right(centre, s.centre_of_rest, vec![Tab::Script, Tab::Todo]);
    if !extra.is_empty() {
        tree.split_right(bottom, s.terminal_of_bottom, extra.to_vec());
    }
    dock
}

pub struct Btop;
impl ChromeView for Btop {
    /// Three columns over the terminal, the Stack's shape: deck.sys over warez/ on the left
    /// (narrow), the map in the centre (widest, the fight view in a fight), `ice.bas` in front
    /// of todo on the right; secondary panes beside the terminal.
    fn default_layout(&self, extra: &[Tab]) -> DockState<Tab> {
        three_columns(
            extra,
            Shares {
                over_bottom: 0.74,
                left: 0.22,
                deck: 0.62,
                centre_of_rest: 0.59,
                terminal_of_bottom: 0.6,
            },
        )
    }
    fn network(&self, p: &mut Panes, ui: &mut Ui) {
        // In a fight the map dives into the ICE's node and becomes the fight view.
        if p.game.phase == ninsei_core::Phase::Combat {
            crate::fightview::show(ui, p);
        } else {
            p.network(ui);
        }
    }
    fn combat_tab(&self) -> Tab {
        Tab::Network
    }
    fn deck(&self, p: &mut Panes, ui: &mut Ui) {
        p.btop_deck(ui);
    }
    fn warez(&self, p: &mut Panes, ui: &mut Ui) {
        p.btop_warez(ui);
    }
    fn target(&self, p: &mut Panes, ui: &mut Ui) {
        p.btop_target(ui);
    }
    fn terminal(&self, p: &mut Panes, ui: &mut Ui) {
        p.btop_terminal(ui);
    }
    fn todo(&self, p: &mut Panes, ui: &mut Ui) {
        p.btop_todo(ui);
    }
    /// The `ice.bas` tile, the whole run.
    fn script(&self, p: &mut Panes, ui: &mut Ui) {
        crate::btop::script::show(p, ui);
    }
    fn title(&self, tab: Tab) -> Option<&'static str> {
        (tab == Tab::Script).then_some(" ice.bas ")
    }
    /// The `ice.bas` tile's letters, while it has the keyboard.
    fn keys(&self, ctx: &egui::Context, cx: &mut FrameCx) -> Vec<Request> {
        crate::btop::script::keys(ctx, cx)
    }
    fn script_has_keys(&self, ctx: &egui::Context) -> bool {
        crate::btop::script::has_keys(ctx)
    }
    /// The tile lays out its own rows.
    fn scroll_bars(&self, tab: Tab) -> Option<[bool; 2]> {
        (tab == Tab::Script).then_some([false, false])
    }
    fn net(&self, p: &mut Panes, ui: &mut Ui) {
        let site = p
            .game
            .site
            .as_ref()
            .map_or("LOCAL".to_string(), |s| s.name.to_uppercase());
        let (game, pal) = (p.game, p.pal);
        crate::btop::boxed(ui, &pal, &format!("net  {site}"), "en0", |ui| {
            crate::netpane::live(ui, game, &pal);
        });
    }
    fn status_extra(&self, ui: &mut Ui, cx: &mut FrameCx) -> Vec<Request> {
        crate::btop::status_sparks(ui, &cx.pal, cx.game);
        crate::btop::speaker(ui, cx)
    }
    fn sound(&self, ctx: &egui::Context, cx: &mut FrameCx) -> Vec<Request> {
        crate::btop::sound_popup(ctx, cx)
    }
    fn vis(&self, p: &mut Panes, ui: &mut Ui) {
        let samples = (p.hear)();
        crate::vispane::alive(ui, &samples);
        crate::btop::vis_box(ui, &p.pal, &samples, p.scope_rate);
    }
}

/// The renderer for a chrome.
pub fn view(c: Chrome) -> &'static dyn ChromeView {
    match c {
        Chrome::Classic => &Classic,
        Chrome::Btop => &Btop,
        Chrome::Suite => &crate::suite::Suite,
        Chrome::Stack => &crate::stack::Stack,
    }
}

/// Pushes a game action as a request.
pub fn act(a: Action) -> Request {
    Request::Cmd(ninsei_core::Command::Game(a))
}

#[cfg(test)]
mod version_tests {
    use super::*;
    use crate::uitest::Harness;
    use egui::{Rect, UiBuilder, pos2, vec2};
    use ninsei_core::ice_version::{Bump, Version};

    /// A fight against a kind whose version in force is 1.0.1.
    fn patched_fight() -> ninsei_core::Game {
        let mut g = crate::stack::tests::a_fight(&ninsei_core::Data::builtin().unwrap());
        let c = g.combat.as_ref().unwrap();
        let ice = c.ice;
        let prog = c.ice_prog.clone();
        g.ice_versions.push((
            ice,
            Version::default().bumped(Bump::Patch),
            prog,
            Some(Bump::Patch),
        ));
        g
    }

    /// Everything painted or labelled by chrome `chrome`'s pane `draw` in a fight.
    fn drawn(
        g: &ninsei_core::Game,
        chrome: Chrome,
        draw: impl Fn(&dyn ChromeView, &mut Panes, &mut Ui),
    ) -> Vec<String> {
        let mut h = Harness::new(1280.0, 900.0);
        // Long enough for the Ono's fight view to finish jacking in.
        for _ in 0..40 {
            let (mut term, mut focus, mut hex, mut keep) =
                (ninsei_term::LineEditor::default(), false, None, Vec::new());
            let mut suite = crate::suite::State::default();
            h.frame(vec![], |ui| {
                let mut panes = Panes {
                    game: g,
                    pal: crate::theme::Palette::from_def(&g.data.themes[0]),
                    map: ninsei_core::netview::MapView::Iso,
                    deck: String::new(),
                    chrome,
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
                let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(900.0, 700.0));
                let mut w = ui.new_child(UiBuilder::new().max_rect(rect));
                draw(view(chrome), &mut panes, &mut w);
            });
        }
        let mut out: Vec<String> = h.texts().iter().map(|s| s.to_string()).collect();
        // Text-mode panes paint a glyph a cell: put each painted row back together.
        let mut glyphs = h.painted_text();
        glyphs.sort_by(|a, b| {
            (a.1.center().y.round(), a.1.left())
                .partial_cmp(&(b.1.center().y.round(), b.1.left()))
                .unwrap()
        });
        let mut rows: Vec<(f32, String)> = Vec::new();
        for (t, r) in glyphs {
            let y = r.center().y.round();
            match rows.last_mut() {
                Some((ly, s)) if (*ly - y).abs() < 2.0 => s.push_str(&t),
                _ => rows.push((y, t)),
            }
        }
        out.extend(rows.into_iter().map(|(_, s)| s));
        out
    }

    #[test]
    fn every_deck_names_the_ice_with_its_version() {
        let g = patched_fight();
        let label = g.ice_label(g.combat.as_ref().unwrap().ice).to_uppercase();
        assert!(label.ends_with(" 1.0.1"), "{label}");
        // Cell-painted rows have no glyphs for their spaces: compare without them.
        let bare = |t: &str| t.chars().filter(|c| !c.is_whitespace()).collect::<String>();
        let has = |texts: &[String]| texts.iter().any(|t| bare(t).contains(&bare(&label)));
        // The Sense/Net's ICE card (and the classic panes' target).
        let t = drawn(&g, Chrome::Stack, |v, p, ui| v.target(p, ui));
        assert!(has(&t), "stack card: {t:?}");
        // The Ono-Sendai's target.ice and its fight view.
        let t = drawn(&g, Chrome::Btop, |v, p, ui| v.target(p, ui));
        assert!(has(&t), "target.ice: {t:?}");
        let t = drawn(&g, Chrome::Btop, |v, p, ui| v.network(p, ui));
        assert!(has(&t), "fight view: {t:?}");
        // The Hosaka's incident report.
        let t = drawn(&g, Chrome::Suite, |v, p, ui| v.target(p, ui));
        assert!(has(&t), "incident report: {t:?}");
    }
}
