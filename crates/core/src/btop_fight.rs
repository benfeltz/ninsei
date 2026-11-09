//! The Ono-Sendai's fight screen, decided here and drawn by the app's btop chrome: the
//! `ice.bas` tile, the Script pane's face, on screen for the whole run. The listing as gdb's
//! TUI shows a source window: line numbers and text, the line about to run marked in the
//! gutter, a STOP's dot beside it, a bad line dimmed with its fault; the budgets cut into the
//! bottom edge in btop's words. A line is retyped where it stands, as the C64's screen editor
//! did. What every listing shares (rows, keys, the cursor, the edits) is `listing`'s; this is
//! btop's words for it. The terminal is the prompt: the trace prints there.

use crate::game::Game;
use crate::ice_listing::IceRow;
pub use crate::listing::{
    Gutter, Keys, Row, delete, editing_text, home, keys, moved, open_after, retype, rows,
};

/// The tile's title, cut into its top edge.
pub const TITLE: &str = "ice.bas";

/// A gutter mark as the tile draws it: `▶` on the line about to run, `●` on a STOP.
pub fn mark(g: Gutter) -> &'static str {
    match g {
        Gutter::Current => "▶",
        Gutter::Stop => "●",
        Gutter::None => " ",
    }
}

/// The legend at the tile's foot, btop's way of showing a box's keys: two rows of a key, its
/// word and whether it presses now.
pub fn legend(k: &Keys) -> [[(&'static str, &'static str, bool); 4]; 2] {
    [
        [
            ("r", "run", k.run),
            ("s", "step", k.step),
            ("c", "cont", k.cont),
            ("p", "pause", k.pause),
        ],
        [
            ("e", "edit", k.edit),
            ("o", "open", k.open),
            ("d", "del", k.delete),
            ("j/k", "move", k.moves),
        ],
    ]
}

/// The bottom edge: what's left this turn in a fight, the deck's limits outside one; a hang
/// after them, in btop's lowercase, while it holds the cursor. A narrow tile cuts the note
/// from its end, so the budgets are the last thing to go.
pub fn note(g: &Game) -> String {
    let cpu = g.combat.as_ref().map_or(g.deck.cpu_max, |c| c.cpu);
    let broke = crate::listing::broke(g).map_or(String::new(), |n| format!("  ?break in {n}"));
    format!(
        "cpu {cpu}/{}  ram {}/{}{broke}",
        g.deck.cpu_max,
        g.ifs_left(),
        g.deck.ram_max
    )
}

/// The tile's title.
pub fn title(_g: &Game) -> &'static str {
    TITLE
}

/// The ICE's listing as the fight screens show it: up to `n` rows from the line it carries on
/// from, wrapping to the top as its program does.
pub fn ice_window(g: &Game, n: usize) -> Vec<IceRow> {
    let rows = crate::ice_listing::rows(g);
    let at = rows.iter().position(|r| r.cursor).unwrap_or(0);
    rows.iter()
        .cycle()
        .skip(at)
        .take(n.min(rows.len()))
        .cloned()
        .collect()
}

/// A row of the ICE's listing in btop's lowercase: `▶` on the line it carries on from.
pub fn ice_line(r: &IceRow) -> String {
    let mark = if r.cursor { "▶" } else { "·" };
    format!("{mark} {:>3} {}", r.no, r.shown().to_lowercase())
}

/// The heading over the ICE's listing: how much of it the deck has seen.
pub fn ice_heading(g: &Game) -> String {
    let (seen, all) = crate::ice_listing::known(&crate::ice_listing::rows(g));
    format!("PROG {seen}/{all}")
}
