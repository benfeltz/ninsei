//! The hub: your machine between runs. The save document, the screen flow between the
//! Workbench desktop and a run, and the desktop Shell's commands. No UI here.

use serde::{Deserialize, Serialize};

use crate::binary::{self, Bin};
use crate::data::Data;
use crate::game::{Game, Kit, Phase};
use crate::tuning::{
    DEADMAN_PCT, FRAGMENTS_PER_OPT, HUB_DRAG_SECS, HUB_PALETTE_CHARS, MAX_OPT, START_HDD,
    START_WAREZ, STASH_BASE_SLOTS,
};

/// Bump when the save's shape changes incompatibly.
pub const SAVE_VERSION: u32 = 1;
/// Storage key the app saves under.
pub const SAVE_KEY: &str = "ninsei.save";

/// What double-clicking a desktop icon opens.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tool {
    /// Temporary until the phonebook lands: jack into a fresh random network.
    JackIn,
    Terminal,
    Rig,
    Stash,
    HexEd,
    Shell,
}

impl Tool {
    pub const ALL: [Tool; 6] = [
        Tool::JackIn,
        Tool::Terminal,
        Tool::Rig,
        Tool::Stash,
        Tool::HexEd,
        Tool::Shell,
    ];

    /// Tools that work yet. The rest ask you to insert their disk.
    pub fn installed(self) -> bool {
        !matches!(self, Tool::Terminal)
    }

    /// Volume name for the "insert volume" requester.
    pub fn volume(self) -> &'static str {
        match self {
            Tool::JackIn => "JackIn",
            Tool::Terminal => "CarrierTerm",
            Tool::Rig => "RIG",
            Tool::Stash => "STASH",
            Tool::HexEd => "HexEd",
            Tool::Shell => "Shell",
        }
    }
}

/// A desktop icon: pixel art in the 1.3 palette (see `data/hub/icons.ron`).
#[derive(Deserialize, Clone, Debug)]
pub struct IconDef {
    pub label: String,
    pub tool: Tool,
    pub pixels: Vec<String>,
}

impl IconDef {
    pub fn width(&self) -> usize {
        self.pixels.first().map_or(0, |r| r.chars().count())
    }

    pub fn validate(icons: &[IconDef]) -> Result<(), String> {
        for t in Tool::ALL {
            if !icons.iter().any(|i| i.tool == t) {
                return Err(format!("no desktop icon for {t:?}"));
            }
        }
        for i in icons {
            let w = i.width();
            if w == 0 || i.pixels.is_empty() {
                return Err(format!("icon `{}` is empty", i.label));
            }
            for (n, row) in i.pixels.iter().enumerate() {
                if row.chars().count() != w {
                    return Err(format!("icon `{}` row {n} is not {w} wide", i.label));
                }
                if let Some(c) = row.chars().find(|c| !HUB_PALETTE_CHARS.contains(*c)) {
                    return Err(format!("icon `{}` uses unknown colour `{c}`", i.label));
                }
            }
        }
        Ok(())
    }
}

/// A RIG upgrade line.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Upgrade {
    Ram,
    Cpu,
    Hdd,
    Integrity,
    Stash,
}

impl Upgrade {
    pub const ALL: [Upgrade; 5] = [
        Upgrade::Ram,
        Upgrade::Cpu,
        Upgrade::Hdd,
        Upgrade::Integrity,
        Upgrade::Stash,
    ];
}

/// One line in the RIG shop (`data/hub/rig.ron`).
#[derive(Deserialize, Clone, Debug)]
pub struct UpgradeDef {
    pub kind: Upgrade,
    pub label: String,
    pub unit: String,
    /// What one purchase adds.
    pub step: u32,
    /// Most purchases allowed.
    pub max: u32,
    pub base_cost: u32,
    /// Each purchase costs this much more than the last, in percent.
    pub growth_pct: u32,
    pub blurb: String,
}

