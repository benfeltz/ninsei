//! CPU Shopper: RIG as a mail-order magazine. It's data-driven end to end.
//!
//! - **The catalog** ([`Save::catalog`]) is everything with a price in its own file: RIG
//!   upgrades (`rig.ron`), monitors and decks (`hardware.ron`), desktop add-ons
//!   (`desktops.ron`), run panes (`panes.ron`), any tool with a `shop` block (`tools.ron`) and
//!   any game with one (`games.ron`, boxed by its publisher).
//!   A new item is an entry in its own file; it's in the next issue without a line of code.
//! - **The magazine** (`data/hub/shopper.ron`, [`ShopperDef`]) only dresses the catalog: the
//!   cover, the sections and their page numbers, the advertisers, and the ads. An item without an
//!   ad still sells, as a dot-leader line in its section's price list.
//! - **The pages** ([`layout`]) are worked out here, so they're tested headlessly; the app only
//!   draws them.
//! - **Buying** goes through one [`Item`] and [`Save::buy_item`], which hands off to each kind's
//!   own rules. A new kind of item is a variant here plus its buy rule.

use serde::{Deserialize, Serialize};

use crate::data::Data;
use crate::hardware::{DeckDef, Desktop, Hardware};
use crate::hub::{GameCopy, Save, Upgrade};

/// Something CPU Shopper sells.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Item {
    /// The next level of a RIG upgrade.
    Upgrade(Upgrade),
    Monitor(String),
    Deck(String),
    /// A desktop add-on (`desktops.ron`).
    Addon(String),
    /// A secondary run pane (`panes.ron`).
    Pane(String),
    /// A desktop tool with a `shop` block (`tools.ron`), by mail order.
    Tool(String),
    /// A boxed copy of a game with a `shop` block (`games.ron`), by mail order.
    Game(String),
}

/// A part of the magazine. Every item belongs to exactly one.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Section {
    Decks,
    Upgrades,
    Monitors,
    Addons,
    Panes,
    Software,
    Games,
}

impl Section {
    pub const ALL: [Section; 7] = [
        Section::Decks,
        Section::Upgrades,
        Section::Monitors,
        Section::Addons,
        Section::Panes,
        Section::Software,
        Section::Games,
    ];

    pub fn of(item: &Item) -> Section {
        match item {
            Item::Upgrade(_) => Section::Upgrades,
            Item::Monitor(_) => Section::Monitors,
            Item::Deck(_) => Section::Decks,
            Item::Addon(_) => Section::Addons,
            Item::Pane(_) => Section::Panes,
            Item::Tool(_) => Section::Software,
            Item::Game(_) => Section::Games,
        }
    }
}

/// How a tool is sold by mail order (the optional `shop` block in `tools.ron`).
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ToolShop {
    pub price: u32,
    /// The advertiser selling it (a vendor id in `shopper.ron`).
    pub vendor: String,
    /// A test listing: tagged REVIEW COPY, and the probe's bots never buy it.
    #[serde(default)]
    pub review: bool,
}

/// Whether you can order an item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stock {
    /// For sale at this price.
    Price(u32),
    /// You have it already. The magazine doesn't know that: it prints the price as usual,
    /// greyed out, and the item can't be ordered again.
    Owned(u32),
    /// A RIG upgrade at its top level.
    SoldOut,
    /// Listed, but not for you yet: a desktop add-on for an OS none of your decks run. The
    /// magazine says CALL NOW instead of a price, and it can't be ordered.
    Call,
}

/// One thing in the catalog, with what the magazine needs to show it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listing {
    pub item: Item,
    pub section: Section,
    /// Which price list it goes in when it has no ad, e.g. "ADD-ONS for LIZA".
    pub group: String,
    pub name: String,
    pub blurb: String,
    /// A short fact: "level 1 of 6", "mono", "runs Liza".
    pub detail: String,
    pub stock: Stock,
    /// The advertiser it's sold under, if its own file names one.
    pub vendor: Option<String>,
    pub review: bool,
    /// The OS it belongs to: a deck's first firmware's desktop, or an add-on's. A deck's
    /// full-page ad carries the add-ons for its desktop on its page.
    pub desktop: Option<Desktop>,
    /// A deck's spec box: per line, what it ships with and what it expands to ([`specs`]).
    /// Empty for everything else.
    pub specs: Vec<Spec>,
}

