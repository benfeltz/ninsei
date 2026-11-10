//! ICE that patch themselves (141.9): a kind's version, and the next version's program written
//! from the last. Pure: nothing here reads a run, the clock or any RNG but the one built from
//! the seed it's given.
//!
//! A version is semver on the kind (`Rototurret 1.2.3`), and 1.0.0 is its row in `ice.ron`. A
//! **patch** moves a block or changes one number, a **minor** adds or removes a read and its
//! response, a **major** replaces a read with a different one. Every version fits the row's
//! machine (cpu, ram, hdd, ranges), so versions climb for ever and never raise it.
//!
//! The generator works on **blocks**: an IF and the line it guards, or a single line. They move,
//! come and go whole, since a false IF skips the line after it. A program with a GOTO, a STOP,
//! a REM, two IFs in a row or an IF as its last line isn't in block form, and no version of it
//! is written.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::basic::{Cond, Program, Stmt, Verb, Vocab, parse_stmt_with, parse_with};
use crate::data::{Data, IceDef, ReadDef};
use crate::rng::Rng;

/// Draws of one kind of change before the generator falls back to a smaller one.
const TRIES: usize = 32;

/// A kind's version, `major.minor.patch`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Default for Version {
    /// 1.0.0: the program on the kind's row in `ice.ron`.
    fn default() -> Self {
        Version::FIRST
    }
}

impl Version {
    /// The version on the row in `ice.ron`.
    pub const FIRST: Version = Version {
        major: 1,
        minor: 0,
        patch: 0,
    };

