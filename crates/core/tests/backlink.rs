//! Backlink: a hack that turns around the one-way links into the node you stand on (task 111.1).

use ninsei_core::hub::{self, Save};
use ninsei_core::*;

fn data() -> std::sync::Arc<Data> {
    std::sync::Arc::new(Data::builtin().unwrap())
}

/// A save with one phonebook entry of `tier` with exactly `traits`.
fn site(tier: u32, traits: &[&str]) -> Save {
    let mut s = Save::default();
    s.ensure_key(7);
    s.phonebook.push(hub::Entry {
        id: 0,
        name: "probe".into(),
        business: "test".into(),
        number: "555-0000".into(),
        tier,
        traits: traits.iter().map(|t| t.to_string()).collect(),
        heat: 0,
        runs: 0,
        last: Vec::new(),
    });
    s
}

const TRAITS: [&[&str]; 5] = [
    &[],
    &["mesh"],
    &["chokepoints"],
    &["daisy_chain"],
    &["segmented"],
];

/// Every network and subnet of every tier over a spread of seeds, with a share of one-way
/// links pushed up so there's plenty to turn.
fn nets() -> Vec<Network> {
    let d = data();
    let mut out = Vec::new();
    for tier in 1..=3 {
        for traits in TRAITS {
            for seed in 0..25u64 {
                let g = site(tier, traits).dial(&d, 0, seed).unwrap();
                for pct in [g.params.one_way(), 60] {
                    let mut net = g.net.clone();
                    net.orient(seed, pct);
                    out.push(net);
                }
                for n in g.net.nodes.iter().filter(|n| n.kind == NodeKind::Subnet) {
                    out.push(Network::subnet(seed, n.id, &d, &g.params));
                }
            }
        }
    }
    out
}

/// The link between `a` and `b` as (one_way, reversed), on whichever end holds it.
fn way(net: &Network, a: usize, b: usize) -> (bool, bool) {
    let (s, d) = if net.nodes[a].links.contains(&b) {
        (a, b)
    } else {
        (b, a)
    };
    (net.one_way(s, d), net.reversed(s, d))
}

/// At every node of every network: after a Backlink, the exit is reachable from every node,
/// so from every node the runner could get to; what turned is exactly the one-way links into
/// the node less the ones refused; each turned link now leads out of the node; every other
/// link is as it was.
#[test]
fn a_backlink_never_cuts_anyone_off_from_the_exit() {
    let (mut turned, mut refused) = (0, 0);
    for net in nets() {
        assert!(
            net.reaches_exit().iter().all(|&r| r),
            "a fresh net reaches the exit"
        );
        for at in 0..net.nodes.len() {
            let into = net.one_way_into(at);
            assert!(into.iter().all(|&(_, to)| to == at));
            let mut after = net.clone();
            let flips = after.backlink(at);
            assert_eq!(
                flips,
                net.backlinks(at),
                "the preview's flips are the run's"
            );
            assert!(
                flips.iter().all(|f| into.contains(f)),
                "only links into here turn"
            );
            turned += flips.len();
            refused += into.len() - flips.len();
            assert!(
                after.reaches_exit().iter().all(|&r| r),
                "the exit stays reachable from every node"
            );
            let from_here = after.routes_from(at);
            assert!(from_here[after.exit()], "the exit is ahead of the runner");
            for &(a, _) in &flips {
                assert!(after.routes(at).contains(&a), "a turned link leads out");
                assert!(!after.routes(a).contains(&at), "and no longer in");
            }
            // A refused link is still one way in; nothing else moved.
            for n in &net.nodes {
                for &b in &n.links {
                    let flipped = flips.contains(&(n.id, b)) || flips.contains(&(b, n.id));
                    if !flipped {
                        assert_eq!(way(&after, n.id, b), way(&net, n.id, b));
                    }
                }
            }
        }
    }
    assert!(turned > 100, "plenty turned: {turned}");
    assert!(refused > 0, "some turns would strand someone: {refused}");
}

