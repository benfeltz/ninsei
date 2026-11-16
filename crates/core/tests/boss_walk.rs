//! 138.1 step 2: a net's AI's map verbs and reads, on small hand-built nets.

use ninsei_core::basic::{self, Cond, Goal, Stmt, Verb, Vocab, Walk, WalkAct};
use ninsei_core::boss::{Did, Ground, Hunter, hops, route};
use ninsei_core::data::IceDef;
use ninsei_core::network::{Network, Node, NodeKind};
use ninsei_core::*;

/// A net of `layers` (each a list of kinds), linked deeper by `links` (from, to), with the links
/// in `one_way` walked deeper only.
fn net(layers: &[&[NodeKind]], links: &[(usize, usize)], one_way: &[(usize, usize)]) -> Network {
    let mut nodes = Vec::new();
    let mut ids = Vec::new();
    for (layer, kinds) in layers.iter().enumerate() {
        let mut row = Vec::new();
        for (lane, &kind) in kinds.iter().enumerate() {
            let id = nodes.len();
            nodes.push(Node {
                id,
                layer,
                lane,
                kind,
                name: format!("n{id}"),
                ice: None,
                cleared: false,
                links: links.iter().filter(|l| l.0 == id).map(|l| l.1).collect(),
                one_way: one_way.iter().filter(|l| l.0 == id).map(|l| l.1).collect(),
                walked: Vec::new(),
                beaten: None,
                replaced: false,
            });
            row.push(id);
        }
        ids.push(row);
    }
    Network { nodes, layers: ids }
}

use NodeKind::{Entry, Exit, Ice, Vault};

/// 0 -> {1, 2} -> 3 -> 4 (the exit), every link both ways.
fn diamond() -> Network {
    net(
        &[&[Entry], &[Ice, Vault], &[Ice], &[Exit]],
        &[(0, 1), (0, 2), (1, 3), (2, 3), (3, 4)],
        &[],
    )
}

/// The tier-1 AI's row with `program` and `cpu` cycles a pass.
fn row(program: &str, cpu: u32) -> (usize, IceDef) {
    let d = Data::builtin().unwrap();
    let ix = d.boss_for(1, 0).unwrap();
    let mut def = d.ice[ix].clone();
    def.program = program.into();
    def.cpu = cpu;
    (ix, def)
}

fn ground(net: &Network, runner: usize) -> Ground<'_> {
    Ground {
        net,
        runner,
        here: true,
        trace: 30,
        strength: 22,
    }
}

/// `h` running `def`'s program from now on.
fn runs(h: &mut Hunter, def: &IceDef) {
    h.prog = basic::parse_with(&def.program, def.vocab());
}

fn dids(steps: &[ninsei_core::boss::Step]) -> Vec<Did> {
    steps.iter().map(|s| s.did.clone()).collect()
}

#[test]
fn the_walks_are_words_only_an_ai_has() {
    let verbs = [Verb::Attack];
    let walks = Walk::ALL;
    let boss = Vocab::Boss(&verbs, &walks);
    let text = "10 MOVE TOWARD TARGET\n20 MOVE TOWARD EXIT\n30 HOLD\n40 WAIT\n50 SCAN 2\n\
                60 IF TARGET SEEN\n70 IF TARGET DIST < 2\n80 IF TARGET TRACE > 50\n\
                90 IF AT BLACK ICE\n100 IF TARGET INTEG < 4\n110 ATTACK 3";
    let p = basic::parse_with(text, boss);
    let stmts: Vec<Stmt> = p.lines.iter().map(|l| l.stmt.clone()).collect();
    assert_eq!(stmts[0], Stmt::Walk(WalkAct::Toward(Goal::Target)));
    assert_eq!(stmts[1], Stmt::Walk(WalkAct::Toward(Goal::Exit)));
    assert_eq!(stmts[2], Stmt::Walk(WalkAct::Hold));
    assert_eq!(stmts[3], Stmt::Walk(WalkAct::Wait));
    assert_eq!(stmts[4], Stmt::Walk(WalkAct::Scan(2)));
    assert_eq!(stmts[5], Stmt::If(Cond::Seen));
    assert!(matches!(stmts[6], Stmt::If(Cond::Dist(basic::Op::Lt, 2))));
    assert!(matches!(
        stmts[7],
        Stmt::If(Cond::TargetTrace(basic::Op::Gt, 50))
    ));
    assert_eq!(stmts[8], Stmt::If(Cond::At(NodeKind::BlackIce)));
    assert!(p.lines.iter().all(|l| p.fault(l.no).is_none()), "{p:?}");
    // An ordinary ICE has none of them, and neither has the deck.
    let ice = basic::parse_with(text, Vocab::Ice(&verbs));
    for no in [10, 30, 50, 60, 70, 80, 90] {
        assert!(ice.fault(no).is_some(), "ICE line {no}");
    }
    let deck = basic::parse(text);
    for no in [10, 30, 50, 60] {
        assert!(deck.fault(no).is_some(), "deck line {no}");
    }
    // A walk its model doesn't have is a bad line too.
    let only_move = [Walk::Move];
    let p = basic::parse_with("10 SCAN 2\n20 MOVE", Vocab::Boss(&verbs, &only_move));
    assert!(p.fault(10).is_some() && p.fault(20).is_some());
    let p = basic::parse_with("10 SCAN\n20 HOLD 3\n30 IF AT NOWHERE", boss);
    assert!(p.fault(10).is_some() && p.fault(20).is_some() && p.fault(30).is_some());
}

