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

use crate::basic::{Program, Stmt};

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
