//! A net's AI between fights: where it is on the map, what it knows of the runner, and its
//! program's pass on the map (`Hunter::pass`), one for every hop the runner makes.
//!
//! A pass runs from the top of its program, as the deck's does each turn of a fight, and ends
//! as an ICE's turn does: its cycles (its model's `cpu`) or its IFs (`ram`) run out, it runs off
//! the end, a `STOP`, or it goes round without spending either: a stall. Each walk
//! that does something spends a cycle (`HOLD` spends them all); a walk that can't (no route, or
//! nowhere it knows to go) is skipped and spends nothing, and so are its ICE verbs, which only
//! a fight hears. So a bigger machine is a faster hunter, and nothing else sets its speed.
//!
//! It knows only what its reads give it: where it last saw the runner (when it woke, or a
//! `SCAN` that found them) and whether its last look found them. It walks the links the runner
//! walks: deeper along any link, back along one that isn't one way. Pure: no clock, no RNG.

use crate::basic::{Cond, Goal, Program, Stmt, WalkAct};
use crate::data::IceDef;
use crate::network::{Network, NodeKind};

/// A net's AI on the map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hunter {
    /// Its row in `Data::ice`.
    pub ice: usize,
    /// The node it is on, in the main network.
    pub at: usize,
    /// Where it last saw the runner.
    pub seen: Option<usize>,
    /// Its last look (waking, or a `SCAN`) found the runner.
    pub sighted: bool,
    /// Its program, parsed in its own vocabulary.
    pub prog: Program,
}

/// What a pass reads of the run.
#[derive(Clone, Copy, Debug)]
pub struct Ground<'a> {
    /// The main network.
    pub net: &'a Network,
    /// The node the runner is on in it (a subnet's door while they're inside).
    pub runner: usize,
    /// The runner's trace.
    pub trace: u32,
    /// Its strength, as a fight would open with it.
    pub strength: u32,
}

/// What one line of a pass did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Did {
    /// A `MOVE`: one hop.
    Moved {
        from: usize,
        to: usize,
    },
    /// A `HOLD`: the rest of the pass on this node.
    Held,
    /// A `WAIT`: a cycle on this node.
    Waited,
    /// A `SCAN n` from node `from`, and whether it found the runner.
    Scanned {
        from: usize,
        reach: u32,
        found: bool,
    },
    /// An IF, and what it found.
    If(bool),
    Goto(u32),
    /// A line that did nothing, and why (spends nothing).
    Skipped(String),
    /// It stepped onto the runner's node: the pass is over and they meet.
    Met,
    Stop,
    OutOfCpu,
    OutOfRam,
    /// It ran off the end of its program.
    End,
    /// It went round without spending anything.
    Stall,
}

/// One line of a pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    pub line: u32,
    pub did: Did,
}

impl Hunter {
    /// Wakes `def` (row `ice`) on node `at`, knowing the runner is on `runner`: the trace that
    /// woke it found them.
    pub fn wake(ice: usize, def: &IceDef, at: usize, runner: usize) -> Self {
        Self {
            ice,
            at,
            seen: Some(runner),
            sighted: true,
            prog: crate::basic::parse_with(&def.program, def.vocab()),
        }
    }

