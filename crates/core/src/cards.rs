//! HyperCard-style stacks: cards of text and 1-bit pictures, paged ◀ ▶ from a Home card that
//! lists the sections. The first is **Welcome to the Net** (`data/stacks/welcome.ron`), the
//! guide Sense/Net ships with the deck. Numbers in the text are `{placeholders}` filled from
//! the rules and data ([`fill`]), so a card can never disagree with the game.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::data::Data;

/// The most a card's text may hold, after its placeholders are filled.
pub const TEXT_COLS: usize = 54;
pub const TEXT_ROWS: usize = 11;
/// The largest picture (pixels).
pub const PIC_W: usize = 48;
pub const PIC_H: usize = 40;

#[derive(Deserialize, Clone, Debug)]
pub struct StackDef {
    pub title: String,
    /// 1-bit pictures by name: `#` ink, `.` paper, every row the same width.
    #[serde(default)]
    pub pictures: BTreeMap<String, Vec<String>>,
    pub sections: Vec<SectionDef>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct SectionDef {
    pub name: String,
    pub cards: Vec<CardDef>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct CardDef {
    pub title: String,
    pub text: Vec<String>,
    #[serde(default)]
    pub picture: Option<String>,
}

/// Every placeholder a card may use, and what it reads.
pub const PLACEHOLDERS: &[&str] = &[
    "trace_max",
    "cpu",
    "ram",
    "hdd",
    "integrity",
    "stash",
    "gutter",
    "gutter_phone",
    "call_cost",
];

fn value(key: &str, data: &Data) -> Option<String> {
    use crate::tuning::*;
    let gutter = data.boards.first();
    Some(match key {
        "trace_max" => TRACE_MAX.to_string(),
        "cpu" => START_CPU.to_string(),
        "ram" => START_RAM.to_string(),
        "hdd" => START_HDD.to_string(),
        "integrity" => START_INTEGRITY.to_string(),
        "stash" => crate::hub::Save::default().stash_cap(&data.rig).to_string(),
        "gutter" => gutter?.name.clone(),
        "gutter_phone" => gutter?.phone.clone(),
        "call_cost" => gutter?.call_cost.to_string(),
        _ => return None,
    })
}

/// A line with its `{placeholders}` filled in. Unknown ones are left as they are
/// ([`StackDef::validate`] rejects them).
pub fn fill(line: &str, data: &Data) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close) => {
                let key = &after[..close];
                match value(key, data) {
                    Some(v) => out.push_str(&v),
                    None => out.push_str(&rest[open..open + close + 2]),
                }
                rest = &after[close + 1..];
            }
            None => {
                out.push_str(&rest[open..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

impl StackDef {
    /// Every card in order, with its section's index.
    pub fn cards(&self) -> Vec<(usize, &CardDef)> {
        self.sections
            .iter()
            .enumerate()
            .flat_map(|(s, sec)| sec.cards.iter().map(move |c| (s, c)))
            .collect()
    }

    /// The first card of section `s`, as an index into [`StackDef::cards`].
    pub fn section_start(&self, s: usize) -> usize {
        self.sections[..s.min(self.sections.len())]
            .iter()
            .map(|x| x.cards.len())
            .sum()
    }

    pub fn validate(&self, data: &Data) -> Result<(), String> {
        let err = |m: String| Err(format!("stack `{}`: {m}", self.title));
        if self.sections.iter().all(|s| s.cards.is_empty()) {
            return err("no cards".into());
        }
        for (name, rows) in &self.pictures {
            let w = rows.first().map_or(0, |r| r.chars().count());
            if w == 0 || w > PIC_W || rows.len() > PIC_H {
                return err(format!(
                    "picture `{name}` is {w}x{} (max {PIC_W}x{PIC_H})",
                    rows.len()
                ));
            }
            if rows.iter().any(|r| r.chars().count() != w) {
                return err(format!("picture `{name}`: every row must be {w} wide"));
            }
            if rows.iter().any(|r| r.chars().any(|c| c != '#' && c != '.')) {
                return err(format!("picture `{name}`: only `#` and `.`"));
            }
        }
        for s in &self.sections {
            for c in &s.cards {
                let at = format!("card `{}`", c.title);
                if let Some(p) = &c.picture
                    && !self.pictures.contains_key(p)
                {
                    return err(format!("{at} shows unknown picture `{p}`"));
                }
                if c.text.len() > TEXT_ROWS {
                    return err(format!("{at} has {} lines (max {TEXT_ROWS})", c.text.len()));
                }
                for l in &c.text {
                    let filled = fill(l, data);
                    if let Some(open) = filled.find('{')
                        && filled[open..].contains('}')
                    {
                        return err(format!("{at}: unknown placeholder in `{l}`"));
                    }
                    let w = filled.chars().count();
                    if w > TEXT_COLS {
                        return err(format!(
                            "{at}: line is {w} wide (max {TEXT_COLS}): {filled}"
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

/// Where a reader is in a stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// The Home card: the sections.
    Home,
    /// A card, by index into [`StackDef::cards`].
    Card(usize),
}

impl Place {
    /// ▶: the next card; from Home, the first; the last card stays.
    pub fn next(self, stack: &StackDef) -> Place {
        let n = stack.cards().len();
        match self {
            Place::Home if n > 0 => Place::Card(0),
            Place::Card(i) if i + 1 < n => Place::Card(i + 1),
            p => p,
        }
    }

    /// ◀: the card before; the first card goes Home.
    pub fn prev(self) -> Place {
        match self {
            Place::Card(0) | Place::Home => Place::Home,
            Place::Card(i) => Place::Card(i - 1),
        }
    }
}
