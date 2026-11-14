//! The ICE's program as the deck has it: the source you own of the version in front, listed
//! whole, or nothing at all. Fighting an ICE shows none of its program; only its source does.
//! Every deck's fight screen shows it with the ICE's cursor and the line NEXT acts on, and the
//! ICE DB prints the sources you own. Decided here, once; each deck's words are its own
//! module's (`btop_fight`, `suite`, `stack_fight`).

use crate::basic::{self, Program, Verb, Vocab};
use crate::game::Game;
use crate::listing::fighting;

/// One line of the ICE's program, or a remark row its source carries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IceRow {
    /// Its line number; 0 for a remark row.
    pub no: u32,
    /// The statement, or the remark.
    pub text: String,
    /// The line the ICE carries on from next turn (never a remark row).
    pub cursor: bool,
    /// The line NEXT's move is on (never a remark row).
    pub next: bool,
    /// A remark row: the header or a comment the source's board put on it, never part of the
    /// ICE's program (never run, never counted against its machine).
    pub rem: bool,
}

impl IceRow {
    fn remark(text: String) -> IceRow {
        IceRow {
            no: 0,
            text,
            cursor: false,
            next: false,
            rem: true,
        }
    }
}

/// A source's notes: the header and remarks its board (`IceSource::from`) puts on kind `ice`
/// at `version`, if that board has a `sources` block. `None` for a source from no board.
pub fn notes<'a>(
    data: &'a crate::data::Data,
    from: &str,
    ice: &str,
    version: crate::ice_version::Version,
) -> Option<(Vec<String>, &'a crate::data::SourcesDef)> {
    let b = data.boards.get(data.board(from)?)?;
    let s = b.sources.as_ref()?;
    Some((s.header_lines(ice, &version.to_string(), &b.name), s))
}

/// `rows` with a source's notes, whatever a row is: the header first, then each row, a remark
/// after each whose statement (`text`; `None`: one that takes none) is an IF its board's
/// remarks know. `rem` makes a note's row. The one place notes are placed.
pub fn interleave<T>(
    rows: Vec<T>,
    header: Vec<String>,
    sources: &crate::data::SourcesDef,
    text: impl Fn(&T) -> Option<&str>,
    rem: impl Fn(String) -> T,
) -> Vec<T> {
    let mut out: Vec<T> = header.into_iter().map(&rem).collect();
    for r in rows {
        let remark = text(&r).and_then(|t| sources.remark(t)).map(str::to_string);
        out.push(r);
        out.extend(remark.map(&rem));
    }
    out
}

/// A fight's rows with a source's notes ([`interleave`]): the rows that aren't the program
/// are `rem`.
pub fn annotate(
    rows: Vec<IceRow>,
    header: Vec<String>,
    sources: &crate::data::SourcesDef,
) -> Vec<IceRow> {
    interleave(
        rows,
        header,
        sources,
        |r| Some(r.text.as_str()),
        IceRow::remark,
    )
}

/// A note as the full listings print it (the ICE card, the ICE DB), under the statements.
pub fn rem_line(text: &str) -> String {
    format!("       REM {text}")
}

/// An ICE's program listed whole, every line numbered. No marks.
pub fn decompile(program: &Program) -> Vec<IceRow> {
    program
        .lines
        .iter()
        .map(|l| IceRow {
            no: l.no,
            text: l.text.clone(),
            cursor: false,
            next: false,
            rem: false,
        })
        .collect()
}

/// An ICE's program from its text, listed whole: what the ICE DB prints of a source.
pub fn decompile_def(program: &str, verbs: &[Verb]) -> Vec<IceRow> {
    decompile(&basic::parse_with(program, Vocab::Ice(verbs)))
}

/// The ICE in front, as the deck has it, with its cursor and NEXT's line marked: the fight's
/// own parse of its program, when the deck owns the source of the version it's fighting
/// (`Combat::listed`); otherwise nothing at all, not even how many lines there are. Empty
/// outside a fight. Build it once a frame and hand it to each deck's words.
pub fn rows(g: &Game) -> Vec<IceRow> {
    let Some(c) = g.combat.as_ref().filter(|_| fighting(g)) else {
        return Vec::new();
    };
    if !c.listed {
        return Vec::new();
    }
    let cursor = c.ice_prog.rest(c.ice_exec.pc.or(c.ice_prog.first()));
    // A spoofed or lagged ICE has no NEXT to mark (`ice_next_at` is `None`).
    let next = g.ice_next_at().map(|(no, _)| no);
    // A spoof hides where it carries on from too, which would give the move away.
    let cursor = cursor.filter(|_| c.spoofed == 0);
    let mut rows = decompile(&c.ice_prog);
    for r in &mut rows {
        r.cursor = Some(r.no) == cursor;
        r.next = Some(r.no) == next;
    }
    // The source's board's notes, if it came from one.
    let notes = notes(
        &g.data,
        &c.source_from,
        &g.data.ice[c.ice].name,
        c.ice_version,
    );
    match notes {
        Some((header, sources)) => annotate(rows, header, sources),
        None => rows,
    }
}

