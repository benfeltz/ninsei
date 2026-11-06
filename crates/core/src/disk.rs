//! The deck's hard disk, as System's Disk panel shows it: the programs installed on it *are*
//! the loadout, and the stash is the box of disks beside it. Everything the panel draws is
//! worked out here and tested headlessly: what's installed and in what order, the block map,
//! installing and removing (with the HDD cap's refusal), and how the installed programs will
//! sit in RAM at jack-in.

use serde::{Deserialize, Serialize};

use crate::binary::{self, Spec};
use crate::data::{Data, Effect, WarezKind};
use crate::hub::{Save, Upgrade};
use crate::tuning::START_RAM;

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
    /// Blocks it takes on the map: its RAM, at least one, at most a slot's worth.
    pub fn blocks(&self) -> usize {
        (self.spec.ram as usize).clamp(1, BLOCKS_PER_SLOT)
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
        .take(save.hdd_cap(&data.rig) as usize)
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

/// The block map: one entry per block, the index of the installed program in it (into
/// `installed`) or `None` when free. Programs are laid end to end in load order.
pub fn map(save: &Save, data: &Data) -> Vec<Option<usize>> {
    let total = save.hdd_cap(&data.rig) as usize * BLOCKS_PER_SLOT;
    let mut out = vec![None; total];
    let mut at = 0;
    for (i, p) in installed(save, data).iter().enumerate() {
        for b in out.iter_mut().skip(at).take(p.blocks()) {
            *b = Some(i);
        }
        at += p.blocks();
    }
    out
}

/// How an installed program will sit in RAM at jack-in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InRam {
    /// Index into `installed`.
    pub program: usize,
    /// Its RAM.
    pub ram: u32,
    /// Whether it loads at jack-in. The run loads programs in order while they fit, like
    /// `Game::start`; the rest wait on the HDD.
    pub loads: bool,
}

/// The RAM strip: each installed program, whether it'll load, and the deck's RAM.
pub fn ram_preview(save: &Save, data: &Data) -> (Vec<InRam>, u32) {
    let ram_max = START_RAM + save.rig.bonus(Upgrade::Ram, &data.rig);
    let mut used = 0;
    let strip = installed(save, data)
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let loads = used + p.spec.ram <= ram_max;
            if loads {
                used += p.spec.ram;
            }
            InRam {
                program: i,
                ram: p.spec.ram,
                loads,
            }
        })
        .collect();
    (strip, ram_max)
}

impl Save {
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
        let cap = self.hdd_cap(&data.rig) as usize;
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
