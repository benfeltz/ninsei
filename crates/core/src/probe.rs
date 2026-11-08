//! The balance probe: bots play the real game (`Save::dial`, `Game::apply`, `Save::record`,
//! `Save::buy`, `Save::wardial`), so it measures what a player meets, not a model of it.
//!
//! Two parts:
//! - **Spot checks**: single runs on a plain network of each tier, by a dumb bot (first route,
//!   first breaker, never shields) and a competent one ([`play`]).
//! - **Careers**: the competent bot from a fresh save, run after run. It climbs tiers when it
//!   keeps getting out, falls back when it doesn't, spends its bank in RIG and wardials for
//!   bigger numbers. The report says how long the climb takes, what the money buys and when,
//!   what kills runs and which programs carry fights.
//!
//! [`Probe`] is resumable (`step` plays a few runs and returns), so the home Shell's `probe`
//! command can run it a little each frame in the browser, where there are no threads. The CLI
//! is `cargo run -p ninsei-core --example balance`. Same seed, same report.

use crate::bbs::Line;
use crate::data::{ChoiceEffect, Data, Drop, Effect, Intent, Penalty};
use crate::fight::Outcome;
use crate::game::{Action, ContractStatus, Game, Phase};
use crate::hub::{Entry, RunEnd, Save, Upgrade};
use crate::network::NodeKind;
use crate::rng::Rng;
use crate::tuning::{BETRAY_PCT, TRACE_MAX};

/// How big a probe to run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    /// Single runs per tier, per bot, for the spot checks.
    pub spot_runs: u32,
    pub careers: u32,
    /// Runs in each career.
    pub career_runs: u32,
    pub seed: u64,
}

impl Config {
    /// What the CLI runs.
    pub const FULL: Config = Config {
        spot_runs: 300,
        careers: 100,
        career_runs: 100,
        seed: 0x5EED,
    };

    /// A size for `careers` careers in the Shell (spot checks scale with it).
    pub fn careers(n: u32) -> Config {
        let n = n.clamp(1, 500);
        Config {
            spot_runs: (n * 3).clamp(30, 300),
            careers: n,
            ..Config::FULL
        }
    }
}

/// Which bot plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bot {
    /// First route, first affordable breaker, never shields, walks away from everyone.
    Dumb,
    /// Picks routes, breaks with the best hit per cycle, shields before attacks, repairs,
    /// scrubs trace, loads what it carries, walks away from encounters.
    Smart,
}

/// What a run's bot saw, for the report.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunLog {
    /// Strength each program (by warez id) took off ICE.
    pub breaks: Vec<(String, u32)>,
    pub fights: u32,
    /// BBS downloads: (what kind of file, price).
    pub bought: Vec<(&'static str, u32)>,
    /// Contracts taken (jobs boards and fixers) and settled, and what they actually paid.
    pub taken: u32,
    pub paid: u32,
    pub pay: u32,
    /// Clients who turned on the runner: (betrayal label, pay they kept back).
    pub betrayals: Vec<(String, u32)>,
    /// What it chose in each encounter.
    pub talks: Vec<&'static str>,
}

impl RunLog {
    fn add_break(&mut self, id: &str, hit: u32) {
        match self.breaks.iter_mut().find(|(w, _)| w == id) {
            Some((_, n)) => *n += hit,
            None => self.breaks.push((id.to_string(), hit)),
        }
    }
}

/// Plays a run to its end with `bot`. Never leaves a game unfinished: a run that stalls (it
/// shouldn't) is ended by the turn limit.
pub fn play(g: Game, bot: Bot) -> (Game, RunLog) {
    play_knowing(g, bot, &[])
}

/// [`play`], for a runner who already knows the phone ranges in `known` (so it won't buy them
/// again at a BBS).
pub fn play_knowing(mut g: Game, bot: Bot, known: &[String]) -> (Game, RunLog) {
    let mut log = RunLog::default();
    let mut shopped: Vec<(bool, usize)> = Vec::new();
    let mut steps = 0;
    while steps < 5_000 {
        steps += 1;
        match g.phase {
            Phase::Exploring => {
                if bot == Bot::Smart {
                    load_what_fits(&mut g);
                    let here = (g.outer.is_some(), g.at);
                    if g.at_bbs() && !shopped.contains(&here) {
                        shopped.push(here);
                        shop_bbs(&mut g, known, &mut log);
                        load_what_fits(&mut g);
                    }
                }
                let to = match bot {
                    Bot::Dumb => g.reachable().first().copied(),
                    Bot::Smart => best_route(&g),
                };
                let Some(to) = to else { break };
                g.apply(Action::Move(to));
                if g.phase == Phase::Combat {
                    log.fights += 1;
                }
            }
            Phase::Combat => {
                let act = match bot {
                    Bot::Dumb => dumb_move(&g),
                    Bot::Smart => smart_move(&g),
                };
                if let Action::Run(slot) = act
                    && let Ok(p) = g.preview(slot)
                {
                    let id = g.data.warez[g.hdd[slot].warez].id.clone();
                    log.add_break(&id, p.hit());
                }
                g.apply(act);
            }
            Phase::Encounter => {
                let pick = match bot {
                    Bot::Dumb => g.leave_choice(),
                    Bot::Smart => talk(&g, known, &mut log),
                };
                let Some(choice) = pick.or_else(|| g.leave_choice()) else {
                    break;
                };
                let before = g.contracts.len();
                g.apply(Action::Choose { choice, slot: None });
                log.taken += (g.contracts.len() - before) as u32;
                if g.encounter.as_ref().is_some_and(|e| e.done) {
                    g.apply(Action::EndEncounter);
                }
            }
            Phase::JackedOut | Phase::Flatlined => break,
        }
    }
    for k in g
        .contracts
        .iter()
        .filter(|k| matches!(k.status, ContractStatus::Done | ContractStatus::Betrayed))
    {
        log.paid += 1;
        log.pay += k.paid;
        if let Some(label) = g.betrayal_label(k) {
            let short = g.mods.pay(k.reward).saturating_sub(k.paid);
            log.betrayals.push((label.to_string(), short));
        }
    }
    (g, log)
}

