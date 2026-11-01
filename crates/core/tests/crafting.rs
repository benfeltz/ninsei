use ninsei_core::binary::{
    self, OFF_CHECKSUM, OFF_CPU, OFF_NAME, OFF_OPCODE, OFF_OPT, OFF_POWER, OFF_SIG, OFF_VS,
};
use ninsei_core::hexedit::HexBuffer;
use ninsei_core::*;

/// A run parked on a BBS: in a run, the hex editor only compiles there.
fn game() -> Game {
    let mut g = Game::new(Data::builtin().unwrap(), 0xBEEF);
    let here = g.at;
    g.net.nodes[here].kind = NodeKind::Bbs;
    g
}

fn slot_of(g: &Game, id: &str) -> usize {
    g.hdd
        .iter()
        .position(|s| g.data.warez[s.warez].id == id)
        .unwrap()
}

fn fix_checksum(b: &mut binary::Bin) {
    b[OFF_CHECKSUM] = binary::checksum(b);
}

/// Puts the runner in a fight with ICE `ice_id` on the first linked node.
fn fight(g: &mut Game, ice_id: &str) {
    let target = g.net.nodes[g.at].links[0];
    let ice = g.data.ice.iter().position(|i| i.id == ice_id).unwrap();
    g.net.nodes[target].kind = NodeKind::Ice;
    g.net.nodes[target].ice = Some(ice);
    g.net.nodes[target].cleared = false;
    g.apply(Action::Move(target));
    assert_eq!(g.phase, Phase::Combat);
}

#[test]
fn factory_binaries_decode_to_their_definitions() {
    let g = game();
    for (i, def) in g.data.warez.iter().enumerate() {
        let s = g.factory_slot(i).expect("factory binary is legal");
        assert_eq!(s.spec.effect, def.effect, "{}", def.id);
        assert_eq!((s.spec.cpu, s.spec.ram, s.spec.opt), (def.cpu, def.ram, 0));
        assert_eq!(s.spec.name, def.name);
    }
}

#[test]
fn checksum_must_be_fixed_by_hand() {
    let mut g = game();
    let i = slot_of(&g, "firewall");
    let mut b = g.hdd[i].bin;
    b[OFF_POWER] = 6; // shield 6 costs 3, budget 4: fine
    g.apply(Action::Patch { slot: i, bin: b });
    assert_eq!(
        g.hdd[i].spec.effect,
        Effect::Shield(5),
        "bad checksum must be refused"
    );
    assert!(g.log.last().unwrap().text.contains("checksum"));
    fix_checksum(&mut b);
    g.apply(Action::Patch { slot: i, bin: b });
    assert_eq!(g.hdd[i].spec.effect, Effect::Shield(6));
}

#[test]
fn over_budget_is_refused_and_paying_cpu_buys_power() {
    let mut g = game();
    let i = slot_of(&g, "icepick"); // cpu 1 ram 2: budget 4
    let mut b = g.hdd[i].bin;
    b[OFF_POWER] = 7;
    fix_checksum(&mut b);
    g.apply(Action::Patch { slot: i, bin: b });
    assert!(g.log.last().unwrap().text.contains("over budget"));
    b[OFF_CPU] = 2; // budget 7
    fix_checksum(&mut b);
    g.apply(Action::Patch { slot: i, bin: b });
    assert_eq!(g.hdd[i].spec.effect, Effect::Break { power: 7, vs: None });
    assert_eq!(g.hdd[i].spec.cpu, 2);
}

#[test]
fn opcode_swap_turns_a_shield_into_a_breaker() {
    let mut g = game();
    let i = slot_of(&g, "firewall");
    let mut b = g.hdd[i].bin;
    b[OFF_OPCODE] = 1;
    b[OFF_POWER] = 4;
    fix_checksum(&mut b);
    g.apply(Action::Patch { slot: i, bin: b });
    assert_eq!(g.hdd[i].spec.effect, Effect::Break { power: 4, vs: None });
}

#[test]
fn opt_level_needs_a_valid_signature_but_signatures_can_be_copied() {
    let mut g = game();
    let a = slot_of(&g, "icepick");
    let donor = slot_of(&g, "patch");
    let mut b = g.hdd[a].bin;
    b[OFF_OPT] = 1;
    b[OFF_POWER] = 6; // needs opt 1: budget 4 + 2
    fix_checksum(&mut b);
    g.apply(Action::Patch { slot: a, bin: b });
    assert!(g.log.last().unwrap().text.contains("signature"));

    // Optimize the donor legitimately, then lift its signature.
    g.deck.fragments = 1;
    g.apply(Action::Optimize { slot: donor });
    assert_eq!(g.hdd[donor].spec.opt, 1);
    assert_eq!(g.deck.fragments, 0);
    let sig = &g.hdd[donor].bin[OFF_SIG..OFF_SIG + 2];
    b[OFF_SIG..OFF_SIG + 2].copy_from_slice(sig);
    fix_checksum(&mut b);
    g.apply(Action::Patch { slot: a, bin: b });
    assert_eq!(g.hdd[a].spec.effect, Effect::Break { power: 6, vs: None });
}

#[test]
fn optimizing_needs_fragments() {
    let mut g = game();
    g.apply(Action::Optimize { slot: 0 });
    assert_eq!(g.hdd[0].spec.opt, 0);
}

