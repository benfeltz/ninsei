//! The balance probe: bots play the real game (`Save::dial`, `Game::apply`, `Save::record`,
//! `Save::buy`, `Save::wardial`), so it measures what a player meets, not a model of it.
//!
//! Two parts:
//! - **Spot checks**: single runs on a plain network of each tier, by a dumb bot (first route,
//!   [`DUMB`]: two lines of icepick), a careful one ([`CAREFUL`], its own program) and the
//!   careful one fighting with the starter's `SAFE` plus a breaker ([`SAFE_PICK`]), the first
//!   program a player writes.
//!   In a fight a bot only drives the deck's program, written in the game's BASIC: it plays,
//!   and when the program hands over the turn (a STOP, a hang) it ends the turn, having no
//!   hands of its own. If a bot can't say it in the language, a player can't either.
//! - **Careers**: the competent bot from a fresh save, run after run. It climbs tiers when it
//!   keeps getting out, falls back when it doesn't, spends its bank in RIG and wardials for
//!   bigger numbers. The report says how long the climb takes, what the money buys and when,
//!   what kills runs and which programs carry fights. The same careers are played again
//!   fighting with [`SAFE_PICK`], for their exits and deaths only.
//!
//! [`Probe`] is resumable (`step` plays a few runs and returns), so the home Shell's `probe`
//! command can run it a little each frame in the browser, where there are no threads. The CLI
//! is `cargo run -p ninsei-core --example balance`. Same seed, same report.

use crate::bbs::Line;
use crate::data::{ChoiceEffect, Data, Drop, Effect, Penalty};
use crate::fight::Outcome;
use crate::game::{Action, ContractStatus, ExecState, Game, Phase, StepWhat};
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

/// The dumb bot's program: pick at whatever it is until the cycles run out.
pub const DUMB: &str = "10 RUN icepick\n20 GOTO 10";

/// The careful bot's program. It looks after the deck first: patch when integrity runs low,
/// raise the firewall when a hit that matters is next, scrub a high trace. A SPOOF hides NEXT
/// from it and it has no answer: a guard on integrity alone would cost a seventh IF, which the
/// starter's 6 GB can't pay before the breakers, so it would never break anything. Then it uses the
/// breaker made for the ICE in front of it, and the strongest breaker it carries after that.
/// It buys the specialists, so it names them. Each specialist is written out six times (a
/// maxed CPU) rather than looped: a RUN that can't run costs nothing, so a loop on a breaker
/// the deck doesn't carry would hang, while six skipped lines fall through to line 600.
pub const CAREFUL: &str = "\
10 IF INTEG < 7
20 RUN patch
30 IF NEXT ATTACK
40 IF INTEG < 16
50 RUN firewall
60 IF TRACE > 69
70 RUN scrubber
80 IF ICE CODEGATE
90 GOTO 300
100 IF ICE SENTRY
110 GOTO 400
120 GOTO 500
300 RUN decoder
310 RUN decoder
320 RUN decoder
330 RUN decoder
340 RUN decoder
350 RUN decoder
360 GOTO 600
400 RUN garrote
410 RUN garrote
420 RUN garrote
430 RUN garrote
440 RUN garrote
450 RUN garrote
460 GOTO 600
500 RUN crowbar
510 RUN crowbar
520 RUN crowbar
530 RUN crowbar
540 RUN crowbar
550 RUN crowbar
600 RUN torch
610 RUN wyrm
620 RUN icepick
630 GOTO 600";

/// The careful bot's second program, for when the ICE have been patched against the first: it
/// drops the firewall (ICE written against a shielded deck read it) and patches earlier and
/// harder instead.
pub const CAREFUL_B: &str = "\
10 IF INTEG < 10
20 RUN patch
30 IF TRACE > 69
40 RUN scrubber
50 IF ICE CODEGATE
60 GOTO 300
70 IF ICE SENTRY
80 GOTO 400
90 GOTO 500
300 RUN decoder
310 RUN decoder
320 RUN decoder
330 RUN decoder
340 RUN decoder
350 RUN decoder
360 GOTO 600
400 RUN garrote
410 RUN garrote
420 RUN garrote
430 RUN garrote
440 RUN garrote
450 RUN garrote
460 GOTO 600
500 RUN crowbar
510 RUN crowbar
520 RUN crowbar
530 RUN crowbar
540 RUN crowbar
550 RUN crowbar
600 RUN torch
610 RUN wyrm
620 RUN icepick
630 GOTO 600";

/// The careful bot's third program: breakers first, every cycle into the ICE, and a firewall or
/// a patch only once the deck is low, so a read on its defence finds it rarely.
pub const CAREFUL_C: &str = "\
10 IF ICE CODEGATE
20 GOTO 300
30 IF ICE SENTRY
40 GOTO 400
50 GOTO 500
300 RUN decoder
310 RUN decoder
320 RUN decoder
330 RUN decoder
340 RUN decoder
350 GOTO 600
400 RUN garrote
410 RUN garrote
420 RUN garrote
430 RUN garrote
440 RUN garrote
450 GOTO 600
500 RUN crowbar
510 RUN crowbar
520 RUN crowbar
530 RUN crowbar
540 RUN crowbar
600 IF INTEG < 8
610 RUN patch
620 IF INTEG < 12
630 RUN firewall
640 RUN torch
650 RUN wyrm
660 RUN icepick
670 GOTO 640";