/// A link turned around is turned back by a Backlink from its other end, when that keeps
/// the exit reachable; two-way links are never touched.
#[test]
fn a_second_backlink_from_the_far_end_turns_it_back() {
    let mut back = 0;
    for net in nets() {
        for at in 0..net.nodes.len() {
            let mut after = net.clone();
            for (a, b) in after.backlink(at) {
                let before = way(&net, a, b);
                let mut again = after.clone();
                if again.backlink(a).contains(&(b, a)) {
                    assert_eq!(way(&again, a, b), before, "turned back as it was");
                    back += 1;
                }
            }
            for n in &net.nodes {
                for &b in &n.links {
                    if way(&net, n.id, b) == (false, false) {
                        assert_eq!(way(&after, n.id, b), (false, false), "two way stays");
                    }
                }
            }
        }
    }
    assert!(back > 100, "{back}");
}

/// Nothing points one way into the entry, and a Backlink there turns nothing; it draws
/// from no RNG.
#[test]
fn nothing_to_turn_turns_nothing() {
    for net in nets() {
        let entry = net.entry();
        assert!(net.one_way_into(entry).is_empty());
        let mut after = net.clone();
        assert!(after.backlink(entry).is_empty());
    }
}

/// The map draws a turned link from its deeper end, one way, so its arrow points back up.
#[test]
fn the_map_draws_a_turned_link_pointing_back() {
    let d = data();
    let mut drawn = 0;
    for seed in 0..80u64 {
        let mut g = Game::new(d.clone(), seed);
        g.net.orient(seed, 70);
        // Stood a layer in, on whichever node has the most to turn.
        let to = *g.net.layers[1]
            .iter()
            .max_by_key(|&&n| g.net.backlinks(n).len())
            .unwrap();
        g.at = to;
        let flips = g.net.backlink(to);
        drawn += flips.len();
        for (a, b) in flips {
            let l = netview::links(&g)
                .into_iter()
                .find(|l| (l.from, l.to) == (b, a))
                .expect("drawn from here to there");
            assert!(l.one_way);
            assert_eq!(l.route, Some(a), "a route out of here");
        }
        let reach = g.reachable();
        for l in netview::links(&g) {
            if let Some(r) = l.route {
                assert!(reach.contains(&r));
                assert!(g.route(r).is_ok());
            }
        }
        for r in reach {
            assert!(g.route(r).is_ok(), "the move agrees with reachable");
        }
    }
    assert!(drawn > 5, "{drawn}");
}

/// A run with Backlink on the disk, stood a layer in on the node with the most to turn.
fn on_the_map(seed: u64) -> (Game, usize) {
    let mut g = Game::new(data(), seed);
    g.net.orient(seed, 70);
    let w = g.data.warez_ix("backlink").unwrap();
    let mut s = g.factory_slot(w).unwrap();
    s.uid = 900;
    g.hdd.push(s);
    let at = *g.net.layers[1]
        .iter()
        .max_by_key(|&&n| g.net.backlinks(n).len())
        .unwrap();
    g.at = at;
    let slot = g.hdd.len() - 1;
    (g, slot)
}

/// The first seed whose run has something to turn.
fn ready() -> (Game, usize) {
    (0..200)
        .map(on_the_map)
        .find(|(g, _)| !g.net.backlinks(g.at).is_empty())
        .unwrap()
}

