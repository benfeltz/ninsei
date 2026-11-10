//! The sysops' simulator (141.9.4): how a kind's next version is found. When a deck beats a
//! kind, its sysops study the deck's script: they draft versions ([`next_version`]) and fight
//! each one against the script in the real game ([`crate::game::duel`]), from the starts the
//! deck really beat it from. The first draft that costs the deck more than the version in force
//! does, by the bar, ships. Then they rest on that kind until the deck beats the version in
//! force again.
//!
//! Least effort is the bound: they stop at the first draft that does better, never the best
//! one, and try small changes first (patches, then minors, then majors). After their patience
//! they rest without shipping. The ICE's own machine (its cpu, ram, hdd and verbs) is the only
//! limit on how strong a version gets.
//!
//! Pure: a step reads no clock and no run's RNG. Draft k of a study is drawn from the save's
//! key, the kind, the version and k, and every duel from its own seed, so the same save and
//! script reach the same next version however the steps are batched. Only when it lands
//! depends on time, which the app counts ([`crate::tuning::STUDY_DRAFT_MS`]).

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::data::Data;
use crate::game::{Duel, Kit, RunMods, Start, duel};
use crate::ice_version::{Bump, Next, Version, against, fired, next_version, seed};
use crate::rng::Rng;
use crate::tuning::{STUDY_BAR, STUDY_PATIENCE, STUDY_STARTS};

/// The deck as its sysops logged it when it beat their ICE: its numbers, the programs on its
/// HDD and its script. What their duels are fought against.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct DeckLog {
    pub cpu: u32,
    pub ram: u32,
    pub hdd: u32,
    pub integrity: u32,
    /// Trace gain, percent of normal (the network's `RunMods::trace_pct`).
    pub trace_pct: u32,
    /// The programs on its HDD: (warez id, binary).
    pub programs: Vec<(String, crate::binary::Bin)>,
    /// The deck's program (BASIC text).
    pub script: String,
}

impl DeckLog {
    /// What the sysops log of `kit` (a deck as it stands, [`crate::game::Game::deck_kit`]).
    pub fn of(kit: &Kit, data: &Data) -> DeckLog {
        DeckLog {
            cpu: kit.cpu,
            ram: kit.ram,
            hdd: kit.hdd,
            integrity: kit.integrity,
            trace_pct: kit.mods.trace_pct,
            programs: kit
                .programs
                .iter()
                .filter_map(|(w, bin)| Some((data.warez.get(*w)?.id.clone(), *bin)))
                .collect(),
            script: kit.program.clone(),
        }
    }

    /// The deck to stage a duel with. A program whose warez is gone from `data` stays home.
    pub fn kit(&self, data: &Data) -> Kit {
        Kit {
            cpu: self.cpu,
            ram: self.ram,
            hdd: self.hdd,
            integrity: self.integrity,
            programs: self
                .programs
                .iter()
                .filter_map(|(id, bin)| Some((data.warez_ix(id)?, *bin)))
                .collect(),
            program: self.script.clone(),
            mods: RunMods {
                trace_pct: self.trace_pct,
                ..RunMods::default()
            },
            ..Kit::default()
        }
    }
}

/// A kind's sysops at work on its next version, or resting.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct Study {
    /// By id in `data/ice.ron`.
    pub id: String,
    /// The version they're beating: the one in force when the deck beat it.
    pub version: Version,
    /// The deck that beat it, the last time it did.
    pub deck: DeckLog,
    /// How the fights it lost opened, oldest first: the latest [`STUDY_STARTS`].
    pub starts: Vec<Start>,
    /// Drafts these sysops have tried, of every version: the next is draft number `tried` (of
    /// the stream drawn from the version under study).
    pub tried: u32,
    /// The draft this search began at: sizes and patience count from it.
    pub from: u32,
    /// Resting: a version shipped, or patience ran out, until the deck beats the version in
    /// force again.
    pub rest: bool,
    /// What the version in force costs the deck from `starts`, and the reads the script walked
    /// into there: worked out at a step when missing (never saved: it comes out the same).
    #[serde(skip)]
    base: Option<(u32, Vec<(String, u32)>)>,
}

impl Study {
    /// Kind `id`'s sysops, before the deck has beaten it.
    pub fn new(id: &str) -> Study {
        Study {
            id: id.to_string(),
            ..Study::default()
        }
    }

