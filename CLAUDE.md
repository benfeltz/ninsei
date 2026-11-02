# ninsei

Cyberspace hacking roguelike styled as a retro IDE. Rust workspace: `crates/core` (pure rules) and `crates/app` (eframe/egui + egui_dock).

## Commands

| Purpose | Command |
|---|---|
| Run | `cargo run -p ninsei [-- <hexseed>]` |
| Tests | `cargo test --workspace` |
| Lint | `cargo clippy --workspace --all-targets` (keep it warning-free) |
| Format | `cargo fmt --all` |
| Balance probe | `cargo run -p ninsei-core --example balance` (default network and each tier) |
| Web build | `scripts/build-web.sh` (needs the wasm32 target and `wasm-bindgen-cli` matching Cargo.lock) |
| Web lint | `cargo clippy -p ninsei --target wasm32-unknown-unknown` |

CI (`.github/workflows/ci.yml`) runs fmt, clippy (native and wasm) and tests on every push; `pages.yml` deploys the web build on pushes to `main`.

## Working from issues

Features and bugs come in as GitHub issues (forms in `.github/ISSUE_TEMPLATE/`). One issue per PR; the PR body says `Closes #N` so merging closes it. Read the issue and its comments first, and ask in chat if it's ambiguous. Don't comment on issues unless asked. Bug reports carry a seed: replay it before fixing.

## Rules

- **Core never imports egui.** Rules, generation, BBS navigation and ANSI parsing live in `ninsei-core` and are tested headlessly. The app sends `Action`s to `Game::apply` and renders state; panes push requests and never mutate the game while drawing.
- **Content lives in `data/`, never in code.** Add warez/ICE/loot/BBS files there; `Data::validate` must catch any bad cross-reference.
- **Balance numbers live in `crates/core/src/tuning.rs`.** After changing them, run the balance probe; the `runs_always_end` test fails if a dumb bot always or never wins.
- **Every colour comes from the theme** in `data/themes.ron` (`Data::validate` checks them). One theme (`Save::phosphor`) colours both the desktop and runs; every theme must work in both. In a run that's the `Palette` (`crates/app/src/theme.rs`); on the desktop it's the `Desk` palette register in `workbench.rs`, loaded each frame from the desktop theme and read through `backdrop()`/`ink()`/`shadow()`/`accent()`/`muted()`/`fill()`. Never hard-code a colour in either. The BBS window's true CGA colours and the crash screen's red are the only exceptions.
- **Saves never break.** The hub's save (`crates/core/src/hub.rs`) is one versioned RON document. New fields get `#[serde(default)]` so old saves still load; bump `SAVE_VERSION` only for incompatible changes. An unreadable save starts a fresh hub, never a crash.
- **The hub's rules live in core** (`hub.rs`: save, screen flow, Shell commands); `workbench.rs` and `crash.rs` only draw and route input. Desktop icons are pixel art in `data/hub/icons.ron`.
- **BBS screens are pipe-coded text, max 78x24**, enforced by `every_screen_fits_the_terminal`.
- **l33t goes through one translator** (`crates/core/src/leet.rs`, `leet::translate`). It's one character in, one out, so layouts keep their width; it leaves pipe codes, digits, bracketed hotkeys and 8.3 filenames alone. The BBS applies it to every screen but the splash and the modem lines; an event opts in with `leet: true`.
- **Every encounter needs a `Leave` choice** (`Data::validate`), and `every_choice_in_every_event_resolves` must keep passing: no choice may leave the runner stuck in `Phase::Encounter`.
- **The app runs native and in the browser.** No `std::time::SystemTime`, threads, files or `std::process` in code the web build reaches; gate native-only code with `#[cfg(not(target_arch = "wasm32"))]`.
- **A modal owns the keyboard.** Windows that take keys (BBS, talk) ignore input on the frame they open and call `stop_text_input` so the terminal can't eat their keys.
- **Programs are tracked by `Slot::uid`, not index**, wherever the UI holds on to one across frames: trades and sales reshuffle the HDD.
- **Two signing keys.** In a run, binaries are signed with the run's key (`Game::key`); in the stash, with the save's key (`Save::key`). Programs are re-sealed whenever they cross between home and a run (`Save::kit`/`Game::start` on the way in, `Save::record` on the way out); nothing else may re-sign a binary except optimize.
- **Networks are data.** Tiers, traits and companies live in `data/hub/networks.ron` (`NetworksDef::validate`). A trait is a `TraitEffect` that `Save::site_params` turns into `NetParams` (generation) or `RunMods` (ICE strength, trace %, pay %); add new effects there, not as special cases in the run. Default `NetParams` must keep generating the same networks for the same seed. A trait's `Shape` sets the topology (widths, cross links, chokepoints); tiers and the `Subnets` effect place subnet nodes.
- **Subnets are side trips.** Entering a `Subnet` node swaps in a smaller network (`Network::subnet`, seeded from the run and node) and parks the main one in `Game::outer`; the subnet's exit (the UPLINK) puts you back on the subnet node. Contracts always point into the main network: use `Game::root`/`root_at`, never `game.net`, for contract nodes.
- **Phone ranges are data too.** `data/hub/exchanges.ron` defines ranges (`RangeDef::validate`); `Drop::Hint(range)` teaches one, from loot, BBS files or encounters, and `Data::validate` rejects hints at unknown ranges. A wardial block always answers the same way for a save (seeded from `Save::key`, range and block), so results never depend on when you scan.
- **Run vs home.** In a run, `Action::Patch`/`Optimize` only work at a BBS (`Game::compile_blocker`). Hub tool windows (`hubtools.rs`) never touch the save directly: they emit `WbOut`s that the app applies through `Save`'s methods.
- **A program's stats come from its 32-byte binary** (`crates/core/src/binary.rs`), never from `WarezDef` after creation. A slot never holds an invalid binary: `Action::Patch` runs `binary::check` first. Every factory program must fit the budget rule; `Data::validate` enforces it.
