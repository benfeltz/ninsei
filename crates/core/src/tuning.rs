//! Every balance number in one place.

/// Deck the runner starts each run with.
pub const START_CPU: u32 = 3;
pub const START_RAM: u32 = 6;
pub const START_HDD: u32 = 6;
pub const START_INTEGRITY: u32 = 18;

/// Warez on the HDD at the start of a run. The first ones that fit in RAM start loaded.
pub const START_WAREZ: &[&str] = &["icepick", "firewall", "patch", "decoder"];

/// Trace is the doom clock: at `TRACE_MAX` you are flatlined.
pub const TRACE_MAX: u32 = 100;
pub const TRACE_PER_HOP: u32 = 3;
pub const TRACE_PER_COMBAT_TURN: u32 = 2;

/// CPU it costs to load a program in the middle of a fight. Loading outside a fight is free.
pub const COMBAT_LOAD_CPU: u32 = 1;

/// Network shape: entry layer, `NET_MID_LAYERS` layers of nodes, exit layer.
pub const NET_MID_LAYERS: usize = 5;
pub const NET_MIN_WIDTH: usize = 2;
pub const NET_MAX_WIDTH: usize = 4;
/// Chance (percent) of an extra link to a neighbouring lane.
pub const NET_EXTRA_LINK_PCT: u32 = 35;
/// Node kind weights for mid layers.
pub const NODE_WEIGHT_ICE: u32 = 60;
pub const NODE_WEIGHT_VAULT: u32 = 22;
pub const NODE_WEIGHT_BLACK_ICE: u32 = 18;
/// Black ICE only appears from this layer on.
pub const BLACK_ICE_MIN_LAYER: usize = 3;
/// ICE gets this much extra strength per layer deep.
pub const ICE_STRENGTH_PER_LAYER: u32 = 3;

/// Layer the network's one BBS sits on.
pub const BBS_LAYER: usize = 3;

/// BBS modem theatre, in seconds.
pub const BBS_DIAL_SECS: f32 = 2.4;
pub const BBS_HANGUP_SECS: f32 = 1.2;
/// Simulated Zmodem throughput, bytes per second of game time.
pub const BBS_CPS: f32 = 42_000.0;

/// Lines of terminal history kept.
pub const LOG_LIMIT: usize = 400;
