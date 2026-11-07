//! Desktops at home: the add-ons you can buy for each (`data/hub/desktops.ron`), and the tiling
//! desktop's rules: workspaces, tiles, focus, the master-and-stack layout, and the launcher's
//! search. Pure; the app draws whichever desktop the deck's firmware names.

use serde::{Deserialize, Serialize};

use crate::data::Data;
use crate::hardware::Desktop;
use crate::hub::{Save, Tool};

/// Workspaces the tiling desktop starts with.
pub const BASE_WORKSPACES: usize = 3;
/// Share of the width the master tile gets once there's a stack beside it, in percent.
pub const MASTER_PCT: u8 = 58;
/// How far the master's share can shrink and grow (percent), and one step of it.
pub const MASTER_MIN: u8 = 30;
pub const MASTER_MAX: u8 = 80;
pub const MASTER_STEP: u8 = 5;

/// What an add-on does.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddonEffect {
    Workspaces(usize),
    Fuzzy,
    Stats,
    Clock,
    Pattern,
    /// A desk accessory on Liza: the bank, the next RIG upgrade, and your numbers' heat.
    Calculator,
    /// Liza keeps every window open (without it, opening one past the limit closes the oldest).
    MultiFinder,
}

/// One line of `data/hub/desktops.ron`.
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct AddonDef {
    pub id: String,
    pub desktop: Desktop,
    pub name: String,
    pub price: u32,
    pub blurb: String,
    pub effect: AddonEffect,
}

impl AddonDef {
    pub fn validate(defs: &[AddonDef]) -> Result<(), String> {
        for (i, d) in defs.iter().enumerate() {
            if defs[..i].iter().any(|o| o.id == d.id) {
                return Err(format!("add-on `{}` is listed twice", d.id));
            }
            if d.price == 0 {
                return Err(format!("add-on `{}` is free; give it a price", d.id));
            }
        }
        Ok(())
    }
}

impl Save {
    pub fn has_addon(&self, id: &str) -> bool {
        self.addons.iter().any(|a| a == id)
    }

    /// Whether an owned add-on for `desktop` has this effect.
    pub fn has_effect(&self, data: &Data, desktop: Desktop, effect: AddonEffect) -> bool {
        self.owned_addons(data, desktop).any(|d| d.effect == effect)
    }

    fn owned_addons<'a>(
        &'a self,
        data: &'a Data,
        desktop: Desktop,
    ) -> impl Iterator<Item = &'a AddonDef> {
        data.addons
            .iter()
            .filter(move |d| d.desktop == desktop && self.has_addon(&d.id))
    }

    /// Workspaces the tiling desktop has: the base plus any add-ons.
    pub fn workspaces(&self, data: &Data) -> usize {
        BASE_WORKSPACES
            + self
                .owned_addons(data, Desktop::Tiling)
                .map(|d| match d.effect {
                    AddonEffect::Workspaces(n) => n,
                    _ => 0,
                })
                .sum::<usize>()
    }

    /// Buys a desktop add-on with banked credits.
    pub fn buy_addon(&mut self, data: &Data, id: &str) -> Result<String, String> {
        let d = data
            .addons
            .iter()
            .find(|d| d.id == id)
            .ok_or("no such add-on.")?;
        if self.has_addon(id) {
            return Err("you already have that.".into());
        }
        if self.bank < d.price {
            return Err(format!(
                "{} is {}¢. the bank has {}¢.",
                d.name, d.price, self.bank
            ));
        }
        self.bank -= d.price;
        self.addons.push(d.id.clone());
        Ok(format!("installed {} (-{}¢).", d.name, d.price))
    }
}

/// The tiling desktop's state: which tools are tiled on which workspace, which is shown and
/// which tile has focus. Saved with the hub.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Tiling {
    pub workspaces: Vec<Vec<Tool>>,
    pub current: usize,
    pub focus: usize,
    /// Each workspace's shape (full screen, the master's share), by index. Missing entries
    /// (an older save, a workspace never reshaped) are the default shape.
    #[serde(default)]
    pub shape: Vec<Shape>,
}

/// How a workspace lays out its tiles. It knows nothing about what's in them.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shape {
    /// The focused tile covers the whole workspace; the others wait underneath.
    pub full: bool,
    /// The master's share of the width, in percent (`MASTER_MIN..=MASTER_MAX`).
    pub master: u8,
}

impl Default for Shape {
    fn default() -> Self {
        Self {
            full: false,
            master: MASTER_PCT,
        }
    }
}

impl Default for Tiling {
    fn default() -> Self {
        Self {
            workspaces: vec![vec![Tool::Shell]],
            current: 0,
            focus: 0,
            shape: Vec::new(),
        }
    }
}

/// A tile's rectangle: x, y, width, height.
pub type Area = [f32; 4];

impl Tiling {
    /// Makes sure there are exactly `n` workspaces (tiles on any dropped ones move to the last).
    pub fn ensure(&mut self, n: usize) {
        let n = n.max(1);
        while self.workspaces.len() < n {
            self.workspaces.push(Vec::new());
        }
        while self.workspaces.len() > n {
            let spill = self.workspaces.pop().unwrap_or_default();
            if let Some(last) = self.workspaces.last_mut() {
                for t in spill {
                    if !last.contains(&t) {
                        last.push(t);
                    }
                }
            }
        }
        self.shape.truncate(n);
        self.current = self.current.min(n - 1);
        self.clamp_focus();
    }

    /// The shown workspace's shape.
    pub fn shape(&self) -> Shape {
        self.shape.get(self.current).copied().unwrap_or_default()
    }