fn dumb_move(g: &Game) -> Action {
    let cpu = g.combat.as_ref().map_or(0, |c| c.cpu);
    g.hdd
        .iter()
        .enumerate()
        .find(|(_, s)| {
            s.loaded && s.spec.cpu <= cpu && matches!(s.spec.effect, Effect::Break { .. })
        })
        .map_or(Action::EndTurn, |(i, _)| Action::Run(i))
}

fn smart_move(g: &Game) -> Action {
    let Some(c) = &g.combat else {
        return Action::EndTurn;
    };
    let ok = |want: fn(&Outcome) -> bool| {
        (0..g.hdd.len()).find(|&i| g.preview(i).is_ok_and(|p| want(&p.outcome)))
    };
    let d = &g.deck;
    // Patch up before it's too late; shield an attack coming that would hurt.
    if d.integrity * 3 <= d.integrity_max
        && let Some(i) = ok(|o| matches!(o, Outcome::Repair(_)))
    {
        return Action::Run(i);
    }
    if let Some(Intent::Attack(n)) = g.intent()
        && !c.stunned
        && c.shield < n
        && n * 2 >= d.integrity.min(n * 2)
        && let Some(i) = ok(|o| matches!(o, Outcome::Shield(_)))
    {
        // Only when the hit matters: it would take a third of what's left, or more.
        if n * 3 >= d.integrity {
            return Action::Run(i);
        }
    }
    if d.trace * 10 >= TRACE_MAX * 7
        && let Some(i) = ok(|o| matches!(o, Outcome::Scrub(_)))
    {
        return Action::Run(i);
    }
    // The breaker that takes the most per cycle; a finishing blow beats everything.
    let best = (0..g.hdd.len())
        .filter_map(|i| g.preview(i).ok().map(|p| (i, p)))
        .filter(|(_, p)| p.hit() > 0)
        .max_by_key(|(_, p)| {
            let kills = p.hit() >= c.strength;
            (kills, p.hit() * 100 / p.cpu.max(1), p.hit())
        });
    if let Some((i, _)) = best {
        return Action::Run(i);
    }
    // Nothing breaks: a stun buys a turn against an attack.
    if matches!(g.intent(), Some(Intent::Attack(_)))
        && let Some(i) = ok(|o| matches!(o, Outcome::Stun))
    {
        return Action::Run(i);
    }
    Action::EndTurn
}

/// Loads programs while RAM allows, breakers first.
fn load_what_fits(g: &mut Game) {
    loop {
        let free = g.deck.ram_max.saturating_sub(g.ram_used());
        let pick = g
            .hdd
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.loaded && s.spec.ram <= free)
            .max_by_key(|(_, s)| matches!(s.spec.effect, Effect::Break { .. }))
            .map(|(i, _)| i);
        let Some(i) = pick else { return };
        let before = g.ram_used();
        g.apply(Action::Load(i));
        if g.ram_used() == before {
            return;
        }
    }
}

/// The route a careful runner takes: the exit when it's there, loot when trace allows, and
/// around ICE (black ICE most of all) when there's a choice.
fn best_route(g: &Game) -> Option<usize> {
    let trace = g.deck.trace;
    // Contract targets pull the route toward them (they're on the main network).
    let targets: Vec<usize> = if g.outer.is_none() {
        g.contracts
            .iter()
            .filter(|k| k.status == ContractStatus::Active)
            .map(|k| k.node)
            .collect()
    } else {
        Vec::new()
    };
    g.reachable().iter().copied().max_by_key(|&to| {
        let pull: i64 = if targets.contains(&to) {
            150
        } else if !targets.is_empty() {
            let reach = g.net.reachable_from(to);
            if targets
                .iter()
                .any(|&t| reach.get(t).copied().unwrap_or(false))
            {
                40
            } else {
                0
            }
        } else {
            0
        };
        let n = &g.net.nodes[to];
        let ice_strength = n
            .ice
            .and_then(|i| g.data.ice.get(i))
            .map_or(0, |d| d.strength as i64);
        let score: i64 = if n.cleared {
            20
        } else {
            match n.kind {
                NodeKind::Exit => 1000,
                NodeKind::Vault if trace * 2 < TRACE_MAX => 60,
                NodeKind::Vault => 25,
                NodeKind::Bbs => 30,
                NodeKind::Contact => 15,
                NodeKind::Entry => 10,
                NodeKind::Ice => -10 - ice_strength,
                NodeKind::BlackIce => -200 - ice_strength,
                NodeKind::Subnet => -50,
            }
        };
        (score + pull, -(to as i64))
    })
}

