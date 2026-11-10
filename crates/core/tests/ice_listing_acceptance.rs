//! 141.3.2 acceptance: the deck shows the ICE's program as it has seen it run, through the real
//! save and run, and the ICE DB keeps it between runs.

use ninsei_core::hub::Save;
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

/// A run from `save`, in a fight with `ice`, strong enough to last.
fn fight_with(save: &mut Save, ice: &str) -> Game {
    let d = data();
    let mut g = save.jack_in(&d, 11);
    let target = g.net.nodes[g.at].links[0];
    let ix = g.data.ice.iter().position(|i| i.id == ice).unwrap();
    g.net.nodes[target].kind = NodeKind::Ice;
    g.net.nodes[target].ice = Some(ix);
    g.net.nodes[target].cleared = false;
    g.apply(Action::Move(target));
    let c = g.combat.as_mut().unwrap();
    (c.strength, c.strength_max) = (500, 500);
    g.apply(Action::Program(String::new()));
    g
}

fn shown(g: &Game) -> Vec<String> {
    ice_listing::rows(g)
        .iter()
        .map(|r| r.shown().to_string())
        .collect()
}

#[test]
fn a_fight_fills_in_the_listing_and_the_ice_db_carries_it_to_the_next() {
    let d = data();
    let mut s = Save::default();
    s.ensure_key(0x3132);
    s.ensure_phonebook(&d, 0);
    s.ensure_hardware(&d);
    s.tools.push("icedb".into());
    // First meeting: nothing known.
    let mut g = fight_with(&mut s, "tollbooth");
    assert!(shown(&g).iter().all(|t| t == ice_listing::UNSEEN));
    for _ in 0..4 {
        g.apply(Action::EndTurn);
    }
    let after = shown(&g);
    assert!(after.iter().any(|t| t != ice_listing::UNSEEN), "{after:?}");
    g.phase = Phase::JackedOut;
    s.record(&g, &[], &d.rig).unwrap();
    // The ICE DB page shows the same lines.
    let entry = icedb::entries(&s, &d)
        .into_iter()
        .find(|e| e.def.id == "tollbooth")
        .unwrap();
    let page = entry.lines(&s, &d).join("\n");
    for t in after.iter().filter(|t| *t != ice_listing::UNSEEN) {
        assert!(page.contains(t.as_str()), "{t} on {page}");
    }
    // Next run, the same ICE: known from its first turn.
    let g = fight_with(&mut s, "tollbooth");
    assert_eq!(shown(&g), after);
}

#[test]
fn every_deck_says_the_same_listing_its_own_way() {
    let d = data();
    let mut s = Save::default();
    s.ensure_key(0x3133);
    s.ensure_phonebook(&d, 0);
    s.ensure_hardware(&d);
    let mut g = fight_with(&mut s, "enigma");
    g.apply(Action::EndTurn);
    let rows = ice_listing::rows(&g);
    let seen = rows.iter().filter(|r| r.text.is_some()).count();
    assert_eq!(
        ice_listing::heading(&rows),
        format!("its program: {seen} of 4 lines seen")
    );
    assert_eq!(btop_fight::ice_heading(&rows), format!("PROG {seen}/4"));
    assert_eq!(
        suite::procedure_title(&rows),
        format!("OBSERVED PROCEDURE ({seen} OF 4 STEPS)")
    );
    // Each marks the same line as the one the ICE carries on from.
    let cursor = rows.iter().find(|r| r.cursor).unwrap().no;
    assert!(btop_fight::ice_window(&rows, 1)[0].cursor);
    let hosaka = suite::procedure(&rows);
    assert!(
        hosaka
            .iter()
            .any(|r| r.step.starts_with('>') && r.step.ends_with(&cursor.to_string()))
    );
}
