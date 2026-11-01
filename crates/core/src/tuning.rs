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
pub const TRACE_PER_HOP: u32 = 4;
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
pub const NODE_WEIGHT_ICE: u32 = 50;
pub const NODE_WEIGHT_VAULT: u32 = 18;
pub const NODE_WEIGHT_BLACK_ICE: u32 = 16;
pub const NODE_WEIGHT_CONTACT: u32 = 18;
/// Chance (percent) a network with an ICE-class bias picks ICE of that class.
pub const ICE_BIAS_PCT: u32 = 60;
/// Black ICE only appears from this layer on.
pub const BLACK_ICE_MIN_LAYER: usize = 3;
/// ICE gets this much extra strength per layer deep.
pub const ICE_STRENGTH_PER_LAYER: u32 = 4;

/// Layer the network's one BBS sits on.
pub const BBS_LAYER: usize = 3;

/// BBS modem theatre, in seconds.
pub const BBS_DIAL_SECS: f32 = 2.4;
pub const BBS_HANGUP_SECS: f32 = 1.2;
/// Simulated Zmodem throughput, bytes per second of game time.
pub const BBS_CPS: f32 = 42_000.0;

/// Lines of terminal history kept.
pub const LOG_LIMIT: usize = 400;

/// Hex-editor crafting. A program's effect cost must fit in
/// `BUDGET_PER_CPU * cpu + ram / BUDGET_RAM_DIV + BUDGET_PER_OPT * opt`.
pub const BUDGET_PER_CPU: u32 = 3;
pub const BUDGET_RAM_DIV: u32 = 2;
pub const BUDGET_PER_OPT: u32 = 2;
/// Flat budget cost of a stun.
pub const COST_STUN: u32 = 7;
pub const MAX_POWER: u32 = 40;
pub const MAX_CPU: u32 = 9;
pub const MAX_RAM: u32 = 16;
pub const MAX_OPT: u32 = 5;
/// Optimizer fragments consumed per opt level.
pub const FRAGMENTS_PER_OPT: u32 = 1;

/// Contracts. Reward is rolled in this range, doubled for black ICE, plus a per-layer bonus.
pub const CONTRACT_REWARD_MIN: u32 = 60;
pub const CONTRACT_REWARD_MAX: u32 = 110;
pub const CONTRACT_BLACK_MULT: u32 = 2;
pub const CONTRACT_PER_LAYER: u32 = 12;
/// Chance (percent) a contract carries a ghost bonus: extra pay if trace stays at or under a cap.
pub const CONTRACT_GHOST_PCT: u32 = 45;
/// The ghost cap is current trace plus this much.
pub const CONTRACT_GHOST_MARGIN: u32 = 14;
/// Ghost bonus as a percentage of the reward.
pub const CONTRACT_GHOST_BONUS_PCT: u32 = 50;
/// Jobs posted on the BBS board.
pub const BBS_JOBS: usize = 2;

/// Selling programs: extra credits per opt level on top of the per-GB price.
pub const SELL_PER_OPT: u32 = 25;

/// Hub: stash slots before any RIG upgrade.
pub const STASH_BASE_SLOTS: u32 = 2;
/// Hub: share of a run's credits the dead man's switch wires home on a flatline (percent).
pub const DEADMAN_PCT: u32 = 10;

/// Hub: seconds the Workbench screen takes to drag off (or back over) a run.
pub const HUB_DRAG_SECS: f32 = 0.9;
/// Pixel-art colour codes allowed in desktop icons.
pub const HUB_PALETTE_CHARS: &str = ".wkob";

/// Phonebook: tiers of the numbers a new save starts with.
pub const PHONEBOOK_START: &[u32] = &[1, 1, 1, 1, 2];
/// Heat: added to a network each run on it, removed from every other network.
pub const HEAT_PER_RUN: u32 = 2;
pub const HEAT_COOL: u32 = 1;
pub const HEAT_MAX: u32 = 6;
/// Per point of heat: extra ICE strength, extra trace gain (percent).
pub const HEAT_ICE: u32 = 1;
pub const HEAT_TRACE_PCT: u32 = 5;