/// What a player does at home before dialing: call the boards you know and read what opens
/// doors (the partner directory, a post about another board), then download any tool or free
/// breaker you're missing.
fn errands(save: &mut Save, data: &Data) {
    use crate::game::Reading;
    // Reading can learn boards, so read until nothing new turns up.
    loop {
        let known = save.boards.len();
        for board in save.known_boards(data) {
            let b = &data.boards[board];
            let mut reads: Vec<Reading> = (0..b.bulletins.len())
                .map(Reading::Bulletin)
                .filter(|r| r.effect(b).is_some())
                .collect();
            for (base, m) in b.messages.iter().enumerate() {
                reads.extend(
                    (0..m.posts.len())
                        .map(|post| Reading::Post { base, post })
                        .filter(|r| r.effect(b).is_some()),
                );
            }
            if reads.is_empty() || save.call_board(data, &data.rig, board).is_err() {
                continue;
            }
            for r in reads {
                let _ = save.home_read(data, board, r);
            }
        }
        if save.boards.len() == known {
            break;
        }
    }
    // Missing a tool, or a free breaker the stash doesn't hold? Call a board you know
    // that always stocks it and download it, like a player would.
    let wanted = |s: &Save, f: &crate::data::BbsFileDef| match &f.item {
        Drop::Tool(t) => !s.has_tool(t),
        Drop::Warez(w) => {
            f.price == 0
                && data
                    .warez_ix(w)
                    .is_some_and(|i| matches!(data.warez[i].effect, Effect::Break { .. }))
                && !s.stash.iter().any(|x| x.origin == *w)
        }
        _ => false,
    };
    for board in save.known_boards(data) {
        let has_one = data.boards[board]
            .areas
            .iter()
            .any(|a| a.always && a.files.iter().any(|f| wanted(save, f)));
        if !has_one {
            continue;
        }
        let Ok(call) = save.call_board(data, &data.rig, board) else {
            continue;
        };
        for (a, files) in call.stock().iter().enumerate() {
            for (f, st) in files.iter().enumerate() {
                let def = &call.board().areas[a].files[st.def];
                if wanted(save, def) && call.buy_blocker(a, f).is_none() {
                    let _ = save.home_buy(data, &data.rig, board, a, f);
                }
            }
        }
    }
}

/// Credits a runner keeps when shopping, for whatever comes next.
const BBS_RESERVE: u32 = 20;

/// What a drop is worth to this runner now, in rough credits (0: not worth having).
fn worth(g: &Game, d: &Drop, known: &[String]) -> i64 {
    let deck = &g.deck;
    match d {
        Drop::Warez(id) => {
            let have = g.hdd.iter().any(|s| g.data.warez[s.warez].id == *id);
            match g.data.warez.iter().find(|w| w.id == *id) {
                _ if have => 0,
                Some(w) => match w.effect {
                    Effect::Break { power, .. } => 30 + i64::from(power) * 5,
                    Effect::Shield(_) | Effect::Scrub(_) => 35,
                    Effect::Repair(_) => 20,
                    Effect::Stun => 15,
                },
                None => 0,
            }
        }
        Drop::Credits(lo, hi) => i64::from(lo + hi) / 2,
        Drop::Ram(n) | Drop::Hdd(n) => 60 * i64::from(*n),
        Drop::Cpu(n) => 120 * i64::from(*n),
        Drop::Repair(n) if deck.integrity + n <= deck.integrity_max => 6 * i64::from(*n),
        Drop::Scrub(n) if deck.trace >= 30 => 3 * i64::from(*n),
        Drop::Fragment(n) => 10 * i64::from(*n),
        // A tool is for good: the wardialer is how a career finds its next tier.
        Drop::Tool(id) if !g.has_tool(id) => 500,
        // A game is never progression: the bots never buy one.
        Drop::Game(_) => 0,
        Drop::Hint(id) if !known.contains(id) && !g.hints.contains(id) => g
            .data
            .exchanges
            .iter()
            .find(|r| r.id == *id)
            .map_or(0, |r| 20 + i64::from(r.tiers.2) + i64::from(r.tiers.1) / 2),
        _ => 0,
    }
}

fn kind(d: &Drop) -> &'static str {
    match d {
        Drop::Warez(_) => "warez",
        Drop::Hint(_) => "range file",
        Drop::Ram(_) | Drop::Cpu(_) | Drop::Hdd(_) => "deck mod",
        Drop::Repair(_) | Drop::Scrub(_) => "repair/scrub",
        Drop::Fragment(_) => "fragment",
        Drop::Tool(_) => "tool",
        Drop::Game(_) => "game",
        Drop::Credits(..) | Drop::Text(_) | Drop::Nothing => "other",
    }
}

