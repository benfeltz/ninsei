//! CRT display presets and the curved-glass geometry the mouse has to follow.

use ninsei_core::display::{Effects, Raster, screen_to_content};
use ninsei_core::hub::Save;
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

fn effects(id: &str) -> Effects {
    Effects::from_def(&data().display(id).unwrap().effects)
}

fn flat() -> Effects {
    let mut e = effects("amber");
    e.screen_curvature = 0.0;
    e.frame_size = 0.0;
    e.screen_radius = 0.0;
    e
}

#[test]
fn the_six_presets() {
    let d = data();
    let ids: Vec<&str> = d.displays.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(
        ids,
        ["amber", "paperwhite", "green", "c64", "apple2", "vga"]
    );
    for p in &d.displays {
        let t = p.theme.as_deref().unwrap();
        assert!(d.theme(t).is_some(), "{} pairs with a real theme", p.id);
    }
    assert_eq!(effects("c64").rasterization, Raster::Scanline);
    assert_eq!(effects("amber").frame_color, [0xcf as f32 / 255.0; 3]);
}

#[test]
fn a_flat_screen_maps_one_to_one() {
    let e = flat();
    assert!(!e.warps());
    for p in [[0.0, 0.0], [0.25, 0.75], [1.0, 1.0]] {
        assert_eq!(screen_to_content(p, &e), Some(p));
    }
}

#[test]
fn curved_glass_pushes_the_picture_outward() {
    let e = effects("c64");
    assert!(e.warps());
    let mid = screen_to_content([0.5, 0.5], &e).unwrap();
    assert!(
        (mid[0] - 0.5).abs() < 1e-6 && (mid[1] - 0.5).abs() < 1e-6,
        "the centre stays put"
    );
    let r = screen_to_content([0.75, 0.5], &e).unwrap();
    let l = screen_to_content([0.25, 0.5], &e).unwrap();
    assert!(r[0] > 0.75, "off-centre points show content further out");
    assert!((r[0] - 0.5 - (0.5 - l[0])).abs() < 1e-5, "symmetric");
}

#[test]
fn the_bezel_and_tube_corners_are_not_screen() {
    let e = effects("c64");
    assert_eq!(screen_to_content([0.005, 0.5], &e), None, "bezel");
    assert_eq!(screen_to_content([0.999, 0.999], &e), None, "corner");
}

#[test]
fn saves_start_on_the_white_monitor() {
    let d = data();
    let mut s = Save::load("(version: 1)").unwrap();
    s.ensure_hardware(&d);
    assert_eq!((s.display_id(&d), s.crt), ("paperwhite", true));
    // Crisp glass for 1-bit pictures: no colour fringing, a gentle curve.
    let e = effects("paperwhite");
    assert_eq!(e.rgb_shift, 0.0);
    assert!(e.screen_curvature < effects("amber").screen_curvature);
}

#[test]
fn bad_presets_are_caught() {
    let base = include_str!("../../../data/displays.ron");
    let parse = |displays: &str| {
        Data::parse(Sources {
            displays,
            ..Sources::builtin()
        })
    };
    assert!(
        parse(&base.replacen("bloom: 0.6", "bloom: 1.6", 1))
            .unwrap_err()
            .contains("0.0-1.0")
    );
    assert!(
        parse(&base.replacen("Some(\"c64\")", "Some(\"zx81\")", 1))
            .unwrap_err()
            .contains("zx81")
    );
    assert!(
        parse(&base.replacen("\"#999999\"", "\"grey\"", 1))
            .unwrap_err()
            .contains("frame colour")
    );
}
