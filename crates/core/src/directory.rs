//! CarrierTerm's directory: every number a deck knows, the bulletin boards and the networks,
//! searched and filtered the way the app shows them. It lists who they are and how to reach
//! them, never what's behind a number (that's NiGHTCOMM's business).

use crate::data::Data;
use crate::hub::Save;

/// What a line in the directory points at.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Listed {
    /// A phonebook entry, by [`crate::hub::Entry::id`]: dialing it jacks in.
    Entry(u32),
    /// A known bulletin board, by index into [`Data::boards`]: calling it opens the board.
    Board(usize),
}

/// The directory's filter buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Filter {
    #[default]
    All,
    /// Networks: the companies, schools and the rest of the phonebook.
    Businesses,
    /// Bulletin boards.
    Boards,
}

impl Filter {
    pub const ALL: [Filter; 3] = [Filter::All, Filter::Businesses, Filter::Boards];

    pub fn label(self) -> &'static str {
        match self {
            Filter::All => "All",
            Filter::Businesses => "Businesses",
            Filter::Boards => "Bulletin boards",
        }
    }

    fn keeps(self, key: Listed) -> bool {
        matches!(
            (self, key),
            (Filter::All, _)
                | (Filter::Businesses, Listed::Entry(_))
                | (Filter::Boards, Listed::Board(_))
        )
    }
}

/// One line of the directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listing {
    pub key: Listed,
    /// `555-0142` style.
    pub number: String,
    pub name: String,
    /// What the directory lists it as: a network's business, or "bulletin board".
    pub listed_as: String,
}

/// What a board is listed as.
pub const BOARD_LISTING: &str = "bulletin board";

/// Every number the deck knows: its boards first, then the phonebook in its own order.
pub fn listings(save: &Save, data: &Data) -> Vec<Listing> {
    let boards = save.known_boards(data).into_iter().map(|b| {
        let d = &data.boards[b];
        Listing {
            key: Listed::Board(b),
            number: d.phone.clone(),
            name: d.name.clone(),
            listed_as: BOARD_LISTING.to_string(),
        }
    });
    let entries = save.phonebook.iter().map(|e| Listing {
        key: Listed::Entry(e.id),
        number: e.number.clone(),
        name: e.name.clone(),
        listed_as: e.business.clone(),
    });
    boards.chain(entries).collect()
}

/// Whether `l` matches what was typed in the search box: any part of its name or listing,
/// ignoring case, or any part of its number, with or without the dash.
pub fn matches(l: &Listing, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return true;
    }
    let digits = |s: &str| s.chars().filter(|c| *c != '-').collect::<String>();
    l.name.to_lowercase().contains(&q)
        || l.listed_as.to_lowercase().contains(&q)
        || l.number.contains(&q)
        || (!digits(&q).is_empty() && digits(&l.number).contains(&digits(&q)))
}

/// The directory as shown: the listings that pass `filter` and match `query`.
pub fn search(save: &Save, data: &Data, query: &str, filter: Filter) -> Vec<Listing> {
    listings(save, data)
        .into_iter()
        .filter(|l| filter.keeps(l.key) && matches(l, query))
        .collect()
}

/// The selection after an arrow key: `step` lines down (or up, if negative) from `current`
/// in `shown`, held at the ends. With nothing selected, or the selection filtered out, it
/// lands on the first line.
pub fn step(shown: &[Listing], current: Option<Listed>, step: i32) -> Option<Listed> {
    let first = shown.first()?.key;
    let Some(at) = current.and_then(|c| shown.iter().position(|l| l.key == c)) else {
        return Some(first);
    };
    let to = (at as i64 + step as i64).clamp(0, shown.len() as i64 - 1) as usize;
    Some(shown[to].key)
}
