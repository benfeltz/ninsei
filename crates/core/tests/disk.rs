//! System's Disk panel: the disk is the loadout, the box beside it is the stash.

use ninsei_core::disk::{self, BLOCKS_PER_SLOT, Disk};
use ninsei_core::hub::{Save, ShellEffect, StashItem};
use ninsei_core::system::Panel;
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

fn fresh() -> Save {
    let d = data();
    let mut s = Save::default();
    s.ensure_hardware(&d);
    s
}

/// A save with one stash program (a factory torch, sealed with the save's key).
fn with_torch() -> (Data, Save, u32) {
    let d = data();
    let mut s = fresh();
    let w = d.warez_ix("torch").unwrap();
    let bin = binary::encode(&d.warez[w], s.key);
    s.stash.push(StashItem {
        id: 7,
        origin: "torch".into(),
        hex: binary::to_hex(&bin),
    });
    (d, s, 7)
}

#[test]
fn the_disk_is_the_loadout_in_load_order() {
    let d = data();
    let s = fresh();
    let on = disk::installed(&s, &d);
    let kit = s.kit(&d, &d.rig);
    assert_eq!(on.len(), kit.programs.len());
    for (p, (w, _)) in on.iter().zip(&kit.programs) {
        assert_eq!(
            p.warez, *w,
            "the disk lists programs in the order they load"
        );
    }
}

#[test]
fn the_map_lays_programs_end_to_end_and_counts_free_blocks() {
    let d = data();
    let s = fresh();
    let on = disk::installed(&s, &d);
    let map = disk::map(&s, &d);
    assert_eq!(map.len(), s.hdd_cap(&d.rig) as usize * BLOCKS_PER_SLOT);
    let used: usize = on.iter().map(|p| p.blocks()).sum();
    assert_eq!(map.iter().filter(|b| b.is_some()).count(), used);
    // End to end: every used block comes before every free one.
    let first_free = map.iter().position(|b| b.is_none()).unwrap_or(map.len());
    assert!(map[first_free..].iter().all(|b| b.is_none()));
    for p in &on {
        assert!((1..=BLOCKS_PER_SLOT).contains(&p.blocks()));
    }
}

#[test]
fn install_and_remove_move_programs_between_the_box_and_the_disk() {
    let (d, mut s, id) = with_torch();
    let at = Disk::Stash(id);
    assert!(disk::shelf(&s, &d).iter().any(|p| p.at == at));
    s.install(&d, &at).unwrap();
    assert!(s.is_installed(&at));
    assert!(disk::installed(&s, &d).iter().any(|p| p.at == at));
    assert!(!disk::shelf(&s, &d).iter().any(|p| p.at == at));
    assert!(s.install(&d, &at).is_err(), "already on the disk");
    s.remove(&at).unwrap();
    assert!(!s.is_installed(&at));
    assert!(s.remove(&at).is_err());
    // A free program can come off and go back on too.
    let free = disk::installed(&s, &d)[0].at.clone();
    s.remove(&free).unwrap();
    assert!(disk::shelf(&s, &d).iter().any(|p| p.at == free));
    s.install(&d, &free).unwrap();
}

#[test]
fn a_full_disk_refuses_with_the_reason() {
    let (d, mut s, id) = with_torch();
    let cap = s.hdd_cap(&d.rig) as usize;
    // Fill the disk with free programs and stash copies until it's full.
    let mut n = 100;
    while disk::installed(&s, &d).len() < cap {
        let w = d.warez_ix("icepick").unwrap();
        let bin = binary::encode(&d.warez[w], s.key);
        s.stash.push(StashItem {
            id: n,
            origin: "icepick".into(),
            hex: binary::to_hex(&bin),
        });
        s.install(&d, &Disk::Stash(n)).unwrap();
        n += 1;
    }
    let why = s.install_blocker(&d, &Disk::Stash(id)).unwrap();
    assert!(why.contains("Disk full"), "{why}");
    assert!(s.install(&d, &Disk::Stash(id)).is_err());
}

#[test]
fn the_ram_strip_loads_in_order_while_it_fits() {
    let (d, mut s, id) = with_torch();
    s.install(&d, &Disk::Stash(id)).unwrap();
    let (strip, ram_max) = disk::ram_preview(&s, &d);
    let used: u32 = strip.iter().filter(|r| r.loads).map(|r| r.ram).sum();
    assert!(used <= ram_max);
    // Whatever didn't load wouldn't have fitted at its turn.
    let mut so_far = 0;
    for r in &strip {
        if r.loads {
            so_far += r.ram;
        } else {
            assert!(so_far + r.ram > ram_max);
        }
    }
}

#[test]
fn scrap_takes_a_program_off_the_disk_and_out_of_the_box() {
    let (d, mut s, id) = with_torch();
    s.install(&d, &Disk::Stash(id)).unwrap();
    assert!(s.scrap(id).is_some());
    assert!(
        !disk::installed(&s, &d)
            .iter()
            .any(|p| p.at == Disk::Stash(id))
    );
    assert!(!disk::shelf(&s, &d).iter().any(|p| p.at == Disk::Stash(id)));
}

#[test]
fn open_stash_opens_system_on_disk() {
    let d = data();
    let s = fresh();
    assert_eq!(
        hub::shell("open stash", &s, &d).1,
        ShellEffect::System(Panel::Disk)
    );
}

#[test]
fn every_kind_of_program_has_a_fill() {
    let d = data();
    for k in [WarezKind::Breaker, WarezKind::Shield, WarezKind::Utility] {
        let p = d.disk.fill(k).unwrap();
        assert!(d.pattern(p).is_some(), "{p}");
    }
}
