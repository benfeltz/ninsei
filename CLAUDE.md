# ninsei

Cyberspace hacking roguelike styled as a retro IDE. Rust workspace: `crates/core` (pure rules) and `crates/app` (eframe/egui + egui_dock).

## Commands

| Purpose | Command |
|---|---|
| Run | `cargo run -p ninsei [-- <hexseed>]` |
| Tests | `cargo test --workspace` |
| Lint | `cargo clippy --workspace --all-targets` (keep it warning-free) |
| Format | `cargo fmt --all` |
| Balance probe | `cargo run -p ninsei-core --example balance` |

## Rules

- **Core never imports egui.** Rules, generation, BBS navigation and ANSI parsing live in `ninsei-core` and are tested headlessly. The app sends `Action`s to `Game::apply` and renders state; panes push requests and never mutate the game while drawing.
- **Content lives in `data/`, never in code.** Add warez/ICE/loot/BBS files there; `Data::validate` must catch any bad cross-reference.
- **Balance numbers live in `crates/core/src/tuning.rs`.** After changing them, run the balance probe; the `runs_always_end` test fails if a dumb bot always or never wins.
- **Every colour comes from the phosphor `Palette`** (`crates/app/src/theme.rs`). The BBS window is the one exception: it shows true CGA colours unless switched to phosphor.
- **BBS screens are pipe-coded text, max 78x24**, enforced by `every_screen_fits_the_terminal`.
- **A program's stats come from its 32-byte binary** (`crates/core/src/binary.rs`), never from `WarezDef` after creation. A slot never holds an invalid binary: `Action::Patch` runs `binary::check` first. Every factory program must fit the budget rule; `Data::validate` enforces it.
