# AnarchyOS: spec sheet

The Ono-Sendai's desktop. Reference: [Omarchy](https://omarchy.org/) on Hyprland, with a
btop-style bar (see `docs/design.md`, the Ono-Sendai's point of view): tiles with gaps and
rounded borders, a slim top bar, a fuzzy launcher, and everything themed. The kit that draws it
is `crates/app/src/kit/anarchy.rs`; the rules (workspaces, focus, the master-and-stack layout,
full screen, the master's share, the launcher's search) are core's `desktop::Tiling` and
`desktop::search`.

These are the numbers the kit draws to. Change them here and in `kit::anarchy::size` together.

## The bar

| | |
|---|---|
| Height | 26px, across the top of the screen |
| Rule | 1px in `muted` under it |
| Type | 12pt monospace |
| Workspaces | 22×18px pills from 8px in, 26px apart; the shown one filled `accent` with its number in `shadow`, a used one's number in `ink`, an empty one's in `muted`; a click switches |
| Mode | 8px after the pills: `  esc: keys` in `muted`, or ` LEADER ` in `accent` while the leader is down |
| Focused tool | 110px after the pills, in `ink`; `  [full]` after it on a full-screen workspace |
| Widgets | Right-aligned 8px from the edge, in `muted`: bank, heat, deck, monitor, uptime (and runs, exits, flatlines and best haul with the Stats add-on) |

## Tiles

| | |
|---|---|
| Gap | 8px between tiles and around them |
| Corner | 6px radius |
| Fill | `backdrop` |
| Border | 1px `muted`; the focused tile 2px `accent` |
| Label | ` tool name ` in lower case, 12pt, cut into the top border 12px in and 7px up, on a `shadow` patch; `accent` on the focused tile, `muted` on the rest |
| Content | 10px in from the sides and 8px up from the bottom, starting 14px below the tile's top (under the label) |

## The launcher

| | |
|---|---|
| Box | 460×320px, centred across, its centre 35% down the screen |
| Frame | `backdrop` fill, 2px `accent` border, 6px corners, a 3px `shadow` halo |
| Inside | 12px and 10px in: the search box, then the list |
| List | 20px rows, the picked one in `accent` with `shadow` text; as many rows as fit, scrolling with the pick |
| Empty | `nothing matches` in `muted` |
| Keys | type to filter, ↑↓ to pick, Enter to open or run, Esc to close |

## The help (`?`)

| | |
|---|---|
| Box | 580px wide, 22px a line plus 24px, centred on the screen |
| Frame | as the launcher's, 2px `accent` |
| Lines | the key in `accent` 18px in, what it does in `ink` 150px in |
| Closes | a click, or any key but `?` and `/` (Esc too, on the first-boot help) |
