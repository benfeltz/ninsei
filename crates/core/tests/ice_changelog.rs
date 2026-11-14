//! 141.9.3, made strict by 141.11.1: the ICE DB is a changelog of the sources you own. A kind's
//! page lists the newest source you own, marked against the one before it you own, and keeps the
//! fight record of every version you met; a version you met without its source has its number
//! and record and no code.

use ninsei_core::hub::{IceSeen, IceSource, Save, SeenVersion};
use ninsei_core::ice_listing::{Mark, changes};
use ninsei_core::ice_version::{Bump, Version};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

const V100: &str = "10 IF TARGET SHIELD > 0\n20 TRACE 10\n30 ATTACK 4\n40 TRACE 4";
/// 1.0.0 with ATTACK 4 moved to the top.
const V101: &str = "10 ATTACK 4\n20 IF TARGET SHIELD > 0\n30 TRACE 10\n40 TRACE 4";
/// 1.0.1 with the last TRACE 4 changed to TRACE 5.
const V102: &str = "10 ATTACK 4\n20 IF TARGET SHIELD > 0\n30 TRACE 10\n40 TRACE 5";

fn enigma(d: &Data) -> &IceDef {
    d.ice.iter().find(|i| i.id == "enigma").unwrap()
}

fn marks(rows: &[ice_listing::MarkedRow]) -> Vec<String> {
    rows.iter()
        .map(|r| format!("{}{} {}", r.mark.glyph(), r.no, r.text))
        .collect()
}

#[test]
fn a_moved_block_is_marked_moved_and_only_it() {
    let d = data();
    let v = &enigma(&d).verbs;
    let rows = changes(v, Some(V100), V101);
    assert_eq!(
        marks(&rows),
        [
            "~10 ATTACK 4",
            " 20 IF TARGET SHIELD > 0",
            " 30 TRACE 10",
            " 40 TRACE 4"
        ]
    );
    // A number changed is a new line, and the old one is gone.
    let rows = changes(v, Some(V101), V102);
    assert_eq!(
        marks(&rows),
        [
            " 10 ATTACK 4",
            " 20 IF TARGET SHIELD > 0",
            " 30 TRACE 10",
            "+40 TRACE 5",
            "-40 TRACE 4"
        ]
    );
    // The oldest source has nothing to be marked against.
    let rows = changes(v, None, V100);
    assert!(rows.iter().all(|r| r.mark == Mark::None));
    assert_eq!(rows.len(), 4);
}

#[test]
fn an_added_a_removed_and_a_replaced_read_are_marked() {
    let d = data();
    let v = &enigma(&d).verbs;
    // A minor adds a read and its response.
    let added = "10 IF TARGET SHIELD > 0\n20 TRACE 10\n30 IF TARGET INTEG < 10\n40 ATTACK 5\n50 ATTACK 4\n60 TRACE 4";
    assert_eq!(
        marks(&changes(v, Some(V100), added)),
        [
            " 10 IF TARGET SHIELD > 0",
            " 20 TRACE 10",
            "+30 IF TARGET INTEG < 10",
            "+40 ATTACK 5",
            " 50 ATTACK 4",
            " 60 TRACE 4"
        ]
    );
    // Removing it again: its lines are gone, marked as they were.
    assert_eq!(
        marks(&changes(v, Some(added), V100)),
        [
            " 10 IF TARGET SHIELD > 0",
            " 20 TRACE 10",
            " 30 ATTACK 4",
            " 40 TRACE 4",
            "-30 IF TARGET INTEG < 10",
            "-40 ATTACK 5"
        ]
    );
    // A major replaces the read: the block is new, the old one gone.
    let replaced = "10 IF TARGET RAN patch\n20 TRACE 10\n30 ATTACK 4\n40 TRACE 4";
    assert_eq!(
        marks(&changes(v, Some(V100), replaced)),
        [
            "+10 IF TARGET RAN patch",
            "+20 TRACE 10",
            " 30 ATTACK 4",
            " 40 TRACE 4",
            "-10 IF TARGET SHIELD > 0",
            "-20 TRACE 10"
        ]
    );
}

#[test]
fn a_changed_number_is_new_even_when_the_same_line_is_elsewhere() {
    // 30 TRACE 10 became 30 TRACE 11; TRACE 11 was already at 10 and 40, and still is.
    let d = data();
    let v = &enigma(&d).verbs;
    let before = "10 TRACE 11\n20 IF TARGET SHIELD > 0\n30 TRACE 10\n40 TRACE 11";
    let after = "10 TRACE 11\n20 IF TARGET SHIELD > 0\n30 TRACE 11\n40 TRACE 11";
    assert_eq!(
        marks(&changes(v, Some(before), after)),
        [
            " 10 TRACE 11",
            " 20 IF TARGET SHIELD > 0",
            "+30 TRACE 11",
            " 40 TRACE 11",
            "-30 TRACE 10"
        ]
    );
}

