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
    /// A program action, run by [`debug`], which says what each pass did.
    Debug(Action),
    /// Lines the terminal prints as they are: the program's listing.
    Print(Vec<String>),
}

pub const HELP: &[&str] = &[
    "run <slot>      run a program               (r)",
    "end             end your turn               (e)",
    "jack <node>     move to a linked node       (j)",
    "prog            list the deck's program      (list)",
    "prog <n> <line> type line n in; prog <n> deletes it",
    "step            run one pass of the program  (s)",
    "play            run passes until a STOP      (cont)",
    "pause           take the turn back",
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
    "run", "end", "jack", "prog", "list", "step", "play", "pause", "hex", "opt", "newrun", "theme",
    "crt", "sound", "clear", "help",
];

/// Verbs whose argument is an HDD slot.
pub const SLOT_VERBS: &[&str] = &["run", "hex", "opt"];

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
        Some(v) if matches!(v.to_ascii_lowercase().as_str(), "prog" | "list") => prog(g, line),
        _ => parse_command(line),
    }
}

/// `prog` lists the program; `prog <n> <statement>` types line `n` in, and `prog <n>` or
/// `prog del <n>` deletes it, as typing a bare line number did.
fn prog(g: &crate::game::Game, line: &str) -> Result<Command, String> {
    let rest = line.trim_start();
    let rest = rest[rest.find(char::is_whitespace).unwrap_or(rest.len())..].trim();
    if rest.is_empty() {
        return Ok(Command::Print(listing(g)));
    }
    let (first, text) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
    let (no, text) = if first.eq_ignore_ascii_case("del") {
        (text.trim(), "")
    } else {
        (first, text)
    };
    let no: u32 = no
        .parse()
        .ok()
        .filter(|&n| n <= crate::basic::MAX_LINE)
        .ok_or_else(|| format!("`{no}` is not a line number."))?;
    let mut p = g.program.clone();
    p.set_line(no, text);
    Ok(Command::Game(Action::Program(p.text())))
}

/// The program as the terminal lists it: the budgets left this turn, then each line, `>` on
/// the one about to run, and under a line that won't run, why.
pub fn listing(g: &crate::game::Game) -> Vec<String> {
    let v = g.program_view();
    let state = match v.state {
        crate::game::ExecState::Paused => "paused".to_string(),
        crate::game::ExecState::Running => "running".to_string(),
        crate::game::ExecState::Stopped(n) => format!("stopped in {n}"),
    };
    let mut out = Vec::new();
    if v.fighting {
        out.push(format!(
            "turn {}  cpu {}/{}  ram {}/{} GB  {state}",
            v.turn, v.cpu_left, g.deck.cpu_max, v.ifs_left, g.deck.ram_max
        ));
    }
    for l in &v.lines {
        let mark = if l.current { ">" } else { " " };
        out.push(format!("{mark}{:>5} {}", l.no, l.text));
        if let Some(why) = &l.fault {
            out.push(format!("       {why}"));
        }
    }
    if v.lines.is_empty() {
        out.push("no program.".to_string());
    }
    out
}

/// What the last pass did, a sentence a line.
pub fn trace_lines(g: &crate::game::Game) -> Vec<String> {
    g.exec.trace.iter().map(|s| g.step_text(s)).collect()
}

/// Passes `play` runs from the terminal before it hands back, however the fight goes.
pub const PLAY_PASSES: usize = 200;

/// Applies a program action from the terminal and returns what to print: each pass, a
/// sentence a line. `play` keeps stepping while the program runs (until a STOP, a hang or
/// the fight's end), as the screens do a pass at a time.
pub fn debug(g: &mut crate::game::Game, action: Action) -> Vec<String> {
    let play = action == Action::Play;
    g.apply(action);
    let mut out = trace_lines(g);
    let mut passes = 1;
    while play
        && passes < PLAY_PASSES
        && g.exec.state == crate::game::ExecState::Running
        && g.phase == crate::game::Phase::Combat
    {
        g.apply(Action::Step);
        out.extend(trace_lines(g));
        passes += 1;
    }
    out
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
        "step" | "s" => Command::Debug(Action::Step),
        "play" | "cont" => Command::Debug(Action::Play),
        "pause" => Command::Game(Action::Pause),
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
