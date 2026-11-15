//! 138.3: beating the net's AI: the payout, the net's sources with its notes, its ICE DB page.

use ninsei_core::boss::Hunter;
use ninsei_core::game::BossState;
use ninsei_core::hub::{Entry, Save};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

fn save(tier: u32) -> Save {
    let mut s = Save::default();
    s.ensure_key(0x1383);
    s.ensure_hardware(&data());
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

/// A run on `s`'s number with the AI awake and holding the runner's next node: stepping on
/// opens its fight.
fn facing_it(s: &mut Save, seed: u64) -> Game {
    let d = data();
    let mut g = s.dial(&d, 0, seed).unwrap();
    let to = g.net.nodes[g.at].links[0];
    g.net.nodes[to].kind = NodeKind::Vault;
    g.net.nodes[to].ice = None;
    g.net.nodes[to].cleared = true;
    let ice = g.boss.as_ref().unwrap().ice;
    let mut h = Hunter::wake(ice, &g.data.ice[ice], to, g.at);
    h.prog = basic::parse_with("10 HOLD", g.data.ice[ice].vocab());
    g.boss.as_mut().unwrap().state = BossState::Awake(h);
    g.apply(Action::Move(to));
    assert_eq!(g.phase, Phase::Combat);
    g
}

/// Beats the AI in front of the runner.
fn beat(g: &mut Game) {
    g.combat.as_mut().unwrap().strength = 1;
    let pick = g.slot_named("icepick").unwrap();
    g.apply(Action::Run(pick));
    assert_eq!(
        g.boss.as_ref().unwrap().state,
        BossState::Down,
        "{:?}",
        g.log.last()
    );
}

#[test]
fn beating_it_pays_the_nets_big_payout_once() {
    for tier in 1..=3 {
        let mut s = save(tier);
        let mut g = facing_it(&mut s, 3);
        let before = g.deck.credits;
        let name = g.data.ice[g.boss.as_ref().unwrap().ice].name.clone();
        beat(&mut g);
        let pay = g.mods.pay(tuning::AI_PAY[tier as usize - 1]);
        let line = format!("+ {pay}¢ from {name}'s accounts.");
        assert_eq!(
            g.log.iter().filter(|l| l.text == line).count(),
            1,
            "tier {tier}"
        );
        assert!(
            g.deck.credits >= before + pay,
            "the payout and whatever the loot rolled"
        );
    }
}

#[test]
fn losing_to_it_pays_nothing() {
    let mut s = save(1);
    let mut g = facing_it(&mut s, 4);
    g.deck.integrity = 1;
    for _ in 0..20 {
        if g.phase != Phase::Combat {
            break;
        }
        g.apply(Action::EndTurn);
    }
    assert_eq!(g.phase, Phase::Flatlined);
    assert!(!g.log.iter().any(|l| l.text.contains("'s accounts")));
}

#[test]
fn beaten_it_gives_up_the_source_of_every_kind_on_its_net_and_its_own() {
    let mut s = save(2);
    let mut g = facing_it(&mut s, 5);
    let ai = g.boss.as_ref().unwrap().ice;
    let model = g.data.boss_model(ai).unwrap().id.clone();
    let mut kinds: Vec<usize> = g.net.nodes.iter().filter_map(|n| n.ice).collect();
    kinds.sort_unstable();
    kinds.dedup();
    assert!(!kinds.is_empty());
    beat(&mut g);
    for &k in kinds.iter().chain([&ai]) {
        let id = &g.data.ice[k].id;
        let v = g.ice_version(k);
        let got = g.sources_got.iter().find(|s| s.id == *id && s.version == v);
        let got = got.unwrap_or_else(|| panic!("no source of {id} {v}"));
        assert_eq!(got.from, model);
    }
    assert_eq!(g.sources_got.len(), kinds.len() + 1);
    // They come home even when the run then ends in a flatline.
    g.deck.trace = tuning::TRACE_MAX;
    let to = g.net.nodes[g.at].links[0];
    g.apply(Action::Move(to));
    assert_eq!(g.phase, Phase::Flatlined);
    s.record(&g, &[], &data().rig).unwrap();
    for &k in kinds.iter().chain([&ai]) {
        assert!(
            s.ice_sources.iter().any(|x| x.id == g.data.ice[k].id),
            "{}",
            g.data.ice[k].id
        );
    }
}

#[test]
fn a_source_it_gave_up_lists_with_its_notes() {
    let d = data();
    let ai = d.boss_for(3, 0).unwrap();
    let model = d.boss_model(ai).unwrap();
    let hydra = d.ice.iter().find(|i| i.id == "hydra").unwrap();
    let (header, remarks) =
        ice_listing::notes(&d, &model.id, hydra, ice_version::Version::FIRST).unwrap();
    assert!(header.iter().any(|l| l.contains(&model.name)), "{header:?}");
    assert!(header.iter().all(|l| l.chars().count() <= data::NOTE_MAX));
    // Hydra's `IF TARGET RAN firewall` gets the AI's remark on TARGET RAN.
    let p = basic::parse_with(&hydra.program, hydra.vocab());
    let rows = ice_listing::decompile(&p);
    let all = ice_listing::annotate(rows, header.clone(), remarks);
    let rem = data::remark_on(remarks, "IF TARGET RAN firewall").unwrap();
    assert!(all.iter().any(|r| r.text.contains(rem)), "{all:?}");
}

#[test]
fn validate_refuses_bad_ai_notes() {
    let ai = include_str!("../../../data/ai.ron");
    let bad = [
        (
            r#"header: ["{ice} REV. {version}","#,
            r#"header: ["{ice} REV. {version} {nope}","#,
            "can't fill",
        ),
        (
            r#"(read: "TURN",          text: "COUNTERS: LONG SESSIONS"),"#,
            r#"(read: "TURN",          text: "COUNTERS: VERY VERY LONG SESSIONS INDEED"),"#,
            "wider than",
        ),
        (
            r#"(read: "TURN",          text: "COUNTERS: LONG SESSIONS"),"#,
            r#"(read: "TARGET MOOD",   text: "COUNTERS: MOODS"),"#,
            "no ICE reads",
        ),
        (
            r#""PREPARED BY {ai}","#,
            r#""PREPARED BY {ai} FOR {ice} OF THE NETWORK","#,
            "wider than",
        ),
    ];
    for (find, replace, why) in bad {
        assert!(ai.contains(find), "{find}");
        let err = Data::parse(Sources {
            ai: &ai.replacen(find, replace, 1),
            ..Sources::builtin()
        })
        .unwrap_err();
        assert!(err.contains(why), "{why}: {err}");
    }
}

#[test]
fn beaten_its_ice_db_page_lists_its_program_with_its_notes() {
    let d = data();
    let mut s = save(3);
    let mut g = facing_it(&mut s, 6);
    let ai = g.boss.as_ref().unwrap().ice;
    beat(&mut g);
    // The run ends (traced out on the next hop): what it gave up comes home all the same.
    g.deck.trace = tuning::TRACE_MAX;
    let to = g.net.nodes[g.at].links[0];
    g.apply(Action::Move(to));
    s.record(&g, &[], &d.rig).unwrap();
    let entries = icedb::entries(&s, &d);
    let e = entries.iter().find(|e| e.def.id == d.ice[ai].id).unwrap();
    let page = e.lines(&s, &d).join("\n");
    let model = d.boss_model(ai).unwrap();
    assert!(page.contains("SEGFAULT"), "its own source lists: {page}");
    // Its own program is its maker's release, not a revision it drafted, and its name carries
    // the version: no 1.0.0 anywhere on its page.
    assert!(
        page.contains(&format!("{} {}", model.company, d.ice[ai].name)),
        "its maker's header: {page}"
    );
    assert!(page.contains("source         on file"), "{page}");
    assert!(
        !page.contains("PREPARED BY") && !page.contains("1.0.0"),
        "{page}"
    );
    // And a kind from its net, listed with the AI's notes rather than a crew's.
    let kind = d
        .ice
        .iter()
        .find(|i| !i.boss && s.ice_sources.iter().any(|x| x.id == i.id))
        .unwrap();
    let ke = entries.iter().find(|e| e.def.id == kind.id).unwrap();
    let kp = ke.lines(&s, &d).join("\n");
    assert!(kp.contains("REVIEWED AND APPROVED"), "{kp}");
}

#[test]
fn one_source_is_one_source() {
    let d = data();
    let mut s = save(1);
    // The deck owns every kind on file already: only the AI's own is new.
    for i in d.ice.iter().filter(|i| !i.boss) {
        assert!(s.learn_source(&d, &i.id, ice_version::Version::FIRST));
    }
    let mut g = facing_it(&mut s, 7);
    let name = g.data.ice[g.boss.as_ref().unwrap().ice].name.clone();
    beat(&mut g);
    assert!(
        g.log
            .iter()
            .any(|l| l.text == format!("+ 1 source from {name}'s revision store.")),
        "{:?}",
        g.log.iter().rev().take(5).collect::<Vec<_>>()
    );
}

#[test]
fn validate_holds_the_ais_notes_to_the_boards_rules() {
    let ai = include_str!("../../../data/ai.ron");
    let bad = [
        // A read that's only part of a word never matches a line.
        (
            r#"(read: "TARGET CPU",    text: "COUNTERS: SPENT INTRUDERS"),"#,
            r#"(read: "TARGET C",      text: "COUNTERS: SPENT INTRUDERS"),"#,
            "no ICE reads",
        ),
        // Its own header, too.
        (
            r#"own: ["{company} {ai}", "LICENSED SOFTWARE. NOT FOR RESALE.","#,
            r#"own: ["{company} {ai} {ice}", "LICENSED SOFTWARE. NOT FOR RESALE.","#,
            "can't fill",
        ),
        // A model may not share a board's id: a source's origin would mean both.
        (
            r#"(id: "warden", company: "Hosaka""#,
            r#"(id: "gutter", company: "Hosaka""#,
            "a board's id",
        ),
    ];
    for (find, replace, why) in bad {
        assert!(ai.contains(find), "{find}");
        let mut text = ai.replacen(find, replace, 1);
        if why == "a board's id" {
            text = text.replace(r#"model: "warden""#, r#"model: "gutter""#);
        }
        let err = Data::parse(Sources {
            ai: &text,
            ..Sources::builtin()
        })
        .unwrap_err();
        assert!(err.contains(why), "{why}: {err}");
    }
}
