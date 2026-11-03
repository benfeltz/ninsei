//! A terminal line editor for ninsei's terminals: readline (emacs-mode) keys, a kill ring of one,
//! history with `Ctrl+R` search, and Tab completion over candidates the caller supplies.
//!
//! The editor ([`LineEditor`]) and the key table ([`keys`]) are pure and tested headlessly. The
//! egui widget (feature `egui`, [`widget`]) only maps key events to [`Edit`]s and draws the line.
//! The design follows egui_term (a bindings table from key + modifiers to actions, and a
//! painter-drawn line), without its PTY: the game is the shell, so there's no process and no
//! thread, and the same code runs natively and in the browser.

mod editor;
pub mod keys;
#[cfg(feature = "egui")]
pub mod widget;

pub use editor::{Edit, LineEditor, Outcome, View};