/// One row of a deck ad's spec box: a product's numbers, as the maker prints them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spec {
    pub line: Upgrade,
    /// The magazine's word for the line: RIG's label ("CPU", "RAM").
    pub label: String,
    /// Its unit ("cyc", "GB", "slots"), RIG's too; may be empty.
    pub unit: String,
    pub ships: u32,
    pub max: u32,
}

impl Spec {
    /// The SHIPS WITH cell: "4 cyc".
    pub fn ships_text(&self) -> String {
        crate::hub::amount(self.ships, &self.unit)
    }

    /// The EXPANDS TO cell: "6 cyc".
    pub fn max_text(&self) -> String {
        crate::hub::amount(self.max, &self.unit)
    }
}

/// What a deck's ad prints in its spec box: each of its `stats` rows, in its order, labelled
/// with RIG's words for the line. The one place an ad's numbers come from, so an ad can
/// never disagree with the deck. A line RIG doesn't sell has no words, so no row.
pub fn specs(data: &Data, deck: &DeckDef) -> Vec<Spec> {
    deck.stats
        .iter()
        .filter_map(|st| {
            let def = data.rig.iter().find(|u| u.kind == st.line)?;
            Some(Spec {
                line: st.line,
                label: def.label.clone(),
                unit: def.unit.clone(),
                ships: st.base,
                max: st.max,
            })
        })
        .collect()
}

impl Listing {
    /// What you'd pay to order it now (`None` once owned, sold out or not for you).
    pub fn price(&self) -> Option<u32> {
        match self.stock {
            Stock::Price(p) => Some(p),
            _ => None,
        }
    }

    /// The printed price: an owned item still shows its price, greyed out.
    pub fn list_price(&self) -> Option<u32> {
        match self.stock {
            Stock::Price(p) | Stock::Owned(p) => Some(p),
            _ => None,
        }
    }
}

/// An advertiser: who sells what, how to call them, and their small print.
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct VendorDef {
    pub id: String,
    pub name: String,
    /// A 1-800 number: decoration, never a line you can dial.
    #[serde(default)]
    pub hotline: Option<String>,
    /// The banner under the hotline, e.g. "JACK IN TODAY".
    #[serde(default)]
    pub slogan: Option<String>,
    /// The TERMS box on their full-page ads.
    #[serde(default)]
    pub terms: Vec<String>,
}

/// How much room an ad takes.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdSize {
    /// A whole page: the vendor's banner, bullets and terms.
    Full,
    /// Half a page.
    Half,
    /// A small boxed ad, four to a page.
    Box,
}

impl AdSize {
    /// Grid cells it covers: (columns, rows) of the page's `GRID_COLS` x `GRID_ROWS`.
    pub fn cells(self) -> (usize, usize) {
        match self {
            AdSize::Full => (GRID_COLS, GRID_ROWS),
            AdSize::Half => (GRID_COLS, GRID_ROWS / 2),
            AdSize::Box => (1, GRID_ROWS / 2),
        }
    }
}

/// A hand-written ad for one item. Without one, the item is a line in a price list.
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct AdDef {
    pub item: Item,
    pub size: AdSize,
    /// Overrides the item's own vendor.
    #[serde(default)]
    pub vendor: Option<String>,
    /// Defaults to the item's name.
    #[serde(default)]
    pub headline: Option<String>,
    /// ★ points.
    #[serde(default)]
    pub bullets: Vec<String>,
    /// Small print under the picture, e.g. "trodes not included".
    #[serde(default)]
    pub note: Option<String>,
    /// Shown instead of the price, e.g. "CALL for SPECIAL PRICE!" (the item still orders).
    #[serde(default)]
    pub callout: Option<String>,
}

/// A section of the magazine: its title, where it starts and who sells there by default.
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SectionDef {
    pub section: Section,
    pub title: String,
    /// The printed page it starts on.
    pub page: u32,
    /// The advertiser for its items that don't name one.
    pub vendor: String,
}

/// A line on the cover.
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct CoverLine {
    #[serde(default)]
    pub kicker: Option<String>,
    pub headline: String,
    pub line: String,
    /// The section it points to: the cover prints that section's page.
    pub section: Section,
}

