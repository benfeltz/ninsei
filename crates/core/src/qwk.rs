//! The QWK mail reader: an offline reader for the boards you know. Each board's packet holds
//! its bulletins and the textfiles you downloaded from it (`Save::texts`). It draws pipe-coded
//! text at most [`COLS`]x[`ROWS`], like a BBS screen, and reading never changes play.

use crate::bbs::{COLS, ROWS};
use crate::data::Data;
use crate::hub::Save;

/// One message in a board's packet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    /// A bulletin, by index into the board's `bulletins`.
    Bulletin(usize),
    /// A downloaded textfile, by id in the board's `texts`.
    Text(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    /// The packets: one per board you know.
    Boards,
    /// A board's messages, by index into `Data::boards`.
    Board(usize),
    /// Reading message `item` of a board's list.
    Read { board: usize, item: usize },
}

/// Messages listed per page (hotkeys 1-9).
pub const PAGE: usize = 9;

#[derive(Clone, Debug)]
pub struct Reader {
    pub view: View,
    /// First message shown on a board's page.
    pub page: usize,
}

impl Default for Reader {
    fn default() -> Self {
        Self::new()
    }
}

/// The boards with a packet: every board you know, in phonebook order.
pub fn boards(save: &Save, data: &Data) -> Vec<usize> {
    save.known_boards(data)
}

/// A board's messages: its bulletins, then the textfiles you've downloaded from it.
pub fn items(save: &Save, data: &Data, board: usize) -> Vec<Item> {
    let b = &data.boards[board];
    let mut v: Vec<Item> = (0..b.bulletins.len()).map(Item::Bulletin).collect();
    v.extend(
        save.texts
            .iter()
            .filter(|k| k.board == b.id && b.texts.iter().any(|t| t.id == k.id))
            .map(|k| Item::Text(k.id.clone())),
    );
    v
}

/// A message's title and body lines.
pub fn message<'a>(data: &'a Data, board: usize, item: &Item) -> Option<(&'a str, &'a [String])> {
    let b = &data.boards[board];
    match item {
        Item::Bulletin(i) => b.bulletins.get(*i).map(|x| (x.title.as_str(), &x.body[..])),
        Item::Text(id) => b
            .texts
            .iter()
            .find(|t| t.id == *id)
            .map(|t| (t.title.as_str(), &t.lines[..])),
    }
}

fn kind(item: &Item) -> &'static str {
    match item {
        Item::Bulletin(_) => "BULLETIN",
        Item::Text(_) => "TEXTFILE",
    }
}

/// The reader's header bar, exactly [`COLS`] wide.
fn header(out: &mut Vec<String>, right: &str) {
    let left = " OLX QWK READER 2.1 ";
    let right = format!(" {right} ");
    let pad = COLS.saturating_sub(left.chars().count() + right.chars().count());
    out.push(format!("|17|15{left}{}|14{right}|16", " ".repeat(pad)));
}

impl Reader {
    pub fn new() -> Self {
        Self {
            view: View::Boards,
            page: 0,
        }
    }

    /// The hotkeys this view takes, with labels (for clickable buttons).
    pub fn keys(&self, save: &Save, data: &Data) -> Vec<(char, String)> {
        let digits = |n: usize| {
            (1..=n.min(PAGE))
                .filter_map(|i| char::from_digit(i as u32, 10).map(|c| (c, String::new())))
                .collect::<Vec<_>>()
        };
        match self.view {
            View::Boards => digits(boards(save, data).len()),
            View::Board(b) => {
                let n = items(save, data, b).len();
                let mut v = digits(n.saturating_sub(self.page));
                if self.page + PAGE < n {
                    v.push(('N', "next page".into()));
                }
                if self.page > 0 {
                    v.push(('P', "prev page".into()));
                }
                v.push(('Q', "back".into()));
                v
            }
            View::Read { .. } => vec![
                ('N', "next".into()),
                ('P', "prev".into()),
                ('Q', "back".into()),
            ],
        }
    }