/// On a BBS: take the jobs on its board, then buy the best value it can afford while keeping
/// a reserve, until nothing's worth it.
fn shop_bbs(g: &mut Game, known: &[String], log: &mut RunLog) {
    for job in 0..g.bbs_jobs.len() {
        let k = &g.bbs_jobs[job];
        if k.status == ContractStatus::Offered && risk_worth(g, k.reward, g.betray_pct(k)) {
            let before = g.contracts.len();
            g.apply(Action::TakeJob { job });
            log.taken += (g.contracts.len() - before) as u32;
        }
    }
    for _ in 0..16 {
        let mut best: Option<(i64, usize, usize)> = None;
        for (a, area) in g.bbs_stock.iter().enumerate() {
            for (f, s) in area.iter().enumerate() {
                if g.buy_blocker(a, f).is_some() {
                    continue;
                }
                let def = &g.bbs().areas[a].files[s.def];
                if def.price > 0 && g.deck.credits < def.price + BBS_RESERVE {
                    continue;
                }
                let v = worth(g, &def.item, known);
                if v <= i64::from(def.price) / 2 {
                    continue;
                }
                let score = v * 100 / i64::from(def.price.max(1));
                if best.is_none_or(|b| score > b.0) {
                    best = Some((score, a, f));
                }
            }
        }
        let Some((_, a, f)) = best else { return };
        let def = g.bbs().areas[a].files[g.bbs_stock[a][f].def].clone();
        g.apply(Action::Buy { area: a, file: f });
        if g.bbs_stock[a][f].bought {
            log.bought.push((kind(&def.item), def.price));
        } else {
            return;
        }
    }
}

/// Whether a job paying `reward` is worth it when the client turns `pct`% of the time: a
/// betrayal can cost the pay and, above tier 1, the run.
fn risk_worth(g: &Game, reward: u32, pct: u32) -> bool {
    let danger = if g.tier() > 1 { 2 * reward } else { 0 };
    let keep = i64::from(reward) * i64::from(100 - pct) - i64::from(danger) * i64::from(pct);
    keep > 0
}

/// In an encounter: the choice worth most, if any is worth more than walking away. Choices
/// that need a program picked (trades, sales) are left alone.
fn talk(g: &Game, known: &[String], log: &mut RunLog) -> Option<usize> {
    let e = g.encounter.as_ref()?;
    let ev = &g.data.events[e.event];
    let d = &g.deck;
    let active = g
        .contracts
        .iter()
        .filter(|k| k.status == ContractStatus::Active)
        .count();
    let healthy = d.integrity * 3 >= d.integrity_max * 2
        && g.hdd
            .iter()
            .any(|s| s.loaded && matches!(s.spec.effect, Effect::Break { .. }));
    let hurt = |p: &Penalty| match p {
        Penalty::Trace(n) => 4 * i64::from(*n),
        Penalty::Damage(n) => 8 * i64::from(*n),
    };
    let best = ev
        .choices
        .iter()
        .enumerate()
        .filter(|(i, _)| !g.choice_needs_slot(*i) && g.choice_blocker(*i, None).is_none())
        .map(|(i, c)| {
            let v = match &c.effect {
                ChoiceEffect::Leave => (0, "walked away"),
                ChoiceEffect::Contract if active < 2 => {
                    // Whoever's offering, they might turn: what that costs at this tier.
                    let pct = BETRAY_PCT[(g.tier() as usize).clamp(1, 3) - 1];
                    (60 - i64::from(pct) * 2, "took a contract")
                }
                ChoiceEffect::Pay { cost, get } => {
                    (worth(g, get, known) - i64::from(*cost), "paid")
                }
                ChoiceEffect::Gamble { pct, win, lose } => (
                    (i64::from(*pct) * worth(g, win, known) - i64::from(100 - pct) * hurt(lose))
                        / 100,
                    "gambled",
                ),
                ChoiceEffect::Fight { rewards, .. } if healthy => (
                    20 + rewards.iter().map(|r| worth(g, r, known)).sum::<i64>(),
                    "fought",
                ),
                _ => (-1, "walked away"),
            };
            (i, v)
        })
        .filter(|(_, (v, _))| *v > 0)
        .max_by_key(|(i, (v, _))| (*v, -(*i as i64)));
    match best {
        Some((i, (_, what))) => {
            log.talks.push(what);
            Some(i)
        }
        None => {
            log.talks.push("walked away");
            None
        }
    }
}

// ---- careers --------------------------------------------------------------------------------

/// How a run ended, for the report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fate {
    Exit,
    Traced,
    Ice,
    BlackIce,
}

fn fate(g: &Game) -> Fate {
    if g.phase == Phase::JackedOut {
        return Fate::Exit;
    }
    // A run that ends mid-fight is the ICE's doing, whether it took the deck's integrity or
    // kept it there until the trace ran out.
    let fighting =
        g.combat.is_some() || g.net.nodes[g.at].ice.is_some() && !g.net.nodes[g.at].cleared;
    if !fighting && g.deck.trace >= TRACE_MAX {
        return Fate::Traced;
    }
    let black = g
        .combat
        .as_ref()
        .is_some_and(|c| g.data.ice[c.ice].tier >= 2)
        || g.net.nodes[g.at].kind == NodeKind::BlackIce;
    if black { Fate::BlackIce } else { Fate::Ice }
}

