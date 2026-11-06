//! NiGHTCOMM, the hacker's dashboard: what it shows, worked out here so the app only draws.
//! Its phonebook knows **heat** (your own footprint on a number) and how your runs there went;
//! what's *behind* a number (tier, traits, ICE) is not its business. Its wardialer keeps a map
//! of every block you've scanned, and its ranges page lists what you know and where from.

use crate::data::Data;
use crate::hub::{Entry, Find, Found, RangeDef, Save};
use crate::tuning::HEAT_MAX;

/// NiGHTCOMM's pages, in key order (`1` to `4`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Intel,
    Wardial,
    Ranges,
    /// A modem terminal: type AT commands.
    Term,
}

impl Tab {
    pub const ALL: [Tab; 4] = [Tab::Intel, Tab::Wardial, Tab::Ranges, Tab::Term];

    /// The box title, l33t like the rest of the program.
    pub fn title(self) -> &'static str {
        match self {
            Tab::Intel => "iNTEL",
            Tab::Wardial => "WARDiAL",
            Tab::Ranges => "RANGES",
            Tab::Term => "TERM",
        }
    }

    /// The tab a digit key picks (`'1'` is the first).
    pub fn from_key(c: char) -> Option<Tab> {
        let i = c.to_digit(10)?.checked_sub(1)? as usize;
        Tab::ALL.get(i).copied()
    }
}

/// How the iNTEL page orders the phonebook.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Sort {
    /// Hottest first: where you're expected.
    #[default]
    Heat,
    /// Most runs first.
    Runs,
    Number,
    Name,
}

impl Sort {
    pub const ALL: [Sort; 4] = [Sort::Heat, Sort::Runs, Sort::Number, Sort::Name];

    pub fn label(self) -> &'static str {
        match self {
            Sort::Heat => "heat",
            Sort::Runs => "runs",
            Sort::Number => "number",
            Sort::Name => "name",
        }
    }

    /// The next order, for the sort key.
    pub fn next(self) -> Sort {
        let i = Sort::ALL.iter().position(|s| *s == self).unwrap_or(0);
        Sort::ALL[(i + 1) % Sort::ALL.len()]
    }
}

/// Heat at or above this reads HOT.
pub const HOT: u32 = HEAT_MAX.div_ceil(2) + 1;

/// One word for how hot a number is: `cold`, `warm` or `HOT`.
pub fn heat_word(heat: u32) -> &'static str {
    match heat {
        0 => "cold",
        h if h >= HOT => "HOT",
        _ => "warm",
    }
}

/// The phonebook as the iNTEL page lists it: matching `query` (any part of the name, the
/// business or the number, ignoring case and dashes), in `sort` order. Ties keep the
/// phonebook's own order.
pub fn intel<'a>(save: &'a Save, query: &str, sort: Sort) -> Vec<&'a Entry> {
    let q = query.trim().to_lowercase();
    let digits = |s: &str| s.chars().filter(char::is_ascii_digit).collect::<String>();
    let qd = digits(&q);
    let mut rows: Vec<&Entry> = save
        .phonebook
        .iter()
        .filter(|e| {
            q.is_empty()
                || e.name.to_lowercase().contains(&q)
                || e.business.to_lowercase().contains(&q)
                || (!qd.is_empty() && digits(&e.number).contains(&qd))
        })
        .collect();
    match sort {
        Sort::Heat => rows.sort_by_key(|e| std::cmp::Reverse((e.heat, e.runs))),
        Sort::Runs => rows.sort_by_key(|e| std::cmp::Reverse(e.runs)),
        Sort::Number => rows.sort_by(|a, b| a.number.cmp(&b.number)),
        Sort::Name => rows.sort_by_key(|e| e.name.to_lowercase()),
    }
    rows
}

