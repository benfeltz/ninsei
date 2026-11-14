//! 141.9.3 acceptance, made strict by 141.11.1: what a career of real runs shows. Fights name
//! the version they run, the ICE DB shows the code only of the sources you own, and the boards'
//! news tells what shipped, in the words of who's telling it.

use ninsei_core::bbs::{COLS, Line, ROWS, Screen, Session};
use ninsei_core::hub::{Entry, Save};
use ninsei_core::probe::{self, Bot, SAFE_PICK};
use ninsei_core::*;

/// A SAFE_PICK career of `runs` tier-1 runs, its ICE DB on, with a patience's worth of the
/// sysops' time at home after each (141.9.4); every fight's rez line checked against the
/// version the save had in force.
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
        s.study(
            &std::sync::Arc::new(d.clone()),
            tuning::STUDY_PATIENCE,
            u32::MAX,
        );
    }
    s
}

#[test]
fn the_ice_db_shows_only_the_sources_you_own() {
    let d = Data::builtin().unwrap();
    let mut s = career(&d, 90);
    assert!(!s.ice_versions.is_empty(), "the career shipped nothing");
    assert!(s.ice_sources.is_empty(), "fighting got it no source");
    // Every version met of every kind: (kind, version, program as booked).
    let mut met: Vec<(String, ice_version::Version, String)> = Vec::new();
    for seen in &s.ice_seen {
        met.push((seen.id.clone(), seen.version, seen.program.clone()));
        met.extend(
            seen.past
                .iter()
                .map(|p| (seen.id.clone(), p.version, p.program.clone())),
        );
    }
    assert!(
        s.ice_seen.iter().any(|x| !x.past.is_empty()),
        "no kind was met at two versions"
    );
    let statements = |id: &str, program: &str| -> Vec<String> {
        let row = d.ice.iter().find(|i| i.id == id).unwrap();
        let text = if program.is_empty() {
            row.program.as_str()
        } else {
            program
        };
        basic::parse_with(text, basic::Vocab::Ice(&row.verbs))
            .lines
            .iter()
            .map(|l| l.text.clone())
            .collect()
    };
    // No source: the record of every version met, and none of any version's code.
    for e in icedb::entries(&s, &d) {
        let Some(seen) = e.seen else { continue };
        let page = e.lines(&s, &d);
        assert!(
            page[0].starts_with(&format!("{} {}", e.def.name, seen.version)),
            "{}",
            page[0]
        );
        assert!(page.contains(&"source         none".to_string()));
        for (_, _, program) in met.iter().filter(|(id, ..)| *id == e.def.id) {
            for stmt in statements(&e.def.id, program) {
                assert!(
                    !page.iter().any(|l| l.contains(stmt.as_str())),
                    "{}: {stmt:?} on {page:#?}",
                    e.def.id
                );
            }
        }
    }
    // Every version's source owned: each page lists the newest, whole, as it reads.
    for (id, v, program) in &met {
        s.ice_sources.push(hub::IceSource {
            id: id.clone(),
            version: *v,
            program: program.clone(),
        });
    }
    for e in icedb::entries(&s, &d) {
        if e.seen.is_none() {
            continue;
        }
        let (newest, _) = e.sources[0];
        let (_, _, program) = met
            .iter()
            .find(|(id, v, _)| *id == e.def.id && *v == newest)
            .unwrap();
        let page = e.lines(&s, &d);
        let top = page
            .iter()
            .position(|l| l.starts_with(&format!("source         {newest}")))
            .unwrap_or_else(|| panic!("{page:#?}"));
        let listed: Vec<&str> = page[top + 1..]
            .iter()
            .take_while(|l| l.starts_with(' ') && !l.contains("gives up"))
            .filter(|l| !l.starts_with(" -"))
            .map(|l| l[5..].trim_start())
            .collect();
        let want = statements(&e.def.id, program);
        assert_eq!(listed, want, "{}: {page:#?}", e.def.id);
    }
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
            // The news, then a blank line and its caller's sign-off.
            assert_eq!(news.body.len(), want.len() + 2, "{}", b.id);
            assert!(news.body.last().unwrap().contains(&n.by), "{}", b.id);
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