    /// Handles one keypress. Letters are case-insensitive; Escape counts as `Q`.
    pub fn key(&mut self, save: &Save, data: &Data, key: char) {
        let k = key.to_ascii_uppercase();
        let back = k == 'Q' || k == '\x1b';
        let digit = k.to_digit(10).map(|d| d as usize).filter(|d| *d >= 1);
        match self.view {
            View::Boards => {
                let bs = boards(save, data);
                if let Some(b) = digit.and_then(|d| bs.get(d - 1)) {
                    self.view = View::Board(*b);
                    self.page = 0;
                }
            }
            View::Board(b) => {
                let n = items(save, data, b).len();
                match digit {
                    Some(d) if self.page + d <= n && d <= PAGE => {
                        self.view = View::Read {
                            board: b,
                            item: self.page + d - 1,
                        };
                    }
                    _ if k == 'N' && self.page + PAGE < n => self.page += PAGE,
                    _ if k == 'P' => self.page = self.page.saturating_sub(PAGE),
                    _ if back => self.view = View::Boards,
                    _ => {}
                }
            }
            View::Read { board, item } => {
                let n = items(save, data, board).len();
                if k == 'N' && item + 1 < n {
                    self.view = View::Read {
                        board,
                        item: item + 1,
                    };
                } else if k == 'P' && item > 0 {
                    self.view = View::Read {
                        board,
                        item: item - 1,
                    };
                } else if back || k == '\n' {
                    self.page = item / PAGE * PAGE;
                    self.view = View::Board(board);
                }
            }
        }
    }

    /// The whole screen as pipe-coded lines, at most [`ROWS`] of them.
    pub fn render(&self, save: &Save, data: &Data) -> Vec<String> {
        let mut out = Vec::new();
        let hot = |k: &str| format!("|09[|15{k}|09]");
        match self.view {
            View::Boards => {
                header(&mut out, "PACKETS");
                out.push(String::new());
                let bs = boards(save, data);
                if bs.is_empty() {
                    out.push("  |07No packets. Call a board first.".into());
                }
                for (i, &b) in bs.iter().enumerate().take(PAGE) {
                    let d = &data.boards[b];
                    let n = items(save, data, b).len();
                    out.push(format!(
                        "  {} |11{:<30}|08{:<10}|07{n} message(s)",
                        hot(&(i + 1).to_string()),
                        d.name,
                        d.phone
                    ));
                }
                out.push(String::new());
                out.push(format!("  |07Open which? {} |07#", hot("#")));
            }
            View::Board(b) => {
                let d = &data.boards[b];
                header(&mut out, &d.name);
                out.push(String::new());
                out.push(format!("  |08{:<4}{:<10}{}", "#", "KIND", "SUBJECT"));
                out.push(format!("  |08{}", "─".repeat(70)));
                let all = items(save, data, b);
                for (i, it) in all.iter().enumerate().skip(self.page).take(PAGE) {
                    let title = message(data, b, it).map_or("", |m| m.0);
                    let title: String = title.chars().take(60).collect();
                    out.push(format!(
                        "  {} |03{:<10}|15{title}",
                        hot(&(i - self.page + 1).to_string()),
                        kind(it)
                    ));
                }
                out.push(String::new());
                let mut nav = format!("  |07Read which? {} ", hot("#"));
                if self.page + PAGE < all.len() {
                    nav.push_str(&format!("{} |07next page ", hot("N")));
                }
                if self.page > 0 {
                    nav.push_str(&format!("{} |07prev page ", hot("P")));
                }
                nav.push_str(&format!("{} |07back", hot("Q")));
                out.push(nav);
            }
            View::Read { board, item } => {
                let d = &data.boards[board];
                let all = items(save, data, board);
                header(&mut out, &format!("{} {}/{}", d.name, item + 1, all.len()));
                if let Some((title, body)) = all.get(item).and_then(|it| message(data, board, it)) {
                    out.push(format!("  |08From: |11{}  |08Subj: |14{title}", d.sysop));
                    out.push(format!("  |08{}", "─".repeat(title.chars().count() + 6)));
                    out.extend(body.iter().map(|l| format!("  |07{l}")));
                }
                out.push(String::new());
                out.push(format!(
                    "  {} |07next  {} |07prev  {} |07back",
                    hot("N"),
                    hot("P"),
                    hot("Q")
                ));
            }
        }
        out.truncate(ROWS);
        out
    }
}
