use ninsei_core::bbs::{Screen, Session};
use ninsei_core::*;

fn game(seed: u64) -> Game {
    Game::new(Data::builtin().unwrap(), seed)
}

/// Routes onto the first link as a MSG node, then forces event `id`.
fn meet(g: &mut Game, id: &str) {
    let to = g.net.nodes[g.at].links[0];
    g.net.nodes[to].kind = NodeKind::Contact;
    g.net.nodes[to].cleared = false;
    g.net.nodes[to].ice = None;
    g.apply(Action::Move(to));
    assert_eq!(g.phase, Phase::Encounter);
    let ev = g.data.events.iter().position(|e| e.id == id).unwrap();
    let enc = g.encounter.as_mut().unwrap();
    enc.event = ev;
    enc.offer = g.data.warez_ix("wyrm");
}

fn choice(g: &Game, pred: impl Fn(&ChoiceEffect) -> bool) -> usize {
    let e = g.encounter.as_ref().unwrap();
    g.data.events[e.event]
        .choices
        .iter()
        .position(|c| pred(&c.effect))
        .unwrap()
}

#[test]
fn you_must_answer_before_moving_and_can_always_leave() {
    let mut g = game(3);
    meet(&mut g, "fence");
    let here = g.at;
    let next = g.net.nodes[here].links[0];
    g.apply(Action::Move(next));
    assert_eq!(g.at, here);
    let leave = g.leave_choice().unwrap();
    g.apply(Action::Choose {
        choice: leave,
        slot: None,
    });
    assert_eq!(g.phase, Phase::Exploring);
    assert!(g.encounter.as_ref().unwrap().done);
    g.apply(Action::EndEncounter);
    assert!(g.encounter.is_none());
    g.apply(Action::Move(next));
    assert_eq!(g.at, next);
}

#[test]
fn fence_trades_a_program_in_place() {
    let mut g = game(3);
    meet(&mut g, "fence");
    let (len, uid) = (g.hdd.len(), g.hdd[1].uid);
    let c = choice(&g, |e| matches!(e, ChoiceEffect::Trade { .. }));
    g.apply(Action::Choose {
        choice: c,
        slot: None,
    });
    assert_eq!(
        g.phase,
        Phase::Encounter,
        "a trade without a slot is refused"
    );
    g.apply(Action::Choose {
        choice: c,
        slot: Some(1),
    });
    assert_eq!(g.hdd.len(), len);
    assert_eq!(g.hdd[1].spec.name, "Wyrm");
    assert_ne!(g.hdd[1].uid, uid);
    assert!(!g.hdd[1].loaded);
    assert!(g.ram_used() <= g.deck.ram_max);
}

#[test]
fn fence_buys_programs_but_not_your_last() {
    let mut g = game(3);
    meet(&mut g, "fence");
    let c = choice(&g, |e| matches!(e, ChoiceEffect::Sell { .. }));
    let price = g.sale_price(0, 20);
    assert_eq!(price, 20 * g.hdd[0].spec.ram);
    let len = g.hdd.len();
    g.apply(Action::Choose {
        choice: c,
        slot: Some(0),
    });
    assert_eq!((g.hdd.len(), g.deck.credits), (len - 1, price));

    let mut g = game(3);
    g.hdd.truncate(1);
    meet(&mut g, "fence");
    g.apply(Action::Choose {
        choice: c,
        slot: Some(0),
    });
    assert_eq!(g.hdd.len(), 1);
}

#[test]
fn paying_needs_the_creds() {
    let mut g = game(5);
    meet(&mut g, "sysop");
    let c = choice(&g, |e| matches!(e, ChoiceEffect::Pay { .. }));
    assert!(g.choice_blocker(c, None).unwrap().contains("needs 60"));
    g.deck.credits = 100;
    g.deck.trace = 30;
    g.apply(Action::Choose {
        choice: c,
        slot: None,
    });
    assert_eq!((g.deck.credits, g.deck.trace), (40, 10));
}

#[test]
fn helping_the_runner_is_a_fight_with_a_bonus() {
    let mut g = game(5);
    meet(&mut g, "pinned");
    let c = choice(&g, |e| matches!(e, ChoiceEffect::Fight { .. }));
    g.apply(Action::Choose {
        choice: c,
        slot: None,
    });
    assert_eq!(g.phase, Phase::Combat);
    assert!(g.encounter.is_none());
    g.combat.as_mut().unwrap().strength = 1;
    let i = g
        .hdd
        .iter()
        .position(|s| s.loaded && matches!(s.spec.effect, Effect::Break { .. }))
        .unwrap();
    let before = g.deck.fragments;
    g.apply(Action::Run(i));
    assert_eq!(g.phase, Phase::Exploring);
    assert!(g.deck.fragments >= before + 2);
}

/// Strips ICE and encounters from every node except `keep`, so walks are quiet.
fn quiet_except(g: &mut Game, keep: usize) {
    for n in &mut g.net.nodes {
        if n.id != keep && n.id != g.at && !matches!(n.kind, NodeKind::Exit | NodeKind::Bbs) {
            n.kind = NodeKind::Vault;
            n.cleared = true;
            n.ice = None;
        }
    }
}

