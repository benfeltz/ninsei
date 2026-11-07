//! NIBBLER: a snake in a walled field. It eats the food, grows, speeds up every few meals, and
//! dies on the wall or its own tail.

use std::collections::VecDeque;

use super::{Arcade, Cell, Input, unpaused};
use crate::rng::Rng;

/// Ticks between moves at the start, and the fastest it ever goes.
pub const START_PERIOD: u32 = 4;
pub const FASTEST_PERIOD: u32 = 1;
/// Meals between speed-ups.
pub const SPEEDUP_EVERY: u32 = 5;
/// Points a meal.
pub const FOOD_POINTS: u32 = 10;
/// The snake's length when it hatches.
pub const START_LEN: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Dir {
    Up,
    Down,
    Left,
    Right,
}

impl Dir {
    fn delta(self) -> (i32, i32) {
        match self {
            Dir::Up => (0, -1),
            Dir::Down => (0, 1),
            Dir::Left => (-1, 0),
            Dir::Right => (1, 0),
        }
    }

    fn opposite(self) -> Dir {
        match self {
            Dir::Up => Dir::Down,
            Dir::Down => Dir::Up,
            Dir::Left => Dir::Right,
            Dir::Right => Dir::Left,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Nibbler {
    cols: usize,
    rows: usize,
    rng: Rng,
    /// Head first.
    body: VecDeque<(usize, usize)>,
    heading: Dir,
    /// The turn to take on the next move.
    next: Dir,
    food: Option<(usize, usize)>,
    meals: u32,
    clock: u32,
    over: bool,
    paused: bool,
}

impl Nibbler {
    /// A snake heading right from the middle of a `cols` x `rows` field (walls included).
    pub fn new(seed: u64, cols: usize, rows: usize) -> Self {
        let (cols, rows) = super::Engine::Nibbler.field(cols, rows);
        let (x, y) = (cols / 2, rows / 2);
        let body = (0..START_LEN).map(|i| (x - i, y)).collect();
        let mut n = Self {
            cols,
            rows,
            rng: Rng::new(seed),
            body,
            heading: Dir::Right,
            next: Dir::Right,
            food: None,
            meals: 0,
            clock: 0,
            over: false,
            paused: false,
        };
        n.place_food();
        n
    }

    fn wall(&self, x: usize, y: usize) -> bool {
        x == 0 || y == 0 || x == self.cols - 1 || y == self.rows - 1
    }

    /// Food on a random free square; none left means the field is full and the game is won.
    fn place_food(&mut self) {
        let free: Vec<(usize, usize)> = (0..self.rows)
            .flat_map(|y| (0..self.cols).map(move |x| (x, y)))
            .filter(|&(x, y)| !self.wall(x, y) && !self.body.contains(&(x, y)))
            .collect();
        if free.is_empty() {
            self.food = None;
            self.over = true;
            return;
        }
        self.food = Some(free[self.rng.below(free.len())]);
    }

    /// Ticks between moves now: faster every few meals.
    pub fn period(&self) -> u32 {
        START_PERIOD
            .saturating_sub(self.meals / SPEEDUP_EVERY)
            .max(FASTEST_PERIOD)
    }

    /// Where the food is, if any is left.
    pub fn food(&self) -> Option<(usize, usize)> {
        self.food
    }

    pub fn head(&self) -> (usize, usize) {
        self.body[0]
    }

    pub fn len(&self) -> usize {
        self.body.len()
    }

    pub fn is_empty(&self) -> bool {
        self.body.is_empty()
    }

    fn advance(&mut self) {
        self.heading = self.next;
        let (hx, hy) = self.body[0];
        let (dx, dy) = self.heading.delta();
        let (x, y) = ((hx as i32 + dx) as usize, (hy as i32 + dy) as usize);
        let eats = self.food == Some((x, y));
        // The tail moves out of the way this move unless the snake grows.
        let tail = if eats {
            None
        } else {
            self.body.back().copied()
        };
        let bites = self.body.iter().any(|&p| p == (x, y) && Some(p) != tail);
        if self.wall(x, y) || bites {
            self.over = true;
            return;
        }
        self.body.push_front((x, y));
        if eats {
            self.meals += 1;
            self.place_food();
        } else {
            self.body.pop_back();
        }
    }
}

impl Arcade for Nibbler {
    fn step(&mut self, input: Input, ticks: u32) {
        if !unpaused(&mut self.paused, self.over, input) {
            return;
        }
        let turn = match input {
            Input::Up => Some(Dir::Up),
            Input::Down => Some(Dir::Down),
            Input::Left => Some(Dir::Left),
            Input::Right => Some(Dir::Right),
            _ => None,
        };
        // It can't turn back on itself.
        if let Some(d) = turn
            && d != self.heading.opposite()
        {
            self.next = d;
        }
        for _ in 0..ticks {
            if self.over {
                return;
            }
            self.clock += 1;
            if self.clock >= self.period() {
                self.clock = 0;
                self.advance();
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
        let mut out = vec![Cell::Empty; self.cols * self.rows];
        for y in 0..self.rows {
            for x in 0..self.cols {
                if self.wall(x, y) {
                    out[y * self.cols + x] = Cell::Wall;
                }
            }
        }
        if let Some((x, y)) = self.food {
            out[y * self.cols + x] = Cell::Food;
        }
        for (i, &(x, y)) in self.body.iter().enumerate() {
            out[y * self.cols + x] = if i == 0 { Cell::Head } else { Cell::Body };
        }
        out
    }

    fn over(&self) -> bool {
        self.over
    }

    fn won(&self) -> bool {
        self.over && self.food.is_none()
    }

    fn paused(&self) -> bool {
        self.paused
    }

    fn score(&self) -> u32 {
        self.meals * FOOD_POINTS
    }

    fn status(&self) -> String {
        let state = if self.won() {
            "  FIELD CLEARED"
        } else if self.over {
            "  GAME OVER"
        } else if self.paused {
            "  PAUSED"
        } else {
            ""
        };
        format!("SCORE {}  LENGTH {}{state}", self.score(), self.len())
    }
}
