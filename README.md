# ninsei

A cyberspace hacking roguelike dressed up as a retro IDE. Rust + egui, with data-driven themes and a GPU CRT shader whose presets come from cool-retro-term.

![network map, green phosphor](docs/shots/network-green.png)

You jack in at a gateway and route through a layered network toward the exit, FTL-style. ICE nodes put up a fight, data vaults drop loot, MSG nodes are encounters (fences, fixers, honeypots), and one BBS per network sells warez and deck mods for the credits you've stolen. Fixers hand out contracts to take systems down. Trace climbs with every hop and every combat turn. At 100% you're flatlined.

## Play it

In the browser: **https://benfeltz.github.io/ninsei/** (put a seed on the end, like `#1a2b`, and your first dial uses it).

Natively:

```bash
cargo run -p ninsei            # random seed
cargo run -p ninsei -- 1a2b    # first dial uses this seed (hex)
```

On Linux you need the usual X11/Wayland libraries (`libxkbcommon-x11`, GL drivers), plus ALSA for sound (`libasound2-dev` to build).

### Web build

The same app compiles to WebAssembly and renders through WebGL.

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.130 --locked   # must match wasm-bindgen in Cargo.lock
scripts/build-web.sh                                        # -> dist/
python3 -m http.server -d dist 8000                         # http://localhost:8000
```

Every push to `main` builds `dist/` and publishes it to GitHub Pages (`.github/workflows/pages.yml`). One-time setup: repo **Settings → Pages → Source: GitHub Actions**.

## Home: the Workbench

Between runs you're on your own machine: a Workbench 1.3-style desktop, on the same monitor as your runs. One theme colours both; pick it in **Prefs**. Open **CarrierTerm**, pick a number from your phonebook and dial it: the modem handshakes, then the desktop drags down like an Amiga screen to reveal the run behind it. Reach the exit and you jack out home; flatline and the deck crashes first.

![the hub](docs/shots/hub.png)
![jacking in](docs/shots/hub-jackin.png)

- **NinShell**: `help`, `dir`, `phonebook`, `dial <n> [seed]`, `ranges`, `wardial <range>`, `run <tool>`, `info`, `theme <id>`, `crt`, `monitor [id]`, `deck [id]`, `echo`, `leet <text>`, `clear`
- **Prefs**: the theme (desktop and runs alike) with colour swatches, and the CRT display preset
- Windows drag by their title bar; the close gadget is top left, the depth gadget top right
- Everything below saves automatically: a file on desktop, local storage in the browser

### Hardware: monitors and decks

The look and feel each deck is going for (and the references behind it) is in [`docs/design.md`](docs/design.md).

What your screen looks like is the kit you own. Buy more in **RIG → HARDWARE** with banked credits; swap in Prefs, the view menu, or the Shell (`monitor <id>`, `deck <id>`). Testing? **F8** at home drops 1000¢ in the bank.

- **Monitors** set the CRT look. You start with a Zenith Amber Terminal; the Hercules Green Phosphor, Apple Monitor II, Commodore 1702 and IBM 8513 VGA are on the shelf. A **mono** tube only shows its own colour (amber is amber); a **colour** monitor runs any theme, Synthwave and Workbench included.
- **Decks** set the UX. You start on the **Ono-Sendai Cyberspace VII** (wireframes over a glowing grid); the **Hosaka Executive 4** sees the net as a technical drawing. Decks are look and feel only: stats come from RIG upgrades.

Everything is data in [`data/hub/hardware.ron`](data/hub/hardware.ron). A deck lists **firmware** (UX versions), each put together from named parts: `map` (`Iso` | `Draft`), `chrome` (pane style) and `desktop` (the overworld). A new UX version for a deck is a new firmware entry; a new kind of part is one renderer plus one name.

![hardware in RIG](docs/shots/rig-hardware.png)
![prefs](docs/shots/prefs-hardware.png)
![a run on the Hosaka](docs/shots/run-hosaka.png)

### Themes

One theme colours everything: the desktop and your runs. Themes live in [`data/themes.ron`](data/themes.ron), six colours each. To add one, add an entry; Prefs, the `theme` command and the IDE's theme menu pick it up.

![prefs](docs/shots/prefs.png)
![plasma desktop](docs/shots/hub-plasma.png)
![a run in Workbench colours](docs/shots/run-workbench.png)

### Displays: the CRT shader

The monitor is a GPU shader pass over the whole frame, modelled on [cool-retro-term](https://github.com/Swordfish90/cool-retro-term): curved glass in a bezel, bloom, burn-in, scanlines or pixel raster, RGB shift, flicker, horizontal sync wobble, static and ambient glare. Five presets ship, using cool-retro-term's own parameter names and values: **Default Amber**, **Monochrome Green**, **Commodore 64**, **Apple ][** and **IBM VGA 8x16**. Each one is a monitor you own (see Hardware below); plug one in from Prefs, the view menu or `monitor <id>`. `crt` turns the effects off.

Presets live in [`data/displays.ron`](data/displays.ron). To add one, add an entry; every value is 0 to 1. Clicks are mapped through the same curve the shader draws, so you click what you see; a click on the bezel goes nowhere. If the shader can't start, the old flat scanline overlay is used instead.

![amber](docs/shots/crt-amber.png)
![commodore 64](docs/shots/crt-c64.png)
![apple ][](docs/shots/crt-apple2.png)
![ibm vga](docs/shots/crt-vga.png)

### Choosing a network: CarrierTerm

Your phonebook starts with a handful of numbers, mostly small LANs. Each entry tells you what you're walking into:

- **Tier**: LAN, CORP or MEGACORP. Bigger networks, tougher ICE, better pay.
- **Traits**: *sentry-heavy* / *barrier-heavy* / *code-gate-heavy* (plan your breakers), *black ice*, *no BBS*, *graveyard shift* (trace -25%), *night shift* (+25%), *chatty*, *vault-rich*, *skint*.
- **Heat**: every run on a network heats it up (stronger ICE, faster trace); it cools while you hit other numbers.

Every dial is a fresh layout with the same personality. Tiers, traits and companies live in [`data/hub/networks.ron`](data/hub/networks.ron).

![carrierterm](docs/shots/carrierterm.png)
![dialing](docs/shots/dialing.png)

### Finding numbers: WarDial

CarrierTerm's **WarDial** tab scans a number range you know, 100 numbers at a time, and the grid fills in as the modem chews through them: `.` voice, `-` busy, `F` fax, `C` **carrier** (straight into your phonebook), `!` **trap** (the telco is listening: your next call runs hotter). Each scan costs a phone bill.

You always know the wide, sparse local exchange (`555-xxxx`). Better targets live on **tighter ranges**: a corp's lines (`555-7xxx`), then its core (`555-77xx`), where carriers are dense and mostly CORP or MEGACORP. You learn ranges from hints: data vaults and black ICE drop them, BBS **TEXTFILES** sell them, and a phone phreak on the street trades them. Hints you find come home even if you flatline. Ranges live in [`data/hub/exchanges.ron`](data/hub/exchanges.ron).

![wardial](docs/shots/wardial.png)

### What you keep

Runs are for taking things; home is for building. **Reach the exit** and the run's credits and fragments are banked, and you pick programs to keep in your stash. **Flatline** and you lose it all, except the 10% your dead man's switch wires out.

- **RIG**: spend banked credits on permanent upgrades: RAM, CPU, HDD, integrity, stash slots. Each level costs more than the last ([`data/hub/rig.ron`](data/hub/rig.ron)).
- **STASH**: programs you kept (2 slots to start) and your loadout for the next run. The four starter programs are always free. Stash programs you take into a run leave the stash: they come back only if you do.
- **HexEd**: the hex editor, at home, on your stash, with banked fragments to optimize. In a run you can only compile while connected to a BBS.

![rig](docs/shots/rig.png)
![stash](docs/shots/stash.png)
![hexed at home](docs/shots/hexed-home.png)
![keeping programs at the exit](docs/shots/exit-keep.png)

![crash](docs/shots/crash.png)

## The IDE

| Pane | What it is |
|---|---|
| `deck.sys` | CPU (actions per turn), RAM (what's loaded), HDD (what you own), integrity (HP), trace, credits |
| `warez/` | Your programs as a file tree: `ram/` is loaded and runnable, `hdd/` is stored |
| `network.map` | The run, as floating wireframes or a technical draft (below). Click a lit node to route there; nodes further than one layer ahead are fogged |
| `target.ice` | The ICE you're fighting: strength, next intent, run buttons |
| `hexedit` | Open any program's binary and patch its bytes (see Crafting) |
| `terminal` | Log and command line (`help`) |
| `todo` | Your contracts, as an IDE TODO list: `[ ]` open, `[x]` paid, `[-]` failed |

Tabs can be dragged, split and rearranged; **view → reset layout** puts them back.

On the **Ono-Sendai** the panes are dressed btop-style: boxed with their titles cut into the border, gradient meters with sparklines of their recent history, a trace-per-hop graph, a process table for your warez (a program's row lights up and its activity graph spikes when it runs), and in a fight a draining strength meter, a strength graph, the ICE's next moves as a waterfall falling toward *now*, and a damage waterfall of the last turns. The terminal stamps each line with its hop and turn. Graphs are painted at braille resolution (2x4 dots a character). Prefer the plain panes? Switch the deck's firmware to **Cyberspace 6.2** in Prefs.

![btop panes, synthwave](docs/shots/btop-synthwave.png)
![btop panes, amber](docs/shots/btop-amber.png)

### The network map

Nodes float over a glowing isometric grid as slowly spinning wireframes, one shape per kind:
a cube for barrier ICE, a ring for a code gate, an icosahedron "eye" for a sentry, a spiked star for black ICE, a drum for a data vault, an octahedron for the BBS, a pyramid for MSG, a box in a box for a subnet, a landing pad for the entry and an upright ring for the exit. Your routes glow, with pulses running down them; cleared nodes go dim; the fog is `?` on dashed lines.

The **Hosaka** deck (see Hardware) draws the map as a flat technical drawing instead: grid bubbles over each layer, a drafting symbol per node (section hatching, valve, eye, door swing...), hidden lines for the fog, a revision cloud round you, a dimension line and a title block.

The map reacts to the run. ICE you break blows apart (on the draft it's struck through and stamped `VOID`). The way you came stays lit. The BBS node wears its board's colours and name. As trace climbs the grid lights up and pulses, faster past 75%; the draft's paper darkens and its title block grows a TRACE row. Each new network draws itself in, layer by layer, and a route you hover lifts and spins.

![derez burst](docs/shots/map-derez-burst.png)
![draft: VOID, trail and TRACE](docs/shots/map-draft-void.png)

Both are drawn in the theme's colours. The **Synthwave** theme gives the magenta-grid, cyan-wire look; Workbench turns the draft into a blueprint. Which shape and symbol each node gets is data, in [`data/netmap.ron`](data/netmap.ron).

![isometric, synthwave](docs/shots/map-iso-synthwave.png)
![isometric, amber](docs/shots/map-iso-amber.png)
![draft, amber](docs/shots/map-draft-amber.png)
![draft as a blueprint](docs/shots/map-draft-blueprint.png)

**Keys:** `0`-`9` run that slot, `space` ends the turn, `enter` or `/` focuses the terminal.
**Terminal:** `run 2`, `end`, `jack 5`, `load 3`, `unload 1`, `hex 0`, `opt 0`, `newrun [hexseed]`, `theme amber|green|ice`, `crt`, `clear`.

## Combat

Turn-based. Each turn you get your deck's CPU to spend running loaded warez; then the ICE does what its intent says (attack, trace or fortify) and your shields drop. Class breakers (decoder vs code gate, garrote vs sentry, crowbar vs barrier) hit their class for double and everything else for half. Stunned ICE adapts: it can't be stunned again the turn after. Loading a program mid-fight costs 1 CPU.

## Encounters

Routing into a **MSG** node opens a `talk` window with whoever's on the other end. Pick a numbered choice (or press its number); you can't move on until you answer, and every encounter has a way to walk away.

![encounter](docs/shots/encounter.png)

- **a fence** trades you a program for one of yours, or buys one (per GB of RAM, more for optimized code)
- **a fixer** offers a contract
- **an unsecured cache** might be payroll, might be a honeypot
- **a runner in trouble** pays fragments if you fight off the Hunter pinning them
- **a bent sysop** sells you a trace wipe
- **a ROM construct** sells fragments, or offers to soup up your deck (or fry it)

Add `leet: true` to an event and it speaks l33t too (the phone phreak does). Events live in [`data/events.ron`](data/events.ron): text, choices and effects (`Trade`, `Sell`, `Contract`, `Pay`, `Gamble`, `Fight`, `Leave`).

## Contracts

A contract names an ICE node ahead: derez it and the client pays. Some carry a **ghost bonus**, paid only if your trace is at or under a cap when the target falls. The target is revealed on the map with crosshairs and a `$`, even through the fog. Because links only run forward, a contract fails the moment its target is out of reach.

![contract target](docs/shots/contract.png)

Get them from fixers on MSG nodes or from the BBS's **[J]obs board**, which posts fresh work each time you dial in. Clients and briefs live in [`data/contracts.ron`](data/contracts.ron).

## Crafting: the hex editor

Every program is a 32-byte binary, and its stats are whatever the bytes say. Hit **hex** next to a program (or `hex <slot>`) to open it.

![hex editor](docs/shots/hexedit.png)

```
00-02  57 5A 01   magic "WZ" v1          08     opt level
03     opcode     01 BRK 02 SHD 03 REP    09-0A  signature of the opt level
                  04 SCR 05 STN           0B-0F  reserved scratch
