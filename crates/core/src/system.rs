//! System: the deck's settings app (it replaced Prefs). Its panels only draw; what they report
//! is worked out here, from the rules (`tuning`, `rig.ron`, the loadout, programs' binaries),
//! and tested headlessly.

use serde::{Deserialize, Serialize};

use crate::binary;
use crate::data::Data;
use crate::hub::{Save, Upgrade};

/// System's panels, in the order its list shows them.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Panel {
    /// About This Deck: the OS, the specs, installed software and the switches.
    #[default]
    Overview,
    Monitor,
    Theme,
    /// The HDD as a block map: the loadout, the stash beside it, and the RAM it'll load into.
    Disk,
}

impl Panel {
    pub const ALL: [Panel; 4] = [Panel::Overview, Panel::Monitor, Panel::Theme, Panel::Disk];

    pub fn label(self) -> &'static str {
        match self {
            Panel::Overview => "About This Deck",
            Panel::Monitor => "Monitor",
            Panel::Theme => "Theme",
            Panel::Disk => "Disk",
        }
    }
}

/// A panel's icon in Liza's Control Panel list (`data/hub/panel_icons.ron`): 1-bit pixel art,
/// `k` ink, `w` paper, `.` clear.
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PanelIconDef {
    pub panel: Panel,
    pub pixels: Vec<String>,
}

/// A panel icon's size in pixels: square, like the Control Panel's.
pub const PANEL_ICON: usize = 16;

impl PanelIconDef {
    pub fn validate(defs: &[PanelIconDef]) -> Result<(), String> {
        for p in Panel::ALL {
            let n = defs.iter().filter(|d| d.panel == p).count();
            if n != 1 {
                return Err(format!("{p:?} has {n} icons, not one"));
            }
        }
        for d in defs {
            if d.pixels.len() != PANEL_ICON {
                return Err(format!("{:?}'s icon has {} rows", d.panel, d.pixels.len()));
            }
            for (y, row) in d.pixels.iter().enumerate() {
                if row.chars().count() != PANEL_ICON {
                    return Err(format!(
                        "{:?}'s icon row {y} isn't {PANEL_ICON} wide",
                        d.panel
                    ));
                }
                if let Some(c) = row.chars().find(|c| !".kw".contains(*c)) {
                    return Err(format!("{:?}'s icon has {c:?}: only k, w and .", d.panel));
                }
            }
        }
        Ok(())
    }
}

/// A panel's icon.
pub fn panel_icon(data: &Data, panel: Panel) -> &[String] {
    data.panel_icons
        .iter()
        .find(|d| d.panel == panel)
        .map_or(&[], |d| d.pixels.as_slice())
}

/// An OS's logo for the neofetch-style About panel (`data/hub/fetch.ron`).
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct FetchDef {
    pub desktop: crate::hardware::Desktop,
    pub logo: Vec<String>,
}

/// How big a logo may be: (columns, rows).
pub const LOGO_MAX: (usize, usize) = (20, 10);

impl FetchDef {
    pub fn validate(defs: &[FetchDef]) -> Result<(), String> {
        use crate::hardware::Desktop;
        for d in [Desktop::Workbench, Desktop::Tiling, Desktop::Liza] {
            let f = defs
                .iter()
                .find(|f| f.desktop == d)
                .ok_or_else(|| format!("no logo for {}", d.name()))?;
            if f.logo.is_empty() || f.logo.len() > LOGO_MAX.1 {
                return Err(format!("{}'s logo has {} rows", d.name(), f.logo.len()));
            }
            if let Some(l) = f.logo.iter().find(|l| l.chars().count() > LOGO_MAX.0) {
                return Err(format!("{}'s logo line is too wide: {l:?}", d.name()));
            }
        }
        Ok(())
    }
}

/// The logo for a desktop.
pub fn logo(data: &Data, desktop: crate::hardware::Desktop) -> &[String] {
    data.fetch
        .iter()
        .find(|f| f.desktop == desktop)
        .map_or(&[], |f| f.logo.as_slice())
}