/// The career bot's programs, in the order it rotates through them: when the sysops ship a new
/// version of a kind it met, or ICE killed it, it writes its next one.
pub const ROTATION: [&str; 3] = [CAREFUL, CAREFUL_B, CAREFUL_C];

/// Targets the report checks the ladder against (141.9.2). The competent careers must get out
/// of at least this share of their runs at every tier, however the ICE evolve.
pub const CURVE_FLOOR: u32 = 90;

/// The starter kit's `SAFE` (`data/hub/starts.ron`) with `55 RUN icepick` typed in: patch when
/// low, firewall before an attack, and pick at the ICE. The first edit a player makes, and the
/// program ICE that read the deck have to beat.
pub const SAFE_PICK: &str = "\
10 REM SAFE 1.0  (c) Sense/Net
20 IF INTEG < 7
30 RUN patch
40 IF NEXT ATTACK
50 RUN firewall
55 RUN icepick
60 GOTO 20";

/// Which bot plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bot {
    /// First route, [`DUMB`] in fights, walks away from everyone.
    Dumb,
    /// The careful bot: picks routes, shops, talks, and fights with [`CAREFUL`].
    Smart,
    /// The careful bot fighting with [`SAFE_PICK`].
    SafePick,
}

impl Bot {
    /// The three, in the order the report lists them.
    pub const ALL: [Bot; 3] = [Bot::Dumb, Bot::Smart, Bot::SafePick];

    /// The program the bot fights with.
    pub fn program(self) -> &'static str {
        match self {
            Bot::Dumb => DUMB,
            Bot::Smart => CAREFUL,
            Bot::SafePick => SAFE_PICK,
        }
    }

    /// What the report calls it.
    pub fn label(self) -> &'static str {
        match self {
            Bot::Dumb => "DUMB",
            Bot::Smart => "CAREFUL",
            Bot::SafePick => "SAFE_PICK",
        }
    }
}

/// What a run's bot saw, for the report.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunLog {
    /// Strength each program (by warez id) took off ICE.
    pub breaks: Vec<(String, u32)>,
    /// Fights it walked into on the map.
    pub fights: u32,
    /// Every fight it had, on the map, from an encounter or a client's setup, and the turns
    /// they took together.
    pub fought: u32,
    pub turns: u32,
    /// ICE turns that hid NEXT (`SPOOF`) or garbled a line of its program (`SEGFAULT`). A bot
    /// can't retype a line mid-fight, so a garbled line stays garbled until the run ends.
    pub spoofs: u32,
    pub segfaults: u32,
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

/// Tells fights apart as the bot plays, to count them and their turns. A fight is its node and
/// its ICE, and its turn only goes up: one that starts on the node the last one fell on starts
/// back at turn 1. A fight ends in the turn it's on (won in it, or lost at its end), so the last
/// turn seen is the turns it took.
#[derive(Default)]
struct FightClock {
    on: Option<(usize, usize, u32)>,
}

impl FightClock {
    /// Looks at the game after an action.
    fn see(&mut self, g: &Game, log: &mut RunLog) {
        let now = g
            .combat
            .as_ref()
            .filter(|_| g.phase == Phase::Combat)
            .map(|c| (c.node, c.ice, c.turn));
        let same =
            matches!((self.on, now), (Some(a), Some(b)) if (a.0, a.1) == (b.0, b.1) && b.2 >= a.2);
        // A turn of the ICE's went by: count what it did to the program.
        if same
            && self.on.zip(now).is_some_and(|(a, b)| b.2 > a.2)
            && let Some(c) = g.combat.as_ref()
        {
            for s in &c.ice_exec.trace {
                match s.what {
                    StepWhat::Spoofed(_) => log.spoofs += 1,
                    StepWhat::Segfaulted(Some(_)) => log.segfaults += 1,
                    _ => {}
                }
            }
        }
        if !same {
            if let Some(was) = self.on {
                log.turns += was.2;
            }
            if now.is_some() {
                log.fought += 1;
            }
        }
        self.on = now;
    }

