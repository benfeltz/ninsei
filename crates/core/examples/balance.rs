//! Balance probe: bots play the real game, single runs and whole careers, and print what they
//! met (`ninsei_core::probe`). The same report is the home Shell's `probe` command.
//! `cargo run -p ninsei-core --example balance [careers] [--kit <id>]`: careers start from a
//! starting kit in `data/hub/starts.ron` (the default if none).
use ninsei_core::Data;
use ninsei_core::probe::{Config, Probe};

fn main() {
    let data = Data::builtin().unwrap();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let kit = args
        .iter()
        .position(|a| a == "--kit")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_default();
    if !kit.is_empty() && !data.starts.kits.iter().any(|k| k.id == kit) {
        let ids: Vec<&str> = data.starts.kits.iter().map(|k| k.id.as_str()).collect();
        eprintln!("no kit `{kit}`. kits: {}", ids.join(", "));
        std::process::exit(2);
    }
    let cfg = match args.first().and_then(|a| a.parse().ok()) {
        Some(n) => Config::careers(n),
        None => Config::FULL,
    };
    let mut probe = Probe::new(&data, cfg).with_kit(&kit);
    while !probe.step(500) {}
    for line in probe.report() {
        println!("{line}");
    }
}
