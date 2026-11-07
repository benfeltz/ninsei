//! Terminal command parsing. Slot and node numbers are what the UI shows.

use crate::game::Action;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Game(Action),
    Help,
    Clear,
    /// Handled by the app, e.g. `theme amber`.
    Theme(String),
    Crt,
    /// Modem sounds on/off.
    Sound,
    /// New run on a fresh seed the app picks.
    NewRun,
    /// Open HDD slot `n` in the hex editor.
    Hex(usize),
}

pub const HELP: &[&str] = &[
    "run <slot>      run a loaded program        (r)",
    "end             end your turn               (e)",
    "jack <node>     move to a linked node       (j)",
    "load <slot>     load a program into ram",
    "unload <slot>   free its ram",
    "hex <slot>      open a program in the hex editor",
    "opt <slot>      spend a fragment: +budget",
    "newrun [seed]   start a new run",
    "theme <name>    amber | green | ice",
    "crt             toggle scanlines",
    "sound           all sound on/off",
    "clear           clear the terminal",
];

/// Every verb the run terminal knows, for Tab completion.
pub const VERBS: &[&str] = &[
    "run", "end", "jack", "load", "unload", "hex", "opt", "newrun", "theme", "crt", "sound",
    "clear", "help",
];

/// Verbs whose argument is an HDD slot.
pub const SLOT_VERBS: &[&str] = &["run", "load", "unload", "hex", "opt"];

/// Commands kept in the terminal's history (`Save::term_history`).
pub const HISTORY_KEEP: usize = 100;

/// The tool a verb needs, if any: `hex` and `opt` are HexEd's.
pub fn needs_tool(verb: &str) -> Option<&'static str> {
    match verb.to_ascii_lowercase().as_str() {
        "hex" | "hexedit" | "xxd" | "opt" | "optimize" => Some("hexed"),
        _ => None,
    }
}

/// Whether a run knows a verb: a tool's verbs only once you own the tool.
pub fn knows(g: &crate::game::Game, verb: &str) -> bool {
    needs_tool(verb).is_none_or(|t| g.has_tool(t))
}

/// The terminal's verbs and help lines as this run knows them.
pub fn verbs(g: &crate::game::Game) -> impl Iterator<Item = &'static str> {
    VERBS.iter().copied().filter(|v| knows(g, v))
}

pub fn help(g: &crate::game::Game) -> impl Iterator<Item = &'static str> {
    HELP.iter()
        .copied()
        .filter(|l| knows(g, l.split_whitespace().next().unwrap_or("")))
}

/// Parses a line the way this run's terminal does: a tool's verbs are unknown until you own it.
pub fn parse_in(g: &crate::game::Game, line: &str) -> Result<Command, String> {
    match line.split_whitespace().next() {
        Some(v) if !knows(g, v) => Err(format!("command not found: {v}")),
        _ => parse_command(line),
    }
}

pub fn parse_command(line: &str) -> Result<Command, String> {
    let mut words = line.split_whitespace();
    let Some(verb) = words.next() else {
        return Err("empty command.".into());
    };
    let arg = words.next();
    let num = |what: &str| -> Result<usize, String> {
        arg.ok_or_else(|| format!("`{verb}` needs a {what} number."))?
            .parse()
            .map_err(|_| format!("`{}` is not a {what} number.", arg.unwrap_or_default()))
    };
    Ok(match verb.to_ascii_lowercase().as_str() {
        "run" | "r" | "exec" => Command::Game(Action::Run(num("slot")?)),
        "end" | "e" | "pass" => Command::Game(Action::EndTurn),
        "jack" | "j" | "cd" | "ssh" => Command::Game(Action::Move(num("node")?)),
        "load" => Command::Game(Action::Load(num("slot")?)),
        "unload" | "kill" => Command::Game(Action::Unload(num("slot")?)),
        "newrun" | "reboot" => match arg {
            Some(s) => Command::Game(Action::NewRun(
                u64::from_str_radix(s.trim_start_matches("0x"), 16)
                    .map_err(|_| format!("`{s}` is not a hex seed."))?,
            )),
            None => Command::NewRun,
        },
        "hex" | "hexedit" | "xxd" => Command::Hex(num("slot")?),
        "opt" | "optimize" => Command::Game(Action::Optimize { slot: num("slot")? }),
        "help" | "?" | "man" => Command::Help,
        "clear" | "cls" => Command::Clear,
        "theme" => Command::Theme(arg.ok_or("`theme` needs a name.")?.to_ascii_lowercase()),
        "crt" => Command::Crt,
        "sound" => Command::Sound,
        other => return Err(format!("command not found: {other}")),
    })
}
