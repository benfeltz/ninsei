//! The hub: your machine between runs. The save document, the screen flow between the
//! Workbench desktop and a run, and the desktop Shell's commands. No UI here.

use serde::{Deserialize, Serialize};

use crate::binary::{self, Bin};
use crate::data::{Data, IceClass};
use crate::game::{Caps, ContractStatus, Game, Kit, Phase, RunMods, Site};
use crate::network::{NetParams, Shape};
use crate::rng::Rng;
use crate::tuning::{
    BBS_LAYER, DEADMAN_PCT, FRAGMENTS_PER_OPT, HEAT_COOL, HEAT_ICE, HEAT_MAX, HEAT_PER_RUN,
    HEAT_TRACE_PCT, HUB_DRAG_SECS, HUB_PALETTE_CHARS, LOCAL_RANGE, MAX_OPT, NET_MID_LAYERS,
    PHONEBOOK_START, RESULTS_KEPT, START_WAREZ, STASH_BASE_SLOTS, TRAP_TRACE_PCT, WARDIAL_BLOCK,
};

/// Bump when the save's shape changes incompatibly. 2: binary byte 07 (the RAM cost) became
/// reserved, so a version-1 stash would fail its checks: a version-1 save starts fresh.
pub const SAVE_VERSION: u32 = 2;
/// Run theme a new save starts with.
pub const DEFAULT_RUN_THEME: &str = "amber";

/// The one number a new deck comes with: Sense/Net's customer board (`data/bbs/sensenet/`).
pub const FIRST_BOARD: &str = "sensenet";

/// Storage key the app saves under.
pub const SAVE_KEY: &str = "ninsei.save";

/// A home tool: what a desktop icon, tile or launcher entry opens.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tool {
    /// CarrierTerm: the phonebook you dial networks from.
    Terminal,
    Rig,
    HexEd,
    Shell,
    /// System: the deck's settings app (About This Deck, Monitor, Theme). It replaced Prefs.
    System,
    /// The QWK mail reader: boards' bulletins and saved textfiles, offline.
    Qwk,
    /// The ICE database: every ICE you've fought.
    IceDb,
    /// NiGHTCOMM, the hacker's dashboard: heat, a wardialer, ranges and a modem terminal.
    NightComm,
    /// Tiger Team Pro, the corporate audit suite: everything NiGHTCOMM does, plus network intel.
    TigerTeam,
    /// The Welcome to the Net stack (`data/stacks/welcome.ron`).
    Guide,
    /// TRAX, the music player: the tunes you own as a playlist (`player.rs`).
    Player,
    /// A copy of a game you own (`data/hub/games.ron`): its name, field, icon and intro all
    /// come from its row, and the cracked and the boxed copy are two tools side by side.
    Game(GameId, GameCopy),
}

/// The longest id a game may have (`id` in `games.ron`), so a [`Tool`] carries it by value.
pub const GAME_ID_MAX: usize = 15;

/// A game's id, held inline so a [`Tool::Game`] stays `Copy`. Saved as the plain id.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct GameId {
    len: u8,
    bytes: [u8; GAME_ID_MAX],
}

impl GameId {
    /// The id, if it's 1 to [`GAME_ID_MAX`] ASCII bytes.
    pub fn new(id: &str) -> Option<GameId> {
        if id.is_empty() || id.len() > GAME_ID_MAX || !id.is_ascii() {
            return None;
        }
        let mut bytes = [0; GAME_ID_MAX];
        bytes[..id.len()].copy_from_slice(id.as_bytes());
        Some(GameId {
            len: id.len() as u8,
            bytes,
        })
    }

    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..self.len as usize]).unwrap_or("")
    }
}

impl std::fmt::Debug for GameId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.as_str())
    }
}

impl Serialize for GameId {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for GameId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let id = String::deserialize(d)?;
        GameId::new(&id).ok_or_else(|| serde::de::Error::custom(format!("bad game id `{id}`")))
    }
}

impl Tool {
    pub const ALL: [Tool; 11] = [
        Tool::Terminal,
        Tool::Rig,
        Tool::HexEd,
        Tool::Qwk,
        Tool::IceDb,
        Tool::NightComm,
        Tool::TigerTeam,
        Tool::Player,
        Tool::Guide,
        Tool::Shell,
        Tool::System,
    ];

    /// Tools that work yet. The rest ask you to insert their disk.
    pub fn installed(self) -> bool {
        true
    }

    /// A copy of game `id`, if the id can be one.
    pub fn game(id: &str, copy: GameCopy) -> Option<Tool> {
        GameId::new(id).map(|g| Tool::Game(g, copy))
    }

    /// The copies of games you own, as tools, in the order you came by them.
    pub fn games(data: &Data, save: &Save) -> Vec<Tool> {
        save.games
            .iter()
            .filter(|g| data.game(&g.id).is_some())
            .filter_map(|g| Tool::game(&g.id, g.copy))
            .collect()
    }

    /// What the tool is called: its volume name (the "insert volume" requester, the tiling
    /// bar, the fit notice), or a game copy's label (`Nibbler`, `NIBBLER [2K4]`).
    pub fn name(self, data: &Data) -> String {
        match self {
            Tool::Game(id, copy) => data.game_label(id.as_str(), copy),
            t => t.volume().to_string(),
        }
    }

    /// The game this is a copy of, with its row.
    pub fn game_def(self, data: &Data) -> Option<(&crate::data::GameDef, GameCopy)> {
        match self {
            Tool::Game(id, copy) => data.game(id.as_str()).map(|g| (g, copy)),
            _ => None,
        }
    }

    /// Volume name for the "insert volume" requester.
    fn volume(self) -> &'static str {
        match self {
            Tool::Game(..) => "Game",
            Tool::Terminal => "CarrierTerm",
            Tool::Rig => "CPU Shopper",
            Tool::HexEd => "HexEd",
            Tool::Shell => "Shell",
            Tool::System => "System",
            Tool::Qwk => "QWK",
            Tool::IceDb => "ICE DB",
            Tool::NightComm => "NiGHTCOMM",
            Tool::TigerTeam => "Tiger Team Pro",
            Tool::Guide => "Welcome",
            Tool::Player => "TRAX",
        }
    }
}

/// The Liza desktop's own 1-bit pixel art (`data/hub/liza.ron`): `#` ink, `.` paper.
#[derive(Deserialize, Clone, Debug)]
pub struct LizaArt {
    /// The happy deck on the boot card.
    pub boot: Vec<String>,
    pub trash: Vec<String>,
}

impl LizaArt {
    pub fn validate(&self) -> Result<(), String> {
        for (name, rows) in [("boot", &self.boot), ("trash", &self.trash)] {
            let w = rows.first().map_or(0, |r| r.chars().count());
            if w == 0 {
                return Err(format!("`{name}` is empty"));
            }
            if let Some(n) = rows.iter().position(|r| r.chars().count() != w) {
                return Err(format!("`{name}` row {n} is not {w} wide"));
            }
            if let Some(c) = rows
                .iter()
                .flat_map(|r| r.chars())
                .find(|c| !"#.".contains(*c))
            {
                return Err(format!("`{name}` has `{c}`: only `#` and `.`"));
            }
        }
        Ok(())
    }
}

/// A desktop icon: pixel art in the 1.3 palette (see `data/hub/icons.ron`).
#[derive(Deserialize, Clone, Debug)]
pub struct IconDef {
    pub label: String,
    pub tool: Tool,
    pub pixels: Vec<String>,
}

/// How wide a piece of pixel art is: its first row's length (every row is as wide, checked
/// by [`IconDef::check_art`]).
fn art_width(pixels: &[String]) -> usize {
    pixels.first().map_or(0, |r| r.chars().count())
}

impl IconDef {
    pub fn width(&self) -> usize {
        art_width(&self.pixels)
    }

    pub fn validate(icons: &[IconDef]) -> Result<(), String> {
        for t in Tool::ALL {
            if !icons.iter().any(|i| i.tool == t) {
                return Err(format!("no desktop icon for {t:?}"));
            }
        }
        for i in icons {
            if matches!(i.tool, Tool::Game(..)) {
                return Err(format!(
                    "icon `{}` is a game's: a game's icon is its `icon` in games.ron",
                    i.label
                ));
            }
            IconDef::check_art(&i.label, &i.pixels)?;
        }
        Ok(())
    }

    /// One icon's pixels: not empty, every row as wide as the first, every pixel in the 1.3
    /// palette.
    pub fn check_art(label: &str, pixels: &[String]) -> Result<(), String> {
        let w = art_width(pixels);
        if w == 0 {
            return Err(format!("icon `{label}` is empty"));
        }
        for (n, row) in pixels.iter().enumerate() {
            if row.chars().count() != w {
                return Err(format!("icon `{label}` row {n} is not {w} wide"));
            }
            if let Some(c) = row.chars().find(|c| !HUB_PALETTE_CHARS.contains(*c)) {
                return Err(format!("icon `{label}` uses unknown colour `{c}`"));
            }
        }
        Ok(())
    }
}

/// A desktop icon as a desktop shows it, borrowed from `icons.ron` or a game's row: its label
/// (a game copy's, worked out), the tool it opens and its pixel art.
#[derive(Clone, Debug)]
pub struct Icon<'a> {
    pub label: std::borrow::Cow<'a, str>,
    pub tool: Tool,
    pub pixels: &'a [String],
}

impl Icon<'_> {
    pub fn width(&self) -> usize {
        art_width(self.pixels)
    }
}

/// A network tier: size, ICE strength and pay (`data/hub/networks.ron`).
#[derive(Deserialize, Clone, Debug)]
pub struct TierDef {
    pub tier: u32,
    pub label: String,
    pub mid_layers: usize,
    pub ice_bonus: u32,
    pub pay_pct: u32,
    /// How many traits a network of this tier rolls: (min, max).
    pub traits: (u32, u32),
    /// Subnet nodes on a network of this tier.
    #[serde(default)]
    pub subnets: usize,
    /// Every `choke`th middle layer is a single guarded node, so some fights can't be routed
    /// around (0 = none). A trait's `Shape` replaces it.
    #[serde(default)]
    pub choke: usize,
    /// Extra weight for black ICE, on top of any trait's.
    #[serde(default)]
    pub black: u32,
    /// Trace gain, percent of normal, before traits and heat.
    #[serde(default = "full_pct")]
    pub trace_pct: u32,
}

fn full_pct() -> u32 {
    100
}

/// What a trait does to a network.
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum TraitEffect {
    IceClass(IceClass),
    BlackIce(u32),
    NoBbs,
    Trace(u32),
    Chatty(u32),
    VaultRich(u32),
    Pay(u32),
    /// The network's topology: layer widths, cross links, chokepoints.
    Shape(Shape),
    /// +n subnet nodes.
    Subnets(usize),
    /// This network's BBS is the board with this id (`data/bbs/<id>/`).
    Board(String),
}

#[derive(Deserialize, Clone, Debug)]
pub struct TraitDef {
    pub id: String,
    pub label: String,
    pub blurb: String,
    pub effect: TraitEffect,
    /// How Tiger Team Pro's assessment words it: the corporate view of the same thing.
    pub finding: String,
    pub severity: Severity,
}

/// How much a finding should worry a security team.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Info,
    Low,
    Medium,
    High,
}

