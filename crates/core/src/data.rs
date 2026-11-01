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
    pub tier: u32,
    pub strength: u32,
    pub intents: Vec<Intent>,
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum Drop {
    Warez(String),
    Credits(u32, u32),
    Ram(u32),
    Cpu(u32),
    Hdd(u32),
    Repair(u32),
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
    pub name: String,
    pub sysop: String,
    pub phone: String,
    pub baud: u32,
    pub bulletins: Vec<BulletinDef>,
    pub users: Vec<BbsUser>,
    pub areas: Vec<BbsAreaDef>,
    /// Pipe-coded splash art, from `bbs/splash.txt`.
    #[serde(skip)]
    pub splash: String,
}

/// The raw text of every data file.
#[derive(Clone, Copy, Debug)]
pub struct Sources<'a> {
    pub warez: &'a str,
    pub ice: &'a str,
    pub loot: &'a str,
    pub bbs: &'a str,
    pub splash: &'a str,
}

#[derive(Clone, Debug)]
pub struct Data {
    pub warez: Vec<WarezDef>,
    pub ice: Vec<IceDef>,
    pub loot: LootTables,
    pub bbs: BbsDef,
}

impl Data {
    /// The content shipped in `/data`.
    pub fn builtin() -> Result<Self, String> {
        Self::parse(Sources {
            warez: include_str!("../../../data/warez.ron"),
            ice: include_str!("../../../data/ice.ron"),
            loot: include_str!("../../../data/loot.ron"),
            bbs: include_str!("../../../data/bbs/board.ron"),
            splash: include_str!("../../../data/bbs/splash.txt"),
        })
    }

    pub fn parse(src: Sources) -> Result<Self, String> {
        let mut bbs: BbsDef = ron::from_str(src.bbs).map_err(|e| format!("bbs/board.ron: {e}"))?;
        bbs.splash = src.splash.to_string();
        let data = Self {
            warez: ron::from_str(src.warez).map_err(|e| format!("warez.ron: {e}"))?,
            ice: ron::from_str(src.ice).map_err(|e| format!("ice.ron: {e}"))?,
            loot: ron::from_str(src.loot).map_err(|e| format!("loot.ron: {e}"))?,
            bbs,
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
        for area in &self.bbs.areas {
            if area.files.is_empty() || area.stock == 0 {
                return Err(format!("bbs area `{}` stocks nothing", area.name));
            }
            for f in &area.files {
                if let Drop::Warez(id) = &f.item
                    && self.warez_ix(id).is_none()
                {
                    return Err(format!("bbs file `{}` names unknown warez `{id}`", f.file));
                }
                if matches!(f.item, Drop::Nothing | Drop::Credits(..)) {
                    return Err(format!("bbs file `{}` sells nothing", f.file));
                }
            }
        }
        for id in crate::tuning::START_WAREZ {
            if self.warez_ix(id).is_none() {
                return Err(format!("starting warez `{id}` is not defined"));
            }
        }
        Ok(())
    }

    pub fn warez_ix(&self, id: &str) -> Option<usize> {
        self.warez.iter().position(|w| w.id == id)
    }
}
