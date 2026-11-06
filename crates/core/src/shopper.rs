//! CPU Shopper: RIG as a mail-order magazine. It's data-driven end to end.
//!
//! - **The catalog** ([`Save::catalog`]) is everything with a price in its own file: RIG
//!   upgrades (`rig.ron`), monitors and decks (`hardware.ron`), desktop add-ons
//!   (`desktops.ron`), run panes (`panes.ron`) and any tool with a `shop` block (`tools.ron`).
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
use crate::hardware::{Desktop, Hardware};
use crate::hub::{Save, Upgrade};

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
}

impl Section {
    pub const ALL: [Section; 6] = [
        Section::Decks,
        Section::Upgrades,
        Section::Monitors,
        Section::Addons,
        Section::Panes,
        Section::Software,
    ];

    pub fn of(item: &Item) -> Section {
        match item {
            Item::Upgrade(_) => Section::Upgrades,
            Item::Monitor(_) => Section::Monitors,
            Item::Deck(_) => Section::Decks,
            Item::Addon(_) => Section::Addons,
            Item::Pane(_) => Section::Panes,
            Item::Tool(_) => Section::Software,
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
    /// You have it ("IN YOUR RIG").
    Owned,
    /// A RIG upgrade at its top level.
    SoldOut,
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
}

impl Listing {
    pub fn price(&self) -> Option<u32> {
        match self.stock {
            Stock::Price(p) => Some(p),
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
            };
        for d in &data.hardware.decks {
            let hw = Hardware::Deck(d.id.clone());
            let stock = if self.owns(&hw) {
                Stock::Owned
            } else {
                Stock::Price(d.price)
            };
            let os: Vec<&str> = d.firmware.iter().map(|f| f.desktop.name()).collect();
            out.push(listing(
                Item::Deck(d.id.clone()),
                "DECKS",
                &d.title(),
                &d.blurb,
                format!("runs {}", os.join(", ")),
                stock,
            ));
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
                Stock::Owned
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
        for a in data.addons.iter().filter(|a| desks.contains(&a.desktop)) {
            let stock = if self.has_addon(&a.id) {
                Stock::Owned
            } else {
                Stock::Price(a.price)
            };
            let os = a.desktop.name();
            out.push(listing(
                Item::Addon(a.id.clone()),
                &format!("ADD-ONS for {}", os.to_uppercase()),
                &a.name,
                &a.blurb,
                format!("for {os}"),
                stock,
            ));
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
                Stock::Owned
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
                Stock::Owned
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
        out
    }

    /// Why an item can't be ordered right now, if it can't.
    pub fn order_blocker(&self, data: &Data, item: &Item) -> Option<String> {
        let l = self.catalog(data).into_iter().find(|l| &l.item == item);
        match l.map(|l| l.stock) {
            None => Some("that isn't for sale.".into()),
            Some(Stock::Owned) => Some("already in your rig.".into()),
            Some(Stock::SoldOut) => Some("sold out: that upgrade is maxed.".into()),
            Some(Stock::Price(p)) if self.bank < p => {
                Some(format!("{p}¢. the bank has {}¢.", self.bank))
            }
            Some(Stock::Price(_)) => None,
        }
    }

    /// Orders anything from the catalog with banked credits, through that item's own rules.
    pub fn buy_item(&mut self, data: &Data, item: &Item) -> Result<String, String> {
        match item {
            Item::Upgrade(u) => self.buy(&data.rig, *u),
            Item::Monitor(id) => self.buy_hardware(data, &Hardware::Monitor(id.clone())),
            Item::Deck(id) => self.buy_hardware(data, &Hardware::Deck(id.clone())),
            Item::Addon(id) => self.buy_addon(data, id),
            Item::Pane(id) => self.buy_pane(data, id),
            Item::Tool(id) => self.buy_tool(data, id),
        }
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
    for sd in &def.sections {
        let in_section: Vec<usize> = (0..catalog.len())
            .filter(|&i| catalog[i].section == sd.section)
            .collect();
        if in_section.is_empty() {
            continue;
        }
        // Blocks with their size in cells: ads first, biggest first, then the price lists.
        let mut blocks: Vec<(Block, (usize, usize), String)> = Vec::new();
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
            blocks.push((
                Block::Ad { listing: l, ad: a },
                def.ads[a].size.cells(),
                def.vendor_of(&catalog[l]).to_string(),
            ));
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
                blocks.push((
                    Block::List {
                        title: title.clone(),
                        listings: chunk.to_vec(),
                    },
                    (1, list_rows(chunk.len())),
                    sd.vendor.clone(),
                ));
            }
        }
        // Place each block on the first page with room, from the page the last section ended
        // on (so small sections share a page), else on a new page.
        let from = sheets.len().saturating_sub(1);
        for (block, (cols, rows), v) in blocks {
            let spot = sheets[from..].iter_mut().find_map(|sh| {
                let used: Vec<Cell> = sh.blocks.iter().map(|(_, c)| *c).collect();
                fit(&used, cols, rows).map(|c| (sh, c))
            });
            match spot {
                Some((sh, cell)) => {
                    sh.blocks.push((block, cell));
                    if !sh.sections.contains(&sd.section) {
                        sh.sections.push(sd.section);
                    }
                }
                None => {
                    let cell = fit(&[], cols, rows).expect("a block always fits an empty page");
                    let whole = cols == GRID_COLS && rows == GRID_ROWS;
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
                        sections: vec![sd.section],
                        vendor: if whole { v } else { sd.vendor.clone() },
                        number,
                    });
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
