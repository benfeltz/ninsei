use ninsei_core::hub::Save;
use ninsei_core::icedb;
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

fn fresh(d: &Data) -> Save {
    let mut s = Save::default();
    s.ensure_key(0x1CE);
    s.ensure_phonebook(d, 3);
    s.add_starters(d);
    s.learn_board("gutter", "test");
    s.ensure_hardware(d);
    // Breakers from the Gutter's FREEWARE, so the fights can be won.
    for id in ["icepick", "decoder"] {
        let w = d.warez_ix(id).unwrap();
        let item = s.next_item;
        s.next_item += 1;
        s.stash.push(ninsei_core::hub::StashItem {
            id: item,
            origin: id.into(),
            hex: ninsei_core::binary::to_hex(&ninsei_core::binary::encode(&d.warez[w], s.key)),
        });
        s.toggle_stash(&d.rig, item).unwrap();
    }
    s
}

/// Plays a run until its first fight, then wins or loses it by fiat and books the run.
/// `None` if the walk never met ICE.
fn fight_once(s: &mut Save, d: &Data, seed: u64, win: bool) -> Option<String> {
    let e = s.phonebook[0].id;
    let mut g = s.dial(d, e, seed).unwrap();
    for _ in 0..200 {
        if g.phase == Phase::Combat {
            break;
        }
        let Some(&to) = g.reachable().first() else {
            break;
        };
        g.apply(Action::Move(to));
        if g.phase == Phase::Encounter {
            // Not what this test is about: walk past the conversation.
            g.encounter = None;
            g.phase = Phase::Exploring;
        }
    }
    if g.phase != Phase::Combat {
        return None;
    }
    let id = g.data.ice[g.combat.as_ref().unwrap().ice].id.clone();
    if win {
        // Break it the honest way: run breakers and end turns until it falls or we do.
        for _ in 0..200 {
            if g.phase != Phase::Combat {
                break;
            }
            let ok = |i: &usize| g.preview(*i).is_ok();
            let breaks = |i: &usize| matches!(g.hdd[*i].spec.effect, Effect::Break { .. });
            let ran = (0..g.hdd.len())
                .filter(ok)
                .find(breaks)
                .or_else(|| (0..g.hdd.len()).find(ok));
            match ran {
                Some(i) => g.apply(Action::Run(i)),
                None => g.apply(Action::EndTurn),
            }
            g.deck.integrity = g.deck.integrity_max;
        }
        assert_ne!(g.phase, Phase::Combat);
    }
    g.phase = Phase::Flatlined;
    s.record(&g, &[], &d.rig).unwrap();
    Some(id)
}

#[test]
fn unmet_ice_is_question_marks() {
    let d = data();
    let s = fresh(&d);
    let all = icedb::entries(&s, &d);
    assert_eq!(all.len(), d.ice.len());
    for e in &all {
        assert_eq!(e.name(), "????");
        assert!(e.lines(&s, &d).iter().all(|l| !l.contains(&e.def.name)));
    }
}

#[test]
fn a_fight_fills_in_its_entry() {
    let d = data();
    let mut s = fresh(&d);
    let id = (1..40)
        .find_map(|seed| fight_once(&mut s, &d, seed, true))
        .expect("some seed fights");
    let seen = s.ice_seen.iter().find(|x| x.id == id).unwrap();
    assert_eq!((seen.met, seen.broken), (1, 1));
    let e = icedb::entries(&s, &d)
        .into_iter()
        .find(|e| e.def.id == id)
        .unwrap();
    assert_eq!(e.name(), e.def.name);
    let page = e.lines(&s, &d).join("\n");
    assert!(page.contains(e.def.class.label()));
    assert!(page.contains(&e.def.intents[0].describe()));
    // Fought again and lost: met twice, broken once.
    let mut again = 0;
    for seed in 5..60 {
        let mut t = s.clone();
        if fight_once(&mut t, &d, seed, false).as_ref() == Some(&id) {
            s = t;
            again = seed;
            break;
        }
    }
    assert!(again > 0, "never met {id} again");
    let seen = s.ice_seen.iter().find(|x| x.id == id).unwrap();
    assert_eq!((seen.met, seen.broken), (2, 1));
    assert!(seen.lo <= seen.hi);
}

#[test]
fn breakers_that_match_are_named() {
    let d = data();
    let s = fresh(&d);
    // The starters: icepick is generic; check every class gets a fair answer.
    for class in [IceClass::Barrier, IceClass::CodeGate, IceClass::Sentry] {
        let m = icedb::matching(&s, &d, class);
        for f in &m {
            let w = d
                .warez
                .iter()
                .find(|w| format!("{}.exe", w.name.to_ascii_lowercase()) == *f)
                .expect("a real program");
            assert!(matches!(w.effect, Effect::Break { vs: Some(c), .. } if c == class));
        }
    }
}