/// Totals for one tier.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TierStats {
    pub runs: u32,
    pub exits: u32,
    pub traced: u32,
    pub ice: u32,
    pub black_ice: u32,
    pub banked: u64,
}

impl TierStats {
    fn add(&mut self, f: Fate, banked: u32) {
        self.runs += 1;
        self.banked += u64::from(banked);
        match f {
            Fate::Exit => self.exits += 1,
            Fate::Traced => self.traced += 1,
            Fate::Ice => self.ice += 1,
            Fate::BlackIce => self.black_ice += 1,
        }
    }

    pub fn win_pct(&self) -> u32 {
        pct(self.exits, self.runs)
    }
}

fn pct(n: u32, of: u32) -> u32 {
    (n * 100).checked_div(of).unwrap_or(0)
}

/// Checkpoints, in runs into a career, where the report samples the bank and upgrades.
pub const CHECKPOINTS: [u32; 6] = [10, 20, 40, 60, 80, 100];

/// Everything the probe has seen so far.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Spot checks: [dumb, smart] per tier 1..=3.
    pub spot: [[TierStats; 3]; 2],
    /// Career runs, per tier 1..=3.
    pub career: [TierStats; 3],
    /// Run (1-based) of each career's first exit at tier 2 and tier 3; 0 if never.
    pub first_exit: Vec<[u32; 2]>,
    /// At each checkpoint: (careers that got there, total bank, total upgrade levels).
    pub at: [(u32, u64, u64); CHECKPOINTS.len()],
    /// RIG upgrades bought over all careers: (upgrade, purchases, sum of the run it happened).
    pub buys: Vec<(Upgrade, u32, u64)>,
    /// Strength taken off ICE per program (warez id), over all career runs.
    pub breaks: Vec<(String, u64)>,
    pub fights: u64,
    /// Career runs' BBS downloads by kind: (kind, count, credits spent).
    pub bought: Vec<(String, u32, u64)>,
    /// Career contracts taken, paid, and their pay.
    pub taken: u32,
    pub paid: u32,
    pub pay: u64,
    /// Betrayals by label: how many, and the pay they kept back.
    pub betrayals: Vec<(String, u32, u64)>,
    /// Career encounter choices: (what, count).
    pub talks: Vec<(String, u32)>,
    /// Phone ranges known at each checkpoint (summed over careers).
    pub ranges_at: [u64; CHECKPOINTS.len()],
    /// Run (1-based) each career first knew a range that can answer with tier 3; 0 if never.
    pub t3_range: Vec<u32>,
    /// Wardial scans run, and phonebook numbers they found.
    pub scans: u32,
    pub found: u32,
}

/// The probe, resumable: call [`Probe::step`] until it's done, then read [`Probe::report`].
#[derive(Clone)]
pub struct Probe {
    cfg: Config,
    data: Data,
    stats: Stats,
    /// Spot-check runs played (both bots, every tier).
    spot_done: u32,
    career: u32,
    run: u32,
    save: Option<Save>,
    /// The tier this career is playing, and how its recent runs there went (true = exit).
    tier: u32,
    recent: Vec<bool>,
    first: [u32; 2],
    t3_range: u32,
    /// The starting kit careers begin with (an id in `data/hub/starts.ron`; empty: the default).
    kit: String,
}

impl Probe {
    /// Careers start from starting kit `id` instead of the default.
    pub fn with_kit(mut self, id: &str) -> Probe {
        self.kit = id.to_string();
        self
    }

    pub fn new(data: &Data, cfg: Config) -> Probe {
        Probe {
            cfg,
            data: data.clone(),
            stats: Stats::default(),
            spot_done: 0,
            career: 0,
            run: 0,
            save: None,
            tier: 1,
            recent: Vec::new(),
            first: [0, 0],
            t3_range: 0,
            kit: String::new(),
        }
    }

    pub fn config(&self) -> Config {
        self.cfg
    }

    pub fn stats(&self) -> &Stats {
        &self.stats
    }

    fn spot_total(&self) -> u32 {
        self.cfg.spot_runs * 3 * 2
    }

    /// Runs played and runs in all.
    pub fn progress(&self) -> (u32, u32) {
        let total = self.spot_total() + self.cfg.careers * self.cfg.career_runs;
        let done = self.spot_done + self.career * self.cfg.career_runs + self.run;
        (done.min(total), total)
    }

    pub fn done(&self) -> bool {
        let (d, t) = self.progress();
        d >= t
    }

    /// Plays up to `runs` more runs. Returns true once everything's been played.
    pub fn step(&mut self, runs: u32) -> bool {
        for _ in 0..runs {
            if self.spot_done < self.spot_total() {
                self.spot_run();
            } else if self.career < self.cfg.careers {
                self.career_run();
            } else {
                return true;
            }
        }
        self.done()
    }

