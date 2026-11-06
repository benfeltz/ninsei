//! Hayes AT commands for NiGHTCOMM's TERM page: you type to the modem, it answers in result
//! codes. Parsing and what each command does live here, so the app only shows the lines and
//! carries out the [`Effect`].

use crate::data::Data;
use crate::hub::{Answer, Save};
use crate::nightcomm;

/// What a typed line asks the modem to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// Nothing typed.
    Empty,
    /// `ATZ`: reset.
    Reset,
    /// `ATI`: identify.
    Info,
    /// `ATH`: hang up.
    Hang,
    /// `ATM0` / `ATM1`: speaker off or on.
    Speaker(bool),
    /// `+++`: back to command mode.
    Escape,
    /// `ATDT 555-0420` (with `!` on the end: dial a watched line anyway).
    Dial { number: String, force: bool },
    /// `ATDS2`: the second number in the phonebook.
    Stored(usize),
    /// Anything else.
    Unknown,
}

/// What the app does after a command, besides printing its lines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    None,
    /// Ask before jacking in to this phonebook entry (`jack in? [Y/n]`).
    Ask(u32),
    /// Jack in to this entry: the handshake plays, then the run.
    Dial(u32),
    /// Call this board (index into `Data::boards`).
    Call(usize),
    /// Drop the line.
    Hang,
    /// The modem speaker (the game's modem sound) on or off.
    Speaker(bool),
}

/// The modem's answer: lines to print, and what happens next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reply {
    pub lines: Vec<String>,
    pub effect: Effect,
}

impl Reply {
    fn say(lines: &[&str]) -> Reply {
        Reply {
            lines: lines.iter().map(|l| l.to_string()).collect(),
            effect: Effect::None,
        }
    }
}

/// The commands TERM's Tab completes.
pub const COMMANDS: &[&str] = &["ATDT", "ATDS", "ATZ", "ATH", "ATI", "ATM0", "ATM1", "+++"];

/// What the modem says when you ask who it is.
pub const IDENT: &str = "NiGHTCOMM 3.1 / Hayes compatible / 2400 bps";

/// A phone number as the network writes it: `555-0420` from `5550420`, `555 0420` or `0420`.
pub fn normalize(s: &str) -> Option<String> {
    let digits: String = s.chars().filter(char::is_ascii_digit).collect();
    let rest = s
        .chars()
        .filter(|c| !c.is_ascii_digit() && !matches!(c, '-' | ' ' | '(' | ')'))
        .count();
    if rest > 0 {
        return None;
    }
    match digits.len() {
        4 => Some(format!("555-{digits}")),
        7 => Some(format!("{}-{}", &digits[..3], &digits[3..])),
        _ => None,
    }
}

pub fn parse(line: &str) -> Command {
    let t = line.trim();
    if t.is_empty() {
        return Command::Empty;
    }
    if t == "+++" {
        return Command::Escape;
    }
    let up = t.to_uppercase();
    let Some(rest) = up.strip_prefix("AT") else {
        return Command::Unknown;
    };
    let rest = rest.trim();
    match rest {
        "Z" => Command::Reset,
        "I" => Command::Info,
        "H" | "H0" => Command::Hang,
        "M0" => Command::Speaker(false),
        "M1" | "M2" => Command::Speaker(true),
        _ => {
            if let Some(n) = rest.strip_prefix("DS") {
                return n
                    .trim()
                    .parse()
                    .ok()
                    .filter(|k| *k >= 1)
                    .map_or(Command::Unknown, Command::Stored);
            }
            let Some(n) = rest.strip_prefix("DT").or_else(|| rest.strip_prefix('D')) else {
                return Command::Unknown;
            };
            let n = n.trim();
            let (n, force) = match n.strip_suffix('!') {
                Some(n) => (n, true),
                None => (n, false),
            };
            normalize(n).map_or(Command::Unknown, |number| Command::Dial { number, force })
        }
    }
}

