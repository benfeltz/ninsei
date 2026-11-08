//! The deck's BASIC: the language a fight program is written in. Line-numbered, four keywords
//! (`RUN`, `IF`, `GOTO`, `STOP`) plus `REM` for comments, and conditions that read what the
//! fight screen already shows. This module only parses and prints; `Game` runs it (see
//! `Game::pass`), so nothing here knows about a fight.
//!
//! A line that doesn't parse is kept, marked bad with the interpreter's reason, and skipped
//! when the program runs, so a typo never halts a fight.

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
    /// Didn't parse; the interpreter's reason. Skipped when the program runs.
    Bad(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cond {
    Cmp(Var, Op, u32),
    IceClass(IceClass),
    Next(Move),
    Have(String),
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

/// Parses a whole program. A line with no number is kept as stray text, outside the program;
/// a number given twice keeps the last.
pub fn parse(text: &str) -> Program {
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
                    p.set_line(no, rest);
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

/// Parses one statement (the text after a line number).
pub fn parse_stmt(text: &str) -> Stmt {
    let t = tokens(text);
    let Some(head) = t.first() else {
        return bad("empty line");
    };
    let rest = &t[1..];
    match head.to_ascii_uppercase().as_str() {
        "REM" => Stmt::Rem,
        "STOP" if rest.is_empty() => Stmt::Stop,
        "STOP" => bad("STOP takes nothing"),
        "RUN" => match rest {
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
        "IF" => match parse_cond(rest) {
            Ok(c) => Stmt::If(c),
            Err(why) => bad(why),
        },
        _ => bad("unknown keyword"),
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
    let op = match op {
        "<" => Op::Lt,
        ">" => Op::Gt,
        "=" => Op::Eq,
        _ => return Err("compare with <, > or ="),
    };
    let n = n.parse::<u32>().map_err(|_| "compare with a number")?;
    Ok(Cond::Cmp(var, op, n))
}
