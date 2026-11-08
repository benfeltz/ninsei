//! 141.4 acceptance: the Sense/Net's Script card, played through the real save and run.

use ninsei_core::game::ExecState;
use ninsei_core::hub::Save;
use ninsei_core::stack_fight::{self, BlockKind, Field};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

/// A fresh deck's run, in a fight with ICE too strong to fall.
fn fight() -> (Save, Game) {
    let d = data();
    let mut s = Save::default();
    s.ensure_key(0x5AFE);
    s.ensure_phonebook(&d, 0);
    s.ensure_hardware(&d);
    let mut g = s.jack_in(&d, 21);
    let target = g.net.nodes[g.at].links[0];
    g.net.nodes[target].kind = NodeKind::Ice;
    g.net.nodes[target].ice = Some(0);
    g.net.nodes[target].cleared = false;
    g.apply(Action::Move(target));
    let c = g.combat.as_mut().unwrap();
    (c.strength, c.strength_max) = (500, 500);
    (s, g)
}

#[test]
fn safe_shows_as_blocks_with_menus_of_the_disk() {
    let (_, g) = fight();
    let card = stack_fight::card(&g);
    assert_eq!(card.blocks.len(), g.program.lines.len(), "one block a line");
    assert!(card.stray.is_empty());
    let disk = stack_fight::disk(&g);
    for b in &card.blocks {
        assert!(b.fault.is_none(), "SAFE reads: line {}", b.no);
        for f in &b.fields {
            if let Field::Menu { options, value } = f
                && b.kind == BlockKind::Run
            {
                assert_eq!(options, &disk, "line {}: the menu is the disk", b.no);
                assert!(!value.is_empty());
            }
        }
    }
    assert_eq!(card.blocks.iter().filter(|b| b.current).count(), 1);
    assert!(
        card.blocks[1].current,
        "the cursor on the first line that runs, past SAFE's REM"
    );
}

#[test]
fn every_field_of_every_block_edits_one_line_into_a_program_that_reads() {
    let (_, g) = fight();
    let card = stack_fight::card(&g);
    for b in &card.blocks {
        for (ix, f) in b.fields.iter().enumerate() {
            let values: Vec<String> = match f {
                Field::Menu { options, .. } => options.clone(),
                Field::Number(n) => vec![(n + 1).to_string()],
            };
            for v in values {
                let Some(a) = stack_fight::set_field(&g, b.no, ix, &v) else {
                    continue;
                };
                let mut after = g.clone();
                after.apply(a);
                let changed: Vec<u32> = g
                    .program
                    .lines
                    .iter()
                    .filter(|l| after.program.line(l.no).map(|m| &m.text) != Some(&l.text))
                    .map(|l| l.no)
                    .collect();
                assert_eq!(changed, [b.no], "{} field {ix} = {v}", b.no);
                assert_eq!(after.program.lines.len(), g.program.lines.len());
                let fault = after.program.fault(b.no);
                // A GOTO to a number that isn't a line is the only fault an edit can make.
                assert!(
                    fault.is_none() || b.kind == BlockKind::Goto,
                    "{} field {ix} = {v}: {fault:?}",
                    b.no
                );
            }
        }
    }
}

#[test]
fn the_buttons_play_the_fight_and_the_card_follows() {
    let (mut s, mut g) = fight();
    let d = data();
    // SAFE only defends: the player's first edit puts a breaker in its loop.
    let icepick = g.hdd[g.slot_named("icepick").unwrap()].spec.name.clone();
    let looped = format!("{}\n55 RUN {icepick}", g.program.text());
    g.apply(Action::Program(looped));
    // A STOP block added from the palette, after SAFE's first IF.
    let first_if = stack_fight::card(&g)
        .blocks
        .iter()
        .find(|b| b.kind == BlockKind::If)
        .map(|b| b.no)
        .unwrap();
    g.apply(stack_fight::insert_before(&g, first_if, BlockKind::Stop).unwrap());
    let b = stack_fight::buttons(&g);
    assert!(b.run && b.step && !b.pause);
    g.apply(Action::Play);
    assert!(
        matches!(g.exec.state, ExecState::Stopped(_)),
        "Run stopped at the STOP block"
    );
    let card = stack_fight::card(&g);
    assert!(card.buttons.run && !card.buttons.pause);
    assert!(
        card.blocks
            .iter()
            .any(|b| b.current && b.no > first_if - 10)
    );
    g.apply(Action::Play);
    assert_eq!(g.exec.state, ExecState::Running);
    assert!(stack_fight::buttons(&g).pause);
    g.apply(Action::Pause);
    assert_eq!(g.exec.state, ExecState::Paused);
    // What the card made is what the deck keeps.
    let edited = g.program.text();
    g.phase = Phase::JackedOut;
    s.record(&g, &[], &d.rig).unwrap();
    assert_eq!(s.program_text(&d), edited);
}

