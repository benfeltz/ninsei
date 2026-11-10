//! The ICE's program as the deck has seen it run: the lines it watched execute in clear, the
//! rest `????`. Every deck's fight screen shows it with the ICE's cursor and the line NEXT acts
//! on, and the ICE DB prints it from everything the save has seen. Decided here, once; each
//! deck's words are its own module's (`btop_fight`, `suite`, `stack_fight`).

use crate::basic::{self, Program, Verb, Vocab};
use crate::game::Game;
use crate::listing::fighting;

/// What a line the deck hasn't seen run reads.
pub const UNSEEN: &str = "????";

/// One line of the ICE's program.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IceRow {
    pub no: u32,
    /// The statement, once seen run; `None` until then.
    pub text: Option<String>,
    /// The line the ICE carries on from next turn.
    pub cursor: bool,
    /// The line NEXT's move is on.
    pub next: bool,
}

impl IceRow {
    /// The statement, or `????`.
    pub fn shown(&self) -> &str {
        self.text.as_deref().unwrap_or(UNSEEN)
    }
}

/// An ICE's program decompiled from the lines in `known` (sorted): every line numbered, the
/// known ones in clear. No marks.
pub fn decompile(program: &Program, known: &[u32]) -> Vec<IceRow> {
    program
        .lines
        .iter()
        .map(|l| IceRow {
            no: l.no,
            text: known.binary_search(&l.no).is_ok().then(|| l.text.clone()),
            cursor: false,
            next: false,
        })
        .collect()
}

/// An ICE's program from its data, decompiled from `known`: what the ICE DB prints.
pub fn decompile_def(program: &str, verbs: &[Verb], known: &[u32]) -> Vec<IceRow> {
    decompile(&basic::parse_with(program, Vocab::Ice(verbs)), known)
}

/// The ICE in front, as the deck knows it, with its cursor and NEXT's line marked: the
/// fight's own parse of its program. Empty outside a fight. Build it once a frame and hand it
/// to each deck's words.
pub fn rows(g: &Game) -> Vec<IceRow> {
    let Some(c) = g.combat.as_ref().filter(|_| fighting(g)) else {
        return Vec::new();
    };
    let cursor = c.ice_prog.rest(c.ice_exec.pc.or(c.ice_prog.first()));
    // A spoofed or lagged ICE has no NEXT to mark (`ice_next_at` is `None`).
    let next = g.ice_next_at().map(|(no, _)| no);
    // A spoof hides where it carries on from too, which would give the move away.
    let cursor = cursor.filter(|_| c.spoofed == 0);
    let mut rows = decompile(&c.ice_prog, g.ice_seen_lines(c.ice));
    for r in &mut rows {
        r.cursor = Some(r.no) == cursor;
        r.next = Some(r.no) == next;
    }
    rows
}

/// The ICE card's heading over the listing (the Sense/Net's, and the classic panes'): how much
/// of its program the deck has seen.
pub fn heading(rows: &[IceRow]) -> String {
    let (seen, all) = known(rows);
    format!("its program: {seen} of {all} lines seen")
}

/// A row as the ICE card prints it: the cursor's ▶, the number right-aligned in five cells,
/// the statement or `????`.
pub fn card_line(r: &IceRow) -> String {
    let mark = if r.cursor { "▶" } else { " " };
    format!("{mark}{:>5} {}", r.no, r.shown())
}

/// Adds line `no` to `lines`, kept sorted, each once: how a deck remembers a line it saw.
pub fn note_line(lines: &mut Vec<u32>, no: u32) {
    if let Err(at) = lines.binary_search(&no) {
        lines.insert(at, no);
    }
}

/// Line numbers as a deck keeps them: sorted, each once, whatever order they came in.
pub fn tidy(mut lines: Vec<u32>) -> Vec<u32> {
    lines.sort_unstable();
    lines.dedup();
    lines
}

/// How many of the program's lines are known: (seen, all).
pub fn known(rows: &[IceRow]) -> (usize, usize) {
    (rows.iter().filter(|r| r.text.is_some()).count(), rows.len())
}

