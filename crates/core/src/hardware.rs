//! Hardware you own: monitors (the CRT look) and decks (the UX). Data in
//! `data/hub/hardware.ron`. A deck's UX is put together from named parts, so a new UX version
//! is a data entry and a new kind of part is one renderer plus one name.

use serde::{Deserialize, Serialize};

use crate::data::Data;
use crate::netview::MapView;

/// How the run screen is dressed: its panes, menus, status line and default layout. A new
/// chrome is a name here plus a `ChromeView` in the app's `chrome.rs`; anything it doesn't
/// draw itself falls back to `Classic`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Chrome {
    /// Plain IDE panes.
    #[default]
    Classic,
    /// btop-style: boxed panes, gradient meters, dot graphs and waterfalls (the hacker deck).
    Btop,
    /// A 90s productivity suite: forms, spreadsheets, report charts, an F-key legend and
    /// dialogs (the corpo deck).
    Suite,
    /// A HyperCard stack: a card, the Message box, pattern meters, rounded buttons and Mac
    /// alerts, 1-bit (the starter deck).
    Stack,
}

impl Chrome {
    pub const ALL: [Chrome; 4] = [Chrome::Classic, Chrome::Btop, Chrome::Suite, Chrome::Stack];
}

/// The overworld's look. A new desktop is a new name here, a renderer in the app's
/// `desktops` registry, and (optionally) add-ons in `data/hub/desktops.ron`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Desktop {
    /// "Desktop", after Amiga Workbench 1.3: icons and windows you drag (the Hosaka).
    #[default]
    Workbench,
    /// "AnarchyOS", after Omarchy: a keyboard-first tiling desktop with a top bar and a
    /// launcher (the Ono-Sendai).
    Tiling,
    /// "Liza", after the first Mac and the Lisa: a 1-bit menu bar, Finder windows and a Trash
    /// (the Sense/Net).
    Liza,
}

impl Desktop {
    /// The OS's name, shown at boot, on the desktop and in Prefs.
    pub fn name(self) -> &'static str {
        match self {
            Desktop::Workbench => "Desktop",
            Desktop::Tiling => "AnarchyOS",
            Desktop::Liza => "Liza",
        }
    }
}

/// One UX version of a deck: which part it uses for each piece of the screen.
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct FirmwareDef {
    pub id: String,
    pub label: String,
    pub map: MapView,
    pub chrome: Chrome,
    pub desktop: Desktop,
    /// The sound pack (`data/sounds/<id>.ron`) its cues play from.
    #[serde(default = "default_sounds")]
    pub sounds: String,
    /// Secondary panes (ids in `data/hub/panes.ron`) that come with this firmware.
    #[serde(default)]
    pub panes: Vec<String>,
}

