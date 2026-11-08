//! The Sense/Net's fight screen, decided here and drawn by the app's Stack chrome: the Script
//! card, the Script pane's face, on screen for the whole run. The deck's program is a stack of
//! operator blocks (RUN, IF, GOTO, STOP), each with fields for its parameters; a gutter mark on
//! the line about to run; Step, Run and Pause, which only press with ICE in front.
//! Every edit is an `Action::Program` with one line rewritten, so the card never holds a
//! program of its own. Reference: HyperCard 1.2's button script editor, 1-bit.

use crate::basic::{self, Stmt};
use crate::game::{Action, ExecState, Game, Phase};

/// What a block is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlockKind {
    Run,
    If,
    Goto,
    Stop,
    /// A comment: HyperCard's `--` line.
    Rem,
    /// A line that doesn't read: shown as typed, with the interpreter's reason.
    Bad,
}

impl BlockKind {
    /// The block's name on its face and in the palette.
    pub fn label(self) -> &'static str {
        match self {
            BlockKind::Run => "RUN",
            BlockKind::If => "IF",
            BlockKind::Goto => "GOTO",
            BlockKind::Stop => "STOP",
            BlockKind::Rem => "--",
            BlockKind::Bad => "?",
        }
    }
}

/// The palette row, in order.
pub const PALETTE: [BlockKind; 4] = [
    BlockKind::Run,
    BlockKind::If,
    BlockKind::Goto,
    BlockKind::Stop,
];

/// What an IF can ask, as its first field lists it.
pub const SUBJECTS: [&str; 8] = [
    "INTEG",
    "TRACE",
    "CPU",
    "TURN",
    "ICE",
    "ICE CLASS",
    "NEXT",
    "HAVE",
];

const OPS: [&str; 3] = ["<", ">", "="];

/// One field on a block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Field {
    /// A pop-up menu: its value and what it offers.
    Menu { value: String, options: Vec<String> },
    /// A number typed in.
    Number(u32),
}

impl Field {
    pub fn value(&self) -> String {
        match self {
            Field::Menu { value, .. } => value.clone(),
            Field::Number(n) => n.to_string(),
        }
    }
}

/// One line of the program, as a block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub no: u32,
    pub kind: BlockKind,
    /// The word before the fields (`program`, `line`), if the block has one.
    pub caption: Option<&'static str>,
    pub fields: Vec<Field>,
    /// A comment's words, or a bad line as typed.
    pub text: String,
    /// The line about to run: the ▶ in the gutter.
    pub current: bool,
    /// Why it won't run as written, in the interpreter's words.
    pub fault: Option<String>,
}

/// Which of Step, Run and Pause can be pressed. Run is the default (Enter) while it can be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Buttons {
    pub step: bool,
    pub run: bool,
    pub pause: bool,
}

/// The deck's limits this turn, and how the Sense/Net words them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budgets {
    /// Cycles left, of the deck's CPU.
    pub cpu: (u32, u32),
    /// GB left for IFs, of the deck's RAM.
    pub ifs: (u32, u32),
    /// Programs on the disk, of its slots.
    pub hdd: (u32, u32),
}

impl Budgets {
    /// "3 cycles/turn": a program takes one or two.
    pub fn cpu_words(&self) -> String {
        format!("{} cycles/turn", self.cpu.1)
    }

    /// "6 IFs".
    pub fn ifs_words(&self) -> String {
        format!("{} IFs", self.ifs.1)
    }

    /// "3 of 6".
    pub fn hdd_words(&self) -> String {
        format!("{} of {}", self.hdd.0, self.hdd.1)
    }

    /// The Script card's title line: what's left this turn.
    pub fn left_words(&self) -> String {
        let cycles = self.cpu.0;
        format!(
            "{} of {} IFs, {} cycle{} left",
            self.ifs.0,
            self.ifs.1,
            cycles,
            if cycles == 1 { "" } else { "s" }
        )
    }
}

/// The Script card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Card {
    pub blocks: Vec<Block>,
    /// Text typed with no line number: shown under the blocks, never run.
    pub stray: Vec<(String, &'static str)>,
    pub buttons: Buttons,
    pub budgets: Budgets,
    pub state: ExecState,
    /// ICE is in front: the program plays and the title line counts down this turn.
    pub fighting: bool,
}

impl Card {
    /// The title line's right side: what's left this turn in a fight, the deck's limits a
    /// turn outside one.
    pub fn budget_words(&self) -> String {
        if self.fighting {
            self.budgets.left_words()
        } else {
            format!("{}, {}", self.budgets.ifs_words(), self.budgets.cpu_words())
        }
    }
}

/// The programs a RUN or HAVE menu offers: what's on the disk, by name, once each.
pub fn disk(g: &Game) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for s in &g.hdd {
        if !names.contains(&s.spec.name) {
            names.push(s.spec.name.clone());
        }
    }
    names
}