    /// The version a bump of kind `bump` makes of this one.
    pub fn bumped(self, bump: Bump) -> Version {
        match bump {
            Bump::Patch => Version {
                patch: self.patch + 1,
                ..self
            },
            Bump::Minor => Version {
                minor: self.minor + 1,
                patch: 0,
                ..self
            },
            Bump::Major => Version {
                major: self.major + 1,
                minor: 0,
                patch: 0,
            },
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// How big a change a new version is.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Bump {
    /// A block moved, or one number changed inside its range.
    Patch,
    /// A read and its response added or removed.
    Minor,
    /// A read replaced by a different one.
    Major,
}

impl Bump {
    /// The next smaller change, if any: what the generator falls back to when no change of this
    /// kind fits.
    pub fn smaller(self) -> Option<Bump> {
        match self {
            Bump::Major => Some(Bump::Minor),
            Bump::Minor => Some(Bump::Patch),
            Bump::Patch => None,
        }
    }
}

/// A unit of a program in block form: a read (an IF's condition, as written after `IF`) and the
/// line it guards, or a line on its own. Statements are kept as written, without numbers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub read: Option<String>,
    pub line: String,
}

/// `program` as blocks, or `None` if it isn't in block form: every line one of the ICE's verbs,
/// or an IF guarding one.
pub fn blocks(program: &Program) -> Option<Vec<Block>> {
    if !program.stray.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    let mut lines = program.lines.iter();
    while let Some(l) = lines.next() {
        match &l.stmt {
            Stmt::Act(_) => out.push(Block {
                read: None,
                line: l.text.clone(),
            }),
            Stmt::If(_) => {
                let then = lines.next()?;
                if !matches!(then.stmt, Stmt::Act(_)) {
                    return None;
                }
                let read = l.text.trim_start()[2..].trim().to_string();
                out.push(Block {
                    read: Some(read),
                    line: then.text.clone(),
                });
            }
            _ => return None,
        }
    }
    Some(out)
}

/// Blocks as a program's text, lines numbered by tens.
pub fn render(blocks: &[Block]) -> String {
    let mut out = Vec::new();
    for b in blocks {
        if let Some(r) = &b.read {
            out.push(format!("IF {r}"));
        }
        out.push(b.line.clone());
    }
    out.iter()
        .enumerate()
        .map(|(i, s)| format!("{} {s}", (i + 1) * 10))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The seed a kind's next version is drawn from: the save's key, the kind's id and the version
/// being replaced. Never a run's RNG or the clock, so the same save writes the same versions.
pub fn seed(key: u64, id: &str, version: Version) -> u64 {
    // FNV-1a over the three, then mixed by the RNG's own step.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let bytes = key
        .to_le_bytes()
        .into_iter()
        .chain(id.bytes())
        .chain([0])
        .chain(version.major.to_le_bytes())
        .chain(version.minor.to_le_bytes())
        .chain(version.patch.to_le_bytes());
    for b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    Rng::new(h).next_u64()
}

/// A new version's program, and the change it is: `None` when no change fit and the program is
/// the one given.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Next {
    pub bump: Option<Bump>,
    pub program: String,
}

/// The next version of ICE `row`'s `program`: a change of kind `bump` drawn from `seed`, from
/// its class's reads in `data.ice_grammar` (each read's weight raised by its entry in `bias`, a
/// list of (read as the grammar writes it, extra weight)). Every candidate passes
/// [`Data::check_ice_program`], the check the row's own program passed; a draw that fails is
/// redrawn from the same stream, and when no change of that kind fits it falls back to the next
/// smaller one. Total: at worst it returns `program` unchanged, as it does a program that isn't
/// in block form.
pub fn next_version(
    data: &Data,
    row: &IceDef,
    program: &str,
    bump: Bump,
    bias: &[(String, u32)],
    seed: u64,
) -> Next {
    let unchanged = Next {
        bump: None,
        program: program.to_string(),
    };
    let Some(blocks) = blocks(&parse_with(program, Vocab::Ice(&row.verbs))) else {
        return unchanged;
    };
    let draws = Draws {
        data,
        row,
        reads: data.ice_grammar.reads(row.class),
        bias,
    };
    let before = render(&blocks);
    let mut rng = Rng::new(seed);
    let mut want = Some(bump);
    while let Some(b) = want {
        for _ in 0..TRIES {
            let drawn = match b {
                Bump::Patch => draws.patch(&blocks, &mut rng),
                Bump::Minor => draws.minor(&blocks, &mut rng),
                Bump::Major => draws.major(&blocks, &mut rng),
            };
            let Some(drawn) = drawn else { break };
            let text = render(&drawn);
            if text != before && data.check_ice_program(row, &text).is_ok() {
                return Next {
                    bump: Some(b),
                    program: text,
                };
            }
        }
        want = b.smaller();
    }
    unchanged
}

/// What a draw needs: the row, its class's reads and the bias.
struct Draws<'a> {
    data: &'a Data,
    row: &'a IceDef,
    reads: &'a [ReadDef],
    bias: &'a [(String, u32)],
}