#[test]
fn a_moved_block_is_marked_whole() {
    // IF A / X / IF B / X, then the second block moved to the top.
    let d = data();
    let v = &enigma(&d).verbs;
    let before = "10 IF TARGET SHIELD > 0\n20 TRACE 4\n30 IF TURN > 2\n40 TRACE 4";
    let after = "10 IF TURN > 2\n20 TRACE 4\n30 IF TARGET SHIELD > 0\n40 TRACE 4";
    let rows = changes(v, Some(before), after);
    let m: Vec<Mark> = rows.iter().map(|r| r.mark).collect();
    // One block kept, the other moved, each whole: never an IF moved over a kept line.
    assert!(
        m == [Mark::Moved, Mark::Moved, Mark::None, Mark::None]
            || m == [Mark::None, Mark::None, Mark::Moved, Mark::Moved],
        "{:?}",
        marks(&rows)
    );
}

fn source(version: Version, program: &str) -> IceSource {
    IceSource {
        id: "enigma".into(),
        version,
        program: program.into(),
    }
}

fn v101() -> Version {
    Version::default().bumped(Bump::Patch)
}

/// A save that met Enigma at 1.0.0 (won 2, lost 1), then at 1.0.1, a patch (won 1), owning
/// the sources of `owned`.
fn met_twice(owned: &[Version]) -> Save {
    let mut s = Save::default();
    s.ice_seen.push(IceSeen {
        id: "enigma".into(),
        met: 4,
        broken: 3,
        lo: 12,
        hi: 12,
        version: v101(),
        bump: Some(Bump::Patch),
        program: V101.into(),
        won: 1,
        lost: 0,
        past: vec![SeenVersion {
            version: Version::default(),
            bump: None,
            program: String::new(),
            won: 2,
            lost: 1,
        }],
    });
    for v in owned {
        let program = if *v == Version::FIRST { "" } else { V101 };
        s.ice_sources.push(source(*v, program));
    }
    s
}

fn entry<'a>(s: &'a Save, d: &'a Data) -> icedb::Entry<'a> {
    icedb::entries(s, d)
        .into_iter()
        .find(|e| e.def.id == "enigma")
        .unwrap()
}

fn page(s: &Save, d: &Data) -> Vec<String> {
    entry(s, d).lines(s, d)
}

fn at(p: &[String], t: &str) -> usize {
    p.iter()
        .position(|l| l.contains(t))
        .unwrap_or_else(|| panic!("{t}: {p:#?}"))
}

#[test]
fn a_version_met_without_its_source_has_its_record_and_no_code() {
    let d = data();
    let s = met_twice(&[Version::FIRST]);
    let p = page(&s, &d);
    assert!(p[0].starts_with("Enigma 1.0.1  [CODE GATE]"), "{p:#?}");
    // The newest source you own is 1.0.0's: listed whole, against nothing.
    let top = at(&p, "source         1.0.0");
    assert_eq!(p[top], "source         1.0.0");
    assert_eq!(
        p[top + 1..top + 5],
        [
            "   10 IF TARGET SHIELD > 0",
            "   20 TRACE 10",
            "   30 ATTACK 4",
            "   40 TRACE 4"
        ]
    );
    // A row per version, newest first, with whether its source is on disk.
    let rows = at(&p, "versions       bump     won  lost  src");
    assert_eq!(
        p[rows + 1].split_whitespace().collect::<Vec<_>>(),
        ["1.0.1", "patch", "1", "0", "-"]
    );
    assert_eq!(
        p[rows + 2].split_whitespace().collect::<Vec<_>>(),
        ["1.0.0", "-", "2", "1", "yes"]
    );
    // Nothing of 1.0.1's code: its line 10 is ATTACK 4.
    assert!(!p.iter().any(|l| l.contains("10 ATTACK 4")), "{p:#?}");
}

#[test]
fn with_no_source_the_page_says_so_and_shows_no_code() {
    let d = data();
    let s = met_twice(&[]);
    let p = page(&s, &d);
    let muted: Vec<String> = entry(&s, &d)
        .page(&s, &d)
        .into_iter()
        .filter(|l| l.muted)
        .map(|l| l.text)
        .collect();
    assert_eq!(muted, ["source         none"]);
    for stmt in ["SHIELD", "TRACE 10", "ATTACK 4", "TRACE 4"] {
        assert!(!p.iter().any(|l| l.contains(stmt)), "{stmt}: {p:#?}");
    }
    // The record is still there, version by version.
    assert!(p.iter().any(|l| l.contains("versions")), "{p:#?}");
}

