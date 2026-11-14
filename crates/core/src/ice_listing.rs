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
/// unmarked when there's none. Only what was seen is compared, a block at a time (an IF and the
/// line it guards, or a line on its own: the unit a version moves, adds or removes): a block
/// seen whole is matched against the blocks seen whole in the version before, by statement, in
/// order (the longest run they share), so a moved block is `~` on both its lines and the blocks
/// around it aren't (the unseen take no part); it's `+` only when the version before
/// was seen whole and didn't have it (otherwise it may have been one of the unseen lines, and
/// there's no mark). The version before's seen lines that are gone follow as `-`, once every line
/// of `now` has been seen. Nothing is ever shown, or implied, that wasn't seen.
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
    let (uq, up) = (units(&q), units(&p));
    let all_seen = |u: &[usize], prog: &Program, known: &[u32]| {
        u.iter().all(|&k| seen(known, prog.lines[k].no))
    };
    // The version before as it was known (its blocks seen whole), and this one as seen so far:
    // the unseen take no part.
    let was: Vec<&Vec<usize>> = uq.iter().filter(|u| all_seen(u, &q, known)).collect();
    let whole = was.len() == uq.len();
    let is: Vec<usize> = (0..up.len())
        .filter(|&k| all_seen(&up[k], &p, now.1))
        .collect();
    let stmts = |u: &[usize], prog: &'_ Program| -> Vec<basic::Stmt> {
        u.iter().map(|&k| prog.lines[k].stmt.clone()).collect()
    };
    let a: Vec<Vec<basic::Stmt>> = was.iter().map(|u| stmts(u, &q)).collect();
    let b: Vec<Vec<basic::Stmt>> = is.iter().map(|&k| stmts(&up[k], &p)).collect();
    let na: Vec<u32> = was.iter().map(|u| q.lines[u[0]].no).collect();
    let nb: Vec<u32> = is.iter().map(|&k| p.lines[up[k][0]].no).collect();
    let (kept_a, kept_b) = common(&a, &b, &na, &nb);
    // The known blocks the common run leaves over: each can account for one block of `now`. The
    // same block is a move; one with the same read (or the same line) changed inside, and only
    // its changed lines are marked. What's still left was removed.
    let mut spare: Vec<bool> = kept_a.iter().map(|k| !k).collect();
    let mut gone: Vec<usize> = Vec::new();
    let mut marks = vec![Mark::None; p.lines.len()];
    let new = if whole { Mark::New } else { Mark::None };
    for (n, &k) in is.iter().enumerate() {
        if kept_b[n] {
            continue;
        }
        let lines = &up[k];
        if let Some(j) = (0..a.len()).find(|&j| spare[j] && a[j] == b[n]) {
            spare[j] = false;
            for &line in lines {
                marks[line] = Mark::Moved;
            }
        } else if let Some(j) = (0..a.len())
            .find(|&j| spare[j] && a[j].len() == b[n].len() && b[n].len() > 1 && a[j][0] == b[n][0])
        {
            spare[j] = false;
            for (i, &line) in lines.iter().enumerate() {
                if a[j][i] != b[n][i] {
                    marks[line] = new;
                    gone.push(was[j][i]);
                }
            }
        } else {
            for &line in lines {
                marks[line] = new;
            }
        }
    }
    gone.extend(
        was.iter()
            .zip(&spare)
            .filter(|(_, s)| **s)
            .flat_map(|(u, _)| u.iter().copied()),
    );
    gone.sort_unstable();
    let mut rows: Vec<MarkedRow> = p
        .lines
        .iter()
        .zip(marks)
        .map(|(l, mark)| MarkedRow {
            no: l.no,
            text: seen(now.1, l.no).then(|| l.text.clone()),
            mark,
        })
        .collect();
    if p.lines.iter().all(|l| seen(now.1, l.no)) {
        rows.extend(gone.into_iter().map(|k| MarkedRow {
            no: q.lines[k].no,
            text: Some(q.lines[k].text.clone()),
            mark: Mark::Removed,
        }));
    }
    rows
}

/// A program's lines in the units a version changes: an IF and the line it guards, or a line on
/// its own (indices into `Program::lines`).
fn units(p: &Program) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    let mut k = 0;
    while k < p.lines.len() {
        if matches!(p.lines[k].stmt, basic::Stmt::If(_)) && k + 1 < p.lines.len() {
            out.push(vec![k, k + 1]);
            k += 2;
        } else {
            out.push(vec![k]);
            k += 1;
        }
    }
    out
}

/// Which blocks of `a` and `b` are in their common run (in order) that keeps the most lines:
/// (a's, b's). Of those, the one that keeps the most blocks at the same number (`na`, `nb`):
/// versions are numbered by tens, so a block that stayed where it was is the likelier match.
fn common<T: PartialEq>(
    a: &[Vec<T>],
    b: &[Vec<T>],
    na: &[u32],
    nb: &[u32],
) -> (Vec<bool>, Vec<bool>) {
    let (n, m) = (a.len(), b.len());
    // Score: lines kept first, then blocks kept at the same number.
    let score =
        |i: usize, j: usize| -> u64 { 1000 * a[i].len() as u64 + u64::from(na[i] == nb[j]) };
    let mut best = vec![vec![0u64; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            let skip = best[i + 1][j].max(best[i][j + 1]);
            best[i][j] = if a[i] == b[j] {
                skip.max(best[i + 1][j + 1] + score(i, j))
            } else {
                skip
            };
        }
    }
    let (mut ka, mut kb) = (vec![false; n], vec![false; m]);
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if a[i] == b[j] && best[i][j] == best[i + 1][j + 1] + score(i, j) {
            ka[i] = true;
            kb[j] = true;
            i += 1;
            j += 1;
        } else if best[i + 1][j] >= best[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    (ka, kb)
}