/// `data/hub/shopper.ron`: the magazine's dressing.
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ShopperDef {
    pub title: String,
    pub issue: String,
    pub month: String,
    pub strap: String,
    pub newsstand: String,
    pub banner: String,
    /// "of 880".
    pub pages: u32,
    pub cover: Vec<CoverLine>,
    pub sections: Vec<SectionDef>,
    pub vendors: Vec<VendorDef>,
    #[serde(default)]
    pub ads: Vec<AdDef>,
}

impl ShopperDef {
    pub fn vendor(&self, id: &str) -> Option<&VendorDef> {
        self.vendors.iter().find(|v| v.id == id)
    }

    pub fn section(&self, s: Section) -> Option<&SectionDef> {
        self.sections.iter().find(|d| d.section == s)
    }

    pub fn ad(&self, item: &Item) -> Option<&AdDef> {
        self.ads.iter().find(|a| &a.item == item)
    }

    /// Who an item is sold under: its ad's vendor, else its own, else its section's.
    pub fn vendor_of<'a>(&'a self, l: &'a Listing) -> &'a str {
        self.ad(&l.item)
            .and_then(|a| a.vendor.as_deref())
            .or(l.vendor.as_deref())
            .or_else(|| self.section(l.section).map(|s| s.vendor.as_str()))
            .unwrap_or("")
    }

    /// Checks the magazine against everything it sells.
    pub fn validate(&self, data: &Data) -> Result<(), String> {
        for (i, v) in self.vendors.iter().enumerate() {
            if self.vendors[..i].iter().any(|o| o.id == v.id) {
                return Err(format!("vendor `{}` is listed twice", v.id));
            }
            if v.hotline.as_deref().is_some_and(|h| h.contains("555")) {
                return Err(format!(
                    "vendor `{}`'s hotline looks dialable; use a 1-800 number",
                    v.id
                ));
            }
        }
        for s in Section::ALL {
            let n = self.sections.iter().filter(|d| d.section == s).count();
            if n != 1 {
                return Err(format!("section {s:?} must be listed exactly once"));
            }
        }
        for d in &self.sections {
            if self.vendor(&d.vendor).is_none() {
                return Err(format!(
                    "section {:?} sells as unknown vendor `{}`",
                    d.section, d.vendor
                ));
            }
        }
        for c in &self.cover {
            if self.section(c.section).is_none() {
                return Err(format!("cover line `{}` points nowhere", c.headline));
            }
        }
        for (i, a) in self.ads.iter().enumerate() {
            if self.ads[..i].iter().any(|o| o.item == a.item) {
                return Err(format!("{:?} has two ads", a.item));
            }
            if !exists(data, &a.item) {
                return Err(format!("an ad sells {:?}, which isn't for sale", a.item));
            }
            if let Some(v) = &a.vendor
                && self.vendor(v).is_none()
            {
                return Err(format!(
                    "the ad for {:?} names unknown vendor `{v}`",
                    a.item
                ));
            }
        }
        for t in &data.tools {
            if let Some(shop) = &t.shop {
                if shop.price == 0 {
                    return Err(format!("tool `{}` is sold for nothing", t.id));
                }
                if self.vendor(&shop.vendor).is_none() {
                    return Err(format!(
                        "tool `{}` is sold by unknown vendor `{}`",
                        t.id, shop.vendor
                    ));
                }
            }
        }
        Ok(())
    }
}

/// Whether the item is something the data sells (regardless of what a save owns).
fn exists(data: &Data, item: &Item) -> bool {
    match item {
        Item::Upgrade(u) => data.rig.iter().any(|d| d.kind == *u),
        Item::Monitor(id) => data.hardware.monitor(id).is_some(),
        Item::Deck(id) => data.hardware.deck(id).is_some(),
        Item::Addon(id) => data.addons.iter().any(|d| &d.id == id),
        Item::Pane(id) => data.panes.iter().any(|d| &d.id == id),
        Item::Tool(id) => data.tools.iter().any(|t| &t.id == id && t.shop.is_some()),
        Item::Game(id) => data.game(id).is_some_and(|g| g.shop.is_some()),
    }
}

