//! 141.3.2 acceptance, made strict by 141.11.1: through the real save and run, a fight shows an
//! ICE's program only from the source the save owns, and the ICE DB prints the same.

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
        .map(|r| r.text.clone())
        .collect()
}

fn deck(key: u64) -> Save {
    let d = data();
    let mut s = Save::default();
    s.ensure_key(key);
    s.ensure_phonebook(&d, 0);
    s.ensure_hardware(&d);
    s
}

#[test]
fn a_fight_shows_nothing_and_a_source_shows_it_all_from_the_first_turn() {
    let d = data();
    let mut s = deck(0x3132);
    s.tools.push("icedb".into());
    // First meeting, no source: nothing, however long the fight.
    let mut g = fight_with(&mut s, "tollbooth");
    for _ in 0..4 {
        assert!(shown(&g).is_empty());
        g.apply(Action::EndTurn);
    }
    g.phase = Phase::JackedOut;
    s.record(&g, &[], &d.rig).unwrap();
    let page = |s: &Save| {
        icedb::entries(s, &d)
            .into_iter()
            .find(|e| e.def.id == "tollbooth")
            .unwrap()
            .lines(s, &d)
            .join("\n")
    };
    let row = d.ice.iter().find(|i| i.id == "tollbooth").unwrap();
    let all: Vec<String> = ice_listing::decompile_def(&row.program, &row.verbs)
        .into_iter()
        .map(|r| r.text)
        .collect();
    let p = page(&s);
    assert!(p.contains("met            1"), "the record is yours: {p}");
    assert!(all.iter().all(|t| !p.contains(t.as_str())), "{p}");
    // With its source, the next fight lists all of it from its first turn, and so does the DB.
    assert!(s.learn_source(&d, "tollbooth", ice_version::Version::FIRST));
    let g = fight_with(&mut s, "tollbooth");
    assert_eq!(shown(&g), all);
    let p = page(&s);
    assert!(all.iter().all(|t| p.contains(t.as_str())), "{p}");
}

#[test]
fn every_deck_marks_the_same_line_its_own_way() {
    let mut s = deck(0x3133);
    assert!(s.learn_source(&data(), "enigma", ice_version::Version::FIRST));
    let mut g = fight_with(&mut s, "enigma");
    g.apply(Action::EndTurn);
    let rows = ice_listing::rows(&g);
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
