//! BBSes you find go in the phonebook; calling one from home costs a phone bill, sells to the
//! bank, and posts jobs for the networks you know.

use ninsei_core::bbs::{COLS, Line, ROWS, Screen, Session};
use ninsei_core::hub::{self, Answer, Save, ShellEffect};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

fn save(bank: u32) -> Save {
    let d = data();
    let mut s = Save::default();
    s.ensure_key(0xB0A2D);
    s.learn_tool("wardial");
    s.ensure_phonebook(&d, 7);
    // These tests learn boards themselves.
    s.boards.clear();
    s.bank = bank;
    s
}

#[test]
fn a_fresh_phonebook_has_the_gutter() {
    let d = data();
    let mut s = Save::default();
    s.ensure_phonebook(&d, 7);
    assert!(
        s.knows_board("gutter"),
        "the starter tools must be reachable"
    );
}

/// A save that already knows every board.
fn knows_all(bank: u32) -> Save {
    let mut s = save(bank);
    for b in &data().boards {
        s.learn_board(&b.id, "a test");
    }
    s
}

#[test]
fn old_saves_know_no_boards() {
    let s = Save::load("(version: 1, runs: 1)").unwrap();
    assert!(s.boards.is_empty() && s.pending.is_empty() && s.home_bought.is_empty());
}

#[test]
fn connecting_in_a_run_puts_the_board_in_the_phonebook() {
    let d = data();
    for end in [Phase::JackedOut, Phase::Flatlined] {
        let mut s = save(0);
        let id = s.phonebook[0].id;
        let mut g = s.dial(&d, id, 4).unwrap();
        let to = g.net.nodes[g.at].links[0];
        g.net.nodes[to].kind = NodeKind::Bbs;
        g.apply(Action::Move(to));
        assert!(g.bbs_seen);
        assert!(g.log.iter().any(|l| l.text.contains("noted")));
        let board = g.bbs().id.clone();
        g.phase = end;
        let r = s.record(&g, &[], &d.rig).unwrap();
        assert!(s.knows_board(&board), "{end:?}");
        assert_eq!(r.boards.len(), 1);
        // Known boards aren't news twice.
        let mut g = s.dial(&d, id, 5).unwrap();
        g.bbs_seen = true;
        g.phase = end;
        assert!(s.record(&g, &[], &d.rig).unwrap().boards.is_empty());
    }
}

#[test]
fn a_run_that_never_connects_finds_nothing() {
    let d = data();
    let mut s = save(0);
    let id = s.phonebook[0].id;
    let mut g = s.dial(&d, id, 4).unwrap();
    g.phase = Phase::JackedOut;
    s.record(&g, &[], &d.rig).unwrap();
    assert!(s.boards.is_empty());
}

#[test]
fn wardialing_a_boards_block_finds_it() {
    let d = data();
    let mut s = save(100_000);
    let mut seen = Vec::new();
    while s.scanned_in("local") < 100 {
        let r = s.wardial(&d, "local").unwrap();
        for (number, a) in &r.cells {
            if let Answer::Board { board, new } = a {
                assert!(new);
                assert_eq!(&d.boards[*board].phone, number);
                assert_eq!(a.glyph(), 'B');
                seen.push(*board);
            }
        }
    }
    seen.sort_unstable();
    assert_eq!(
        seen,
        (0..d.boards.len()).collect::<Vec<_>>(),
        "every board, once"
    );
    assert_eq!(s.boards.len(), d.boards.len());
    assert!(s.boards.iter().all(|b| b.source.contains("wardial")));
}

#[test]
fn a_call_costs_the_phone_bill() {
    let d = data();
    let rig = &d.rig;
    let far = d.board("deadsector").unwrap();
    let mut s = save(100);
    assert!(
        s.call_board(&d, rig, far)
            .unwrap_err()
            .contains("don't have")
    );
    s.learn_board("deadsector", "a test");
    let cost = d.boards[far].call_cost;
    assert!(cost > 0);
    let call = s.call_board(&d, rig, far).unwrap();
    assert_eq!(s.bank, 100 - cost);
    assert_eq!(call.credits(), s.bank);
    let mut broke = knows_all(0);
    assert!(
        broke
            .call_board(&d, rig, far)
            .unwrap_err()
            .contains("phone bill")
    );
    // The Gutter is a local number: free, even with an empty bank.
    let gutter = d.board("gutter").unwrap();
    assert_eq!(d.boards[gutter].call_cost, 0);
    assert!(broke.call_board(&d, rig, gutter).is_ok());
}

