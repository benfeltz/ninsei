//! 138.4 acceptance: through real dialed runs (`Save::dial`, `Game::apply`) on every tier,
//! against the task's acceptance criteria: the AI waking files no dialog on any deck; no map
//! style writes "is on this card" and a node the AI holds shows the AI, not its ICE; beating it
//! on an ICE node is the only fight there for the rest of the run, and the preview said so; and
//! its payout is still `AI_PAY`, once.

use ninsei_core::boss::Hunter;
use ninsei_core::game::BossState;
use ninsei_core::hub::{Entry, Save};
use ninsei_core::netview::{self, MapView};
use ninsei_core::*;

fn save(tier: u32) -> Save {
    let mut s = Save {
        program: Some(probe::CAREFUL.into()),
        ..Save::default()
    };
    s.ensure_key(0x1384);
    s.phonebook.push(Entry {
        id: 0,
        name: "probe".into(),
        business: "test".into(),
        number: "555-0000".into(),
        tier,
        traits: Vec::new(),
        heat: 0,
        runs: 0,
        last: Vec::new(),
    });
    s
}

const VIEWS: [MapView; 3] = [MapView::Iso, MapView::Draft, MapView::Paint];

/// A dialed run with the runner's first route made an ICE node of an ordinary kind, never
/// fought, and the AI awake on it, holding. `None` when the entry has no route.
fn ai_on_ice(d: &Data, tier: u32, seed: u64) -> Option<(Game, usize)> {
    let mut g = save(tier).dial(d, 0, seed).unwrap();
    let to = *g.reachable().first()?;
    let ice = d.ice.iter().position(|i| !i.boss && i.tier == 1).unwrap();
    let n = &mut g.net.nodes[to];
    (n.kind, n.ice, n.cleared) = (NodeKind::Ice, Some(ice), false);
    let ai = g.boss.as_ref().unwrap().ice;
    let mut h = Hunter::wake(ai, &g.data.ice[ai], to, g.at);
    h.prog = basic::parse_with("10 HOLD", g.data.ice[ai].vocab());
    g.boss.as_mut().unwrap().state = BossState::Awake(h);
    Some((g, to))
}

/// Beats the AI in the fight under way: weakened, the deck's ICEPICK finishes it.
fn beat_it(g: &mut Game) {
    assert!(g.data.ice[g.combat.as_ref().unwrap().ice].boss);
    g.combat.as_mut().unwrap().strength = 1;
    let pick = g.slot_named("icepick").unwrap();
    g.apply(Action::Run(pick));
}

#[test]
fn beating_it_on_an_ice_node_is_the_only_fight_there_on_every_tier() {
    let d = Data::builtin().unwrap();
    let mut beaten = 0;
    for tier in 1..=3 {
        for seed in 0..15 {
            let Some((mut g, to)) = ai_on_ice(&d, tier, seed) else {
                continue;
            };
            let ai = g.boss.as_ref().unwrap().ice;
            let name = g.data.ice[ai].name.clone();
            // The preview said so: the hop meets the AI, and names only it.
            assert!(g.route(to).unwrap().meets, "tier {tier} seed {seed}");
            assert_eq!(
                netview::route_tip(&g, to).unwrap(),
                format!("jack {to}  →  {name}")
            );
            let before = g.deck.credits;
            g.apply(Action::Move(to));
            assert_eq!(g.phase, Phase::Combat, "tier {tier} seed {seed}");
            beat_it(&mut g);
            beaten += 1;
            // Down, and no second fight: the node is clear, its ICE gone and unpaid.
            assert_eq!(g.boss.as_ref().unwrap().state, BossState::Down);
            assert_eq!(g.phase, Phase::Exploring, "tier {tier} seed {seed}");
            let n = &g.net.nodes[to];
            assert!(n.cleared && n.replaced && n.beaten.is_none());
            assert!(!g.log.iter().any(|l| l.text.contains("node clear")));
            // Its payout, once, by tier, and the black ICE loot is all that rides on it.
            let pay = g.mods.pay(tuning::AI_PAY[tier as usize - 1]);
            let line = format!("+ {pay}¢ from {name}'s accounts.");
            assert_eq!(g.log.iter().filter(|l| l.text == line).count(), 1);
            assert!(g.deck.credits >= before + pay);
            // For the rest of the run: on and back by every two-way link, nothing wakes there.
            let back: Vec<usize> = g.net.nodes[to]
                .links
                .iter()
                .copied()
                .filter(|&b| !g.net.one_way(to, b))
                // On the main network: a subnet's door or the exit takes the runner off it.
                .filter(|&b| !matches!(g.net.nodes[b].kind, NodeKind::Subnet | NodeKind::Exit))
                .collect();
            for b in back {
                if g.phase != Phase::Exploring || g.at != to {
                    break;
                }
                g.apply(Action::Move(b));
                if g.phase != Phase::Exploring || g.in_subnet() {
                    break;
                }
                let hop = g.route(to).unwrap();
                assert!(
                    !hop.rearms && !hop.meets,
                    "tier {tier} seed {seed}: {hop:?}"
                );
                g.apply(Action::Move(to));
                assert_eq!(
                    g.phase,
                    Phase::Exploring,
                    "tier {tier} seed {seed}: {:?} {:?}",
                    g.encounter,
                    g.log
                        .iter()
                        .rev()
                        .take(6)
                        .map(|l| &l.text)
                        .collect::<Vec<_>>()
                );
            }
        }
    }
    assert!(beaten >= 40, "beaten {beaten}");
}