/// A deck's words for the ICE's listing: the heading over it, and what stands in its place in a
/// fight when the deck has no source for the version in front (`rows` is empty). The words say
/// what is there, as that deck's software would, and never where a source might come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IceWords {
    pub heading: &'static str,
    pub none: &'static str,
}

/// The classic panes' words: the terminal's caps.
pub const CLASSIC: IceWords = IceWords {
    heading: "PROGRAM",
    none: "NO SOURCE.",
};

/// A row as the ICE card prints it: the cursor's ▶, the number right-aligned in five cells,
/// the statement.
pub fn card_line(r: &IceRow) -> String {
    if r.rem {
        return rem_line(&r.text);
    }
    let mark = if r.cursor { "▶" } else { " " };
    format!("{mark}{:>5} {}", r.no, r.text)
}

/// How a version's line differs from the version before, as the ICE DB marks it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    /// Unchanged, or no version before to compare with: nothing to say.
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
    /// The statement (for a removed line, as the version before had it).
    pub text: String,
    pub mark: Mark,
}

/// A version's listing as the ICE DB prints it from two sources you own: `now` listed whole,
/// marked against `prev` (the version before it that you own), or unmarked when there's none.
/// It's compared a block at a time (an IF and the line it guards, or a line on its own: the
/// unit a version moves, adds or removes): the blocks the two share in order (the longest run)
/// are unmarked; a block of `now` the run leaves over is `~` on both its lines if `prev` has it
/// elsewhere, changed inside (only the changed lines `+`) if `prev` has a block with the same
/// read left over, and `+` otherwise. `prev`'s lines that are gone follow as `-`.
pub fn changes(verbs: &[Verb], prev: Option<&str>, now: &str) -> Vec<MarkedRow> {
    let p = basic::parse_with(now, Vocab::Ice(verbs));
    let Some(before) = prev else {
        return p
            .lines
            .iter()
            .map(|l| MarkedRow {
                no: l.no,
                text: l.text.clone(),
                mark: Mark::None,
            })
            .collect();
    };
    let q = basic::parse_with(before, Vocab::Ice(verbs));
    let (uq, up) = (units(&q), units(&p));
    let stmts = |u: &[usize], prog: &'_ Program| -> Vec<basic::Stmt> {
        u.iter().map(|&k| prog.lines[k].stmt.clone()).collect()
    };
    let a: Vec<Vec<basic::Stmt>> = uq.iter().map(|u| stmts(u, &q)).collect();
    let b: Vec<Vec<basic::Stmt>> = up.iter().map(|u| stmts(u, &p)).collect();
    let na: Vec<u32> = uq.iter().map(|u| q.lines[u[0]].no).collect();
    let nb: Vec<u32> = up.iter().map(|u| p.lines[u[0]].no).collect();
    let (kept_a, kept_b) = common(&a, &b, &na, &nb);
    // The blocks the common run leaves over: each can account for one block of `now`. The
    // same block is a move; one with the same read (or the same line) changed inside, and only
    // its changed lines are marked. What's still left was removed.
    let mut spare: Vec<bool> = kept_a.iter().map(|k| !k).collect();
    let mut gone: Vec<usize> = Vec::new();
    let mut marks = vec![Mark::None; p.lines.len()];
    for (n, lines) in up.iter().enumerate() {
        if kept_b[n] {
            continue;
        }
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
                    marks[line] = Mark::New;
                    gone.push(uq[j][i]);
                }
            }
        } else {
            for &line in lines {
                marks[line] = Mark::New;
            }
        }
    }
    gone.extend(
        uq.iter()
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
            text: l.text.clone(),
            mark,
        })
        .collect();
    rows.extend(gone.into_iter().map(|k| MarkedRow {
        no: q.lines[k].no,
        text: q.lines[k].text.clone(),
        mark: Mark::Removed,
    }));
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