fn default_sounds() -> String {
    "ono".into()
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct DeckDef {
    pub id: String,
    pub name: String,
    pub maker: String,
    pub price: u32,
    pub blurb: String,
    #[serde(default)]
    pub starter: bool,
    pub firmware: Vec<FirmwareDef>,
    /// Its picture in ads (`data/art/<id>.txt`), if it has one.
    #[serde(default)]
    pub art: Option<String>,
}

impl DeckDef {
    /// "Ono-Sendai Cyberspace VII".
    pub fn title(&self) -> String {
        format!("{} {}", self.maker, self.name)
    }
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct MonitorDef {
    pub id: String,
    pub name: String,
    pub maker: String,
    pub price: u32,
    pub blurb: String,
    /// The CRT display preset it shows (`data/displays.ron`).
    pub display: String,
    /// A colour tube shows any theme; a mono one only its display's theme.
    pub colour: bool,
    #[serde(default)]
    pub starter: bool,
}

impl MonitorDef {
    pub fn title(&self) -> String {
        format!("{} {}", self.maker, self.name)
    }

    /// The one theme a mono tube can show; `None` for colour.
    pub fn locked_theme<'a>(&self, data: &'a Data) -> Option<&'a str> {
        if self.colour {
            return None;
        }
        data.display(&self.display).and_then(|d| d.theme.as_deref())
    }
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct HardwareDef {
    pub monitors: Vec<MonitorDef>,
    pub decks: Vec<DeckDef>,
}

impl HardwareDef {
    pub fn monitor(&self, id: &str) -> Option<&MonitorDef> {
        self.monitors.iter().find(|m| m.id == id)
    }

    pub fn deck(&self, id: &str) -> Option<&DeckDef> {
        self.decks.iter().find(|d| d.id == id)
    }

    pub fn starter_monitor(&self) -> &MonitorDef {
        self.monitors
            .iter()
            .find(|m| m.starter)
            .unwrap_or(&self.monitors[0])
    }

    pub fn starter_deck(&self) -> &DeckDef {
        self.decks
            .iter()
            .find(|d| d.starter)
            .unwrap_or(&self.decks[0])
    }

    pub fn validate(&self, data: &Data) -> Result<(), String> {
        if self.monitors.iter().filter(|m| m.starter).count() != 1 {
            return Err("exactly one monitor must be the starter".into());
        }
        if self.decks.iter().filter(|d| d.starter).count() != 1 {
            return Err("exactly one deck must be the starter".into());
        }
        for (i, m) in self.monitors.iter().enumerate() {
            if self.monitors[..i].iter().any(|o| o.id == m.id) {
                return Err(format!("monitor `{}` is listed twice", m.id));
            }
            let Some(d) = data.display(&m.display) else {
                return Err(format!(
                    "monitor `{}` shows unknown display `{}`",
                    m.id, m.display
                ));
            };
            if !m.colour && d.theme.is_none() {
                return Err(format!(
                    "mono monitor `{}` needs a display with a theme to lock to",
                    m.id
                ));
            }
        }
        for (i, d) in self.decks.iter().enumerate() {
            if self.decks[..i].iter().any(|o| o.id == d.id) {
                return Err(format!("deck `{}` is listed twice", d.id));
            }
            if d.firmware.is_empty() {
                return Err(format!("deck `{}` has no firmware", d.id));
            }
            for (j, f) in d.firmware.iter().enumerate() {
                if data.sound_pack(&f.sounds).is_none() {
                    return Err(format!(
                        "deck `{}` firmware `{}` plays unknown sound pack `{}`",
                        d.id, f.id, f.sounds
                    ));
                }
                if let Some(p) = f
                    .panes
                    .iter()
                    .find(|p| !data.panes.iter().any(|d| &d.id == *p))
                {
                    return Err(format!(
                        "deck `{}` firmware `{}` bundles unknown pane `{p}`",
                        d.id, f.id
                    ));
                }
                if d.firmware[..j].iter().any(|o| o.id == f.id) {
                    return Err(format!("deck `{}` lists firmware `{}` twice", d.id, f.id));
                }
            }
        }
        Ok(())
    }
}

/// Something the HARDWARE shop sells.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Hardware {
    Monitor(String),
    Deck(String),
}

use crate::hub::Save;

impl Save {
    /// Gives a save its starter monitor and deck, and makes sure what's plugged in is owned
    /// and valid (old saves have none; data can change). A mono monitor sets its theme.
    /// Returns true if anything changed.
    pub fn ensure_hardware(&mut self, data: &Data) -> bool {
        let hw = &data.hardware;
        let before = (
            self.monitors.clone(),
            self.monitor.clone(),
            self.decks.clone(),
            self.deck.clone(),
            self.firmware.clone(),
            self.phosphor.clone(),
        );
        self.monitors.retain(|m| hw.monitor(m).is_some());
        self.decks.retain(|d| hw.deck(d).is_some());
        let (m0, d0) = (
            hw.starter_monitor().id.clone(),
            hw.starter_deck().id.clone(),
        );
        if !self.monitors.contains(&m0) {
            self.monitors.insert(0, m0.clone());
        }
        if !self.decks.contains(&d0) {
            self.decks.insert(0, d0.clone());
        }
        if !self.monitors.contains(&self.monitor) {
            self.monitor = m0;
        }
        if !self.decks.contains(&self.deck) {
            self.deck = d0;
        }
        let deck = self.deck_def(data);
        if !deck.firmware.iter().any(|f| f.id == self.firmware) {
            self.firmware = deck.firmware[0].id.clone();
        }
        if let Some(t) = self.monitor_def(data).locked_theme(data) {
            self.phosphor = t.to_string();
        }
        before
            != (
                self.monitors.clone(),
                self.monitor.clone(),
                self.decks.clone(),
                self.deck.clone(),
                self.firmware.clone(),
                self.phosphor.clone(),
            )
    }

