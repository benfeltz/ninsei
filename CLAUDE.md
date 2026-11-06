# ninsei

Cyberspace hacking roguelike styled as a retro IDE. Rust workspace: `crates/core` (pure rules), `crates/term` (the terminals' line editor) and `crates/app` (eframe/egui + egui_dock).

## Read first

- [`docs/design.md`](docs/design.md): how the game should look and feel: the pillars, how theme, monitor and deck combine, and each deck's point of view. The **Ono-Sendai** is the hacker deck (Omarchy, btop, cyberpunk), the **Hosaka** the corpo deck (a productivity suite), the **Sense/Net** the starter (early Mac, HyperCard). Read it before building UI, and add a deck there before adding it to the data.
- [`docs/architecture.md`](docs/architecture.md): where things live and how they connect (the hub, desktops, tools, boards, chromes, panes, the map, terminals, sound). Read the section for the area you're changing before you change it, and update it when the shape changes.
- Skills in `.claude/skills/` carry the taste: `fake-app-craft` before building or redesigning a screen, `ninsei-ux-review` to judge one from screenshots, `ninsei-game-design` for mechanics and progression (see `.claude/skills/README.md`).

## Commands

| Purpose | Command |
|---|---|
| Run | `cargo run -p ninsei [-- <hexseed>]` |
| Tests | `cargo test --workspace` |
| Lint | `cargo clippy --workspace --all-targets` (keep it warning-free) |
| Format | `cargo fmt --all` |
| Balance probe | `cargo run --release -p ninsei-core --example balance [careers]` (in game: `probe [n]` in the home Shell) |
| Web build | `scripts/build-web.sh` (needs the wasm32 target and `wasm-bindgen-cli` matching Cargo.lock) |
| Web lint | `cargo clippy -p ninsei --target wasm32-unknown-unknown` |

CI (`.github/workflows/ci.yml`) runs fmt, clippy (native and wasm) and tests on every push; `pages.yml` deploys the web build on pushes to `main`.

## Working from tasks

Tasks are Obsidian notes in `~/Documents/obsidian/Ben/notes/ninsei/` (one folder note per task, stacked work as numbered sub-task folders; `ninsei.md` there has the order and the numbering). The note is the source of truth: read its Description and Log before planning, and record decisions in its Log. GitHub issues are no longer used; tasks numbered up to 133 came from issues of the same number, and a PR for one of those may still say `Closes #N`. A PR names its task note in the body. Ask in chat if a task is ambiguous. Bug notes carry a seed: replay it before fixing.

## Definition of done

A PR is done when all of these hold:

- `cargo fmt --all`, both clippies and `cargo test --workspace` are clean.
- Every UX change ships with tests: what a screen shows is decided in core and tested there; how it behaves (focus, keys, clicks, what opens where) is tested headlessly in the app (`home.rs`'s and `liza.rs`'s tests are the pattern). A UX bug gets a test that fails before the fix. Screenshots judge the look; tests prove it works.
- If pixels changed, screenshots of the result (both a mono and a colour theme, the deck that owns the screen) are in the PR and `docs/shots/` is refreshed.
- If `tuning.rs` or the content that feeds it changed, the balance probe ran and the PR quotes before and after.
- `docs/architecture.md` and `docs/design.md` say what the code now does.

## Rules

What must stay true. The how is in `docs/architecture.md`.

- **Core never imports egui.** Rules live in `ninsei-core` and are tested headlessly; the app sends `Action`s to `Game::apply` and renders state. Panes push requests and never mutate the game while drawing.
- **Content lives in `data/`, never in code**, and `Data::validate` must reject every bad cross-reference. Boards, tools, networks, ranges, hardware, add-ons, panes, kits, sound packs, cracktros, stacks and map styles are all data; a new one is a file, not a special case in code.
- **Balance numbers live in `crates/core/src/tuning.rs`.** After changing them, run the probe; `runs_always_end` must keep passing.
- **The probe plays the real game**, through `Save` and `Game::apply`, never a model of the rules. When the game gains a way to progress, teach the career bot to use it, or the probe stops telling the truth. History and graphs read the game and never change play: the probe must not move when they do.
- **Every colour comes from the theme** (`data/themes.ron`), in runs and on the desktop, and every theme must work in both. The BBS window's true CGA colours and the crash screen's red are the only exceptions.
- **Looks are hardware, hardware is look only.** Monitors, decks and firmware are data; a deck's look is built from named parts (map view, chrome, desktop, sound pack). Add a new look as a new part, never as a special case on a deck id. No stats on hardware.
- **Saves load or start fresh, never crash.** New fields get `#[serde(default)]`; an unreadable save starts a fresh hub. There are no players' saves to keep yet, so no legacy defaults or migrations unless asked.
- **The hub's rules live in core.** Desktops, tool windows and the run's panes only draw and route input; they emit `WbOut`s or `Action`s and never touch the save or the game directly.
- **Desktops are front-ends on one home.** A new OS is a `Desktop` variant, a `DesktopView` renderer and a registry entry; layout and focus rules are core and tested there.
- **Chromes are plug-ins and panes are modular.** A new chrome overrides only what it changes in `ChromeView`; a new pane is a variant, an id, a catalog entry and a renderer dressed by each chrome. Never test the chrome in a pane; ask the view.
- **Tools are downloaded, never built in.** Until owned, a tool isn't there at all: no icon, menu item, tab, pane, command or hint. Only the boards' textfiles say where to get them, and a fresh phonebook always reaches the starter tools.
- **The shop is a magazine and it's data.** CPU Shopper prices nothing itself: every item is priced in its own data file and appears in the magazine on its own; `shopper.ron` only dresses the issue. Buying goes through one `Item` and `Save::buy_item`, and buying never plugs in, switches to or installs what you bought.
- **Books share one page control** (`pager.rs`): a fixed page, one bar, the same keys. A new book is a page renderer, never its own controls.
- **The OS never coaches.** Help goes where the real software put it; the world's text (boards, posts, textfiles) teaches. Nothing hints at what the player hasn't found.
- **Everything speaks from where it is.** Every text has an author inside the world, and it knows only what that author would know. A magazine is printed by a publisher who has never seen your deck, so it can name products, boards and numbers but never your settings app or a key to press; a BBS knows its own menus and its callers, not your desktop; an OS knows its own windows and tools, not what a magazine printed; a cracktro knows its crew and its board. Game-side words (pane, deck mod, theme, the names of our apps) only appear where that world would use them. When a screen needs to tell the player something its author couldn't know, that's the wrong screen: put it where the in-world author would.
- **Reading opens doors, and discovery goes through core.** Boards, starters, hints and tools are learned by actions that core records (`Save::record` books them even on a flatline).
- **The preview is the rule.** `Game::preview` says what an action will do or why it can't, and the action resolves through it, so a hint can never disagree with what happens. New risk shows in trace or integrity before it bites.
- **Two signing keys**, the run's and the save's; programs are re-sealed when they cross between home and a run and nowhere else except optimize. A program's stats come from its 32-byte binary, never from `WarezDef` after creation, and a slot never holds an invalid binary. The UI tracks programs by `Slot::uid`, never index.
- **Same seed, same game.** Choosing a board, cues, history and `always` areas never touch the run's RNG; wardial blocks and home listings are seeded from the save, so nothing depends on when you play.
- **BBS screens are pipe-coded text, max 78x24**, drawn with the board's colours, and `every_screen_fits_the_terminal` keeps them so. l33t goes through one width-keeping translator. Every encounter needs a `Leave` choice, and no choice may strand the runner.
- **Sound is synthesized, never sampled.** Dial text and the handshake come from one timeline; run sounds are cues from `Game::apply`, with their recipes in data.
- **Text-mode screens lay out in whole cells**, GUI screens in integer pixels; nothing truncates at a clip rect (see `fake-app-craft`).
- **Damage glitches are visual only.** They garble log text, never meters or numbers, keep the mouse geometry, stay under the flicker cap and honour `Save::glitches`.
- **The app runs native and in the browser.** No `SystemTime`, threads, files or `std::process` in code the web build reaches. Keys must work in a browser: Esc is AnarchyOS's leader, never Ctrl/Alt shortcuts; F5, F6, F7, F11 and F12 belong to the browser, and a new key that a browser would act on goes in `web/index.html` first.
- **A modal owns the keyboard**, and programs, panes and windows that take keys stop text input so the terminal can't eat their keys.