/// A range you know, as the RANGES page lists it.
#[derive(Clone, Debug)]
pub struct RangeRow<'a> {
    pub def: &'a RangeDef,
    /// How many known ranges it sits inside (a core range under its company's lines is 1).
    pub depth: usize,
    /// Where you learned it.
    pub source: &'a str,
    pub scanned: u32,
    pub blocks: u32,
    /// Networks and boards its scans turned up.
    pub hits: usize,
    pub watched: usize,
}

/// Every range you know, each followed by the narrower ranges inside it. Unknown ranges are
/// never listed: the OS doesn't hint at what you haven't found.
pub fn ranges<'a>(save: &'a Save, data: &'a Data) -> Vec<RangeRow<'a>> {
    let known: Vec<(&RangeDef, &str)> = save
        .ranges
        .iter()
        .filter_map(|k| {
            data.exchanges
                .iter()
                .find(|r| r.id == k.id)
                .map(|r| (r, k.source.as_str()))
        })
        .collect();
    // A range's parent is the known range with the longest prefix that starts it.
    let parent = |r: &RangeDef| {
        known
            .iter()
            .filter(|(o, _)| o.id != r.id && r.prefix.starts_with(&o.prefix))
            .max_by_key(|(o, _)| o.prefix.len())
            .map(|(o, _)| o.id.as_str())
    };
    let row = |r: &'a RangeDef, source: &'a str, depth: usize| {
        let finds = save.finds.iter().filter(|f| f.range == r.id);
        let watched = finds.clone().filter(|f| f.what == Found::Watched).count();
        RangeRow {
            def: r,
            depth,
            source,
            scanned: save.scanned_in(&r.id) as u32,
            blocks: r.blocks(),
            hits: finds.count() - watched,
            watched,
        }
    };
    fn walk<'a>(
        at: Option<&str>,
        depth: usize,
        known: &[(&'a RangeDef, &'a str)],
        parent: &dyn Fn(&RangeDef) -> Option<&'a str>,
        row: &dyn Fn(&'a RangeDef, &'a str, usize) -> RangeRow<'a>,
        out: &mut Vec<RangeRow<'a>>,
    ) {
        for (r, source) in known {
            if parent(r) == at {
                out.push(row(r, source, depth));
                walk(Some(&r.id), depth + 1, known, parent, row, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(None, 0, &known, &parent, &row, &mut out);
    out
}

/// What the map shows for one 100-number block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Block {
    /// Not dialed yet.
    Open,
    /// Dialed, nothing worth keeping.
    Dry,
    /// Dialed: this many networks and boards answered, and whether a watched line is in it.
    Hits { found: u32, watched: bool },
    /// Dialed: only watched lines.
    Watched,
}

/// Every block of `range`, in order.
pub fn block_map(save: &Save, range: &RangeDef) -> Vec<Block> {
    (0..range.blocks())
        .map(|b| {
            if !save.scanned.contains(&format!("{}:{b}", range.id)) {
                return Block::Open;
            }
            let here = || finds(save, &range.id, b);
            let watched = here().any(|f| f.what == Found::Watched);
            let found = here().filter(|f| f.what != Found::Watched).count() as u32;
            match (found, watched) {
                (0, false) => Block::Dry,
                (0, true) => Block::Watched,
                (found, watched) => Block::Hits { found, watched },
            }
        })
        .collect()
}

/// The finds in one block of a range.
pub fn finds<'a>(save: &'a Save, range: &'a str, block: u32) -> impl Iterator<Item = &'a Find> {
    save.finds
        .iter()
        .filter(move |f| f.range == range && f.block == block)
}

/// Every watched line you know, in the order found.
pub fn watched(save: &Save) -> Vec<&Find> {
    save.finds
        .iter()
        .filter(|f| f.what == Found::Watched)
        .collect()
}

/// Whether `number` is a watched line you've found.
pub fn is_watched(save: &Save, number: &str) -> bool {
    save.finds
        .iter()
        .any(|f| f.what == Found::Watched && f.number == number)
}