#[test]
fn signatures_are_per_run() {
    let a = Game::new(Data::builtin().unwrap(), 1);
    let b = Game::new(Data::builtin().unwrap(), 2);
    assert_ne!(
        a.hdd[0].bin[OFF_SIG..OFF_SIG + 2],
        b.hdd[0].bin[OFF_SIG..OFF_SIG + 2]
    );
}

#[test]
fn renaming_works_and_padding_is_checked() {
    let mut g = game();
    let i = slot_of(&g, "patch");
    let mut b = g.hdd[i].bin;
    b[OFF_NAME..OFF_NAME + 15].fill(0);
    b[OFF_NAME..OFF_NAME + 4].copy_from_slice(b"MEND");
    fix_checksum(&mut b);
    g.apply(Action::Patch { slot: i, bin: b });
    assert_eq!(g.hdd[i].spec.file(), "mend.exe");
    b[OFF_NAME + 10] = b'X';
    fix_checksum(&mut b);
    g.apply(Action::Patch { slot: i, bin: b });
    assert_eq!(
        g.hdd[i].spec.name, "MEND",
        "junk after the name must be refused"
    );
}

#[test]
fn growing_ram_cost_unloads_if_it_no_longer_fits() {
    let mut g = game();
    let i = slot_of(&g, "icepick");
    assert!(g.hdd[i].loaded);
    let mut b = g.hdd[i].bin;
    b[binary::OFF_RAM] = 9;
    fix_checksum(&mut b);
    g.apply(Action::Patch { slot: i, bin: b });
    assert_eq!(g.hdd[i].spec.ram, 9);
    assert!(!g.hdd[i].loaded);
    assert!(g.ram_used() <= g.deck.ram_max);
}

#[test]
fn no_patching_mid_fight() {
    let mut g = game();
    fight(&mut g, "icewall");
    let i = slot_of(&g, "firewall");
    let mut b = g.hdd[i].bin;
    b[OFF_POWER] = 6;
    fix_checksum(&mut b);
    g.apply(Action::Patch { slot: i, bin: b });
    assert_eq!(g.hdd[i].spec.effect, Effect::Shield(5));
}

#[test]
fn edited_power_is_what_runs() {
    let mut g = game();
    let i = slot_of(&g, "icepick");
    let mut b = g.hdd[i].bin;
    b[OFF_POWER] = 4;
    fix_checksum(&mut b);
    g.apply(Action::Patch { slot: i, bin: b });
    fight(&mut g, "icewall");
    let before = g.combat.as_ref().unwrap().strength;
    g.apply(Action::Run(i));
    assert_eq!(before - g.combat.as_ref().unwrap().strength, 4);
}

#[test]
fn specialised_breakers_halve_against_other_classes() {
    let mut g = game();
    fight(&mut g, "icewall"); // barrier
    let i = slot_of(&g, "decoder"); // code gate breaker, power 4
    g.hdd[i].loaded = true;
    let before = g.combat.as_ref().unwrap().strength;
    g.apply(Action::Run(i));
    assert_eq!(before - g.combat.as_ref().unwrap().strength, 2);
}

#[test]
fn stun_cannot_land_two_turns_running() {
    let mut g = game();
    g.deck.cpu_max = 9;
    let lag = g.data.warez_ix("lagspike").unwrap();
    let mut s = g.factory_slot(lag).unwrap();
    s.loaded = true;
    g.deck.ram_max = 20;
    g.hdd.push(s);
    let i = g.hdd.len() - 1;
    fight(&mut g, "rototurret");
    g.apply(Action::Run(i));
    assert!(g.combat.as_ref().unwrap().stunned);
    g.apply(Action::EndTurn);
    g.apply(Action::Run(i));
    assert!(
        !g.combat.as_ref().unwrap().stunned,
        "second stun must not land"
    );
    g.apply(Action::EndTurn);
    g.apply(Action::Run(i));
    assert!(
        g.combat.as_ref().unwrap().stunned,
        "after a turn it works again"
    );
}

#[test]
fn hex_buffer_nibbles_and_ascii() {
    let mut h = HexBuffer::open(0, [0u8; binary::LEN]);
    assert!(h.input('a'));
    assert_eq!((h.buf[0], h.cursor, h.low_nibble), (0xA0, 0, true));
    assert!(h.input('F'));
    assert_eq!((h.buf[0], h.cursor), (0xAF, 1));
    assert!(!h.input('z'));
    h.ascii = true;
    h.input('Z');
    assert_eq!(h.buf[1], b'Z');
    assert!(h.dirty() && h.changed(0) && !h.changed(5));
    h.step(-100);
    assert_eq!(h.cursor, 0);
    h.step(100);
    assert_eq!(h.cursor, binary::LEN - 1);
    h.revert();
    assert!(!h.dirty());
}

#[test]
fn bad_vs_on_non_breaker_is_a_fault() {
    let g = game();
    let mut b = g.hdd[slot_of(&g, "firewall")].bin;
    b[OFF_VS] = 2;
    fix_checksum(&mut b);
    let faults = binary::check(&b, g.key()).unwrap_err();
    assert!(faults.iter().any(|f| f.contains("only valid on BRK")));
}
