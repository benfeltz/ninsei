//! The deck's hard disk, as System's Disk panel shows it: the programs installed on it *are*
//! the loadout, and the stash is the box of disks beside it. Everything the panel draws is
//! worked out here and tested headlessly: what's installed and in what order, the block map,
//! installing and removing (with the HDD cap's refusal), and how the installed programs will
//! sit in RAM at jack-in.

use serde::{Deserialize, Serialize};

use crate::binary::{self, Spec};
use crate::data::{Data, Effect, WarezKind};
use crate::hub::Save;

/// Blocks per HDD slot on the map. The map is `hdd_cap * BLOCKS_PER_SLOT` blocks; a program
/// takes `blocks(spec)` of them. Display only: the HDD's rule is still one program a slot.
pub const BLOCKS_PER_SLOT: usize = 4;

/// `data/hub/disk.ron`: the pattern each kind of program fills its blocks with.
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct DiskDef {
    pub fills: Vec<KindFill>,
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct KindFill {
    pub kind: WarezKind,
    pub pattern: String,
}

impl DiskDef {
    /// The pattern a kind's blocks are filled with.
    pub fn fill(&self, kind: WarezKind) -> Option<&str> {
        self.fills
            .iter()
            .find(|f| f.kind == kind)
            .map(|f| f.pattern.as_str())
    }

    pub fn validate(&self, data: &Data) -> Result<(), String> {
        for k in [WarezKind::Breaker, WarezKind::Shield, WarezKind::Utility] {
            let p = self
                .fill(k)
                .ok_or_else(|| format!("no fill for {k:?} programs"))?;
            if data.pattern(p).is_none() {
                return Err(format!("{k:?} fills with unknown pattern `{p}`"));
            }
        }
        Ok(())
    }
}

/// Something that can sit on the disk.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Disk {
    /// One of the kit's free programs, by warez id. Always yours; it comes back every run.
    Free(String),
    /// A program from the stash, by its stash id.
    Stash(u32),
}

/// A program on the disk or in the stash, with what its binary says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Program {
    pub at: Disk,
    /// The factory program it started as (index into `Data::warez`), for its blurb.
    pub warez: usize,
    pub spec: Spec,
    pub kind: WarezKind,
}

impl Program {
    /// Blocks it takes on the map: half its effect's budget cost (a bigger program is a
    /// bigger file), at least one, at most a slot's worth.
    pub fn blocks(&self) -> usize {
        (binary::cost(self.spec.effect).div_ceil(2) as usize).clamp(1, BLOCKS_PER_SLOT)
    }

    /// Power, for the info box: what a breaker breaks, a shield holds or a utility does.
    pub fn power(&self) -> Option<u32> {
        match self.spec.effect {
            Effect::Break { power, .. } => Some(power),
            Effect::Shield(n) | Effect::Repair(n) | Effect::Scrub(n) => Some(n),
            Effect::Stun => None,
        }
    }
}

/// What kind a binary's effect makes it.
pub fn kind(spec: &Spec) -> WarezKind {
    match spec.effect {
        Effect::Break { .. } => WarezKind::Breaker,
        Effect::Shield(_) => WarezKind::Shield,
        _ => WarezKind::Utility,
    }
}

fn program(save: &Save, data: &Data, at: Disk) -> Option<Program> {
    let (warez, bin) = match &at {
        Disk::Free(id) => {
            let w = data.warez_ix(id)?;
            (w, binary::encode(&data.warez[w], save.key))
        }
        Disk::Stash(id) => {
            let item = save.item(*id)?;
            (data.warez_ix(&item.origin)?, item.bin()?)
        }
    };
    let spec = binary::check(&bin, save.key).ok()?;
    let kind = kind(&spec);
    Some(Program {
        at,
        warez,
        spec,
        kind,
    })
}

/// The programs installed on the disk, in the order they'll load: the free programs first, then
/// the stash's, as the loadout lists them. Only free programs the kit still gives count.
pub fn installed(save: &Save, data: &Data) -> Vec<Program> {
    let free = save.free(data);
    save.loadout
        .starters
        .iter()
        .filter(|id| free.contains(id))
        .map(|id| Disk::Free(id.clone()))
        .chain(save.loadout.stash.iter().map(|id| Disk::Stash(*id)))
        .filter_map(|at| program(save, data, at))
        .take(save.hdd_cap(data) as usize)
        .collect()
}

/// What's in the box beside the disk: free programs not installed, then stash programs not
/// installed.
pub fn shelf(save: &Save, data: &Data) -> Vec<Program> {
    let on = |at: &Disk| match at {
        Disk::Free(id) => save.loadout.starters.contains(id),
        Disk::Stash(id) => save.loadout.stash.contains(id),
    };
    save.free(data)
        .iter()
        .map(|id| Disk::Free(id.clone()))
        .chain(save.stash.iter().map(|s| Disk::Stash(s.id)))
        .filter(|at| !on(at))
        .filter_map(|at| program(save, data, at))
        .collect()
}

/// The block map laid out end to end in load order: what Defragment makes.
fn compact(on: &[Program], total: usize) -> Vec<Option<Disk>> {
    let mut out = vec![None; total];
    let mut at = 0;
    for p in on {
        for b in out.iter_mut().skip(at).take(p.blocks()) {
            *b = Some(p.at.clone());
        }
        at += p.blocks();
    }
    out
}

