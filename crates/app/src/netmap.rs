//! The main editor pane: the network. Two views share what's visible, what's clickable and
//! what each node is called: floating isometric wireframes (`netmap_iso`) or a flat technical
//! draft (`netmap_draft`). Returns the node the player clicked.

use egui::{Rect, Sense, Ui};
use ninsei_core::netview::{MapNode, MapView, style};
use ninsei_core::{ContractStatus, Game, NodeKind, Phase};

use crate::theme::Palette;

/// Nodes further than this many layers past the runner are fogged.
const SIGHT: usize = 1;

pub fn show(ui: &mut Ui, game: &Game, p: &Palette, view: MapView) -> Option<usize> {
    match view {
        MapView::Iso => crate::netmap_iso::show(ui, game, p),
        MapView::Draft => crate::netmap_draft::show(ui, game, p),
    }
}

/// Whether the runner can see what node `id` is. The BBS number is passed around, and the
/// client gave you contract targets' hostnames; the rest is fog past the next layer.
pub fn visible(game: &Game, id: usize) -> bool {
    let net = &game.net;
    net.nodes[id].layer <= net.nodes[game.at].layer + SIGHT
        || net.nodes[id].kind == NodeKind::Bbs
        || target(game, id)
}

/// Whether node `id` is an active contract's target. They live on the main network, so none
/// show inside a subnet.
pub fn target(game: &Game, id: usize) -> bool {
    !game.in_subnet()
        && game
            .contracts
            .iter()
            .any(|k| k.node == id && k.status == ContractStatus::Active)
}

pub fn kind(game: &Game, id: usize) -> MapNode {
    MapNode::of(&game.net, id, &game.data)
}

/// The short label under a node.
pub fn tag(game: &Game, id: usize) -> String {
    if !visible(game, id) {
        return "???".into();
    }
    let k = kind(game, id);
    if k == MapNode::Exit && game.in_subnet() {
        return "UPLINK".into();
    }
    style(&game.data, k).tag.clone()
}

/// Makes a node's screen area clickable when it's a route, with a tooltip. Returns the node
/// if it was clicked, and whether it's hovered.
pub fn interact(ui: &mut Ui, game: &Game, id: usize, r: Rect) -> (Option<usize>, bool) {
    let can_go = game.reachable().contains(&id);
    let sense = if can_go {
        Sense::click()
    } else {
        Sense::hover()
    };
    let resp = ui.interact(r, ui.id().with(("node", id)), sense);
    let hovered = resp.hovered();
    if !can_go {
        return (None, hovered);
    }
    let what = if visible(game, id) {
        game.node_label(id)
    } else {
        "unknown"
    };
    let resp = resp.on_hover_text(format!("jack {id}  →  {what}"));
    (resp.clicked().then_some(id), hovered)
}

/// The line along the bottom: what to do now.
pub fn hint(game: &Game) -> &'static str {
    match game.phase {
        Phase::Exploring if game.in_subnet() => {
            "inside a subnet. the UPLINK takes you back to the main network"
        }
        Phase::Exploring => "click a lit node (or `jack <n>`) to route there",
        Phase::Combat => "ICE holds the line. break it in target.ice",
        Phase::Encounter => "someone wants to talk",
        Phase::Flatlined => "FLATLINED. `newrun` to jack in again",
        Phase::JackedOut => "JACKED OUT. `newrun` to go deeper",
    }
}