#[test]
fn the_deck_card_words_the_limits_from_the_deck() {
    let (_, mut g) = fight();
    let b = stack_fight::budgets(&g);
    assert_eq!(b.cpu_words(), format!("{} cycles/turn", g.deck.cpu_max));
    assert_eq!(b.ifs_words(), format!("{} IFs", g.deck.ram_max));
    assert_eq!(
        b.hdd_words(),
        format!("{} of {}", g.hdd.len(), g.deck.hdd_max)
    );
    // A true IF, so the STOP it guards runs and the turn waits.
    g.apply(Action::Program("10 IF TURN < 9\n20 STOP".into()));
    g.apply(Action::Finish);
    let b = stack_fight::budgets(&g);
    assert_eq!(b.ifs.0, g.deck.ram_max - 1, "the IF spent a GB");
    assert!(b.left_words().starts_with(&format!(
        "{} of {} IFs",
        g.deck.ram_max - 1,
        g.deck.ram_max
    )));
}

#[test]
fn outside_a_fight_the_card_shows_the_program_and_wont_play() {
    let d = data();
    let mut s = Save::default();
    s.ensure_key(0x5AFE);
    s.ensure_phonebook(&d, 0);
    s.ensure_hardware(&d);
    let g = s.jack_in(&d, 21);
    assert_ne!(g.phase, Phase::Combat);
    let card = stack_fight::card(&g);
    assert_eq!(
        card.blocks.len(),
        g.program.lines.len(),
        "the program is there"
    );
    assert!(!card.fighting);
    let b = card.buttons;
    assert!(!b.step && !b.run && !b.pause, "nothing to play against");
    assert!(
        card.blocks.iter().all(|b| !b.current),
        "no line about to run"
    );
    // The deck's limits a turn, not a countdown.
    assert_eq!(card.budget_words(), "6 IFs, 3 cycles/turn");
    let (_, g) = fight();
    let card = stack_fight::card(&g);
    assert!(card.fighting);
    assert_eq!(card.budget_words(), card.budgets.left_words());
}

#[test]
fn a_moved_block_lands_on_its_neighbours_number() {
    let (_, mut g) = fight();
    g.apply(Action::Program(
        "10 RUN icepick\n20 STOP\n30 GOTO 10".into(),
    ));
    assert_eq!(stack_fight::moved_to(&g, 20, true), Some(10));
    assert_eq!(stack_fight::moved_to(&g, 20, false), Some(30));
    assert_eq!(stack_fight::moved_to(&g, 10, true), None, "the top");
    assert_eq!(stack_fight::moved_to(&g, 30, false), None, "the bottom");
    g.apply(stack_fight::swap(&g, 20, true).unwrap());
    assert_eq!(
        g.program.line(10).unwrap().text,
        "STOP",
        "the STOP went where it said"
    );
}

#[test]
fn the_script_is_one_program_before_during_and_after_a_fight() {
    let d = data();
    let mut s = Save::default();
    s.ensure_key(0x5AFE);
    s.ensure_phonebook(&d, 0);
    s.ensure_hardware(&d);
    let mut g = s.jack_in(&d, 21);
    // Between fights: the card is there, greyed, and it edits.
    let card = stack_fight::card(&g);
    assert!(!card.fighting && !card.buttons.run && !card.buttons.step);
    g.apply(stack_fight::add(&g, BlockKind::Stop).unwrap());
    let edited = g.program.text();
    assert!(edited.ends_with("STOP"), "{edited}");
    // Into ICE: the same program, the cursor on its first line, the buttons live.
    let target = g.net.nodes[g.at].links[0];
    g.net.nodes[target].kind = NodeKind::Ice;
    g.net.nodes[target].ice = Some(0);
    g.net.nodes[target].cleared = false;
    g.apply(Action::Move(target));
    assert_eq!(g.phase, Phase::Combat);
    let card = stack_fight::card(&g);
    assert!(card.fighting && card.buttons.run && card.buttons.step);
    assert_eq!(
        g.program.text(),
        edited,
        "the fight opens with what was edited"
    );
    assert!(
        card.blocks[1].current,
        "on the first line that runs, past SAFE's REM"
    );
    assert!(card.blocks.iter().any(|b| b.kind == BlockKind::Stop));
    // And the deck keeps it.
    g.phase = Phase::JackedOut;
    s.record(&g, &[], &d.rig).unwrap();
    assert_eq!(s.program_text(&d), edited);
}
