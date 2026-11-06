//! CarrierTerm's directory: who's listed, searching and filtering, and moving the selection.

use ninsei_core::directory::{self, Filter, Listed};
use ninsei_core::hub::Save;
use ninsei_core::*;

/// A fresh deck: Sense/Net, the school net and the starter numbers from the partner directory.
fn deck() -> (Data, Save) {
    let d = Data::builtin().unwrap();
    let mut s = Save::default();
    s.ensure_key(7);
    s.ensure_phonebook(&d, 1);
    s.add_starters(&d);
    (d, s)
}

#[test]
fn boards_come_first_then_the_phonebook() {
    let (d, s) = deck();
    let all = directory::listings(&s, &d);
    let boards = s.known_boards(&d).len();
    assert_eq!(all.len(), boards + s.phonebook.len());
    assert!(boards > 0, "a new deck knows Sense/Net");
    assert!(
        all[..boards]
            .iter()
            .all(|l| matches!(l.key, Listed::Board(_)))
    );
    assert!(
        all[..boards]
            .iter()
            .all(|l| l.listed_as == directory::BOARD_LISTING)
    );
    for (l, e) in all[boards..].iter().zip(&s.phonebook) {
        assert_eq!(l.key, Listed::Entry(e.id));
        assert_eq!(
            (&l.number, &l.name, &l.listed_as),
            (&e.number, &e.name, &e.business)
        );
    }
}

#[test]
fn filters_split_boards_from_businesses() {
    let (d, s) = deck();
    let boards = directory::search(&s, &d, "", Filter::Boards);
    let firms = directory::search(&s, &d, "", Filter::Businesses);
    let all = directory::search(&s, &d, "", Filter::All);
    assert!(boards.iter().all(|l| matches!(l.key, Listed::Board(_))));
    assert!(firms.iter().all(|l| matches!(l.key, Listed::Entry(_))));
    assert_eq!(boards.len() + firms.len(), all.len());
}

#[test]
fn search_finds_names_listings_and_numbers() {
    let (d, s) = deck();
    let find = |q: &str| directory::search(&s, &d, q, Filter::All);
    let school = find("community");
    assert_eq!(school.len(), 1, "names, ignoring case");
    assert_eq!(school[0].name, "Ninsei Community College");
    assert!(
        find("SCHOOL").iter().any(|l| l.name == school[0].name),
        "listings"
    );
    for q in [
        "555-0420",
        "0420",
        "5550420",
        " 0420 ",
        "555 0420",
        "(555) 0420",
    ] {
        assert!(
            find(q).iter().any(|l| l.number == "555-0420"),
            "numbers, however they're punctuated: {q:?}"
        );
    }
    for q in ["-", "--", "()"] {
        assert!(
            find(q)
                .iter()
                .all(|l| l.name.contains(q) || l.listed_as.contains(q)),
            "punctuation alone isn't a number: {q:?}"
        );
    }
    assert!(find("zzz no such thing").is_empty());
    assert_eq!(find("").len(), directory::listings(&s, &d).len());
    assert!(
        directory::search(&s, &d, "community", Filter::Boards).is_empty(),
        "the filter still applies"
    );
}

#[test]
fn arrows_move_the_selection_and_stop_at_the_ends() {
    let (d, s) = deck();
    let shown = directory::listings(&s, &d);
    let key = |i: usize| shown[i].key;
    assert_eq!(
        directory::step(&shown, None, 1),
        Some(key(0)),
        "nothing picked yet"
    );
    assert_eq!(directory::step(&shown, Some(key(0)), 1), Some(key(1)));
    assert_eq!(directory::step(&shown, Some(key(1)), -1), Some(key(0)));
    assert_eq!(directory::step(&shown, Some(key(0)), -1), Some(key(0)));
    let last = shown.len() - 1;
    assert_eq!(directory::step(&shown, Some(key(last)), 1), Some(key(last)));
    assert_eq!(
        directory::step(&shown, Some(Listed::Entry(9999)), 1),
        Some(key(0)),
        "a selection that's filtered out starts over"
    );
    assert_eq!(directory::step(&[], Some(key(0)), 1), None);
}
