//! The deck's BASIC: the language a fight program is written in. Line-numbered, four keywords
//! (`RUN`, `IF`, `GOTO`, `STOP`) plus `REM` for comments, and conditions that read what the
//! fight screen already shows. This module only parses and prints; `Game` runs it (see
//! `Game::line`), so nothing here knows about a fight.
//!
//! ICE are written in the same BASIC with their own vocabulary ([`Vocab`]): the verbs on their
//! row in `ice.ron` (`ATTACK n`, `TRACE n`, `FORTIFY n`) where the deck has `RUN`, and
//! conditions that read the fight from their side (`TARGET INTEG`, `TARGET SHIELD`,
//! `TARGET CPU`, `TARGET RAN <program>`, `SELF STR`, `TURN`). A word outside a program's
//! vocabulary is a bad line: the deck can't `ATTACK` and an ICE can't `RUN`.
//!
//! A line that doesn't parse is kept, marked bad with the interpreter's reason, and skipped
//! when the program runs, so a typo never halts a fight.

use serde::Deserialize;

use crate::data::{IceClass, Intent};

/// The highest line number, as on the machines this BASIC imitates.
pub const MAX_LINE: u32 = 63999;

/// The keywords, in the order a palette lists them.
pub const KEYWORDS: [&str; 5] = ["RUN", "IF", "GOTO", "STOP", "REM"];

/// The words a condition can start with, in the order a palette lists them.
pub const CONDITIONS: [&str; 7] = ["INTEG", "TRACE", "CPU", "TURN", "ICE", "NEXT", "HAVE"];

/// The ICE classes `IF ICE <class>` reads.
pub const CLASSES: [&str; 3] = ["BARRIER", "CODEGATE", "SENTRY"];

/// The moves `IF NEXT <intent>` reads.
pub const INTENTS: [&str; 3] = ["ATTACK", "TRACE", "FORTIFY"];

/// A verb an ICE's program may use, as its row in `ice.ron` lists it.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verb {
    /// `ATTACK n`: hits the deck for n, less its shield.
    Attack,
    /// `TRACE n`: raises the trace by n.
    Trace,
    /// `FORTIFY n`: adds n to the ICE's strength.
    Fortify,
}

impl Verb {
    /// The word a program writes it as.
    pub fn word(self) -> &'static str {
        match self {
            Verb::Attack => "ATTACK",
            Verb::Trace => "TRACE",
            Verb::Fortify => "FORTIFY",
        }
    }
}

/// The words a program may use: the deck's (`RUN` and the player's conditions) or an ICE's
/// (its verbs and the conditions that read the deck). `IF`, `GOTO`, `STOP` and `REM` are both's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vocab<'a> {
    Deck,
    Ice(&'a [Verb]),
}

/// Whether `typed`, a program name in a `RUN` or `HAVE`, means the program called `name`
/// whose factory id is `id`: its name, its file name (`name.exe`) or its id, in any case.
/// Fights and `Data::validate` both ask this, so they agree on what a name means.
pub fn names(typed: &str, name: &str, id: &str) -> bool {
    typed.eq_ignore_ascii_case(name)
        || typed.eq_ignore_ascii_case(id)
        || typed
            .to_ascii_lowercase()
            .strip_suffix(".exe")
            .is_some_and(|stem| stem.eq_ignore_ascii_case(name))
}

/// A whole program: its lines, sorted by number, no number twice.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Program {
    pub lines: Vec<Line>,
    /// Text with no line number (or one past `MAX_LINE`): kept as typed, never run, and
    /// printed back after the program, so it can't take another line's number.
    pub stray: Vec<String>,
}

/// Why a stray line isn't part of the program: a number past `MAX_LINE`, or none at all.
pub fn stray_fault(text: &str) -> &'static str {
    if text.starts_with(|c: char| c.is_ascii_digit()) {
        "?ILLEGAL LINE NUMBER"
    } else {
        "?SYNTAX ERROR: no line number"
    }
}