impl Severity {
    pub fn label(self) -> &'static str {
        match self {
            Severity::Info => "Info",
            Severity::Low => "Low",
            Severity::Medium => "Medium",
            Severity::High => "High",
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct CompanyDef {
    pub name: String,
    pub business: String,
}

/// How a new deck starts (`data/hub/starts.ron`): what every deck gets, and the kits.
#[derive(Deserialize, Clone, Debug)]
pub struct StartsDef {
    pub always: Vec<FixedNet>,
    pub kits: Vec<KitDef>,
}

/// A network every deck starts with, with its own name, number and traits.
#[derive(Deserialize, Clone, Debug)]
pub struct FixedNet {
    pub name: String,
    pub business: String,
    pub number: String,
    pub tier: u32,
    #[serde(default)]
    pub traits: Vec<String>,
}

/// One starting kit.
#[derive(Deserialize, Clone, Debug)]
pub struct KitDef {
    pub id: String,
    #[serde(default)]
    pub default: bool,
    /// Programs the deck ships with (warez ids): free, always in the loadout, never lost.
    pub free: Vec<String>,
    #[serde(default)]
    pub boards: Vec<String>,
    /// The starter networks (`PHONEBOOK_START`) already in the phonebook.
    #[serde(default)]
    pub numbers: bool,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub ranges: Vec<String>,
    #[serde(default)]
    pub bank: u32,
    /// Games already on the desktop (a game id and the copy): the disk the shop threw in.
    #[serde(default)]
    pub games: Vec<(String, GameCopy)>,
    /// Tunes the deck ships with (ids in `data/music/`), for the player once you have one.
    #[serde(default)]
    pub tunes: Vec<String>,
    /// The program the deck ships with (BASIC text): every fight opens with it loaded. Empty:
    /// the default kit's (read it with `StartsDef::program`).
    #[serde(default)]
    pub program: String,
}

/// What's wrong with a shipped program, if anything: a line that doesn't parse, a GOTO to
/// nowhere, or a RUN or HAVE naming a program no warez is called.
pub fn program_fault(text: &str, data: &Data) -> Option<String> {
    let p = crate::basic::parse(text);
    if let Some(s) = p.stray.first() {
        return Some(format!("`{s}`: {}", crate::basic::stray_fault(s)));
    }
    let known = |typed: &str| {
        data.warez
            .iter()
            .any(|w| crate::basic::names(typed, &w.name, &w.id))
    };
    p.lines.iter().find_map(|l| {
        if let Some(why) = p.fault(l.no) {
            return Some(format!("line {}: {why}", l.no));
        }
        match &l.stmt {
            crate::basic::Stmt::Run(n) | crate::basic::Stmt::If(crate::basic::Cond::Have(n))
                if !known(n) =>
            {
                Some(format!("line {}: no warez called `{n}`", l.no))
            }
            _ => None,
        }
    })
}

impl StartsDef {
    /// The program kit `id` ships: its own, or the default kit's.
    pub fn program(&self, id: &str) -> &str {
        let own = &self.kit(id).program;
        if own.is_empty() {
            &self.kit("").program
        } else {
            own
        }
    }

    /// The kit called `id`, or the default kit.
    pub fn kit(&self, id: &str) -> &KitDef {
        self.kits
            .iter()
            .find(|k| k.id == id)
            .or_else(|| self.kits.iter().find(|k| k.default))
            .unwrap_or(&self.kits[0])
    }

    pub fn validate(&self, data: &Data) -> Result<(), String> {
        if self.kits.iter().filter(|k| k.default).count() != 1 {
            return Err("starts.ron: exactly one kit must be `default: true`".into());
        }
        for (i, k) in self.kits.iter().enumerate() {
            let err = |m: String| Err(format!("starts.ron: kit `{}`: {m}", k.id));
            if self.kits[..i].iter().any(|o| o.id == k.id) {
                return err("listed twice".into());
            }
            if let Some(w) = k.free.iter().find(|w| data.warez_ix(w).is_none()) {
                return err(format!("unknown warez `{w}`"));
            }
            if let Some(b) = k.boards.iter().find(|b| data.board(b).is_none()) {
                return err(format!("unknown board `{b}`"));
            }
            if let Some(t) = k.tools.iter().find(|t| data.tool(t).is_none()) {
                return err(format!("unknown tool `{t}`"));
            }
            if let Some(r) = k
                .ranges
                .iter()
                .find(|r| !data.exchanges.iter().any(|x| x.id == **r))
            {
                return err(format!("unknown range `{r}`"));
            }
            if let Some((g, _)) = k.games.iter().find(|(g, _)| data.game(g).is_none()) {
                return err(format!("unknown game `{g}`"));
            }
            if let Some(t) = k.tunes.iter().find(|t| data.tune(t).is_none()) {
                return err(format!("unknown tune `{t}`"));
            }
            if let Some(why) = program_fault(&k.program, data) {
                return err(format!("program: {why}"));
            }
            if k.default && k.program.is_empty() {
                return err("the default kit needs a program: the others fall back to it".into());
            }
        }
        for n in &self.always {
            if data.networks.tier(n.tier).is_none() {
                return Err(format!(
                    "starts.ron: `{}` has unknown tier {}",
                    n.name, n.tier
                ));
            }
            if let Some(t) = n
                .traits
                .iter()
                .find(|t| data.networks.trait_def(t).is_none())
            {
                return Err(format!("starts.ron: `{}` has unknown trait `{t}`", n.name));
            }
            if data.boards.iter().any(|b| b.phone == n.number) {
                return Err(format!("starts.ron: `{}` shares a board's number", n.name));
            }
        }
        Ok(())
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct NetworksDef {
    pub tiers: Vec<TierDef>,
    pub traits: Vec<TraitDef>,
    pub companies: Vec<CompanyDef>,
}

impl NetworksDef {
    pub fn tier(&self, t: u32) -> Option<&TierDef> {
        self.tiers.iter().find(|d| d.tier == t)
    }

    pub fn trait_def(&self, id: &str) -> Option<&TraitDef> {
        self.traits.iter().find(|d| d.id == id)
    }

    pub fn validate(&self) -> Result<(), String> {
        for t in 1..=3 {
            let d = self.tier(t).ok_or_else(|| format!("no tier {t}"))?;
            if d.mid_layers < BBS_LAYER {
                return Err(format!("tier {t} needs at least {BBS_LAYER} middle layers"));
            }
            if d.traits.0 > d.traits.1 {
                return Err(format!("tier {t} trait range is backwards"));
            }
        }
        for (i, t) in self.traits.iter().enumerate() {
            if t.finding.trim().is_empty() {
                return Err(format!("trait `{}` has no finding", t.id));
            }
            if self.traits[..i].iter().any(|o| o.id == t.id) {
                return Err(format!("trait `{}` is listed twice", t.id));
            }
            if let TraitEffect::Shape(shape) = t.effect {
                shape
                    .validate()
                    .map_err(|e| format!("trait `{}`: {e}", t.id))?;
            }
        }
        if self.companies.len() < PHONEBOOK_START.len() * 2 {
            return Err("too few companies for a phonebook".into());
        }
        Ok(())
    }
}

/// A number in your phonebook.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: u32,
    pub name: String,
    pub business: String,
    /// `555-0142` style.
    pub number: String,
    pub tier: u32,
    /// Trait ids.
    pub traits: Vec<String>,
    /// How hot this network is from recent runs (0 to `HEAT_MAX`).
    #[serde(default)]
    pub heat: u32,
    #[serde(default)]
    pub runs: u32,
    /// How the last few runs here ended, oldest first (at most `tuning::RESULTS_KEPT`).
    #[serde(default)]
    pub last: Vec<RunEnd>,
}

/// What a wardial scan turned up on one number worth remembering.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum Found {
    /// A network's modem: the phonebook entry it went into.
    Network(u32),
    /// A bulletin board, by id.
    Board(String),
    /// A line the telco is watching.
    Watched,
}

/// One find from a wardial scan: where it was and what answered.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Find {
    pub range: String,
    pub block: u32,
    pub number: String,
    pub what: Found,
}

/// A range of phone numbers the wardialer can scan (`data/hub/exchanges.ron`).
#[derive(Deserialize, Clone, Debug)]
pub struct RangeDef {
    pub id: String,
    pub label: String,
    /// Known digits, e.g. `555-` or `555-77`.
    pub prefix: String,
    /// Company whose lines these are.
    pub owner: Option<String>,
    pub carrier_pct: u32,
    pub trap_pct: u32,
    /// Weights for a carrier's tier: (LAN, CORP, MEGACORP).
    pub tiers: (u32, u32, u32),
    /// Credits per scan.
    pub cost: u32,
}

/// A full number is `555-` and four digits.
const NUMBER_LEN: usize = 8;

impl RangeDef {
    /// Digits still unknown.
    pub fn free_digits(&self) -> usize {
        NUMBER_LEN - self.prefix.len()
    }

    /// `555-77xx` style.
    pub fn pattern(&self) -> String {
        format!("{}{}", self.prefix, "x".repeat(self.free_digits()))
    }

    /// 100-number blocks in the range.
    pub fn blocks(&self) -> u32 {
        10u32.pow(self.free_digits().saturating_sub(2) as u32)
    }

    /// The numbers in block `b`.
    pub fn numbers(&self, b: u32) -> Vec<String> {
        let w = self.free_digits();
        (0..WARDIAL_BLOCK)
            .map(|i| format!("{}{:0w$}", self.prefix, b * WARDIAL_BLOCK + i))
            .collect()
    }

    pub fn validate(ranges: &[RangeDef], nets: &NetworksDef) -> Result<(), String> {
        if !ranges.iter().any(|r| r.id == LOCAL_RANGE) {
            return Err(format!("no `{LOCAL_RANGE}` range"));
        }
        for (i, r) in ranges.iter().enumerate() {
            if ranges[..i].iter().any(|o| o.id == r.id) {
                return Err(format!("range `{}` is listed twice", r.id));
            }
            let digits_ok =
                r.prefix.starts_with("555-") && r.prefix[4..].chars().all(|c| c.is_ascii_digit());
            if !digits_ok || !(2..=4).contains(&r.free_digits()) {
                return Err(format!(
                    "range `{}` prefix must be 555- plus 0-2 digits",
                    r.id
                ));
            }
            if let Some(o) = &r.owner
                && !nets.companies.iter().any(|c| &c.name == o)
            {
                return Err(format!("range `{}` belongs to unknown company `{o}`", r.id));
            }
            if r.tiers.0 + r.tiers.1 + r.tiers.2 == 0 || r.carrier_pct + r.trap_pct > 100 {
                return Err(format!("range `{}` has bad odds", r.id));
            }
        }
        Ok(())
    }
}

/// A range you know, and how you learned it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct KnownRange {
    pub id: String,
    pub source: String,
}

fn local_range() -> Vec<KnownRange> {
    vec![KnownRange {
        id: LOCAL_RANGE.into(),
        source: "the phone book".into(),
    }]
}

/// What a dialed number answered with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    Voice,
    Busy,
    NoAnswer,
    Fax,
    /// A modem: the phonebook entry it went into, and whether it's new.
    Carrier {
        entry: u32,
        new: bool,
    },
    /// The telco is watching this line.
    Trap,
    /// A BBS (index into `Data::boards`), and whether its number is new to you.
    Board {
        board: usize,
        new: bool,
    },
}

impl Answer {
    /// One character for the scan grid.
    pub fn glyph(&self) -> char {
        match self {
            Answer::Voice => '.',
            Answer::Busy => '-',
            Answer::NoAnswer => ' ',
            Answer::Fax => 'F',
            Answer::Carrier { .. } => 'C',
            Answer::Trap => '!',
            Answer::Board { .. } => 'B',
        }
    }
}