#[test]
fn listings_hold_until_the_next_run() {
    let d = data();
    let mut s = knows_all(10_000);
    let board = d.board("deadsector").unwrap();
    let a = s.home_call(&d, &d.rig, board);
    let b = s.home_call(&d, &d.rig, board);
    assert_eq!(a.stock(), b.stock());
    assert_eq!(a.jobs(), b.jobs());
    // Some run later, the listing changes on some board.
    let id = s.phonebook[0].id;
    let changed = (0..5).any(|i| {
        let mut g = s.dial(&d, id, i).unwrap();
        g.phase = Phase::JackedOut;
        s.record(&g, &[], &d.rig);
        s.home_call(&d, &d.rig, board).stock() != a.stock()
    });
    assert!(changed);
}

/// The first file in the listing whose item matches.
fn find(call: &home_bbs::HomeCall, want: impl Fn(&Drop) -> bool) -> Option<(usize, usize)> {
    let b = call.board();
    for (a, files) in call.stock().iter().enumerate() {
        for (f, s) in files.iter().enumerate() {
            if want(&b.areas[a].files[s.def].item) {
                return Some((a, f));
            }
        }
    }
    None
}

#[test]
fn buying_from_home_spends_the_bank() {
    let d = data();
    let rig = &d.rig;
    let mut s = knows_all(10_000);
    let board = d.board("gutter").unwrap();
    let call = s.home_call(&d, rig, board);

    let (a, f) = find(&call, |i| matches!(i, Drop::Warez(_))).unwrap();
    let price = call.board().areas[a].files[call.stock()[a][f].def].price;
    let stash = s.stash.len();
    s.home_buy(&d, rig, board, a, f).unwrap();
    assert_eq!(s.bank, 10_000 - price);
    assert_eq!(s.stash.len(), stash + 1);
    let item = s.stash.last().unwrap();
    let bin = binary::from_hex(&item.hex).unwrap();
    assert!(
        binary::check(&bin, s.key).is_ok(),
        "sealed with the save's key"
    );
    // Bought stays bought, even on the next call.
    assert!(s.home_call(&d, rig, board).stock()[a][f].bought);
    assert!(s.home_buy(&d, rig, board, a, f).is_err());

    // Deck mods need a run.
    if let Some((a, f)) = find(&call, |i| {
        matches!(
            i,
            Drop::Ram(_) | Drop::Cpu(_) | Drop::Hdd(_) | Drop::Repair(_)
        )
    }) {
        let bank = s.bank;
        assert!(
            s.home_buy(&d, rig, board, a, f)
                .unwrap_err()
                .contains("run")
        );
        assert_eq!(s.bank, bank);
    }
    let mut broke = knows_all(0);
    assert!(
        broke
            .home_buy(&d, rig, board, a, f)
            .unwrap_err()
            .contains("insufficient")
    );
}

#[test]
fn textfiles_from_home_teach_ranges() {
    let d = data();
    for board in 0..d.boards.len() {
        let mut s = knows_all(10_000);
        let call = s.home_call(&d, &d.rig, board);
        if let Some((a, f)) = find(&call, |i| matches!(i, Drop::Hint(_))) {
            let before = s.ranges.len();
            s.home_buy(&d, &d.rig, board, a, f).unwrap();
            assert!(s.ranges.len() >= before);
        }
    }
}

