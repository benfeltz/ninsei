# Liza: spec sheet

The Sense/Net deck's desktop. Reference: the first Macintosh's System 1.x on a 512×342 screen,
with the Lisa's name (see `docs/design.md`, the Sense/Net's point of view), and the 1992 Human
Interface Guidelines as the review bar (`.claude/skills/fake-app-craft/references/`). The kit that
draws it is `crates/app/src/kit/liza.rs`, shared by the Liza desktop (`liza.rs`), System's
Control Panel (`system.rs`) and the Stack chrome's alert (`stack.rs`).

These are the numbers the kit draws to. Change them here and in `kit::liza::size` together.

## Grid, type and colour

| | System 1 | Liza | Why |
|---|---|---|---|
| Scale | 1 screen pixel | 1 logical pixel, every rect on a whole pixel | The game's screen is bigger than 512×342; whole pixels keep 1-bit lines sharp |
| Type | Chicago 12 | 14px monospace (`home::font`) | No bitmap font yet (a separate decision); the bars are a few pixels taller to hold it |
| Colour | black on white | the theme's ink lit on its darkest background (`ink` and `shadow`), and the patterns from `data/patterns.ron` in the same two | Every colour from the theme; lit ink reads on every tube |

## The menu bar

| | System 1 | Liza |
|---|---|---|
| Height | 20px with its rule | 22px, a 1px ink rule under it |
| Mark | the Apple, 10px in | Sense/Net's eye, 18px in; it opens the Calculator when owned |
| Titles | from 10px after the mark, 10px either side of the text | from 40px in, 10px either side of the text; the open one inverted |
| Menu | 16px items, 1px frame, 1px shadow | 22px items from 4px down, 190px wide, 1px frame, 2px shadow; the item under the pointer inverted; items 14px in |
| Right | nothing | the bank (and the time with the Alarm Clock), 12px from the edge |

## Windows

| | System 1 | Liza |
|---|---|---|
| Frame | 1px, 1px shadow right and bottom | 1px ink, 2px shadow |
| Title bar | 19px, six pinstripes when active | 20px with its rule; the `pinstripes` pattern 3px and 4px in when in front, plain behind |
| Title | centred on a white backing | centred on a paper backing 8px either side |
| Close box | 11×11 in a white margin, 8px in, active window only | 12×12 in a 2px paper margin, 10px in, the front window only |
| Content | under the title bar | 6px in from the sides, 4px under the title bar and above the bottom |
| Scroll bar | 16px, on the right edge | as System 1 (below), when the tool scrolls |
| Grow box | 16×16, bottom right | none: windows open at their tool's size and don't resize |

## Scroll bar

| | |
|---|---|
| Width | 16px, a 1px ink rule on its left, the arrow boxes 16×16 at either end |
| Arrows | outlined, pointing up and down; a click scrolls a line (the type's height) |
| Track | the 50% gray (`p50`) when there's more than fits; a click above or below the thumb scrolls a page |
| Thumb | 16×16, paper with an ink frame; dragged, the view follows |
| Nothing to scroll | the arrows and an empty track, no thumb |
| Wheel | scrolls the view under the pointer, as egui does |

## List box (the Control Panel's)

| | |
|---|---|
| Frame | 1px ink, paper inside, the scroll bar down its right side |
| Cells | fixed height, the selected one inverted; a click selects, ↑↓ walk the list once it has the focus |

## Controls

| | System 1 | Liza |
|---|---|---|
| Push button | 20px, round-rect, 1px frame, text centred; inverted while pressed | 22px, 8px corners, at least 60px wide, 12px either side of the text |
| Default button | a 3px ring 4px outside the button | the same; Enter presses it |
| Cancel | Esc or ⌘. | Esc (no ⌘: keys are browser-safe) |
| Check box | 12×12, an X corner to corner when on, label 5px after | the same |
| Disabled | gray text | the text at half strength |

## Alert

| | System 1 | Liza |
|---|---|---|
| Frame | a thin outer line and a thick inner one | 1px outer, 3px inner 4px inside it, 12px in to the content |
| Icon | 32×32 (note, caution, stop), top left | the glyph at 28px, top left |
| Text | the message beside the icon, wrapped | the title in bold, then the message, wrapped to the box |
| Buttons | along the bottom, the action at the right, Cancel left of it | the same, 10px apart, right-aligned under the text |
| Keys | Enter for the default, Esc to cancel | the same, while no text field holds the keyboard (the run's Message box keeps its Enter) |