impl Save {
    /// The desktops the decks you own run: add-ons are only sold for these.
    pub fn desktops_owned(&self, data: &Data) -> Vec<Desktop> {
        let mut out = Vec::new();
        for d in self.decks.iter().filter_map(|d| data.hardware.deck(d)) {
            for f in &d.firmware {
                if !out.contains(&f.desktop) {
                    out.push(f.desktop);
                }
            }
        }
        out
    }

    /// Everything CPU Shopper sells you, in section order, each with whether you can order it.
    pub fn catalog(&self, data: &Data) -> Vec<Listing> {
        let mut out = Vec::new();
        let listing =
            |item: Item, group: &str, name: &str, blurb: &str, detail: String, stock| Listing {
                section: Section::of(&item),
                item,
                group: group.to_string(),
                name: name.to_string(),
                blurb: blurb.to_string(),
                detail,
                stock,
                vendor: None,
                review: false,
                desktop: None,
                specs: Vec::new(),
            };
        for d in &data.hardware.decks {
            let hw = Hardware::Deck(d.id.clone());
            let stock = if self.owns(&hw) {
                Stock::Owned(d.price)
            } else {
                Stock::Price(d.price)
            };
            let os: Vec<&str> = d.firmware.iter().map(|f| f.desktop.name()).collect();
            let mut l = listing(
                Item::Deck(d.id.clone()),
                "DECKS",
                &d.title(),
                &d.blurb,
                format!("runs {}", os.join(", ")),
                stock,
            );
            l.desktop = d.firmware.first().map(|f| f.desktop);
            l.specs = specs(data, d);
            out.push(l);
        }
        for def in &data.rig {
            let level = self.rig.level(def.kind);
            let stock = def.cost(level).map_or(Stock::SoldOut, Stock::Price);
            let unit = if def.unit.is_empty() {
                String::new()
            } else {
                format!(" {}", def.unit)
            };
            out.push(listing(
                Item::Upgrade(def.kind),
                "RIG UPGRADES",
                &format!("{}, +{}{unit}", def.label, def.step),
                &def.blurb,
                format!("level {level} of {}", def.max),
                stock,
            ));
        }
        for m in &data.hardware.monitors {
            let hw = Hardware::Monitor(m.id.clone());
            let stock = if self.owns(&hw) {
                Stock::Owned(m.price)
            } else {
                Stock::Price(m.price)
            };
            out.push(listing(
                Item::Monitor(m.id.clone()),
                "MONITORS for EVERY DECK",
                &m.title(),
                &m.blurb,
                if m.colour { "colour" } else { "mono" }.to_string(),
                stock,
            ));
        }
        let desks = self.desktops_owned(data);
        // Every add-on is listed; ones for an OS none of your decks run say CALL NOW.
        for a in &data.addons {
            let stock = if self.has_addon(&a.id) {
                Stock::Owned(a.price)
            } else if desks.contains(&a.desktop) {
                Stock::Price(a.price)
            } else {
                Stock::Call
            };
            let os = a.desktop.name();
            let mut l = listing(
                Item::Addon(a.id.clone()),
                &format!("ADD-ON for {}", os.to_uppercase()),
                &a.name,
                &a.blurb,
                format!("for {os}"),
                stock,
            );
            l.desktop = Some(a.desktop);
            out.push(l);
        }
        for p in &data.panes {
            let bundled = self
                .decks
                .iter()
                .filter_map(|d| data.hardware.deck(d))
                .flat_map(|d| d.firmware.iter().map(move |f| (d, f)))
                .find(|(_, f)| f.panes.iter().any(|x| x == &p.id))
                .map(|(d, f)| format!("free on {} {}", d.name, f.id));
            let stock = if self.panes.iter().any(|x| x == &p.id) {
                Stock::Owned(p.price)
            } else {
                Stock::Price(p.price)
            };
            out.push(listing(
                Item::Pane(p.id.clone()),
                "RUN PANES",
                &p.name,
                &p.blurb,
                bundled.unwrap_or_default(),
                stock,
            ));
        }
        for t in &data.tools {
            let Some(shop) = &t.shop else { continue };
            let stock = if self.has_tool(&t.id) {
                Stock::Owned(shop.price)
            } else {
                Stock::Price(shop.price)
            };
            let mut l = listing(
                Item::Tool(t.id.clone()),
                "MAIL-ORDER SOFTWARE",
                &t.name,
                &t.blurb,
                String::new(),
                stock,
            );
            l.vendor = Some(shop.vendor.clone());
            l.review = shop.review;
            out.push(l);
        }
        // A cracked copy doesn't count: the boxed one is still for sale.
        for g in &data.games {
            let Some(shop) = &g.shop else { continue };
            let stock = if self.has_copy(&g.id, GameCopy::Boxed) {
                Stock::Owned(shop.price)
            } else {
                Stock::Price(shop.price)
            };
            let mut l = listing(
                Item::Game(g.id.clone()),
                "GAMES",
                &g.name,
                &g.blurb,
                format!("version {}", g.version),
                stock,
            );
            l.vendor = Some(g.publisher.clone());
            out.push(l);
        }
        out
    }

