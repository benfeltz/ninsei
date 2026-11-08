//! Every balance number in one place.

/// Deck the runner starts each run with.
pub const START_CPU: u32 = 3;
pub const START_RAM: u32 = 6;
pub const START_HDD: u32 = 6;
pub const START_INTEGRITY: u32 = 18;

/// What a new deck ships with before its kit is applied: the default kit's free programs
/// (`data/hub/starts.ron`; `Data::validate` keeps them the same). The Security Suite and a
/// generic breaker; the specialists come from the boards.
pub const START_WAREZ: &[&str] = &["firewall", "patch", "icepick"];
/// The four a deck shipped with before the suite: still free on saves from then, and what a
/// bare `Game::new` carries.
pub const CLASSIC_WAREZ: &[&str] = &["icepick", "firewall", "patch", "decoder"];

/// Trace is the doom clock: at `TRACE_MAX` you are flatlined.
pub const TRACE_MAX: u32 = 100;
pub const TRACE_PER_HOP: u32 = 4;
pub const TRACE_PER_COMBAT_TURN: u32 = 2;

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
/// Widest layer a network shape may ask for (the map has room for this many lanes).
pub const NET_WIDEST: usize = 6;

/// Subnets: the earliest layer one can sit on, and the private network behind it.
pub const SUBNET_MIN_LAYER: usize = 2;
pub const SUBNET_MID_LAYERS: usize = 3;
/// Extra vault weight inside a subnet: the reason to go in.
pub const SUBNET_VAULT_BONUS: u32 = 30;

/// BBS modem theatre, in seconds. Dialing lasts as long as the handshake (`modem.rs`).
pub const BBS_HANGUP_SECS: f32 = 1.2;

/// The dial-up handshake, in seconds: a 1200/2400-baud call, squeezed down.
/// `ATZ`/`OK`, then dial tone, touch tones, rings, the answer tone and the carrier until CONNECT.
pub const MODEM_OFFHOOK_SECS: f32 = 0.3;
pub const MODEM_DIALTONE_SECS: f32 = 0.3;
pub const MODEM_DIGIT_SECS: f32 = 0.07;
pub const MODEM_DIGIT_GAP_SECS: f32 = 0.05;
pub const MODEM_RINGS: usize = 2;
pub const MODEM_RING_SECS: f32 = 0.4;
pub const MODEM_RING_GAP_SECS: f32 = 0.25;
pub const MODEM_ANSWER_SECS: f32 = 0.7;
pub const MODEM_CARRIER_SECS: f32 = 0.9;
/// Quiet after CONNECT before the screen moves on.
pub const MODEM_SETTLE_SECS: f32 = 0.35;
/// Carrier symbol rate (both 1200 and 2400 bps modems signal at 600 baud).
pub const MODEM_BAUD: f32 = 600.0;
/// Fade in/out per tone, so nothing clicks.
pub const MODEM_RAMP_SECS: f32 = 0.004;
/// Peak loudness, 0-1.
pub const MODEM_VOLUME: f32 = 0.25;
/// The run's music reads the trace in two coarse steps (`music::arrangement`): from
/// `MUSIC_DROP_AT` percent the tune's drop track falls silent, and from `MUSIC_TEMPO_AT` it
/// plays `MUSIC_TEMPO_BPM` faster. Look only: no play depends on it.
pub const MUSIC_DROP_AT: u32 = 50;
pub const MUSIC_TEMPO_AT: u32 = 75;
pub const MUSIC_TEMPO_BPM: u32 = 12;
/// Simulated Zmodem throughput, bytes per second of game time.
pub const BBS_CPS: f32 = 42_000.0;

/// Lines of terminal history kept.
pub const LOG_LIMIT: usize = 400;

/// Hex-editor crafting. A program's effect cost must fit in
/// `BUDGET_PER_CPU * cpu + BUDGET_PER_OPT * opt`. 4 a CPU keeps every shipped program in
/// budget now that RAM is no longer a program's cost (a class breaker costs 4 on 1 CPU).
pub const BUDGET_PER_CPU: u32 = 4;
pub const BUDGET_PER_OPT: u32 = 2;
/// Flat budget cost of a stun.
pub const COST_STUN: u32 = 7;
pub const MAX_POWER: u32 = 40;
pub const MAX_CPU: u32 = 9;
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
/// Betrayal: chance (percent) a client turns on you when the target falls, by tier (1, 2, 3)...
pub const BETRAY_PCT: [u32; 3] = [5, 16, 24];
/// ... plus a point for every this many credits they owe ...
pub const BETRAY_PER_CREDITS: u32 = 30;
/// ... less this much for every clean job you've done for them (counted up to `BETRAY_TRUST_MAX`) ...
pub const BETRAY_TRUST_PCT: u32 = 4;
pub const BETRAY_TRUST_MAX: u32 = 5;
/// ... halved when you earned the ghost bonus, and never more than this.
pub const BETRAY_MAX_PCT: u32 = 40;

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
/// Run results remembered per phonebook number (NiGHTCOMM's LAST column).
pub const RESULTS_KEPT: usize = 5;
/// Per point of heat: extra ICE strength, extra trace gain (percent).
pub const HEAT_ICE: u32 = 1;
pub const HEAT_TRACE_PCT: u32 = 5;

/// Wardialer: numbers per scan, the range everyone knows, extra trace per tripped trap (percent).
pub const WARDIAL_BLOCK: u32 = 100;
pub const LOCAL_RANGE: &str = "local";
pub const TRAP_TRACE_PCT: u32 = 5;

/// Default phone bill for calling a BBS from home (a board can set its own `call_cost`).
pub const BBS_CALL_COST: u32 = 10;

/// Credits the dev key (F8 at home) drops in the bank, for trying out the shops.
pub const DEV_CREDITS: u32 = 1000;

/// The games on the desktop (`crates/core/src/games/`). Arcade rules, never a run's balance:
/// the probe never plays them. Periods are in the game's own ticks (`GameDef::tick_ms`).
/// NIBBLER: ticks between moves at the start and at its fastest, meals between speed-ups,
/// points a meal, and the snake's length when it hatches.
pub const NIBBLER_START_PERIOD: u32 = 4;
pub const NIBBLER_FASTEST_PERIOD: u32 = 1;
pub const NIBBLER_SPEEDUP_EVERY: u32 = 5;
pub const NIBBLER_FOOD_POINTS: u32 = 10;
pub const NIBBLER_START_LEN: usize = 3;
/// BLOX: ticks between drops at level 0, how much faster each level is and the fastest drop;
/// rows a level; points for clearing 1-4 rows at once (times the level plus one); points a
/// row for a soft and a hard drop.
pub const BLOX_START_PERIOD: u32 = 16;
pub const BLOX_PERIOD_STEP: u32 = 1;
pub const BLOX_FASTEST_PERIOD: u32 = 2;
pub const BLOX_LINES_PER_LEVEL: u32 = 10;
pub const BLOX_LINE_POINTS: [u32; 5] = [0, 40, 100, 300, 1200];
pub const BLOX_SOFT_POINTS: u32 = 1;
pub const BLOX_HARD_POINTS: u32 = 2;
/// MINEFIELD: share of the squares that hide a mine, in percent (at least one mine).
pub const MINEFIELD_MINE_PCT: usize = 15;
