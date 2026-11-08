//! The kit: the one layer every fake app draws with. Its text-mode parts (cells, lists,
//! sheets, how much room a screen needs) own geometry and behaviour and leave the look to the
//! caller; a desktop's GUI idiom (`anarchy`, `liza`, `workbench`) is that desktop's parts, drawn to its spec
//! sheet in `docs/refs/`. Every colour comes in from the theme.
//!
//! **The rule:** nothing here names a tool or a deck. If a widget only works by checking which
//! app is drawing it, it belongs in that app's renderer. New screens use the kit, and a
//! touched screen moves onto it.

pub mod anarchy;
pub mod cells;
pub mod fit;
pub mod list;
pub mod liza;
pub mod scrollbar;
pub mod sheet;
pub mod workbench;

pub use cells::Cells;
pub use fit::Fit;
pub use list::List;
pub use sheet::Sheet;
