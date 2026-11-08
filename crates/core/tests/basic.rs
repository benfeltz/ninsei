//! The deck's BASIC: parsing and printing programs, and running them in a fight.

use ninsei_core::basic::{self, Cond, Move, Op, Stmt, Var};

const SAMPLE: &str = "10 RUN icepick\n20 IF INTEG < 10\n30 RUN patch\n40 GOTO 10";

#[test]
fn parses_the_sample() {
    let p = basic::parse(SAMPLE);
    let stmts: Vec<&Stmt> = p.lines.iter().map(|l| &l.stmt).collect();
    assert_eq!(
        stmts,
        [
            &Stmt::Run("icepick".into()),
            &Stmt::If(Cond::Cmp(Var::Integ, Op::Lt, 10)),
            &Stmt::Run("patch".into()),
            &Stmt::Goto(10),
        ]
    );
    assert_eq!(
        p.lines.iter().map(|l| l.no).collect::<Vec<_>>(),
        [10, 20, 30, 40]
    );
    assert!(p.lines.iter().all(|l| p.fault(l.no).is_none()));
}

#[test]
fn bad_lines_are_kept_and_named() {
    let p = basic::parse("10 RUN\n20 JUMP 10\nRUN icepick\n30 IF INTEG 10\n40 STOP");
    assert_eq!(p.lines.len(), 5, "{p:?}");
    for no in [10, 20, 21, 30] {
        let why = p
            .fault(no)
            .unwrap_or_else(|| panic!("line {no} should be bad: {p:?}"));
        assert!(why.starts_with('?'), "{why}");
    }
    assert_eq!(p.line(21).map(|l| l.text.as_str()), Some("RUN icepick"));
    assert_eq!(p.line(40).map(|l| &l.stmt), Some(&Stmt::Stop));
}

#[test]
fn text_round_trips() {
    let p = basic::parse(SAMPLE);
    assert_eq!(p.text(), SAMPLE);
    assert_eq!(basic::parse(&p.text()), p);
    let messy = basic::parse("40 GOTO 10\n  10   RUN icepick  \n20 IF INTEG<10\n10 RUN decoder");
    assert_eq!(messy.text(), "10 RUN decoder\n20 IF INTEG<10\n40 GOTO 10");
    assert_eq!(
        messy.line(20).map(|l| &l.stmt),
        Some(&Stmt::If(Cond::Cmp(Var::Integ, Op::Lt, 10)))
    );
}

#[test]
fn keywords_are_case_insensitive() {
    let p =
        basic::parse("10 run IcePick\n20 if next attack\n30 Goto 10\n40 stop\n50 if ice codegate");
    assert_eq!(
        p.line(10).map(|l| &l.stmt),
        Some(&Stmt::Run("IcePick".into()))
    );
    assert_eq!(
        p.line(20).map(|l| &l.stmt),
        Some(&Stmt::If(Cond::Next(Move::Attack)))
    );
    assert_eq!(p.line(30).map(|l| &l.stmt), Some(&Stmt::Goto(10)));
    assert_eq!(p.line(40).map(|l| &l.stmt), Some(&Stmt::Stop));
    assert_eq!(
        p.line(50).map(|l| &l.stmt),
        Some(&Stmt::If(Cond::IceClass(ninsei_core::IceClass::CodeGate)))
    );
}

#[test]
fn goto_to_a_missing_line_is_bad() {
    let mut p = basic::parse("10 RUN icepick\n20 GOTO 15");
    assert!(p.fault(20).is_some_and(|w| w.contains("15")));
    p.set_line(15, "STOP");
    assert_eq!(p.fault(20), None);
    p.remove(15);
    assert!(p.fault(20).is_some());
}

#[test]
fn edits_insert_replace_and_remove() {
    let mut p = basic::parse(SAMPLE);
    p.set_line(25, "REM patch when low");
    p.set_line(10, "RUN decoder");
    p.set_line(40, "");
    assert_eq!(
        p.text(),
        "10 RUN decoder\n20 IF INTEG < 10\n25 REM patch when low\n30 RUN patch"
    );
    assert_eq!(p.after(20), Some(25));
    assert_eq!(p.after(30), None);
}