impl UpgradeDef {
    /// Price of the next purchase when `level` are already owned, or `None` when maxed.
    pub fn cost(&self, level: u32) -> Option<u32> {
        if level >= self.max {
            return None;
        }
        let mut c = u64::from(self.base_cost);
        for _ in 0..level {
            c = c * u64::from(100 + self.growth_pct) / 100;
        }
        Some(c.min(u64::from(u32::MAX)) as u32)
    }

    pub fn validate(defs: &[UpgradeDef]) -> Result<(), String> {
        for u in Upgrade::ALL {
            match defs.iter().filter(|d| d.kind == u).count() {
                1 => {}
                n => return Err(format!("{u:?} is listed {n} times (need exactly 1)")),
            }
        }
        if let Some(d) = defs
            .iter()
            .find(|d| d.step == 0 || d.max == 0 || d.base_cost == 0)
        {
            return Err(format!(
                "{:?} needs a step, max and base cost above 0",
                d.kind
            ));
        }
        Ok(())
    }
}

/// RIG levels owned, per upgrade.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rig {
    #[serde(default)]
    pub ram: u32,
    #[serde(default)]
    pub cpu: u32,
    #[serde(default)]
    pub hdd: u32,
    #[serde(default)]
    pub integrity: u32,
    #[serde(default)]
    pub stash: u32,
}

impl Rig {
    pub fn level(&self, u: Upgrade) -> u32 {
        match u {
            Upgrade::Ram => self.ram,
            Upgrade::Cpu => self.cpu,
            Upgrade::Hdd => self.hdd,
            Upgrade::Integrity => self.integrity,
            Upgrade::Stash => self.stash,
        }
    }

    fn level_mut(&mut self, u: Upgrade) -> &mut u32 {
        match u {
            Upgrade::Ram => &mut self.ram,
            Upgrade::Cpu => &mut self.cpu,
            Upgrade::Hdd => &mut self.hdd,
            Upgrade::Integrity => &mut self.integrity,
            Upgrade::Stash => &mut self.stash,
        }
    }

    /// Total bonus an upgrade line gives.
    pub fn bonus(&self, u: Upgrade, defs: &[UpgradeDef]) -> u32 {
        defs.iter()
            .find(|d| d.kind == u)
            .map_or(0, |d| d.step * self.level(u))
    }
}

/// A program kept at home. Its binary is signed with the save's key.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct StashItem {
    pub id: u32,
    /// Factory program it started as (warez id), for its blurb.
    pub origin: String,
    /// The 32-byte binary as hex.
    pub hex: String,
}

impl StashItem {
    pub fn bin(&self) -> Option<Bin> {
        binary::from_hex(&self.hex)
    }
}

/// What goes into the next run.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Loadout {
    /// Starter programs by warez id. Always available, free, never at risk.
    #[serde(default = "starter_ids")]
    pub starters: Vec<String>,
    /// Stash items by id. These leave the stash while you're in a run.
    #[serde(default)]
    pub stash: Vec<u32>,
}

fn starter_ids() -> Vec<String> {
    START_WAREZ.iter().map(|s| s.to_string()).collect()
}

impl Default for Loadout {
    fn default() -> Self {
        Self {
            starters: starter_ids(),
            stash: Vec::new(),
        }
    }
}

/// Everything the hub keeps between sessions. One RON document, versioned.
/// Later features add fields with `#[serde(default)]` so older saves still load.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Save {
    pub version: u32,
    #[serde(default)]
    pub runs: u32,
    #[serde(default)]
    pub exits: u32,
    #[serde(default)]
    pub flatlines: u32,
    /// Most credits carried out of a single run.
    #[serde(default)]
    pub best_haul: u32,
    /// Phosphor theme name for runs (`amber`, `green`, `ice`).
    #[serde(default = "default_phosphor")]
    pub phosphor: String,
    #[serde(default = "default_crt")]
    pub crt: bool,
    /// Credits banked at home.
    #[serde(default)]
    pub bank: u32,
    /// Optimizer fragments banked at home.
    #[serde(default)]
    pub fragments: u32,
    /// Signs stash binaries. Set once per save (see [`Save::ensure_key`]).
    #[serde(default)]
    pub key: u64,
    #[serde(default)]
    pub rig: Rig,
    #[serde(default)]
    pub stash: Vec<StashItem>,
    #[serde(default)]
    pub next_item: u32,
    #[serde(default)]
    pub loadout: Loadout,
}

