//! BLOX: falling blocks. Seven pieces dealt from a shuffled bag; turn them, shift them, drop
//! them, and a full row clears. It ends when a new piece has no room.

use super::{Arcade, Cell, Input, unpaused};
use crate::rng::Rng;
use crate::tuning::{
    BLOX_FASTEST_PERIOD, BLOX_HARD_POINTS, BLOX_LINE_POINTS, BLOX_LINES_PER_LEVEL,
    BLOX_PERIOD_STEP, BLOX_SOFT_POINTS, BLOX_START_PERIOD,
};

/// Sideways nudges tried when a turn doesn't fit where it is.
const KICKS: [i32; 5] = [0, -1, 1, -2, 2];

/// The seven pieces in their first turn: each one's box size and its four squares.
const PIECES: [(i32, [(i32, i32); 4]); 7] = [
    (4, [(0, 1), (1, 1), (2, 1), (3, 1)]), // I
    (2, [(0, 0), (1, 0), (0, 1), (1, 1)]), // O
    (3, [(1, 0), (0, 1), (1, 1), (2, 1)]), // T
    (3, [(1, 0), (2, 0), (0, 1), (1, 1)]), // S
    (3, [(0, 0), (1, 0), (1, 1), (2, 1)]), // Z
    (3, [(0, 0), (0, 1), (1, 1), (2, 1)]), // J
    (3, [(2, 0), (0, 1), (1, 1), (2, 1)]), // L
];

/// A piece's squares in its box at turn `rot` (0-3), each turn a quarter clockwise.
fn shape(kind: u8, rot: u8) -> [(i32, i32); 4] {
    let (n, mut sq) = PIECES[usize::from(kind) % PIECES.len()];
    for _ in 0..rot % 4 {
        for s in &mut sq {
            *s = (n - 1 - s.1, s.0);
        }
    }
    sq
}

#[derive(Clone, Debug)]
pub struct Blox {
    cols: usize,
    rows: usize,
    rng: Rng,
    /// Settled squares, row by row: 0 is empty, else the piece kind plus one.
    board: Vec<u8>,
    /// What's left of the shuffled bag, dealt from the end.
    bag: Vec<u8>,
    kind: u8,
    rot: u8,
    x: i32,
    y: i32,
    clock: u32,
    score: u32,
    lines: u32,
    over: bool,
    paused: bool,
}

impl Blox {
    pub fn new(seed: u64, cols: usize, rows: usize) -> Self {
        let (cols, rows) = super::Engine::Blox.field(cols, rows);
        let mut b = Self {
            cols,
            rows,
            rng: Rng::new(seed),
            board: vec![0; cols * rows],
            bag: Vec::new(),
            kind: 0,
            rot: 0,
            x: 0,
            y: 0,
            clock: 0,
            score: 0,
            lines: 0,
            over: false,
            paused: false,
        };
        b.spawn();
        b
    }

    pub fn level(&self) -> u32 {
        self.lines / BLOX_LINES_PER_LEVEL
    }

    pub fn lines(&self) -> u32 {
        self.lines
    }

    /// Ticks between drops at this level.
    pub fn period(&self) -> u32 {
        BLOX_START_PERIOD
            .saturating_sub(self.level() * BLOX_PERIOD_STEP)
            .max(BLOX_FASTEST_PERIOD)
    }

    /// The falling piece's squares, (col, row).
    pub fn piece(&self) -> Vec<(usize, usize)> {
        shape(self.kind, self.rot)
            .iter()
            .map(|&(dx, dy)| ((self.x + dx) as usize, (self.y + dy) as usize))
            .collect()
    }

    /// Lays a settled square at (col, row), as if a piece had left it there. For setting up a
    /// board: a demo, or a test.
    pub fn lay(&mut self, col: usize, row: usize) {
        if col < self.cols && row < self.rows {
            self.board[row * self.cols + col] = 1;
        }
    }

    fn deal(&mut self) -> u8 {
        if self.bag.is_empty() {
            self.bag = (0..PIECES.len() as u8).collect();
            for i in (1..self.bag.len()).rev() {
                let j = self.rng.below(i + 1);
                self.bag.swap(i, j);
            }
        }
        self.bag.pop().unwrap_or(0)
    }

