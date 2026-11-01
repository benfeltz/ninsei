//! The hub: your machine between runs. The save document, the screen flow between the
//! Workbench desktop and a run, and the desktop Shell's commands. No UI here.

use serde::{Deserialize, Serialize};

use crate::game::{Game, Phase};
use crate::tuning::{HUB_DRAG_SECS, HUB_PALETTE_CHARS};

/// Bump when the save's shape changes incompatibly.
pub const SAVE_VERSION: u32 = 1;
/// Storage key the app saves under.
pub const SAVE_KEY: &str = "ninsei.save";

/// What double-clicking a desktop icon opens.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tool {
    /// Temporary until the phonebook lands: jack into a fresh random network.
    JackIn,
    Terminal,
    Rig,
    Stash,
    HexEd,
    Shell,
}

impl Tool {
    pub const ALL: [Tool; 6] = [
        Tool::JackIn,
        Tool::Terminal,
        Tool::Rig,
        Tool::Stash,
        Tool::HexEd,
        Tool::Shell,
    ];

    /// Tools that work yet. The rest ask you to insert their disk.
    pub fn installed(self) -> bool {
        matches!(self, Tool::JackIn | Tool::Shell)
    }

    /// Volume name for the "insert volume" requester.
    pub fn volume(self) -> &'static str {
        match self {
            Tool::JackIn => "JackIn",
            Tool::Terminal => "CarrierTerm",
            Tool::Rig => "RIG",
            Tool::Stash => "STASH",
            Tool::HexEd => "HexEd",
            Tool::Shell => "Shell",
        }
    }
}

/// A desktop icon: pixel art in the 1.3 palette (see `data/hub/icons.ron`).
#[derive(Deserialize, Clone, Debug)]
pub struct IconDef {
    pub label: String,
    pub tool: Tool,
    pub pixels: Vec<String>,
}

impl IconDef {
    pub fn width(&self) -> usize {
        self.pixels.first().map_or(0, |r| r.chars().count())
    }

    pub fn validate(icons: &[IconDef]) -> Result<(), String> {
        for t in Tool::ALL {
            if !icons.iter().any(|i| i.tool == t) {
                return Err(format!("no desktop icon for {t:?}"));
            }
        }
        for i in icons {
            let w = i.width();
            if w == 0 || i.pixels.is_empty() {
                return Err(format!("icon `{}` is empty", i.label));
            }
            for (n, row) in i.pixels.iter().enumerate() {
                if row.chars().count() != w {
                    return Err(format!("icon `{}` row {n} is not {w} wide", i.label));
                }
                if let Some(c) = row.chars().find(|c| !HUB_PALETTE_CHARS.contains(*c)) {
                    return Err(format!("icon `{}` uses unknown colour `{c}`", i.label));
                }
            }
        }
        Ok(())
    }
}

/// Everything the hub keeps between sessions. One RON document, versioned.
/// Later features add fields with `#[serde(default)]` so older saves still load.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Save {
    pub version: u32,
    #[serde(default)]
    pub runs: u32,
    #[serde(default)]
    pub exits: u32,
    #[serde(default)]
    pub flatlines: u32,
    /// Most credits carried out of a single run.
    #[serde(default)]
    pub best_haul: u32,
    /// Phosphor theme name for runs (`amber`, `green`, `ice`).
    #[serde(default = "default_phosphor")]
    pub phosphor: String,
    #[serde(default = "default_crt")]
    pub crt: bool,
}

fn default_phosphor() -> String {
    "amber".into()
}

fn default_crt() -> bool {
    true
}

impl Default for Save {
    fn default() -> Self {
        Self {
            version: SAVE_VERSION,
            runs: 0,
            exits: 0,
            flatlines: 0,
            best_haul: 0,
            phosphor: default_phosphor(),
            crt: default_crt(),
        }
    }
}

#[derive(Deserialize)]
struct VersionProbe {
    version: u32,
}

impl Save {
    /// Parses a save. Anything unreadable or from another version is an error;
    /// the app then starts a fresh hub instead of crashing.
    pub fn load(text: &str) -> Result<Save, String> {
        let probe: VersionProbe =
            ron::from_str(text).map_err(|e| format!("unreadable save: {e}"))?;
        if probe.version != SAVE_VERSION {
            return Err(format!(
                "save is version {}, this build reads {SAVE_VERSION}",
                probe.version
            ));
        }
        ron::from_str(text).map_err(|e| format!("corrupt save: {e}"))
    }