/// One numbered line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub no: u32,
    pub stmt: Stmt,
    /// The statement as typed, after the line number.
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stmt {
    /// Run a program on the HDD, by name as typed.
    Run(String),
    /// Guards the next line: false skips it.
    If(Cond),
    Goto(u32),
    Stop,
    /// A comment: does nothing.
    Rem,
    /// An ICE's verb: hit the deck for n, less its shield.
    Attack(u32),
    /// An ICE's verb: raise the trace by n.
    Trace(u32),
    /// An ICE's verb: add n to its strength.
    Fortify(u32),
    /// Didn't parse; the interpreter's reason. Skipped when the program runs.
    Bad(String),
}

impl Stmt {
    /// The ICE's action this statement takes, if it's one of the ICE's verbs.
    pub fn action(&self) -> Option<Intent> {
        match *self {
            Stmt::Attack(n) => Some(Intent::Attack(n)),
            Stmt::Trace(n) => Some(Intent::Trace(n)),
            Stmt::Fortify(n) => Some(Intent::Fortify(n)),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cond {
    Cmp(Var, Op, u32),
    IceClass(IceClass),
    Next(Move),
    Have(String),
    /// An ICE's: a number on the deck it's fighting.
    Target(Target, Op, u32),
    /// An ICE's: the deck ran this program this turn (by name, as `RUN` names it).
    TargetRan(String),
    /// An ICE's: its own strength.
    SelfStr(Op, u32),
}

/// A number on the deck an ICE's condition reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Integ,
    /// The shield up this turn.
    Shield,
    /// The deck's cycles left this turn.
    Cpu,
}

/// A number a condition compares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Var {
    Integ,
    Trace,
    /// Cycles left this turn.
    Cpu,
    Turn,
    /// The ICE's strength.
    Ice,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Lt,
    Gt,
    Eq,
}

impl Op {
    pub fn test(self, a: u32, b: u32) -> bool {
        match self {
            Op::Lt => a < b,
            Op::Gt => a > b,
            Op::Eq => a == b,
        }
    }
}

/// The ICE's telegraphed move, as `IF NEXT` names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    Attack,
    Trace,
    Fortify,
}

impl Move {
    pub fn is(self, intent: Intent) -> bool {
        matches!(
            (self, intent),
            (Move::Attack, Intent::Attack(_))
                | (Move::Trace, Intent::Trace(_))
                | (Move::Fortify, Intent::Fortify(_))
        )
    }
}

impl Program {
    /// The line numbered `no`.
    pub fn line(&self, no: u32) -> Option<&Line> {
        self.lines
            .binary_search_by_key(&no, |l| l.no)
            .ok()
            .map(|i| &self.lines[i])
    }

    /// The number of the line after `no`, if any.
    pub fn after(&self, no: u32) -> Option<u32> {
        self.lines.iter().map(|l| l.no).find(|&n| n > no)
    }

    pub fn first(&self) -> Option<u32> {
        self.lines.first().map(|l| l.no)
    }

    /// A free line number between the line `above` and the line `below` (either may be the
    /// program's edge): halfway between two lines, half of the first line's number before it,
    /// ten on from the last after it, ten in an empty program. `None` when no number is free
    /// there. Every deck numbers a new line through this, so none overwrites a neighbour.
    pub fn free_line_between(above: Option<u32>, below: Option<u32>) -> Option<u32> {
        match (above, below) {
            (Some(a), Some(b)) => (b > a && b - a >= 2).then(|| a + (b - a) / 2),
            (None, Some(b)) => (b >= 1).then_some(b / 2),
            (Some(a), None) => (a < MAX_LINE).then(|| (a + 10).min(MAX_LINE)),
            (None, None) => Some(10),
        }
    }

    /// The number a new line opened after line `no` takes: between it and the line after it,
    /// or ten on at the end. `None` when there's no number free.
    pub fn next_line_after(&self, no: u32) -> Option<u32> {
        Program::free_line_between(Some(no), self.after(no))
    }

