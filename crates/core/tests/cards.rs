use ninsei_core::cards::{self, Place, TEXT_COLS, TEXT_ROWS};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

#[test]
fn welcome_covers_the_world_and_the_game() {
    let d = data();
    let s = &d.welcome;
    let names: Vec<&str> = s.sections.iter().map(|x| x.name.as_str()).collect();
    for want in [
        "The Net",
        "Dialing in",
        "A run",
        "ICE",
        "Boards & work",
        "Your deck",
    ] {
        assert!(names.contains(&want), "missing section {want}");
    }
    for (_, c) in s.cards() {
        assert!(c.text.len() <= TEXT_ROWS, "{}", c.title);
        for l in &c.text {
            let f = cards::fill(l, &d);
            assert!(f.chars().count() <= TEXT_COLS, "{}: {f}", c.title);
            assert!(!f.contains('{'), "{}: unfilled {f}", c.title);
        }
    }
}

#[test]
fn facts_come_from_the_rules() {
    let d = data();
    let all: String = d
        .welcome
        .cards()
        .iter()
        .flat_map(|(_, c)| c.text.iter().map(|l| cards::fill(l, &d)))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(all.contains(&format!("At {}%", tuning::TRACE_MAX)));
    assert!(all.contains(&format!("{} CPU", tuning::START_CPU)));
    assert!(all.contains(&d.boards[0].phone));
    assert_eq!(cards::fill("{nope} x", &d), "{nope} x");
}

#[test]
fn paging_runs_home_to_the_end_and_back() {
    let d = data();
    let s = &d.welcome;
    let n = s.cards().len();
    let mut p = Place::Home;
    for i in 0..n {
        p = p.next(s);
        assert_eq!(p, Place::Card(i));
    }
    assert_eq!(p.next(s), Place::Card(n - 1), "the last card stays");
    for _ in 0..n {
        p = p.prev();
    }
    assert_eq!(p, Place::Home);
    assert_eq!(s.section_start(1), s.sections[0].cards.len());
}

#[test]
fn a_bad_card_is_caught() {
    let d = data();
    let mut s = d.welcome.clone();
    s.sections[0].cards[0].text.push("x".repeat(TEXT_COLS + 1));
    assert!(s.validate(&d).unwrap_err().contains("wide"));
    let mut s = d.welcome.clone();
    s.sections[0].cards[0].text.push("{mystery}".into());
    assert!(s.validate(&d).unwrap_err().contains("placeholder"));
    let mut s = d.welcome.clone();
    s.sections[0].cards[0].picture = Some("nope".into());
    assert!(s.validate(&d).unwrap_err().contains("unknown picture"));
}
