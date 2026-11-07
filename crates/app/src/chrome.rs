//! Chromes: how the run screen is dressed. A chrome is a [`ChromeView`]: one method per pane,
//! plus its tab titles, menus, status line, an optional bar under the panes, keys, an overlay
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
    /// CRT effects and sounds on or off.
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
    /// The net pane (secondary).
    fn net(&self, p: &mut Panes, ui: &mut Ui) {
        crate::netpane::live(ui, p.game, &p.pal);
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
    /// Whether it has a bar under the panes (the suite's F-key legend).
    fn has_legend(&self) -> bool {
        false
    }
    /// Draws the bar under the panes.
    fn legend(&self, _ui: &mut Ui, _cx: &mut FrameCx) -> Vec<Request> {
        Vec::new()
    }
    /// Keys of its own, read every frame the run is on screen (on top of the shared hotkeys).
    fn keys(&self, _ctx: &egui::Context, _cx: &mut FrameCx) -> Vec<Request> {
        Vec::new()
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
/// terminal and todo below, and any secondary panes beside the terminal.
pub fn base_layout(extra: &[Tab]) -> DockState<Tab> {
    let mut dock = DockState::new(vec![Tab::Network, Tab::Target, Tab::Hex]);
    let tree = dock.main_surface_mut();
    let [center, left] = tree.split_left(NodeIndex::root(), 0.27, vec![Tab::Deck]);
    tree.split_below(left, 0.42, vec![Tab::Warez]);
    let [_, bottom] = tree.split_below(center, 0.68, vec![Tab::Terminal, Tab::Todo]);
    if !extra.is_empty() {
        tree.split_right(bottom, 0.55, extra.to_vec());
    }
    dock
}

pub struct Classic;
impl ChromeView for Classic {}

pub struct Btop;
impl ChromeView for Btop {
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