    fn shape_mut(&mut self) -> &mut Shape {
        if self.shape.len() <= self.current {
            self.shape.resize(self.current + 1, Shape::default());
        }
        &mut self.shape[self.current]
    }

    /// Full screen on the shown workspace, or back.
    pub fn toggle_full(&mut self) {
        let s = self.shape_mut();
        s.full = !s.full;
    }

    /// Grows (positive `steps`) or shrinks the master's share by `MASTER_STEP`s, clamped.
    pub fn resize_master(&mut self, steps: i32) {
        let s = self.shape_mut();
        let to = s.master as i32 + steps * MASTER_STEP as i32;
        s.master = to.clamp(MASTER_MIN as i32, MASTER_MAX as i32) as u8;
    }

    fn clamp_focus(&mut self) {
        let n = self.workspaces[self.current].len();
        self.focus = if n == 0 { 0 } else { self.focus.min(n - 1) };
    }

    /// The tools tiled on the shown workspace, master first.
    pub fn tiles(&self) -> &[Tool] {
        &self.workspaces[self.current]
    }

    pub fn focused(&self) -> Option<Tool> {
        self.tiles().get(self.focus).copied()
    }

    /// Opens a tool: if it's tiled somewhere, go there and focus it; otherwise tile it here.
    pub fn open(&mut self, tool: Tool) {
        for (w, tiles) in self.workspaces.iter().enumerate() {
            if let Some(i) = tiles.iter().position(|&t| t == tool) {
                self.current = w;
                self.focus = i;
                return;
            }
        }
        self.workspaces[self.current].push(tool);
        self.focus = self.workspaces[self.current].len() - 1;
    }

    /// Closes the focused tile.
    pub fn close(&mut self) -> Option<Tool> {
        let tiles = &mut self.workspaces[self.current];
        if tiles.is_empty() {
            return None;
        }
        let t = tiles.remove(self.focus);
        self.clamp_focus();
        Some(t)
    }

    /// Moves focus along the tiles, wrapping.
    pub fn step(&mut self, by: isize) {
        let n = self.tiles().len() as isize;
        if n > 0 {
            self.focus = (self.focus as isize + by).rem_euclid(n) as usize;
        }
    }

    /// Swaps the focused tile with the master (or, on the master, with the next one).
    pub fn swap_master(&mut self) {
        let tiles = &mut self.workspaces[self.current];
        if tiles.len() < 2 {
            return;
        }
        let other = if self.focus == 0 { 1 } else { 0 };
        tiles.swap(self.focus, other);
        self.focus = 0;
    }

    pub fn switch(&mut self, w: usize) {
        if w < self.workspaces.len() {
            self.current = w;
            self.clamp_focus();
        }
    }

    /// Sends the focused tile to workspace `w`.
    pub fn send(&mut self, w: usize) {
        if w >= self.workspaces.len() || w == self.current {
            return;
        }
        if let Some(t) = self.close() {
            self.workspaces[w].push(t);
        }
    }

    /// Where each tile goes in `area`: one tile fills it; with more, the master takes the left
    /// share the workspace's `Shape` says and the rest stack down the right. Full screen, only
    /// the focused tile is laid out, over all of it. `gap` pixels between and around tiles.
    pub fn layout(&self, area: Area, gap: f32) -> Vec<(Tool, Area)> {
        let [x, y, w, h] = area;
        let (x, y, w, h) = (
            x + gap,
            y + gap,
            (w - 2.0 * gap).max(0.0),
            (h - 2.0 * gap).max(0.0),
        );
        let tiles = self.tiles();
        let shape = self.shape();
        if shape.full
            && let Some(t) = self.focused()
        {
            return vec![(t, [x, y, w, h])];
        }
        match tiles.len() {
            0 => Vec::new(),
            1 => vec![(tiles[0], [x, y, w, h])],
            n => {
                let mw = ((w - gap) * shape.master as f32 / 100.0).floor();
                let sx = x + mw + gap;
                let sw = w - mw - gap;
                let rows = (n - 1) as f32;
                let sh = (h - gap * (rows - 1.0)) / rows;
                let mut out = vec![(tiles[0], [x, y, mw, h])];
                for (i, &t) in tiles[1..].iter().enumerate() {
                    out.push((t, [sx, y + i as f32 * (sh + gap), sw, sh]));
                }
                out
            }
        }
    }
}

/// The launcher's search: indices of `items` matching `query`, best first. Prefix matches
/// (case-insensitive) always count; `fuzzy` also lets the letters match in order with gaps,
/// tighter matches first. An empty query matches everything.
pub fn search(query: &str, items: &[String], fuzzy: bool) -> Vec<usize> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return (0..items.len()).collect();
    }
    let mut scored: Vec<(u32, usize)> = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let s = item.to_lowercase();
        if s.starts_with(&q) {
            scored.push((0, i));
        } else if s.split_whitespace().any(|w| w.starts_with(&q)) {
            scored.push((1, i));
        } else if fuzzy && let Some(gaps) = subsequence(&q, &s) {
            scored.push((10 + gaps, i));
        }
    }
    scored.sort();
    scored.into_iter().map(|(_, i)| i).collect()
}

/// How many characters of `hay` sit between `needle`'s letters, if they all appear in order.
fn subsequence(needle: &str, hay: &str) -> Option<u32> {
    let mut it = hay.chars();
    let mut gaps = 0;
    for c in needle.chars() {
        loop {
            let h = it.next()?;
            if h == c {
                break;
            }
            gaps += 1;
        }
    }
    Some(gaps)
}
