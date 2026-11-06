//! NiGHTCOMM's TERM: Hayes commands, and dialing one number by hand.

use ninsei_core::at::{self, Command, Effect};
use ninsei_core::hub::{Answer, Found, Save};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

fn deck(d: &Data) -> Save {
    let mut s = Save::default();
    s.ensure_key(0xA7D7);
    s.ensure_phonebook(d, 1);
    s.add_starters(d);
    s.learn_tool("nightcomm");
    s.bank = 100_000;
    s
}

#[test]
fn commands_parse_like_a_hayes_modem() {
    let dial = |n: &str, force| Command::Dial {
        number: n.into(),
        force,
    };
    assert_eq!(at::parse("atdt 555-0420"), dial("555-0420", false));
    assert_eq!(at::parse("ATDT5550420"), dial("555-0420", false));
    assert_eq!(at::parse("ATD 0420"), dial("555-0420", false));
    assert_eq!(at::parse("ATDT 555-0911!"), dial("555-0911", true));
    assert_eq!(at::parse("ATDS2"), Command::Stored(2));
    assert_eq!(at::parse("ATDS0"), Command::Unknown);
    assert_eq!(at::parse("ATZ"), Command::Reset);
    assert_eq!(at::parse("ati"), Command::Info);
    assert_eq!(at::parse("ATH"), Command::Hang);
    assert_eq!(at::parse("ATM0"), Command::Speaker(false));
    assert_eq!(at::parse("ATM1"), Command::Speaker(true));
    assert_eq!(at::parse("+++"), Command::Escape);
    assert_eq!(at::parse("   "), Command::Empty);
    assert_eq!(at::parse("hello"), Command::Unknown);
    assert_eq!(at::parse("ATDT 12"), Command::Unknown);
    assert_eq!(at::normalize("(555) 0420").as_deref(), Some("555-0420"));
    assert_eq!(at::normalize("555-04x0"), None);
}

#[test]
fn known_numbers_answer_as_you_know_them() {
    let d = data();
    let mut s = deck(&d);
    let school = s
        .phonebook
        .iter()
        .find(|e| e.number == "555-0420")
        .unwrap()
        .id;
    let r = at::run(&mut s, &d, "ATDT 555-0420", None);
    assert_eq!(r.effect, Effect::Ask(school));
    assert!(r.lines.last().unwrap().contains("jack in? [Y/n]"));
    assert_eq!(
        at::run(&mut s, &d, "", Some(school)).effect,
        Effect::Dial(school)
    );
    assert_eq!(
        at::run(&mut s, &d, "y", Some(school)).effect,
        Effect::Dial(school)
    );
    let no = at::run(&mut s, &d, "n", Some(school));
    assert_eq!(
        (no.effect, no.lines),
        (Effect::None, vec!["NO CARRIER".to_string()])
    );
    // Sense/Net is a board you know: TERM calls it.
    let sn = d.board(hub::FIRST_BOARD).unwrap();
    let r = at::run(&mut s, &d, &format!("ATDT {}", d.boards[sn].phone), None);
    assert_eq!(r.effect, Effect::Call(sn));
    // Stored numbers dial by position.
    assert_eq!(
        at::run(&mut s, &d, "ATDS1", None).effect,
        Effect::Ask(s.phonebook[0].id)
    );
    assert_eq!(at::run(&mut s, &d, "ATDS99", None).lines, vec!["ERROR"]);
    assert_eq!(at::run(&mut s, &d, "ATH", None).effect, Effect::Hang);
    assert_eq!(
        at::run(&mut s, &d, "ATM0", None).effect,
        Effect::Speaker(false)
    );
    assert_eq!(
        at::run(&mut s, &d, "ATI", None).lines,
        vec![at::IDENT, "OK"]
    );
}

#[test]
fn a_number_answers_the_same_by_hand_or_by_scan() {
    let d = data();
    let s = deck(&d);
    let local = d
        .exchanges
        .iter()
        .find(|r| r.id == tuning::LOCAL_RANGE)
        .unwrap();
    // Scan block 3 on a copy, then dial each of its numbers by hand on another.
    let mut scanned = s.clone();
    scanned.scanned = (0..local.blocks())
        .filter(|b| *b != 3)
        .map(|b| format!("{}:{b}", local.id))
        .collect();
    let res = scanned.wardial(&d, &local.id).unwrap();
    assert_eq!(res.block, 3);
    let mut by_hand = s.clone();
    for (number, answer) in &res.cells {
        let got = by_hand.atdt(&d, number).unwrap();
        let same = match (answer, &got) {
            (Answer::Carrier { .. }, Answer::Carrier { .. }) => true,
            (Answer::Board { board: a, .. }, Answer::Board { board: b, .. }) => a == b,
            (a, b) => a == b,
        };
        assert!(same, "{number}: scan {answer:?}, by hand {got:?}");
    }
    let price = Save::call_price(local) * res.cells.len() as u32;
    assert_eq!(by_hand.bank, s.bank - price, "each call is billed");
}

#[test]
fn watched_lines_are_refused_unless_forced() {
    let d = data();
    let mut s = deck(&d);
    // Scan until a watched line turns up.
    while !s.finds.iter().any(|f| f.what == Found::Watched) {
        s.wardial(&d, tuning::LOCAL_RANGE).unwrap();
    }
    let n = s
        .finds
        .iter()
        .find(|f| f.what == Found::Watched)
        .unwrap()
        .number
        .clone();
    let heat = s.telco_heat;
    let r = at::run(&mut s, &d, &format!("ATDT {n}"), None);
    assert!(r.lines.iter().any(|l| l.contains("watched line")));
    assert_eq!(s.telco_heat, heat, "refused: no call made");
    let r = at::run(&mut s, &d, &format!("ATDT {n}!"), None);
    assert!(r.lines.iter().any(|l| l.contains("telco is listening")));
    assert_eq!(s.telco_heat, heat + 1);
}

#[test]
fn new_numbers_go_in_the_phonebook_and_cost_money() {
    let d = data();
    let mut s = deck(&d);
    let local = d
        .exchanges
        .iter()
        .find(|r| r.id == tuning::LOCAL_RANGE)
        .unwrap();
    let before = s.phonebook.len();
    let mut found = None;
    'blocks: for b in 0..local.blocks() {
        for n in local.numbers(b) {
            if s.phonebook.iter().any(|e| e.number == n) || d.boards.iter().any(|x| x.phone == n) {
                continue;
            }
            if let Ok(Answer::Carrier { entry, new: true }) = s.atdt(&d, &n) {
                found = Some((n, entry));
                break 'blocks;
            }
        }
    }
    let (number, entry) = found.expect("a carrier somewhere on the exchange");
    assert_eq!(s.phonebook.len(), before + 1);
    assert_eq!(s.entry(entry).unwrap().number, number);
    assert!(s.finds.iter().any(|f| f.number == number));
    s.bank = 0;
    assert!(s.atdt(&d, "555-0001").is_err(), "no money, no call");
    let mut bare = deck(&d);
    bare.tools.clear();
    assert!(bare.atdt(&d, "555-0001").is_err(), "TERM is NiGHTCOMM's");
}

#[test]
fn tab_completes_commands_and_numbers() {
    let d = data();
    let s = deck(&d);
    assert!(at::complete("atd", &s).contains(&"ATDT".to_string()));
    let nums = at::complete("ATDT 555-04", &s);
    assert!(nums.contains(&"555-0420".to_string()), "{nums:?}");
}
