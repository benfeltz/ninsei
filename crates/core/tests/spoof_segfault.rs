//! 141.3.3: ICE verbs that attack the deck's program, SPOOF and SEGFAULT, and the sniffer.

use ninsei_core::basic::{self, Stmt, Verb, Vocab};
use ninsei_core::data::Intent;

const BOTH: [Verb; 3] = [Verb::Attack, Verb::Spoof, Verb::Segfault];

#[test]
fn spoof_and_segfault_are_an_ices_words() {
    let p = basic::parse_with("10 SPOOF 2\n20 SEGFAULT\n30 ATTACK 1", Vocab::Ice(&BOTH));
    let stmts: Vec<&Stmt> = p.lines.iter().map(|l| &l.stmt).collect();
    assert_eq!(
        stmts,
        [
            &Stmt::Act(Intent::Spoof(2)),
            &Stmt::Act(Intent::Segfault),
            &Stmt::Act(Intent::Attack(1))
        ]
    );
    assert_eq!(p.text(), "10 SPOOF 2\n20 SEGFAULT\n30 ATTACK 1");
    let bad = basic::parse_with("10 SEGFAULT 3\n20 SPOOF", Vocab::Ice(&BOTH));
    assert!(
        bad.fault(10).is_some() && bad.fault(20).is_some(),
        "{bad:?}"
    );
    let deck = basic::parse("10 SPOOF 2\n20 SEGFAULT");
    assert!(deck.fault(10).is_some() && deck.fault(20).is_some());
    assert_eq!(Intent::Spoof(2).describe(), "SPOOF 2");
    assert_eq!(Intent::Segfault.describe(), "SEGFAULT");
}