    /// One pass of its program on the map, with `def`'s machine (its row). Says what each line
    /// did, in order; the last step says how it ended.
    pub fn pass(&mut self, def: &IceDef, g: &Ground) -> Vec<Step> {
        let mut out = Vec::new();
        let (mut cpu, mut ifs) = (def.cpu, def.ram);
        let mut visited: Vec<u32> = Vec::new();
        let mut pc = self.prog.first();
        let mut last = pc.unwrap_or(0);
        let step = |out: &mut Vec<Step>, line: u32, did: Did| out.push(Step { line, did });
        loop {
            if cpu == 0 {
                step(&mut out, last, Did::OutOfCpu);
                break;
            }
            let Some(no) = pc else {
                step(&mut out, last, Did::End);
                break;
            };
            last = no;
            if visited.contains(&no) {
                step(&mut out, no, Did::Stall);
                break;
            }
            visited.push(no);
            let Some(stmt) = self.prog.line(no).map(|l| l.stmt.clone()) else {
                pc = self.prog.after(no);
                continue;
            };
            let next = self.prog.after(no);
            pc = next;
            match stmt {
                Stmt::Rem => {}
                Stmt::Bad(why) => {
                    let why = self.prog.fault(no).unwrap_or(why);
                    step(&mut out, no, Did::Skipped(why));
                }
                Stmt::Act(a) => step(
                    &mut out,
                    no,
                    Did::Skipped(format!("{}: nothing to hit off the line.", a.describe())),
                ),
                Stmt::Run(_) => {
                    step(
                        &mut out,
                        no,
                        Did::Skipped("?SYNTAX ERROR: unknown keyword".into()),
                    );
                }
                Stmt::Goto(to) if self.prog.line(to).is_some() => {
                    step(&mut out, no, Did::Goto(to));
                    pc = Some(to);
                }
                Stmt::Goto(to) => {
                    step(
                        &mut out,
                        no,
                        Did::Skipped(format!("?UNDEF'D STATEMENT {to}")),
                    );
                }
                Stmt::Stop => {
                    step(&mut out, no, Did::Stop);
                    break;
                }
                Stmt::If(cond) => {
                    if ifs == 0 {
                        step(&mut out, no, Did::OutOfRam);
                        break;
                    }
                    ifs -= 1;
                    visited.clear();
                    let yes = self.holds(&cond, g);
                    step(&mut out, no, Did::If(yes));
                    if !yes {
                        pc = self.prog.skip(next);
                    }
                }
                Stmt::Walk(WalkAct::Toward(goal)) => {
                    let to = match goal {
                        Goal::Target => self.seen,
                        Goal::Exit => exit(g.net),
                    };
                    match to.and_then(|to| next_hop(g.net, self.at, to)) {
                        Some(to) => {
                            cpu -= 1;
                            visited.clear();
                            step(&mut out, no, Did::Moved { from: self.at, to });
                            self.at = to;
                            if to == g.runner {
                                step(&mut out, no, Did::Met);
                                break;
                            }
                        }
                        None => {
                            let why = match (goal, to) {
                                (Goal::Target, None) => "no target.",
                                (_, Some(t)) if t == self.at => "already there.",
                                _ => "no route.",
                            };
                            step(&mut out, no, Did::Skipped(why.into()));
                        }
                    }
                }
                Stmt::Walk(WalkAct::Hold) => {
                    cpu = 0;
                    visited.clear();
                    step(&mut out, no, Did::Held);
                }
                Stmt::Walk(WalkAct::Wait) => {
                    cpu -= 1;
                    visited.clear();
                    step(&mut out, no, Did::Waited);
                }
                Stmt::Walk(WalkAct::Scan(reach)) => {
                    cpu -= 1;
                    visited.clear();
                    let found = hops(g.net, self.at, g.runner).is_some_and(|d| d <= reach);
                    self.sighted = found;
                    if found {
                        self.seen = Some(g.runner);
                    }
                    let from = self.at;
                    step(&mut out, no, Did::Scanned { from, reach, found });
                }
            }
        }
        out
    }

    /// What an IF finds on the map. The reads of a fight (the deck's numbers, what it ran, the
    /// turn) find nothing off the line.
    pub fn holds(&self, cond: &Cond, g: &Ground) -> bool {
        match cond {
            Cond::Seen => self.sighted,
            Cond::Dist(op, n) => op.test(self.distance(g.net).unwrap_or(u32::MAX), *n),
            Cond::TargetTrace(op, n) => op.test(g.trace, *n),
            Cond::At(k) => g.net.nodes[self.at].kind == *k,
            Cond::SelfStr(op, n) => op.test(g.strength, *n),
            _ => false,
        }
    }

    /// The hops it would walk to where it last saw the runner, if it can walk there.
    pub fn distance(&self, net: &Network) -> Option<u32> {
        route(net, self.at, self.seen?).map(|r| r.len() as u32 - 1)
    }
}

/// The net's exit.
pub fn exit(net: &Network) -> Option<usize> {
    net.nodes.iter().position(|n| n.kind == NodeKind::Exit)
}

/// The shortest route the runner's rules let it walk from `from` to `to`, both ends included
/// (deeper along a link, back along one that isn't one way), ties going to the lower node id
/// first, as `Network::routes` lists them. `None` if it can't get there.
pub fn route(net: &Network, from: usize, to: usize) -> Option<Vec<usize>> {
    let mut prev: Vec<Option<usize>> = vec![None; net.nodes.len()];
    let mut seen = vec![false; net.nodes.len()];
    let mut queue = std::collections::VecDeque::from([from]);
    seen[from] = true;
    while let Some(n) = queue.pop_front() {
        if n == to {
            let mut out = vec![to];
            let mut at = to;
            while let Some(p) = prev[at] {
                out.push(p);
                at = p;
            }
            out.reverse();
            return Some(out);
        }
        let mut next = net.routes(n);
        next.sort_unstable();
        for m in next {
            if !std::mem::replace(&mut seen[m], true) {
                prev[m] = Some(n);
                queue.push_back(m);
            }
        }
    }
    None
}

/// The first hop of [`route`], if there is one (none when it's already there).
pub fn next_hop(net: &Network, from: usize, to: usize) -> Option<usize> {
    route(net, from, to)?.get(1).copied()
}

/// Hops between two nodes along links either way, one way or not: how far a scan reaches.
pub fn hops(net: &Network, a: usize, b: usize) -> Option<u32> {
    let mut dist: Vec<Option<u32>> = vec![None; net.nodes.len()];
    dist[a] = Some(0);
    let mut queue = std::collections::VecDeque::from([a]);
    while let Some(n) = queue.pop_front() {
        let d = dist[n]?;
        if n == b {
            return Some(d);
        }
        let back = net
            .nodes
            .iter()
            .filter(|o| o.links.contains(&n))
            .map(|o| o.id);
        let near: Vec<usize> = net.nodes[n].links.iter().copied().chain(back).collect();
        for m in near {
            if dist[m].is_none() {
                dist[m] = Some(d + 1);
                queue.push_back(m);
            }
        }
    }
    None
}