/// A program name as the block shows it: the disk's own name when it's there, else as typed.
fn shown(g: &Game, typed: &str) -> String {
    g.slot_named(typed)
        .map_or(typed.to_string(), |i| g.hdd[i].spec.name.clone())
}

fn menu(value: impl Into<String>, options: &[&str]) -> Field {
    Field::Menu {
        value: value.into(),
        options: options.iter().map(|s| s.to_string()).collect(),
    }
}

fn programs(g: &Game, value: String) -> Field {
    Field::Menu {
        value,
        options: disk(g),
    }
}

/// The fields of an IF, from its condition's text after `IF`.
fn if_fields(g: &Game, cond: &basic::Cond) -> Vec<Field> {
    use basic::{Cond, Move, Op, Var};
    let op = |o: &Op| match o {
        Op::Lt => "<",
        Op::Gt => ">",
        Op::Eq => "=",
    };
    match cond {
        Cond::Cmp(v, o, n) => {
            let var = match v {
                Var::Integ => "INTEG",
                Var::Trace => "TRACE",
                Var::Cpu => "CPU",
                Var::Turn => "TURN",
                Var::Ice => "ICE",
            };
            vec![menu(var, &SUBJECTS), menu(op(o), &OPS), Field::Number(*n)]
        }
        Cond::IceClass(c) => {
            let class = match c {
                crate::data::IceClass::Barrier => "BARRIER",
                crate::data::IceClass::CodeGate => "CODEGATE",
                crate::data::IceClass::Sentry => "SENTRY",
            };
            vec![menu("ICE CLASS", &SUBJECTS), menu(class, &basic::CLASSES)]
        }
        Cond::Next(m) => {
            let m = match m {
                Move::Attack => "ATTACK",
                Move::Trace => "TRACE",
                Move::Fortify => "FORTIFY",
            };
            vec![menu("NEXT", &SUBJECTS), menu(m, &basic::INTENTS)]
        }
        Cond::Have(name) => vec![menu("HAVE", &SUBJECTS), programs(g, shown(g, name))],
    }
}

/// A line as a block.
fn block(g: &Game, l: &basic::Line, pc: Option<u32>, fighting: bool) -> Block {
    let (kind, caption, fields, text) = match &l.stmt {
        Stmt::Run(name) => (
            BlockKind::Run,
            Some("program"),
            vec![programs(g, shown(g, name))],
            String::new(),
        ),
        Stmt::If(c) => (BlockKind::If, None, if_fields(g, c), String::new()),
        Stmt::Goto(n) => (
            BlockKind::Goto,
            Some("line"),
            vec![Field::Number(*n)],
            String::new(),
        ),
        Stmt::Stop => (BlockKind::Stop, None, Vec::new(), String::new()),
        Stmt::Rem => {
            let words = l.text.get(3..).unwrap_or("").trim().to_string();
            (BlockKind::Rem, None, Vec::new(), words)
        }
        Stmt::Bad(_) => (BlockKind::Bad, None, Vec::new(), l.text.clone()),
    };
    Block {
        no: l.no,
        kind,
        caption,
        fields,
        text,
        current: fighting && pc == Some(l.no),
        fault: g.program.fault(l.no),
    }
}

/// The card, as the game stands.
pub fn card(g: &Game) -> Card {
    let fighting = g.phase == Phase::Combat && g.combat.is_some();
    let blocks = g
        .program
        .lines
        .iter()
        .map(|l| block(g, l, g.exec.pc, fighting))
        .collect();
    let stray = g
        .program
        .stray
        .iter()
        .map(|s| (s.clone(), basic::stray_fault(s)))
        .collect();
    Card {
        blocks,
        stray,
        buttons: buttons(g),
        budgets: budgets(g),
        state: g.exec.state,
        fighting,
    }
}

/// Which of Step, Run and Pause can be pressed, without building the card.
pub fn buttons(g: &Game) -> Buttons {
    let fighting = g.phase == Phase::Combat && g.combat.is_some();
    let running = g.exec.state == ExecState::Running;
    Buttons {
        step: fighting && !running,
        run: fighting && !running,
        pause: fighting && running,
    }
}

/// The deck's limits this turn, without building the card.
pub fn budgets(g: &Game) -> Budgets {
    let c = g.combat.as_ref();
    Budgets {
        cpu: (c.map_or(g.deck.cpu_max, |c| c.cpu), g.deck.cpu_max),
        ifs: (g.ifs_left(), g.deck.ram_max),
        hdd: (g.hdd.len() as u32, g.deck.hdd_max),
    }
}

/// A block's line, written back from its kind and fields.
pub fn line_text(kind: BlockKind, fields: &[Field], text: &str) -> String {
    let f = |i: usize| fields.get(i).map(Field::value).unwrap_or_default();
    match kind {
        BlockKind::Run => format!("RUN {}", f(0)),
        BlockKind::Goto => format!("GOTO {}", f(0)),
        BlockKind::Stop => "STOP".to_string(),
        BlockKind::Rem => format!("REM {text}").trim_end().to_string(),
        BlockKind::Bad => text.to_string(),
        BlockKind::If => match f(0).as_str() {
            "ICE CLASS" => format!("IF ICE {}", f(1)),
            s @ ("NEXT" | "HAVE") => format!("IF {s} {}", f(1)),
            s => format!("IF {s} {} {}", f(1), f(2)),
        },
    }
}