#[test]
fn it_moves_toward_the_target_by_the_shortest_walkable_route() {
    let n = diamond();
    let (ix, def) = row("10 MOVE TOWARD TARGET", 1);
    let mut h = Hunter::wake(ix, &def, 0, 3);
    let s = h.pass(&def, &ground(&n, 4));
    assert_eq!(dids(&s), [Did::Moved { from: 0, to: 1 }, Did::OutOfCpu]);
    h.pass(&def, &ground(&n, 4));
    assert_eq!(h.at, 3);
    // There, and nowhere new to go: the move is skipped, spending nothing.
    let s = h.pass(&def, &ground(&n, 4));
    assert_eq!(h.at, 3);
    assert_eq!(dids(&s), [Did::Skipped("already there.".into()), Did::End]);
    // Toward the exit instead.
    let (_, exit) = row("10 MOVE TOWARD EXIT", 1);
    runs(&mut h, &exit);
    h.pass(&exit, &ground(&n, 0));
    assert_eq!(h.at, 4);
    assert_eq!(route(&n, 0, 4), Some(vec![0, 1, 3, 4]));
}

#[test]
fn a_bigger_machine_walks_further_a_pass() {
    let n = diamond();
    let (ix, one) = row("10 MOVE TOWARD TARGET\n20 MOVE TOWARD TARGET", 1);
    let (_, two) = row("10 MOVE TOWARD TARGET\n20 MOVE TOWARD TARGET", 2);
    let mut a = Hunter::wake(ix, &one, 0, 3);
    let mut b = a.clone();
    a.pass(&one, &ground(&n, 4));
    b.pass(&two, &ground(&n, 4));
    assert_eq!((a.at, b.at), (1, 3));
}

#[test]
fn it_walks_one_way_links_deeper_only() {
    // 0 -> {1, 2} -> 3: 1 -> 3 is one way, so from 3 the way back to 1 goes round by 2 and 0.
    let mut n = net(
        &[&[Entry], &[Ice, Vault], &[Exit]],
        &[(0, 1), (0, 2), (1, 3), (2, 3)],
        &[(1, 3)],
    );
    assert_eq!(route(&n, 3, 1), Some(vec![3, 2, 0, 1]));
    let (ix, def) = row("10 MOVE TOWARD TARGET", 1);
    let mut h = Hunter::wake(ix, &def, 3, 1);
    h.pass(&def, &ground(&n, 1));
    assert_eq!(h.at, 2);
    // With 2 -> 3 one way too there's no way back: it stays, and the move costs nothing.
    n.nodes[2].one_way.push(3);
    let mut h = Hunter::wake(ix, &def, 3, 1);
    let s = h.pass(&def, &ground(&n, 1));
    assert_eq!(h.at, 3);
    assert_eq!(dids(&s), [Did::Skipped("no route.".into()), Did::End]);
    // A scan looks along a one way link either way.
    assert_eq!(hops(&n, 3, 1), Some(1));
}

#[test]
fn hold_keeps_it_on_its_node_for_the_pass_and_wait_for_a_cycle() {
    let n = diamond();
    let (ix, def) = row("10 HOLD\n20 MOVE TOWARD TARGET", 2);
    let mut h = Hunter::wake(ix, &def, 0, 3);
    let s = h.pass(&def, &ground(&n, 4));
    assert_eq!(h.at, 0);
    assert_eq!(dids(&s), [Did::Held, Did::OutOfCpu]);
    let (_, def) = row("10 WAIT\n20 MOVE TOWARD TARGET", 2);
    runs(&mut h, &def);
    let s = h.pass(&def, &ground(&n, 4));
    assert_eq!(
        dids(&s),
        [Did::Waited, Did::Moved { from: 0, to: 1 }, Did::OutOfCpu]
    );
}

#[test]
fn a_scan_finds_the_runner_inside_its_reach_and_not_outside() {
    let n = diamond();
    let (ix, near) = row("10 SCAN 2", 1);
    let (_, far) = row("10 SCAN 1", 1);
    let mut h = Hunter::wake(ix, &far, 4, 0);
    // The runner is on 1, two hops from 4.
    let s = h.pass(&far, &ground(&n, 1));
    assert_eq!(
        dids(&s)[0],
        Did::Scanned {
            from: 4,
            reach: 1,
            found: false
        }
    );
    assert!(
        !h.sighted && h.seen == Some(0),
        "it remembers where it last saw them"
    );
    runs(&mut h, &near);
    let s = h.pass(&near, &ground(&n, 1));
    assert_eq!(
        dids(&s)[0],
        Did::Scanned {
            from: 4,
            reach: 2,
            found: true
        }
    );
    assert!(h.sighted && h.seen == Some(1));
}

