//! MINEFIELD: mines under covered squares. Open a square and it says how many mines touch it;
//! an empty one opens its neighbours. The first square you open is never a mine. Open every
//! safe square to win; open a mine and it's over.

use super::{Arcade, Cell, Input, unpaused};
use crate::rng::Rng;

/// Share of the squares that hide a mine, in percent (at least one mine).
pub const MINE_PCT: usize = 15;

#[derive(Clone, Debug)]
pub struct Minefield {
    cols: usize,
    rows: usize,
    rng: Rng,
    mines: usize,
    /// Laid on the first open, so it can't be under your first square.
    mine: Vec<bool>,
    open: Vec<bool>,
    flag: Vec<bool>,
    laid: bool,
    cursor: (usize, usize),
    over: bool,
    won: bool,
    paused: bool,
}

impl Minefield {
    pub fn new(seed: u64, cols: usize, rows: usize) -> Self {
        let (cols, rows) = super::Engine::Minefield.field(cols, rows);
        let n = cols * rows;
        Self {
            cols,
            rows,
            rng: Rng::new(seed),
            mines: (n * MINE_PCT / 100).clamp(1, n - 1),
            mine: vec![false; n],
            open: vec![false; n],
            flag: vec![false; n],
            laid: false,
            cursor: (cols / 2, rows / 2),
            over: false,
            won: false,
            paused: false,
        }
    }

    pub fn mines(&self) -> usize {
        self.mines
    }

    /// Mines not yet flagged (can go below zero on a wrong guess, shown as 0).
    pub fn mines_left(&self) -> usize {
        self.mines
            .saturating_sub(self.flag.iter().filter(|f| **f).count())
    }

    /// Whether (col, row) hides a mine. Before the first open, nothing does yet.
    pub fn is_mine(&self, col: usize, row: usize) -> bool {
        col < self.cols && row < self.rows && self.mine[row * self.cols + col]
    }

    pub fn is_open(&self, col: usize, row: usize) -> bool {
        col < self.cols && row < self.rows && self.open[row * self.cols + col]
    }

    fn neighbours(&self, i: usize) -> impl Iterator<Item = usize> + '_ {
        let (x, y) = ((i % self.cols) as i32, (i / self.cols) as i32);
        (-1..=1)
            .flat_map(move |dy| (-1..=1).map(move |dx| (x + dx, y + dy)))
            .filter(move |&(nx, ny)| {
                (nx, ny) != (x, y)
                    && nx >= 0
                    && ny >= 0
                    && (nx as usize) < self.cols
                    && (ny as usize) < self.rows
            })
            .map(move |(nx, ny)| ny as usize * self.cols + nx as usize)
    }

    fn count(&self, i: usize) -> u8 {
        self.neighbours(i).filter(|&n| self.mine[n]).count() as u8
    }

    /// Lays the mines anywhere but `first` and, if there's room, its neighbours.
    fn lay(&mut self, first: usize) {
        let near: Vec<usize> = self.neighbours(first).collect();
        let n = self.cols * self.rows;
        let room = n - 1 - near.len();
        let mut pool: Vec<usize> = (0..n)
            .filter(|&i| i != first && (room < self.mines || !near.contains(&i)))
            .collect();
        for _ in 0..self.mines.min(pool.len()) {
            let i = pool.swap_remove(self.rng.below(pool.len()));
            self.mine[i] = true;
        }
        self.laid = true;
    }

    fn reveal(&mut self, col: usize, row: usize) {
        if col >= self.cols || row >= self.rows {
            return;
        }
        let i = row * self.cols + col;
        if self.open[i] || self.flag[i] {
            return;
        }
        if !self.laid {
            self.lay(i);
        }
        if self.mine[i] {
            self.open[i] = true;
            self.over = true;
            return;
        }
        // Flood: an empty square opens everything around it.
        let mut todo = vec![i];
        while let Some(j) = todo.pop() {
            if self.open[j] || self.flag[j] {
                continue;
            }
            self.open[j] = true;
            if self.count(j) == 0 {
                todo.extend(self.neighbours(j).filter(|&n| !self.open[n]));
            }
        }
        let safe = self.cols * self.rows - self.mines;
        if self.open.iter().filter(|o| **o).count() == safe {
            self.won = true;
            self.over = true;
        }
    }

    fn toggle_flag(&mut self, col: usize, row: usize) {
        if col < self.cols && row < self.rows {
            let i = row * self.cols + col;
            if !self.open[i] {
                self.flag[i] = !self.flag[i];
            }
        }
    }
}

impl Arcade for Minefield {
    fn step(&mut self, input: Input, _ticks: u32) {
        if !unpaused(&mut self.paused, self.over, input) {
            return;
        }
        let (x, y) = self.cursor;
        match input {
            Input::Up => self.cursor.1 = y.saturating_sub(1),
            Input::Down => self.cursor.1 = (y + 1).min(self.rows - 1),
            Input::Left => self.cursor.0 = x.saturating_sub(1),
            Input::Right => self.cursor.0 = (x + 1).min(self.cols - 1),
            Input::Fire => self.reveal(x, y),
            Input::Alt => self.toggle_flag(x, y),
            Input::Click { col, row } => {
                if col < self.cols && row < self.rows {
                    self.cursor = (col, row);
                }
                self.reveal(col, row);
            }
            Input::Flag { col, row } => self.toggle_flag(col, row),
            _ => {}
        }
    }

    fn cols(&self) -> usize {
        self.cols
    }

    fn rows(&self) -> usize {
        self.rows
    }

    fn cells(&self) -> Vec<Cell> {
        (0..self.cols * self.rows)
            .map(|i| {
                // When it's lost, every mine shows.
                let shown = self.open[i] || (self.over && !self.won && self.mine[i]);
                match (shown, self.mine[i], self.flag[i]) {
                    (true, true, _) => Cell::Mine,
                    (true, false, _) => match self.count(i) {
                        0 => Cell::Empty,
                        n => Cell::Number(n),
                    },
                    (false, _, true) => Cell::Flag,
                    (false, _, false) => Cell::Covered,
                }
            })
            .collect()
    }

    fn cursor(&self) -> Option<(usize, usize)> {
        Some(self.cursor)
    }

    fn over(&self) -> bool {
        self.over
    }

    fn won(&self) -> bool {
        self.won
    }

    fn paused(&self) -> bool {
        self.paused
    }

    /// The safe squares opened.
    fn score(&self) -> u32 {
        (0..self.open.len())
            .filter(|&i| self.open[i] && !self.mine[i])
            .count() as u32
    }

    fn status(&self) -> String {
        let state = if self.won {
            "  FIELD CLEARED"
        } else if self.over {
            "  BOOM"
        } else if self.paused {
            "  PAUSED"
        } else {
            ""
        };
        format!("MINES {}{state}", self.mines_left())
    }
}