    pub fn to_text(&self) -> String {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()).unwrap_or_default()
    }

    /// Books a finished run. Returns the report, or `None` if the run isn't over.
    pub fn record(&mut self, game: &Game) -> Option<RunReport> {
        let end = match game.phase {
            Phase::JackedOut => RunEnd::Exit,
            Phase::Flatlined => RunEnd::Flatline,
            _ => return None,
        };
        self.runs += 1;
        match end {
            RunEnd::Exit => {
                self.exits += 1;
                self.best_haul = self.best_haul.max(game.deck.credits);
            }
            RunEnd::Flatline => self.flatlines += 1,
        }
        Some(RunReport {
            end,
            credits: game.deck.credits,
            hops: game.hops,
            seed: game.seed,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunEnd {
    Exit,
    Flatline,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunReport {
    pub end: RunEnd,
    pub credits: u32,
    pub hops: u32,
    pub seed: u64,
}

/// Which machine is on screen, and the drags between them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Screen {
    /// The Workbench desktop.
    Hub,
    /// The desktop sliding down to reveal the run behind it.
    JackIn {
        t: f32,
    },
    Run,
    /// Flatlined: the crash screen, until you click.
    Crash,
    /// The desktop sliding back up over the run.
    JackOut {
        t: f32,
    },
}

impl Screen {
    pub fn tick(self, dt: f32) -> Screen {
        match self {
            Screen::JackIn { t } if t + dt >= HUB_DRAG_SECS => Screen::Run,
            Screen::JackIn { t } => Screen::JackIn { t: t + dt },
            Screen::JackOut { t } if t + dt >= HUB_DRAG_SECS => Screen::Hub,
            Screen::JackOut { t } => Screen::JackOut { t: t + dt },
            s => s,
        }
    }

    /// How far the desktop is dragged down, 0 (covering everything) to 1 (off the bottom).
    /// Eased like a hand pulling a screen: fast start, soft landing.
    pub fn drag(self) -> f32 {
        let ease = |x: f32| 1.0 - (1.0 - x.clamp(0.0, 1.0)).powi(3);
        match self {
            Screen::Hub => 0.0,
            Screen::JackIn { t } => ease(t / HUB_DRAG_SECS),
            Screen::Run | Screen::Crash => 1.0,
            Screen::JackOut { t } => 1.0 - ease(t / HUB_DRAG_SECS),
        }
    }

    /// Whether the run's UI is visible at all.
    pub fn shows_run(self) -> bool {
        !matches!(self, Screen::Hub)
    }

    /// Whether the desktop takes input.
    pub fn hub_live(self) -> bool {
        self == Screen::Hub
    }
}

/// What a Shell command asks the app to do, besides printing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShellEffect {
    None,
    JackIn(Option<u64>),
    Open(Tool),
    Theme(String),
    Crt,
    Clear,
}

/// Runs one Shell line. Returns the lines to print and what to do.
pub fn shell(line: &str, save: &Save, icons: &[IconDef]) -> (Vec<String>, ShellEffect) {
    let mut words = line.split_whitespace();
    let Some(cmd) = words.next() else {
        return (Vec::new(), ShellEffect::None);
    };
    let arg = words.next();
    let lines = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    match cmd.to_ascii_lowercase().as_str() {
        "help" | "?" => (
            lines(&[
                "dir              list the desktop",
                "run <tool>       open a tool (jackin, shell, ...)",
                "jackin [seed]    jack into a network (hex seed optional)",
                "info             your record",
                "theme <name>     run phosphor: amber | green | ice",
                "crt              toggle scanlines in runs",
                "echo <text>      say something",
                "clear            clear the window",
            ]),
            ShellEffect::None,
        ),
        "dir" | "ls" => {
            let mut out = vec!["Workbench:".to_string()];
            for i in icons {
                let state = if i.tool.installed() {
                    ""
                } else {
                    "  (not installed)"
                };
                out.push(format!(
                    "  {:<10} {}{state}",
                    i.label,
                    if i.tool == Tool::Shell {
                        "tool"
                    } else {
                        "drawer"
                    }
                ));
            }
            (out, ShellEffect::None)
        }
        "info" | "stats" => (
            vec![
                format!("runs       {}", save.runs),
                format!("exits      {}", save.exits),
                format!("flatlines  {}", save.flatlines),
                format!("best haul  {}¢", save.best_haul),
                format!("phosphor   {}", save.phosphor),
            ],
            ShellEffect::None,
        ),
        "jackin" | "jack" | "dial" => match arg {
            None => (vec!["dialing...".into()], ShellEffect::JackIn(None)),
            Some(s) => {
                match u64::from_str_radix(s.trim_start_matches("0x").trim_start_matches('#'), 16) {
                    Ok(seed) => (
                        vec![format!("dialing seed {seed:x}...")],
                        ShellEffect::JackIn(Some(seed)),
                    ),
                    Err(_) => (vec![format!("`{s}` is not a hex seed.")], ShellEffect::None),
                }
            }
        },
        "run" | "open" => {
            let Some(name) = arg else {
                return (vec!["run what? try `dir`.".into()], ShellEffect::None);
            };
            match icons.iter().find(|i| i.label.eq_ignore_ascii_case(name)) {
                Some(i) => (Vec::new(), ShellEffect::Open(i.tool)),
                None => (vec![format!("{name}: object not found")], ShellEffect::None),
            }
        }
        "theme" => match arg {
            Some(t @ ("amber" | "green" | "ice")) => {
                (vec![format!("phosphor: {t}")], ShellEffect::Theme(t.into()))
            }
            _ => (vec!["theme amber | green | ice".into()], ShellEffect::None),
        },
        "crt" => (Vec::new(), ShellEffect::Crt),
        "clear" | "cls" => (Vec::new(), ShellEffect::Clear),
        "echo" => (
            vec![line.trim_start()[cmd.len()..].trim().to_string()],
            ShellEffect::None,
        ),
        other => (vec![format!("Unknown command {other}")], ShellEffect::None),
    }
}
