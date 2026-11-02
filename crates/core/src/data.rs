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
    Break { power: u32, vs: Option<IceClass> },
    Shield(u32),
    Repair(u32),
    Scrub(u32),
    Stun,
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
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct WarezDef {
    pub id: String,
    pub name: String,
    pub kind: WarezKind,
    pub ram: u32,
    pub cpu: u32,
    pub effect: Effect,
    pub blurb: String,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Intent {
    Attack(u32),
    Trace(u32),
    Fortify(u32),
}

impl Intent {
    pub fn describe(self) -> String {
        match self {
            Intent::Attack(n) => format!("ATTACK {n}"),
            Intent::Trace(n) => format!("TRACE +{n}"),
            Intent::Fortify(n) => format!("FORTIFY +{n}"),
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
    pub intents: Vec<Intent>,
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
    pub bulletins: Vec<BulletinDef>,
    pub users: Vec<BbsUser>,
    pub areas: Vec<BbsAreaDef>,
    /// Pipe-coded splash art, from the board's `splash.txt`.
    #[serde(skip)]
    pub splash: String,
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
    /// Sell a program for `per_ram` credits per GB of its ram cost.
    Sell {
        per_ram: u32,
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
    /// Workbench desktop icons.
    pub icons: Vec<crate::hub::IconDef>,
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
        let data = Self {
            warez: ron::from_str(src.warez).map_err(|e| format!("warez.ron: {e}"))?,
            ice: ron::from_str(src.ice).map_err(|e| format!("ice.ron: {e}"))?,
            loot: ron::from_str(src.loot).map_err(|e| format!("loot.ron: {e}"))?,
            boards,
            events: ron::from_str(src.events).map_err(|e| format!("events.ron: {e}"))?,
            contracts: ron::from_str(src.contracts).map_err(|e| format!("contracts.ron: {e}"))?,
            icons: ron::from_str(src.icons).map_err(|e| format!("hub/icons.ron: {e}"))?,
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
        for (i, w) in self.warez.iter().enumerate() {
            if self.warez[..i].iter().any(|o| o.id == w.id) {
                return Err(format!("duplicate warez id `{}`", w.id));
            }
        }
        for ice in &self.ice {
            if ice.intents.is_empty() {
                return Err(format!("ICE `{}` has no intents", ice.id));
            }
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
        crate::hub::UpgradeDef::validate(&self.rig).map_err(|e| format!("hub/rig.ron: {e}"))?;
        self.networks
            .validate()
            .map_err(|e| format!("hub/networks.ron: {e}"))?;
        self.validate_themes()
            .map_err(|e| format!("themes.ron: {e}"))?;
        crate::display::DisplayDef::validate(&self.displays, &self.themes)
            .map_err(|e| format!("displays.ron: {e}"))?;
        crate::hub::RangeDef::validate(&self.exchanges, &self.networks)
            .map_err(|e| format!("hub/exchanges.ron: {e}"))?;
        // Every hint anywhere must name a range that exists.
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
        for b in &self.boards {
            for a in &b.areas {
                drops.extend(
                    a.files
                        .iter()
                        .map(|f| (format!("bbs/{} file `{}`", b.id, f.file), &f.item)),
                );
            }
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
        for (place, d) in drops {
            if let Drop::Hint(r) = d
                && !self.exchanges.iter().any(|x| &x.id == r)
            {
                return Err(format!("{place} hints at unknown range `{r}`"));
            }
        }
        if self.contracts.is_empty() {
            return Err("contracts.ron has no templates".into());
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
        for id in crate::tuning::START_WAREZ {
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
            b.validate().map_err(err)?;
            if let Some(t) = b.tiers.iter().find(|&&t| self.networks.tier(t).is_none()) {
                return Err(err(format!("names unknown tier {t}")));
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

    pub fn warez_ix(&self, id: &str) -> Option<usize> {
        self.warez.iter().position(|w| w.id == id)
    }
}