    /// The program as text, one numbered line each, then the stray lines as typed.
    pub fn text(&self) -> String {
        self.lines
            .iter()
            .map(|l| format!("{} {}", l.no, l.text))
            .chain(self.stray.iter().cloned())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Types a line in: `text` is the statement, without its number. Empty text removes the
    /// line, as typing a bare number did.
    pub fn set_line(&mut self, no: u32, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            self.remove(no);
            return;
        }
        self.put(Line {
            no,
            stmt: parse_stmt(text),
            text: text.to_string(),
        });
    }

    fn put(&mut self, line: Line) {
        match self.lines.binary_search_by_key(&line.no, |l| l.no) {
            Ok(i) => self.lines[i] = line,
            Err(i) => self.lines.insert(i, line),
        }
    }

    pub fn remove(&mut self, no: u32) {
        self.lines.retain(|l| l.no != no);
    }

    /// What's wrong with line `no`, in the interpreter's words: a line that didn't parse, or a
    /// GOTO to a line that isn't there.
    pub fn fault(&self, no: u32) -> Option<String> {
        match &self.line(no)?.stmt {
            Stmt::Bad(why) => Some(why.clone()),
            Stmt::Goto(to) if self.line(*to).is_none() => Some(format!("?UNDEF'D STATEMENT {to}")),
            _ => None,
        }
    }
}

/// Parses a whole deck's program. A line with no number is kept as stray text, outside the
/// program; a number given twice keeps the last.
pub fn parse(text: &str) -> Program {
    parse_with(text, Vocab::Deck)
}

/// Parses a whole program in vocabulary `vocab`, as [`parse`] does the deck's.
pub fn parse_with(text: &str, vocab: Vocab) -> Program {
    let mut p = Program::default();
    for raw in text.lines() {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        let digits = raw.bytes().take_while(u8::is_ascii_digit).count();
        let no = raw[..digits].parse::<u32>().ok().filter(|&n| n <= MAX_LINE);
        match no {
            Some(no) => {
                let rest = raw[digits..].trim();
                if rest.is_empty() {
                    p.remove(no);
                } else {
                    p.put(Line {
                        no,
                        stmt: parse_stmt_with(rest, vocab),
                        text: rest.to_string(),
                    });
                }
            }
            None => p.stray.push(raw.to_string()),
        }
    }
    p
}

/// Splits a statement into words, with `<`, `>` and `=` as words of their own.
fn tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut word = String::new();
    for ch in text.chars() {
        if ch.is_whitespace() || matches!(ch, '<' | '>' | '=') {
            if !word.is_empty() {
                out.push(std::mem::take(&mut word));
            }
            if !ch.is_whitespace() {
                out.push(ch.to_string());
            }
        } else {
            word.push(ch);
        }
    }
    if !word.is_empty() {
        out.push(word);
    }
    out
}

fn bad(why: &str) -> Stmt {
    Stmt::Bad(format!("?SYNTAX ERROR: {why}"))
}

/// Parses one statement of the deck's program (the text after a line number).
pub fn parse_stmt(text: &str) -> Stmt {
    parse_stmt_with(text, Vocab::Deck)
}

/// Parses one statement in vocabulary `vocab`. A word the vocabulary doesn't have is an
/// unknown keyword, as any other word would be: the deck's BASIC has never heard of `ATTACK`.
pub fn parse_stmt_with(text: &str, vocab: Vocab) -> Stmt {
    let t = tokens(text);
    let Some(head) = t.first() else {
        return bad("empty line");
    };
    let rest = &t[1..];
    match head.to_ascii_uppercase().as_str() {
        "REM" => Stmt::Rem,
        "STOP" if rest.is_empty() => Stmt::Stop,
        "STOP" => bad("STOP takes nothing"),
        "RUN" if vocab == Vocab::Deck => match rest {
            [name] => Stmt::Run(name.clone()),
            [] => bad("RUN needs a program"),
            _ => bad("RUN takes one program"),
        },
        "GOTO" => match rest {
            [n] => match n.parse::<u32>() {
                Ok(n) if n <= MAX_LINE => Stmt::Goto(n),
                _ => bad("GOTO needs a line number"),
            },
            _ => bad("GOTO needs a line number"),
        },
        "IF" => match vocab {
            Vocab::Deck => parse_cond(rest),
            Vocab::Ice(_) => parse_ice_cond(rest),
        }
        .map_or_else(bad, Stmt::If),
        word => match vocab {
            Vocab::Ice(verbs) => match verbs.iter().find(|v| v.word() == word) {
                Some(v) => match rest {
                    [n] => match n.parse::<u32>() {
                        Ok(n) => match v {
                            Verb::Attack => Stmt::Attack(n),
                            Verb::Trace => Stmt::Trace(n),
                            Verb::Fortify => Stmt::Fortify(n),
                        },
                        Err(_) => bad("needs a number"),
                    },
                    _ => bad("needs a number"),
                },
                None => bad("unknown keyword"),
            },
            Vocab::Deck => bad("unknown keyword"),
        },
    }
}