/// One wardial scan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScanResult {
    pub range: String,
    pub block: u32,
    pub cells: Vec<(String, Answer)>,
    pub cost: u32,
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

    /// What a line starts at on a deck that doesn't list it: the `tuning` constants. The only
    /// place they're read for a deck's numbers.
    pub fn fallback(self) -> u32 {
        match self {
            Upgrade::Ram => crate::tuning::START_RAM,
            Upgrade::Cpu => crate::tuning::START_CPU,
            Upgrade::Hdd => crate::tuning::START_HDD,
            Upgrade::Integrity => crate::tuning::START_INTEGRITY,
            Upgrade::Stash => STASH_BASE_SLOTS,
        }
    }
}

/// An amount of a RIG line in its unit: "6 GB", or "18" when the line has none.
pub fn amount(n: u32, unit: &str) -> String {
    if unit.is_empty() {
        n.to_string()
    } else {
        format!("{n} {unit}")
    }
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

/// A copy of a game: a cracked one plays its crew's cracktro, a boxed one its publisher's
/// splash.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GameCopy {
    /// Downloaded from a board.
    Cracked,
    /// Bought by mail order.
    Boxed,
}

/// One copy of a game on the save. Owning both copies is two entries, side by side: ordering
/// the box never erases the crack, and a board still sells the crack to someone with the box.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct OwnedGame {
    pub id: String,
    pub copy: GameCopy,
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
    /// Theme id (see `data/themes.ron`), for the desktop and runs alike.
    #[serde(default = "default_phosphor")]
    pub phosphor: String,
    /// CRT effects on or off.
    #[serde(default = "default_crt")]
    pub crt: bool,
    /// Modem sounds on or off.
    #[serde(default = "default_sound")]
    pub sound: bool,
    /// How loud everything plays, in percent (100 is full); [`Save::sound`] mutes it all.
    #[serde(default = "default_volume")]
    pub volume: u8,
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
    /// The disk's block map, block by block: which program owns it (`None` free). Empty means
    /// laid out end to end. Something can fragment it (`Save::fragment`); Defragment compacts
    /// it again (`Save::defragment`).
    #[serde(default)]
    pub disk_map: Vec<Option<crate::disk::Disk>>,
    /// Numbers you can dial.
    #[serde(default)]
    pub phonebook: Vec<Entry>,
    #[serde(default)]
    pub next_entry: u32,
    /// Number ranges you know (the wardialer scans these).
    #[serde(default = "local_range")]
    pub ranges: Vec<KnownRange>,
    /// Blocks already wardialed, as `range:block`.
    #[serde(default)]
    pub scanned: Vec<String>,
    /// What those scans turned up: networks, boards and watched lines, in the order found.
    #[serde(default)]
    pub finds: Vec<Find>,
    /// Traps tripped since the last dial: extra trace on the next run.
    #[serde(default)]
    pub telco_heat: u32,
    /// Clean jobs done for each client: trusted clients turn on you less. A betrayal resets it.
    #[serde(default)]
    pub trust: Vec<(String, u32)>,
    /// BBS numbers you know (board ids), and where you found them.
    #[serde(default)]
    pub boards: Vec<KnownBoard>,
    /// Files downloaded from home since the last run: (board, area, listing #).
    #[serde(default)]
    pub home_bought: Vec<(String, usize, usize)>,
    /// Jobs taken from home since the last run: (board, job #).
    #[serde(default)]
    pub home_taken: Vec<(String, usize)>,
    /// Jobs taken from home, waiting for their network to be dialed.
    #[serde(default)]
    pub pending: Vec<PendingJob>,
    /// Monitors you own (ids in `data/hub/hardware.ron`), and the one plugged in.
    #[serde(default)]
    pub monitors: Vec<String>,
    #[serde(default)]
    pub monitor: String,
    /// Decks you own, the one you run with, and its firmware (UX version).
    #[serde(default)]
    pub decks: Vec<String>,
    #[serde(default)]
    pub deck: String,
    #[serde(default)]
    pub firmware: String,
    /// The run screen's pane layout per deck firmware (`"deck/firmware"`). Opaque text the
    /// app writes and reads; core never looks inside. Missing or unreadable: the default.
    #[serde(default)]
    pub layouts: std::collections::BTreeMap<String, String>,
    /// Damage glitches: on, reduced or off.
    #[serde(default)]
    pub glitches: crate::glitch::Glitches,
    /// Desktop add-ons bought (ids in `data/hub/desktops.ron`).
    #[serde(default)]
    pub addons: Vec<String>,
    /// The tiling desktop's tiles and workspaces.
    #[serde(default)]
    pub tiling: crate::desktop::Tiling,
    /// The tiling desktop's key help has been seen (it shows on first boot).
    #[serde(default)]
    pub tiling_help_seen: bool,
    /// Secondary panes bought (ids in `data/hub/panes.ron`).
    #[serde(default)]
    pub panes: Vec<String>,
    /// The run terminal's command history, oldest first (capped at `command::HISTORY_KEEP`).
    #[serde(default)]
    pub term_history: Vec<String>,
    /// The home Shell's command history, oldest first (capped at `command::HISTORY_KEEP`).
    #[serde(default)]
    pub shell_history: Vec<String>,
    /// Desktop tools downloaded from boards (ids in `data/hub/tools.ron`). A save from before
    /// tools had to be found has the wardialer and HexEd already.
    #[serde(default = "legacy_tools")]
    pub tools: Vec<String>,
    /// The copies of games you own (ids in `data/hub/games.ron`): one entry per game and copy.
    #[serde(default)]
    pub games: Vec<OwnedGame>,
    /// The tunes you own (ids in `data/music/`), in the player's playlist order: what you came
    /// by, in the order you came by it, unless you've moved them.
    #[serde(default)]
    pub tunes: Vec<String>,
    /// The music player's settings: the tune it's on, repeat and shuffle.
    #[serde(default)]
    pub player: crate::player::PlayerState,
    /// How many rounds of each game have started, by game id: each new round deals a new
    /// field ([`crate::games::new_game`]), and the same save and round replay the same game.
    #[serde(default)]
    pub game_rounds: std::collections::BTreeMap<String, u64>,
    /// Textfiles downloaded from boards, for the QWK reader.
    #[serde(default)]
    pub texts: Vec<KeptText>,
    /// Every ICE you've fought, for the ICE database. It fills in whether or not you own the
    /// database, and never changes play.
    #[serde(default)]
    pub ice_seen: Vec<IceSeen>,
    /// The ICE kinds the sysops have patched: each one's version in force and its program,
    /// what every net runs from the next fight with that kind. A kind not here runs 1.0.0, its row in
    /// `ice.ron`.
    #[serde(default)]
    pub ice_versions: Vec<IceVer>,
    /// Each beaten kind's sysops, at work on its next version or resting
    /// ([`crate::ice_study`]), in the order the deck first beat them.
    #[serde(default)]
    pub ice_study: Vec<crate::ice_study::Study>,
    /// Programs open with their cracktros (`data/intros/`). On unless switched off in System.
    #[serde(default = "default_intros")]
    pub intros: bool,
    /// The Welcome to the Net stack has opened (on a new save's first boot). Old saves have
    /// seen it: they know the Net.
    #[serde(default = "seen")]
    pub guide_seen: bool,
    /// The starter networks have been handed out (by reading Sense/Net's partner directory).
    #[serde(default)]
    pub starters_given: bool,
    /// The starting kit this deck began with (an id in `data/hub/starts.ron`; empty is the
    /// default kit). Its free programs stay free.
    #[serde(default)]
    pub kit: String,
    /// The kit has been applied (a new save's first boot). Old saves have started.
    #[serde(default = "seen")]
    pub started: bool,
    /// The deck's program (BASIC text), as the last fight left it. `None`: the one the
    /// starting kit shipped.
    #[serde(default)]
    pub program: Option<String>,
    /// Intros that have played at least once (ids in `data/intros/`), so a replay teaches
    /// nothing new.
    #[serde(default)]
    pub intros_seen: Vec<String>,
}

fn seen() -> bool {
    true
}

fn default_intros() -> bool {
    true
}

/// What you know about one ICE: by id in `data/ice.ron`.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct IceSeen {
    pub id: String,
    pub met: u32,
    pub broken: u32,
    /// The weakest and strongest you've met it at.
    pub lo: u32,
    pub hi: u32,
    /// The lines of its program you've seen it run, sorted: what the ICE DB decompiles.
    #[serde(default)]
    pub lines: Vec<u32>,
    /// The version of its kind `lines` were seen on: the last one met. Seen lines belong to a
    /// version: a new one is met knowing nothing, since its line 20 is not the last one's.
    #[serde(default)]
    pub version: crate::ice_version::Version,
    /// How `version` was made from the one before (`None` for 1.0.0).
    #[serde(default)]
    pub bump: Option<crate::ice_version::Bump>,
    /// `version`'s program, as it ran; empty for 1.0.0, the row's.
    #[serde(default)]
    pub program: String,
    /// Fights against `version` won and lost (it ended the run).
    #[serde(default)]
    pub won: u32,
    #[serde(default)]
    pub lost: u32,
    /// The versions met before `version`, oldest first, each kept as it was known.
    #[serde(default)]
    pub past: Vec<SeenVersion>,
}

/// A version of an ICE met before the last one, as it was known when a newer one was met.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SeenVersion {
    pub version: crate::ice_version::Version,
    pub bump: Option<crate::ice_version::Bump>,
    /// Its program; empty for 1.0.0, the row's.
    pub program: String,
    pub lines: Vec<u32>,
    pub won: u32,
    pub lost: u32,
}

impl IceSeen {
    /// Whether this entry is of `version` running `program` (empty for 1.0.0, the row's). A
    /// version is its number and its program: when the data changes under a save, a saved
    /// version can fall back to 1.0.0 and the next shipped can take a number met before.
    pub fn is(&self, version: crate::ice_version::Version, program: &str) -> bool {
        self.version == version && self.program == program
    }

    /// Meets `version` (made by `bump`, running `program`): a version other than the last one
    /// met (by number or program, older or newer) puts that one in `past` as it was known and
    /// starts this one with nothing seen.
    pub fn meet(
        &mut self,
        version: crate::ice_version::Version,
        bump: Option<crate::ice_version::Bump>,
        program: &str,
    ) {
        if self.is(version, program) {
            return;
        }
        self.past.push(SeenVersion {
            version: self.version,
            bump: self.bump,
            program: std::mem::take(&mut self.program),
            lines: std::mem::take(&mut self.lines),
            won: std::mem::take(&mut self.won),
            lost: std::mem::take(&mut self.lost),
        });
        self.version = version;
        self.bump = bump;
        self.program = program.to_string();
    }
}

/// The first item of `v` that `found` picks, or `make()` pushed onto the end when there's none.
fn find_or_push<T>(v: &mut Vec<T>, found: impl Fn(&T) -> bool, make: impl FnOnce() -> T) -> &mut T {
    let i = match v.iter().position(found) {
        Some(i) => i,
        None => {
            v.push(make());
            v.len() - 1
        }
    };
    &mut v[i]
}

/// Whether two deck programs are the same play: the same lines, each with the same text, or
/// garbled in one of them (a SEGFAULT's garbage comes home with the save until it's retyped, and
/// a player who retypes it hasn't changed their play).
pub fn same_play(a: &str, b: &str) -> bool {
    let (a, b) = (crate::basic::parse(a), crate::basic::parse(b));
    a.stray == b.stray
        && a.lines.len() == b.lines.len()
        && a.lines.iter().zip(&b.lines).all(|(x, y)| {
            x.no == y.no
                && (x.text == y.text
                    || crate::basic::garbled(&x.text)
                    || crate::basic::garbled(&y.text))
        })
}