/// How a version's line differs from the version before, as the ICE DB marks it: only ever on a
/// line seen to run (a removed line only once the whole newer listing has been).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    /// Unchanged, unseen, or the first version met: nothing to say.
    None,
    /// Not in the version before.
    New,
    /// In the version before, somewhere else.
    Moved,
    /// In the version before, gone from this one.
    Removed,
}

impl Mark {
    /// The one-character column the ICE DB prints it in.
    pub fn glyph(self) -> char {
        match self {
            Mark::None => ' ',
            Mark::New => '+',
            Mark::Moved => '~',
            Mark::Removed => '-',
        }
    }
}

/// A row of a version's listing in the ICE DB, marked against the version before.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkedRow {
    /// Its line number in this version; for a removed line, its number in the version before.
    pub no: u32,
    /// The statement if it was seen to run (in this version, or for a removed line in the one
    /// before); `None` reads `????`.
    pub text: Option<String>,
    pub mark: Mark,
}

impl MarkedRow {
    pub fn shown(&self) -> &str {
        self.text.as_deref().unwrap_or(UNSEEN)
    }
}

/// A version's listing as the ICE DB keeps it: `now` (its program and the lines seen run, sorted)
/// decompiled, each seen line marked against `prev` (the version before, as it was known), or
/// unmarked when there's none. Lines are matched by statement, in order (the longest run they
/// share), so a moved block is `~` and the lines around it aren't; a statement the version
/// before didn't have is `+`. The version before's lines that are gone follow as `-`, only once
/// every line of `now` has been seen, with their text only if they were seen there. Nothing is
/// ever shown that wasn't seen.
pub fn changes(
    verbs: &[Verb],
    prev: Option<(&str, &[u32])>,
    now: (&str, &[u32]),
) -> Vec<MarkedRow> {
    let p = basic::parse_with(now.0, Vocab::Ice(verbs));
    let seen = |known: &[u32], no: u32| known.binary_search(&no).is_ok();
    let Some((before, known)) = prev else {
        return p
            .lines
            .iter()
            .map(|l| MarkedRow {
                no: l.no,
                text: seen(now.1, l.no).then(|| l.text.clone()),
                mark: Mark::None,
            })
            .collect();
    };
    let q = basic::parse_with(before, Vocab::Ice(verbs));
    let (a, b): (Vec<&basic::Stmt>, Vec<&basic::Stmt>) = (
        q.lines.iter().map(|l| &l.stmt).collect(),
        p.lines.iter().map(|l| &l.stmt).collect(),
    );
    let (kept_a, kept_b) = common(&a, &b);
    let mut rows: Vec<MarkedRow> = p
        .lines
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let is_seen = seen(now.1, l.no);
            let mark = match () {
                _ if !is_seen || kept_b[i] => Mark::None,
                _ if a.contains(&&l.stmt) => Mark::Moved,
                _ => Mark::New,
            };
            MarkedRow {
                no: l.no,
                text: is_seen.then(|| l.text.clone()),
                mark,
            }
        })
        .collect();
    let all_seen = p.lines.iter().all(|l| seen(now.1, l.no));
    if all_seen {
        rows.extend(
            q.lines
                .iter()
                .enumerate()
                .filter(|(i, l)| !kept_a[*i] && !b.contains(&&l.stmt))
                .map(|(_, l)| MarkedRow {
                    no: l.no,
                    text: seen(known, l.no).then(|| l.text.clone()),
                    mark: Mark::Removed,
                }),
        );
    }
    rows
}

/// Which items of `a` and `b` are in their longest common run (in order): (a's, b's).
fn common<T: PartialEq>(a: &[T], b: &[T]) -> (Vec<bool>, Vec<bool>) {
    let (n, m) = (a.len(), b.len());
    let mut len = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            len[i][j] = if a[i] == b[j] {
                len[i + 1][j + 1] + 1
            } else {
                len[i + 1][j].max(len[i][j + 1])
            };
        }
    }
    let (mut ka, mut kb) = (vec![false; n], vec![false; m]);
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if a[i] == b[j] {
            ka[i] = true;
            kb[j] = true;
            i += 1;
            j += 1;
        } else if len[i + 1][j] >= len[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    (ka, kb)
}
