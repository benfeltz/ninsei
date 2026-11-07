//! The kit: the one layer every fake app draws with. It owns geometry and behaviour (whole
//! cells, scrolling, how much room a screen needs); the look stays in each app's renderer and
//! every colour comes in from the caller, from the theme.
//!
//! **The rule:** nothing here names a tool or a deck. If a widget only works by checking which
//! app is drawing it, it belongs in that app's renderer. New screens use the kit, and a
//! touched screen moves onto it.

pub mod cells;
pub mod fit;
pub mod list;

pub use cells::Cells;
pub use fit::Fit;
pub use list::List;