#[test]
fn a_node_the_ai_holds_shows_the_ai_and_no_style_says_is_on_this_card() {
    let d = Data::builtin().unwrap();
    let mut held = 0;
    for tier in 1..=3 {
        for seed in 0..15 {
            let Some((g, to)) = ai_on_ice(&d, tier, seed) else {
                continue;
            };
            let m = netview::ai_mark(&g).unwrap();
            assert!(m.holds(to), "tier {tier} seed {seed}: {m:?}");
            held += 1;
            // The AI's name where the node's ICE tag was, and nothing of the ICE.
            assert_eq!(netview::tag(&g, to), m.label);
            for view in VIEWS {
                assert_eq!(netview::ai_caption(&m, view), None, "{view:?}");
            }
            // Out of sight, the node is its own again, and every caption says last seen.
            let mut gone = g.clone();
            let exit = boss::exit(&gone.net).unwrap();
            let b = gone.boss.as_mut().unwrap();
            b.known = Some(to);
            if let BossState::Awake(h) = &mut b.state {
                h.at = exit;
            }
            if let Some(last) = netview::ai_mark(&gone).filter(|m| !m.now) {
                assert_ne!(netview::tag(&gone, to), last.label);
                for view in VIEWS {
                    let cap = netview::ai_caption(&last, view).unwrap();
                    assert!(!cap.contains("is on this card"), "{view:?}: {cap}");
                    assert!(cap.to_lowercase().contains("last seen"), "{view:?}: {cap}");
                }
            }
        }
    }
    assert!(held >= 40, "held {held}");
}

#[test]
fn its_waking_files_no_dialog_on_any_tier() {
    let d = Data::builtin().unwrap();
    let mut woke = 0;
    for tier in 1..=3 {
        for seed in 0..15 {
            let mut g = save(tier).dial(&d, 0, seed).unwrap();
            let mut watch = suite::Watch::default();
            watch.step(&g);
            // Walk on until it wakes (the hop that crosses its threshold), watching every hop.
            let wake = g.boss.as_ref().unwrap().wake;
            for _ in 0..12 {
                if g.phase != Phase::Exploring || g.ai_woke().is_some() {
                    break;
                }
                let Some(&to) = g.reachable().first() else {
                    break;
                };
                g.deck.trace = g.deck.trace.max(wake.saturating_sub(1));
                g.apply(Action::Move(to));
                assert!(
                    watch.step(&g).iter().all(|n| n.title != "SECURITY NOTICE"),
                    "tier {tier} seed {seed}"
                );
            }
            if let Some((name, _)) = g.ai_woke() {
                woke += 1;
                let line = format!("!! {name} is up on the net.");
                assert_eq!(g.log.iter().filter(|l| l.text == line).count(), 1);
                assert!(watch.step(&g).is_empty(), "tier {tier} seed {seed}");
            }
        }
    }
    assert!(woke >= 30, "woke {woke}");
}