04     power                              10-1E  name (ASCII, zero padded)
05     vs class   00 any 01 barrier       1F     checksum: sum of 00-1E mod 256
                  02 code gate 03 sentry
06     cpu cost   07  ram cost
```

The linker refuses to write a binary unless it checks out:

- **Budget.** The effect's cost (break power, repair power, half of shield or scrub, 7 for a stun) must fit in `3×cpu + ram/2 + 2×opt`. Want more power? Pay for it in CPU or RAM. Flip the opcode to turn a shield into a breaker. Rename it while you're in there.
- **Checksum.** Fixing it is your job; the inspector shows the computed value.
- **Signature.** The opt level is signed with a per-run key, so you can't just type a bigger number. **Optimizer fragments** (loot, or the BBS's OPTIMIZERS area) raise it properly. A program at the same opt level carries a valid signature you can copy, if you're clever.

Click a byte (or tab into the ASCII column) to type over it; arrows move, `ctrl+s` writes, `esc` releases the keyboard. At home, HexEd works on your stash; in a run, the compiler is only available while you're connected to a BBS.

## The BBS

Landing on the `BBS` node dials the network's board: modem handshake, an ANSI splash, then a real single-key BBS: `[F]`iles, `[J]`obs, `[B]`ulletins, `[W]`ho's online, `[G]`oodbye. File areas list today's stock. Pick a number to start a Zmodem download, and the credits come off when the transfer completes. **display** flips between true 16-colour ANSI and the current phosphor.

Three boards ship, each with its own sysop, wares and look:

- **THE NEON GUTTER** (LAN and corp nets, and every plain `newrun`): the hangout. Blue bars, l33t (`F1l3 4r34z`, `Rul3z 0f th3 Gutt3r`; hotkeys, filenames and numbers stay readable).
- **DEAD SECTOR** (LANs and megacorps, or any net with the *pirate board* trait): an anarchy board at 2400 baud. Red box-drawn rules, cheap hot breakers, phreak textfiles, no deck-mod shop.
- **CORPLINK EMPLOYEE SERVICES** (corp and megacorp backbones): the staff intranet. Plain inverse-blue headers, no l33t, a guest login, memos, hardware requisitions and priced like an expense report.

A phonebook number always reaches the same board: one of the boards for its tier, picked by the number, unless a trait names one.

![the gutter](docs/shots/bbs-gutter-main.png)
![gutter splash](docs/shots/bbs-splash.png)
![gutter download](docs/shots/bbs-download.png)
![corplink splash](docs/shots/bbs-corplink-splash.png)
![corplink menu](docs/shots/bbs-corplink-main.png)
![dead sector splash](docs/shots/bbs-deadsector-splash.png)
![dead sector files](docs/shots/bbs-deadsector-areas.png)

### Calling boards from home

Connect to a board in a run and its number goes in CarrierTerm's phonebook, even if you flatline. Or wardial the local exchange: every board picks up on its own number and shows as `B` in the scan grid. Then call it from home (**Call** in CarrierTerm, or `dial 555-0666` in the Shell). It opens over the desktop in its own look, with the modem handshake, and costs a phone bill per call (`call_cost`).

- **Files** cost bank credits. Warez go to the stash, textfiles teach their range, fragments go in the drawer. Deck mods need a live deck, so those only download in a run.
- **Jobs** target networks already in your phonebook. A job you take waits on that number. Dial it, and the contract is on your todo list with a target on that network. If you don't finish it in that run, it's gone.
- Listings and jobs are fixed until your next run, so calling again won't reroll them.
- Patching and optimizing still need a BBS in a run.

![boards in CarrierTerm](docs/shots/carrierterm-boards.png)
![jobs from home](docs/shots/bbs-home-jobs.png)
![a download from home](docs/shots/bbs-home-download.png)

### Adding a board

A board is a folder: `data/bbs/<id>/board.ron` and `data/bbs/<id>/splash.txt`. The build finds every folder, so there's nothing to register. In `board.ron`:

- `name`, `sysop`, `phone`, `baud`, `bulletins`, `users`, `areas` (files with prices, `stock` per run)
- `tiers: [..]`: the network tiers it turns up on. Leave it empty for a board only a trait reaches (`Board("<id>")` in `hub/networks.ron`).
- `look: (..)`: optional, every field defaults to the Gutter's. `header` (`Bar`, `Rule` or `Inverse`), CGA colours for `frame`, `bar`, `key`, `label`, `dim`, `text`, `money`, `ok`, `warn`, `aside`, plus `prompt`, `handle`, `software`, the main `menu` wording, the `goodbye` line and `leet`.
- `call_cost`: the phone bill for calling it from home.
- `default: true` on exactly one board (the Gutter).

Splash art and bulletins use pipe codes (below), up to 78x24. `Data::validate` checks every board's warez, hints, tiers and colours, and the tests render every screen of every board to make sure it fits.

## Content

Everything is data in [`data/`](data), compiled into the binary:

- `warez.ron`: programs (RAM/CPU cost, effect)
- `ice.ron`: ICE (class, strength, intent cycle; tier 2 is black ICE)
- `loot.ron`: weighted loot tables for ICE, black ICE and vaults (credits, warez, upgrades, fragments)
- `events.ron`: MSG-node encounters
- `contracts.ron`: contract clients and briefs
- `bbs/<id>/board.ron`: one BBS per folder: bulletins, users, file areas, prices, stock per run, look
- `themes.ron`, `displays.ron`: colour themes and CRT display presets
- `netmap.ron`: the network map's shape and drafting symbol for each kind of node
- `hub/`: desktop icons, RIG upgrades, network tiers/traits/companies, wardial ranges
- `bbs/<id>/splash.txt`: that board's splash art. Colours use Mystic/Renegade **pipe codes** (`|00`-`|15` foreground, `|16`-`|23` background); raw ANSI `ESC[...m` escapes work too, so `.ans` art from PabloDraw or Moebius can be pasted in.

`Data::validate` cross-checks the files (every id a table names must exist), and the tests run it.

## Layout

```
crates/core/   ninsei-core: pure rules, no UI. network gen, combat, loot, warez binaries + hex buffer, BBS navigation, ANSI parsing
crates/app/    ninsei: the egui + egui_dock shell. renders state, sends actions
data/          content (RON + pipe-coded art)
```

The core is deterministic (seeded SplitMix64), so a seed replays a run exactly. `cargo test` covers generation, combat, crafting, encounters, contracts, the BBS flow and screen sizes; `cargo run -p ninsei-core --example balance` plays 500 seeds with a dumb bot on the default network and on each tier, and prints the win rates.