    /// Why an item can't be ordered right now, if it can't.
    pub fn order_blocker(&self, data: &Data, item: &Item) -> Option<String> {
        let l = self.catalog(data).into_iter().find(|l| &l.item == item);
        match l.map(|l| l.stock) {
            None => Some("that isn't for sale.".into()),
            Some(Stock::Owned(_)) => Some("you already have this.".into()),
            Some(Stock::SoldOut) => Some("sold out: that upgrade is maxed.".into()),
            Some(Stock::Call) => Some(
                "call now: it's for an OS none of your decks run yet. order the deck first.".into(),
            ),
            // The magazine doesn't know your deck; the order form does.
            Some(Stock::Price(_))
                if let Item::Upgrade(u) = item
                    && let Some(why) = self.upgrade_blocker(data, *u) =>
            {
                Some(why)
            }
            Some(Stock::Price(p)) if self.bank < p => {
                Some(format!("{p}¢. the bank has {}¢.", self.bank))
            }
            Some(Stock::Price(_)) => None,
        }
    }

    /// Orders anything from the catalog with banked credits, through that item's own rules.
    pub fn buy_item(&mut self, data: &Data, item: &Item) -> Result<String, String> {
        // Listed-but-not-yours items never sell, whatever the item's own rules say.
        if self
            .catalog(data)
            .iter()
            .any(|l| &l.item == item && l.stock == Stock::Call)
        {
            return Err(self.order_blocker(data, item).unwrap_or_default());
        }
        match item {
            Item::Upgrade(u) => self.buy(data, *u),
            Item::Monitor(id) => self.buy_hardware(data, &Hardware::Monitor(id.clone())),
            Item::Deck(id) => self.buy_hardware(data, &Hardware::Deck(id.clone())),
            Item::Addon(id) => self.buy_addon(data, id),
            Item::Pane(id) => self.buy_pane(data, id),
            Item::Tool(id) => self.buy_tool(data, id),
            Item::Game(id) => self.buy_game(data, id),
        }
    }

    /// Orders a boxed copy of a game by mail. A cracked copy you had stays on the disk.
    pub fn buy_game(&mut self, data: &Data, id: &str) -> Result<String, String> {
        let g = data.game(id).ok_or("no such game.")?;
        let shop = g.shop.as_ref().ok_or("that game isn't sold by mail.")?;
        if self.has_copy(id, GameCopy::Boxed) {
            return Err("you already have that.".into());
        }
        if self.bank < shop.price {
            return Err(format!(
                "{} is {}¢. the bank has {}¢.",
                g.name, shop.price, self.bank
            ));
        }
        self.bank -= shop.price;
        self.learn_game(id, GameCopy::Boxed);
        Ok(format!("{} arrived by mail (-{}¢).", g.name, shop.price))
    }

    /// Orders a desktop tool by mail. It's yours for good, like one downloaded from a board.
    pub fn buy_tool(&mut self, data: &Data, id: &str) -> Result<String, String> {
        let t = data
            .tools
            .iter()
            .find(|t| t.id == id)
            .ok_or("no such tool.")?;
        let shop = t.shop.as_ref().ok_or("that tool isn't sold by mail.")?;
        if self.has_tool(id) {
            return Err("you already have that.".into());
        }
        if self.bank < shop.price {
            return Err(format!(
                "{} is {}¢. the bank has {}¢.",
                t.name, shop.price, self.bank
            ));
        }
        self.bank -= shop.price;
        self.learn_tool(id);
        Ok(format!(
            "{} arrived by mail (-{}¢). it's on your desktop.",
            t.name, shop.price
        ))
    }
}