    fn spot_run(&mut self) {
        let k = self.spot_done;
        let per = self.cfg.spot_runs;
        let (bot_ix, tier, seed) = (k / (per * 3), (k / per) % 3 + 1, u64::from(k % per));
        let bot = if bot_ix == 0 { Bot::Dumb } else { Bot::Smart };
        let mut save = Save::default();
        save.phonebook.push(Entry {
            id: 0,
            name: "probe".into(),
            business: "test".into(),
            number: "555-0000".into(),
            tier,
            traits: Vec::new(),
            heat: 0,
            runs: 0,
            last: Vec::new(),
        });
        if let Ok(g) = save.dial(&self.data, 0, self.cfg.seed ^ seed.wrapping_mul(0x9E37)) {
            let (g, _) = play(g, bot);
            let f = fate(&g);
            let banked = if f == Fate::Exit { g.deck.credits } else { 0 };
            self.stats.spot[bot_ix as usize][tier as usize - 1].add(f, banked);
        }
        self.spot_done += 1;
    }

    fn career_run(&mut self) {
        let mut rng = Rng::new(
            self.cfg.seed
                ^ u64::from(self.career).wrapping_mul(0xC0FF_EE11)
                ^ u64::from(self.run).wrapping_mul(0x9E37_79B9),
        );
        let data = self.data.clone();
        let kit = self.kit.clone();
        let save = self.save.get_or_insert_with(|| {
            let mut s = Save {
                kit,
                ..Save::default()
            };
            let seed = rng.next_u64();
            s.ensure_key(seed);
            s.ensure_phonebook(&data, seed);
            s.ensure_hardware(&data);
            s
        });
        // Call the boards you know first: they're where numbers, tools and breakers come from.
        errands(save, &data);
        // Bring everything stashed that fits.
        let ids: Vec<u32> = save.stash.iter().map(|s| s.id).collect();
        for id in ids {
            if !save.loadout.stash.contains(&id) {
                let _ = save.toggle_stash(&data, id);
            }
        }
        // The coolest number at the tier it's playing (or the nearest below it). Heat only
        // shows in NiGHTCOMM, so without it the bot just spreads its runs around. With Tiger
        // Team Pro's profiles it takes the best paying number that isn't hot.
        let sees_heat = save.has_dialer();
        let sees_pay = save.has_tool("tigerteam");
        let pick = (1..=self.tier).rev().find_map(|t| {
            save.phonebook
                .iter()
                .filter(|e| e.tier == t)
                .min_by_key(|e| {
                    let heat = if sees_heat { e.heat } else { 0 };
                    let pay = if sees_pay {
                        crate::tigerteam::profile(&data, e).pay_pct
                    } else {
                        0
                    };
                    (
                        sees_pay && heat >= crate::nightcomm::HOT,
                        std::cmp::Reverse(pay),
                        heat,
                        e.runs,
                    )
                })
                .map(|e| (e.id, e.tier))
        });
        let Some((entry, tier)) = pick else {
            self.next_run();
            return;
        };
        let Ok(g) = save.dial(&data, entry, rng.next_u64()) else {
            self.next_run();
            return;
        };
        let known: Vec<String> = save.ranges.iter().map(|r| r.id.clone()).collect();
        let (g, log) = play_knowing(g, Bot::Smart, &known);
        // Take home every program that isn't a starter, while the stash has room.
        let keep: Vec<usize> = (0..g.hdd.len())
            .filter(|&i| !save.free(&data).contains(&g.data.warez[g.hdd[i].warez].id))
            .collect();
        let report = save.record(&g, &keep, &data.rig);
        let banked = report.as_ref().map_or(0, |r| r.banked);
        let f = fate(&g);
        self.stats.career[tier as usize - 1].add(f, banked);
        self.stats.fights += u64::from(log.fights);
        for (k, price) in &log.bought {
            match self.stats.bought.iter_mut().find(|(w, _, _)| w == k) {
                Some((_, n, spent)) => {
                    *n += 1;
                    *spent += u64::from(*price);
                }
                None => self
                    .stats
                    .bought
                    .push((k.to_string(), 1, u64::from(*price))),
            }
        }
        self.stats.taken += log.taken;
        self.stats.paid += log.paid;
        self.stats.pay += u64::from(log.pay);
        for (label, short) in &log.betrayals {
            match self.stats.betrayals.iter_mut().find(|(l, _, _)| l == label) {
                Some((_, n, lost)) => {
                    *n += 1;
                    *lost += u64::from(*short);
                }
                None => self
                    .stats
                    .betrayals
                    .push((label.clone(), 1, u64::from(*short))),
            }
        }
        for t in &log.talks {
            match self.stats.talks.iter_mut().find(|(w, _)| w == t) {
                Some((_, n)) => *n += 1,
                None => self.stats.talks.push((t.to_string(), 1)),
            }
        }
        for (id, n) in log.breaks {
            match self.stats.breaks.iter_mut().find(|(w, _)| *w == id) {
                Some((_, t)) => *t += u64::from(n),
                None => self.stats.breaks.push((id, u64::from(n))),
            }
        }
        let exit = report.is_some_and(|r| r.end == RunEnd::Exit);
        if exit && tier >= 2 && self.first[tier as usize - 2] == 0 {
            self.first[tier as usize - 2] = self.run + 1;
        }
        // Climb after three exits in a row, drop back after two flatlines in a row.
        if tier == self.tier {
            self.recent.push(exit);
        }
        let last = |n: usize, want: bool| {
            self.recent.len() >= n
                && self.recent[self.recent.len() - n..]
                    .iter()
                    .all(|&x| x == want)
        };
        if last(3, true) && self.tier < 3 {
            self.tier += 1;
            self.recent.clear();
        } else if last(2, false) && self.tier > 1 {
            self.tier -= 1;
            self.recent.clear();
        }
        self.shop(rng.next_u64());
        self.next_run();
    }