#[test]
fn two_sources_owned_make_a_changelog() {
    let d = data();
    let s = met_twice(&[Version::FIRST, v101()]);
    let p = page(&s, &d);
    let top = at(&p, "source         1.0.1, against 1.0.0");
    assert_eq!(
        p[top + 1..top + 5],
        [
            " ~ 10 ATTACK 4",
            "   20 IF TARGET SHIELD > 0",
            "   30 TRACE 10",
            "   40 TRACE 4"
        ]
    );
    // Under the table, the older source as it is, against nothing older.
    let old = p
        .iter()
        .rposition(|l| l == "1.0.0")
        .expect("1.0.0's listing");
    assert!(old > at(&p, "versions"));
    assert_eq!(p[old + 1], "   10 IF TARGET SHIELD > 0");
}

#[test]
fn a_listing_says_which_version_its_marks_are_against() {
    // 1.0.0 and 1.1.0 owned, 1.0.1 never: 1.1.0's marks are against 1.0.0.
    let d = data();
    let mut s = met_twice(&[Version::FIRST]);
    let minor = Version::default().bumped(Bump::Minor);
    s.ice_sources.push(source(minor, V102));
    let p = page(&s, &d);
    assert!(
        p.iter().any(|l| l == "source         1.1.0, against 1.0.0"),
        "{p:#?}"
    );
    // A source of a version never met has a row, with no record.
    let rows = at(&p, "versions");
    assert_eq!(
        p[rows + 1].split_whitespace().collect::<Vec<_>>(),
        ["1.1.0", "-", "-", "-", "yes"]
    );
}

#[test]
fn one_version_has_no_changelog_and_an_unknown_kind_is_question_marks() {
    let d = data();
    let mut s = Save::default();
    s.ice_seen.push(IceSeen {
        id: "enigma".into(),
        met: 1,
        ..Default::default()
    });
    let p = page(&s, &d);
    assert!(p[0].starts_with("Enigma 1.0.0"));
    assert!(!p.iter().any(|l| l.starts_with("versions")), "{p:#?}");
    let unmet = icedb::entries(&s, &d)
        .into_iter()
        .find(|e| e.def.id == "rototurret")
        .unwrap();
    assert_eq!(unmet.name(), "????");
    assert!(unmet.changelog().is_empty());
}

#[test]
fn a_source_of_a_kind_never_met_names_it_and_lists_it() {
    let d = data();
    let mut s = Save::default();
    s.ice_sources.push(source(Version::FIRST, ""));
    let e = entry(&s, &d);
    assert_eq!(e.name(), "Enigma");
    let p = e.lines(&s, &d);
    assert!(p[0].starts_with("Enigma 1.0.0"), "{p:#?}");
    assert!(p.contains(&"No record. You haven't met this one yet.".to_string()));
    assert!(p.contains(&"   30 ATTACK 4".to_string()), "{p:#?}");
}

#[test]
fn a_source_of_another_program_at_the_version_met_is_not_that_version_s() {
    // 1.0.1 met running V101; the save holds a 1.0.1 source of another program (the row
    // changed under it). The ICE DB lists nothing for it and doesn't claim the source.
    let d = data();
    let mut s = met_twice(&[]);
    s.ice_sources.push(source(v101(), V102));
    let p = page(&s, &d);
    assert!(p.contains(&"source         none".to_string()), "{p:#?}");
    assert!(!p.iter().any(|l| l.contains("TRACE 5")), "{p:#?}");
    let rows = at(&p, "versions");
    assert_eq!(
        p[rows + 1].split_whitespace().collect::<Vec<_>>(),
        ["1.0.1", "patch", "1", "0", "-"]
    );
    // With the right one too, that's the one listed and claimed.
    s.ice_sources.push(source(v101(), V101));
    let p = page(&s, &d);
    assert!(p.iter().any(|l| l == "source         1.0.1"), "{p:#?}");
    assert!(p.iter().any(|l| l == "   10 ATTACK 4"), "{p:#?}");
    let rows = at(&p, "versions");
    assert_eq!(p[rows + 1].split_whitespace().last(), Some("yes"));
}

#[test]
fn two_programs_met_at_one_number_keep_both_records() {
    // `IceSeen::meet` keeps 1.0.1 running another program apart: both records are listed.
    let d = data();
    let mut s = met_twice(&[]);
    s.ice_seen[0].past.push(SeenVersion {
        version: v101(),
        bump: Some(Bump::Patch),
        program: V102.into(),
        won: 4,
        lost: 2,
    });
    let p = page(&s, &d);
    let rows = at(&p, "versions");
    let table: Vec<Vec<&str>> = p[rows + 1..rows + 4]
        .iter()
        .map(|l| l.split_whitespace().collect())
        .collect();
    assert!(
        table.contains(&vec!["1.0.1", "patch", "1", "0", "-"]),
        "{table:?}"
    );
    assert!(
        table.contains(&vec!["1.0.1", "patch", "4", "2", "-"]),
        "{table:?}"
    );
    assert!(
        table.contains(&vec!["1.0.0", "-", "2", "1", "-"]),
        "{table:?}"
    );
}
