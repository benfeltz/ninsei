//! Content definitions, loaded from the RON files in `/data`. The files are compiled in,
//! so the game has no file I/O and runs the same on web.

use serde::Deserialize;

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IceClass {
    Barrier,
    CodeGate,
    Sentry,
}

impl IceClass {
    pub fn label(self) -> &'static str {
        match self {
            IceClass::Barrier => "BARRIER",
            IceClass::CodeGate => "CODE GATE",
            IceClass::Sentry => "SENTRY",
        }
    }
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum WarezKind {
    Breaker,
    Shield,
    Utility,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    Break {
        power: u32,
        vs: Option<IceClass>,
    },
    Shield(u32),
    Repair(u32),
    Scrub(u32),
    Stun,
    /// Clears an ICE's SPOOF: its NEXT shows again.
    Sniff,
}

impl Effect {
    pub fn describe(self) -> String {
        match self {
            Effect::Break { power, vs: None } => format!("break {power}"),
            Effect::Break { power, vs: Some(c) } => format!("break {power} (x2 vs {})", c.label()),
            Effect::Shield(n) => format!("shield {n}"),
            Effect::Repair(n) => format!("repair {n}"),
            Effect::Scrub(n) => format!("trace -{n}"),
            Effect::Stun => "stun ICE 1 turn".to_string(),
            Effect::Sniff => "clear a spoof".to_string(),
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct WarezDef {
    pub id: String,
    pub name: String,
    pub kind: WarezKind,
    pub cpu: u32,
    pub effect: Effect,
    pub blurb: String,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Intent {
    Attack(u32),
    Trace(u32),
    Fortify(u32),
    /// Hides its NEXT from the deck for n turns.
    Spoof(u32),
    /// Garbles a line of the deck's program.
    Segfault,
}

impl Intent {
    pub fn describe(self) -> String {
        match self {
            Intent::Attack(n) => format!("ATTACK {n}"),
            Intent::Trace(n) => format!("TRACE +{n}"),
            Intent::Fortify(n) => format!("FORTIFY +{n}"),
            Intent::Spoof(n) => format!("SPOOF {n}"),
            Intent::Segfault => "SEGFAULT".to_string(),
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct IceDef {
    pub id: String,
    pub name: String,
    pub class: IceClass,
    /// 1 = ICE, 2 = black ICE, 0 = never placed on the map (met only through encounters).
    pub tier: u32,
    pub strength: u32,
    /// Its behaviour, in the game's BASIC with its own vocabulary (`verbs`): run at the end of
    /// every turn of a fight from where it left off.
    pub program: String,
    /// Actions a turn: each verb spends one, and at 0 its turn is over.
    pub cpu: u32,
    /// IFs a turn: each spends one, and with none left its turn is over.
    pub ram: u32,
    /// The verbs its program may use; any other word is a bad line.
    pub verbs: Vec<crate::basic::Verb>,
    /// Whether it gives up when it's losing.
    #[serde(default)]
    pub yields: Option<YieldDef>,
}

/// A fighter that can give up: once a hit drops it to `below`% of its strength or less,
/// each hit after that has a `chance`% chance it quits. Quitting counts as a win.
#[derive(Deserialize, Clone, Debug)]
pub struct YieldDef {
    pub below: u32,
    pub chance: u32,
    /// What it says when it quits.
    pub line: String,
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum Drop {
    Warez(String),
    Credits(u32, u32),
    Ram(u32),
    Cpu(u32),
    Hdd(u32),
    Repair(u32),
    /// Optimizer fragments, the hex-editor crafting material.
    Fragment(u32),
    /// Lowers trace.
    Scrub(u32),
    /// A number range for the wardialer (id in `hub/exchanges.ron`). Comes home even on a flatline.
    Hint(String),
    /// A desktop tool (id in `hub/tools.ron`): yours for good once downloaded, even on a flatline.
    Tool(String),
    /// A cracked copy of a game (id in `hub/games.ron`): yours for good, even on a flatline.
    Game(String),
    /// A tune on a music disk (id in `data/music/`): yours for good, even on a flatline.
    Tune(String),
    /// A textfile on a board (id in the board's `texts`): you read it there, free.
    Text(String),
    Nothing,
}

#[derive(Deserialize, Clone, Debug)]
pub struct LootEntry {
    pub weight: u32,
    pub drop: Drop,
}

#[derive(Deserialize, Clone, Debug)]
pub struct LootTable {
    pub rolls: u32,
    pub entries: Vec<LootEntry>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct LootTables {
    pub ice: LootTable,
    pub black_ice: LootTable,
    pub vault: LootTable,
}

#[derive(Deserialize, Clone, Debug)]
pub struct BulletinDef {
    pub title: String,
    pub body: Vec<String>,
    /// What reading it does, if anything.
    #[serde(default)]
    pub on_read: Option<ReadEffect>,
}

/// What reading a bulletin or a post does: the way a board's words open doors.
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum ReadEffect {
    /// A board's number goes in the phonebook (an id in `data/bbs/`).
    Board(String),
    /// The starter networks go in the phonebook (`tuning::PHONEBOOK_START`), once.
    Starters,
    /// A number range, as `Drop::Hint` teaches.
    Hint(String),
}

/// A message base on a board: a name and its posts, oldest first.
#[derive(Deserialize, Clone, Debug)]
pub struct BaseDef {
    pub name: String,
    pub posts: Vec<PostDef>,
}

/// One post in a message base.
#[derive(Deserialize, Clone, Debug)]
pub struct PostDef {
    pub from: String,
    pub subject: String,
    pub body: Vec<String>,
    #[serde(default)]
    pub on_read: Option<ReadEffect>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct BbsUser {
    pub handle: String,
    pub location: String,
    pub doing: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct BbsFileDef {
    /// 8.3 filename shown in the listing.
    pub file: String,
    pub item: Drop,
    pub price: u32,
    pub desc: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct BbsAreaDef {
    pub name: String,
    /// Files stocked per run, picked at random from `files`.
    pub stock: u32,
    /// Every file is always stocked (no roll): tools and textfiles you can count on.
    #[serde(default)]
    pub always: bool,
    pub files: Vec<BbsFileDef>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct BbsDef {
    /// The folder name under `data/bbs/`.
    #[serde(skip)]
    pub id: String,
    pub name: String,
    pub sysop: String,
    pub phone: String,
    pub baud: u32,
    /// The board plain runs (`Game::new`, `newrun`) and unknown networks connect to. Exactly one.
    #[serde(default)]
    pub default: bool,
    /// Network tiers this board turns up on. Empty means none: it's only reached by a trait's
    /// `Board` effect or as the default.
    #[serde(default)]
    pub tiers: Vec<u32>,
    #[serde(default)]
    pub look: Look,
    /// Phone bill for calling the board from home.
    #[serde(default = "default_call_cost")]
    pub call_cost: u32,
    pub bulletins: Vec<BulletinDef>,
    pub users: Vec<BbsUser>,
    pub areas: Vec<BbsAreaDef>,
    /// Textfiles you read on the board (`Text(id)` files).
    #[serde(default)]
    pub texts: Vec<TextDef>,
    /// Message bases (none on most boards).
    #[serde(default)]
    pub messages: Vec<BaseDef>,
    /// Whether it posts contracts. A corporate customer line doesn't.
    #[serde(default = "yes")]
    pub jobs: bool,
    /// Pipe-coded splash art, from the board's `splash.txt`.
    #[serde(skip)]
    pub splash: String,
}

/// A textfile on a board: a title and pipe-coded lines, read on a BBS screen.
#[derive(Deserialize, Clone, Debug)]
pub struct TextDef {
    pub id: String,
    pub title: String,
    pub lines: Vec<String>,
}

/// A desktop tool you download (`data/hub/tools.ron`).
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ToolDef {
    pub id: String,
    pub name: String,
    pub blurb: String,
    /// The cracktro it opens with (an id in `data/intros/`), if any.
    #[serde(default)]
    pub intro: Option<String>,
    /// Sold by mail order in CPU Shopper, if it is.
    #[serde(default)]
    pub shop: Option<crate::shopper::ToolShop>,
    /// Its version string, as System's About This Deck lists it ("3.1").
    #[serde(default = "first_version")]
    pub version: String,
}

fn first_version() -> String {
    "1.0".into()
}

/// A game you can own (`data/hub/games.ron`): a cracked copy from a board, a boxed one by mail
/// order, or both side by side. Each copy opens with its own intro and has its own label
/// ([`Data::game_label`]).
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct GameDef {
    pub id: String,
    /// The box's name, `Nibbler`; a crack is labelled from it ([`Data::game_label`]).
    pub name: String,
    pub blurb: String,
    /// Who publishes the boxed copy (a vendor id in `hub/shopper.ron`).
    pub publisher: String,
    /// Its version string, as System's About This Deck lists it.
    #[serde(default = "first_version")]
    pub version: String,
    /// The rules it runs.
    pub engine: crate::games::Engine,
    /// The play field, in cells: at least the engine's `min_size`.
    pub cols: u32,
    pub rows: u32,
    /// How long one tick of the game's clock lasts: the app steps it once per tick.
    pub tick_ms: u32,
    /// The cracktro a board's copy opens with (a crack in `data/intros/`).
    pub crack: String,
    /// The splash the boxed copy opens with (a splash in `data/intros/`).
    pub splash: String,
    /// Sold boxed in CPU Shopper's Games section, if it is.
    #[serde(default)]
    pub shop: Option<GameShop>,
    /// Its desktop icon, in the 1.3 palette like `hub/icons.ron`'s (both copies wear it).
    pub icon: Vec<String>,
}

impl GameDef {
    /// The intro a copy plays when it opens: a crack its crew's cracktro, a box its
    /// publisher's splash.
    pub fn intro(&self, copy: crate::hub::GameCopy) -> &str {
        match copy {
            crate::hub::GameCopy::Cracked => &self.crack,
            crate::hub::GameCopy::Boxed => &self.splash,
        }
    }
}

/// How a game is sold by mail order (the optional `shop` block in `games.ron`).
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct GameShop {
    pub price: u32,
}

/// A music disk sold boxed by mail order (`data/hub/compilations.ron`): one order, several
/// tunes. Its publisher writes it up; it knows its own tunes and nothing of the scene's.
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct CompilationDef {
    pub id: String,
    pub name: String,
    pub blurb: String,
    /// Who sells it (a vendor id in `hub/shopper.ron`).
    pub publisher: String,
    pub price: u32,
    /// The tunes on it (ids in `data/music/`), in the order they play.
    pub tunes: Vec<String>,
}

/// The shortest and longest tick a game may ask for.
pub const GAME_TICK_MS: std::ops::RangeInclusive<u32> = 10..=1000;

/// Every tool's id: what `hub::Save::has_tool` and the desktops ask about.
pub const TOOLS: &[&str] = &[
    "wardial",
    "hexed",
    "qwkread",
    "icedb",
    "nightcomm",
    "tigerteam",
    "player",
];

fn yes() -> bool {
    true
}

fn default_call_cost() -> u32 {
    crate::tuning::BBS_CALL_COST
}

/// How the header bar across the top of every menu screen is drawn.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum HeaderStyle {
    /// `▄▄ BOARD ▄▄▄▄▄ TITLE ▄▄` with the names on the `bar` background.
    #[default]
    Bar,
    /// `══[ BOARD ]══════[ TITLE ]══` box-drawing rule.
    Rule,
    /// One solid inverse bar the full width of the screen.
    Inverse,
}

/// A board's look and feel. Colours are CGA indices (0-15) for pipe codes; `bar` is a
/// background (0-7). Every field has a default, so a board only lists what it changes.
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(default)]
pub struct Look {
    /// BBS software, shown once the modem connects.
    pub software: String,
    pub header: HeaderStyle,
    /// Header fill colour and the brackets around hotkeys.
    pub frame: u8,
    /// Header background.
    pub bar: u8,
    /// Hotkeys and highlighted text.
    pub key: u8,
    /// Menu items and file names.
    pub label: u8,
    /// Notes, rules, column headings.
    pub dim: u8,
    /// Body text.
    pub text: u8,
    /// Prices, credits and bulletin titles.
    pub money: u8,
    pub ok: u8,
    pub warn: u8,
    /// Who's-online activity and other asides.
    pub aside: u8,
    /// The prompt marker after "Command".
    pub prompt: String,
    /// The handle you're logged in as.
    pub handle: String,
    /// Main menu wording: (label, note) for each command. The hotkeys stay F J B W G.
    pub menu: MenuText,
    /// The line shown as the board hangs up. `{board}` becomes the board's name.
    pub goodbye: String,
    /// Whether the board speaks l33t (every screen but the splash and the modem lines).
    pub leet: bool,
}

impl Default for Look {
    fn default() -> Self {
        Self {
            software: "Renegade v05-11-91".into(),
            header: HeaderStyle::Bar,
            frame: 9,
            bar: 1,
            key: 15,
            label: 11,
            dim: 8,
            text: 7,
            money: 14,
            ok: 10,
            warn: 12,
            aside: 3,
            prompt: "»".into(),
            handle: "runner".into(),
            menu: MenuText::default(),
            goodbye: "Thanks for calling {board}. Stay frosty.".into(),
            leet: true,
        }
    }
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(default)]
pub struct MenuText {
    pub files: (String, String),
    pub messages: (String, String),
    pub jobs: (String, String),
    pub bulletins: (String, String),
    pub who: (String, String),
    pub goodbye: (String, String),
}

impl Default for MenuText {
    fn default() -> Self {
        let p = |a: &str, b: &str| (a.to_string(), b.to_string());
        Self {
            files: p("File Areas", "warez, utilities, deck mods"),
            messages: p("Message Bases", "read the boards"),
            jobs: p("Jobs Board", "contracts. no names, just creds"),
            bulletins: p("Bulletins", "read before you leech"),
            who: p("Who's Online", ""),
            goodbye: p("Goodbye", "log off"),
        }
    }
}

impl BbsDef {
    pub fn validate(&self) -> Result<(), String> {
        let l = &self.look;
        for (name, c) in [
            ("frame", l.frame),
            ("key", l.key),
            ("label", l.label),
            ("dim", l.dim),
            ("text", l.text),
            ("money", l.money),
            ("ok", l.ok),
            ("warn", l.warn),
            ("aside", l.aside),
        ] {
            if c > 15 {
                return Err(format!("look.{name} is {c}; colours are 0-15"));
            }
        }
        if l.bar > 7 {
            return Err(format!("look.bar is {}; backgrounds are 0-7", l.bar));
        }
        if self.name.chars().count() > 30 {
            return Err("name is longer than 30 characters".into());
        }
        if self.bulletins.is_empty() || self.areas.is_empty() {
            return Err("needs at least one bulletin and one file area".into());
        }
        if self.areas.len() > 9 || self.bulletins.len() > 9 {
            return Err("at most 9 areas and 9 bulletins (one keypress each)".into());
        }
        if self.messages.len() > 9 || self.messages.iter().any(|b| b.posts.len() > 9) {
            return Err("at most 9 message bases of 9 posts (one keypress each)".into());
        }
        if self.messages.iter().any(|b| b.posts.is_empty()) {
            return Err("a message base with no posts".into());
        }
        Ok(())
    }
}

/// One board's raw files, found under `data/bbs/<id>/` at build time.
#[derive(Clone, Copy, Debug)]
pub struct BoardSrc<'a> {
    pub id: &'a str,
    pub board: &'a str,
    pub splash: &'a str,
}

/// One sound pack's raw file, found under `data/sounds/<id>.ron` at build time.
#[derive(Clone, Copy, Debug)]
pub struct PackSrc<'a> {
    pub id: &'a str,
    pub sounds: &'a str,
}

/// One intro's raw file, found under `data/intros/<id>.ron` at build time.
#[derive(Clone, Copy, Debug)]
pub struct IntroSrc<'a> {
    pub id: &'a str,
    pub intro: &'a str,
}

/// Every intro in `data/intros/`, in file order.
pub const BUILTIN_INTROS: &[IntroSrc<'static>] = include!(concat!(env!("OUT_DIR"), "/intros.rs"));

/// One picture's source: `data/art/<id>.txt`.
#[derive(Clone, Copy, Debug)]
pub struct ArtSrc<'a> {
    pub id: &'a str,
    pub art: &'a str,
}

/// One tune's raw file, found under `data/music/<id>.ron` at build time.
#[derive(Clone, Copy, Debug)]
pub struct TuneSrc<'a> {
    pub id: &'a str,
    pub tune: &'a str,
}

/// Every tune in `data/music/`, in file order.
pub const BUILTIN_TUNES: &[TuneSrc<'static>] = include!(concat!(env!("OUT_DIR"), "/music.rs"));

/// A tune on a music disk (`data/music/<id>.ron`): what the desktop's player plays. Its title
/// and its author are the author's own words; the tune is a cracktro-format pattern,
/// synthesized like everything else.
#[derive(Deserialize, Clone, Debug)]
pub struct TuneFile {
    /// The file's name, set when it loads.
    #[serde(skip)]
    pub id: String,
    /// What a playlist calls it, as its author wrote it.
    pub title: String,
    /// Who wrote it: a crew or a publisher that signs a cracktro in `data/intros/`.
    pub by: String,
    /// How long it plays in a playlist before the next one, in seconds: one loop or more.
    pub secs: u32,
    pub tune: crate::intro::TuneDef,
    /// Steps one pass of the loop lasts (`TuneDef::loop_steps`), worked out once when it loads.
    #[serde(skip)]
    pub loop_steps: usize,
}

/// The longest a tune's title may be, so a playlist row never cuts one.
pub const TUNE_TITLE_MAX: usize = 24;

/// The longest a tune may play in a playlist, in seconds.
pub const TUNE_SECS_MAX: u32 = 600;

/// Every picture in `data/art/`, in file order.
pub const BUILTIN_ART: &[ArtSrc<'static>] = include!(concat!(env!("OUT_DIR"), "/art.rs"));

/// A 1-bit picture (`data/art/<id>.txt`): rows of `#` (ink) and `.` (paper). It's drawn in
/// the theme's colours at a whole-number scale, so replacing one is overwriting its text file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtDef {
    pub id: String,
    pub rows: Vec<String>,
}

/// The biggest picture `data/art/` may hold.
pub const ART_MAX: (usize, usize) = (128, 96);

impl ArtDef {
    pub fn parse(id: &str, text: &str) -> Self {
        Self {
            id: id.to_string(),
            rows: text
                .lines()
                .map(str::trim_end)
                .filter(|l| !l.is_empty())
                .map(String::from)
                .collect(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let w = self.rows.first().map_or(0, |r| r.chars().count());
        if w == 0 {
            return Err("is empty".into());
        }
        if self.rows.iter().any(|r| r.chars().count() != w) {
            return Err("has rows of different widths".into());
        }
        if self
            .rows
            .iter()
            .any(|r| r.chars().any(|c| c != '#' && c != '.'))
        {
            return Err("may only hold `#` (ink) and `.` (paper)".into());
        }
        if w > ART_MAX.0 || self.rows.len() > ART_MAX.1 {
            return Err(format!("is bigger than {}x{}", ART_MAX.0, ART_MAX.1));
        }
        Ok(())
    }
}

/// Every sound pack in `data/sounds/`, in file order.
pub const BUILTIN_SOUNDS: &[PackSrc<'static>] = include!(concat!(env!("OUT_DIR"), "/sounds.rs"));

/// Every board in `data/bbs/`, in folder order.
pub const BUILTIN_BOARDS: &[BoardSrc<'static>] = include!(concat!(env!("OUT_DIR"), "/boards.rs"));

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Penalty {
    Trace(u32),
    Damage(u32),
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum ChoiceEffect {
    Leave,
    /// Swap one of your programs for one picked from `pool` when the event spawns.
    Trade {
        pool: Vec<String>,
    },
    /// Sell a program for `per_cpu` credits per CPU it takes to run.
    Sell {
        per_cpu: u32,
    },
    /// Take a contract on a node ahead.
    Contract,
    Pay {
        cost: u32,
        get: Drop,
    },
    Gamble {
        pct: u32,
        win: Drop,
        lose: Penalty,
    },
    /// Fight ICE `ice` here; every drop in `rewards` on top of the usual loot.
    Fight {
        ice: String,
        rewards: Vec<Drop>,
    },
}

#[derive(Deserialize, Clone, Debug)]
pub struct ChoiceDef {
    pub label: String,
    pub effect: ChoiceEffect,
}

#[derive(Deserialize, Clone, Debug)]
pub struct EventDef {
    pub id: String,
    /// Chat handle of whoever is talking.
    pub from: String,
    pub title: String,
    pub text: Vec<String>,
    pub choices: Vec<ChoiceDef>,
    /// Speaks l33t: text, title and choice labels go through `leet::translate`.
    #[serde(default)]
    pub leet: bool,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ContractTemplate {
    pub client: String,
    /// `{host}` becomes the target's hostname.
    pub brief: String,
}

/// `data/contracts.ron`: the clients, and the ways they can turn on you.
#[derive(Deserialize, Clone, Debug)]
struct ContractsFile {
    clients: Vec<ContractTemplate>,
    betrayals: Vec<BetrayalDef>,
}

/// One way a client can double-cross you once the target falls.
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct BetrayalDef {
    pub id: String,
    /// Short status for the todo pane and the work orders, e.g. `DEFAULTED`.
    pub label: String,
    pub weight: u32,
    /// The lowest tier it happens on (plain runs count as tier 1).
    #[serde(default = "tier_one")]
    pub min_tier: u32,
    pub effect: BetrayalEffect,
    /// What the log says. `{client}` is the client.
    pub says: String,
}

fn tier_one() -> u32 {
    1
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum BetrayalEffect {
    /// They pay only `pct`% of what they owe (bonus included).
    Stiffed(u32),
    /// They tip off the target: trace jumps, and the rest of the run's ICE rezzes stronger.
    Burned { trace: u32, ice: u32 },
    /// They hand your number to the telco: that phonebook entry's heat jumps.
    SoldOut(u32),
    /// A hunter rezzes on your node right after the payout: `Data::ice` id.
    Setup(String),
}

/// A theme's six colours, as `#rrggbb`.
#[derive(Deserialize, Clone, Debug)]
pub struct ThemeColors {
    pub bg: String,
    pub panel: String,
    pub faint: String,
    pub dim: String,
    pub fg: String,
    pub bright: String,
}

impl ThemeColors {
    /// The six colours in role order: bg, panel, faint, dim, fg, bright.
    pub fn all(&self) -> [&str; 6] {
        [
            &self.bg,
            &self.panel,
            &self.faint,
            &self.dim,
            &self.fg,
            &self.bright,
        ]
    }
}

/// A colour theme (`data/themes.ron`).
#[derive(Deserialize, Clone, Debug)]
pub struct ThemeDef {
    pub id: String,
    pub label: String,
    pub colors: ThemeColors,
}

/// Parses `#rrggbb`.
pub fn rgb(hex: &str) -> Option<[u8; 3]> {
    let h = hex.strip_prefix('#')?;
    if h.len() != 6 || !h.is_ascii() {
        return None;
    }
    let c = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some([c(0)?, c(2)?, c(4)?])
}

/// The raw text of every data file.
#[derive(Clone, Copy, Debug)]
pub struct Sources<'a> {
    pub warez: &'a str,
    pub ice: &'a str,
    pub loot: &'a str,
    pub boards: &'a [BoardSrc<'a>],
    pub events: &'a str,
    pub contracts: &'a str,
    pub icons: &'a str,
    pub liza: &'a str,
    pub patterns: &'a str,
    pub netmap: &'a str,
    pub hardware: &'a str,
    pub tools: &'a str,
    pub games: &'a str,
    pub compilations: &'a str,
    pub addons: &'a str,
    pub panes: &'a str,
    pub shopper: &'a str,
    pub disk: &'a str,
    pub fetch: &'a str,
    pub panel_icons: &'a str,
    pub sounds: &'a [PackSrc<'a>],
    pub art: &'a [ArtSrc<'a>],
    pub intros: &'a [IntroSrc<'a>],
    pub tunes: &'a [TuneSrc<'a>],
    pub welcome: &'a str,
    pub starts: &'a str,
    pub rig: &'a str,
    pub networks: &'a str,
    pub exchanges: &'a str,
    pub themes: &'a str,
    pub displays: &'a str,
}

impl Sources<'static> {
    /// The content shipped in `/data`.
    pub fn builtin() -> Self {
        Self {
            warez: include_str!("../../../data/warez.ron"),
            ice: include_str!("../../../data/ice.ron"),
            loot: include_str!("../../../data/loot.ron"),
            boards: BUILTIN_BOARDS,
            events: include_str!("../../../data/events.ron"),
            contracts: include_str!("../../../data/contracts.ron"),
            icons: include_str!("../../../data/hub/icons.ron"),
            liza: include_str!("../../../data/hub/liza.ron"),
            patterns: include_str!("../../../data/patterns.ron"),
            netmap: include_str!("../../../data/netmap.ron"),
            hardware: include_str!("../../../data/hub/hardware.ron"),
            tools: include_str!("../../../data/hub/tools.ron"),
            games: include_str!("../../../data/hub/games.ron"),
            compilations: include_str!("../../../data/hub/compilations.ron"),
            addons: include_str!("../../../data/hub/desktops.ron"),
            panes: include_str!("../../../data/hub/panes.ron"),
            shopper: include_str!("../../../data/hub/shopper.ron"),
            disk: include_str!("../../../data/hub/disk.ron"),
            fetch: include_str!("../../../data/hub/fetch.ron"),
            panel_icons: include_str!("../../../data/hub/panel_icons.ron"),
            sounds: BUILTIN_SOUNDS,
            art: BUILTIN_ART,
            intros: BUILTIN_INTROS,
            tunes: BUILTIN_TUNES,
            welcome: include_str!("../../../data/stacks/welcome.ron"),
            starts: include_str!("../../../data/hub/starts.ron"),
            rig: include_str!("../../../data/hub/rig.ron"),
            networks: include_str!("../../../data/hub/networks.ron"),
            exchanges: include_str!("../../../data/hub/exchanges.ron"),
            themes: include_str!("../../../data/themes.ron"),
            displays: include_str!("../../../data/displays.ron"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Data {
    pub warez: Vec<WarezDef>,
    pub ice: Vec<IceDef>,
    pub loot: LootTables,
    /// Every BBS, the default board first.
    pub boards: Vec<BbsDef>,
    pub events: Vec<EventDef>,
    pub contracts: Vec<ContractTemplate>,
    /// How clients turn on you, from `contracts.ron`.
    pub betrayals: Vec<BetrayalDef>,
    /// Workbench desktop icons.
    pub icons: Vec<crate::hub::IconDef>,
    /// The Liza desktop's pixel art: the boot card's face and the Trash.
    pub liza: crate::hub::LizaArt,
    /// 1-bit fill patterns for the Sense/Net deck's parts.
    pub patterns: Vec<crate::pattern::PatternDef>,
    /// How the network map draws each kind of node.
    pub netmap: Vec<crate::netview::MapStyleDef>,
    /// Monitors and decks you can own.
    pub hardware: crate::hardware::HardwareDef,
    /// Desktop tools you download from boards.
    pub tools: Vec<ToolDef>,
    /// Games you can own: cracked from a board or boxed by mail.
    pub games: Vec<GameDef>,
    /// Music disks sold boxed by mail order.
    pub compilations: Vec<CompilationDef>,
    /// Desktop add-ons you can buy.
    pub addons: Vec<crate::desktop::AddonDef>,
    /// Secondary panes you can buy for the run screen.
    pub panes: Vec<crate::panes::PaneDef>,
    /// CPU Shopper, the RIG magazine: its cover, sections, advertisers and ads.
    pub shopper: crate::shopper::ShopperDef,
    /// System's Disk panel: the pattern per program kind on the block map.
    pub disk: crate::disk::DiskDef,
    /// System's neofetch-style logos, one per desktop.
    pub fetch: Vec<crate::system::FetchDef>,
    /// System's panel icons for Liza's Control Panel list.
    pub panel_icons: Vec<crate::system::PanelIconDef>,
    /// Sound packs: what each cue plays, one pack per file in `data/sounds/`.
    pub sounds: Vec<crate::cue::SoundPack>,
    /// 1-bit pictures, one per file in `data/art/` (deck heroes and the like).
    pub art: Vec<ArtDef>,
    /// Cracktros, one per file in `data/intros/`.
    pub intros: Vec<crate::intro::IntroDef>,
    /// Tunes on music disks, one per file in `data/music/`.
    pub tunes: Vec<TuneFile>,
    /// The Welcome to the Net stack (`data/stacks/welcome.ron`).
    pub welcome: crate::cards::StackDef,
    /// How a new deck starts: what everyone gets, and the kits (`data/hub/starts.ron`).
    pub starts: crate::hub::StartsDef,
    /// RIG deck-shop upgrades.
    pub rig: Vec<crate::hub::UpgradeDef>,
    /// Network tiers, traits and companies for the phonebook.
    pub networks: crate::hub::NetworksDef,
    /// Number ranges the wardialer scans.
    pub exchanges: Vec<crate::hub::RangeDef>,
    /// Colour themes for runs and the desktop.
    pub themes: Vec<ThemeDef>,
    /// CRT display presets.
    pub displays: Vec<crate::display::DisplayDef>,
}

impl Data {
    /// The content shipped in `/data`.
    pub fn builtin() -> Result<Self, String> {
        Self::parse(Sources::builtin())
    }

    pub fn parse(src: Sources) -> Result<Self, String> {
        let mut boards = Vec::new();
        for b in src.boards {
            let mut def: BbsDef =
                ron::from_str(b.board).map_err(|e| format!("bbs/{}/board.ron: {e}", b.id))?;
            def.id = b.id.to_string();
            def.splash = b.splash.to_string();
            boards.push(def);
        }
        // The default board goes first, so index 0 is always it.
        boards.sort_by_key(|b| !b.default);
        let contracts: ContractsFile =
            ron::from_str(src.contracts).map_err(|e| format!("contracts.ron: {e}"))?;
        let data = Self {
            warez: ron::from_str(src.warez).map_err(|e| format!("warez.ron: {e}"))?,
            ice: ron::from_str(src.ice).map_err(|e| format!("ice.ron: {e}"))?,
            loot: ron::from_str(src.loot).map_err(|e| format!("loot.ron: {e}"))?,
            boards,
            events: ron::from_str(src.events).map_err(|e| format!("events.ron: {e}"))?,
            contracts: contracts.clients,
            betrayals: contracts.betrayals,
            icons: ron::from_str(src.icons).map_err(|e| format!("hub/icons.ron: {e}"))?,
            liza: ron::from_str(src.liza).map_err(|e| format!("hub/liza.ron: {e}"))?,
            patterns: ron::from_str(src.patterns).map_err(|e| format!("patterns.ron: {e}"))?,
            netmap: ron::from_str(src.netmap).map_err(|e| format!("netmap.ron: {e}"))?,
            hardware: ron::from_str(src.hardware).map_err(|e| format!("hub/hardware.ron: {e}"))?,
            tools: ron::from_str(src.tools).map_err(|e| format!("hub/tools.ron: {e}"))?,
            games: ron::from_str(src.games).map_err(|e| format!("hub/games.ron: {e}"))?,
            compilations: ron::from_str(src.compilations)
                .map_err(|e| format!("hub/compilations.ron: {e}"))?,
            addons: ron::from_str(src.addons).map_err(|e| format!("hub/desktops.ron: {e}"))?,
            panes: ron::from_str(src.panes).map_err(|e| format!("hub/panes.ron: {e}"))?,
            shopper: ron::from_str(src.shopper).map_err(|e| format!("hub/shopper.ron: {e}"))?,
            disk: ron::from_str(src.disk).map_err(|e| format!("hub/disk.ron: {e}"))?,
            fetch: ron::from_str(src.fetch).map_err(|e| format!("hub/fetch.ron: {e}"))?,
            panel_icons: ron::from_str(src.panel_icons)
                .map_err(|e| format!("hub/panel_icons.ron: {e}"))?,
            welcome: ron::from_str(src.welcome).map_err(|e| format!("stacks/welcome.ron: {e}"))?,
            starts: ron::from_str(src.starts).map_err(|e| format!("hub/starts.ron: {e}"))?,
            intros: src
                .intros
                .iter()
                .map(|i| {
                    ron::from_str::<crate::intro::IntroDef>(i.intro)
                        .map(|mut d| {
                            d.id = i.id.to_string();
                            d
                        })
                        .map_err(|e| format!("intros/{}.ron: {e}", i.id))
                })
                .collect::<Result<_, _>>()?,
            tunes: src
                .tunes
                .iter()
                .map(|t| {
                    ron::from_str::<TuneFile>(t.tune)
                        .map(|mut d| {
                            d.id = t.id.to_string();
                            d.loop_steps = d.tune.loop_steps();
                            d
                        })
                        .map_err(|e| format!("music/{}.ron: {e}", t.id))
                })
                .collect::<Result<_, _>>()?,
            art: src.art.iter().map(|a| ArtDef::parse(a.id, a.art)).collect(),
            sounds: src
                .sounds
                .iter()
                .map(|p| {
                    ron::from_str(p.sounds)
                        .map(|f: crate::cue::PackFile| crate::cue::SoundPack {
                            id: p.id.to_string(),
                            sounds: f.cues,
                            music: f.music,
                        })
                        .map_err(|e| format!("sounds/{}.ron: {e}", p.id))
                })
                .collect::<Result<_, _>>()?,
            rig: ron::from_str(src.rig).map_err(|e| format!("hub/rig.ron: {e}"))?,
            networks: ron::from_str(src.networks).map_err(|e| format!("hub/networks.ron: {e}"))?,
            exchanges: ron::from_str(src.exchanges)
                .map_err(|e| format!("hub/exchanges.ron: {e}"))?,
            themes: ron::from_str(src.themes).map_err(|e| format!("themes.ron: {e}"))?,
            displays: ron::from_str(src.displays).map_err(|e| format!("displays.ron: {e}"))?,
        };
        data.validate()?;
        Ok(data)
    }

    /// Cross-file checks: every id a table names must exist, and nothing may be empty.
    pub fn validate(&self) -> Result<(), String> {
        self.welcome.validate(self)?;
        self.starts.validate(self)?;
        for (n, i) in self.intros.iter().enumerate() {
            i.validate(self)?;
            // A crew signs with one tag everywhere.
            if let Some(t) = &i.tag
                && let Some(o) = self.intros[..n]
                    .iter()
                    .find(|o| o.group == i.group && o.tag.as_ref().is_some_and(|ot| ot != t))
            {
                return Err(format!(
                    "intro `{}`: {} signs as `{t}` here but `{}` on `{}`",
                    i.id,
                    i.group,
                    o.tag.as_deref().unwrap_or(""),
                    o.id
                ));
            }
        }
        for t in &self.tools {
            if let Some(id) = &t.intro
                && self.intro(id).is_none()
            {
                return Err(format!("tool `{}` names unknown intro `{id}`", t.id));
            }
        }
        self.validate_games()
            .map_err(|e| format!("hub/games.ron: {e}"))?;
        self.validate_tunes()?;
        self.validate_compilations()
            .map_err(|e| format!("hub/compilations.ron: {e}"))?;
        for (i, w) in self.warez.iter().enumerate() {
            if self.warez[..i].iter().any(|o| o.id == w.id) {
                return Err(format!("duplicate warez id `{}`", w.id));
            }
        }
        for ice in &self.ice {
            self.validate_ice_program(ice)
                .map_err(|e| format!("ICE `{}`: {e}", ice.id))?;
            if ice.tier > 2 {
                return Err(format!(
                    "ICE `{}` has tier {} (0, 1 or 2)",
                    ice.id, ice.tier
                ));
            }
            if let Some(y) = &ice.yields
                && (y.below == 0 || y.below >= 100 || y.chance == 0 || y.chance > 100)
            {
                return Err(format!(
                    "ICE `{}` yields below {}% at {}%: both must be 1-99/1-100",
                    ice.id, y.below, y.chance
                ));
            }
        }
        for tier in [1, 2] {
            if !self.ice.iter().any(|i| i.tier == tier) {
                return Err(format!("no ICE of tier {tier}"));
            }
        }
        for (name, table) in [
            ("ice", &self.loot.ice),
            ("black_ice", &self.loot.black_ice),
            ("vault", &self.loot.vault),
        ] {
            if table.entries.iter().all(|e| e.weight == 0) {
                return Err(format!("loot table `{name}` has no weighted entries"));
            }
            for e in &table.entries {
                if let Drop::Warez(id) = &e.drop
                    && self.warez_ix(id).is_none()
                {
                    return Err(format!("loot table `{name}` names unknown warez `{id}`"));
                }
                if let Drop::Credits(lo, hi) = e.drop
                    && lo > hi
                {
                    return Err(format!("loot table `{name}` has credits {lo} > {hi}"));
                }
            }
        }
        self.validate_boards()?;
        crate::hub::IconDef::validate(&self.icons).map_err(|e| format!("hub/icons.ron: {e}"))?;
        self.liza
            .validate()
            .map_err(|e| format!("hub/liza.ron: {e}"))?;
        crate::pattern::PatternDef::validate(&self.patterns)
            .map_err(|e| format!("patterns.ron: {e}"))?;
        for s in &self.netmap {
            let p = s.paint.as_str();
            if p != crate::netview::PAINT_PAPER
                && p != crate::netview::PAINT_SOLID
                && self.pattern(p).is_none()
            {
                return Err(format!(
                    "netmap.ron: {:?} paints with unknown pattern `{p}`",
                    s.node
                ));
            }
        }
        crate::netview::MapStyleDef::validate(&self.netmap)
            .map_err(|e| format!("netmap.ron: {e}"))?;
        crate::hub::UpgradeDef::validate(&self.rig).map_err(|e| format!("hub/rig.ron: {e}"))?;
        self.networks
            .validate()
            .map_err(|e| format!("hub/networks.ron: {e}"))?;
        self.validate_themes()
            .map_err(|e| format!("themes.ron: {e}"))?;
        crate::display::DisplayDef::validate(&self.displays, &self.themes)
            .map_err(|e| format!("displays.ron: {e}"))?;
        self.hardware
            .validate(self)
            .map_err(|e| format!("hub/hardware.ron: {e}"))?;
        crate::desktop::AddonDef::validate(&self.addons)
            .map_err(|e| format!("hub/desktops.ron: {e}"))?;
        crate::panes::PaneDef::validate(&self.panes).map_err(|e| format!("hub/panes.ron: {e}"))?;
        for a in &self.art {
            a.validate().map_err(|e| format!("art/{}.txt {e}", a.id))?;
        }
        for d in &self.hardware.decks {
            if let Some(id) = &d.art
                && self.art(id).is_none()
            {
                return Err(format!(
                    "hub/hardware.ron: deck `{}` shows art `{id}`, but there's no data/art/{id}.txt",
                    d.id
                ));
            }
        }
        self.shopper
            .validate(self)
            .map_err(|e| format!("hub/shopper.ron: {e}"))?;
        self.disk
            .validate(self)
            .map_err(|e| format!("hub/disk.ron: {e}"))?;
        crate::system::FetchDef::validate(&self.fetch)
            .map_err(|e| format!("hub/fetch.ron: {e}"))?;
        crate::system::PanelIconDef::validate(&self.panel_icons)
            .map_err(|e| format!("hub/panel_icons.ron: {e}"))?;
        if self.sounds.is_empty() {
            return Err("data/sounds/ has no sound packs".into());
        }
        for p in &self.sounds {
            crate::cue::SoundDef::validate(&p.sounds)
                .and_then(|()| crate::music::validate(&p.music, &self.networks))
                .map_err(|e| format!("sounds/{}.ron: {e}", p.id))?;
        }
        crate::hub::RangeDef::validate(&self.exchanges, &self.networks)
            .map_err(|e| format!("hub/exchanges.ron: {e}"))?;
        // Every hint anywhere must name a range that exists.
        // Drops that turn up on a run (loot and encounters), then the boards' files.
        let mut drops: Vec<(String, &Drop)> = Vec::new();
        for (name, t) in [
            ("ice", &self.loot.ice),
            ("black_ice", &self.loot.black_ice),
            ("vault", &self.loot.vault),
        ] {
            drops.extend(
                t.entries
                    .iter()
                    .map(|e| (format!("loot table `{name}`"), &e.drop)),
            );
        }
        for ev in &self.events {
            for c in &ev.choices {
                match &c.effect {
                    ChoiceEffect::Pay { get: d, .. } | ChoiceEffect::Gamble { win: d, .. } => {
                        drops.push((format!("event `{}`", ev.id), d));
                    }
                    ChoiceEffect::Fight { rewards, .. } => {
                        drops.extend(rewards.iter().map(|d| (format!("event `{}`", ev.id), d)));
                    }
                    _ => {}
                }
            }
        }
        // A game is a crew's crack posted on a board: it never drops from ICE, a vault or an
        // encounter.
        if let Some((place, _)) = drops.iter().find(|(_, d)| matches!(d, Drop::Game(_))) {
            return Err(format!(
                "{place} drops a game: games only come from a board's files"
            ));
        }
        // A music disk is a crew's release, posted on a board too.
        if let Some((place, _)) = drops.iter().find(|(_, d)| matches!(d, Drop::Tune(_))) {
            return Err(format!(
                "{place} drops a tune: music disks only come from a board's files"
            ));
        }
        for b in &self.boards {
            for a in &b.areas {
                drops.extend(
                    a.files
                        .iter()
                        .map(|f| (format!("bbs/{} file `{}`", b.id, f.file), &f.item)),
                );
            }
        }
        for (place, d) in drops {
            if let Drop::Hint(r) = d
                && !self.exchanges.iter().any(|x| &x.id == r)
            {
                return Err(format!("{place} hints at unknown range `{r}`"));
            }
        }
        if self.contracts.is_empty() {
            return Err("contracts.ron has no clients".into());
        }
        for b in &self.betrayals {
            if b.weight == 0 {
                return Err(format!("betrayal `{}` has weight 0", b.id));
            }
            if b.label.is_empty() || b.label.chars().count() > 10 {
                return Err(format!(
                    "betrayal `{}`: label must be 1-10 characters",
                    b.id
                ));
            }
            match &b.effect {
                BetrayalEffect::Stiffed(pct) if *pct >= 100 => {
                    return Err(format!("betrayal `{}`: Stiffed must pay under 100%", b.id));
                }
                BetrayalEffect::Setup(ice) if self.ice_by_id(ice).is_none() => {
                    return Err(format!("betrayal `{}` rezzes unknown ICE `{ice}`", b.id));
                }
                _ => {}
            }
        }
        // Tier 1 stays friendly: something always fits it, and it's only ever money.
        let t1: Vec<&BetrayalDef> = self.betrayals.iter().filter(|b| b.min_tier <= 1).collect();
        if !t1.is_empty()
            && t1
                .iter()
                .any(|b| !matches!(b.effect, BetrayalEffect::Stiffed(_)))
        {
            return Err("tier 1 betrayals may only stiff you".into());
        }
        if self.events.is_empty() {
            return Err("events.ron has no events".into());
        }
        for ev in &self.events {
            if !ev.choices.iter().any(|c| c.effect == ChoiceEffect::Leave) {
                return Err(format!("event `{}` has no Leave choice", ev.id));
            }
            for c in &ev.choices {
                let drops: Vec<&Drop> = match &c.effect {
                    ChoiceEffect::Pay { get, .. } => vec![get],
                    ChoiceEffect::Gamble { win, .. } => vec![win],
                    ChoiceEffect::Fight { rewards, .. } => rewards.iter().collect(),
                    _ => Vec::new(),
                };
                for d in drops {
                    if let Drop::Warez(id) = d
                        && self.warez_ix(id).is_none()
                    {
                        return Err(format!("event `{}` names unknown warez `{id}`", ev.id));
                    }
                }
                match &c.effect {
                    ChoiceEffect::Trade { pool } => {
                        if pool.is_empty() {
                            return Err(format!("event `{}` trades from an empty pool", ev.id));
                        }
                        if let Some(id) = pool.iter().find(|id| self.warez_ix(id).is_none()) {
                            return Err(format!("event `{}` trades unknown warez `{id}`", ev.id));
                        }
                    }
                    ChoiceEffect::Fight { ice, .. } if !self.ice.iter().any(|i| &i.id == ice) => {
                        return Err(format!("event `{}` fights unknown ICE `{ice}`", ev.id));
                    }
                    ChoiceEffect::Gamble { pct, .. } if *pct > 100 => {
                        return Err(format!("event `{}` gambles at {pct}%", ev.id));
                    }
                    _ => {}
                }
            }
        }
        // Every factory program must be a legal binary, or the hex editor's rules lie.
        for w in &self.warez {
            let bin = crate::binary::encode(w, 0);
            if let Err(f) = crate::binary::check(&bin, 0) {
                return Err(format!(
                    "warez `{}` breaks the binary rules: {}",
                    w.id,
                    f.join("; ")
                ));
            }
        }
        let default_free = &self.starts.kit("").free;
        if default_free
            .iter()
            .map(String::as_str)
            .ne(crate::tuning::START_WAREZ.iter().copied())
        {
            return Err("tuning::START_WAREZ must match the default kit's free programs".into());
        }
        for id in crate::tuning::START_WAREZ
            .iter()
            .chain(crate::tuning::CLASSIC_WAREZ)
        {
            if self.warez_ix(id).is_none() {
                return Err(format!("starting warez `{id}` is not defined"));
            }
        }
        Ok(())
    }

    pub fn display(&self, id: &str) -> Option<&crate::display::DisplayDef> {
        self.displays.iter().find(|d| d.id == id)
    }

    /// A board's index in `boards` by id.
    pub fn board(&self, id: &str) -> Option<usize> {
        self.boards.iter().position(|b| b.id == id)
    }

    fn validate_boards(&self) -> Result<(), String> {
        if self.boards.iter().filter(|b| b.default).count() != 1 {
            return Err("exactly one bbs must be `default: true`".into());
        }
        for (i, b) in self.boards.iter().enumerate() {
            let err = |e: String| format!("bbs/{}: {e}", b.id);
            if self.boards[..i].iter().any(|o| o.id == b.id) {
                return Err(err("listed twice".into()));
            }
            if let Some(o) = self.boards[..i].iter().find(|o| o.phone == b.phone) {
                return Err(err(format!("shares its number with {}", o.id)));
            }
            b.validate().map_err(err)?;
            if let Some(t) = b.tiers.iter().find(|&&t| self.networks.tier(t).is_none()) {
                return Err(err(format!("names unknown tier {t}")));
            }
            let effects = b.bulletins.iter().filter_map(|x| x.on_read.as_ref()).chain(
                b.messages
                    .iter()
                    .flat_map(|m| m.posts.iter().filter_map(|p| p.on_read.as_ref())),
            );
            for e in effects {
                match e {
                    ReadEffect::Board(id) if self.board(id).is_none() => {
                        return Err(err(format!("a post names unknown board `{id}`")));
                    }
                    ReadEffect::Hint(r) if !self.exchanges.iter().any(|x| x.id == *r) => {
                        return Err(err(format!("a post names unknown range `{r}`")));
                    }
                    _ => {}
                }
            }
            for area in &b.areas {
                if area.files.is_empty() || area.stock == 0 {
                    return Err(err(format!("area `{}` stocks nothing", area.name)));
                }
                for f in &area.files {
                    if let Drop::Warez(id) = &f.item
                        && self.warez_ix(id).is_none()
                    {
                        return Err(err(format!("file `{}` names unknown warez `{id}`", f.file)));
                    }
                    if matches!(f.item, Drop::Nothing | Drop::Credits(..)) {
                        return Err(err(format!("file `{}` sells nothing", f.file)));
                    }
                    if let Drop::Tool(id) = &f.item
                        && self.tool(id).is_none()
                    {
                        return Err(err(format!("file `{}` names unknown tool `{id}`", f.file)));
                    }
                    if let Drop::Tune(id) = &f.item
                        && self.tune(id).is_none()
                    {
                        return Err(err(format!("file `{}` names unknown tune `{id}`", f.file)));
                    }
                    if let Drop::Game(id) = &f.item {
                        let Some(g) = self.game(id) else {
                            return Err(err(format!(
                                "file `{}` names unknown game `{id}`",
                                f.file
                            )));
                        };
                        // A crack is posted where its crew posts it, named the way a sysop
                        // names a crack: 8.3, the stem ending in the crew's tag.
                        let crack = self.intro(&g.crack);
                        if crack.and_then(|i| i.board.as_deref()) != Some(b.id.as_str()) {
                            return Err(err(format!(
                                "file `{}`: {id}'s crack is posted on its crew's board, not here",
                                f.file
                            )));
                        }
                        let tag = crack.and_then(|i| i.tag.as_deref()).unwrap_or("");
                        let ok = f.file.split_once('.').is_some_and(|(stem, ext)| {
                            stem.len() <= 8 && ext.len() <= 3 && stem.ends_with(&format!("-{tag}"))
                        });
                        if !ok {
                            return Err(err(format!(
                                "file `{}`: a crack is named in 8.3 ending in its crew's tag (-{tag})",
                                f.file
                            )));
                        }
                    }
                    if let Drop::Text(id) = &f.item {
                        if !b.texts.iter().any(|t| &t.id == id) {
                            return Err(err(format!(
                                "file `{}` names unknown text `{id}`",
                                f.file
                            )));
                        }
                        if f.price != 0 {
                            return Err(err(format!("textfile `{}` must be free", f.file)));
                        }
                    }
                }
            }
        }
        for t in &self.networks.traits {
            if let crate::hub::TraitEffect::Board(id) = &t.effect
                && self.board(id).is_none()
            {
                return Err(format!(
                    "hub/networks.ron: trait `{}` names unknown board `{id}`",
                    t.id
                ));
            }
        }
        Ok(())
    }

    /// Every tune: a unique id, a title that fits a playlist row, an author who signs a
    /// cracktro (a crew, or a publisher's splash), a sound pattern whose loop fits
    /// [`crate::music::MAX_LOOP_SECS`], and a length of at least one loop.
    fn validate_tunes(&self) -> Result<(), String> {
        for (i, t) in self.tunes.iter().enumerate() {
            let err = |m: String| Err(format!("music/{}.ron: {m}", t.id));
            if self.tunes[..i].iter().any(|o| o.id == t.id) {
                return err("listed twice".into());
            }
            let title = t.title.chars().count();
            if title == 0 || title > TUNE_TITLE_MAX {
                return err(format!("a title is 1 to {TUNE_TITLE_MAX} characters"));
            }
            if !self.signs(&t.by) {
                return err(format!("`{}` signs no cracktro in data/intros/", t.by));
            }
            t.tune.validate().or_else(err)?;
            let secs = t.tune.loop_secs(0);
            if secs > crate::music::MAX_LOOP_SECS {
                return err(format!(
                    "loops after {secs:.0} s (keep it under {} s)",
                    crate::music::MAX_LOOP_SECS
                ));
            }
            if (t.secs as f32) < secs || t.secs > TUNE_SECS_MAX {
                return err(format!(
                    "plays {} s: at least one loop ({secs:.0} s), at most {TUNE_SECS_MAX}",
                    t.secs
                ));
            }
        }
        Ok(())
    }

    /// Every compilation: a unique id, a price, a publisher that advertises in CPU Shopper,
    /// and one or more tunes that exist, each once.
    fn validate_compilations(&self) -> Result<(), String> {
        for (i, c) in self.compilations.iter().enumerate() {
            let err = |m: String| Err(format!("`{}`: {m}", c.id));
            if self.compilations[..i].iter().any(|o| o.id == c.id) {
                return err("listed twice".into());
            }
            if c.price == 0 {
                return err("a boxed disk has a price".into());
            }
            if !self.shopper.vendors.iter().any(|v| v.id == c.publisher) {
                return err(format!("unknown publisher `{}`", c.publisher));
            }
            if c.tunes.is_empty() {
                return err("holds no tunes".into());
            }
            for (n, t) in c.tunes.iter().enumerate() {
                if self.tune(t).is_none() {
                    return err(format!("names unknown tune `{t}`"));
                }
                if c.tunes[..n].contains(t) {
                    return err(format!("holds `{t}` twice"));
                }
            }
        }
        Ok(())
    }

    /// The compilation called `id` (in `hub/compilations.ron`).
    pub fn compilation(&self, id: &str) -> Option<&CompilationDef> {
        self.compilations.iter().find(|c| c.id == id)
    }

    /// Whether `who` signs a cracktro or a splash: a group, or one of the names a group
    /// joins with ` & ` (`m0th & the NiGHTMOTHS` is m0th and the NiGHTMOTHS).
    pub fn signs(&self, who: &str) -> bool {
        self.intros
            .iter()
            .any(|i| i.group == who || i.group.split(" & ").any(|g| g == who))
    }

    /// The tune called `id` (a file in `data/music/`).
    pub fn tune(&self, id: &str) -> Option<&TuneFile> {
        self.tunes.iter().find(|t| t.id == id)
    }

    /// An ICE's program must parse in its own vocabulary, every GOTO must land, every
    /// `TARGET RAN` must name a program that exists, a verb must be reachable from the top, and
    /// it must have budgets to run with.
    fn validate_ice_program(&self, ice: &IceDef) -> Result<(), String> {
        use crate::basic::{Cond, Stmt, Vocab};
        if ice.cpu == 0 || ice.ram == 0 {
            return Err(format!(
                "cpu {} and ram {} must both be at least 1",
                ice.cpu, ice.ram
            ));
        }
        if ice.verbs.is_empty() {
            return Err("has no verbs".into());
        }
        let p = crate::basic::parse_with(&ice.program, Vocab::Ice(&ice.verbs));
        if let Some(s) = p.stray.first() {
            return Err(format!("program line `{s}` has no line number"));
        }
        if !p.can_act() {
            return Err("program never acts: no verb can be reached from the top".into());
        }
        for l in &p.lines {
            if let Some(why) = p.fault(l.no) {
                return Err(format!("program line {} `{}`: {why}", l.no, l.text));
            }
            if let Stmt::If(Cond::TargetRan(name)) = &l.stmt
                && !self
                    .warez
                    .iter()
                    .any(|w| crate::basic::names(name, &w.name, &w.id))
            {
                return Err(format!(
                    "program line {} reads `{name}`, which no program is called",
                    l.no
                ));
            }
        }
        Ok(())
    }

    /// Every engine has a game, every game's field fits its engine and a screen, its tick is
    /// sane, its crack is a crack and
    /// its splash a splash, and its publisher advertises in CPU Shopper.
    fn validate_games(&self) -> Result<(), String> {
        for (i, g) in self.games.iter().enumerate() {
            let err = |m: String| Err(format!("game `{}`: {m}", g.id));
            if self.games[..i].iter().any(|o| o.id == g.id) {
                return err("listed twice".into());
            }
            if crate::hub::GameId::new(&g.id).is_none() {
                return err(format!(
                    "an id is 1-{} ASCII characters",
                    crate::hub::GAME_ID_MAX
                ));
            }
            crate::hub::IconDef::check_art(&g.id, &g.icon).or_else(err)?;
            let (cols, rows) = (crate::bbs::COLS as u32, crate::bbs::ROWS as u32);
            let (min_c, min_r) = g.engine.min_size();
            if !(min_c..=cols).contains(&g.cols) || !(min_r..=rows).contains(&g.rows) {
                return err(format!(
                    "a {}x{} field ({:?} plays on {min_c}-{cols} x {min_r}-{rows})",
                    g.cols, g.rows, g.engine
                ));
            }
            if !GAME_TICK_MS.contains(&g.tick_ms) {
                return err(format!(
                    "tick_ms {} ({}-{})",
                    g.tick_ms,
                    GAME_TICK_MS.start(),
                    GAME_TICK_MS.end()
                ));
            }
            match self.intro(&g.crack) {
                None => return err(format!("unknown crack intro `{}`", g.crack)),
                Some(i) if i.is_splash() => {
                    return err(format!("crack intro `{}` is a splash", g.crack));
                }
                Some(i) if i.tag.is_none() => {
                    return err(format!("crack intro `{}` has no crew tag", g.crack));
                }
                _ => {}
            }
            match self.intro(&g.splash) {
                None => return err(format!("unknown splash intro `{}`", g.splash)),
                Some(i) if !i.is_splash() => {
                    return err(format!("splash intro `{}` is a crack", g.splash));
                }
                // The publisher prints the product and its version: they must be this row's.
                Some(i) => {
                    let title = format!("{} {}", g.name, g.version);
                    if !i.tagline.as_deref().unwrap_or("").contains(&title) {
                        return err(format!(
                            "splash `{}`'s tagline doesn't name `{title}`",
                            g.splash
                        ));
                    }
                }
            }
            if self.shopper.vendor(&g.publisher).is_none() {
                return err(format!("unknown publisher `{}`", g.publisher));
            }
            if g.shop.as_ref().is_some_and(|s| s.price == 0) {
                return err("sold for nothing".into());
            }
        }
        if let Some(e) = crate::games::Engine::ALL
            .iter()
            .find(|e| !self.games.iter().any(|g| g.engine == **e))
        {
            return Err(format!("no game runs the {e:?} engine"));
        }
        Ok(())
    }

    /// A picture from `data/art/` by id.
    pub fn art(&self, id: &str) -> Option<&ArtDef> {
        self.art.iter().find(|a| a.id == id)
    }

    /// A sound pack's cues by id.
    pub fn sound_pack(&self, id: &str) -> Option<&[crate::cue::SoundDef]> {
        self.sounds
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.sounds.as_slice())
    }

    /// The run tune sound pack `pack` plays on a tier-`tier` network, if it has one.
    pub fn run_tune(&self, pack: &str, tier: u32) -> Option<&crate::music::RunTune> {
        self.sounds
            .iter()
            .find(|p| p.id == pack)?
            .music
            .iter()
            .find(|m| m.tier == tier)
    }

    pub fn theme(&self, id: &str) -> Option<&ThemeDef> {
        self.themes.iter().find(|t| t.id == id)
    }

    fn validate_themes(&self) -> Result<(), String> {
        for (i, t) in self.themes.iter().enumerate() {
            if self.themes[..i].iter().any(|o| o.id == t.id) {
                return Err(format!("theme `{}` is listed twice", t.id));
            }
            if let Some(bad) = t.colors.all().into_iter().find(|c| rgb(c).is_none()) {
                return Err(format!(
                    "theme `{}` has a bad colour `{bad}` (want #rrggbb)",
                    t.id
                ));
            }
        }
        match self.theme(crate::hub::DEFAULT_RUN_THEME) {
            Some(_) => Ok(()),
            None => Err(format!(
                "the default run theme `{}` must exist",
                crate::hub::DEFAULT_RUN_THEME
            )),
        }
    }

    pub fn pattern(&self, id: &str) -> Option<&crate::pattern::PatternDef> {
        self.patterns.iter().find(|p| p.id == id)
    }

    pub fn intro(&self, id: &str) -> Option<&crate::intro::IntroDef> {
        self.intros.iter().find(|i| i.id == id)
    }

    pub fn tool(&self, id: &str) -> Option<&ToolDef> {
        self.tools.iter().find(|t| t.id == id)
    }

    pub fn game(&self, id: &str) -> Option<&GameDef> {
        self.games.iter().find(|g| g.id == id)
    }

    /// Every tune's id with its title.
    pub fn tune_titles(&self) -> Vec<(String, String)> {
        self.tunes
            .iter()
            .map(|t| (t.id.clone(), t.title.clone()))
            .collect()
    }

    /// Every game's id with its crack's label.
    pub fn crack_labels(&self) -> Vec<(String, String)> {
        self.games
            .iter()
            .map(|g| {
                (
                    g.id.clone(),
                    self.game_label(&g.id, crate::hub::GameCopy::Cracked),
                )
            })
            .collect()
    }

    /// What a copy of a game is called: a box is the plain name (`Nibbler`), a crack the name
    /// in capitals with its crew's tag (`NIBBLER [2K4]`), as it came off the board.
    pub fn game_label(&self, id: &str, copy: crate::hub::GameCopy) -> String {
        let Some(g) = self.game(id) else {
            return id.to_string();
        };
        match copy {
            crate::hub::GameCopy::Boxed => g.name.clone(),
            crate::hub::GameCopy::Cracked => {
                let name = g.name.to_uppercase();
                match self.intro(&g.crack).and_then(|i| i.tag.as_deref()) {
                    Some(tag) => format!("{name} [{tag}]"),
                    None => name,
                }
            }
        }
    }

    pub fn warez_ix(&self, id: &str) -> Option<usize> {
        self.warez.iter().position(|w| w.id == id)
    }

    pub fn ice_by_id(&self, id: &str) -> Option<usize> {
        self.ice.iter().position(|i| i.id == id)
    }
}