/// One block on a page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    /// A hand-written ad: the listing (index into the catalog) and its ad (index into `ads`).
    Ad { listing: usize, ad: usize },
    /// A dot-leader price list.
    List { title: String, listings: Vec<usize> },
}

/// Where a block sits on a page: a rectangle of grid cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub col: usize,
    pub row: usize,
    pub cols: usize,
    pub rows: usize,
}

impl Cell {
    fn covers(&self, col: usize, row: usize) -> bool {
        (self.col..self.col + self.cols).contains(&col)
            && (self.row..self.row + self.rows).contains(&row)
    }
}

/// A page of the magazine. Every page is the same size; a section page is a grid of
/// `GRID_COLS` x `GRID_ROWS` cells, and each block owns a rectangle of them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Page {
    Cover,
    Contents,
    Section {
        /// The first section on the page (its running title).
        section: Section,
        /// Every section with something on the page, in order: small sections share a page.
        sections: Vec<Section>,
        /// The printed page number.
        number: u32,
        /// The advertiser whose page it is (a full-page ad); otherwise the section's.
        vendor: String,
        blocks: Vec<(Block, Cell)>,
    },
}

/// A section page's grid.
pub const GRID_COLS: usize = 2;
pub const GRID_ROWS: usize = 4;
/// Price-list lines that fit in a list `rows` cells tall (its title takes a line).
pub fn list_lines(rows: usize) -> usize {
    rows * LIST_LINES_PER_ROW - 1
}
const LIST_LINES_PER_ROW: usize = 5;

/// The rows a price list of `n` lines needs.
fn list_rows(n: usize) -> usize {
    (1..=GRID_ROWS)
        .find(|&r| list_lines(r) >= n)
        .unwrap_or(GRID_ROWS)
}

/// The first free spot on a page for a block of `cols` x `rows` cells, row by row.
fn fit(used: &[Cell], cols: usize, rows: usize) -> Option<Cell> {
    for row in 0..=GRID_ROWS.saturating_sub(rows) {
        for col in 0..=GRID_COLS.saturating_sub(cols) {
            let free = (row..row + rows)
                .all(|r| (col..col + cols).all(|c| !used.iter().any(|u| u.covers(c, r))));
            if free {
                return Some(Cell {
                    col,
                    row,
                    cols,
                    rows,
                });
            }
        }
    }
    None
}

