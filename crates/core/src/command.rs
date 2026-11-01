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
    /// New run on a fresh seed the app picks.
    NewRun,
}

pub const HELP: &[&str] = &[
    "run <slot>      run a loaded program        (r)",
    "end             end your turn               (e)",
    "jack <node>     move to a linked node       (j)",
    "load <slot>     load a program into ram",
    "unload <slot>   free its ram",
    "newrun [seed]   start a new run",
    "theme <name>    amber | green | ice",
    "crt             toggle scanlines",
    "clear           clear the terminal",
];

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
        "help" | "?" | "man" => Command::Help,
        "clear" | "cls" => Command::Clear,
        "theme" => Command::Theme(arg.ok_or("`theme` needs a name.")?.to_ascii_lowercase()),
        "crt" => Command::Crt,
        other => return Err(format!("command not found: {other}")),
    })
}