#[test]
fn its_reads_are_what_it_knows() {
    let n = diamond();
    let (ix, def) = row("10 IF TARGET SEEN\n20 MOVE TOWARD TARGET\n30 SCAN 3", 1);
    let mut h = Hunter::wake(ix, &def, 4, 0);
    let g = ground(&n, 2);
    assert!(h.holds(&Cond::Seen, &g));
    assert!(
        h.holds(&Cond::Dist(basic::Op::Gt, 2), &g),
        "it would walk 4 -> 3 -> 1 -> 0"
    );
    assert!(h.holds(&Cond::At(NodeKind::Exit), &g));
    assert!(h.holds(&Cond::TargetTrace(basic::Op::Gt, 29), &g));
    assert!(!h.holds(&Cond::Target(basic::Target::Integ, basic::Op::Lt, 99), &g));
    h.sighted = false;
    // Lost: the IF skips the move, and it scans instead.
    let s = h.pass(&def, &g);
    assert_eq!(
        dids(&s),
        [
            Did::If(false),
            Did::Scanned {
                from: 4,
                reach: 3,
                found: true
            },
            Did::OutOfCpu
        ]
    );
    // Never seen at all: no distance, so DIST < n is false and DIST > n true.
    h.seen = None;
    assert!(!h.holds(&Cond::Dist(basic::Op::Lt, 999), &g));
}

#[test]
fn stepping_onto_the_runner_ends_its_pass() {
    let n = diamond();
    let (ix, def) = row("10 MOVE TOWARD TARGET\n20 MOVE TOWARD TARGET", 2);
    let mut h = Hunter::wake(ix, &def, 0, 1);
    let s = h.pass(&def, &ground(&n, 1));
    assert_eq!(dids(&s), [Did::Moved { from: 0, to: 1 }, Did::Met]);
}

#[test]
fn its_ice_verbs_do_nothing_on_the_map_and_a_stall_ends_the_pass() {
    let n = diamond();
    let (ix, def) = row("10 ATTACK 5\n20 MOVE TOWARD TARGET", 1);
    let mut h = Hunter::wake(ix, &def, 0, 3);
    let s = h.pass(&def, &ground(&n, 4));
    assert!(matches!(dids(&s)[0], Did::Skipped(_)));
    assert_eq!(h.at, 1, "the verb spent nothing; the move did");
    // Round and round spending nothing: a stall.
    let (_, def) = row("10 MOVE TOWARD EXIT\n20 GOTO 10", 1);
    let mut h = Hunter::wake(ix, &def, 4, 0);
    let s = h.pass(&def, &ground(&n, 0));
    assert_eq!(s.last().unwrap().did, Did::Stall);
    assert_eq!(h.at, 4);
    // Out of IFs, the pass is over.
    let (_, def) = row(
        "10 IF TARGET SEEN\n20 WAIT\n30 IF TARGET SEEN\n40 WAIT\n50 IF TARGET SEEN",
        9,
    );
    runs(&mut h, &def);
    let s = h.pass(&def, &ground(&n, 0));
    assert_eq!(s.last().unwrap().did, Did::OutOfRam);
}

#[test]
fn every_builtin_ai_hunts_a_runner_down_a_line() {
    // On a straight line, the AI at the exit end and a runner who never moves: each version
    // reaches them, through its own program only.
    let d = Data::builtin().unwrap();
    let n = net(
        &[&[Entry], &[Ice], &[Vault], &[Ice], &[Exit]],
        &[(0, 1), (1, 2), (2, 3), (3, 4)],
        &[],
    );
    for (ix, def) in d.ice.iter().enumerate().filter(|(_, r)| r.boss) {
        let mut h = Hunter::wake(ix, def, 4, 0);
        let mut met = false;
        for _ in 0..12 {
            if h.pass(def, &ground(&n, 0))
                .iter()
                .any(|s| s.did == Did::Met)
            {
                met = true;
                break;
            }
        }
        assert!(met && h.at == 0, "{} stuck on {}", def.id, h.at);
    }
}

#[test]
fn it_meets_no_one_on_a_subnets_door_while_the_runner_is_inside() {
    let n = diamond();
    let (ix, def) = row("10 MOVE TOWARD TARGET\n20 MOVE TOWARD TARGET", 2);
    let mut h = Hunter::wake(ix, &def, 0, 3);
    // The runner is through the door on 1, inside its subnet.
    let g = Ground {
        here: false,
        ..ground(&n, 1)
    };
    h.seen = Some(1);
    let s = h.pass(&def, &g);
    assert_eq!(h.at, 1);
    assert!(s.iter().all(|s| s.did != Did::Met), "{s:?}");
}
