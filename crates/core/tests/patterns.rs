//! The 1-bit pattern library the Sense/Net deck shades with.

use ninsei_core::data::Sources;
use ninsei_core::pattern::{PatternDef, REQUIRED, SIZE};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

#[test]
fn the_library_has_what_the_parts_need() {
    let d = data();
    for id in REQUIRED {
        let p = d.pattern(id).unwrap_or_else(|| panic!("no {id}"));
        assert_eq!(p.rows.len(), SIZE);
    }
    // The greys are the greys they say.
    let pct = |id: &str| d.pattern(id).unwrap().coverage_pct();
    assert_eq!(
        (pct("p12"), pct("p25"), pct("p50"), pct("p75")),
        (12, 25, 50, 75)
    );
    // Fine patterns fall back to coarser ones.
    let p50 = d.pattern("p50").unwrap();
    let fb = d.pattern(p50.fallback.as_deref().unwrap()).unwrap();
    assert!(fb.coverage_pct() < p50.coverage_pct());
}

#[test]
fn tiles_repeat() {
    let d = data();
    let p = d.pattern("p50").unwrap();
    assert!(p.ink(0, 0) && !p.ink(1, 0) && p.ink(1, 1));
    for (x, y) in [(3, 5), (12, 7), (100, 33)] {
        assert_eq!(p.ink(x, y), p.ink(x % SIZE, y % SIZE));
    }
}

#[test]
fn bad_patterns_are_caught() {
    let src = include_str!("../../../data/patterns.ron");
    let err = |s: String| {
        Data::parse(Sources {
            patterns: &s,
            ..Sources::builtin()
        })
        .unwrap_err()
    };
    assert!(err(src.replacen("\"#...#...\",", "\"#...#..\",", 1)).contains("8x8"));
    assert!(err(src.replacen("\"#...#...\",", "\"#...#..x\",", 1)).contains("only `#` and `.`"));
    assert!(err(src.replace("(id: \"dots\",", "(id: \"bricks\",")).contains("twice"));
    assert!(
        err(src.replace("fallback: Some(\"p25\")", "fallback: Some(\"nope\")"))
            .contains("unknown `nope`")
    );
    assert!(err(src.replace("(id: \"bricks\",", "(id: \"tiles\",")).contains("no `bricks`"));
    let one = PatternDef {
        id: "x".into(),
        label: "x".into(),
        rows: vec!["########".into(); SIZE],
        fallback: None,
    };
    assert_eq!(one.coverage_pct(), 100);
}