/// An ICE's condition: what it reads on the deck, its own strength, or the turn.
fn parse_ice_cond(t: &[String]) -> Result<Cond, &'static str> {
    let up: Vec<String> = t.iter().map(|w| w.to_ascii_uppercase()).collect();
    let words: Vec<&str> = up.iter().map(String::as_str).collect();
    match words.as_slice() {
        [] => Err("IF needs a condition"),
        ["TARGET", "RAN", _] => Ok(Cond::TargetRan(t[2].clone())),
        ["TARGET", "RAN"] => Err("RAN needs a program"),
        ["TARGET", v, op, n] => {
            let v = match *v {
                "INTEG" => Target::Integ,
                "SHIELD" => Target::Shield,
                "CPU" => Target::Cpu,
                _ => return Err("unknown condition"),
            };
            let (op, n) = compare(op, n)?;
            Ok(Cond::Target(v, op, n))
        }
        ["SELF", "STR", op, n] => {
            let (op, n) = compare(op, n)?;
            Ok(Cond::SelfStr(op, n))
        }
        ["TURN", op, n] => cmp(Var::Turn, op, n),
        _ => Err("unknown condition"),
    }
}

fn parse_cond(t: &[String]) -> Result<Cond, &'static str> {
    let up: Vec<String> = t.iter().map(|w| w.to_ascii_uppercase()).collect();
    let words: Vec<&str> = up.iter().map(String::as_str).collect();
    match words.as_slice() {
        [] => Err("IF needs a condition"),
        ["HAVE", _] => Ok(Cond::Have(t[1].clone())),
        ["HAVE"] => Err("HAVE needs a program"),
        ["NEXT", m] => match *m {
            "ATTACK" => Ok(Cond::Next(Move::Attack)),
            "TRACE" => Ok(Cond::Next(Move::Trace)),
            "FORTIFY" => Ok(Cond::Next(Move::Fortify)),
            _ => Err("NEXT reads ATTACK, TRACE or FORTIFY"),
        },
        ["ICE", "BARRIER"] => Ok(Cond::IceClass(IceClass::Barrier)),
        ["ICE", "CODEGATE"] | ["ICE", "CODE", "GATE"] => Ok(Cond::IceClass(IceClass::CodeGate)),
        ["ICE", "SENTRY"] => Ok(Cond::IceClass(IceClass::Sentry)),
        ["ICE", "STRENGTH", op, n] => cmp(Var::Ice, op, n),
        [v, op, n] => {
            let var = match *v {
                "INTEG" => Var::Integ,
                "TRACE" => Var::Trace,
                "CPU" => Var::Cpu,
                "TURN" => Var::Turn,
                "ICE" => Var::Ice,
                _ => return Err("unknown condition"),
            };
            cmp(var, op, n)
        }
        _ => Err("unknown condition"),
    }
}

fn cmp(var: Var, op: &str, n: &str) -> Result<Cond, &'static str> {
    let (op, n) = compare(op, n)?;
    Ok(Cond::Cmp(var, op, n))
}

/// A comparison's operator and number.
fn compare(op: &str, n: &str) -> Result<(Op, u32), &'static str> {
    let op = match op {
        "<" => Op::Lt,
        ">" => Op::Gt,
        "=" => Op::Eq,
        _ => return Err("compare with <, > or ="),
    };
    let n = n.parse::<u32>().map_err(|_| "compare with a number")?;
    Ok((op, n))
}
