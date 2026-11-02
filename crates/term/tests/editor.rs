//! The line editor: readline keys, the kill ring, history, search and completion.

use ninsei_term::keys::{self, Key, Mods};
use ninsei_term::{Edit, LineEditor, Outcome};

fn none(_: &str) -> Vec<String> {
    Vec::new()
}

fn typed(text: &str) -> LineEditor {
    let mut e = LineEditor::default();
    e.apply(Edit::InsertStr(text.into()), &none);
    e
}

fn run(e: &mut LineEditor, edits: &[Edit]) -> Outcome {
    let mut last = Outcome::Nothing;
    for x in edits {
        last = e.apply(x.clone(), &none);
    }
    last
}

#[test]
fn cursor_moves_by_char_word_and_line() {
    let mut e = typed("jack 12 now");
    assert_eq!(e.view().cursor, 11);
    run(&mut e, &[Edit::WordLeft]);
    assert_eq!(e.view().cursor, 8);
    run(&mut e, &[Edit::WordLeft, Edit::WordLeft]);
    assert_eq!(e.view().cursor, 0);
    run(&mut e, &[Edit::WordRight]);
    assert_eq!(e.view().cursor, 4);
    run(&mut e, &[Edit::End, Edit::Left, Edit::Home, Edit::Right]);
    assert_eq!(e.view().cursor, 1);
    run(&mut e, &[Edit::Insert('X')]);
    assert_eq!(e.text(), "jXack 12 now");
}

#[test]
fn kills_and_yank() {
    let mut e = typed("run 3 now");
    run(&mut e, &[Edit::KillWordBack]);
    assert_eq!(e.text(), "run 3 ");
    run(&mut e, &[Edit::Home, Edit::Yank]);
    assert_eq!(e.text(), "nowrun 3 ");
    run(&mut e, &[Edit::KillToEnd]);
    assert_eq!(e.text(), "now");
    run(&mut e, &[Edit::KillToStart]);
    assert_eq!(e.text(), "");
    run(&mut e, &[Edit::Yank]);
    assert_eq!(e.text(), "now", "Ctrl+U cut what was before the cursor");
    let mut e = typed("jack 12");
    run(&mut e, &[Edit::Home, Edit::KillWordForward]);
    assert_eq!(e.text(), " 12");
    run(&mut e, &[Edit::Backspace, Edit::Delete]);
    assert_eq!(e.text(), "12");
}

#[test]
fn history_walks_back_and_returns_to_the_draft() {
    let mut e = LineEditor::new(3);
    for c in ["jack 1", "run 0", "run 0", "end"] {
        e.set_text(c);
        assert_eq!(e.apply(Edit::Submit, &none), Outcome::Submit(c.into()));
    }
    assert_eq!(
        e.history(),
        ["jack 1", "run 0", "end"],
        "no repeats, capped"
    );
    e.set_text("half");
    run(&mut e, &[Edit::Prev]);
    assert_eq!(e.text(), "end");
    run(&mut e, &[Edit::Prev, Edit::Prev, Edit::Prev]);
    assert_eq!(e.text(), "jack 1", "stops at the oldest");
    run(&mut e, &[Edit::Next]);
    assert_eq!(e.text(), "run 0");
    run(&mut e, &[Edit::Next, Edit::Next]);
    assert_eq!(e.text(), "half", "back to what was being typed");
    e.clear();
    assert_eq!(e.apply(Edit::Submit, &none), Outcome::Submit(String::new()));
    assert_eq!(e.history().len(), 3, "empty lines aren't kept");
}