    fn fits(&self, rot: u8, x: i32, y: i32) -> bool {
        shape(self.kind, rot).iter().all(|&(dx, dy)| {
            let (cx, cy) = (x + dx, y + dy);
            cx >= 0
                && cy >= 0
                && (cx as usize) < self.cols
                && (cy as usize) < self.rows
                && self.board[cy as usize * self.cols + cx as usize] == 0
        })
    }

    fn spawn(&mut self) {
        self.kind = self.deal();
        self.rot = 0;
        let n = PIECES[usize::from(self.kind)].0;
        self.x = (self.cols as i32 - n) / 2;
        self.y = 0;
        self.clock = 0;
        if !self.fits(self.rot, self.x, self.y) {
            self.over = true;
        }
    }

    fn shift(&mut self, dx: i32) {
        if self.fits(self.rot, self.x + dx, self.y) {
            self.x += dx;
        }
    }

    fn turn(&mut self, by: u8) {
        let rot = (self.rot + by) % 4;
        if let Some(k) = KICKS.iter().find(|&&k| self.fits(rot, self.x + k, self.y)) {
            self.rot = rot;
            self.x += k;
        }
    }

    /// Down a row; a piece that can't go down settles. True if it moved.
    fn fall(&mut self) -> bool {
        if self.fits(self.rot, self.x, self.y + 1) {
            self.y += 1;
            true
        } else {
            self.settle();
            false
        }
    }

    fn settle(&mut self) {
        for (cx, cy) in self.piece() {
            self.board[cy * self.cols + cx] = self.kind + 1;
        }
        let cols = self.cols;
        let kept: Vec<u8> = self
            .board
            .chunks(cols)
            .filter(|row| row.contains(&0))
            .flatten()
            .copied()
            .collect();
        let cleared = (self.board.len() - kept.len()) / cols;
        let mut board = vec![0; cleared * cols];
        board.extend(kept);
        self.board = board;
        self.score += BLOX_LINE_POINTS[cleared.min(4)] * (self.level() + 1);
        self.lines += cleared as u32;
        self.spawn();
    }
}

impl Arcade for Blox {
    fn step(&mut self, input: Input, ticks: u32) {
        if !unpaused(&mut self.paused, self.over, input) {
            return;
        }
        match input {
            Input::Left => self.shift(-1),
            Input::Right => self.shift(1),
            Input::Up => self.turn(1),
            Input::Alt => self.turn(3),
            Input::Down => {
                if self.fall() {
                    self.score += BLOX_SOFT_POINTS;
                }
            }
            Input::Fire => {
                while self.fits(self.rot, self.x, self.y + 1) {
                    self.y += 1;
                    self.score += BLOX_HARD_POINTS;
                }
                self.settle();
            }
            _ => {}
        }
        for _ in 0..ticks {
            if self.over {
                return;
            }
            self.clock += 1;
            if self.clock >= self.period() {
                self.clock = 0;
                self.fall();
            }
        }
    }

    fn cols(&self) -> usize {
        self.cols
    }

    fn rows(&self) -> usize {
        self.rows
    }

    fn cells(&self) -> Vec<Cell> {
        let mut out: Vec<Cell> = self
            .board
            .iter()
            .map(|&k| if k == 0 { Cell::Empty } else { Cell::Block(k) })
            .collect();
        if !self.over {
            for (cx, cy) in self.piece() {
                out[cy * self.cols + cx] = Cell::Block(self.kind + 1);
            }
        }
        out
    }

    fn over(&self) -> bool {
        self.over
    }

    fn paused(&self) -> bool {
        self.paused
    }

    fn score(&self) -> u32 {
        self.score
    }

    fn status(&self) -> String {
        let state = if self.over {
            "  GAME OVER"
        } else if self.paused {
            "  PAUSED"
        } else {
            ""
        };
        format!(
            "SCORE {}  LINES {}  LEVEL {}{state}",
            self.score,
            self.lines,
            self.level()
        )
    }
}