/// The version of ICE kind `id` in force in `versions` and its program, or 1.0.0 and the row's
/// own ([`Save::ice_program`]).
fn in_force<'a>(
    versions: &'a [IceVer],
    id: &str,
    data: &'a Data,
) -> Option<(crate::ice_version::Version, &'a str)> {
    let row = data.ice.iter().find(|i| i.id == id)?;
    Some(
        match versions
            .iter()
            .find(|v| v.id == id && Save::in_force(v, row, data))
        {
            Some(v) => (v.version, v.program.as_str()),
            None => (crate::ice_version::Version::default(), row.program.as_str()),
        },
    )
}

/// Version `v` of `row`, running `program`, as a fight books it (`Bout::program`): empty for
/// 1.0.0, the row's, else the program as it parses.
fn as_met(row: &crate::data::IceDef, v: crate::ice_version::Version, program: &str) -> String {
    if v == crate::ice_version::Version::FIRST {
        String::new()
    } else {
        crate::basic::parse_with(program, crate::basic::Vocab::Ice(&row.verbs)).text()
    }
}

/// Puts `next`, the version after `was` of kind `id`, in force in `versions`: the latest
/// shipped goes last, which is the order the boards hear of them.
pub fn ship(
    versions: &mut Vec<IceVer>,
    id: &str,
    was: crate::ice_version::Version,
    next: crate::ice_version::Next,
) {
    let Some(bump) = next.bump else { return };
    versions.retain(|v| v.id != id);
    versions.push(IceVer {
        id: id.to_string(),
        version: was.bumped(bump),
        program: next.program,
        bump: Some(bump),
    });
}

/// A kind's version in force and the program it runs.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct IceVer {
    /// By id in `data/ice.ron`.
    pub id: String,
    pub version: crate::ice_version::Version,
    pub program: String,
    /// How it was made from the version before.
    #[serde(default)]
    pub bump: Option<crate::ice_version::Bump>,
}

/// A textfile on your disk: which board it came from and its id in that board's `texts`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct KeptText {
    pub board: String,
    pub id: String,
}

fn legacy_tools() -> Vec<String> {
    vec!["wardial".into(), "hexed".into()]
}

/// A BBS whose number you know, and how you found it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct KnownBoard {
    pub id: String,
    pub source: String,
}

/// A contract taken from a board at home. It rides into the next run on `entry`'s network,
/// where it gets a target node.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PendingJob {
    pub entry: u32,
    pub client: String,
    /// With `{host}` still in it: the node isn't known until the network is.
    pub brief: String,
    pub reward: u32,
    pub ghost: Option<(u32, u32)>,
}

fn default_phosphor() -> String {
    DEFAULT_RUN_THEME.into()
}

fn default_crt() -> bool {
    true
}

fn default_sound() -> bool {
    true
}

fn default_volume() -> u8 {
    100
}

/// A level in percent as the nearest notch of a control with `steps` notches above silent (a
/// slider of 0-7, ten meter cells).
pub fn volume_notch(pct: u32, steps: u32) -> u32 {
    (pct.min(100) * steps + 50) / 100
}

/// The level in percent at notch `notch` of `steps` (past the top is full).
pub fn notch_volume(notch: u32, steps: u32) -> u32 {
    let steps = steps.max(1);
    (notch.min(steps) * 100 + steps / 2) / steps
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
            sound: default_sound(),
            volume: default_volume(),
            bank: 0,
            fragments: 0,
            key: 0,
            rig: Rig::default(),
            stash: Vec::new(),
            next_item: 0,
            loadout: Loadout::default(),
            disk_map: Vec::new(),
            phonebook: Vec::new(),
            next_entry: 0,
            ranges: local_range(),
            scanned: Vec::new(),
            finds: Vec::new(),
            telco_heat: 0,
            trust: Vec::new(),
            boards: Vec::new(),
            home_bought: Vec::new(),
            home_taken: Vec::new(),
            pending: Vec::new(),
            monitors: Vec::new(),
            monitor: String::new(),
            decks: Vec::new(),
            deck: String::new(),
            firmware: String::new(),
            layouts: std::collections::BTreeMap::new(),
            glitches: crate::glitch::Glitches::On,
            addons: Vec::new(),
            tiling: crate::desktop::Tiling::default(),
            tiling_help_seen: false,
            panes: Vec::new(),
            term_history: Vec::new(),
            shell_history: Vec::new(),
            tools: Vec::new(),
            games: Vec::new(),
            tunes: Vec::new(),
            player: Default::default(),
            game_rounds: Default::default(),
            texts: Vec::new(),
            ice_seen: Vec::new(),
            ice_versions: Vec::new(),
            ice_study: Vec::new(),
            intros: default_intros(),
            guide_seen: false,
            starters_given: false,
            kit: String::new(),
            started: false,
            program: None,
            intros_seen: Vec::new(),
        }
    }
}

#[derive(Deserialize)]
struct VersionProbe {
    version: u32,
}

impl Save {
    /// Sets how loud everything plays, in percent (past 100 is full).
    pub fn set_volume(&mut self, pct: u32) {
        self.volume = pct.min(100) as u8;
    }

    /// The level as a gain for the speaker: 0 (silent) to 1 (full).
    pub fn volume_gain(&self) -> f32 {
        f32::from(self.volume.min(100)) / 100.0
    }

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
    /// Whether you own a desktop tool (an id in `hub/tools.ron`).
    pub fn has_tool(&self, id: &str) -> bool {
        self.tools.iter().any(|t| t == id)
    }

    /// Puts a board's textfile on your disk. False if it was already there.
    pub fn keep_text(&mut self, board: &str, id: &str) -> bool {
        if self.texts.iter().any(|k| k.board == board && k.id == id) {
            return false;
        }
        self.texts.push(KeptText {
            board: board.into(),
            id: id.into(),
        });
        true
    }

    /// Adds a tool you downloaded. False if you had it.
    pub fn learn_tool(&mut self, id: &str) -> bool {
        if self.has_tool(id) {
            return false;
        }
        self.tools.push(id.into());
        true
    }

    /// Whether you own a game, in either copy (an id in `hub/games.ron`).
    pub fn has_game(&self, id: &str) -> bool {
        self.games.iter().any(|g| g.id == id)
    }

    /// Whether you have this copy of a game.
    pub fn has_copy(&self, id: &str, copy: GameCopy) -> bool {
        self.games.iter().any(|g| g.id == id && g.copy == copy)
    }

    /// The copies of a game you own, in the order you came by them (empty if none).
    pub fn copies(&self, id: &str) -> Vec<GameCopy> {
        self.games
            .iter()
            .filter(|g| g.id == id)
            .map(|g| g.copy)
            .collect()
    }

    /// Adds a copy of a game you came by, beside any other copy you have. False if you had
    /// that copy already.
    pub fn learn_game(&mut self, id: &str, copy: GameCopy) -> bool {
        if self.has_copy(id, copy) {
            return false;
        }
        self.games.push(OwnedGame {
            id: id.into(),
            copy,
        });
        true
    }

    /// Whether you own a tune (an id in `data/music/`).
    pub fn has_tune(&self, id: &str) -> bool {
        self.tunes.iter().any(|t| t == id)
    }

    /// Adds a tune you came by to the end of the playlist. False if you had it.
    pub fn learn_tune(&mut self, id: &str) -> bool {
        if self.has_tune(id) {
            return false;
        }
        self.tunes.push(id.into());
        true
    }

    /// The games you have a cracked copy of: a board won't sell you one of these again.
    pub fn cracks(&self) -> Vec<String> {
        self.games
            .iter()
            .filter(|g| g.copy == GameCopy::Cracked)
            .map(|g| g.id.clone())
            .collect()
    }

    /// Whether a desktop tool is there at all: HexEd only once you've downloaded it.
    pub fn opens(&self, tool: Tool) -> bool {
        match tool {
            Tool::HexEd => self.has_tool("hexed"),
            Tool::Qwk => self.has_tool("qwkread"),
            Tool::IceDb => self.has_tool("icedb"),
            Tool::NightComm => self.has_tool("nightcomm"),
            Tool::TigerTeam => self.has_tool("tigerteam"),
            Tool::Player => self.has_tool("player"),
            Tool::Game(id, copy) => self.has_copy(id.as_str(), copy),
            _ => true,
        }
    }

