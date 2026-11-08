//! The Ono-Sendai's fight screen, decided here and drawn by the app's btop chrome: the
//! `ice.bas` tile, the Script pane's face, on screen for the whole run. The listing as gdb's
//! TUI shows a source window: line numbers and text, the line about to run marked in the
//! gutter, a STOP's dot beside it, a bad line dimmed with its fault; the budgets cut into the
//! bottom edge in btop's words. A line is retyped where it stands, as the C64's screen editor
//! did, and every edit is one `Action::Program` with one line rewritten, so the tile never
//! holds a program of its own. The terminal is the prompt: the trace prints there.

use crate::basic;
use crate::game::{Action, ExecState, Game, Phase};

/// The tile's title, cut into its top edge.
pub const TITLE: &str = "ice.bas";

/// What sits in a row's gutter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gutter {
    /// The line about to run.
    Current,
    /// A STOP: the breakpoint.
    Stop,
    None,
}

impl Gutter {
    /// The mark itself.
    pub fn mark(self) -> &'static str {
        match self {
            Gutter::Current => "▶",
            Gutter::Stop => "●",
            Gutter::None => " ",
        }
    }
}

/// One row of the listing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// The line's number; `None` for text typed with no number (shown, never run).
    pub no: Option<u32>,
    /// The statement as typed.
    pub text: String,
    pub gutter: Gutter,
    /// A line that won't run as written: drawn dim.
    pub dim: bool,
    /// Why, in the interpreter's words, shown after the text.
    pub fault: Option<String>,
}

/// Which of the tile's keys press now: the debugger's (`r` run, `s` step, `c` continue, `p`
/// pause) and the editor's (`e` edit, `o` open, `d` delete, `j`/`k` move).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Keys {
    pub run: bool,
    pub step: bool,
    pub cont: bool,
    pub pause: bool,
    /// The cursor is on a line to retype.
    pub edit: bool,
    /// A number is free after the cursor.
    pub open: bool,
    /// The cursor is on a line to take out.
    pub delete: bool,
    /// There's another line to move to.
    pub moves: bool,
}

impl Keys {
    /// The legend at the tile's foot, btop's way of showing a box's keys: two rows of a key,
    /// its word and whether it presses now.
    pub fn legend(&self) -> [[(&'static str, &'static str, bool); 4]; 2] {
        [
            [
                ("r", "run", self.run),
                ("s", "step", self.step),
                ("c", "cont", self.cont),
                ("p", "pause", self.pause),
            ],
            [
                ("e", "edit", self.edit),
                ("o", "open", self.open),
                ("d", "del", self.delete),
                ("j/k", "move", self.moves),
            ],
        ]
    }
}

/// ICE in front: the program plays.
fn fighting(g: &Game) -> bool {
    g.combat.is_some() && g.phase == Phase::Combat
}

/// The listing, as the game stands: the program's lines, then any stray text. Read straight
/// from the program, once a frame.
pub fn rows(g: &Game) -> Vec<Row> {
    let pc = g.exec.pc.filter(|_| fighting(g));
    let lines = g.program.lines.iter().map(|l| {
        let fault = g.program.fault(l.no);
        Row {
            no: Some(l.no),
            gutter: if pc == Some(l.no) {
                Gutter::Current
            } else if l.stmt == basic::Stmt::Stop {
                Gutter::Stop
            } else {
                Gutter::None
            },
            dim: fault.is_some(),
            fault,
            text: l.text.clone(),
        }
    });
    let stray = g.program.stray.iter().map(|s| Row {
        no: None,
        gutter: Gutter::None,
        dim: true,
        fault: Some(basic::stray_fault(s).to_string()),
        text: s.clone(),
    });
    lines.chain(stray).collect()
}

/// The bottom edge: what's left this turn in a fight, the deck's limits outside one.
pub fn note(g: &Game) -> String {
    let cpu = g.combat.as_ref().map_or(g.deck.cpu_max, |c| c.cpu);
    format!(
        "cpu {cpu}/{}  ram {}/{}",
        g.deck.cpu_max,
        g.ifs_left(),
        g.deck.ram_max
    )
}

/// The tile's title.
pub fn title(_g: &Game) -> &'static str {
    TITLE
}

/// Which keys press now, with the cursor on line `cursor`. The debugger's: nothing outside a
/// fight; paused, run and step; running, pause; after a STOP, continue and step. The editor's
/// whenever there's a line under the cursor (open also when a number is free after it, or on an
/// empty program).
pub fn keys(g: &Game, cursor: Option<u32>) -> Keys {
    let on_line = cursor.is_some_and(|n| g.program.line(n).is_some());
    let editor = Keys {
        edit: on_line,
        delete: on_line,
        open: open_after(g, cursor.filter(|_| on_line)).is_some(),
        moves: g.program.lines.len() > 1,
        ..Keys::default()
    };
    if !fighting(g) {
        return editor;
    }
    match g.exec.state {
        ExecState::Paused => Keys {
            run: true,
            step: true,
            ..editor
        },
        ExecState::Running => Keys {
            pause: true,
            ..editor
        },
        ExecState::Stopped(_) => Keys {
            cont: true,
            step: true,
            ..editor
        },
    }
}

/// Where the cursor starts: the line about to run, else the first line.
pub fn home(g: &Game) -> Option<u32> {
    g.exec
        .pc
        .filter(|&n| fighting(g) && g.program.line(n).is_some())
        .or(g.program.first())
}

/// The line `by` lines from `no` (up when negative), stopping at either end. From a line that
/// has gone, the cursor starts again at [`home`].
pub fn moved(g: &Game, no: Option<u32>, by: i32) -> Option<u32> {
    let lines = &g.program.lines;
    let Some(i) = no.and_then(|n| lines.iter().position(|l| l.no == n)) else {
        return home(g);
    };
    let j = (i as i64 + by as i64).clamp(0, lines.len() as i64 - 1) as usize;
    Some(lines[j].no)
}

/// The text a line is retyped from: what it says now, or nothing for a new one.
pub fn editing_text(g: &Game, no: u32) -> String {
    g.program.line(no).map_or(String::new(), |l| l.text.clone())
}

/// The number a line opened after the cursor takes (on an empty program, the first line).
/// `None` when no number is free there.
pub fn open_after(g: &Game, no: Option<u32>) -> Option<u32> {
    match no {
        Some(n) => g.program.next_line_after(n),
        None => basic::Program::free_line_between(g.program.lines.last().map(|l| l.no), None),
    }
}

/// Line `no` retyped as `text`: the whole program with that one line rewritten (empty text
/// takes it out). `None` when nothing changes.
pub fn retype(g: &Game, no: u32, text: &str) -> Option<Action> {
    let mut p = g.program.clone();
    p.set_line(no, text);
    (p != g.program).then(|| Action::Program(p.text()))
}

/// Line `no` deleted. `None` if there's no such line.
pub fn delete(g: &Game, no: u32) -> Option<Action> {
    g.program.line(no)?;
    let mut p = g.program.clone();
    p.remove(no);
    Some(Action::Program(p.text()))
}
