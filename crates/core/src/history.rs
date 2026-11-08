//! What a run has been doing, for the graphs: per action, per hop and per combat turn.
//! Filled by `Game::apply` from before/after snapshots, so it only ever watches the rules;
//! it never changes them. Capped, so a long run never grows it without bound.

use crate::game::{Action, Game};

/// How many points each series keeps.
pub const HISTORY_LEN: usize = 96;

/// The run's state after one action, for the sparklines.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sample {
    pub trace: u32,
    pub integrity: u32,
    /// CPU left this turn (the deck's max outside a fight).
    pub cpu: u32,
    /// GB left for IFs this turn (the deck's RAM outside a fight).
    pub ram: u32,
    pub credits: u32,
    pub hops: u32,
}

/// One turn of a fight.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TurnRow {
    pub turn: u32,
    /// ICE strength your programs took off.
    pub dealt: u32,
    /// Integrity the ICE took off you.
    pub taken: u32,
    /// Shield that soaked a hit.
    pub absorbed: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct History {
    /// Counts every action: the clock the graphs and log stamps run on.
    pub tick: u32,
    /// One sample per action.
    pub samples: Vec<Sample>,
    /// Trace after each hop, starting at the entry.
    pub trace_hops: Vec<u32>,
    /// This fight's (or the last fight's) ICE strength after each action, from its start.
    pub ice: Vec<u32>,
    /// This fight's (or the last fight's) turns, oldest first.
    pub turns: Vec<TurnRow>,
    /// Programs run: (slot uid, tick).
    pub runs: Vec<(u32, u32)>,
    /// The last time something hit the deck, for the damage glitches.
    pub last_hit: Option<Hit>,
    /// Every fight this run, in order: for the ICE database at home.
    pub met: Vec<Met>,
    /// Damage dealt so far this turn.
    pending: u32,
}

/// A fight met on a run: which ICE, its strength when the fight began, and whether it fell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Met {
    /// Index into `Data::ice`.
    pub ice: usize,
    pub strength: u32,
    pub broken: bool,
}

/// A hit on the deck.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    /// The tick it landed on.
    pub tick: u32,
    /// Integrity lost, or (if `soaked`) shield spent.
    pub amount: u32,
    /// A shield took it all.
    pub soaked: bool,
}

/// The bits of a game the history compares before and after an action.
#[derive(Clone, Copy, Debug)]
pub struct Snap {
    hops: u32,
    integrity: u32,
    /// (node, ICE strength, shield, CPU left) while fighting.
    fight: Option<(usize, u32, u32, u32)>,
}

impl Snap {
    pub fn of(g: &Game) -> Snap {
        Snap {
            hops: g.hops,
            integrity: g.deck.integrity,
            fight: g
                .combat
                .as_ref()
                .map(|c| (c.node, c.strength, c.shield, c.cpu)),
        }
    }
}

fn push<T>(v: &mut Vec<T>, x: T) {
    v.push(x);
    if v.len() > HISTORY_LEN {
        v.remove(0);
    }
}

impl History {
    /// A fresh history for a run starting now.
    pub fn start(g: &Game) -> History {
        let mut h = History::default();
        push(&mut h.samples, sample(g));
        push(&mut h.trace_hops, g.deck.trace);
        h
    }

    /// Records what `action` did, given the game before (`was`) and after (`g`).
    pub fn record(&mut self, was: &Snap, g: &Game, action: Action, uid: Option<u32>) {
        self.tick += 1;
        push(&mut self.samples, sample(g));
        if g.hops != was.hops {
            push(&mut self.trace_hops, g.deck.trace);
        }
        let now = g
            .combat
            .as_ref()
            .map(|c| (c.node, c.strength, c.shield, c.cpu));
        let lost = was.integrity.saturating_sub(g.deck.integrity);
        let soaked = match (was.fight, now) {
            (Some(a), Some(b)) => a.2.saturating_sub(b.2),
            _ => 0,
        };
        if lost > 0 || soaked > 0 {
            self.last_hit = Some(Hit {
                tick: self.tick,
                amount: lost.max(soaked),
                soaked: lost == 0,
            });
        }
        match (was.fight, now) {
            // A new fight: fresh graphs.
            (None, Some((_, strength, ..))) => {
                if let Some(c) = &g.combat {
                    self.met.push(Met {
                        ice: c.ice,
                        strength,
                        broken: false,
                    });
                }
                self.ice = vec![strength];
                self.turns.clear();
                self.pending = 0;
            }
            (Some((node, before, ..)), Some((n2, after, ..))) if node == n2 => {
                self.pending += before.saturating_sub(after);
                push(&mut self.ice, after);
            }
            // The fight ended: the ICE fell (or you left in a flatline).
            (Some((_, before, ..)), None) => {
                let won = !matches!(g.phase, crate::game::Phase::Flatlined);
                if won {
                    self.pending += before;
                    push(&mut self.ice, 0);
                    if let Some(m) = self.met.last_mut() {
                        m.broken = true;
                    }
                }
                self.close_turn(g, was);
            }
            _ => {}
        }
        if matches!(action, Action::EndTurn) && was.fight.is_some() && now.is_some() {
            self.close_turn(g, was);
        }
        // A program ran if it cost CPU (or ended the fight).
        let spent = match (was.fight, now) {
            (Some(a), Some(b)) => b.3 < a.3,
            (Some(_), None) => true,
            _ => false,
        };
        if let (Action::Run(_), Some(uid), true) = (action, uid, spent) {
            push(&mut self.runs, (uid, self.tick));
        }
    }

