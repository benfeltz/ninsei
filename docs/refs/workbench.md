# Desktop (Workbench): spec sheet

The desktop of the Hosaka's PlanView firmware and the Ono-Sendai's classic one. Reference: the
Amiga's Workbench 1.3 on a 640×200 high-resolution screen, in Topaz 8 (see `docs/design.md`).
The kit that draws it is `crates/app/src/kit/workbench.rs`, shared by the desktop
(`workbench.rs`) and System's Preferences (`system.rs`).

These are the numbers the kit draws to. Change them here and in `kit::workbench::size` together.

## Grid, type and colour

| | Workbench 1.3 | Desktop | Why |
|---|---|---|---|
| Scale | a hires pixel, about twice as tall as wide | 1 logical pixel across, 2 down, every rect on a whole pixel | Keeps the 1.3 proportions: bars and borders are thin across and tall down |
| Type | Topaz 8 (8×8) | 14px monospace (`home::font`) | No bitmap font yet (a separate decision) |
| Colour | blue, white, black, orange | the theme's `backdrop`, `ink`, `shadow`, `accent`, and `fill` for the dither and the gadgets' faces | Every colour from the theme |

## The screen

| | Workbench 1.3 | Desktop |
|---|---|---|
| Title bar | 10px, white, the screen's name in blue, the depth gadget at the right | 22px in `ink` with a 1px `shadow` rule under it; the name and the run counts in `backdrop` 8px in; the bank and clock (with the add-on) 36px from the right; the depth gadget 26px from the right edge, 3px down |
| Backdrop | solid blue, or a pattern | `backdrop`, or (with the add-on) a dither of one-hires-pixel dots (1×2) in `fill`, every 6px across and 3px down, alternate rows offset by 3px, on whole pixels |
| Icons | down the left, disks at the right | down the left 18px in; once that column is full, down the right edge |

## Windows

| | Workbench 1.3 | Desktop |
|---|---|---|
| Border | 2px across, 1px down | 2px in `ink`, all round |
| Title bar | 10px, filled when active | 20px; filled `ink` with `backdrop` pinstripes every 3px when active, `backdrop` when not; a 2px `ink` frame |
| Title | after the close gadget | 26px in on a solid backing, 5px either side of the text |
| Close gadget | top left | 16×14, 4px in and 3px down: `ink` with a `shadow` outline and a 4×4 `shadow` dot |
| Depth gadgets | two, front and back, top right | one, 22×16 at 26px from the right edge, 2px down: an outlined back square and an `accent` front one. On the front window it sends it to the back; on any other it brings it forward |
| Sizing gadget | bottom right | none: windows open at their tool's size and don't resize |
| Content | inside the border | 6px in from the border at the sides and bottom, 4px under the title bar |
| Scroll bar | a proportional gadget on the right border, no arrows | the same (below), flush inside the right border from the title bar down, only while the tool runs past the window; each tool window opens the bar's room wider than its tool |

## Proportional gadget (the scroll bar)

| | |
|---|---|
| Width | 16px |
| Container | `fill`, with a 1px `shadow` edge on the left |
| Knob | as tall as the view's share of the whole (at least 12px), in `ink` (it stands out from the container on every theme, mono tubes included), bevelled with `shadow` bottom and right |
| Track | a click above or below the knob scrolls a page |
| Knob dragged | the view follows |
| Wheel | over the window or the gadget, unless a window in front covers it |

## Gadgets and buttons

| | Workbench 1.3 | Desktop |
|---|---|---|
| Bevel | a light edge top and left, a dark one bottom and right; reversed while pressed | 2px: `ink` top and left, `shadow` bottom and right; reversed while pressed or on |
| Push button | a bevelled box, its label centred | 84×22 in requesters, `backdrop` face; pressed, an `accent` face with `shadow` text |
| Toggle gadget (Preferences) | a bevelled box, filled when selected | the label in 13px with 9px either side and 5px above and below; `fill` face, `accent` when on |
| Mutually exclusive choice | a row of toggle gadgets, the chosen one pressed in | the same (Preferences' decks, monitors, themes and damage glitches) |
| Check box | a bevelled box | 16×14, raised on `fill` when off; pressed in on `accent` with a `shadow` check when on; the label 6px after |
| Laid-out push button | | the label with 12px either side, 22px tall |

## Requester

| | Workbench 1.3 | Desktop |
|---|---|---|
| Text | "Please insert volume X in any drive", centred | the same, three centred lines 18px apart, the volume in `accent` |
| Buttons | Retry at the left, Cancel at the right, along the bottom | the same, 6px in |