/// The disk as it lies, by owner: the saved map kept where it can be (each program's blocks
/// stay where they are), programs gone from the disk freed, and any program short of blocks
/// (just installed) given the first free ones. A map that no longer fits the disk (the HDD
/// changed size) lies compact.
pub fn layout(save: &Save, data: &Data) -> Vec<Option<Disk>> {
    let on = installed(save, data);
    let total = save.hdd_cap(data) as usize * BLOCKS_PER_SLOT;
    if save.disk_map.len() != total {
        return compact(&on, total);
    }
    let mut out: Vec<Option<Disk>> = save
        .disk_map
        .iter()
        .map(|b| b.clone().filter(|d| on.iter().any(|p| &p.at == d)))
        .collect();
    for p in &on {
        let have = out.iter().filter(|b| b.as_ref() == Some(&p.at)).count();
        if have > p.blocks() {
            // Too many (its binary shrank): free the extras from the end.
            let mut extra = have - p.blocks();
            for b in out.iter_mut().rev() {
                if extra > 0 && b.as_ref() == Some(&p.at) {
                    *b = None;
                    extra -= 1;
                }
            }
        }
        for _ in have..p.blocks() {
            match out.iter().position(Option::is_none) {
                Some(i) => out[i] = Some(p.at.clone()),
                None => return compact(&on, total),
            }
        }
    }
    out
}

/// The block map: one entry per block, the index of the installed program in it (into
/// `installed`) or `None` when free.
pub fn map(save: &Save, data: &Data) -> Vec<Option<usize>> {
    let on = installed(save, data);
    layout(save, data)
        .iter()
        .map(|b| b.as_ref().and_then(|d| on.iter().position(|p| &p.at == d)))
        .collect()
}

/// How fragmented the disk is, in percent: the share of installed programs whose blocks
/// aren't one unbroken run (as DEFRAG reports it).
pub fn fragmentation(save: &Save, data: &Data) -> u32 {
    let on = installed(save, data);
    if on.is_empty() {
        return 0;
    }
    let lay = layout(save, data);
    let split = on
        .iter()
        .filter(|p| {
            let at: Vec<usize> = (0..lay.len())
                .filter(|&i| lay[i].as_ref() == Some(&p.at))
                .collect();
            at.windows(2).any(|w| w[1] != w[0] + 1)
        })
        .count();
    (split * 100 / on.len()) as u32
}

/// Defragment's pass, frame by frame: from the map as it lies to the compact one, one block
/// moved into place each step (a swap). The first frame is the map now; the last is compact.
/// An unfragmented disk still gets a short pass: it reads every block, moves none.
pub fn defrag_steps(save: &Save, data: &Data) -> Vec<Vec<Option<usize>>> {
    let on = installed(save, data);
    let total = save.hdd_cap(data) as usize * BLOCKS_PER_SLOT;
    let index = |m: &[Option<Disk>]| -> Vec<Option<usize>> {
        m.iter()
            .map(|b| b.as_ref().and_then(|d| on.iter().position(|p| &p.at == d)))
            .collect()
    };
    let target = compact(&on, total);
    let mut cur = layout(save, data);
    let mut frames = vec![index(&cur)];
    for i in 0..total {
        if cur[i] != target[i]
            && let Some(j) = (i + 1..total).find(|&j| cur[j] == target[i])
        {
            cur.swap(i, j);
            frames.push(index(&cur));
        }
    }
    frames
}

impl Save {
    /// Scatters the installed programs' blocks across the disk, the same way for the same
    /// seed. Nothing in play does this yet: it's for ICE that fragments your HDD (#125), and
    /// for tests.
    pub fn fragment(&mut self, data: &Data, seed: u64) {
        let mut lay = layout(self, data);
        let mut rng = crate::rng::Rng::new(seed);
        for i in (1..lay.len()).rev() {
            let j = rng.below(i + 1);
            lay.swap(i, j);
        }
        self.disk_map = lay;
    }

    /// Defragment: lays the disk out end to end again.
    pub fn defragment(&mut self) -> String {
        self.disk_map.clear();
        "Defragmentation complete. 0% fragmented.".into()
    }

    /// Whether a program is on the disk now.
    pub fn is_installed(&self, at: &Disk) -> bool {
        match at {
            Disk::Free(id) => self.loadout.starters.contains(id),
            Disk::Stash(id) => self.loadout.stash.contains(id),
        }
    }

    /// Why a program can't be installed now, if it can't.
    pub fn install_blocker(&self, data: &Data, at: &Disk) -> Option<String> {
        if self.is_installed(at) {
            return Some("it's already on the disk.".into());
        }
        let known = match at {
            Disk::Free(id) => self.free(data).contains(id),
            Disk::Stash(id) => self.item(*id).is_some(),
        };
        if !known {
            return Some("no such program.".into());
        }
        let cap = self.hdd_cap(data) as usize;
        if installed(self, data).len() >= cap {
            return Some(format!(
                "Disk full: {cap} of {cap} slots used. Remove a program first."
            ));
        }
        None
    }

    /// Installs a program from the box onto the disk: it's in the next run.
    pub fn install(&mut self, data: &Data, at: &Disk) -> Result<String, String> {
        if let Some(why) = self.install_blocker(data, at) {
            return Err(why);
        }
        match at {
            Disk::Free(id) => self.loadout.starters.push(id.clone()),
            Disk::Stash(id) => self.loadout.stash.push(*id),
        }
        Ok(String::new())
    }

    /// Takes a program off the disk, back into the box.
    pub fn remove(&mut self, at: &Disk) -> Result<String, String> {
        let before = (self.loadout.starters.len(), self.loadout.stash.len());
        match at {
            Disk::Free(id) => self.loadout.starters.retain(|s| s != id),
            Disk::Stash(id) => self.loadout.stash.retain(|s| s != id),
        }
        if before == (self.loadout.starters.len(), self.loadout.stash.len()) {
            return Err("it isn't on the disk.".into());
        }
        Ok(String::new())
    }
}