    fn close_turn(&mut self, g: &Game, was: &Snap) {
        let shield_before = was.fight.map_or(0, |f| f.2);
        let shield_after = g.combat.as_ref().map_or(0, |c| c.shield);
        let turn = g.combat.as_ref().map_or(self.turns.len() as u32 + 1, |c| {
            c.turn.saturating_sub(1).max(1)
        });
        push(
            &mut self.turns,
            TurnRow {
                turn,
                dealt: self.pending,
                taken: was.integrity.saturating_sub(g.deck.integrity),
                absorbed: shield_before.saturating_sub(shield_after),
            },
        );
        self.pending = 0;
    }

    /// One field of every sample, oldest first.
    pub fn series(&self, f: impl Fn(&Sample) -> u32) -> Vec<u32> {
        self.samples.iter().map(f).collect()
    }

    /// Ticks since the program with `uid` last ran, if it has.
    pub fn since_run(&self, uid: u32) -> Option<u32> {
        self.runs
            .iter()
            .rev()
            .find(|(u, _)| *u == uid)
            .map(|(_, t)| self.tick - t)
    }
}

fn sample(g: &Game) -> Sample {
    Sample {
        trace: g.deck.trace,
        integrity: g.deck.integrity,
        cpu: g.combat.as_ref().map_or(g.deck.cpu_max, |c| c.cpu),
        ram: g.ifs_left(),
        credits: g.deck.credits,
        hops: g.hops,
    }
}

/// Plotting for btop-style dot graphs, at braille resolution: each character cell is 2 dots
/// wide and 4 tall. The app paints the dots (the font has no braille); this does the maths.
pub mod plot {
    /// The last `cols` values, right-aligned: `None` where there's no data yet.
    pub fn window(values: &[u32], cols: usize) -> Vec<Option<u32>> {
        let start = values.len().saturating_sub(cols);
        let mut out = vec![None; cols.saturating_sub(values.len() - start)];
        out.extend(values[start..].iter().map(|&v| Some(v)));
        out
    }

    /// How many dots tall a value stands in a graph `height` dots tall, scaled to `max`.
    /// Any value above zero shows at least one dot.
    pub fn dots(value: u32, max: u32, height: usize) -> usize {
        if value == 0 || max == 0 || height == 0 {
            return 0;
        }
        let h = (value.min(max) as f32 / max as f32 * height as f32).round() as usize;
        h.clamp(1, height)
    }

    /// Dot heights for a graph `cols` dots wide and `height` dots tall (a cell is 2x4 dots).
    pub fn heights(values: &[u32], max: u32, cols: usize, height: usize) -> Vec<Option<usize>> {
        window(values, cols)
            .into_iter()
            .map(|v| v.map(|v| dots(v, max, height)))
            .collect()
    }

    /// The same graph as braille text, top row first: for tests, logs and anywhere a font
    /// has braille. Empty cells are U+2800.
    pub fn braille(heights: &[Option<usize>], rows: usize) -> Vec<String> {
        let bit = |col: usize, row: usize| -> u32 {
            // Dot numbering: left column 1,2,3,7 and right column 4,5,6,8, top to bottom.
            const LEFT: [u32; 4] = [0, 1, 2, 6];
            const RIGHT: [u32; 4] = [3, 4, 5, 7];
            1 << if col == 0 { LEFT[row] } else { RIGHT[row] }
        };
        (0..rows)
            .map(|r| {
                heights
                    .chunks(2)
                    .map(|pair| {
                        let mut code = 0x2800u32;
                        for (c, h) in pair.iter().enumerate() {
                            let h = h.unwrap_or(0);
                            for dy in 0..4 {
                                // Dot row counted from the bottom of the whole graph.
                                let from_bottom = (rows - 1 - r) * 4 + (3 - dy);
                                if from_bottom < h {
                                    code |= bit(c, dy);
                                }
                            }
                        }
                        char::from_u32(code).unwrap_or(' ')
                    })
                    .collect()
            })
            .collect()
    }
}
