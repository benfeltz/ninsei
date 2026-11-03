//! Balance probe: bots play the real game, single runs and whole careers, and print what they
//! met (`ninsei_core::probe`). The same report is the home Shell's `probe` command.
//! `cargo run -p ninsei-core --example balance [careers]`
use ninsei_core::Data;
use ninsei_core::probe::{Config, Probe};

fn main() {
    let data = Data::builtin().unwrap();
    let cfg = match std::env::args().nth(1).and_then(|a| a.parse().ok()) {
        Some(n) => Config::careers(n),
        None => Config::FULL,
    };
    let mut probe = Probe::new(&data, cfg);
    while !probe.step(500) {}
    for line in probe.report() {
        println!("{line}");
    }
}