    /// The desktop's icons: the tools you have, then each copy of a game you own, labelled as
    /// that copy is (`Nibbler`, `NIBBLER [2K4]`) with the game's own art.
    pub fn icons<'a>(&self, data: &'a Data) -> Vec<Icon<'a>> {
        let tools = data
            .icons
            .iter()
            .filter(|i| self.opens(i.tool))
            .map(|i| Icon {
                label: std::borrow::Cow::Borrowed(i.label.as_str()),
                tool: i.tool,
                pixels: &i.pixels,
            });
        let games = Tool::games(data, self).into_iter().filter_map(|t| {
            let (g, _) = t.game_def(data)?;
            Some(Icon {
                label: std::borrow::Cow::Owned(t.name(data)),
                tool: t,
                pixels: &g.icon,
            })
        });
        tools.chain(games).collect()
    }

    /// The round the next game of `id` deals (0 for the first).
    pub fn game_round(&self, id: &str) -> u64 {
        self.game_rounds.get(id).copied().unwrap_or(0)
    }

    /// Starts a round of game `id`: returns the round it deals, and the next one deals anew.
    pub fn start_game(&mut self, id: &str) -> u64 {
        let n = self.game_rounds.entry(id.to_string()).or_insert(0);
        let round = *n;
        *n += 1;
        round
    }

    pub fn ensure_key(&mut self, fresh: u64) -> bool {
        if self.key == 0 {
            self.key = fresh | 1;
            return true;
        }
        false
    }

    /// A new deck's first boot: Sense/Net's customer board, the networks every deck starts with,
    /// and the starting kit (`self.kit`, or the default). Once only. Returns true if it did.
    pub fn ensure_phonebook(&mut self, data: &Data, _seed: u64) -> bool {
        if self.started {
            return false;
        }
        self.started = true;
        let kit = data.starts.kit(&self.kit).clone();
        self.kit = kit.id.clone();
        let came = "came with your deck";
        if data.board(FIRST_BOARD).is_some() {
            self.learn_board(FIRST_BOARD, came);
        }
        for n in &data.starts.always {
            if self.phonebook.iter().any(|e| e.number == n.number) {
                continue;
            }
            let id = self.next_entry;
            self.next_entry += 1;
            self.phonebook.push(Entry {
                id,
                name: n.name.clone(),
                business: n.business.clone(),
                number: n.number.clone(),
                tier: n.tier,
                traits: n.traits.clone(),
                heat: 0,
                runs: 0,
                last: Vec::new(),
            });
        }
        for b in &kit.boards {
            self.learn_board(b, came);
        }
        if kit.numbers {
            self.add_starters(data);
        }
        for t in &kit.tools {
            self.learn_tool(t);
        }
        for r in &kit.ranges {
            self.learn_range(r, came);
        }
        for (g, copy) in &kit.games {
            self.learn_game(g, *copy);
        }
        for t in &kit.tunes {
            self.learn_tune(t);
        }
        self.bank += kit.bank;
        self.loadout.starters = kit.free.clone();
        true
    }

    /// The programs this deck shipped with: free, and never lost (its kit's `free`).
    pub fn free<'a>(&self, data: &'a Data) -> &'a [String] {
        &data.starts.kit(&self.kit).free
    }

    /// The starter networks (`PHONEBOOK_START`), handed out once: Sense/Net's partner
    /// directory reads them out. Seeded from the save, so a save always gets the same ones.
    /// Returns their names.
    pub fn add_starters(&mut self, data: &Data) -> Vec<String> {
        if self.starters_given {
            return Vec::new();
        }
        self.starters_given = true;
        let mut rng = Rng::new(self.key ^ 0x5EED_B00C);
        let ids: Vec<u32> = PHONEBOOK_START
            .iter()
            .filter_map(|&tier| self.add_number(data, &mut rng, tier))
            .collect();
        ids.into_iter()
            .filter_map(|id| self.entry(id).map(|e| format!("{} ({})", e.name, e.number)))
            .collect()
    }

    /// An intro has played. The first time, a crack's scroller teaches the board it advertises
    /// (a splash advertises none). True if that put a new board in the phonebook.
    pub fn saw_intro(&mut self, data: &Data, id: &str) -> bool {
        if self.intros_seen.iter().any(|s| s == id) {
            return false;
        }
        let Some(intro) = data.intro(id) else {
            return false;
        };
        self.intros_seen.push(id.into());
        match intro.board.as_deref() {
            Some(b) if data.board(b).is_some() => self.learn_board(b, "a cracktro"),
            _ => false,
        }
    }

    /// What reading `effect` on `source` does for the save. Returns what it added, if anything.
    pub fn read_effect(
        &mut self,
        data: &Data,
        effect: &crate::data::ReadEffect,
        source: &str,
    ) -> Option<String> {
        use crate::data::ReadEffect;
        match effect {
            ReadEffect::Board(id) => {
                let b = &data.boards[data.board(id)?];
                self.learn_board(id, &format!("a message on {source}"))
                    .then(|| format!("New BBS in CarrierTerm: {} ({})", b.name, b.phone))
            }
            ReadEffect::Starters => {
                let names = self.add_starters(data);
                (!names.is_empty())
                    .then(|| format!("New numbers in CarrierTerm: {}", names.join(", ")))
            }
            ReadEffect::Hint(r) => self
                .learn_range(r, &format!("a post on {source}"))
                .then(|| format!("new numbers to wardial: {r}")),
        }
    }

    /// Adds a fresh number of `tier` with a company not already in the book.
    pub fn add_number(&mut self, data: &Data, rng: &mut Rng, tier: u32) -> Option<u32> {
        self.add_entry(data, rng, tier, None, None)
    }

    /// Adds an entry. `owner` pins the company (a wardialed corp range); `number` pins the number.
    pub fn add_entry(
        &mut self,
        data: &Data,
        rng: &mut Rng,
        tier: u32,
        owner: Option<&str>,
        number: Option<String>,
    ) -> Option<u32> {
        let nets = &data.networks;
        let company = match owner {
            Some(name) => nets.companies.iter().find(|c| c.name == name)?,
            None => {
                let free: Vec<&CompanyDef> = nets
                    .companies
                    .iter()
                    .filter(|c| !self.phonebook.iter().any(|e| e.name == c.name))
                    .collect();
                let pool: Vec<&CompanyDef> = if free.is_empty() {
                    nets.companies.iter().collect()
                } else {
                    free
                };
                pool[rng.below(pool.len())]
            }
        };
        let number = match number {
            Some(n) if !self.phonebook.iter().any(|e| e.number == n) => n,
            Some(_) => return None,
            None => loop {
                let n = format!("555-{:04}", rng.range(0, 9999));
                if !self.phonebook.iter().any(|e| e.number == n) {
                    break n;
                }
            },
        };
        let (lo, hi) = nets.tier(tier).map_or((0, 0), |t| t.traits);
        let want = rng.range(lo, hi) as usize;
        let mut pool: Vec<&TraitDef> = nets.traits.iter().collect();
        let mut traits = Vec::new();
        while traits.len() < want && !pool.is_empty() {
            let t = pool.swap_remove(rng.below(pool.len()));
            // One ICE-class bias and one trace shift per network, or they'd cancel out.
            let clash = |e: &TraitEffect| {
                traits.iter().any(|id: &String| {
                    nets.trait_def(id).is_some_and(|d| {
                        std::mem::discriminant(&d.effect) == std::mem::discriminant(e)
                    })
                })
            };
            if !clash(&t.effect) {
                traits.push(t.id.clone());
            }
        }
        let id = self.next_entry;
        self.next_entry += 1;
        self.phonebook.push(Entry {
            id,
            name: company.name.clone(),
            business: company.business.clone(),
            number,
            tier,
            traits,
            heat: 0,
            runs: 0,
            last: Vec::new(),
        });
        Some(id)
    }

    pub fn entry(&self, id: u32) -> Option<&Entry> {
        self.phonebook.iter().find(|e| e.id == id)
    }

    /// Finds an entry by its 1-based position in the book or by its number.
    pub fn lookup(&self, what: &str) -> Option<&Entry> {
        if let Ok(n) = what.parse::<usize>()
            && n >= 1
        {
            return self.phonebook.get(n - 1);
        }
        self.phonebook.iter().find(|e| e.number == what)
    }

    /// The network shape and run modifiers an entry dials into, heat included.
    pub fn site_params(data: &Data, e: &Entry) -> (NetParams, RunMods) {
        let nets = &data.networks;
        let tier = nets.tier(e.tier);
        let mut p = NetParams {
            mid_layers: tier.map_or(NET_MID_LAYERS, |t| t.mid_layers),
            subnets: tier.map_or(0, |t| t.subnets),
            black_bonus: tier.map_or(0, |t| t.black),
            ..NetParams::default()
        };
        p.shape.choke = tier.map_or(0, |t| t.choke);
        let mut m = RunMods {
            ice_bonus: tier.map_or(0, |t| t.ice_bonus) + HEAT_ICE * e.heat,
            trace_pct: tier.map_or(100, |t| t.trace_pct) + HEAT_TRACE_PCT * e.heat,
            pay_pct: tier.map_or(100, |t| t.pay_pct),
        };
        for id in &e.traits {
            match nets.trait_def(id).map(|d| d.effect.clone()) {
                Some(TraitEffect::IceClass(c)) => p.ice_bias = Some(c),
                Some(TraitEffect::BlackIce(n)) => p.black_bonus += n,
                Some(TraitEffect::NoBbs) => p.bbs = false,
                Some(TraitEffect::Trace(pct)) => m.trace_pct = m.trace_pct * pct / 100,
                Some(TraitEffect::Chatty(n)) => p.contact_bonus += n,
                Some(TraitEffect::VaultRich(n)) => p.vault_bonus += n,
                Some(TraitEffect::Pay(pct)) => m.pay_pct = m.pay_pct * pct / 100,
                Some(TraitEffect::Shape(shape)) => p.shape = shape,
                Some(TraitEffect::Subnets(n)) => p.subnets += n,
                Some(TraitEffect::Board(_)) => {}
                None => {}
            }
        }
        (p, m)
    }

    /// Which BBS an entry's network runs: a trait's `Board` if it has one, else one of the
    /// boards for its tier, picked by the phone number so a number always reaches the same
    /// board. Falls back to the default board.
    pub fn site_board(data: &Data, e: &Entry) -> usize {
        let forced =
            e.traits
                .iter()
                .find_map(|id| match data.networks.trait_def(id).map(|d| &d.effect) {
                    Some(TraitEffect::Board(b)) => data.board(b),
                    _ => None,
                });
        if let Some(b) = forced {
            return b;
        }
        let fits: Vec<usize> = (0..data.boards.len())
            .filter(|&i| data.boards[i].tiers.contains(&e.tier))
            .collect();
        if fits.is_empty() {
            return 0;
        }
        let h = e.number.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
        });
        fits[(h % fits.len() as u64) as usize]
    }

    /// Dials a phonebook entry: a run on that network with your loadout and RIG. An
    /// `Arc<Data>` is shared with the run; a plain `Data` is copied into it.
    pub fn dial(
        &mut self,
        content: &impl crate::data::Shared,
        entry: u32,
        seed: u64,
    ) -> Result<Game, String> {
        let data = content.data();
        let e = self
            .entry(entry)
            .ok_or("no such number in the phonebook.")?
            .clone();
        let (net, mut mods) = Self::site_params(data, &e);
        // Tripped traps mean the telco is listening on this call.
        mods.trace_pct += TRAP_TRACE_PCT * self.telco_heat;
        self.telco_heat = 0;
        let mut kit = self.kit(data);
        kit.net = net;
        kit.mods = mods;
        kit.board = Self::site_board(data, &e);
        // Jobs taken at home for this number come along. Unfinished, they're gone with the run.
        let (mine, rest) = std::mem::take(&mut self.pending)
            .into_iter()
            .partition(|j| j.entry == e.id);
        self.pending = rest;
        kit.jobs = mine;
        let labels = e
            .traits
            .iter()
            .filter_map(|t| data.networks.trait_def(t).map(|d| d.label.clone()))
            .collect();
        kit.site = Some(Site {
            entry: e.id,
            name: e.name.clone(),
            tier: e.tier,
            traits: labels,
        });
        // Stash programs on the disk leave the stash for the run; the loadout remembers them.
        let taken: Vec<u32> = kit.origins.iter().flatten().copied().collect();
        self.stash.retain(|s| !taken.contains(&s.id));
        Ok(Game::start(content.share(), seed, &kit))
    }

    /// Heat after a run on `entry`: it heats up, every other number cools down.
    fn heat_up(&mut self, entry: u32, end: RunEnd) {
        for e in &mut self.phonebook {
            if e.id == entry {
                e.runs += 1;
                e.heat = (e.heat + HEAT_PER_RUN).min(HEAT_MAX);
                e.last.push(end);
                let extra = e.last.len().saturating_sub(RESULTS_KEPT);
                e.last.drain(..extra);
            } else {
                e.heat = e.heat.saturating_sub(HEAT_COOL);
            }
        }
    }

    /// Whether this deck can wardial: WARDIAL does, and NiGHTCOMM has its own dialer.
    pub fn can_wardial(&self) -> bool {
        self.has_tool("wardial") || self.has_dialer()
    }

    /// Whether this deck has a dashboard with its own modem terminal: NiGHTCOMM, or Tiger Team
    /// Pro, which does everything NiGHTCOMM does.
    pub fn has_dialer(&self) -> bool {
        self.has_tool("nightcomm") || self.has_tool("tigerteam")
    }

    pub fn knows_range(&self, id: &str) -> bool {
        self.ranges.iter().any(|r| r.id == id)
    }

    /// Learns a range. Returns false if it was already known.
    pub fn learn_range(&mut self, id: &str, source: &str) -> bool {
        if self.knows_range(id) {
            return false;
        }
        self.ranges.push(KnownRange {
            id: id.into(),
            source: source.into(),
        });
        true
    }

    /// Blocks of a range already scanned.
    pub fn scanned_in(&self, range: &str) -> usize {
        let key = format!("{range}:");
        self.scanned.iter().filter(|s| s.starts_with(&key)).count()
    }

    /// Dials one unscanned 100-number block of a known range. The phone bill comes out of the bank.
    /// The same block always answers the same way: a number is what it is.
    pub fn wardial(&mut self, data: &Data, range: &str) -> Result<ScanResult, String> {
        let def = data
            .exchanges
            .iter()
            .find(|r| r.id == range)
            .ok_or("no such range.")?
            .clone();
        if !self.can_wardial() {
            return Err("wardial: unknown command.".into());
        }
        if !self.knows_range(range) {
            return Err(format!("you don't know any {} numbers yet.", def.label));
        }
        let open: Vec<u32> = (0..def.blocks())
            .filter(|b| !self.scanned.contains(&format!("{range}:{b}")))
            .collect();
        if open.is_empty() {
            return Err(format!(
                "{} is tapped out. every number's been dialed.",
                def.pattern()
            ));
        }
        if self.bank < def.cost {
            return Err(format!(
                "the phone bill for a scan is {}¢. you have {}¢.",
                def.cost, self.bank
            ));
        }
        let mut pick = Rng::new(self.key ^ self.scanned.len() as u64 ^ 0xD1A1);
        let block = open[pick.below(open.len())];
        self.bank -= def.cost;
        self.scanned.push(format!("{range}:{block}"));
        let cells = self.dial_block(data, &def, block);
        Ok(ScanResult {
            range: range.into(),
            block,
            cells,
            cost: def.cost,
        })
    }

    /// Dials every number in one block of a range, booking what answers: new carriers into
    /// the phonebook, boards learned, traps on the telco's list, finds kept. The same block
    /// always answers the same way for a save.
    fn dial_block(&mut self, data: &Data, def: &RangeDef, block: u32) -> Vec<(String, Answer)> {
        let range = def.id.as_str();
        let id_hash = range
            .bytes()
            .fold(0u64, |h, b| h.wrapping_mul(31).wrapping_add(u64::from(b)));
        let mut rng = Rng::new(
            self.key ^ id_hash.rotate_left(17) ^ u64::from(block).wrapping_mul(0x9E37_79B9),
        );
        let mut cells = Vec::new();
        for number in def.numbers(block) {
            let roll = rng.range(1, 100);
            // A board always picks up on its own number, so scanning its block finds it.
            let board = data.boards.iter().position(|b| b.phone == number);
            let answer = if let Some(board) = board {
                let new =
                    self.learn_board(&data.boards[board].id, &format!("wardialing {}", def.label));
                Answer::Board { board, new }
            } else if roll <= def.carrier_pct {
                // Every carrier takes the same draws, known or not, so a number you already
                // have never changes how the rest of its block answers. A new one gets its
                // company from a stream of its own.
                let w = [def.tiers.0, def.tiers.1, def.tiers.2];
                let tier = rng.weighted(w).map_or(1, |i| i as u32 + 1);
                let mut own = Rng::new(rng.next_u64());
                if let Some(e) = self.phonebook.iter().find(|e| e.number == number) {
                    Answer::Carrier {
                        entry: e.id,
                        new: false,
                    }
                } else {
                    match self.add_entry(
                        data,
                        &mut own,
                        tier,
                        def.owner.as_deref(),
                        Some(number.clone()),
                    ) {
                        Some(entry) => Answer::Carrier { entry, new: true },
                        None => Answer::NoAnswer,
                    }
                }
            } else if roll <= def.carrier_pct + def.trap_pct {
                self.telco_heat += 1;
                Answer::Trap
            } else {
                match rng.range(1, 100) {
                    1..=6 => Answer::Fax,
                    7..=22 => Answer::Busy,
                    23..=55 => Answer::NoAnswer,
                    _ => Answer::Voice,
                }
            };
            let what = match &answer {
                Answer::Carrier { entry, .. } => Some(Found::Network(*entry)),
                Answer::Board { board, .. } => Some(Found::Board(data.boards[*board].id.clone())),
                Answer::Trap => Some(Found::Watched),
                _ => None,
            };
            if let Some(what) = what {
                self.finds.push(Find {
                    range: range.into(),
                    block,
                    number: number.clone(),
                    what,
                });
            }
            cells.push((number, answer));
        }
        cells
    }

    /// The known range a number is best dialed through: the narrowest one it falls in.
    pub fn range_of<'a>(&self, data: &'a Data, number: &str) -> Option<&'a RangeDef> {
        data.exchanges
            .iter()
            .filter(|r| self.knows_range(&r.id) && number.starts_with(&r.prefix))
            .max_by_key(|r| r.prefix.len())
    }

    /// What one call costs: the range's scan price shared over a block's numbers.
    pub fn call_price(def: &RangeDef) -> u32 {
        def.cost.div_ceil(WARDIAL_BLOCK).max(1)
    }

    /// Dials one number by hand (NiGHTCOMM's TERM): a real call, billed like a share of a
    /// scan, with the same trap risk. It answers the way its block would in a scan, so a
    /// number never tells TERM one thing and WARDiAL another.
    pub fn atdt(&mut self, data: &Data, number: &str) -> Result<Answer, String> {
        if !self.has_dialer() {
            return Err("atdt: unknown command.".into());
        }
        let def = self
            .range_of(data, number)
            .ok_or_else(|| format!("{number}: no range you know covers it."))?
            .clone();
        let price = Save::call_price(&def);
        if self.bank < price {
            return Err(format!("a call is {price}¢. you have {}¢.", self.bank));
        }
        let index: u32 = number[def.prefix.len()..]
            .parse()
            .map_err(|_| format!("{number} isn't a number."))?;
        let block = index / WARDIAL_BLOCK;
        // Dial the whole block on a copy, then keep only this number's answer.
        let mut sim = self.clone();
        let answer = sim
            .dial_block(data, &def, block)
            .into_iter()
            .find(|(n, _)| n == number)
            .map(|(_, a)| a)
            .ok_or_else(|| format!("{number} isn't in {}.", def.pattern()))?;
        self.bank -= price;
        let (found, answer) = match answer {
            Answer::Carrier { entry, new: false } => (Found::Network(entry), answer),
            Answer::Carrier { entry, new: true } => {
                let mut e = sim
                    .phonebook
                    .iter()
                    .find(|e| e.id == entry)
                    .cloned()
                    .ok_or("the line dropped.")?;
                e.id = self.next_entry;
                self.next_entry += 1;
                let id = e.id;
                self.phonebook.push(e);
                (
                    Found::Network(id),
                    Answer::Carrier {
                        entry: id,
                        new: true,
                    },
                )
            }
            Answer::Board { board, .. } => {
                let new = self.learn_board(&data.boards[board].id, &format!("dialing {number}"));
                (
                    Found::Board(data.boards[board].id.clone()),
                    Answer::Board { board, new },
                )
            }
            Answer::Trap => {
                self.telco_heat += 1;
                (Found::Watched, Answer::Trap)
            }
            other => return Ok(other),
        };
        if !self
            .finds
            .iter()
            .any(|f| f.number == number && f.what == found)
        {
            self.finds.push(Find {
                range: def.id.clone(),
                block,
                number: number.into(),
                what: found,
            });
        }
        Ok(answer)
    }

    /// What the deck plugged in ships with on a line.
    pub fn stat_base(&self, data: &Data, line: Upgrade) -> u32 {
        self.deck_def(data)
            .stat(line)
            .map_or(line.fallback(), |s| s.base)
    }

    /// The most a line can reach on the deck plugged in, or `None` if the deck doesn't cap it.
    pub fn stat_cap(&self, data: &Data, line: Upgrade) -> Option<u32> {
        self.deck_def(data).stat(line).map(|s| s.max)
    }

    /// What a line comes to on the deck plugged in: its base plus RIG, up to the deck's cap.
    pub fn stat_value(&self, data: &Data, line: Upgrade) -> u32 {
        let v = self.stat_base(data, line) + self.rig.bonus(line, &data.rig);
        self.stat_cap(data, line).map_or(v, |c| v.min(c))
    }

    pub fn stash_cap(&self, defs: &[UpgradeDef]) -> u32 {
        Upgrade::Stash.fallback() + self.rig.bonus(Upgrade::Stash, defs)
    }

    /// HDD slots the next run's deck will have: the loadout's limit.
    pub fn hdd_cap(&self, data: &Data) -> u32 {
        self.stat_value(data, Upgrade::Hdd)
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

    /// The cheapest upgrade the deck plugged in can still take, and what it costs.
    pub fn next_upgrade(&self, data: &Data) -> Option<(Upgrade, u32)> {
        data.rig
            .iter()
            .filter(|d| self.upgrade_blocker(data, d.kind).is_none())
            .filter_map(|d| d.cost(self.rig.level(d.kind)).map(|c| (d.kind, c)))
            .min_by_key(|&(_, c)| c)
    }

    /// Why the deck plugged in can't take the next level of an upgrade, if it can't: the
    /// level would take the line past the chassis' ceiling.
    pub fn upgrade_blocker(&self, data: &Data, u: Upgrade) -> Option<String> {
        let def = data.rig.iter().find(|d| d.kind == u)?;
        let cap = self.stat_cap(data, u)?;
        let after = self.stat_base(data, u) + def.step * (self.rig.level(u) + 1);
        if after <= cap {
            return None;
        }
        let unit = if def.unit.is_empty() {
            &def.label
        } else {
            &def.unit
        };
        Some(format!(
            "the {} tops out at {cap} {unit}. a bigger chassis takes more.",
            self.deck_def(data).title()
        ))
    }

    /// Buys the next level of an upgrade with banked credits, if the deck can take it.
    pub fn buy(&mut self, data: &Data, u: Upgrade) -> Result<String, String> {
        let def = data
            .rig
            .iter()
            .find(|d| d.kind == u)
            .ok_or("no such upgrade")?;
        let level = self.rig.level(u);
        let cost = def
            .cost(level)
            .ok_or_else(|| format!("{} is maxed out.", def.label))?;
        if let Some(why) = self.upgrade_blocker(data, u) {
            return Err(why);
        }
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
    pub fn toggle_starter(&mut self, data: &Data, id: &str) -> Result<(), String> {
        if let Some(i) = self.loadout.starters.iter().position(|s| s == id) {
            self.loadout.starters.remove(i);
            return Ok(());
        }
        if !self.free(data).iter().any(|f| f == id) {
            return Err(format!("{id} isn't a starter program."));
        }
        self.room_in_loadout(data)?;
        self.loadout.starters.push(id.to_string());
        Ok(())
    }

    /// Adds or removes a stash program from the loadout.
    pub fn toggle_stash(&mut self, data: &Data, id: u32) -> Result<(), String> {
        if let Some(i) = self.loadout.stash.iter().position(|s| *s == id) {
            self.loadout.stash.remove(i);
            return Ok(());
        }
        if self.item(id).is_none() {
            return Err("no such program in the stash.".into());
        }
        self.room_in_loadout(data)?;
        self.loadout.stash.push(id);
        Ok(())
    }

    fn room_in_loadout(&self, data: &Data) -> Result<(), String> {
        let cap = self.hdd_cap(data) as usize;
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

    /// What the next run starts with: the deck's numbers plus the loadout's programs.
    /// An empty loadout falls back to the starters, so you can never jack in with nothing.
    pub fn kit(&self, data: &Data) -> Kit {
        let mut programs = Vec::new();
        let mut origins = Vec::new();
        // Only the Security Suite is free: a loadout from an older save loses its breakers.
        for id in self
            .loadout
            .starters
            .iter()
            .filter(|id| self.free(data).contains(id))
        {
            if let Some(w) = data.warez_ix(id) {
                programs.push((w, binary::encode(&data.warez[w], self.key)));
                origins.push(None);
            }
        }
        for id in &self.loadout.stash {
            if let Some(item) = self.item(*id)
                && let (Some(w), Some(bin)) = (data.warez_ix(&item.origin), item.bin())
            {
                programs.push((w, bin));
                origins.push(Some(*id));
            }
        }
        programs.truncate(self.hdd_cap(data) as usize);
        origins.truncate(programs.len());
        Kit {
            ram: self.stat_value(data, Upgrade::Ram),
            cpu: self.stat_value(data, Upgrade::Cpu),
            hdd: self.stat_value(data, Upgrade::Hdd),
            integrity: self.stat_value(data, Upgrade::Integrity),
            caps: Caps {
                ram: self.stat_cap(data, Upgrade::Ram),
                cpu: self.stat_cap(data, Upgrade::Cpu),
                hdd: self.stat_cap(data, Upgrade::Hdd),
            },
            programs,
            program: self.program_text(data),
            net: NetParams::default(),
            mods: RunMods::default(),
            site: None,
            board: 0,
            jobs: Vec::new(),
            trust: self.trust.clone(),
            tools: self.tools.clone(),
            games: self.cracks(),
            tunes: self.tunes.clone(),
            free: self.free(data).to_vec(),
            texts: self
                .texts
                .iter()
                .map(|k| (k.board.clone(), k.id.clone()))
                .collect(),
            // What earlier runs saw is the ICE DB's: without it the deck starts each fight blind.
            // Only lines seen on the version in force: a new version starts blind.
            ice_known: self
                .ice_seen
                .iter()
                .filter(|s| self.has_tool("icedb") && !s.lines.is_empty())
                .filter(|s| {
                    let row = data.ice.iter().find(|r| r.id == s.id);
                    let now = self.ice_program(&s.id, data);
                    row.zip(now)
                        .is_some_and(|(row, (v, p))| s.is(v, &as_met(row, v, p)))
                })
                .map(|s| (s.id.clone(), s.lines.clone()))
                .collect(),
            ice_study: self.ice_study.clone(),
            study_key: self.key,
            // The sysops' patches: every net runs a patched kind's version.
            ice_programs: self
                .patches(data)
                .into_iter()
                .map(|v| (v.id.clone(), v.version, v.program.clone(), v.bump))
                .collect(),
            origins,
        }
    }

    /// The version of ICE kind `id` in force and its program: the save's, or 1.0.0 and the
    /// row's own when the sysops haven't patched it. A saved program that no longer passes the
    /// row's check (the row changed under it) falls back to the row's: saves load, never crash.
    /// `None` for an id that isn't in `ice.ron`.
    pub fn ice_program<'a>(
        &'a self,
        id: &str,
        data: &'a Data,
    ) -> Option<(crate::ice_version::Version, &'a str)> {
        in_force(&self.ice_versions, id, data)
    }

    /// The sysops' time at home: `steps` steps of it pass and at most `drafts` owed drafts are
    /// worked ([`crate::ice_study::work`]). What ships is in force on every net from the next
    /// fight with that kind. Returns how many versions shipped.
    pub fn study(&mut self, data: &std::sync::Arc<Data>, steps: u32, drafts: u32) -> usize {
        let versions = &self.ice_versions;
        let shipped =
            crate::ice_study::work(&mut self.ice_study, data, self.key, steps, drafts, |id| {
                in_force(versions, id, data).map(|(v, p)| (v, p.to_string()))
            });
        let n = shipped.len();
        for (id, was, next) in shipped {
            ship(&mut self.ice_versions, &id, was, next);
        }
        n
    }

    /// Whether saved version `v` of `row` is in force: past 1.0.0 (which is always the row's
    /// own program) and passing the row's check.
    fn in_force(v: &IceVer, row: &crate::data::IceDef, data: &Data) -> bool {
        v.version > crate::ice_version::Version::FIRST
            && data.check_ice_program(row, &v.program).is_ok()
    }

    /// The versions in force as the boards heard of them, in the order they shipped (the order
    /// `ice_versions` keeps): (index into `Data::ice`, version, how it was made). A version with
    /// no bump (set by hand) isn't news.
    pub fn shipped(
        &self,
        data: &Data,
    ) -> Vec<(usize, crate::ice_version::Version, crate::ice_version::Bump)> {
        self.patches(data)
            .into_iter()
            .filter_map(|v| Some((data.ice_by_id(&v.id)?, v.version, v.bump?)))
            .collect()
    }

    /// The saved versions in force, the first for each kind ([`Save::ice_program`]'s), each
    /// checked once.
    fn patches<'a>(&'a self, data: &Data) -> Vec<&'a IceVer> {
        let mut out: Vec<&IceVer> = Vec::new();
        for v in &self.ice_versions {
            if out.iter().any(|o| o.id == v.id) {
                continue;
            }
            if let Some(row) = data.ice.iter().find(|r| r.id == v.id)
                && Self::in_force(v, row, data)
            {
                out.push(v);
            }
        }
        out
    }

    /// The deck's program: as the last run left it, or the one its kit shipped.
    pub fn program_text(&self, data: &Data) -> String {
        self.program
            .clone()
            .unwrap_or_else(|| data.starts.program(&self.kit).to_string())
    }

    /// Starts a run. Stash programs in the loadout leave the stash: they come back only
    /// if you reach the exit and keep them. The loadout itself is remembered: a program that
    /// comes home goes back onto the disk in its place.
    pub fn jack_in(&mut self, content: &impl crate::data::Shared, seed: u64) -> Game {
        let kit = self.kit(content.data());
        let taken: Vec<u32> = kit.origins.iter().flatten().copied().collect();
        self.stash.retain(|s| !taken.contains(&s.id));
        Game::start(content.share(), seed, &kit)
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
        // What you leave in the program is what the deck keeps, however the run ended.
        self.program = Some(game.program.text());
        if let Some(site) = &game.site {
            self.heat_up(site.entry, end);
            // A client sold the number to the telco.
            if let Some(e) = self.phonebook.iter_mut().find(|e| e.id == site.entry) {
                e.heat = (e.heat + game.sold_out).min(HEAT_MAX);
            }
        }
        for k in &game.contracts {
            let clean = match k.status {
                ContractStatus::Done => true,
                ContractStatus::Betrayed => false,
                _ => continue,
            };
            match self.trust.iter_mut().find(|(c, _)| *c == k.client) {
                Some((_, n)) if clean => *n += 1,
                Some((_, n)) => *n = 0,
                None => self.trust.push((k.client.clone(), u32::from(clean))),
            }
        }
        // What you learned stays in your head, deck or no deck.
        let source = game
            .site
            .as_ref()
            .map_or("a run".to_string(), |s| s.name.clone());
        let learned: Vec<String> = game
            .hints
            .iter()
            .filter(|h| self.learn_range(h, &format!("found on {source}")))
            .cloned()
            .collect();
        let tools: Vec<String> = game
            .tools
            .iter()
            .filter(|t| self.learn_tool(t))
            .map(|t| game.data.tool(t).map_or(t.clone(), |d| d.name.clone()))
            .collect();
        // A game pulled off a board is a cracked copy, and it comes home even on a flatline.
        let games: Vec<String> = game
            .games
            .iter()
            .filter(|g| self.learn_game(g, GameCopy::Cracked))
            .map(|g| game.data.game_label(g, GameCopy::Cracked))
            .collect();
        // So does a music disk.
        let tunes: Vec<String> = game
            .tunes
            .iter()
            .filter(|t| self.learn_tune(t))
            .map(|t| game.data.tune(t).map_or(t.clone(), |d| d.title.clone()))
            .collect();
        for (b, t) in &game.texts {
            self.keep_text(b, t);
        }
        let here = game.bbs().name.clone();
        for id in &game.boards_read {
            self.read_effect(
                &game.data,
                &crate::data::ReadEffect::Board(id.clone()),
                &here,
            );
        }
        if game.starters_read {
            self.add_starters(&game.data);
        }
        let killer = game.killed_by();
        // Every fight as the rules booked it, in order, on the version it ran: a version that
        // landed mid-run is met as a new one, and the last one before it kept as it was known.
        // The last fight is the one that ended a flatlined run.
        for (k, b) in game.bouts.iter().enumerate() {
            let Some(def) = game.data.ice.get(b.ice) else {
                continue;
            };
            let strength = b.start.strength;
            let s = find_or_push(
                &mut self.ice_seen,
                |s| s.id == def.id,
                || IceSeen {
                    id: def.id.clone(),
                    lo: strength,
                    hi: strength,
                    version: b.version,
                    bump: b.bump,
                    program: b.program.clone(),
                    ..IceSeen::default()
                },
            );
            s.meet(b.version, b.bump, &b.program);
            s.met += 1;
            s.broken += u32::from(b.broken);
            s.lo = s.lo.min(strength);
            s.hi = s.hi.max(strength);
            s.won += u32::from(b.broken);
            s.lost += u32::from(killer == Some(b.ice) && k + 1 == game.bouts.len());
            // What the deck saw this version run comes home with its entry, once, at its
            // last fight on it. A save's own list may not be tidy (hand-edited, or older):
            // tidy it first.
            let later = game.bouts[k + 1..]
                .iter()
                .any(|x| x.ice == b.ice && x.version == b.version);
            if !later {
                s.lines = crate::ice_listing::tidy(std::mem::take(&mut s.lines));
                for &no in game.ice_lines_of(b.ice, b.version) {
                    crate::ice_listing::note_line(&mut s.lines, no);
                }
            }
        }
        // The sysops come home as the run left them, with what they shipped in it; the kind
        // that ended the run did its job, and they rest on it until it's beaten again.
        self.ice_study = game.ice_study.clone();
        if let Some(def) = killer.and_then(|k| game.data.ice.get(k))
            && let Some(s) = self.ice_study.iter_mut().find(|s| s.id == def.id)
        {
            s.rest = true;
        }
        for (id, was, next) in &game.shipped {
            ship(&mut self.ice_versions, id, *was, next.clone());
        }
        let mut boards = Vec::new();
        if game.bbs_seen {
            let b = game.bbs();
            if self.learn_board(&b.id, &format!("dialed in from {source}")) {
                boards.push(format!("{} ({})", b.name, b.phone));
            }
        }
        // A new run, new listings on every board.
        self.home_bought.clear();
        self.home_taken.clear();
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
                    // A program from the loadout goes home under its old id, so it's still
                    // installed in its place; a new find gets a new id.
                    let id = match game.from_stash.iter().find(|(uid, _)| *uid == slot.uid) {
                        Some((_, id)) => *id,
                        None => {
                            self.next_item += 1;
                            self.next_item - 1
                        }
                    };
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
        // What didn't come home drops off the disk; the rest stays installed.
        let stash = &self.stash;
        self.loadout
            .stash
            .retain(|id| stash.iter().any(|s| s.id == *id));
        Some(RunReport {
            end,
            credits,
            banked,
            hops: game.hops,
            seed: game.seed,
            kept,
            learned,
            boards,
            tools,
            games,
            tunes,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
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
    /// Phone ranges learned on the run.
    pub learned: Vec<String>,
    /// BBS numbers found on the run, as "NAME (555-0199)".
    pub boards: Vec<String>,
    /// Desktop tools downloaded on the run, by name.
    pub tools: Vec<String>,
    /// Games downloaded on the run (cracked copies), by label (`BLOX [PHS]`).
    pub games: Vec<String>,
    /// Tunes downloaded on the run, by title.
    pub tunes: Vec<String>,
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
    /// Dial a phonebook entry (by id), with an optional seed.
    Dial(u32, Option<u64>),
    /// Wardial a range (by id).
    Scan(String),
    /// Call a BBS from home (board id).
    Call(String),
    Open(Tool),
    /// Open System on a panel (`open stash` opens it on Disk).
    System(crate::system::Panel),
    /// Set the theme (by id): desktop and runs.
    Theme(String),
    /// Plug in a monitor you own (by id).
    Monitor(String),
    /// Run with a deck you own (by id).
    Deck(String),
    Crt,
    /// Modem sounds on/off.
    Sound,
    Clear,
    /// Run the balance probe with this many careers (`probe::Config::careers`).
    Probe(u32),
    ProbeStop,
}

/// Careers `probe` plays when you don't say.
pub const PROBE_CAREERS: u32 = 20;

/// Every verb the home Shell knows, for Tab completion.
pub const SHELL_VERBS: &[&str] = &[
    "help",
    "dir",
    "info",
    "phonebook",
    "dial",
    "ranges",
    "wardial",
    "run",
    "theme",
    "crt",
    "sound",
    "monitor",
    "deck",
    "echo",
    "leet",
    "clear",
    "probe",
];

/// Tab completion for the home Shell: the text before the cursor in, the words that could
/// finish its last word out. Verbs first, then what each verb takes: a phonebook entry or a
/// known board, a range, a tool, a theme, a monitor or deck you own.
pub fn shell_complete(before: &str, save: &Save, data: &Data) -> Vec<String> {
    let words: Vec<&str> = before.split_whitespace().collect();
    let fresh = before.is_empty() || before.ends_with(char::is_whitespace);
    let arg = (words.len() + usize::from(fresh)).saturating_sub(1);
    if arg == 0 {
        return SHELL_VERBS
            .iter()
            .filter(|v| shell_knows(save, v))
            .map(|v| v.to_string())
            .collect();
    }
    if !shell_knows(save, words[0]) {
        return Vec::new();
    }
    if arg > 1 {
        return Vec::new();
    }
    match words[0].to_ascii_lowercase().as_str() {
        "dial" | "atdt" => (1..=save.phonebook.len() + data.boards.len())
            .map(|n| n.to_string())
            .filter(|n| save.lookup(n).is_some() || save.lookup_board(data, n).is_some())
            .collect(),
        "wardial" | "scan" => save.ranges.iter().map(|r| r.id.clone()).collect(),
        "run" | "open" => save
            .icons(data)
            .iter()
            .map(|i| i.label.replace(' ', "").to_ascii_lowercase())
            .collect(),
        "theme" => data.themes.iter().map(|t| t.id.clone()).collect(),
        "monitor" | "display" => save.monitors.clone(),
        "deck" => save.decks.clone(),
        "probe" => vec!["stop".into()],
        "help" | "?" => vec!["net".into()],
        _ => Vec::new(),
    }
}

/// Whether the Shell knows a verb: a tool's commands only once you've downloaded the tool.
fn shell_knows(save: &Save, verb: &str) -> bool {
    match verb.to_ascii_lowercase().as_str() {
        "wardial" | "scan" => save.can_wardial(),
        _ => true,
    }
}

/// Runs one Shell line. Returns the lines to print and what to do.
pub fn shell(line: &str, save: &Save, data: &Data) -> (Vec<String>, ShellEffect) {
    let icons = save.icons(data);
    let mut words = line.split_whitespace();
    let Some(cmd) = words.next() else {
        return (Vec::new(), ShellEffect::None);
    };
    if !shell_knows(save, cmd) {
        return (vec![format!("Unknown command {cmd}")], ShellEffect::None);
    }
    let arg = words.next();
    let lines = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    match cmd.to_ascii_lowercase().as_str() {
        "help" | "?" if arg == Some("net") => (
            vec!["opening Welcome to the Net...".into()],
            ShellEffect::Open(Tool::Guide),
        ),
        "help" | "?" => (
            lines(&[
                "help net         a guide to the Net",
                "dir              list the desktop",
                "run <tool>       open a tool (jackin, shell, ...)",
                "phonebook        list numbers you can dial",
                "dial <n> [seed]  dial entry n (or a number) and jack in; a BBS opens here",
                "ranges           number ranges you know",
                "wardial <range>  scan 100 numbers of a range",
                "info             your record",
                "theme <name>     colour theme, desktop and runs (see System)",
                "crt              CRT effects on/off",
                "monitor [id]     plug in a monitor you own",
                "deck [id]        run with a deck you own",
                "sound            all sound on/off",
                "echo <text>      say something",
                "leet <text>      say it in l33t",
                "clear            clear the window",
                "probe [n|stop]   balance check: bots play n careers (20)",
            ])
            .into_iter()
            .filter(|l| shell_knows(save, l.split_whitespace().next().unwrap_or("")))
            .collect(),
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
                    match i.tool {
                        Tool::Shell => "tool",
                        Tool::Game(..) => "game",
                        _ => "drawer",
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
        "phonebook" | "pb" => {
            let boards = save.known_boards(data);
            if save.phonebook.is_empty() && boards.is_empty() {
                return (vec!["the phonebook is empty.".into()], ShellEffect::None);
            }
            // CarrierTerm's directory is names and numbers; NiGHTCOMM knows your heat, and
            // Tiger Team Pro knows what's behind the number too.
            let intel = save.has_dialer();
            let audit = save.has_tool("tigerteam");
            let mut out: Vec<String> = save
                .phonebook
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    if audit {
                        let traits: Vec<&str> = e
                            .traits
                            .iter()
                            .filter_map(|t| data.networks.trait_def(t).map(|d| d.label.as_str()))
                            .collect();
                        format!(
                            "{:>2}  {}  {:<22} T{} heat {}  {}",
                            i + 1,
                            e.number,
                            e.name,
                            e.tier,
                            e.heat,
                            traits.join(", ")
                        )
                    } else if intel {
                        format!(
                            "{:>2}  {}  {:<22} heat {} {}",
                            i + 1,
                            e.number,
                            e.name,
                            e.heat,
                            crate::nightcomm::heat_word(e.heat)
                        )
                    } else {
                        format!("{:>2}  {}  {:<22} {}", i + 1, e.number, e.name, e.business)
                    }
                })
                .collect();
            let first = save.phonebook.len() + 1;
            out.extend(boards.iter().enumerate().map(|(i, &b)| {
                let d = &data.boards[b];
                format!(
                    "{:>2}  {}  {:<22} BBS {}¢/call",
                    first + i,
                    d.phone,
                    d.name,
                    d.call_cost
                )
            }));
            (out, ShellEffect::None)
        }
        "dial" | "atdt" => {
            let Some(what) = arg else {
                return (
                    vec!["dial what? try `phonebook`.".into()],
                    ShellEffect::None,
                );
            };
            if let Some(b) = save.lookup_board(data, what) {
                let d = &data.boards[b];
                return (
                    vec![format!("ATDT {}", d.phone)],
                    ShellEffect::Call(d.id.clone()),
                );
            }
            let Some(e) = save.lookup(what) else {
                return (
                    vec![format!("{what}: no such number in the phonebook.")],
                    ShellEffect::None,
                );
            };
            let seed = words
                .next()
                .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok());
            (
                vec![format!("ATDT {}", e.number)],
                ShellEffect::Dial(e.id, seed),
            )
        }
        "ranges" => {
            let out = save
                .ranges
                .iter()
                .map(|k| format!("{:<18} from {}", k.id, k.source))
                .collect();
            (out, ShellEffect::None)
        }
        "wardial" | "scan" => match arg {
            Some(r) if save.knows_range(r) => (
                vec![format!("scanning {r}...")],
                ShellEffect::Scan(r.into()),
            ),
            Some(r) => (
                vec![format!("{r}: you don't know that range. try `ranges`.")],
                ShellEffect::None,
            ),
            None => (
                vec!["wardial which range? try `ranges`.".into()],
                ShellEffect::None,
            ),
        },
        "jackin" => (
            vec!["networks are dialed now: `phonebook`, then `dial <n>`.".into()],
            ShellEffect::None,
        ),
        "run" | "open" => {
            if arg.is_none() {
                return (vec!["run what? try `dir`.".into()], ShellEffect::None);
            }
            // The whole rest of the line, so names with spaces work: `run cpu shopper`,
            // `run cpushopper` and `run ice db` all open their tool.
            let name = line
                .trim()
                .split_once(char::is_whitespace)
                .map_or("", |(_, rest)| rest.trim());
            let key = |s: &str| s.replace(' ', "").to_ascii_lowercase();
            let alias = |name: &str| match key(name).as_str() {
                // Old names open what replaced them: RIG is CPU Shopper, Prefs and STASH are
                // System (STASH on its Disk panel).
                "rig" => Some(ShellEffect::Open(Tool::Rig)),
                "prefs" => Some(ShellEffect::Open(Tool::System)),
                "stash" => Some(ShellEffect::System(crate::system::Panel::Disk)),
                _ => None,
            };
            // A game by its box's name too (`run nibbler` opens the crack when that's the
            // copy you have); its full label picks the copy.
            let found = icons
                .iter()
                .find(|i| key(&i.label) == key(name))
                .or_else(|| {
                    icons.iter().find(|i| {
                        i.tool
                            .game_def(data)
                            .is_some_and(|(g, _)| key(&g.name) == key(name))
                    })
                });
            match found {
                Some(i) => (Vec::new(), ShellEffect::Open(i.tool)),
                None => match alias(name) {
                    Some(e) => (Vec::new(), e),
                    None => (vec![format!("{name}: object not found")], ShellEffect::None),
                },
            }
        }
        "theme" => match arg.and_then(|t| data.theme(t)) {
            Some(t) => match save.theme_blocker(data, &t.id) {
                Some(why) => (vec![why], ShellEffect::None),
                None => (Vec::new(), ShellEffect::Theme(t.id.clone())),
            },
            None => {
                let ids: Vec<&str> = data.themes.iter().map(|t| t.id.as_str()).collect();
                (
                    vec![format!("theme {}", ids.join(" | "))],
                    ShellEffect::None,
                )
            }
        },
        "crt" => (Vec::new(), ShellEffect::Crt),
        "sound" => (Vec::new(), ShellEffect::Sound),
        "monitor" | "display" => match arg {
            Some(id) if save.monitors.iter().any(|m| m == id) => {
                (Vec::new(), ShellEffect::Monitor(id.to_string()))
            }
            Some(id) => (
                vec![format!(
                    "{id}: you don't own that monitor. CPU Shopper sells them."
                )],
                ShellEffect::None,
            ),
            None => (
                save.monitors
                    .iter()
                    .filter_map(|id| data.hardware.monitor(id))
                    .map(|m| {
                        let on = if m.id == save.monitor { "*" } else { " " };
                        format!("{on} {:<8} {}", m.id, m.title())
                    })
                    .collect(),
                ShellEffect::None,
            ),
        },
        "deck" => match arg {
            Some(id) if save.decks.iter().any(|d| d == id) => {
                (Vec::new(), ShellEffect::Deck(id.to_string()))
            }
            Some(id) => (
                vec![format!(
                    "{id}: you don't own that deck. CPU Shopper sells them."
                )],
                ShellEffect::None,
            ),
            None => (
                save.decks
                    .iter()
                    .filter_map(|id| data.hardware.deck(id))
                    .map(|d| {
                        let on = if d.id == save.deck { "*" } else { " " };
                        format!("{on} {:<10} {}", d.id, d.title())
                    })
                    .collect(),
                ShellEffect::None,
            ),
        },
        "clear" | "cls" => (Vec::new(), ShellEffect::Clear),
        "probe" => match arg {
            None => (Vec::new(), ShellEffect::Probe(PROBE_CAREERS)),
            Some("stop") => (Vec::new(), ShellEffect::ProbeStop),
            Some(n) => match n.parse::<u32>() {
                Ok(n) if n > 0 => (Vec::new(), ShellEffect::Probe(n.min(500))),
                _ => (
                    vec!["probe [careers]: how many careers to play (1-500), or `stop`.".into()],
                    ShellEffect::None,
                ),
            },
        },
        "leet" | "l33t" => (
            vec![crate::leet::translate(
                line.trim_start()[cmd.len()..].trim(),
            )],
            ShellEffect::None,
        ),
        "echo" => (
            vec![line.trim_start()[cmd.len()..].trim().to_string()],
            ShellEffect::None,
        ),
        other => (vec![format!("Unknown command {other}")], ShellEffect::None),
    }
}
