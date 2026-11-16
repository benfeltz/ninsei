//! 111.1 acceptance, from the outside: Backlink bought on Dead Sector from home, installed,
//! carried into dialed runs of every tier and run wherever its preview allows; refused in a
//! fight and skipped in the deck's script; hex-edited only as dear as its budget allows; and
//! the probe's bots finish every run carrying it.

use std::sync::Arc;

use ninsei_core::bbs::Line;
use ninsei_core::data::Drop;
use ninsei_core::disk::Disk;
use ninsei_core::fight::Outcome;
use ninsei_core::hub::{Entry, Save};
use ninsei_core::probe::{self, Bot};
use ninsei_core::rng::Rng;
use ninsei_core::*;

fn data() -> Arc<Data> {
    Arc::new(Data::builtin().unwrap())
}

/// A save that knows every board, with `bank` to spend.
fn save(bank: u32) -> Save {
    let d = data();
    let mut s = Save::default();
    s.ensure_key(0x111);
    s.learn_tool("wardial");
    s.learn_tool("hexed");
    s.ensure_phonebook(&d, 7);
    s.add_starters(&d);
    for b in &d.boards {
        s.learn_board(&b.id, "a test");
    }
    s.bank = bank;
    s
}

/// Buys Backlink from Dead Sector at home and installs it; its stash id.
fn buy_backlink(d: &Data, s: &mut Save) -> u32 {
    let board = d.board("deadsector").unwrap();
    let call = s.home_call(d, &d.rig, board);
    let b = call.board();
    let (a, f) = call
        .stock()
        .iter()
        .enumerate()
        .flat_map(|(a, files)| files.iter().enumerate().map(move |(f, s)| (a, f, s.def)))
        .find(|&(a, _, def)| b.areas[a].files[def].item == Drop::Warez("backlink".into()))
        .map(|(a, f, _)| (a, f))
        .expect("Dead Sector lists Backlink on every call");
    let price = b.areas[a].files[call.stock()[a][f].def].price;
    let bank = s.bank;
    s.home_buy(d, &d.rig, board, a, f).unwrap();
    assert_eq!(s.bank, bank - price);
    let item = s.stash.last().unwrap();
    let bin = binary::from_hex(&item.hex).unwrap();
    let spec = binary::check(&bin, s.key).expect("a sealed, valid binary");
    assert!(spec.effect.is_hack());
    let id = item.id;
    s.install(d, &Disk::Stash(id)).unwrap();
    id
}

fn dial_entry(s: &mut Save, tier: u32) -> u32 {
    let id = 900 + tier;
    if !s.phonebook.iter().any(|e| e.id == id) {
        s.phonebook.push(Entry {
            id,
            name: format!("tier {tier}"),
            business: "test".into(),
            number: format!("555-09{tier:02}"),
            tier,
            traits: Vec::new(),
            heat: 0,
            runs: 0,
            last: Vec::new(),
        });
    }
    id
}

fn backlink_slot(g: &Game) -> usize {
    g.hdd
        .iter()
        .position(|s| s.spec.effect.is_hack())
        .expect("Backlink rides into the run")
}

#[test]
fn bought_on_dead_sector_it_rides_into_a_run_on_every_call() {
    let d = data();
    let mut s = save(10_000);
    buy_backlink(&d, &mut s);
    let entry = dial_entry(&mut s, 1);
    let g = s.clone().dial(&d, entry, 5).unwrap();
    let slot = backlink_slot(&g);
    assert_eq!(g.hdd[slot].spec.name, "Backlink");
    // Fresh at the entry: nothing runs one way into it.
    let why = g.preview(slot).unwrap_err();
    assert!(why.starts_with("nothing runs one way into"), "{why}");
}

/// A runner on dialed networks of every tier wandering at random, running Backlink wherever its
/// preview allows: each run turns exactly what the preview counted, adds exactly its trace and
/// nothing else, every node keeps a way to the exit, and every run ends.
#[test]
fn run_wherever_it_can_it_never_strands_anyone_and_every_run_ends() {
    let d = data();
    let mut s = save(10_000);
    buy_backlink(&d, &mut s);
    let mut ran = 0;
    for tier in 1..=3 {
        let entry = dial_entry(&mut s, tier);
        for seed in 0..40u64 {
            let mut g = s.clone().dial(&d, entry, seed).unwrap();
            let slot = backlink_slot(&g);
            let mut walk = Rng::new(seed ^ 0xBAC);
            let mut steps = 0;
            while matches!(g.phase, Phase::Exploring | Phase::Combat | Phase::Encounter) {
                steps += 1;
                assert!(steps < 4_000, "tier {tier} seed {seed} never ended");
                match g.phase {
                    Phase::Exploring => {
                        if let Ok(p) = g.preview(slot) {
                            let Outcome::Backlink { links, trace } = p.outcome else {
                                panic!("{p:?}")
                            };
                            let before = (g.deck.trace, g.hops, g.at, g.deck.integrity);
                            g.apply(Action::Run(slot));
                            ran += 1;
                            assert_eq!(
                                g.deck.trace.min(tuning::TRACE_MAX),
                                (before.0 + trace).min(tuning::TRACE_MAX)
                            );
                            assert_eq!(
                                (g.hops, g.at, g.deck.integrity),
                                (before.1, before.2, before.3)
                            );
                            assert!(g.net.reaches_exit().iter().all(|&r| r), "no one stranded");
                            assert!(links > 0);
                            continue;
                        }
                        let r = g.reachable();
                        if r.is_empty() {
                            break;
                        }
                        g.apply(Action::Move(r[walk.below(r.len())]));
                    }
                    Phase::Combat => {
                        assert!(g.preview(slot).is_err(), "refused in a fight");
                        g.apply(Action::Finish);
                        if g.phase == Phase::Combat {
                            g.apply(Action::EndTurn);
                        }
                    }
                    Phase::Encounter => {
                        assert!(g.preview(slot).is_err(), "refused while someone talks");
                        let c = g.leave_choice().unwrap();
                        g.apply(Action::Choose {
                            choice: c,
                            slot: None,
                        });
                        g.apply(Action::EndEncounter);
                    }
                    _ => {}
                }
            }
        }
    }
    assert!(ran > 20, "it ran {ran} times");
}