    /// The deck (`deck`) beat `version` of kind `id` from `start`. Another version, or a new
    /// script, starts a new search, at patches, from this start alone; the same script adds the
    /// start to what they fight it from, and wakes them if they were resting.
    pub fn beaten(&mut self, version: Version, deck: DeckLog, start: Start) {
        let same = self.version == version
            && !self.starts.is_empty()
            && crate::hub::same_play(&self.deck.script, &deck.script);
        self.version = version;
        if !same {
            self.starts.clear();
            self.from = self.tried;
        } else if self.rest {
            self.from = self.tried;
        }
        self.rest = false;
        self.deck = deck;
        self.starts.push(start);
        let extra = self.starts.len().saturating_sub(STUDY_STARTS);
        self.starts.drain(..extra);
        self.base = None;
    }

    /// One draft, fought from every start against the deck's script. `program` is the version
    /// in force's (the one being beaten) and `key` the save's. Returns the version to ship if
    /// this draft clears the bar; they rest then, and when patience runs out.
    pub fn step(&mut self, data: &Arc<Data>, program: &str, key: u64) -> Option<Next> {
        if self.rest {
            return None;
        }
        let (Some(ice), false) = (data.ice_by_id(&self.id), self.starts.is_empty()) else {
            self.rest = true;
            return None;
        };
        let row = &data.ice[ice];
        let first = seed(key, &self.id, self.version);
        let kit = self.deck.kit(data);
        let (base, bias) = match &self.base {
            Some(b) => b.clone(),
            None => {
                let fights = self.fights(data, ice, program, &kit, first);
                let tells: Vec<_> = fights.iter().flat_map(|d| d.tells.clone()).collect();
                let bias = against(&fired(data, row.class, &tells), tells.len() as u32);
                let b = (self.cost(&fights), bias);
                self.base = Some(b.clone());
                b
            }
        };
        let k = self.tried;
        self.tried += 1;
        let at = k - self.from;
        let draft = next_version(data, row, program, size(at), &bias, draft_seed(first, k));
        let out = draft.bump.is_some().then(|| {
            let cost = self.cost(&self.fights(data, ice, &draft.program, &kit, first));
            (cost >= base + STUDY_BAR * self.starts.len() as u32).then_some(draft)
        });
        let ship = out.flatten();
        if ship.is_some() || at + 1 >= STUDY_PATIENCE {
            self.rest = true;
        }
        ship
    }

    /// `program` fought from every start, each duel from its own seed (the same for every
    /// draft, so they're judged on the same luck).
    fn fights(
        &self,
        data: &Arc<Data>,
        ice: usize,
        program: &str,
        kit: &Kit,
        first: u64,
    ) -> Vec<Duel> {
        self.starts
            .iter()
            .enumerate()
            .map(|(i, s)| duel(data, ice, program, kit, *s, draft_seed(!first, i as u32)))
            .collect()
    }

    /// What fights cost the deck, in points: its integrity lost, as a percent of its most, and
    /// trace gained (already a percent). A flatline costs it all its integrity.
    fn cost(&self, fights: &[Duel]) -> u32 {
        let most = self.deck.integrity.max(1);
        fights
            .iter()
            .map(|d| d.integrity_lost * 100 / most + d.trace_gained)
            .sum()
    }
}

/// The size of the change a search tries at its draft `at`: patches for the first third of
/// the sysops' patience, then minors, then majors.
pub fn size(at: u32) -> Bump {
    let third = (STUDY_PATIENCE / 3).max(1);
    Bump::ALL[((at / third) as usize).min(2)]
}

/// The seed of draw `k` of the stream that starts at `first`: the stream advanced `k` times.
fn draft_seed(first: u64, k: u32) -> u64 {
    Rng::new(first.wrapping_add(u64::from(k).wrapping_mul(0x9E37_79B9_7F4A_7C15))).next_u64()
}

/// Steps every study that's at work once, `steps` times, in the order they're kept. `in_force`
/// gives a kind's version in force and its program, by id. Returns what shipped, in order:
/// (kind's id, the version it replaces, the new one). A study whose version is no longer in
/// force rests.
pub fn work(
    studies: &mut [Study],
    data: &Arc<Data>,
    key: u64,
    steps: u32,
    mut in_force: impl FnMut(&str) -> Option<(Version, String)>,
) -> Vec<(String, Version, Next)> {
    let mut out = Vec::new();
    for _ in 0..steps {
        if studies.iter().all(|s| s.rest) {
            break;
        }
        for s in studies.iter_mut().filter(|s| !s.rest) {
            let Some((v, program)) = in_force(&s.id).filter(|(v, _)| *v == s.version) else {
                s.rest = true;
                continue;
            };
            if let Some(next) = s.step(data, &program, key) {
                out.push((s.id.clone(), v, next));
            }
        }
    }
    out
}