/// Runs a typed line. `asking` is the entry a `jack in? [Y/n]` is waiting on, if any: then the
/// line is its answer.
pub fn run(save: &mut Save, data: &Data, line: &str, asking: Option<u32>) -> Reply {
    if let Some(entry) = asking {
        return match line.trim().to_lowercase().as_str() {
            "" | "y" | "yes" => Reply {
                lines: Vec::new(),
                effect: Effect::Dial(entry),
            },
            _ => Reply::say(&["NO CARRIER"]),
        };
    }
    match parse(line) {
        Command::Empty => Reply::say(&[]),
        Command::Reset | Command::Escape => Reply::say(&["OK"]),
        Command::Info => Reply::say(&[IDENT, "OK"]),
        Command::Hang => Reply {
            lines: vec!["OK".into()],
            effect: Effect::Hang,
        },
        Command::Speaker(on) => Reply {
            lines: vec!["OK".into()],
            effect: Effect::Speaker(on),
        },
        Command::Unknown => Reply::say(&["ERROR"]),
        Command::Stored(k) => match save.phonebook.get(k - 1).map(|e| e.number.clone()) {
            Some(number) => dial(save, data, &number, false),
            None => Reply::say(&["ERROR"]),
        },
        Command::Dial { number, force } => dial(save, data, &number, force),
    }
}

fn dial(save: &mut Save, data: &Data, number: &str, force: bool) -> Reply {
    let mut lines = vec![format!("DIALING {number}")];
    if nightcomm::is_watched(save, number) && !force {
        lines.push(format!(
            "!! {number} is a watched line. ATDT {number}! dials it anyway."
        ));
        lines.push("NO CARRIER".into());
        return Reply {
            lines,
            effect: Effect::None,
        };
    }
    // Numbers you know answer as you know them.
    if let Some(b) = data.boards.iter().position(|b| b.phone == number)
        && save.knows_board(&data.boards[b].id)
    {
        lines.push(format!("CONNECT {}", data.boards[b].baud));
        return Reply {
            lines,
            effect: Effect::Call(b),
        };
    }
    if let Some(e) = save.phonebook.iter().find(|e| e.number == number) {
        lines.push(format!("{} answers. jack in? [Y/n]", e.name));
        return Reply {
            lines,
            effect: Effect::Ask(e.id),
        };
    }
    match save.atdt(data, number) {
        Err(e) => {
            lines.push(e);
            lines.push("NO DIALTONE".into());
            Reply {
                lines,
                effect: Effect::None,
            }
        }
        Ok(answer) => {
            let effect = match &answer {
                Answer::Carrier { entry, .. } => {
                    let e = save.entry(*entry);
                    lines.push("CONNECT 2400".into());
                    lines.push(format!(
                        "{} answers. new number, in your phonebook. jack in? [Y/n]",
                        e.map_or("a modem", |e| e.name.as_str())
                    ));
                    Effect::Ask(*entry)
                }
                Answer::Board { board, .. } => {
                    lines.push(format!("CONNECT {}", data.boards[*board].baud));
                    Effect::Call(*board)
                }
                Answer::Trap => {
                    lines.push("NO CARRIER".into());
                    lines.push("!! the line clicked twice. the telco is listening.".into());
                    Effect::None
                }
                Answer::Busy => {
                    lines.push("BUSY".into());
                    Effect::None
                }
                Answer::Voice => {
                    lines.push("VOICE".into());
                    Effect::None
                }
                Answer::NoAnswer => {
                    lines.push("NO ANSWER".into());
                    Effect::None
                }
                Answer::Fax => {
                    lines.push("NO CARRIER".into());
                    Effect::None
                }
            };
            Reply { lines, effect }
        }
    }
}

/// Words that could finish the last word of `before`: AT commands, and after `ATDT` the
/// numbers in your phonebook.
pub fn complete(before: &str, save: &Save) -> Vec<String> {
    let last = before.rsplit(' ').next().unwrap_or("");
    let up = last.to_uppercase();
    let dialing = before.trim_start().to_uppercase().starts_with("ATDT");
    let mut words: Vec<String> = if dialing && before.contains(' ') {
        save.phonebook
            .iter()
            .map(|e| e.number.clone())
            .filter(|n| n.starts_with(last))
            .collect()
    } else {
        COMMANDS
            .iter()
            .filter(|c| c.starts_with(&up))
            .map(|c| c.to_string())
            .collect()
    };
    words.dedup();
    words
}
