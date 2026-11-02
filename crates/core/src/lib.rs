//! Pure game rules for ninsei. Nothing in this crate knows about egui, windows or pixels:
//! the app sends [`Action`]s in and renders the [`Game`] state that comes out.

pub mod ansi;
pub mod bbs;
pub mod binary;
pub mod command;
pub mod cue;
pub mod data;
pub mod desktop;
pub mod display;
pub mod game;
pub mod glitch;
pub mod hardware;
pub mod hexedit;
pub mod history;
pub mod home_bbs;
pub mod hub;
pub mod leet;
pub mod modem;
pub mod netview;
pub mod network;
pub mod panes;
pub mod rng;
pub mod suite;
pub mod tuning;

pub use command::{Command, parse_command};
pub use data::{
    BbsDef, BoardSrc, ChoiceEffect, Data, Drop, Effect, IceClass, IceDef, Intent, Penalty, Sources,
    WarezDef, WarezKind,
};
pub use game::{
    Action, Combat, Contract, ContractStatus, Deck, Encounter, Game, Kit, LogKind, LogLine, Phase,
    RunMods, Site, Slot, StockFile,
};
pub use network::{NetParams, Network, Node, NodeKind, Shape};
pub use rng::Rng;
