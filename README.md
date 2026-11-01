# ninsei

A cyberspace hacking roguelike dressed up as a retro IDE. Rust + egui, with amber, green and ice phosphor themes and a CRT filter.

![network map, green phosphor](docs/shots/network-green.png)

You jack in at a gateway and route through a layered network toward the exit. ICE nodes put up a fight, data vaults drop loot, and one BBS per network sells warez and deck mods for the credits you've stolen. Trace climbs with every hop and every combat turn. At 100% you're flatlined.

## Run it

```bash
cargo run -p ninsei            # random seed
cargo run -p ninsei -- 1a2b    # fixed seed (hex)
```

On Linux you need the usual X11/Wayland libraries (`libxkbcommon-x11`, GL drivers).

## The IDE

| Pane | What it is |
|---|---|
| `deck.sys` | CPU (actions per turn), RAM (what's loaded), HDD (what you own), integrity (HP), trace, credits |
| `warez/` | Your programs as a file tree: `ram/` is loaded and runnable, `hdd/` is stored |
| `network.map` | The run. Click a lit node to route there; nodes further than one layer ahead are fogged |
| `target.ice` | The ICE you're fighting: strength, next intent, run buttons |
| `terminal` | Log and command line (`help`) |

Tabs can be dragged, split and rearranged; **view → reset layout** puts them back.

**Keys:** `0`-`9` run that slot, `space` ends the turn, `enter` or `/` focuses the terminal.
**Terminal:** `run 2`, `end`, `jack 5`, `load 3`, `unload 1`, `newrun [hexseed]`, `theme amber|green|ice`, `crt`, `clear`.

## Combat

Turn-based. Each turn you get your deck's CPU to spend running loaded warez; then the ICE does what its intent says (attack, trace or fortify) and your shields drop. Breakers matched to the ICE's class (decoder vs code gate, garrote vs sentry, crowbar vs barrier) hit for double. Loading a program mid-fight costs 1 CPU.

## The BBS

Landing on the `BBS` node dials **THE NEON GUTTER**: modem handshake, an ANSI splash, then a real single-key BBS: `[F]`iles, `[B]`ulletins, `[W]`ho's online, `[G]`oodbye. File areas list today's stock. Pick a number to start a Zmodem download, and the credits come off when the transfer completes. **display** flips between true 16-colour ANSI and the current phosphor.

![bbs splash](docs/shots/bbs-splash.png)
![bbs file area](docs/shots/bbs-download.png)

## Content

Everything is data in [`data/`](data), compiled into the binary:

- `warez.ron`: programs (RAM/CPU cost, effect)
- `ice.ron`: ICE (class, strength, intent cycle; tier 2 is black ICE)
- `loot.ron`: weighted loot tables for ICE, black ICE and vaults
- `bbs/board.ron`: the BBS: bulletins, users, file areas, prices, stock per run
- `bbs/splash.txt`: the splash art. Colours use Mystic/Renegade **pipe codes** (`|00`-`|15` foreground, `|16`-`|23` background); raw ANSI `ESC[...m` escapes work too, so `.ans` art from PabloDraw or Moebius can be pasted in.

`Data::validate` cross-checks the files (every id a table names must exist), and the tests run it.

## Layout

```
crates/core/   ninsei-core: pure rules, no UI. network gen, combat, loot, BBS navigation, ANSI parsing
crates/app/    ninsei: the egui + egui_dock shell. renders state, sends actions
data/          content (RON + pipe-coded art)
```

The core is deterministic (seeded SplitMix64), so a seed replays a run exactly. `cargo test` covers generation, combat, the BBS flow and screen sizes; `cargo run -p ninsei-core --example balance` plays 500 seeds with a dumb bot and prints the win rate.
