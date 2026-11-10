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