    /// Spends the bank the way a sensible player might: a reserve for the phone bill, then the
    /// cheapest of CPU, integrity and RAM, then HDD and stash; and a wardial when the next tier
    /// up has no number yet.
    fn shop(&mut self, _seed: u64) {
        let run = self.run + 1;
        let Some(save) = self.save.as_mut() else {
            return;
        };
        let data = &self.data;
        errands(save, data);
        let want_tier = (self.tier + 1).min(3);
        let lacks_next = !save.phonebook.iter().any(|e| e.tier >= want_tier);
        if lacks_next {
            // The range most likely to answer with that tier; the local exchange never has
            // tier 3, so a corp range has to be learned first (loot hints, BBS files).
            let weight = |id: &str| {
                data.exchanges.iter().find(|r| r.id == id).map_or(0, |r| {
                    [r.tiers.0, r.tiers.1, r.tiers.2][want_tier as usize - 1]
                })
            };
            let mut ranges: Vec<String> = save
                .ranges
                .iter()
                .map(|r| r.id.clone())
                .filter(|r| weight(r) > 0)
                .collect();
            ranges.sort_by_key(|r| std::cmp::Reverse(weight(r)));
            for r in ranges {
                let before = save.phonebook.len();
                if save.wardial(data, &r).is_ok() {
                    self.stats.scans += 1;
                    self.stats.found += (save.phonebook.len() - before) as u32;
                    break;
                }
            }
        }
        let reserve = data.exchanges.iter().map(|r| r.cost).max().unwrap_or(0);
        loop {
            let cost = |u: Upgrade| {
                data.rig
                    .iter()
                    .find(|d| d.kind == u)
                    .and_then(|d| d.cost(save.rig.level(u)))
            };
            let core = [Upgrade::Cpu, Upgrade::Integrity, Upgrade::Ram]
                .into_iter()
                .filter_map(|u| cost(u).map(|c| (c, u)))
                .min_by_key(|(c, _)| *c);
            let extra = [Upgrade::Hdd, Upgrade::Stash]
                .into_iter()
                .filter_map(|u| cost(u).map(|c| (c, u)))
                .min_by_key(|(c, _)| *c);
            let Some((c, u)) = core.or(extra) else { break };
            if save.bank < c + reserve {
                break;
            }
            if save.buy(data, u).is_err() {
                break;
            }
            match self.stats.buys.iter_mut().find(|(k, _, _)| *k == u) {
                Some((_, n, sum)) => {
                    *n += 1;
                    *sum += u64::from(run);
                }
                None => self.stats.buys.push((u, 1, u64::from(run))),
            }
        }
    }

    fn next_run(&mut self) {
        self.run += 1;
        if let Some(i) = CHECKPOINTS.iter().position(|&c| c == self.run)
            && let Some(s) = &self.save
        {
            let levels: u32 = [
                Upgrade::Ram,
                Upgrade::Cpu,
                Upgrade::Hdd,
                Upgrade::Integrity,
                Upgrade::Stash,
            ]
            .into_iter()
            .map(|u| s.rig.level(u))
            .sum();
            self.stats.ranges_at[i] += s.ranges.len() as u64;
            let a = &mut self.stats.at[i];
            a.0 += 1;
            a.1 += u64::from(s.bank);
            a.2 += u64::from(levels);
        }
        if self.t3_range == 0
            && let Some(s) = &self.save
            && s.ranges.iter().any(|k| {
                self.data
                    .exchanges
                    .iter()
                    .any(|r| r.id == k.id && r.tiers.2 > 0)
            })
        {
            self.t3_range = self.run;
        }
        if self.run >= self.cfg.career_runs {
            self.stats.first_exit.push(self.first);
            self.stats.t3_range.push(self.t3_range);
            self.t3_range = 0;
            self.career += 1;
            self.run = 0;
            self.save = None;
            self.tier = 1;
            self.recent.clear();
            self.first = [0, 0];
        }
    }