#[test]
fn reverse_search_finds_and_runs() {
    let mut e = LineEditor::default();
    e.set_history(vec![
        "jack 3".into(),
        "run 1".into(),
        "jack 7".into(),
        "theme amber".into(),
    ]);
    run(
        &mut e,
        &[Edit::SearchBack, Edit::Insert('j'), Edit::Insert('a')],
    );
    let v = e.view();
    assert_eq!(v.text, "jack 7");
    assert!(v.search.unwrap().contains("'ja'"));
    run(&mut e, &[Edit::SearchBack]);
    assert_eq!(e.view().text, "jack 3", "again: the next older match");
    assert_eq!(
        e.apply(Edit::Submit, &none),
        Outcome::Submit("jack 3".into())
    );
    assert!(!e.searching());
    // Esc keeps the match to edit; Ctrl+C drops the search.
    run(
        &mut e,
        &[
            Edit::SearchBack,
            Edit::Insert('t'),
            Edit::Insert('h'),
            Edit::Escape,
        ],
    );
    assert_eq!(e.text(), "theme amber");
    assert!(!e.searching());
    e.set_text("draft");
    run(&mut e, &[Edit::SearchBack, Edit::Insert('r'), Edit::Cancel]);
    assert_eq!(e.text(), "draft");
}

#[test]
fn tab_completes_then_lists() {
    let words = |before: &str| -> Vec<String> {
        if before.contains(' ') {
            vec!["amber".into(), "apple2".into(), "ice".into()]
        } else {
            vec!["jack".into(), "help".into(), "hex".into()]
        }
    };
    let mut e = typed("ja");
    e.apply(Edit::Complete, &words);
    assert_eq!(e.text(), "jack ", "one match: the word and a space");
    e.set_text("theme a");
    e.apply(Edit::Complete, &words);
    assert_eq!(e.text(), "theme a", "no common prefix past 'a' yet");
    assert_eq!(
        e.apply(Edit::Complete, &words),
        Outcome::List(vec!["amber".into(), "apple2".into()]),
        "a second Tab lists"
    );
    e.set_text("h");
    e.apply(Edit::Complete, &words);
    assert_eq!(e.text(), "he", "the common prefix");
    e.set_text("xyz");
    assert_eq!(e.apply(Edit::Complete, &words), Outcome::Nothing);
}

#[test]
fn cancel_and_clear() {
    let mut e = typed("jack 4");
    assert_eq!(
        e.apply(Edit::Cancel, &none),
        Outcome::Cancelled("jack 4".into())
    );
    assert_eq!(e.text(), "");
    assert_eq!(e.apply(Edit::ClearScreen, &none), Outcome::ClearScreen);
    // Pasted newlines and control characters don't sneak in.
    e.apply(Edit::InsertStr("a\nb\u{7}".into()), &none);
    assert_eq!(e.text(), "a b");
}

#[test]
fn the_key_table() {
    let ctrl = |c| keys::bind(Key::Letter(c), Mods::CTRL);
    assert_eq!(ctrl('a'), Some(Edit::Home));
    assert_eq!(ctrl('e'), Some(Edit::End));
    assert_eq!(ctrl('r'), Some(Edit::SearchBack));
    assert_eq!(ctrl('w'), Some(Edit::KillWordBack));
    assert_eq!(ctrl('z'), None);
    assert_eq!(
        keys::bind(Key::Backspace, Mods::ALT),
        Some(Edit::KillWordBack)
    );
    assert_eq!(
        keys::bind(Key::Letter('b'), Mods::ALT),
        Some(Edit::WordLeft)
    );
    assert_eq!(keys::bind(Key::Left, Mods::CTRL), Some(Edit::WordLeft));
    assert_eq!(keys::bind(Key::Up, Mods::NONE), Some(Edit::Prev));
    assert_eq!(keys::bind(Key::Tab, Mods::NONE), Some(Edit::Complete));
    assert_eq!(
        keys::bind(Key::Letter('a'), Mods::NONE),
        None,
        "typing is inserts"
    );
    // Every chord the browser keeps for itself has another key.
    for c in keys::BROWSER_KEEPS {
        if let Some(edit) = ctrl(*c) {
            let other = [
                keys::bind(Key::Backspace, Mods::ALT),
                keys::bind(Key::Down, Mods::NONE),
                keys::bind(Key::Up, Mods::NONE),
            ];
            assert!(
                other.contains(&Some(edit.clone())),
                "{c}: {edit:?} has no fallback"
            );
        }
    }
    // The chords the page blocks are all bound.
    for c in keys::BROWSER_CTRL {
        assert!(ctrl(*c).is_some(), "{c} is blocked but does nothing");
    }
}
