//! The Sense/Net's Script card: what it shows is decided in core, and every edit rewrites one
//! line of the deck's program.

use ninsei_core::game::ExecState;
use ninsei_core::stack_fight::{self, BlockKind, Field};
use ninsei_core::*;

/// A bare run (icepick, firewall, patch, decoder on the disk) in a fight with ICE too strong to
/// fall, running `program`.
fn fight(program: &str) -> Game {
    let mut g = Game::new(Data::builtin().unwrap(), 7);
    let target = g.net.nodes[g.at].links[0];
    g.net.nodes[target].kind = NodeKind::Ice;
    g.net.nodes[target].ice = Some(1);
    g.net.nodes[target].cleared = false;
    g.apply(Action::Move(target));
    let c = g.combat.as_mut().unwrap();
    (c.strength, c.strength_max) = (500, 500);
    g.apply(Action::Program(program.into()));
    g
}

fn edit(g: &mut Game, f: impl FnOnce(&Game) -> Option<Action>) {
    let a = f(g).expect("an edit");
    g.apply(a);
}

const SAMPLE: &str = "10 RUN icepick\n20 IF INTEG < 10\n30 RUN patch\n40 GOTO 10\n50 STOP";

#[test]
fn the_card_mirrors_the_program() {
    let g = fight(&format!(
        "5 REM Pick, patch, repeat.\n{SAMPLE}\n60 JUMP\n70 IF HAVE torch"
    ));
    let c = stack_fight::card(&g);
    let kinds: Vec<BlockKind> = c.blocks.iter().map(|b| b.kind).collect();
    use BlockKind::*;
    assert_eq!(kinds, [Rem, Run, If, Run, Goto, Stop, Bad, If]);
    assert_eq!(c.blocks[0].text, "Pick, patch, repeat.");
    // A RUN's menu shows the disk's own name and offers only the disk.
    let Field::Menu { value, options } = &c.blocks[1].fields[0] else {
        panic!()
    };
    assert_eq!(value, "IcePick");
    assert_eq!(options, &["IcePick", "Firewall", "Patch", "Decoder"]);
    assert_eq!(c.blocks[1].caption, Some("program"));
    // IF INTEG < 10: subject, comparison, number.
    let vals: Vec<String> = c.blocks[2].fields.iter().map(Field::value).collect();
    assert_eq!(vals, ["INTEG", "<", "10"]);
    assert_eq!(c.blocks[4].fields, [Field::Number(10)]);
    // A program not on the disk keeps its name; the menu still lists only the disk.
    let Field::Menu { value, options } = &c.blocks[7].fields[1] else {
        panic!()
    };
    assert_eq!(value, "torch");
    assert!(!options.contains(&"torch".to_string()));
    assert!(c.blocks[6].fault.is_some(), "the bad line names its fault");
    // The ▶ is on the line about to run, and only there.
    let current: Vec<u32> = c
        .blocks
        .iter()
        .filter(|b| b.current)
        .map(|b| b.no)
        .collect();
    assert_eq!(current, [5]);
}

#[test]
fn buttons_follow_the_state() {
    let mut g = fight(SAMPLE);
    let b = stack_fight::card(&g).buttons;
    assert!(b.step && b.run && !b.pause);
    g.apply(Action::Play);
    assert_eq!(g.exec.state, ExecState::Running);
    let b = stack_fight::card(&g).buttons;
    assert!(!b.step && !b.run && b.pause);
    let calm = Game::new(Data::builtin().unwrap(), 7);
    let b = stack_fight::card(&calm).buttons;
    assert!(!b.step && !b.run && !b.pause, "no fight, nothing to press");
}

#[test]
fn the_deck_card_words_the_limits() {
    let g = fight(SAMPLE);
    let b = stack_fight::card(&g).budgets;
    assert_eq!(b.cpu_words(), "3 cycles/turn");
    assert_eq!(b.ifs_words(), "6 IFs");
    assert_eq!(b.hdd_words(), "4 of 6");
    assert_eq!(b.left_words(), "6 of 6 IFs, 3 cycles left");
}

