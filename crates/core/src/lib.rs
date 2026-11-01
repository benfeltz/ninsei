//! Pure game rules for ninsei. Nothing in this crate knows about egui, windows or pixels:
//! the app sends [`Action`]s in and renders the [`Game`] state that comes out.

pub mod ansi;
pub mod bbs;
pub mod binary;
pub mod command;
pub mod data;
pub mod game;
pub mod hexedit;
pub mod network;
pub mod rng;
pub mod tuning;

pub use command::{Command, parse_command};
pub use data::{
    BbsDef, Data, Drop, Effect, IceClass, IceDef, Intent, Sources, WarezDef, WarezKind,
};
pub use game::{Action, Combat, Deck, Game, LogKind, LogLine, Phase, Slot, StockFile};
pub use network::{Network, Node, NodeKind};
pub use rng::Rng;