    /// The report, as lines that fit the Shell (under 70 characters).
    pub fn report(&self) -> Vec<String> {
        let s = &self.stats;
        let cfg = self.cfg;
        let mut out = Vec::new();
        out.push(format!(
            "PROBE  seed {:x}  {} careers x {} runs, {} spot runs a tier  kit {}",
            cfg.seed,
            cfg.careers,
            cfg.career_runs,
            cfg.spot_runs,
            self.data.starts.kit(&self.kit).id
        ));
        out.push(String::new());
        out.push("single runs, plain network       dumb bot    smart bot".into());
        for t in 0..3 {
            let (d, m) = (&s.spot[0][t], &s.spot[1][t]);
            out.push(format!(
                "  tier {}                          {:>3}% out     {:>3}% out",
                t + 1,
                d.win_pct(),
                m.win_pct()
            ));
        }
        out.push(String::new());
        out.push("careers (smart bot, climbing)".into());
        out.push("  tier  runs  out   traced  ice  black   avg banked/run".into());
        for (t, c) in s.career.iter().enumerate() {
            let avg = if c.runs == 0 {
                0
            } else {
                c.banked / u64::from(c.runs)
            };
            out.push(format!(
                "  {:>4} {:>5} {:>3}%  {:>5}%  {:>3}% {:>5}%  {:>8}¢",
                t + 1,
                c.runs,
                c.win_pct(),
                pct(c.traced, c.runs),
                pct(c.ice, c.runs),
                pct(c.black_ice, c.runs),
                avg
            ));
        }
        for (k, label) in ["tier 2", "tier 3"].iter().enumerate() {
            let mut runs: Vec<u32> = s
                .first_exit
                .iter()
                .map(|f| f[k])
                .filter(|&r| r > 0)
                .collect();
            runs.sort_unstable();
            let n = s.first_exit.len().max(1) as u32;
            let median = runs.get(runs.len() / 2).copied();
            out.push(match median {
                Some(m) => format!(
                    "  first {label} exit: {}% of careers, median run {m}",
                    pct(runs.len() as u32, n)
                ),
                None => format!("  first {label} exit: never, in {n} careers"),
            });
        }
        out.push(String::new());
        out.push("money".into());
        for (i, c) in CHECKPOINTS.iter().enumerate() {
            let (n, bank, levels) = s.at[i];
            if n == 0 {
                continue;
            }
            out.push(format!(
                "  after {c:>2} runs: bank {:>5}¢, {:>4.1} RIG upgrades",
                bank / u64::from(n),
                levels as f32 / n as f32
            ));
        }
        let mut buys = s.buys.clone();
        buys.sort_by_key(|(_, n, sum)| *sum / u64::from((*n).max(1)));
        let careers = s.first_exit.len().max(1) as u64;
        for (u, n, sum) in buys {
            let def = self.data.rig.iter().find(|d| d.kind == u);
            out.push(format!(
                "  {:<6} {:>4.1} bought a career (of {}), around run {}",
                def.map_or("?".to_string(), |d| d.label.clone()),
                f64::from(n) / careers as f64,
                def.map_or(0, |d| d.max),
                sum / u64::from(n.max(1))
            ));
        }
        out.push(format!(
            "  wardial: {} scans, {} new numbers",
            s.scans, s.found
        ));
        for (i, c) in CHECKPOINTS.iter().enumerate() {
            let n = s.at[i].0;
            if n > 0 {
                out.push(format!(
                    "  ranges known after {c:>2} runs: {:.1}",
                    s.ranges_at[i] as f64 / f64::from(n)
                ));
            }
        }
        let mut t3: Vec<u32> = s.t3_range.iter().copied().filter(|&r| r > 0).collect();
        t3.sort_unstable();
        out.push(match t3.get(t3.len() / 2) {
            Some(m) => format!(
                "  a range with tier 3 numbers: {}% of careers, median run {m}",
                pct(t3.len() as u32, s.t3_range.len().max(1) as u32)
            ),
            None => "  a range with tier 3 numbers: never learned".into(),
        });
        out.push(String::new());
        let runs = u64::from(s.career.iter().map(|t| t.runs).sum::<u32>().max(1));
        out.push("where the money comes from (career runs)".into());
        out.push(format!(
            "  contracts: {} taken, {} settled ({}%), {:.1}¢ a run",
            s.taken,
            s.paid,
            pct(s.paid, s.taken),
            s.pay as f64 / runs as f64
        ));
        let turned: u32 = s.betrayals.iter().map(|b| b.1).sum();
        out.push(format!(
            "  betrayed: {turned} ({}% of settled jobs)",
            pct(turned, s.paid)
        ));
        let mut b = s.betrayals.clone();
        b.sort_by_key(|b| std::cmp::Reverse(b.1));
        for (label, n, lost) in b {
            out.push(format!(
                "    {label:<13} {n:>5} times, {lost:>7}¢ kept back"
            ));
        }
        let spent: u64 = s.bought.iter().map(|b| b.2).sum();
        out.push(format!(
            "  BBS: {:.1}¢ a run spent, on",
            spent as f64 / runs as f64
        ));
        let mut bought = s.bought.clone();
        bought.sort_by_key(|b| std::cmp::Reverse(b.2));
        for (k, n, cr) in bought {
            out.push(format!("    {k:<13} {n:>5} bought, {cr:>7}¢"));
        }
        let talks: u32 = s.talks.iter().map(|t| t.1).sum();
        out.push(format!("  encounters: {talks}"));
        let mut t = s.talks.clone();
        t.sort_by_key(|t| std::cmp::Reverse(t.1));
        for (what, n) in t {
            out.push(format!("    {what:<16} {:>3}%", pct(n, talks)));
        }
        out.push(String::new());
        out.push(format!(
            "fights: {} over all careers; who did the breaking",
            s.fights
        ));
        let total: u64 = s.breaks.iter().map(|b| b.1).sum::<u64>().max(1);
        let mut breaks = s.breaks.clone();
        breaks.sort_by_key(|b| std::cmp::Reverse(b.1));
        for (id, n) in breaks.into_iter().take(8) {
            out.push(format!("  {id:<12} {:>3}%", n * 100 / total));
        }
        out
    }
}