/// One line of the deck's specs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spec {
    pub label: String,
    /// What the deck has now, with its unit ("3 cyc", "6 GB").
    pub value: String,
    /// A note on how it's used ("the loadout uses 4 GB", "5 of 6 filled").
    pub use_note: String,
    /// Upgrade level owned, out of the most there is.
    pub level: (u32, u32),
    /// What the next upgrade adds, if there's one left ("+1 cyc").
    pub next: Option<String>,
}

/// Everything About This Deck shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Overview {
    /// The desktop the deck runs ("Liza").
    pub os: String,
    /// The deck ("Sense/Net Personal Deck").
    pub deck: String,
    /// Its firmware ("Liza 1.0").
    pub firmware: String,
    pub specs: Vec<Spec>,
    /// Tools you own, with their versions: (name, version).
    pub software: Vec<(String, String)>,
}

/// About This Deck, for the deck plugged in now.
pub fn overview(save: &Save, data: &Data) -> Overview {
    let deck = save.deck_def(data);
    let fw = save.firmware_def(data);
    let defs = &data.rig;
    let def = |u: Upgrade| defs.iter().find(|d| d.kind == u);
    let unit = |u: Upgrade| def(u).map_or(String::new(), |d| d.unit.clone());
    let with_unit = |n: u32, u: Upgrade| {
        let unit = unit(u);
        if unit.is_empty() {
            n.to_string()
        } else {
            format!("{n} {unit}")
        }
    };
    let level = |u: Upgrade| (save.rig.level(u), def(u).map_or(0, |d| d.max));
    let next = |u: Upgrade| {
        let d = def(u)?;
        d.cost(save.rig.level(u))?;
        Some(format!("+{}", with_unit(d.step, u)))
    };
    let base = |u: Upgrade| save.stat_value(data, u);

    // What the loadout will load: its programs' RAM, from their binaries.
    let kit = save.kit(data);
    let ram = base(Upgrade::Ram);
    let ram_used: u32 = kit
        .programs
        .iter()
        .filter_map(|(_, bin)| binary::check(bin, save.key).ok())
        .map(|s| s.ram)
        .sum();
    let hdd = save.hdd_cap(data);
    let filled = kit.programs.len() as u32;
    let stash_cap = save.stash_cap(defs);

    let specs = vec![
        Spec {
            label: "CPU".into(),
            value: with_unit(base(Upgrade::Cpu), Upgrade::Cpu),
            use_note: "per turn".into(),
            level: level(Upgrade::Cpu),
            next: next(Upgrade::Cpu),
        },
        Spec {
            label: "RAM".into(),
            value: with_unit(ram, Upgrade::Ram),
            use_note: format!(
                "{} in use, {} free",
                with_unit(ram_used.min(ram), Upgrade::Ram),
                with_unit(ram.saturating_sub(ram_used), Upgrade::Ram)
            ),
            level: level(Upgrade::Ram),
            next: next(Upgrade::Ram),
        },
        Spec {
            label: "HDD".into(),
            value: with_unit(hdd, Upgrade::Hdd),
            use_note: format!(
                "{filled} of {hdd} filled, {}% fragmented",
                crate::disk::fragmentation(save, data)
            ),
            level: level(Upgrade::Hdd),
            next: next(Upgrade::Hdd),
        },
        Spec {
            label: "Integrity".into(),
            value: with_unit(base(Upgrade::Integrity), Upgrade::Integrity),
            use_note: String::new(),
            level: level(Upgrade::Integrity),
            next: next(Upgrade::Integrity),
        },
        Spec {
            label: "Stash".into(),
            value: with_unit(stash_cap, Upgrade::Stash),
            use_note: format!("{} of {stash_cap} used", save.stash.len()),
            level: level(Upgrade::Stash),
            next: next(Upgrade::Stash),
        },
    ];

    // The tools you own, then each copy of each game you own by its label, with versions.
    let tools = data.tools.iter().filter(|t| save.has_tool(&t.id));
    let games = data.games.iter().flat_map(|g| {
        save.copies(&g.id)
            .into_iter()
            .map(|c| (data.game_label(&g.id, c), g.version.clone()))
    });
    let software = tools
        .map(|t| (t.name.clone(), t.version.clone()))
        .chain(games)
        .collect();

    Overview {
        os: fw.desktop.name().to_string(),
        deck: deck.title(),
        firmware: fw.label.clone(),
        specs,
        software,
    }
}
