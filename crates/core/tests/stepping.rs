//! 141.8 acceptance: the deck's program runs a line at a time on every deck, played through the
//! real save and run (`Save::jack_in`, `Game::apply`). Step moves the ▶ one line, Run walks it
//! turn after turn, Finish is the rest of the turn, and a hang is shown on each deck's own
//! surface until the next Step.

use ninsei_core::game::{ExecState, StepWhat};
use ninsei_core::hub::Save;
use ninsei_core::listing::{self, Gutter};
use ninsei_core::*;
use ninsei_core::{btop_fight, stack_fight, suite};

const DECKS: [&str; 3] = ["sensenet", "onosendai", "hosaka"];

/// A run on `deck` with its kit's program, in a fight with ICE too strong to fall.
fn fight_on(deck: &str) -> Game {
    let d = Data::builtin().unwrap();
    let mut s = Save::default();
    s.ensure_key(0x5AFE);
    s.ensure_phonebook(&d, 0);
    s.ensure_hardware(&d);
    if !s.decks.iter().any(|x| x == deck) {
        s.decks.push(deck.into());
    }
    s.deck = deck.into();
    let mut g = s.jack_in(&d, 21);
    let target = g.net.nodes[g.at].links[0];
    let ice = g.data.ice.iter().position(|i| i.id == "bulwark").unwrap();
    g.net.nodes[target].kind = NodeKind::Ice;
    g.net.nodes[target].ice = Some(ice);
    g.net.nodes[target].cleared = false;
    g.apply(Action::Move(target));
    assert_eq!(g.phase, Phase::Combat, "{deck}");
    let c = g.combat.as_mut().unwrap();
    (c.strength, c.strength_max) = (500, 500);
    g
}

/// The line the listing marks as about to run.
fn current(g: &Game) -> Option<u32> {
    listing::rows(g)
        .iter()
        .find(|r| r.gutter == Gutter::Current)
        .and_then(|r| r.no)
}

fn turn(g: &Game) -> u32 {
    g.combat.as_ref().unwrap().turn
}

#[test]
fn on_every_deck_step_walks_the_cursor_a_line_at_a_time() {
    for deck in DECKS {
        let mut g = fight_on(deck);
        g.apply(Action::Program(
            "10 REM breaker first\n20 IF INTEG > 0\n30 RUN icepick\n40 GOTO 20".into(),
        ));
        assert_eq!(current(&g), Some(20), "{deck}: never on the REM");
        let (cpu, ifs) = (g.combat.as_ref().unwrap().cpu, g.ifs_left());
        g.apply(Action::Step);
        assert_eq!(current(&g), Some(30), "{deck}");
        assert_eq!(g.ifs_left(), ifs - 1, "{deck}: the IF took its GB");
        g.apply(Action::Step);
        assert_eq!(current(&g), Some(40), "{deck}");
        assert!(
            g.combat.as_ref().unwrap().cpu < cpu,
            "{deck}: the RUN took cycles"
        );
        assert_eq!(turn(&g), 1, "{deck}: the ICE hasn't acted");
        // Step on until the line that spends the last cycle: the ICE acts, the ▶ to the top.
        while turn(&g) == 1 {
            g.apply(Action::Step);
        }
        assert_eq!(
            g.exec.trace.last().unwrap().what,
            StepWhat::OutOfCpu,
            "{deck}"
        );
        assert_eq!(current(&g), Some(20), "{deck}");
    }
}

#[test]
fn on_every_deck_run_walks_turn_after_turn_and_pause_holds_the_place() {
    for deck in DECKS {
        let mut g = fight_on(deck);
        g.apply(Action::Program(
            "10 IF INTEG > 0\n20 RUN icepick\n30 GOTO 10".into(),
        ));
        g.apply(Action::Play);
        assert_eq!(g.exec.state, ExecState::Running, "{deck}");
        // The driver's beats: a Step each while it runs.
        g.apply(Action::Step);
        g.apply(Action::Pause);
        let held = (current(&g), g.combat.as_ref().unwrap().cpu, g.ifs_left());
        assert_eq!(held.0, Some(30), "{deck}: paused mid-pass");
        g.apply(Action::Play);
        while turn(&g) < 3 && g.exec.state == ExecState::Running {
            g.apply(Action::Step);
        }
        assert_eq!(turn(&g), 3, "{deck}: carried on into the next turn");
        assert_eq!(g.exec.state, ExecState::Running, "{deck}");
    }
}

#[test]
fn on_every_deck_finish_is_the_rest_of_the_turn() {
    for deck in DECKS {
        let mut g = fight_on(deck);
        g.apply(Action::Program(
            "10 IF INTEG > 0\n20 RUN icepick\n30 GOTO 10".into(),
        ));
        g.apply(Action::Step);
        g.apply(Action::Finish);
        assert_eq!(turn(&g), 2, "{deck}");
        assert_eq!(current(&g), Some(10), "{deck}");
        assert_eq!(
            g.exec.state,
            ExecState::Paused,
            "{deck}: Finish leaves it paused"
        );
        let lines: Vec<u32> = g.exec.trace.iter().map(|s| s.line).collect();
        assert_eq!(lines[0], 10, "{deck}: one pass, from the top: {lines:?}");
    }
}

#[test]
fn on_every_deck_safe_hangs_where_you_look_until_the_next_step() {
    for deck in DECKS {
        let mut g = fight_on(deck);
        while g.exec.broke.is_none() {
            g.apply(Action::Step);
        }
        let n = g.exec.broke.unwrap();
        assert_eq!(current(&g), Some(n), "{deck}: the ▶ held on the line");
        assert_eq!(turn(&g), 1, "{deck}: the turn is the player's");
        let card = stack_fight::card(&g);
        assert_eq!(card.broke_words(), Some(format!("?BREAK IN {n}")), "{deck}");
        assert!(
            btop_fight::note(&g).ends_with(&format!("?break in {n}")),
            "{deck}"
        );
        assert!(
            suite::module_note(&g).ends_with(&format!("?BREAK IN {n}")),
            "{deck}"
        );
        g.apply(Action::Step);
        assert_eq!(listing::broke(&g), None, "{deck}: the next Step lets go");
        assert!(!btop_fight::note(&g).contains("break"), "{deck}");
    }
}