/// In a fight from a dialed run: running it by hand changes nothing but the log, and a script
/// line that runs it is skipped in the preview's words while the pass goes on.
#[test]
fn in_a_fight_it_is_refused_and_its_script_line_skipped() {
    let d = data();
    let mut s = save(10_000);
    buy_backlink(&d, &mut s);
    let entry = dial_entry(&mut s, 1);
    let mut g = (0..200)
        .map(|seed| {
            let mut g = s.clone().dial(&d, entry, seed).unwrap();
            let ice = g
                .reachable()
                .into_iter()
                .find(|&n| matches!(g.net.nodes[n].kind, NodeKind::Ice) && !g.net.nodes[n].cleared);
            if let Some(n) = ice {
                g.apply(Action::Move(n));
            }
            g
        })
        .find(|g| g.phase == Phase::Combat)
        .expect("some seed meets ICE a hop in");
    let slot = backlink_slot(&g);
    let why = g.preview(slot).unwrap_err();
    assert_eq!(
        why,
        "Backlink runs on the map. ICE is holding the connection."
    );
    let before = (
        g.deck.trace,
        format!("{:?}", g.net.nodes),
        g.combat.as_ref().unwrap().cpu,
    );
    g.apply(Action::Run(slot));
    let after = (
        g.deck.trace,
        format!("{:?}", g.net.nodes),
        g.combat.as_ref().unwrap().cpu,
    );
    assert_eq!(before, after);
    assert_eq!(g.log.last().unwrap().text, format!("err: {why}"));
    g.apply(Action::Program("10 RUN backlink\n20 RUN icepick".into()));
    g.apply(Action::Step);
    let said: Vec<String> = g.exec.trace.iter().map(|s| g.step_text(s)).collect();
    assert_eq!(
        said.first().unwrap(),
        &format!("10 RUN backlink: skipped. {why}")
    );
}

/// Hex-edited on a board in a run: a Backlink made to add less trace than its budget allows never takes
/// the slot's place; one made dearer does, and its preview adds the new trace.
#[test]
fn hex_editing_never_makes_it_cheaper_than_its_budget() {
    let d = data();
    let mut s = save(10_000);
    buy_backlink(&d, &mut s);
    let entry = dial_entry(&mut s, 1);
    let mut g = s.clone().dial(&d, entry, 3).unwrap();
    let slot = backlink_slot(&g);
    let fix = |b: &mut binary::Bin| b[binary::OFF_CHECKSUM] = binary::checksum(b);
    let mut cheap = g.hdd[slot].bin;
    cheap[binary::OFF_POWER] -= 1;
    fix(&mut cheap);
    let before = g.hdd[slot].bin;
    g.net.nodes[g.at].kind = NodeKind::Bbs; // the hex editor compiles on a board
    g.apply(Action::Patch { slot, bin: cheap });
    assert_eq!(g.hdd[slot].bin, before, "refused: over budget");
    let said = &g.log.last().unwrap().text;
    assert!(said.contains("over budget"), "{said}");
    let mut dear = before;
    dear[binary::OFF_POWER] += 8;
    fix(&mut dear);
    g.apply(Action::Patch { slot, bin: dear });
    assert_eq!(g.hdd[slot].spec.effect, data::Effect::Backlink(20));
    assert_eq!(g.hdd[slot].bin[binary::OFF_SPARE], 0, "byte 07 stays 00");
}

/// The probe's bots carry Backlink through dialed runs of every tier and finish every one.
#[test]
fn the_bots_finish_every_run_carrying_it() {
    let d = data();
    let mut s = save(10_000);
    buy_backlink(&d, &mut s);
    for tier in 1..=3 {
        let entry = dial_entry(&mut s, tier);
        for seed in 0..30u64 {
            for bot in [Bot::Smart, Bot::SafePick, Bot::Dumb] {
                let g = s.clone().dial(&d, entry, seed).unwrap();
                let (g, log) = probe::play(g, bot);
                assert!(
                    matches!(g.phase, Phase::JackedOut | Phase::Flatlined),
                    "tier {tier} seed {seed} {bot:?}"
                );
                assert_eq!(log.hacks, g.hacks);
                if bot == Bot::Dumb {
                    assert_eq!(g.hacks, 0, "only the careful bots run a hack");
                }
            }
        }
    }
}
