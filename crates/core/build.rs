//! Finds every BBS under `data/bbs/<id>/` (a `board.ron` and a `splash.txt`) and every sound
//! pack under `data/sounds/<id>.ron`, and compiles them in, so a new board is a new folder and a
//! new sound pack a new file.

use std::fmt::Write;
use std::path::Path;

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/bbs");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut dirs: Vec<_> = std::fs::read_dir(&root)
        .expect("data/bbs")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.join("board.ron").is_file())
        .collect();
    dirs.sort();
    let mut out = String::from("&[\n");
    for d in &dirs {
        println!("cargo:rerun-if-changed={}", d.display());
        let id = d.file_name().unwrap().to_string_lossy();
        let path = |f: &str| {
            format!(
                "{:?}",
                d.join(f).canonicalize().unwrap().display().to_string()
            )
        };
        let splash = if d.join("splash.txt").is_file() {
            format!("include_str!({})", path("splash.txt"))
        } else {
            "\"\"".into()
        };
        writeln!(
            out,
            "    BoardSrc {{ id: {id:?}, board: include_str!({}), splash: {splash} }},",
            path("board.ron")
        )
        .unwrap();
    }
    out.push(']');
    let dest = Path::new(&std::env::var("OUT_DIR").unwrap()).join("boards.rs");
    std::fs::write(dest, out).unwrap();
    sound_packs();
}

fn sound_packs() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/sounds");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut files: Vec<_> = std::fs::read_dir(&root)
        .expect("data/sounds")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "ron"))
        .collect();
    files.sort();
    let mut out = String::from("&[\n");
    for f in &files {
        println!("cargo:rerun-if-changed={}", f.display());
        let id = f.file_stem().unwrap().to_string_lossy();
        let path = format!("{:?}", f.canonicalize().unwrap().display().to_string());
        writeln!(
            out,
            "    PackSrc {{ id: {id:?}, sounds: include_str!({path}) }},"
        )
        .unwrap();
    }
    out.push(']');
    let dest = Path::new(&std::env::var("OUT_DIR").unwrap()).join("sounds.rs");
    std::fs::write(dest, out).unwrap();
}