/// The whole issue: the cover, the contents (with the index of advertisers), then every
/// section's pages.
/// Every listing appears exactly once, and every block gets its own cells on one page.
pub fn layout(def: &ShopperDef, catalog: &[Listing]) -> Vec<Page> {
    let mut pages = vec![Page::Cover, Page::Contents];
    let mut sheets: Vec<Sheet> = Vec::new();
    let companions = companions(def, catalog);
    let claimed = |l: usize| companions.iter().any(|(_, ls)| ls.contains(&l));
    for sd in &def.sections {
        let in_section: Vec<usize> = (0..catalog.len())
            .filter(|&i| catalog[i].section == sd.section && !claimed(i))
            .collect();
        if in_section.is_empty() {
            continue;
        }
        // Blocks with their size in cells: ads first, biggest first, then the price lists. A
        // full-page ad owns its page; a deck's ad brings its add-ons onto the page with it.
        let mut blocks: Vec<Placing> = Vec::new();
        let mut ads: Vec<(usize, usize)> = in_section
            .iter()
            .filter_map(|&l| {
                def.ads
                    .iter()
                    .position(|a| a.item == catalog[l].item)
                    .map(|a| (l, a))
            })
            .collect();
        ads.sort_by_key(|&(l, a)| {
            let (c, r) = def.ads[a].size.cells();
            (std::cmp::Reverse(c * r), l)
        });
        for &(l, a) in &ads {
            let (cols, rows) = def.ads[a].size.cells();
            let vendor = def.vendor_of(&catalog[l]).to_string();
            let owns = def.ads[a].size == AdSize::Full;
            let with = companions
                .iter()
                .find(|(d, _)| *d == l)
                .map(|(_, ls)| ls.clone())
                .unwrap_or_default();
            let list_rows = if with.is_empty() {
                0
            } else {
                list_rows(with.len())
            };
            blocks.push(Placing {
                block: Block::Ad { listing: l, ad: a },
                size: (cols, rows - list_rows.min(rows - 1)),
                vendor: vendor.clone(),
                owns,
                stick: false,
            });
            if !with.is_empty() {
                blocks.push(Placing {
                    block: Block::List {
                        title: catalog[with[0]].group.clone(),
                        listings: with,
                    },
                    size: (GRID_COLS, list_rows.min(rows - 1)),
                    vendor,
                    owns: false,
                    stick: true,
                });
            }
        }
        let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
        for &l in &in_section {
            if ads.iter().any(|&(x, _)| x == l) {
                continue;
            }
            let g = &catalog[l].group;
            match groups.iter_mut().find(|(t, _)| t == g) {
                Some((_, v)) => v.push(l),
                None => groups.push((g.clone(), vec![l])),
            }
        }
        for (title, ls) in groups {
            for chunk in ls.chunks(list_lines(GRID_ROWS)) {
                blocks.push(Placing {
                    block: Block::List {
                        title: title.clone(),
                        listings: chunk.to_vec(),
                    },
                    size: (1, list_rows(chunk.len())),
                    vendor: sd.vendor.clone(),
                    owns: false,
                    stick: false,
                });
            }
        }
        // Place each block on the first page with room, from the page the last section ended
        // on (so small sections share a page), else on a new page.
        let mut from = sheets.len().saturating_sub(1);
        // A section's ads stay together: if they won't all fit on the page the last section
        // ended on, the section starts a new page.
        if let Some(last) = sheets.last() {
            let mut used: Vec<Cell> = last.blocks.iter().map(|(_, c)| *c).collect();
            let ads_fit = blocks
                .iter()
                .filter(|p| matches!(p.block, Block::Ad { .. }) && !p.owns)
                .all(|p| match fit(&used, p.size.0, p.size.1) {
                    Some(c) => {
                        used.push(c);
                        true
                    }
                    None => false,
                });
            if !ads_fit {
                from = sheets.len();
            }
        }
        for Placing {
            block,
            size: (cols, rows),
            vendor: v,
            owns,
            stick,
        } in blocks
        {
            // A deck's add-ons go on the deck's own page (the last one made).
            let range = if stick {
                sheets.len().saturating_sub(1)..sheets.len()
            } else if owns {
                sheets.len()..sheets.len()
            } else {
                from..sheets.len()
            };
            let spot = sheets[range].iter_mut().find_map(|sh| {
                let used: Vec<Cell> = sh.blocks.iter().map(|(_, c)| *c).collect();
                fit(&used, cols, rows).map(|c| (sh, c))
            });
            let section = match &block {
                Block::List { listings, .. } => catalog[listings[0]].section,
                Block::Ad { listing, .. } => catalog[*listing].section,
            };
            match spot {
                Some((sh, cell)) => {
                    sh.blocks.push((block, cell));
                    if !sh.sections.contains(&section) {
                        sh.sections.push(section);
                    }
                }
                None => {
                    let cell = fit(&[], cols, rows).expect("a block always fits an empty page");
                    let whole = owns;
                    let prev = sheets.last().map_or(0, |sh: &Sheet| sh.number);
                    // A section that opens on a new page starts on its printed page.
                    let opens = !sheets.iter().any(|sh| sh.sections.contains(&sd.section));
                    let number = if opens {
                        sd.page.max(prev + 1)
                    } else {
                        prev + 1
                    };
                    sheets.push(Sheet {
                        blocks: vec![(block, cell)],
                        sections: vec![section],
                        vendor: if whole { v } else { sd.vendor.clone() },
                        number,
                    });
                    // The rest of this section follows it onto the new page, never back.
                    from = sheets.len() - 1;
                }
            }
        }
    }
    for sh in sheets {
        let mut blocks = sh.blocks;
        grow(&mut blocks);
        pages.push(Page::Section {
            section: sh.sections[0],
            sections: sh.sections,
            number: sh.number,
            vendor: sh.vendor,
            blocks,
        });
    }
    pages
}

