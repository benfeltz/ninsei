//! 141.9.3 acceptance: what a career of real runs shows. Fights name the version they run, the
//! ICE DB never shows or marks what wasn't seen, and the boards' news tells what shipped, in
//! the words of who's telling it.

use ninsei_core::bbs::{COLS, Line, ROWS, Screen, Session};
use ninsei_core::hub::{Entry, Save};
use ninsei_core::ice_listing::Mark;
use ninsei_core::probe::{self, Bot, SAFE_PICK};
use ninsei_core::*;

/// A SAFE_PICK career of `runs` tier-1 runs, its ICE DB on; every fight's rez line checked
/// against the version the save had in force.
fn career(d: &Data, runs: u64) -> Save {
    let mut s = Save {
        program: Some(SAFE_PICK.into()),
        ..Save::default()
    };
    s.ensure_key(0x9143);
    s.tools.push("icedb".into());
    s.phonebook.push(Entry {
        id: 0,
        name: "probe".into(),
        business: "test".into(),
        number: "555-0000".into(),
        tier: 1,
        traits: Vec::new(),
        heat: 0,
        runs: 0,
        last: Vec::new(),
    });
    for r in 0..runs {
        s.program = Some(SAFE_PICK.into());
        let Ok(g) = s.dial(d, 0, 0x3E0 + r) else {
            continue;
        };
        let (g, _) = probe::play(g, Bot::SafePick);
        for l in g.log.iter().filter(|l| l.text.starts_with("!! ")) {
            if let Some(row) = d
                .ice
                .iter()
                .find(|i| l.text.starts_with(&format!("!! {} ", i.name)))
            {
                let (v, _) = s.ice_program(&row.id, d).unwrap();
                assert!(
                    l.text.starts_with(&format!("!! {} {v} rezzed", row.name)),
                    "{}",
                    l.text
                );
            }
        }
        s.record(&g, &[], &d.rig).unwrap();
    }
    s
}

#[test]
fn the_ice_db_never_shows_or_marks_what_was_not_seen() {
    let d = Data::builtin().unwrap();
    let s = career(&d, 90);
    assert!(!s.ice_versions.is_empty(), "the career shipped nothing");
    let mut pages_with_history = 0;
    for e in icedb::entries(&s, &d) {
        let Some(seen) = e.seen else { continue };
        // Every version as kept: its program and the lines seen on it.
        let row_or = |p: &str| {
            if p.is_empty() {
                e.def.program.clone()
            } else {
                p.to_string()
            }
        };
        let mut versions = vec![(seen.version, row_or(&seen.program), seen.lines.clone())];
        versions.extend(
            seen.past
                .iter()
                .rev()
                .map(|p| (p.version, row_or(&p.program), p.lines.clone())),
        );
        pages_with_history += usize::from(versions.len() > 1);
        for (k, (v, program, lines)) in versions.iter().enumerate() {
            let prev = versions
                .get(k + 1)
                .map(|(_, p, l)| (p.as_str(), l.as_slice()));
            let rows = ice_listing::changes(&e.def.verbs, prev, (program, lines));
            let parsed = basic::parse_with(program, basic::Vocab::Ice(&e.def.verbs));
            for r in &rows {
                if r.mark == Mark::Removed {
                    // A removed line was seen on the version before, as it reads.
                    let (pp, pl) = prev.expect("a removal needs a version before");
                    let before = basic::parse_with(pp, basic::Vocab::Ice(&e.def.verbs));
                    assert!(
                        pl.contains(&r.no),
                        "{} {v}: removed {} unseen",
                        e.def.id,
                        r.no
                    );
                    assert_eq!(
                        r.text.as_deref(),
                        before.line(r.no).map(|l| l.text.as_str())
                    );
                    continue;
                }
                let is_seen = lines.contains(&r.no);
                // In clear only if seen, and then as the line reads in that version.
                assert_eq!(r.text.is_some(), is_seen, "{} {v} line {}", e.def.id, r.no);
                if let Some(t) = &r.text {
                    assert_eq!(Some(t.as_str()), parsed.line(r.no).map(|l| l.text.as_str()));
                }
                // Marked only if seen.
                assert!(
                    is_seen || r.mark == Mark::None,
                    "{} {v} line {} marked unseen",
                    e.def.id,
                    r.no
                );
            }
        }
        // The page heads with the version last met.
        let page = e.lines(&s, &d);
        assert!(
            page[0].starts_with(&format!("{} {}", e.def.name, seen.version)),
            "{}",
            page[0]
        );
    }
    assert!(pages_with_history > 0, "no kind was met at two versions");
}

#[test]
fn the_boards_tell_what_shipped_in_their_own_words() {
    let d = Data::builtin().unwrap();
    let s = career(&d, 90);
    let shipped = s.shipped(&d);
    assert!(!shipped.is_empty());
    for (i, b) in d.boards.iter().enumerate() {
        let call = s.home_call(&d, &d.rig, i);
        let Some(n) = &b.ice_news else {
            assert!(call.news().is_none(), "{} hears nothing", b.id);
            continue;
        };
        let news = call.news().expect("a board with ice_news posts it");
        // Newest first, only the tiers it hears, at most its count.
        let want: Vec<String> = shipped
            .iter()
            .rev()
            .filter(|(ice, ..)| n.tiers.contains(&d.ice[*ice].tier))
            .take(n.count)
            .map(|(ice, v, _)| format!("{} {v}", d.ice[*ice].name))
            .collect();
        if want.is_empty() {
            assert_eq!(news.body, n.none, "{}", b.id);
        } else {
            assert_eq!(news.body.len(), want.len(), "{}", b.id);
            for (line, w) in news.body.iter().zip(&want) {
                assert!(
                    ansi::plain(line).contains(w.as_str()),
                    "{}: {line} lacks {w}",
                    b.id
                );
            }
        }
        // It reads on the board as every screen does.
        let sess = Session {
            screen: Screen::Bulletin(call.bulletin_count() - 1),
            status: None,
        };
        let lines = sess.render(&call);
        assert!(lines.len() <= ROWS);
        for l in &lines {
            assert!(ansi::visible_len(l) <= COLS, "{}: {}", b.id, ansi::plain(l));
        }
    }
}