fn default_phosphor() -> String {
    "amber".into()
}

fn default_crt() -> bool {
    true
}

impl Default for Save {
    fn default() -> Self {
        Self {
            version: SAVE_VERSION,
            runs: 0,
            exits: 0,
            flatlines: 0,
            best_haul: 0,
            phosphor: default_phosphor(),
            crt: default_crt(),
            bank: 0,
            fragments: 0,
            key: 0,
            rig: Rig::default(),
            stash: Vec::new(),
            next_item: 0,
            loadout: Loadout::default(),
        }
    }
}

#[derive(Deserialize)]
struct VersionProbe {
    version: u32,
}

impl Save {
    /// Parses a save. Anything unreadable or from another version is an error;
    /// the app then starts a fresh hub instead of crashing.
    pub fn load(text: &str) -> Result<Save, String> {
        let probe: VersionProbe =
            ron::from_str(text).map_err(|e| format!("unreadable save: {e}"))?;
        if probe.version != SAVE_VERSION {
            return Err(format!(
                "save is version {}, this build reads {SAVE_VERSION}",
                probe.version
            ));
        }
        ron::from_str(text).map_err(|e| format!("corrupt save: {e}"))
    }

    pub fn to_text(&self) -> String {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()).unwrap_or_default()
    }

    /// Gives a save without a signing key one. Call with something unpredictable.
    pub fn ensure_key(&mut self, fresh: u64) -> bool {
        if self.key == 0 {
            self.key = fresh | 1;
            return true;
        }
        false
    }

    pub fn stash_cap(&self, defs: &[UpgradeDef]) -> u32 {
        STASH_BASE_SLOTS + self.rig.bonus(Upgrade::Stash, defs)
    }

    /// HDD slots the next run's deck will have: the loadout's limit.
    pub fn hdd_cap(&self, defs: &[UpgradeDef]) -> u32 {
        START_HDD + self.rig.bonus(Upgrade::Hdd, defs)
    }

    pub fn loadout_size(&self) -> usize {
        self.loadout.starters.len()
            + self
                .loadout
                .stash
                .iter()
                .filter(|id| self.item(**id).is_some())
                .count()
    }

    pub fn item(&self, id: u32) -> Option<&StashItem> {
        self.stash.iter().find(|s| s.id == id)
    }

    /// Buys the next level of an upgrade with banked credits.
    pub fn buy(&mut self, defs: &[UpgradeDef], u: Upgrade) -> Result<String, String> {
        let def = defs.iter().find(|d| d.kind == u).ok_or("no such upgrade")?;
        let level = self.rig.level(u);
        let cost = def
            .cost(level)
            .ok_or_else(|| format!("{} is maxed out.", def.label))?;
        if self.bank < cost {
            return Err(format!(
                "{} costs {cost}¢. you have {}¢.",
                def.label, self.bank
            ));
        }
        self.bank -= cost;
        *self.rig.level_mut(u) += 1;
        Ok(format!(
            "{} upgraded to level {} (-{cost}¢).",
            def.label,
            level + 1
        ))
    }

    /// Adds or removes a starter program from the loadout.
    pub fn toggle_starter(&mut self, defs: &[UpgradeDef], id: &str) -> Result<(), String> {
        if let Some(i) = self.loadout.starters.iter().position(|s| s == id) {
            self.loadout.starters.remove(i);
            return Ok(());
        }
        if !START_WAREZ.contains(&id) {
            return Err(format!("{id} isn't a starter program."));
        }
        self.room_in_loadout(defs)?;
        self.loadout.starters.push(id.to_string());
        Ok(())
    }

    /// Adds or removes a stash program from the loadout.
    pub fn toggle_stash(&mut self, defs: &[UpgradeDef], id: u32) -> Result<(), String> {
        if let Some(i) = self.loadout.stash.iter().position(|s| *s == id) {
            self.loadout.stash.remove(i);
            return Ok(());
        }
        if self.item(id).is_none() {
            return Err("no such program in the stash.".into());
        }
        self.room_in_loadout(defs)?;
        self.loadout.stash.push(id);
        Ok(())
    }

    fn room_in_loadout(&self, defs: &[UpgradeDef]) -> Result<(), String> {
        let cap = self.hdd_cap(defs) as usize;
        if self.loadout_size() >= cap {
            return Err(format!("the deck's HDD only holds {cap} programs."));
        }
        Ok(())
    }

    /// Bins a stash program for good.
    pub fn scrap(&mut self, id: u32) -> Option<StashItem> {
        let i = self.stash.iter().position(|s| s.id == id)?;
        self.loadout.stash.retain(|s| *s != id);
        Some(self.stash.remove(i))
    }

    /// Writes an edited binary over a stash program. Refused unless it checks out under the save's key.
    pub fn patch(&mut self, id: u32, bin: &Bin) -> Result<String, String> {
        let key = self.key;
        let item = self
            .stash
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or("no such program in the stash.")?;
        let spec = binary::check(bin, key).map_err(|f| format!("link failed: {}", f.join("; ")))?;
        item.hex = binary::to_hex(bin);
        Ok(format!("wrote {}. link ok.", spec.file()))
    }

    /// Spends banked fragments to raise a stash program's opt level.
    pub fn optimize(&mut self, id: u32) -> Result<String, String> {
        if self.fragments < FRAGMENTS_PER_OPT {
            return Err(format!(
                "optimizing needs {FRAGMENTS_PER_OPT} fragment(s). you have {}.",
                self.fragments
            ));
        }
        let key = self.key;
        let item = self
            .stash
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or("no such program in the stash.")?;
        let mut bin = item.bin().ok_or("that program is corrupt.")?;
        if u32::from(bin[binary::OFF_OPT]) >= MAX_OPT {
            return Err("already fully optimized.".into());
        }
        bin[binary::OFF_OPT] += 1;
        binary::seal(&mut bin, key);
        let spec = binary::check(&bin, key).map_err(|f| f.join("; "))?;
        item.hex = binary::to_hex(&bin);
        self.fragments -= FRAGMENTS_PER_OPT;
        Ok(format!("{} optimized to O{}.", spec.file(), spec.opt))
    }

    /// What the next run starts with: RIG bonuses plus the loadout's programs.
    /// An empty loadout falls back to the starters, so you can never jack in with nothing.
    pub fn kit(&self, data: &Data, defs: &[UpgradeDef]) -> Kit {
        let mut programs = Vec::new();
        for id in &self.loadout.starters {
            if let Some(w) = data.warez_ix(id) {
                programs.push((w, binary::encode(&data.warez[w], self.key)));
            }
        }
        for id in &self.loadout.stash {
            if let Some(item) = self.item(*id)
                && let (Some(w), Some(bin)) = (data.warez_ix(&item.origin), item.bin())
            {
                programs.push((w, bin));
            }
        }
        programs.truncate(self.hdd_cap(defs) as usize);
        Kit {
            ram: self.rig.bonus(Upgrade::Ram, defs),
            cpu: self.rig.bonus(Upgrade::Cpu, defs),
            hdd: self.rig.bonus(Upgrade::Hdd, defs),
            integrity: self.rig.bonus(Upgrade::Integrity, defs),
            programs,
        }
    }

    /// Starts a run. Stash programs in the loadout leave the stash: they come back only
    /// if you reach the exit and keep them.
    pub fn jack_in(&mut self, data: &Data, defs: &[UpgradeDef], seed: u64) -> Game {
        let kit = self.kit(data, defs);
        let taken: Vec<u32> = self.loadout.stash.drain(..).collect();
        self.stash.retain(|s| !taken.contains(&s.id));
        Game::start(data.clone(), seed, &kit)
    }

    /// Books a finished run: banks credits and fragments, and on an exit stashes the programs
    /// in `keep` (HDD indices) while there's room. Returns the report, or `None` if the run isn't over.
    pub fn record(
        &mut self,
        game: &Game,
        keep: &[usize],
        defs: &[UpgradeDef],
    ) -> Option<RunReport> {
        let end = match game.phase {
            Phase::JackedOut => RunEnd::Exit,
            Phase::Flatlined => RunEnd::Flatline,
            _ => return None,
        };
        self.runs += 1;
        let credits = game.deck.credits;
        let mut kept = Vec::new();
        let banked = match end {
            RunEnd::Exit => {
                self.exits += 1;
                self.best_haul = self.best_haul.max(credits);
                self.fragments += game.deck.fragments;
                let cap = self.stash_cap(defs) as usize;
                let mut seen = Vec::new();
                for &i in keep {
                    if self.stash.len() >= cap || seen.contains(&i) {
                        continue;
                    }
                    seen.push(i);
                    let Some(slot) = game.hdd.get(i) else {
                        continue;
                    };
                    // The run's signature comes off; home signs it again.
                    let mut bin = slot.bin;
                    binary::seal(&mut bin, self.key);
                    if binary::check(&bin, self.key).is_err() {
                        continue;
                    }
                    let id = self.next_item;
                    self.next_item += 1;
                    self.stash.push(StashItem {
                        id,
                        origin: game.data.warez[slot.warez].id.clone(),
                        hex: binary::to_hex(&bin),
                    });
                    kept.push(slot.spec.file());
                }
                credits
            }
            RunEnd::Flatline => {
                self.flatlines += 1;
                credits * DEADMAN_PCT / 100
            }
        };
        self.bank += banked;
        Some(RunReport {
            end,
            credits,
            banked,
            hops: game.hops,
            seed: game.seed,
            kept,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunEnd {
    Exit,
    Flatline,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunReport {
    pub end: RunEnd,
    /// Credits held when the run ended.
    pub credits: u32,
    /// Credits that made it into the bank.
    pub banked: u32,
    pub hops: u32,
    pub seed: u64,
    /// Files of the programs stashed on the way out.
    pub kept: Vec<String>,
}

/// Which machine is on screen, and the drags between them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Screen {
    /// The Workbench desktop.
    Hub,
    /// The desktop sliding down to reveal the run behind it.
    JackIn {
        t: f32,
    },
    Run,
    /// Flatlined: the crash screen, until you click.
    Crash,
    /// The desktop sliding back up over the run.
    JackOut {
        t: f32,
    },
}

impl Screen {
    pub fn tick(self, dt: f32) -> Screen {
        match self {
            Screen::JackIn { t } if t + dt >= HUB_DRAG_SECS => Screen::Run,
            Screen::JackIn { t } => Screen::JackIn { t: t + dt },
            Screen::JackOut { t } if t + dt >= HUB_DRAG_SECS => Screen::Hub,
            Screen::JackOut { t } => Screen::JackOut { t: t + dt },
            s => s,
        }
    }

    /// How far the desktop is dragged down, 0 (covering everything) to 1 (off the bottom).
    /// Eased like a hand pulling a screen: fast start, soft landing.
    pub fn drag(self) -> f32 {
        let ease = |x: f32| 1.0 - (1.0 - x.clamp(0.0, 1.0)).powi(3);
        match self {
            Screen::Hub => 0.0,
            Screen::JackIn { t } => ease(t / HUB_DRAG_SECS),
            Screen::Run | Screen::Crash => 1.0,
            Screen::JackOut { t } => 1.0 - ease(t / HUB_DRAG_SECS),
        }
    }

    /// Whether the run's UI is visible at all.
    pub fn shows_run(self) -> bool {
        !matches!(self, Screen::Hub)
    }

    /// Whether the desktop takes input.
    pub fn hub_live(self) -> bool {
        self == Screen::Hub
    }
}

/// What a Shell command asks the app to do, besides printing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShellEffect {
    None,
    JackIn(Option<u64>),
    Open(Tool),
    Theme(String),
    Crt,
    Clear,
}

/// Runs one Shell line. Returns the lines to print and what to do.
pub fn shell(line: &str, save: &Save, icons: &[IconDef]) -> (Vec<String>, ShellEffect) {
    let mut words = line.split_whitespace();
    let Some(cmd) = words.next() else {
        return (Vec::new(), ShellEffect::None);
    };
    let arg = words.next();
    let lines = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    match cmd.to_ascii_lowercase().as_str() {
        "help" | "?" => (
            lines(&[
                "dir              list the desktop",
                "run <tool>       open a tool (jackin, shell, ...)",
                "jackin [seed]    jack into a network (hex seed optional)",
                "info             your record",
                "theme <name>     run phosphor: amber | green | ice",
                "crt              toggle scanlines in runs",
                "echo <text>      say something",
                "clear            clear the window",
            ]),
            ShellEffect::None,
        ),
        "dir" | "ls" => {
            let mut out = vec!["Workbench:".to_string()];
            for i in icons {
                let state = if i.tool.installed() {
                    ""
                } else {
                    "  (not installed)"
                };
                out.push(format!(
                    "  {:<10} {}{state}",
                    i.label,
                    if i.tool == Tool::Shell {
                        "tool"
                    } else {
                        "drawer"
                    }
                ));
            }
            (out, ShellEffect::None)
        }
        "info" | "stats" => (
            vec![
                format!("runs       {}", save.runs),
                format!("exits      {}", save.exits),
                format!("flatlines  {}", save.flatlines),
                format!("best haul  {}¢", save.best_haul),
                format!("bank       {}¢", save.bank),
                format!("fragments  {}", save.fragments),
                format!("stash      {} program(s)", save.stash.len()),
                format!("phosphor   {}", save.phosphor),
            ],
            ShellEffect::None,
        ),
        "jackin" | "jack" | "dial" => match arg {
            None => (vec!["dialing...".into()], ShellEffect::JackIn(None)),
            Some(s) => {
                match u64::from_str_radix(s.trim_start_matches("0x").trim_start_matches('#'), 16) {
                    Ok(seed) => (
                        vec![format!("dialing seed {seed:x}...")],
                        ShellEffect::JackIn(Some(seed)),
                    ),
                    Err(_) => (vec![format!("`{s}` is not a hex seed.")], ShellEffect::None),
                }
            }
        },
        "run" | "open" => {
            let Some(name) = arg else {
                return (vec!["run what? try `dir`.".into()], ShellEffect::None);
            };
            match icons.iter().find(|i| i.label.eq_ignore_ascii_case(name)) {
                Some(i) => (Vec::new(), ShellEffect::Open(i.tool)),
                None => (vec![format!("{name}: object not found")], ShellEffect::None),
            }
        }
        "theme" => match arg {
            Some(t @ ("amber" | "green" | "ice")) => {
                (vec![format!("phosphor: {t}")], ShellEffect::Theme(t.into()))
            }
            _ => (vec!["theme amber | green | ice".into()], ShellEffect::None),
        },
        "crt" => (Vec::new(), ShellEffect::Crt),
        "clear" | "cls" => (Vec::new(), ShellEffect::Clear),
        "echo" => (
            vec![line.trim_start()[cmd.len()..].trim().to_string()],
            ShellEffect::None,
        ),
        other => (vec![format!("Unknown command {other}")], ShellEffect::None),
    }
}