#[test]
fn a_job_taken_at_home_rides_into_that_run() {
    let d = data();
    let mut s = knows_all(1_000);
    let board = d.board("corplink").unwrap();
    let call = s.home_call(&d, &d.rig, board);
    let jobs = call.jobs();
    assert!(!jobs.is_empty());
    let post = jobs[0].clone();
    let entry = s
        .phonebook
        .iter()
        .find(|e| e.number == post.detail)
        .unwrap()
        .id;
    s.home_take(&d, board, 0).unwrap();
    assert!(s.home_call(&d, &d.rig, board).jobs()[0].taken);
    assert!(s.home_take(&d, board, 0).is_err(), "once");
    assert_eq!(s.pending.len(), 1);

    // Dialing another number leaves it waiting.
    if let Some(other) = s.phonebook.iter().find(|e| e.id != entry).map(|e| e.id) {
        let g = s.dial(&d, other, 3).unwrap();
        assert!(g.contracts.is_empty());
        assert_eq!(s.pending.len(), 1);
    }
    let g = s.dial(&d, entry, 3).unwrap();
    assert!(s.pending.is_empty(), "it went into the run");
    assert_eq!(g.contracts.len(), 1);
    let k = &g.contracts[0];
    assert_eq!(k.status, ContractStatus::Active);
    assert_eq!(
        (k.client.as_str(), k.reward),
        (post.client.as_str(), post.reward)
    );
    assert!(matches!(
        g.root().nodes[k.node].kind,
        NodeKind::Ice | NodeKind::BlackIce
    ));
    assert!(!k.brief.contains("{host}"));
    assert!(g.log.iter().any(|l| l.text.contains("contract from home")));
}

#[test]
fn jobs_never_change_the_network() {
    let d = data();
    let mut a = knows_all(1_000);
    let mut b = a.clone();
    let board = d.board("gutter").unwrap();
    b.home_take(&d, board, 0).unwrap();
    let entry = b.pending[0].entry;
    let (ga, gb) = (a.dial(&d, entry, 9).unwrap(), b.dial(&d, entry, 9).unwrap());
    let shape = |g: &Game| {
        g.net
            .nodes
            .iter()
            .map(|n| (n.kind, n.links.clone(), n.ice))
            .collect::<Vec<_>>()
    };
    assert_eq!(shape(&ga), shape(&gb));
    assert_eq!(ga.bbs_stock, gb.bbs_stock);
}

#[test]
fn the_shell_lists_and_calls_boards() {
    let d = data();
    let s = knows_all(0);
    let run = |l: &str| hub::shell(l, &s, &d);
    let (lines, _) = run("phonebook");
    assert!(
        lines
            .iter()
            .any(|l| l.contains("DEAD SECTOR") && l.contains("BBS"))
    );
    assert_eq!(
        run("dial 555-0666").1,
        ShellEffect::Call("deadsector".into())
    );
    let n = s.phonebook.len() + 1;
    let first = d.boards[d.board(&s.boards[0].id).unwrap()].id.clone();
    assert_eq!(run(&format!("dial {n}")).1, ShellEffect::Call(first));
    // Unknown boards aren't in the book.
    let none = save(0);
    assert!(matches!(
        hub::shell("dial 555-0666", &none, &d).1,
        ShellEffect::None
    ));
}

#[test]
fn every_home_screen_fits() {
    let d = data();
    let s = knows_all(500);
    for board in 0..d.boards.len() {
        let call = s.home_call(&d, &d.rig, board);
        let mut screens = vec![Screen::Main, Screen::Jobs, Screen::Areas, Screen::Who];
        screens.extend((0..call.board().areas.len()).map(Screen::Area));
        for sc in screens {
            let lines = Session {
                screen: sc,
                status: Some("|12status".into()),
            }
            .render(&call);
            assert!(lines.len() <= ROWS);
            for l in &lines {
                assert!(ansi::visible_len(l) <= COLS, "{sc:?}: {}", ansi::plain(l));
            }
        }
        let main = Session {
            screen: Screen::Main,
            status: None,
        }
        .render(&call)
        .join("\n");
        assert!(
            ansi::plain(&main).to_lowercase().contains("st4sh")
                || ansi::plain(&main).contains("Stash"),
            "{main}"
        );
    }
}
