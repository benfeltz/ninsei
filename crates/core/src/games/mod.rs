//! The games you own, as rules: each is a pure, seeded state machine on a grid of cells. The
//! app sends an [`Input`] and a count of ticks (the game's own `tick_ms` each) to
//! [`Arcade::step`] and draws [`Arcade::cells`]; an engine never reads a clock, and the same
//! seed and inputs always play the same game. Which engine a game runs is its
//! `GameDef::engine` in `data/hub/games.ron`.

use serde::Deserialize;

pub mod blox;
pub mod minefield;
pub mod nibbler;

/// One press, or nothing. Arrows steer and move a cursor, `Fire` is the main button, `Alt`
/// the second (flag a square), and clicks land on a cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    None,
    Up,
    Down,
    Left,
    Right,
    Fire,
    Alt,
    /// Toggles the pause: while paused, ticks don't count and only `Pause` does anything.
    Pause,
    /// The main button on a cell.
    Click {
        col: usize,
        row: usize,
    },
    /// The second button on a cell.
    Flag {
        col: usize,
        row: usize,
    },
}

/// What one cell of a game shows. The app draws each in the theme's colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    Empty,
    Wall,
    Body,
    Head,
    Food,
    /// A settled or falling block, by piece (1-7).
    Block(u8),
    /// A square not yet opened.
    Covered,
    Flag,
    /// A flag on a square with no mine, shown once the game is lost.
    WrongFlag,
    /// An open square and the mines around it (0-8).
    Number(u8),
    Mine,
}

/// A game's rules, as the app sees them.
pub trait Arcade {
    /// Applies `input`, then runs `ticks` ticks of the game's clock.
    fn step(&mut self, input: Input, ticks: u32);
    fn cols(&self) -> usize;
    fn rows(&self) -> usize;
    /// Every cell, row by row: exactly `cols * rows` of them.
    fn cells(&self) -> Vec<Cell>;
    /// Where the keyboard cursor is, for a game that has one.
    fn cursor(&self) -> Option<(usize, usize)> {
        None
    }
    fn over(&self) -> bool;
    /// Whether it ended in a win (a cleared field), not a loss.
    fn won(&self) -> bool {
        false
    }
    fn paused(&self) -> bool;
    fn score(&self) -> u32;
    /// The game's own status line: what it would print under the field.
    fn status(&self) -> String;
}

/// Declares [`Engine`] and [`Engine::ALL`] from one list, so a new engine can't be left out
/// of `ALL` (and so out of the rule that every engine has a row in `games.ron`).
macro_rules! engines {
    ($($(#[$doc:meta])* $name:ident),+ $(,)?) => {
        /// Which rules a game runs (`engine` in `games.ron`).
        #[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Engine {
            $($(#[$doc])* $name,)+
        }

        impl Engine {
            /// Every engine, in declaration order.
            pub const ALL: &'static [Engine] = &[$(Engine::$name),+];
        }
    };
}

engines! {
    /// A snake on a walled field.
    Nibbler,
    /// Falling tetrominoes.
    Blox,
    /// Mines under covered squares.
    Minefield,
}

impl Engine {
    /// The smallest field the engine plays on, (cols, rows). The engines clamp to it, so a
    /// field validation lets through is exactly the field the game plays on.
    pub fn min_size(self) -> (u32, u32) {
        match self {
            Engine::Nibbler => (8, 6),
            Engine::Blox => (6, 8),
            Engine::Minefield => (4, 4),
        }
    }

    /// `cols` x `rows`, grown to at least [`Self::min_size`].
    pub(crate) fn field(self, cols: usize, rows: usize) -> (usize, usize) {
        let (c, r) = self.min_size();
        (cols.max(c as usize), rows.max(r as usize))
    }
}

/// A new game of `def`, on its own field, from `seed`: the same seed always deals the same
/// game.
pub fn new_game(def: &crate::data::GameDef, seed: u64) -> Box<dyn Arcade> {
    let (cols, rows) = (def.cols as usize, def.rows as usize);
    match def.engine {
        Engine::Nibbler => Box::new(nibbler::Nibbler::new(seed, cols, rows)),
        Engine::Blox => Box::new(blox::Blox::new(seed, cols, rows)),
        Engine::Minefield => Box::new(minefield::Minefield::new(seed, cols, rows, def.tick_ms)),
    }
}

/// The pause every engine shares: `Pause` toggles it, and nothing else gets through it.
/// Returns whether the step goes on.
pub(crate) fn unpaused(paused: &mut bool, over: bool, input: Input) -> bool {
    if input == Input::Pause && !over {
        *paused = !*paused;
        return false;
    }
    !*paused && !over
}