/// A block waiting for a page: its size in cells, whose page it would be, whether it owns a
/// page (a full-page ad) and whether it must go on the same page as the block before it.
struct Placing {
    block: Block,
    size: (usize, usize),
    vendor: String,
    owns: bool,
    stick: bool,
}

/// Which listings ride on a deck's full-page ad: each deck with a full-page ad takes the add-ons
/// for its desktop (its first firmware's), and each add-on goes with the first such deck.
fn companions(def: &ShopperDef, catalog: &[Listing]) -> Vec<(usize, Vec<usize>)> {
    let mut out: Vec<(usize, Vec<usize>)> = Vec::new();
    for (d, l) in catalog.iter().enumerate() {
        let full = def.ad(&l.item).is_some_and(|a| a.size == AdSize::Full);
        let (Item::Deck(_), Some(desk), true) = (&l.item, l.desktop, full) else {
            continue;
        };
        let taken = |i: usize| out.iter().any(|(_, ls)| ls.contains(&i));
        let with: Vec<usize> = (0..catalog.len())
            .filter(|&i| matches!(catalog[i].item, Item::Addon(_)))
            .filter(|&i| catalog[i].desktop == Some(desk) && !taken(i))
            .take(list_lines(GRID_ROWS - 1))
            .collect();
        if !with.is_empty() {
            out.push((d, with));
        }
    }
    out
}

/// A page being filled.
struct Sheet {
    blocks: Vec<(Block, Cell)>,
    sections: Vec<Section>,
    vendor: String,
    number: u32,
}

/// Grows each block into free cells below it, then beside it, so a page is filled edge to edge.
fn grow(blocks: &mut [(Block, Cell)]) {
    for i in 0..blocks.len() {
        let free = |blocks: &[(Block, Cell)], col: usize, row: usize| {
            blocks
                .iter()
                .enumerate()
                .all(|(j, (_, c))| j == i || !c.covers(col, row))
        };
        loop {
            let c = blocks[i].1;
            let below = c.row + c.rows;
            if below < GRID_ROWS && (c.col..c.col + c.cols).all(|x| free(blocks, x, below)) {
                blocks[i].1.rows += 1;
            } else {
                break;
            }
        }
        loop {
            let c = blocks[i].1;
            let right = c.col + c.cols;
            if right < GRID_COLS && (c.row..c.row + c.rows).all(|y| free(blocks, right, y)) {
                blocks[i].1.cols += 1;
            } else {
                break;
            }
        }
    }
}

/// The page (index into `pages`) each section starts on, with its printed number.
pub fn contents(pages: &[Page]) -> Vec<(Section, usize, u32)> {
    let mut out: Vec<(Section, usize, u32)> = Vec::new();
    for (i, p) in pages.iter().enumerate() {
        if let Page::Section {
            sections, number, ..
        } = p
        {
            for s in sections {
                if !out.iter().any(|(o, ..)| o == s) {
                    out.push((*s, i, *number));
                }
            }
        }
    }
    out
}

/// The index of advertisers: each vendor that sells something in this issue, and the printed
/// pages their goods are on, in vendor order.
pub fn advertisers(
    def: &ShopperDef,
    catalog: &[Listing],
    pages: &[Page],
) -> Vec<(String, Vec<u32>)> {
    let mut out: Vec<(String, Vec<u32>)> = def
        .vendors
        .iter()
        .map(|v| (v.id.clone(), Vec::new()))
        .collect();
    for p in pages {
        let Page::Section { number, blocks, .. } = p else {
            continue;
        };
        for (b, _) in blocks {
            let ls: Vec<usize> = match b {
                Block::Ad { listing, .. } => vec![*listing],
                Block::List { listings, .. } => listings.clone(),
            };
            for l in ls {
                let v = def.vendor_of(&catalog[l]);
                if let Some((_, nums)) = out.iter_mut().find(|(id, _)| id == v)
                    && !nums.contains(number)
                {
                    nums.push(*number);
                }
            }
        }
    }
    out.retain(|(_, nums)| !nums.is_empty());
    out
}
