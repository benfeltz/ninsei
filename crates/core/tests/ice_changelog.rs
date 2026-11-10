//! 141.9.3: the ICE DB is a changelog. Every version of a kind you've met, as you knew it, with
//! changes marked only where you saw them, so a new version is totally unknown when met.

use ninsei_core::hub::{IceSeen, Save, SeenVersion};
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
        .map(|r| format!("{}{} {}", r.mark.glyph(), r.no, r.shown()))
        .collect()
}

#[test]
fn a_moved_block_is_marked_moved_and_only_it() {
    let d = data();
    let v = &enigma(&d).verbs;
    let all = [10, 20, 30, 40];
    let rows = changes(v, Some((V100, &all)), (V101, &all));
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
    let rows = changes(v, Some((V101, &all)), (V102, &all));
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
}

#[test]
fn nothing_is_marked_or_shown_that_was_not_seen() {
    let d = data();
    let v = &enigma(&d).verbs;
    let all = [10, 20, 30, 40];
    // Just met: every line ????, nothing marked, nothing removed.
    let rows = changes(v, Some((V101, &all)), (V102, &[]));
    assert_eq!(
        marks(&rows),
        [" 10 ????", " 20 ????", " 30 ????", " 40 ????"]
    );
    // Seen up to its new line, but not all of it: the new line is marked, no removal yet.
    let rows = changes(v, Some((V101, &all)), (V102, &[10, 40]));
    assert_eq!(
        marks(&rows),
        [" 10 ATTACK 4", " 20 ????", " 30 ????", "+40 TRACE 5"]
    );
    // All of it seen, but the removed line was never seen in the version before: nothing is
    // said about it, and the new line can't be called new (it may have been the unseen one).
    let rows = changes(v, Some((V101, &[10, 20])), (V102, &all));
    assert_eq!(
        marks(&rows),
        [
            " 10 ATTACK 4",
            " 20 IF TARGET SHIELD > 0",
            " 30 TRACE 10",
            " 40 TRACE 5"
        ]
    );
    // The first version met has no marks.
    let rows = changes(v, None, (V100, &all));
    assert!(rows.iter().all(|r| r.mark == Mark::None));
}

/// A save that met Enigma at 1.0.0 (lines 10 and 30 seen, won 2, lost 1), then at 1.0.1, a
/// patch, with `now` seen.
fn met_twice(now: &[u32]) -> Save {
    let mut s = Save::default();
    s.ice_seen.push(IceSeen {
        id: "enigma".into(),
        met: 4,
        broken: 3,
        lo: 12,
        hi: 12,
        lines: now.to_vec(),
        version: Version::default().bumped(Bump::Patch),
        bump: Some(Bump::Patch),
        program: V101.into(),
        won: 1,
        lost: 0,
        past: vec![SeenVersion {
            version: Version::default(),
            bump: None,
            program: String::new(),
            lines: vec![10, 30],
            won: 2,
            lost: 1,
        }],
    });
    s
}

fn page(s: &Save, d: &Data) -> Vec<String> {
    icedb::entries(s, d)
        .into_iter()
        .find(|e| e.def.id == "enigma")
        .unwrap()
        .lines(s, d)
}

#[test]
fn a_new_version_is_met_unknown_and_the_old_one_kept_as_it_was() {
    let d = data();
    let s = met_twice(&[]);
    let p = page(&s, &d);
    assert!(p[0].starts_with("Enigma 1.0.1  [CODE GATE]"), "{p:#?}");
    let at = |t: &str| {
        p.iter()
            .position(|l| l.contains(t))
            .unwrap_or_else(|| panic!("{t}: {p:#?}"))
    };
    // The version last met: nothing seen, nothing marked.
    let top = at("listing        1.0.1, decompiled, 0 of 4 lines seen");
    assert_eq!(
        p[top + 1..top + 5],
        ["   10 ????", "   20 ????", "   30 ????", "   40 ????"]
    );
    // A row per version, newest first.
    let rows = at("versions       bump     won  lost");
    assert_eq!(
        p[rows + 1].split_whitespace().collect::<Vec<_>>(),
        ["1.0.1", "patch", "1", "0"]
    );
    assert_eq!(
        p[rows + 2].split_whitespace().collect::<Vec<_>>(),
        ["1.0.0", "-", "2", "1"]
    );
    // 1.0.0 as it was known: the lines seen there in clear, unmarked (it's the first met).
    let old = at("1.0.0, decompiled, 2 of 4 lines seen");
    assert_eq!(
        p[old + 1..old + 5],
        [
            "   10 IF TARGET SHIELD > 0",
            "   20 ????",
            "   30 ATTACK 4",
            "   40 ????"
        ]
    );
    // Nothing of 1.0.0 shows on 1.0.1's lines: line 10 is ATTACK 4 now, still unseen.
    assert!(!p[top..rows].iter().any(|l| l.contains("SHIELD")));
}

#[test]
fn the_new_versions_changes_show_as_they_are_seen() {
    let d = data();
    let p = page(&met_twice(&[10]), &d);
    // Its first line seen; but of 1.0.0 only lines 10 and 30 were, and ATTACK 4 was among
    // them: what's known doesn't say it moved, so it isn't marked. The rest stay ????.
    assert!(p.iter().any(|l| l == "   10 ATTACK 4"), "{p:#?}");
    assert!(p.iter().any(|l| l == "   20 ????"), "{p:#?}");
    let muted: Vec<String> = icedb::entries(&met_twice(&[10]), &d)
        .into_iter()
        .find(|e| e.def.id == "enigma")
        .unwrap()
        .listing()
        .into_iter()
        .filter(|l| l.muted)
        .map(|l| l.text)
        .collect();
    assert_eq!(muted, ["   20 ????", "   30 ????", "   40 ????"]);
}

#[test]
fn one_version_met_has_no_changelog_and_an_unmet_kind_is_question_marks() {
    let d = data();
    let mut s = Save::default();
    s.ice_seen.push(IceSeen {
        id: "enigma".into(),
        met: 1,
        lines: vec![10],
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
fn a_changed_number_is_new_even_when_the_same_line_is_elsewhere() {
    // 30 TRACE 10 became 30 TRACE 11; TRACE 11 was already at 10 and 50, and still is.
    let d = data();
    let v = &enigma(&d).verbs;
    let before = "10 TRACE 11\n20 IF TARGET SHIELD > 0\n30 TRACE 10\n40 TRACE 11";
    let after = "10 TRACE 11\n20 IF TARGET SHIELD > 0\n30 TRACE 11\n40 TRACE 11";
    let all = [10, 20, 30, 40];
    let rows = changes(v, Some((before, &all)), (after, &all));
    assert_eq!(
        marks(&rows),
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
fn nothing_about_an_unseen_version_before_leaks_into_the_marks() {
    // 1.0.0 met and broken before it ran anything; 1.0.1's line 10 seen. Nothing of 1.0.0 was
    // seen, so nothing can be said about how 1.0.1 differs from it.
    let d = data();
    let v = &enigma(&d).verbs;
    let rows = changes(v, Some((V100, &[])), (V101, &[10]));
    assert!(
        rows.iter().all(|r| r.mark == Mark::None),
        "{:?}",
        marks(&rows)
    );
    // All of 1.0.1 seen: still nothing, and nothing removed.
    let rows = changes(v, Some((V100, &[])), (V101, &[10, 20, 30, 40]));
    assert_eq!(rows.len(), 4);
    assert!(
        rows.iter().all(|r| r.mark == Mark::None),
        "{:?}",
        marks(&rows)
    );
}