    /// The run is over: a fight still open ends here.
    fn close(&mut self, log: &mut RunLog) {
        if let Some(was) = self.on.take() {
            log.turns += was.2;
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
    if g.program.lines.is_empty() {
        g.apply(Action::Program(bot.program().to_string()));
    }
    // The program handed the turn over (a STOP or a hang): a bot has no hands, so it ends it.
    let mut handed = false;
    let mut clock = FightClock::default();
    while steps < 5_000 {
        steps += 1;
        match g.phase {
            Phase::Exploring => {
                if bot != Bot::Dumb {
                    let here = (g.outer.is_some(), g.at);
                    if g.at_bbs() && !shopped.contains(&here) {
                        shopped.push(here);
                        shop_bbs(&mut g, known, &mut log);
                    }
                }
                let to = match bot {
                    Bot::Dumb => g.reachable().first().copied(),
                    Bot::Smart | Bot::SafePick => best_route(&g),
                };
                let Some(to) = to else { break };
                g.apply(Action::Move(to));
                clock.see(&g, &mut log);
                if g.phase == Phase::Combat {
                    log.fights += 1;
                }
            }
            Phase::Combat => {
                if handed {
                    handed = false;
                    g.apply(Action::EndTurn);
                    clock.see(&g, &mut log);
                    continue;
                }
                // Play sets it running and runs a line; the rest of the turn is Finish's.
                let carry_on = if g.exec.state == ExecState::Running {
                    true
                } else {
                    g.apply(Action::Play);
                    clock.see(&g, &mut log);
                    g.pass_open()
                };
                if carry_on {
                    g.apply(Action::Finish);
                    clock.see(&g, &mut log);
                }
                for s in &g.exec.trace {
                    if let StepWhat::Ran {
                        uid,
                        outcome: Outcome::Break { hit, .. },
                    } = s.what
                        && let Some(p) = g.hdd.iter().find(|p| p.uid == uid)
                    {
                        let id = g.data.warez[p.warez].id.clone();
                        log.add_break(&id, hit);
                    }
                }
                handed = g.phase == Phase::Combat && g.exec.state != ExecState::Running;
            }
            Phase::Encounter => {
                let pick = match bot {
                    Bot::Dumb => g.leave_choice(),
                    Bot::Smart | Bot::SafePick => talk(&g, known, &mut log),
                };
                let Some(choice) = pick.or_else(|| g.leave_choice()) else {
                    break;
                };
                let before = g.contracts.len();
                g.apply(Action::Choose { choice, slot: None });
                clock.see(&g, &mut log);
                log.taken += (g.contracts.len() - before) as u32;
                if g.encounter.as_ref().is_some_and(|e| e.done) {
                    g.apply(Action::EndEncounter);
                }
            }
            Phase::JackedOut | Phase::Flatlined => break,
        }
    }
    // A run cut off by the step limit mid-fight still counts that fight's turns.
    clock.close(&mut log);
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

/// Opens what's on the desktop, as a player does: each game and tool's intro plays, and the
/// first time a crack's does it teaches the board it advertises (`Save::saw_intro`). The
/// shop's disk on a new deck leads to the Gutter this way.
fn open_desktop(save: &mut Save, data: &Data) {
    if !save.intros {
        return;
    }
    let games = save
        .games
        .iter()
        .filter_map(|g| Some(data.game(&g.id)?.intro(g.copy).to_string()));
    let tools = save
        .tools
        .iter()
        .filter_map(|t| data.tool(t).and_then(|t| t.intro.clone()));
    let intros: Vec<String> = games.chain(tools).collect();
    for i in intros {
        save.saw_intro(data, &i);
    }
}

/// What a player does at home before dialing: open what's on the desktop, call the boards you
/// know and read what opens doors (the partner directory, a post about another board), then
/// download any tool or free breaker you're missing.
fn errands(save: &mut Save, data: &Data) {
    use crate::game::Reading;
    open_desktop(save, data);
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
                    // Nothing the careful bot fights with reads it yet.
                    Effect::Sniff => 10,
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
        // Nor is a tune.
        Drop::Tune(_) => 0,
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
        Drop::Tune(_) => "tune",
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
            .any(|s| matches!(s.spec.effect, Effect::Break { .. }));
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
    /// Fights in these runs, and the turns they took.
    pub fights: u32,
    pub turns: u32,
    /// ICE turns that hid NEXT or garbled a line of the bot's program.
    pub spoofs: u32,
    pub segfaults: u32,
}

impl TierStats {
    fn add(&mut self, f: Fate, banked: u32, log: &RunLog) {
        self.runs += 1;
        self.banked += u64::from(banked);
        self.fights += log.fought;
        self.turns += log.turns;
        self.spoofs += log.spoofs;
        self.segfaults += log.segfaults;
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

    /// Turns a fight took, on average.
    pub fn turns_per_fight(&self) -> f32 {
        if self.fights == 0 {
            0.0
        } else {
            self.turns as f32 / self.fights as f32
        }
    }

    /// The row the report prints for it: out, traced, ICE, black ICE, turns a fight.
    fn row(&self) -> String {
        format!(
            "{:>3}%  {:>5}%  {:>3}% {:>5}%  {:>5.1}",
            self.win_pct(),
            pct(self.traced, self.runs),
            pct(self.ice, self.runs),
            pct(self.black_ice, self.runs),
            self.turns_per_fight()
        )
    }
}

fn pct(n: u32, of: u32) -> u32 {
    (n * 100).checked_div(of).unwrap_or(0)
}

/// A deck line that's run out of room: the chassis stops the next RIG level.
fn capped(save: &Save, data: &Data, u: Upgrade) -> bool {
    save.upgrade_blocker(data, u).is_some()
}

/// When a core line (CPU, integrity, RAM) is full on this chassis: the cheapest deck that takes
/// that line further, read from each deck's stats. Owned decks count as free.
fn bigger_chassis(save: &Save, data: &Data, only: &str) -> Option<String> {
    let now = |u: Upgrade| save.stat_cap(data, u);
    let full: Vec<Upgrade> = [Upgrade::Cpu, Upgrade::Integrity, Upgrade::Ram]
        .into_iter()
        .filter(|&u| capped(save, data, u))
        .collect();
    data.hardware
        .decks
        .iter()
        // A probe asked to climb to one deck looks at no other.
        .filter(|d| only.is_empty() || d.id == only)
        .filter(|d| {
            full.iter()
                .any(|&u| d.stat(u).is_some_and(|s| now(u).is_some_and(|c| s.max > c)))
        })
        // Never a step down on a line this deck already takes further.
        .filter(|d| {
            Upgrade::ALL.into_iter().all(|u| match (d.stat(u), now(u)) {
                (Some(s), Some(c)) => s.max >= c,
                _ => true,
            })
        })
        .min_by_key(|d| {
            let price = if save.decks.contains(&d.id) {
                0
            } else {
                d.price
            };
            (price, d.id.clone())
        })
        .map(|d| d.id.clone())
}

/// The top of the climb: every deck line at the most any deck takes it, and every line no
/// deck caps (the stash) bought out in RIG.
fn maxed_out(save: &Save, data: &Data) -> bool {
    Upgrade::ALL.into_iter().all(|u| {
        let top = data
            .hardware
            .decks
            .iter()
            .filter_map(|d| d.stat(u))
            .map(|s| s.max)
            .max();
        match top {
            Some(t) => save.stat_value(data, u) >= t,
            None => data
                .rig
                .iter()
                .find(|r| r.kind == u)
                .is_none_or(|r| save.rig.level(u) >= r.max),
        }
    })
}

/// Checkpoints, in runs into a career, where the report samples the bank and upgrades.
pub const CHECKPOINTS: [u32; 6] = [10, 20, 40, 60, 80, 100];

/// Everything the probe has seen so far.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Spot checks: per bot in [`Bot::ALL`]'s order, per tier 1..=3.
    pub spot: [[TierStats; 3]; 3],
    /// Career runs, per tier 1..=3.
    pub career: [TierStats; 3],
    /// The same careers fighting with [`SAFE_PICK`], per tier 1..=3.
    pub career_pick: [TierStats; 3],
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
    /// The deck's CPU ceiling at each checkpoint (summed over careers): how far up the line
    /// of models the careers have climbed.
    pub chassis_at: [u64; CHECKPOINTS.len()],
    /// Decks bought over all careers: (the deck's title, purchases, sum of the run).
    pub decks: Vec<(String, u32, u64)>,
    /// Career tier-3 runs by the deck it dialed on (its id, its CPU ceiling).
    pub t3_by_chassis: Vec<((String, u32), TierStats)>,
    /// Run (1-based) each career first had every deck line at the most any deck takes; 0 if
    /// never.
    pub maxed: Vec<u32>,
    /// Versions the sysops shipped, per kind (its id) and bump (patch, minor, major), in the
    /// careful careers and in the SAFE_PICK ones.
    pub shipped: Vec<(String, [u32; 3])>,
    pub pick_shipped: Vec<(String, [u32; 3])>,
    /// Careful career runs by how many kinds were past 1.0.0 when they dialed (none, 1-3,
    /// 4-6, 7 or more): (runs, exits).
    pub by_patched: [(u32, u32); 4],
    /// SAFE_PICK's single runs on a plain network of each tier, as the spot checks play them,
    /// but against the versions its careers shipped (each career's last, in turn): the same
    /// program and deck as the spot row, so the difference is the versions.
    pub versus: [TierStats; 3],
}

/// Adds one shipped version of kind `id`, made by `bump`, to a tally.
fn ship(tally: &mut Vec<(String, [u32; 3])>, id: &str, bump: crate::ice_version::Bump) {
    let i = bump as usize;
    match tally.iter_mut().find(|(k, _)| k == id) {
        Some((_, n)) => n[i] += 1,
        None => {
            let mut n = [0; 3];
            n[i] = 1;
            tally.push((id.to_string(), n));
        }
    }
}

/// A deck's name cut to `n` characters, so a report line fits the Shell whatever the data
/// calls it.
fn short(name: &str, n: usize) -> String {
    if name.chars().count() <= n {
        name.to_string()
    } else {
        let mut s: String = name.chars().take(n - 1).collect();
        s.push('…');
        s
    }
}

/// The probe, resumable: call [`Probe::step`] until it's done, then read [`Probe::report`].
#[derive(Clone)]
pub struct Probe {
    cfg: Config,
    data: std::sync::Arc<Data>,
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
    maxed: u32,
    /// The starting kit careers begin with (an id in `data/hub/starts.ron`; empty: the default).
    kit: String,
    /// The deck careers climb to (an id in `hub/hardware.ron`; empty: the cheapest that takes a
    /// full line further).
    deck: String,
    /// Which of [`ROTATION`] the careful career is fighting with.
    rotation: usize,
    /// The versions each SAFE_PICK career had shipped by its end, and the runs played against
    /// them so far (`Stats::versus`).
    finals: Vec<Vec<crate::hub::IceVer>>,
    versus_done: u32,
}

impl Probe {
    /// Careers start from starting kit `id` instead of the default.
    pub fn with_kit(mut self, id: &str) -> Probe {
        self.kit = id.to_string();
        self
    }

    /// Careers climb to deck `id` and no other, so each deck's shape can be compared. An id
    /// that isn't a deck you can buy (unknown, or the starter) means careers never climb, and
    /// the report says so.
    pub fn with_deck(mut self, id: &str) -> Probe {
        self.deck = id.to_string();
        self
    }

    /// The deck `with_deck` named, if careers can climb to it.
    fn climbs_to(&self) -> Option<&crate::hardware::DeckDef> {
        self.data.hardware.deck(&self.deck).filter(|d| !d.starter)
    }

    pub fn new(data: &Data, cfg: Config) -> Probe {
        Probe {
            cfg,
            data: std::sync::Arc::new(data.clone()),
            stats: Stats::default(),
            spot_done: 0,
            career: 0,
            run: 0,
            save: None,
            tier: 1,
            recent: Vec::new(),
            first: [0, 0],
            t3_range: 0,
            maxed: 0,
            kit: String::new(),
            deck: String::new(),
            rotation: 0,
            finals: Vec::new(),
            versus_done: 0,
        }
    }

    pub fn config(&self) -> Config {
        self.cfg
    }

    pub fn stats(&self) -> &Stats {
        &self.stats
    }

    fn spot_total(&self) -> u32 {
        self.cfg.spot_runs * 3 * Bot::ALL.len() as u32
    }

    /// Careers in all: each played with [`CAREFUL`], then again with [`SAFE_PICK`].
    fn careers_total(&self) -> u32 {
        self.cfg.careers * 2
    }

    /// SAFE_PICK's runs against the careers' versions: a spot check's worth at each tier.
    fn versus_total(&self) -> u32 {
        self.cfg.spot_runs * 3
    }

    /// Runs played and runs in all.
    pub fn progress(&self) -> (u32, u32) {
        let total =
            self.spot_total() + self.careers_total() * self.cfg.career_runs + self.versus_total();
        let done =
            self.spot_done + self.career * self.cfg.career_runs + self.run + self.versus_done;
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
            } else if self.career < self.careers_total() {
                self.career_run();
            } else if self.versus_done < self.versus_total() {
                self.versus_run();
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
        let bot = Bot::ALL[bot_ix as usize];
        if let Some((f, banked, log)) = self.single(bot, tier, seed, Vec::new()) {
            self.stats.spot[bot_ix as usize][tier as usize - 1].add(f, banked, &log);
        }
        self.spot_done += 1;
    }

    /// A SAFE_PICK spot run (same seeds as the spot checks') against the versions a SAFE_PICK
    /// career shipped.
    fn versus_run(&mut self) {
        let k = self.versus_done;
        let per = self.cfg.spot_runs;
        let (tier, seed) = (k / per + 1, u64::from(k % per));
        let versions = match self.finals.len() {
            0 => Vec::new(),
            n => self.finals[k as usize % n].clone(),
        };
        if let Some((f, banked, log)) = self.single(Bot::SafePick, tier, seed, versions) {
            self.stats.versus[tier as usize - 1].add(f, banked, &log);
        }
        self.versus_done += 1;
    }

    /// One run by `bot` on a plain network of `tier` from a fresh save running `versions`.
    fn single(
        &self,
        bot: Bot,
        tier: u32,
        seed: u64,
        versions: Vec<crate::hub::IceVer>,
    ) -> Option<(Fate, u32, RunLog)> {
        let mut save = Save {
            program: Some(bot.program().to_string()),
            ice_versions: versions,
            ..Save::default()
        };
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
        let g = save
            .dial(&self.data, 0, self.cfg.seed ^ seed.wrapping_mul(0x9E37))
            .ok()?;
        let (g, log) = play(g, bot);
        let f = fate(&g);
        let banked = if f == Fate::Exit { g.deck.credits } else { 0 };
        Some((f, banked, log))
    }

    /// Whether this career fights with [`SAFE_PICK`]: the second round of careers, on the same
    /// seeds as the first.
    fn picking(&self) -> bool {
        self.career >= self.cfg.careers
    }

    fn career_run(&mut self) {
        let picking = self.picking();
        let bot = if picking { Bot::SafePick } else { Bot::Smart };
        // The careful bot fights with the program it rotated to; SAFE_PICK never changes.
        let program = if picking {
            SAFE_PICK
        } else {
            ROTATION[self.rotation % ROTATION.len()]
        };
        let mut rng = Rng::new(
            self.cfg.seed
                ^ u64::from(self.career % self.cfg.careers.max(1)).wrapping_mul(0xC0FF_EE11)
                ^ u64::from(self.run).wrapping_mul(0x9E37_79B9),
        );
        let data = std::sync::Arc::clone(&self.data);
        let kit = self.kit.clone();
        let save = self.save.get_or_insert_with(|| {
            let mut s = Save {
                kit,
                program: Some(program.to_string()),
                ..Save::default()
            };
            let seed = rng.next_u64();
            s.ensure_key(seed);
            s.ensure_phonebook(&data, seed);
            s.ensure_hardware(&data);
            s
        });
        // A garbled line came home from the last run: the bot retypes its program, as a player
        // would at home.
        save.program = Some(program.to_string());
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
        let patched = save.ice_versions.len();
        let before = save.ice_versions.clone();
        let (g, log) = play_knowing(g, bot, &known);
        // Take home every program that isn't a starter, while the stash has room.
        let keep: Vec<usize> = (0..g.hdd.len())
            .filter(|&i| !save.free(&data).contains(&g.data.warez[g.hdd[i].warez].id))
            .collect();
        let report = save.record(&g, &keep, &data.rig);
        // A patience's worth of the sysops' time at home.
        save.study(&data, crate::tuning::STUDY_PATIENCE);
        let banked = report.as_ref().map_or(0, |r| r.banked);
        let f = fate(&g);
        // What the sysops shipped at this run's end.
        let new: Vec<(String, crate::ice_version::Bump)> = save
            .ice_versions
            .iter()
            .filter(|v| !before.contains(v))
            .filter_map(|v| v.bump.map(|b| (v.id.clone(), b)))
            .collect();
        let tally = if picking {
            &mut self.stats.pick_shipped
        } else {
            &mut self.stats.shipped
        };
        for (id, b) in &new {
            ship(tally, id, *b);
        }
        if !picking {
            let bucket = match patched {
                0 => 0,
                1..=3 => 1,
                4..=6 => 2,
                _ => 3,
            };
            self.stats.by_patched[bucket].0 += 1;
            self.stats.by_patched[bucket].1 += u32::from(f == Fate::Exit);
            // A kind it met was patched, or ICE killed it: the careful bot writes its next
            // program, as a player would.
            if !new.is_empty() || matches!(f, Fate::Ice | Fate::BlackIce) {
                self.rotation += 1;
            }
        }
        if picking {
            // The second round only says how its runs ended.
            self.stats.career_pick[tier as usize - 1].add(f, banked, &log);
        } else {
            self.stats.career[tier as usize - 1].add(f, banked, &log);
        }
        if !picking {
            self.tally(&log, tier, f, banked);
        }
        let exit = report.is_some_and(|r| r.end == RunEnd::Exit);
        if !picking && exit && tier >= 2 && self.first[tier as usize - 2] == 0 {
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

    /// Adds a career run of the first round to everything else the report says.
    fn tally(&mut self, log: &RunLog, tier: u32, f: Fate, banked: u32) {
        let Some(save) = self.save.as_ref() else {
            return;
        };
        let data = &self.data;
        if tier == 3 {
            // A run never changes the deck, so this is the deck it dialed on.
            let chassis = (
                save.deck.clone(),
                save.stat_cap(data, Upgrade::Cpu).unwrap_or(0),
            );
            match self
                .stats
                .t3_by_chassis
                .iter_mut()
                .find(|(c, _)| *c == chassis)
            {
                Some((_, t)) => t.add(f, banked, log),
                None => {
                    let mut t = TierStats::default();
                    t.add(f, banked, log);
                    self.stats.t3_by_chassis.push((chassis, t));
                }
            }
        }
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
        for (id, n) in &log.breaks {
            match self.stats.breaks.iter_mut().find(|(w, _)| w == id) {
                Some((_, t)) => *t += u64::from(*n),
                None => self.stats.breaks.push((id.clone(), u64::from(*n))),
            }
        }
    }

    /// Spends the bank the way a sensible player might: a reserve for the phone bill, then the
    /// cheapest of CPU, integrity and RAM, then HDD and stash; and a wardial when the next tier
    /// up has no number yet. The second round's careers shop the same way, but what they buy
    /// isn't the report's: it only says how their runs ended.
    fn shop(&mut self, _seed: u64) {
        let record = !self.picking();
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
                    if record {
                        self.stats.scans += 1;
                        self.stats.found += (save.phonebook.len() - before) as u32;
                    }
                    break;
                }
            }
        }
        let reserve = data.exchanges.iter().map(|r| r.cost).max().unwrap_or(0);
        loop {
            // A line the chassis can't take any further isn't for sale to this deck.
            let cost = |u: Upgrade| {
                data.rig
                    .iter()
                    .find(|d| d.kind == u)
                    .and_then(|d| d.cost(save.rig.level(u)))
                    .filter(|_| save.upgrade_blocker(data, u).is_none())
            };
            let core = [Upgrade::Cpu, Upgrade::Integrity, Upgrade::Ram]
                .into_iter()
                .filter_map(|u| cost(u).map(|c| (c, u)))
                .min_by_key(|(c, _)| *c);
            let extra = [Upgrade::Hdd, Upgrade::Stash]
                .into_iter()
                .filter_map(|u| cost(u).map(|c| (c, u)))
                .min_by_key(|(c, _)| *c);
            // A core line full on this chassis: the next one up comes before the extras.
            if core.is_none()
                && let Some(deck) = bigger_chassis(save, data, &self.deck)
            {
                let price = data.hardware.deck(&deck).map_or(0, |d| d.price);
                let owned = save.decks.contains(&deck);
                if !owned && save.bank < price + reserve {
                    break;
                }
                if !owned
                    && save
                        .buy_hardware(data, &crate::hardware::Hardware::Deck(deck.clone()))
                        .is_err()
                {
                    break;
                }
                if save.use_deck(data, &deck).is_err() {
                    break;
                }
                if record && !owned {
                    let title = save.deck_def(data).title();
                    match self.stats.decks.iter_mut().find(|(t, _, _)| *t == title) {
                        Some((_, n, sum)) => {
                            *n += 1;
                            *sum += u64::from(run);
                        }
                        None => self.stats.decks.push((title, 1, u64::from(run))),
                    }
                }
                continue;
            }
            let Some((c, u)) = core.or(extra) else { break };
            if save.bank < c + reserve {
                break;
            }
            if save.buy(data, u).is_err() {
                break;
            }
            if !record {
                continue;
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
        // The second round of careers (SAFE_PICK) only reports its exits and deaths.
        let first = !self.picking();
        if first
            && let Some(i) = CHECKPOINTS.iter().position(|&c| c == self.run)
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
            self.stats.chassis_at[i] +=
                u64::from(s.stat_cap(&self.data, Upgrade::Cpu).unwrap_or(0));
            let a = &mut self.stats.at[i];
            a.0 += 1;
            a.1 += u64::from(s.bank);
            a.2 += u64::from(levels);
        }
        if first
            && self.t3_range == 0
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
        if first
            && self.maxed == 0
            && let Some(s) = &self.save
            && maxed_out(s, &self.data)
        {
            self.maxed = self.run;
        }
        if self.run >= self.cfg.career_runs {
            if first {
                self.stats.first_exit.push(self.first);
                self.stats.t3_range.push(self.t3_range);
                self.stats.maxed.push(self.maxed);
            } else if let Some(s) = &self.save {
                self.finals.push(s.ice_versions.clone());
            }
            self.t3_range = 0;
            self.maxed = 0;
            self.career += 1;
            self.run = 0;
            self.save = None;
            self.rotation = 0;
            self.tier = 1;
            self.recent.clear();
            self.first = [0, 0];
        }
    }

    /// What the sysops' ladder did: versions shipped per kind, careful careers' exits as more
    /// kinds are patched, how tier-1 ICE did against the lazy program before and after, and a
    /// met/unmet line per target. Spot runs dial from a fresh save, so they stay on 1.0.0.
    fn versions_report(&self) -> Vec<String> {
        let s = &self.stats;
        let careers = u64::from(self.cfg.careers.max(1));
        let per = |n: u32| n as f32 / careers as f32;
        let mut out = vec![
            String::new(),
            "ICE versions (spot runs stay on 1.0.0)".into(),
            "  shipped a career   careful: patch minor major  SAFE_PICK: same".into(),
        ];
        let tally = |t: &[(String, [u32; 3])], id: &str| {
            t.iter().find(|(k, _)| k == id).map_or([0; 3], |(_, n)| *n)
        };
        for row in &self.data.ice {
            let (a, b) = (tally(&s.shipped, &row.id), tally(&s.pick_shipped, &row.id));
            if a == [0; 3] && b == [0; 3] {
                continue;
            }
            out.push(format!(
                "  {:<12} {:>5.1} {:>5.1} {:>5.1}    {:>5.1} {:>5.1} {:>5.1}",
                short(&row.name, 12),
                per(a[0]),
                per(a[1]),
                per(a[2]),
                per(b[0]),
                per(b[1]),
                per(b[2])
            ));
        }
        out.push("  careful runs by kinds patched when dialed   runs  out".into());
        for (label, (runs, exits)) in ["none", "1-3", "4-6", "7+"].iter().zip(s.by_patched) {
            out.push(format!("  {label:<44}{runs:>5} {:>3}%", pct(exits, runs)));
        }
        out.push("  single runs of SAFE_PICK against the versions its".into());
        out.push("  careers shipped:      out 1.0.0 / versions   ice+black".into());
        let pick = Bot::ALL
            .iter()
            .position(|b| *b == Bot::SafePick)
            .expect("SAFE_PICK is a bot");
        let died = |t: &TierStats| pct(t.ice + t.black_ice, t.runs);
        for (t, (a, b)) in s.spot[pick].iter().zip(&s.versus).enumerate() {
            out.push(format!(
                "    tier {}  {:>21}% / {:>3}%  {:>5}% / {:>3}%",
                t + 1,
                a.win_pct(),
                b.win_pct(),
                died(a),
                died(b)
            ));
        }
        let mark = |ok: bool| if ok { "met" } else { "unmet" };
        let (one, vs) = (&s.spot[pick][0], &s.versus[0]);
        out.push(format!(
            "  target: tier-1 versions beat the lazy program more .. {}",
            mark(vs.runs > 0 && died(vs) > died(one))
        ));
        let curve = s
            .career
            .iter()
            .all(|t| t.runs == 0 || t.win_pct() >= CURVE_FLOOR);
        out.push(format!(
            "  target: careful careers out {CURVE_FLOOR}%+ at every tier ...... {}",
            mark(curve)
        ));
        let tier1: Vec<&str> = self
            .data
            .ice
            .iter()
            .filter(|i| i.tier == 1)
            .map(|i| i.id.as_str())
            .collect();
        let shipped = tier1
            .iter()
            .filter(|id| tally(&s.shipped, id) != [0; 3] || tally(&s.pick_shipped, id) != [0; 3])
            .count();
        out.push(format!(
            "  target: every tier-1 kind ships a version ({shipped} of {}) .. {}",
            tier1.len(),
            mark(shipped == tier1.len())
        ));
        out
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
        if let Some(d) = self.climbs_to() {
            out.push(format!("  careers climb to {}", short(&d.title(), 48)));
        }
        out.push(String::new());
        out.push("single runs, plain network".into());
        out.push("  program    tier  out   traced  ice  black  turns/fight".into());
        for (b, bot) in Bot::ALL.iter().enumerate() {
            for (t, ts) in s.spot[b].iter().enumerate() {
                out.push(format!("  {:<10} {:>4} {}", bot.label(), t + 1, ts.row()));
            }
        }
        // What ICE did to each program rather than the deck. No bot retypes a garbled line
        // mid-fight; the career bot retypes its program at home.
        out.push("  ICE turns that hit the program:  SPOOF  SEGFAULT".into());
        for (b, bot) in Bot::ALL.iter().enumerate() {
            let (sp, sg) = s.spot[b]
                .iter()
                .fold((0, 0), |(a, b), t| (a + t.spoofs, b + t.segfaults));
            out.push(format!(
                "  {:<10}                       {sp:>5}  {sg:>8}",
                bot.label()
            ));
        }
        out.push(String::new());
        out.push("careers (smart bot, climbing)".into());
        out.push("  tier  runs  out   traced  ice  black  turns/fight  banked/run".into());
        for (t, c) in s.career.iter().enumerate() {
            let avg = if c.runs == 0 {
                0
            } else {
                c.banked / u64::from(c.runs)
            };
            out.push(format!(
                "  {:>4} {:>5} {}  {:>12}¢",
                t + 1,
                c.runs,
                c.row(),
                avg
            ));
        }
        let mut by = s.t3_by_chassis.clone();
        let pick_rows: Vec<String> = s
            .career_pick
            .iter()
            .enumerate()
            .map(|(t, c)| format!("  {:>4} {:>5} {}", t + 1, c.runs, c.row()))
            .collect();
        by.sort_by(|a, b| (a.0.1, &a.0.0).cmp(&(b.0.1, &b.0.0)));
        for ((deck, cap), t) in by {
            let deck = short(
                &self.data.hardware.deck(&deck).map_or(deck, |d| d.title()),
                28,
            );
            out.push(format!(
                "  tier 3 on {deck} (cpu {cap}): {:>5} runs, {:>3}% out",
                t.runs,
                t.win_pct()
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
        out.push("the same careers fighting with SAFE_PICK".into());
        out.push("  tier  runs  out   traced  ice  black  turns/fight".into());
        out.extend(pick_rows);
        out.extend(self.versions_report());
        out.push(String::new());
        out.push("money".into());
        for (i, c) in CHECKPOINTS.iter().enumerate() {
            let (n, bank, levels) = s.at[i];
            if n == 0 {
                continue;
            }
            out.push(format!(
                "  after {c:>2} runs: bank {:>5}¢, {:>4.1} RIG upgrades, cpu cap {:.1}",
                bank / u64::from(n),
                levels as f32 / n as f32,
                s.chassis_at[i] as f32 / n as f32
            ));
        }
        let mut buys = s.buys.clone();
        buys.sort_by_key(|(_, n, sum)| *sum / u64::from((*n).max(1)));
        let careers = s.first_exit.len().max(1) as u64;
        // "Of" counts the levels the deck the careers climbed to takes, not every level RIG
        // sells: a deck that ships with more of a line takes fewer levels of it.
        let top = self
            .climbs_to()
            .or_else(|| {
                let (title, _, _) = s.decks.iter().max_by_key(|d| d.1)?;
                self.data
                    .hardware
                    .decks
                    .iter()
                    .find(|d| &d.title() == title)
            })
            .unwrap_or(self.data.hardware.starter_deck());
        for (u, n, sum) in buys {
            let def = self.data.rig.iter().find(|d| d.kind == u);
            let room = def.map_or(0, |d| match top.stat(u) {
                Some(st) => (st.max.saturating_sub(st.base) / d.step).min(d.max),
                None => d.max,
            });
            out.push(format!(
                "  {:<6} {:>4.1} bought a career (of {room}), around run {}",
                def.map_or("?".to_string(), |d| d.label.clone()),
                f64::from(n) / careers as f64,
                sum / u64::from(n.max(1))
            ));
        }
        let mut decks = s.decks.clone();
        decks.sort_by_key(|d| d.2 / u64::from(d.1.max(1)));
        for (title, n, sum) in decks {
            out.push(format!(
                "  deck   {:>4.1} a career: {}, around run {}",
                f64::from(n) / careers as f64,
                short(&title, 28),
                sum / u64::from(n.max(1))
            ));
        }
        out.push(match self.climbs_to() {
            _ if self.deck.is_empty() => {
                "  (the bot climbs to the cheapest deck that takes it further)".into()
            }
            Some(d) => format!("  (the bot climbs only to {})", short(&d.title(), 40)),
            None => format!(
                "  (the bot never climbs: no deck `{}` to buy)",
                short(&self.deck, 24)
            ),
        });
        let mut maxed: Vec<u32> = s.maxed.iter().copied().filter(|&r| r > 0).collect();
        maxed.sort_unstable();
        out.push(match maxed.get(maxed.len() / 2) {
            Some(m) => format!(
                "  rig maxed (top deck, every line full): {}%, median run {m}",
                pct(maxed.len() as u32, s.maxed.len().max(1) as u32)
            ),
            None => "  rig maxed (top deck, every line full): never".into(),
        });
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

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh(data: &Data, intros: bool) -> Save {
        let mut s = Save {
            intros,
            ..Save::default()
        };
        s.ensure_key(0xB07);
        s.ensure_phonebook(data, 1);
        s
    }

    #[test]
    fn the_bot_opens_the_shops_disk_and_its_cracktro_leads_to_the_gutter() {
        let data = Data::builtin().unwrap();
        let mut s = fresh(&data, true);
        errands(&mut s, &data);
        let k = s
            .boards
            .iter()
            .find(|k| k.id == "gutter")
            .expect("knows it");
        assert_eq!(k.source, "a cracktro");
        // With intros off it reads its way there instead, through Sense/Net's post.
        let mut s = fresh(&data, false);
        errands(&mut s, &data);
        let k = s
            .boards
            .iter()
            .find(|k| k.id == "gutter")
            .expect("knows it");
        assert!(k.source.contains("message"), "{}", k.source);
    }
}