#[test]
fn fixer_contract_pays_when_the_target_falls() {
    let mut g = game(8);
    meet(&mut g, "fixer");
    let c = choice(&g, |e| *e == ChoiceEffect::Contract);
    g.apply(Action::Choose {
        choice: c,
        slot: None,
    });
    assert_eq!(g.contracts.len(), 1);
    let k = g.contracts[0].clone();
    assert_eq!(k.status, ContractStatus::Active);
    assert!(g.net.reachable_from(g.at)[k.node]);
    g.apply(Action::EndEncounter);

    quiet_except(&mut g, k.node);
    g.deck.trace = 0;
    g.deck.integrity = 999;
    while g.at != k.node {
        let target = k.node;
        let next = *g.net.nodes[g.at]
            .links
            .iter()
            .find(|&&n| g.net.reachable_from(n)[target])
            .unwrap();
        g.apply(Action::Move(next));
    }
    assert_eq!(g.phase, Phase::Combat);
    g.combat.as_mut().unwrap().strength = 1;
    let i = g
        .hdd
        .iter()
        .position(|s| s.loaded && matches!(s.spec.effect, Effect::Break { .. }))
        .unwrap();
    let before = g.deck.credits;
    g.apply(Action::Run(i));
    assert_eq!(g.contracts[0].status, ContractStatus::Done);
    let bonus = k.ghost.map_or(0, |(_, b)| b);
    assert!(
        g.deck.credits >= before + k.reward + bonus,
        "reward and ghost bonus paid"
    );
}

#[test]
fn contracts_fail_when_the_target_drops_out_of_reach() {
    for seed in 0..40 {
        let mut g = game(seed);
        meet(&mut g, "fixer");
        let c = choice(&g, |e| *e == ChoiceEffect::Contract);
        if g.choice_blocker(c, None).is_some() {
            continue;
        }
        g.apply(Action::Choose {
            choice: c,
            slot: None,
        });
        g.apply(Action::EndEncounter);
        let target = g.contracts[0].node;
        quiet_except(&mut g, target);
        // Walk away from the target if any route does.
        let Some(&away) = g.net.nodes[g.at]
            .links
            .iter()
            .find(|&&n| !g.net.reachable_from(n)[target])
        else {
            continue;
        };
        g.apply(Action::Move(away));
        assert_eq!(g.contracts[0].status, ContractStatus::Failed);
        return;
    }
    panic!("no seed offered a way to walk away from a contract");
}

#[test]
fn bbs_jobs_board_posts_and_hands_out_contracts() {
    let mut taken = false;
    for seed in 0..60 {
        let mut g = game(seed);
        let bbs = g.net.bbs().unwrap();
        // Quiet the route to the BBS but leave ICE beyond it.
        for n in &mut g.net.nodes {
            if n.layer <= tuning::BBS_LAYER && n.id != bbs && n.layer > 0 {
                n.kind = NodeKind::Vault;
                n.cleared = true;
                n.ice = None;
            }
        }
        while g.at != bbs {
            let next = *g.net.nodes[g.at]
                .links
                .iter()
                .find(|&&n| g.net.reachable_from(n)[bbs])
                .unwrap();
            g.apply(Action::Move(next));
        }
        if g.bbs_jobs.is_empty() {
            continue;
        }
        assert!(g.bbs_jobs.len() <= tuning::BBS_JOBS);
        let mut s = Session {
            screen: Screen::Main,
            status: None,
        };
        s.key(&g, 'j');
        assert_eq!(s.screen, Screen::Jobs);
        let out = s.key(&g, '1');
        assert_eq!(out, bbs::Outcome::Act(Action::TakeJob { job: 0 }));
        g.apply(Action::TakeJob { job: 0 });
        assert_eq!(g.contracts.len(), 1);
        g.apply(Action::TakeJob { job: 0 });
        assert_eq!(g.contracts.len(), 1, "a job is taken once");
        let lines = s.render(&g);
        assert!(lines.iter().all(|l| ansi::visible_len(l) <= bbs::COLS));
        taken = true;
        break;
    }
    assert!(taken, "no seed posted a job");
}

#[test]
fn every_choice_in_every_event_resolves() {
    for seed in 0..20 {
        let data = Data::builtin().unwrap();
        for (ev_ix, ev) in data.events.iter().enumerate() {
            for c in 0..ev.choices.len() {
                let mut g = game(seed);
                g.deck.credits = 500;
                meet(&mut g, &ev.id);
                g.encounter.as_mut().unwrap().event = ev_ix;
                let slot = g.choice_needs_slot(c).then_some(0);
                if g.choice_blocker(c, slot).is_some() {
                    continue;
                }
                g.apply(Action::Choose { choice: c, slot });
                assert_ne!(
                    g.phase,
                    Phase::Encounter,
                    "{} choice {c} left you stuck",
                    ev.id
                );
            }
        }
    }
}

#[test]
fn labels_fill_their_placeholders() {
    let mut g = game(2);
    meet(&mut g, "fence");
    let ev = &g.data.events[g.encounter.as_ref().unwrap().event];
    for (i, c) in ev.choices.iter().enumerate() {
        let l = g.fill(&c.label, Some(i));
        assert!(!l.contains('{'), "{l}");
    }
    for t in &ev.text {
        assert!(!g.fill(t, None).contains('{'));
    }
}
