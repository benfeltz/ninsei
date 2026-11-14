//! 141.11.3 acceptance: through a career of real runs (`Save::dial`, the probe's SAFE_PICK bot,
//! `Save::record`, the sysops' time at home), the post that a version is in the wild comes
//! before its source, and a source bought on each board lists with that board's notes.

use ninsei_core::bbs::Line;
use ninsei_core::data::Drop;
use ninsei_core::hub::{Entry, Save};
use ninsei_core::probe::{self, Bot, SAFE_PICK};
use ninsei_core::*;

fn deck() -> Save {
    let mut s = Save {
        program: Some(SAFE_PICK.into()),
        ..Save::default()
    };
    s.ensure_key(0x1411_3003);
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
    for b in ["gutter", "deadsector", "corplink"] {
        s.learn_board(b, "test");
    }
    s
}

#[test]
fn the_post_comes_before_the_source() {
    let d = Data::builtin().unwrap();
    let arc = std::sync::Arc::new(d.clone());
    let gutter = d.board("gutter").unwrap();
    let mut s = deck();
    let mut posted_early = 0;
    for r in 0..60 {
        s.program = Some(SAFE_PICK.into());
        let Ok(g) = s.dial(&d, 0, 0xFEED + r) else {
            continue;
        };
        let (g, _) = probe::play(g, Bot::SafePick);
        s.record(&g, &[], &d.rig).unwrap();
        s.study(&arc, tuning::STUDY_PATIENCE, u32::MAX);
        let call = s.home_call(&d, &d.rig, gutter);
        let Some(news) = call.news() else { continue };
        for v in s.ice_versions.iter().filter(|v| v.crack > 0) {
            let row = d.ice.iter().find(|i| i.id == v.id).unwrap();
            if row.tier != 1 {
                continue;
            }
            // Its source isn't on the board yet...
            let listed = call.board().areas.last().unwrap().files.iter().any(|f| {
                matches!(&f.item, Drop::Source { ice, version } if *ice == v.id && *version == v.version)
            });
            assert!(!listed, "{} {} sold before it's cracked", v.id, v.version);
            // ...but the scene has posted it, signed.
            let said = format!("{} {}", row.name, v.version);
            if news.body.iter().any(|l| ansi::plain(l).contains(&said)) {
                assert!(
                    news.body.last().unwrap().contains("c4se"),
                    "{:?}",
                    news.body
                );
                posted_early += 1;
            }
        }
    }
    assert!(
        posted_early > 0,
        "a version was posted while its source was being cracked"
    );
}

#[test]
fn a_source_from_each_board_fights_with_that_board_s_notes() {
    let d = Data::builtin().unwrap();
    for (board, ice, sign) in [
        ("gutter", "enigma", "NiGHTMOTHS"),
        ("deadsector", "hydra", "RAZORWIRE"),
        ("corplink", "archer", "DOCUMENT CONTROL"),
    ] {
        let mut s = deck();
        s.bank = 5_000;
        let b = d.board(board).unwrap();
        let shown = s.home_call(&d, &d.rig, b);
        let a = shown.board().areas.len() - 1;
        let f = shown.board().areas[a]
            .files
            .iter()
            .position(|f| matches!(&f.item, Drop::Source { ice: i, .. } if i == ice))
            .unwrap();
        s.home_buy_shown(&d, &d.rig, &shown, a, f).unwrap();
        // A fight with it, through the real run: its listing heads with the board's notes.
        let mut g = s.jack_in(&d, 5);
        let n = g.net.nodes[g.at].links[0];
        let ix = d.ice_by_id(ice).unwrap();
        g.net.nodes[n].kind = NodeKind::Ice;
        g.net.nodes[n].ice = Some(ix);
        g.net.nodes[n].cleared = false;
        g.apply(Action::Move(n));
        assert_eq!(g.phase, Phase::Combat, "{board}");
        let rows = ice_listing::rows(&g);
        assert!(
            rows[0].rem && rows[0].text.contains(sign),
            "{board}: {rows:#?}"
        );
        // The program it runs is its row's, notes or no notes.
        let want = basic::parse_with(&d.ice[ix].program, basic::Vocab::Ice(&d.ice[ix].verbs));
        assert_eq!(g.combat.as_ref().unwrap().ice_prog, want, "{board}");
        let code: Vec<&str> = rows
            .iter()
            .filter(|r| !r.rem)
            .map(|r| r.text.as_str())
            .collect();
        let lines: Vec<&str> = want.lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(code, lines, "{board}");
    }
}