impl Draws<'_> {
    // Each draw returns `None` when its kind of change can't happen to these blocks at all, and
    // otherwise a candidate, which may be the blocks unchanged (an unlucky draw, drawn again).

    /// Moves a block, or changes one number inside its range. `None` when neither can happen.
    fn patch(&self, blocks: &[Block], rng: &mut Rng) -> Option<Vec<Block>> {
        let spots = self.numbers(blocks);
        let can_move = blocks.len() >= 2;
        let move_it = match (can_move, spots.is_empty()) {
            (false, true) => return None,
            (true, false) => rng.below(2) == 0,
            (can, _) => can,
        };
        let mut out = blocks.to_vec();
        if move_it {
            let from = rng.below(out.len());
            let b = out.remove(from);
            // Anywhere but where it was.
            let mut to = rng.below(out.len());
            if to >= from {
                to += 1;
            }
            out.insert(to, b);
            return Some(out);
        }
        let spot = &spots[rng.below(spots.len())];
        let (lo, hi) = spot.range;
        // A new number in the range, never the one it had. One outside it (a row or a saved
        // version may read past its grammar read's range) may become any number in it.
        let n = if (lo..=hi).contains(&spot.now) {
            let n = rng.range(lo, hi - 1);
            n + u32::from(n >= spot.now)
        } else {
            rng.range(lo, hi)
        };
        let b = &mut out[spot.block];
        match &spot.kind {
            Spot::Read(r) => b.read = Some(r.with(n)),
            Spot::Line(v) => b.line = format!("{} {n}", v.word()),
        }
        Some(out)
    }

    /// Adds a read and its response where the HDD has room, or removes a read and its line.
    fn minor(&self, blocks: &[Block], rng: &mut Rng) -> Option<Vec<Block>> {
        let lines: usize = blocks
            .iter()
            .map(|b| 1 + usize::from(b.read.is_some()))
            .sum();
        let can_add = lines + 2 <= self.row.hdd as usize && !self.usable(None).is_empty();
        let guarded: Vec<usize> = (0..blocks.len())
            .filter(|&i| blocks[i].read.is_some())
            .collect();
        let add = match (can_add, guarded.is_empty()) {
            (false, true) => return None,
            (true, false) => rng.below(2) == 0,
            (can, _) => can,
        };
        let mut out = blocks.to_vec();
        if add {
            let read = self.draw_read(None, rng)?;
            if self.has(blocks, read) {
                return Some(out);
            }
            let line = self.draw_line(read, rng)?;
            let n = read.n.map_or(0, |(lo, hi)| rng.range(lo, hi));
            let at = rng.below(out.len() + 1);
            out.insert(
                at,
                Block {
                    read: Some(read.with(n)),
                    line,
                },
            );
        } else {
            out.remove(guarded[rng.below(guarded.len())]);
        }
        Some(out)
    }

    /// Replaces one block's read with a different one from the grammar. The line it guards
    /// stays if the new read may guard it, else it is drawn again.
    fn major(&self, blocks: &[Block], rng: &mut Rng) -> Option<Vec<Block>> {
        let guarded: Vec<usize> = (0..blocks.len())
            .filter(|&i| blocks[i].read.is_some())
            .collect();
        if guarded.is_empty() {
            return None;
        }
        let at = guarded[rng.below(guarded.len())];
        // Any read but the one it replaces (whether or not the grammar wrote it).
        let old = blocks[at]
            .read
            .as_deref()
            .and_then(|r| self.data.read_key(r));
        let read = self.draw_read(old.as_ref(), rng)?;
        if self.has(blocks, read) {
            return Some(blocks.to_vec());
        }
        let n = read.n.map_or(0, |(lo, hi)| rng.range(lo, hi));
        let mut out = blocks.to_vec();
        let keeps = self
            .verb(&out[at].line)
            .is_some_and(|v| read.responses.iter().any(|(r, _)| *r == v));
        if !keeps {
            out[at].line = self.draw_line(read, rng)?;
        }
        out[at].read = Some(read.with(n));
        Some(out)
    }

    /// Every number a patch could change: a verb's (inside its row's range) or a read's (inside
    /// its grammar read's), where the range has room for another.
    fn numbers(&self, blocks: &[Block]) -> Vec<NumberSpot<'_>> {
        let mut out = Vec::new();
        for (i, b) in blocks.iter().enumerate() {
            if let Some(read) = &b.read
                && let Some(r) = self.template(read)
                && let (Some((lo, hi)), Some(now)) =
                    (r.n, self.cond(read).as_ref().and_then(cond_number))
                && lo < hi
            {
                out.push(NumberSpot {
                    block: i,
                    range: (lo, hi),
                    now,
                    kind: Spot::Read(r),
                });
            }
            if let Stmt::Act(intent) = self.stmt(&b.line)
                && let (v, Some(now)) = intent.verb()
                && let Some((lo, hi)) = self.row.range(v)
                && lo < hi
            {
                out.push(NumberSpot {
                    block: i,
                    range: (lo, hi),
                    now,
                    kind: Spot::Line(v),
                });
            }
        }
        out
    }

    fn stmt(&self, text: &str) -> Stmt {
        parse_stmt_with(text, Vocab::Ice(&self.row.verbs))
    }

    fn cond(&self, read: &str) -> Option<Cond> {
        match self.stmt(&format!("IF {read}")) {
            Stmt::If(c) => Some(c),
            _ => None,
        }
    }

    fn verb(&self, line: &str) -> Option<Verb> {
        match self.stmt(line) {
            Stmt::Act(i) => Some(i.verb().0),
            _ => None,
        }
    }

    /// The grammar read `read` was written from: the same read but for its number
    /// ([`Data::read_key`]).
    fn template(&self, read: &str) -> Option<&ReadDef> {
        let key = self.data.read_key(read)?;
        self.reads
            .iter()
            .find(|r| self.key(r).as_ref() == Some(&key))
    }

    fn key(&self, r: &ReadDef) -> Option<Cond> {
        self.data.read_key(&r.with(0))
    }

    /// Whether a block already opens with grammar read `read` (at any number, however it names
    /// a program): a new read is one the program doesn't have.
    fn has(&self, blocks: &[Block], read: &ReadDef) -> bool {
        let key = self.key(read);
        blocks
            .iter()
            .filter_map(|b| b.read.as_deref())
            .any(|r| self.data.read_key(r) == key)
    }

    /// The reads that could open a block on this row (any but the read keyed `not`): ones with
    /// a response the row can make.
    fn usable(&self, not: Option<&Cond>) -> Vec<(&ReadDef, u32)> {
        self.reads
            .iter()
            .filter(|r| not.is_none() || self.key(r).as_ref() != not)
            .filter(|r| r.responses.iter().any(|(v, _)| self.can(*v)))
            .map(|r| {
                let extra: u32 = self
                    .bias
                    .iter()
                    .filter(|(b, _)| *b == r.read)
                    .map(|(_, w)| *w)
                    .sum();
                (r, r.weight + extra)
            })
            .collect()
    }

    fn draw_read(&self, not: Option<&Cond>, rng: &mut Rng) -> Option<&ReadDef> {
        let usable = self.usable(not);
        let i = rng.weighted(usable.iter().map(|(_, w)| *w))?;
        Some(usable[i].0)
    }

    /// A line `read` may guard: one of its responses the row can make, with a number from the
    /// row's range.
    fn draw_line(&self, read: &ReadDef, rng: &mut Rng) -> Option<String> {
        let ok: Vec<(Verb, u32)> = read
            .responses
            .iter()
            .copied()
            .filter(|(v, _)| self.can(*v))
            .collect();
        let v = ok[rng.weighted(ok.iter().map(|(_, w)| *w))?].0;
        Some(match self.row.range(v) {
            Some((lo, hi)) => format!("{} {}", v.word(), rng.range(lo, hi)),
            None => v.word().to_string(),
        })
    }

    /// Whether the row can write a line of verb `v`: it's on its verbs, with a range if it
    /// takes a number.
    fn can(&self, v: Verb) -> bool {
        self.row.verbs.contains(&v) && (!v.takes_number() || self.row.range(v).is_some())
    }
}

/// A number a patch could change.
struct NumberSpot<'a> {
    block: usize,
    range: (u32, u32),
    now: u32,
    kind: Spot<'a>,
}

enum Spot<'a> {
    /// The read's number, written back through its grammar read.
    Read(&'a ReadDef),
    /// The guarded line's verb's number.
    Line(Verb),
}

/// The number a condition compares with, if it has one.
fn cond_number(c: &Cond) -> Option<u32> {
    match c {
        Cond::Cmp(_, _, n) | Cond::Target(_, _, n) | Cond::SelfStr(_, n) => Some(*n),
        _ => None,
    }
}
