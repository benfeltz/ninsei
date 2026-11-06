//! The loadout is remembered: what's on the disk goes into every run, and comes home in its
//! place from the exit. A flatline or leaving one behind drops it off the disk.

use ninsei_core::disk::{self, Disk};
use ninsei_core::hub::{Save, StashItem};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

/// A save with two stash programs on the disk (ids 7 and 8).
fn kitted() -> (Data, Save) {
    let d = data();
    let mut s = Save::default();
    s.ensure_hardware(&d);
    for (id, w) in [(7, "torch"), (8, "garrote")] {
        let ix = d.warez_ix(w).unwrap();
        let bin = binary::encode(&d.warez[ix], s.key);
        s.stash.push(StashItem {
            id,
            origin: w.into(),
            hex: binary::to_hex(&bin),
        });
        s.install(&d, &Disk::Stash(id)).unwrap();
    }
    s.next_item = 9;
    (d, s)
}

fn end(g: &mut Game, phase: Phase) {
    g.phase = phase;
}

#[test]
fn the_loadout_goes_in_without_reticking() {
    let (d, mut s) = kitted();
    let g = s.jack_in(&d, &d.rig, 1);
    // They're on the deck, out of the stash, and still remembered on the disk.
    assert!(s.stash.iter().all(|i| i.id != 7 && i.id != 8));
    assert_eq!(s.loadout.stash, vec![7, 8]);
    assert_eq!(g.loadout_keep().len(), 2, "both are ticked to come home");
}

#[test]
fn the_exit_brings_the_loadout_home_in_its_place() {
    let (d, mut s) = kitted();
    let before: Vec<Disk> = disk::installed(&s, &d)
        .iter()
        .map(|p| p.at.clone())
        .collect();
    let mut g = s.jack_in(&d, &d.rig, 1);
    end(&mut g, Phase::JackedOut);
    let keep: Vec<usize> = g
        .loadout_keep()
        .iter()
        .map(|uid| g.hdd.iter().position(|x| x.uid == *uid).unwrap())
        .collect();
    s.record(&g, &keep, &d.rig).unwrap();
    // Home under their old ids, installed again, in the same order.
    assert!(s.item(7).is_some() && s.item(8).is_some());
    let after: Vec<Disk> = disk::installed(&s, &d)
        .iter()
        .map(|p| p.at.clone())
        .collect();
    assert_eq!(after, before);
    // And the next run has them without anyone touching the disk.
    let g2 = s.jack_in(&d, &d.rig, 2);
    assert_eq!(g2.loadout_keep().len(), 2);
}

#[test]
fn leaving_one_behind_drops_it_off_the_disk() {
    let (d, mut s) = kitted();
    let mut g = s.jack_in(&d, &d.rig, 1);
    end(&mut g, Phase::JackedOut);
    let torch = g
        .hdd
        .iter()
        .position(|x| x.spec.name.eq_ignore_ascii_case("torch"));
    let keep: Vec<usize> = torch.into_iter().collect();
    s.record(&g, &keep, &d.rig).unwrap();
    assert!(s.item(7).is_some(), "the torch came home");
    assert!(s.item(8).is_none(), "the garrote was left behind");
    assert_eq!(s.loadout.stash, vec![7]);
}

#[test]
fn a_flatline_loses_the_brought_programs_but_not_the_free_ones() {
    let (d, mut s) = kitted();
    let free_before = s.loadout.starters.clone();
    let mut g = s.jack_in(&d, &d.rig, 1);
    end(&mut g, Phase::Flatlined);
    s.record(&g, &[], &d.rig).unwrap();
    assert!(s.item(7).is_none() && s.item(8).is_none());
    assert!(s.loadout.stash.is_empty());
    assert_eq!(s.loadout.starters, free_before, "free programs always stay");
}

#[test]
fn a_new_find_gets_a_new_id_and_waits_in_the_box() {
    let (d, mut s) = kitted();
    let mut g = s.jack_in(&d, &d.rig, 1);
    let w = d.warez_ix("scrubber").unwrap();
    let mut find = g.factory_slot(w).unwrap();
    find.uid = 9_999;
    g.hdd.push(find);
    end(&mut g, Phase::JackedOut);
    let at = g.hdd.len() - 1;
    s.record(&g, &[at], &d.rig).unwrap();
    let new = s.stash.iter().find(|i| i.origin == "scrubber").unwrap();
    assert!(new.id >= 9, "a new id");
    assert!(
        !s.loadout.stash.contains(&new.id),
        "not installed until you install it"
    );
}