/// The fields a fresh IF on `subject` starts with.
fn if_defaults(g: &Game, subject: &str) -> Vec<Field> {
    let tail = match subject {
        "ICE CLASS" => vec![menu("BARRIER", &basic::CLASSES)],
        "NEXT" => vec![menu("ATTACK", &basic::INTENTS)],
        "HAVE" => vec![programs(g, disk(g).first().cloned().unwrap_or_default())],
        _ => vec![menu("<", &OPS), Field::Number(10)],
    };
    std::iter::once(menu(subject, &SUBJECTS))
        .chain(tail)
        .collect()
}

/// A new block's line: the first program on the disk, `INTEG < 10`, the first line, or STOP.
fn fresh(g: &Game, kind: BlockKind) -> String {
    let fields = match kind {
        BlockKind::Run => vec![programs(g, disk(g).first().cloned().unwrap_or_default())],
        BlockKind::If => if_defaults(g, "INTEG"),
        BlockKind::Goto => vec![Field::Number(g.program.first().unwrap_or(10))],
        _ => Vec::new(),
    };
    line_text(kind, &fields, "")
}

fn write(g: &Game, f: impl FnOnce(&mut basic::Program)) -> Action {
    let mut p = g.program.clone();
    f(&mut p);
    Action::Program(p.text())
}

/// Sets field `ix` of the block on line `no` to `value`, rewriting that one line. Changing an
/// IF's subject starts its other fields afresh. `None` if there's nothing to change.
pub fn set_field(g: &Game, no: u32, ix: usize, value: &str) -> Option<Action> {
    let card = card(g);
    let b = card.blocks.iter().find(|b| b.no == no)?;
    let mut fields = b.fields.clone();
    let field = fields.get_mut(ix)?;
    let was = field.value();
    match field {
        Field::Menu { value: v, .. } => *v = value.to_string(),
        Field::Number(n) => *n = value.trim().parse().ok()?,
    }
    if b.kind == BlockKind::If && ix == 0 && was != value {
        fields = if_defaults(g, value);
    }
    let text = line_text(b.kind, &fields, &b.text);
    (Some(text.as_str()) != g.program.line(no).map(|l| l.text.as_str()))
        .then(|| write(g, |p| p.set_line(no, &text)))
}

/// A new block at the end: the next ten after the last line, or the last number there is.
/// `None` when the last line is already the last number: nothing is overwritten.
pub fn add(g: &Game, kind: BlockKind) -> Option<Action> {
    let no = match g.program.lines.last() {
        None => 10,
        Some(l) if l.no >= basic::MAX_LINE => return None,
        Some(l) => ((l.no / 10 + 1) * 10).min(basic::MAX_LINE),
    };
    let text = fresh(g, kind);
    Some(write(g, |p| p.set_line(no, &text)))
}

/// A new block dropped before line `no`: numbered between it and the line above, or at the
/// end when there's no number free between them (`None` when there's none there either).
pub fn insert_before(g: &Game, no: u32, kind: BlockKind) -> Option<Action> {
    let above = g
        .program
        .lines
        .iter()
        .map(|l| l.no)
        .take_while(|&n| n < no)
        .last();
    let free = basic::Program::free_line_between(above, Some(no));
    match free {
        Some(n) => {
            let text = fresh(g, kind);
            Some(write(g, |p| p.set_line(n, &text)))
        }
        None => add(g, kind),
    }
}

/// Takes the block on line `no` out.
pub fn remove(g: &Game, no: u32) -> Action {
    write(g, |p| p.remove(no))
}

/// Swaps the block on line `no` with the one above (`up`) or below. Line numbers stay where
/// they are, so a GOTO still lands on the same number. `None` at either end.
pub fn swap(g: &Game, no: u32, up: bool) -> Option<Action> {
    let lines = &g.program.lines;
    let i = lines.iter().position(|l| l.no == no)?;
    let j = if up { i.checked_sub(1)? } else { i + 1 };
    let other = lines.get(j)?;
    let (a, b) = (lines[i].text.clone(), other.text.clone());
    let other = other.no;
    Some(write(g, |p| {
        p.set_line(no, &b);
        p.set_line(other, &a);
    }))
}

/// Where the block on line `no` lands when [`swap`] moves it: the line above (`up`) or below.
/// A selection follows it there.
pub fn moved_to(g: &Game, no: u32, up: bool) -> Option<u32> {
    let lines = &g.program.lines;
    let i = lines.iter().position(|l| l.no == no)?;
    let j = if up { i.checked_sub(1)? } else { i + 1 };
    lines.get(j).map(|l| l.no)
}