#[test]
fn on_the_map_backlink_does_what_its_preview_said() {
    let mut turned = 0;
    for seed in 0..60 {
        let (mut g, slot) = on_the_map(seed);
        g.mods.trace_pct = 150;
        let flips = g.net.backlinks(g.at);
        let before = (g.deck.trace, g.hops, g.at);
        match g.preview(slot) {
            Ok(p) => {
                let fight::Outcome::Backlink { links, trace } = p.outcome else {
                    panic!("{p:?}")
                };
                assert_eq!(p.cpu, 0, "a hack spends no cpu");
                assert_eq!(links as usize, flips.len());
                assert_eq!(
                    trace,
                    g.mods.trace(tuning::COST_BACKLINK),
                    "scaled as a hop is"
                );
                assert_eq!(trace, 18);
                g.apply(Action::Run(slot));
                assert_eq!(g.deck.trace, before.0 + trace, "its only cost is the trace");
                assert_eq!((g.hops, g.at), (before.1, before.2), "it moves no one");
                assert_eq!(g.hacks, 1);
                for &(a, _) in &flips {
                    assert!(g.reachable().contains(&a), "the turned link leads out");
                }
                assert!(g.cues.contains(&cue::Cue::Hack), "{:?}", g.cues);
                assert_eq!(
                    g.history.runs.last().map(|r| r.0),
                    Some(900),
                    "history has it"
                );
                turned += 1;
            }
            Err(why) => {
                assert!(flips.is_empty(), "{why}");
                g.apply(Action::Run(slot));
                assert_eq!(g.deck.trace, before.0, "refused costs nothing");
                assert_eq!(g.hacks, 0);
                assert!(g.log.last().unwrap().text.contains(&why));
            }
        }
    }
    assert!(turned > 10, "{turned}");
}

#[test]
fn a_backlink_can_trace_you_out() {
    let (mut g, slot) = ready();
    g.deck.trace = tuning::TRACE_MAX - 1;
    g.apply(Action::Run(slot));
    assert_eq!(g.phase, Phase::Flatlined);
    assert!(g.cues.contains(&cue::Cue::Flatline));
}

#[test]
fn nothing_to_turn_is_refused_in_the_preview_s_words() {
    let (mut g, slot) = ready();
    g.at = g.net.entry();
    let why = g.preview(slot).unwrap_err();
    assert!(why.starts_with("nothing runs one way into"), "{why}");
    g.apply(Action::Run(slot));
    assert_eq!(g.deck.trace, 0);
}

#[test]
fn a_backlink_is_refused_in_a_fight_and_while_someone_talks() {
    let (mut g, slot) = ready();
    g.phase = Phase::Encounter;
    assert_eq!(
        g.preview(slot).unwrap_err(),
        "someone's still talking. answer them first."
    );
    assert_eq!(g.run_control(slot), None);
    g.phase = Phase::Exploring;
    assert_eq!(g.run_control(slot), Some(true));
    // Into a fight, on the node a link ahead.
    let ahead = g.reachable()[0];
    let ice = g.data.ice.iter().position(|i| i.tier == 1).unwrap();
    let n = &mut g.net.nodes[ahead];
    (n.kind, n.ice, n.cleared) = (NodeKind::Ice, Some(ice), false);
    g.apply(Action::Move(ahead));
    assert_eq!(g.phase, Phase::Combat);
    let why = g.preview(slot).unwrap_err();
    assert_eq!(
        why,
        "Backlink runs on the map. ICE is holding the connection."
    );
    assert_eq!(g.run_control(slot), Some(false), "shown, not live");
    let (trace, net) = (g.deck.trace, format!("{:?}", g.net.nodes));
    g.apply(Action::Run(slot));
    assert_eq!((g.deck.trace, format!("{:?}", g.net.nodes)), (trace, net));
    assert!(g.log.last().unwrap().text.contains(&why));
    // A script line that runs it is skipped with the same words, and the pass goes on.
    g.apply(Action::Program("10 RUN backlink\n20 RUN icepick".into()));
    assert!(
        g.starved().contains(&10),
        "the editor marks it: it never runs here"
    );
    g.apply(Action::Step);
    let said: Vec<String> = g.exec.trace.iter().map(|s| g.step_text(s)).collect();
    assert!(
        said.iter()
            .any(|s| s == &format!("10 RUN backlink: skipped. {why}")),
        "{said:?}"
    );
}

#[test]
fn other_programs_are_still_refused_on_the_map() {
    let (g, _) = ready();
    for slot in 0..g.hdd.len() {
        if !g.hdd[slot].spec.effect.is_hack() {
            assert_eq!(g.preview(slot).unwrap_err(), "nothing to run against.");
            assert_eq!(g.run_control(slot), None);
        }
    }
}