#[test]
fn adding_a_block_takes_the_next_number_and_a_real_program() {
    let mut g = fight(SAMPLE);
    g.apply(stack_fight::add(&g, BlockKind::Run).unwrap());
    assert_eq!(g.program.line(60).unwrap().text, "RUN IcePick");
    g.apply(stack_fight::add(&g, BlockKind::If).unwrap());
    assert_eq!(g.program.line(70).unwrap().text, "IF INTEG < 10");
    g.apply(stack_fight::add(&g, BlockKind::Goto).unwrap());
    assert_eq!(g.program.line(80).unwrap().text, "GOTO 10");
    // Dropped before a line: numbered between it and the one above.
    g.apply(stack_fight::insert_before(&g, 20, BlockKind::Stop).unwrap());
    assert_eq!(g.program.line(15).unwrap().text, "STOP");
    // No number free between 15 and 16: it goes to the end.
    g.apply(Action::Program("15 STOP\n16 STOP".into()));
    g.apply(stack_fight::insert_before(&g, 16, BlockKind::Run).unwrap());
    assert_eq!(g.program.text(), "15 STOP\n16 STOP\n20 RUN IcePick");
    let empty = fight("");
    let mut e = empty.clone();
    e.apply(stack_fight::add(&empty, BlockKind::Stop).unwrap());
    assert_eq!(e.program.text(), "10 STOP");
    // The last number taken: nothing is added, and nothing is overwritten.
    let full = fight("63999 RUN icepick");
    assert_eq!(stack_fight::add(&full, BlockKind::Stop), None);
    assert_eq!(
        stack_fight::insert_before(&full, 63999, BlockKind::Stop).map(|_| ()),
        Some(())
    );
    let mut packed = fight("63998 RUN icepick\n63999 RUN patch");
    assert_eq!(
        stack_fight::insert_before(&packed, 63999, BlockKind::Stop),
        None
    );
    packed.apply(Action::Program("63990 STOP".into()));
    packed.apply(stack_fight::add(&packed, BlockKind::Stop).unwrap());
    assert_eq!(packed.program.text(), "63990 STOP\n63999 STOP");
}

#[test]
fn editing_a_field_rewrites_one_line() {
    let mut g = fight(SAMPLE);
    edit(&mut g, |g| stack_fight::set_field(g, 10, 0, "Decoder"));
    edit(&mut g, |g| stack_fight::set_field(g, 20, 2, "7"));
    edit(&mut g, |g| stack_fight::set_field(g, 20, 1, ">"));
    edit(&mut g, |g| stack_fight::set_field(g, 40, 0, "30"));
    assert_eq!(
        g.program.text(),
        "10 RUN Decoder\n20 IF INTEG > 7\n30 RUN patch\n40 GOTO 30\n50 STOP"
    );
    // A new subject starts the IF's other fields afresh.
    edit(&mut g, |g| stack_fight::set_field(g, 20, 0, "NEXT"));
    assert_eq!(g.program.line(20).unwrap().text, "IF NEXT ATTACK");
    edit(&mut g, |g| stack_fight::set_field(g, 20, 1, "FORTIFY"));
    edit(&mut g, |g| stack_fight::set_field(g, 30, 0, "Firewall"));
    edit(&mut g, |g| stack_fight::set_field(g, 20, 0, "ICE CLASS"));
    assert_eq!(g.program.line(20).unwrap().text, "IF ICE BARRIER");
    edit(&mut g, |g| stack_fight::set_field(g, 20, 0, "HAVE"));
    assert_eq!(g.program.line(20).unwrap().text, "IF HAVE IcePick");
    // Every line still reads.
    assert!(
        g.program
            .lines
            .iter()
            .all(|l| g.program.fault(l.no).is_none())
    );
    // Nothing that wouldn't change a line, and nothing that isn't a number.
    assert_eq!(stack_fight::set_field(&g, 30, 0, "Firewall"), None);
    assert_eq!(stack_fight::set_field(&g, 40, 0, "ten"), None);
    // Picking the subject it already has keeps the rest of the IF.
    g.apply(Action::Program("20 IF INTEG < 3".into()));
    assert_eq!(stack_fight::set_field(&g, 20, 0, "INTEG"), None);
}

#[test]
fn blocks_reorder_and_delete() {
    let mut g = fight(SAMPLE);
    edit(&mut g, |g| stack_fight::swap(g, 30, true));
    assert_eq!(g.program.line(20).unwrap().text, "RUN patch");
    assert_eq!(g.program.line(30).unwrap().text, "IF INTEG < 10");
    assert_eq!(stack_fight::swap(&g, 10, true), None, "the top can't go up");
    assert_eq!(stack_fight::swap(&g, 50, false), None);
    g.apply(stack_fight::remove(&g, 50));
    assert!(g.program.line(50).is_none());
}

#[test]
fn the_card_is_a_view_of_the_running_program() {
    let mut g = fight(SAMPLE);
    g.apply(Action::Program(
        "10 RUN icepick\n20 STOP\n30 RUN icepick".into(),
    ));
    g.apply(Action::Finish);
    let c = stack_fight::card(&g);
    assert_eq!(c.state, ExecState::Stopped(20));
    let current: Vec<u32> = c
        .blocks
        .iter()
        .filter(|b| b.current)
        .map(|b| b.no)
        .collect();
    assert_eq!(current, [30], "the ▶ moved past the STOP");
    assert_eq!(c.budgets.cpu.0, 2);
}