    /// The monitor plugged in (the starter if the save names something unknown).
    pub fn monitor_def<'a>(&self, data: &'a Data) -> &'a MonitorDef {
        let hw = &data.hardware;
        hw.monitor(&self.monitor)
            .unwrap_or_else(|| hw.starter_monitor())
    }

    pub fn deck_def<'a>(&self, data: &'a Data) -> &'a DeckDef {
        let hw = &data.hardware;
        hw.deck(&self.deck).unwrap_or_else(|| hw.starter_deck())
    }

    /// The UX version the deck is running.
    pub fn firmware_def<'a>(&self, data: &'a Data) -> &'a FirmwareDef {
        let deck = self.deck_def(data);
        deck.firmware
            .iter()
            .find(|f| f.id == self.firmware)
            .unwrap_or(&deck.firmware[0])
    }

    /// The key a layout is saved under: the deck and its firmware.
    pub fn layout_key(&self, data: &Data) -> String {
        format!("{}/{}", self.deck_def(data).id, self.firmware_def(data).id)
    }

    /// The saved pane layout for the deck and firmware in use, if there is one.
    pub fn layout(&self, data: &Data) -> Option<&str> {
        self.layouts.get(&self.layout_key(data)).map(String::as_str)
    }

    /// Remembers the pane layout for the deck and firmware in use. True if it changed.
    pub fn set_layout(&mut self, data: &Data, text: String) -> bool {
        let key = self.layout_key(data);
        if self.layouts.get(&key) == Some(&text) {
            return false;
        }
        self.layouts.insert(key, text);
        true
    }

    /// Forgets the layout for the deck and firmware in use (reset to its default).
    pub fn clear_layout(&mut self, data: &Data) {
        let key = self.layout_key(data);
        self.layouts.remove(&key);
    }

    /// The CRT display preset the monitor shows.
    pub fn display_id<'a>(&self, data: &'a Data) -> &'a str {
        &self.monitor_def(data).display
    }

    /// Why `theme` can't go on the monitor plugged in, if it can't.
    pub fn theme_blocker(&self, data: &Data, theme: &str) -> Option<String> {
        let m = self.monitor_def(data);
        match m.locked_theme(data) {
            Some(t) if t != theme => Some(format!(
                "the {} is a mono tube: it only shows {t}. a colour monitor shows any theme.",
                m.title()
            )),
            _ => None,
        }
    }

    /// The dev key: drops `DEV_CREDITS` in the bank so the shops can be tried out.
    pub fn dev_credits(&mut self) -> String {
        self.bank += crate::tuning::DEV_CREDITS;
        format!(
            "dev: +{}¢. bank {}¢.",
            crate::tuning::DEV_CREDITS,
            self.bank
        )
    }

    pub fn owns(&self, hw: &Hardware) -> bool {
        match hw {
            Hardware::Monitor(id) => self.monitors.contains(id),
            Hardware::Deck(id) => self.decks.contains(id),
        }
    }

    /// Buys a monitor or deck with banked credits and plugs it in.
    pub fn buy_hardware(&mut self, data: &Data, hw: &Hardware) -> Result<String, String> {
        if self.owns(hw) {
            return Err("you already own that.".into());
        }
        let (title, price) = match hw {
            Hardware::Monitor(id) => {
                let m = data.hardware.monitor(id).ok_or("no such monitor.")?;
                (m.title(), m.price)
            }
            Hardware::Deck(id) => {
                let d = data.hardware.deck(id).ok_or("no such deck.")?;
                (d.title(), d.price)
            }
        };
        if self.bank < price {
            return Err(format!(
                "the {title} is {price}¢. the bank has {}¢.",
                self.bank
            ));
        }
        self.bank -= price;
        match hw {
            Hardware::Monitor(id) => {
                self.monitors.push(id.clone());
                self.plug_monitor(data, id)?;
                Ok(format!("bought the {title} (-{price}¢). it's plugged in."))
            }
            // A new deck waits on the shelf: you switch when you're ready (Prefs, or `deck`).
            Hardware::Deck(id) => {
                self.decks.push(id.clone());
                Ok(format!(
                    "bought the {title} (-{price}¢). switch to it in Prefs, or type `deck {id}`."
                ))
            }
        }
    }

    /// Plugs in a monitor you own. A mono tube switches to its theme.
    pub fn plug_monitor(&mut self, data: &Data, id: &str) -> Result<String, String> {
        if !self.monitors.iter().any(|m| m == id) {
            return Err(format!("{id}: you don't own that monitor."));
        }
        let m = data.hardware.monitor(id).ok_or("no such monitor.")?;
        self.monitor = m.id.clone();
        if let Some(t) = m.locked_theme(data) {
            self.phosphor = t.to_string();
        }
        Ok(format!("monitor: {}", m.title()))
    }

    /// Runs with a deck you own, on its first firmware.
    pub fn use_deck(&mut self, data: &Data, id: &str) -> Result<String, String> {
        if !self.decks.iter().any(|d| d == id) {
            return Err(format!("{id}: you don't own that deck."));
        }
        let d = data.hardware.deck(id).ok_or("no such deck.")?;
        self.deck = d.id.clone();
        self.firmware = d.firmware[0].id.clone();
        Ok(format!("deck: {}", d.title()))
    }

    /// Switches the deck to another of its UX versions.
    pub fn use_firmware(&mut self, data: &Data, id: &str) -> Result<String, String> {
        let d = self.deck_def(data);
        let f = d
            .firmware
            .iter()
            .find(|f| f.id == id)
            .ok_or_else(|| format!("the {} has no firmware {id}.", d.title()))?;
        let label = f.label.clone();
        self.firmware = f.id.clone();
        Ok(format!("firmware: {label}"))
    }
}
