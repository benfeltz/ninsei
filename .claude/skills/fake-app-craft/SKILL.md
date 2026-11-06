---
name: fake-app-craft
description: How to design and build ninsei's fake applications so they look like real period software, not a game skin over egui. Use before building or redesigning any screen, window, pane, chrome, desktop, dialog, BBS screen, tool, cracktro or magazine page in crates/app, and whenever a screen "looks off", "looks bad", "looks cheap" or "doesn't feel like a real computer". Pairs with ninsei-ux-review (judging the result) and the per-era reference skills human-interface-guidelines-1992 and tui-design.
---

# Fake app craft

ninsei is mostly fake applications: a Mac-like desktop, a tiling WM, a btop rice, a DOS productivity suite, BBSes, a mail-order magazine. The game lands only if each one looks like software someone shipped. A fake app is judged against the real one in the player's memory, so "close enough" reads as cheap. **Faithful first, then bend it for the game, on purpose and in writing.**

Read `docs/design.md` first: the pillars, the deck's fantasy, feel, rules of thumb and comps. This skill is how to turn that into pixels.

## Why the current screens look bad

These are the patterns in `docs/shots/` today. Don't add to them; fix them when you touch a screen.

1. **One font for every machine.** `theme.rs` maps every `TextStyle` to egui's one built-in monospace font, and `home::font()` does the same. A Lisa, an Amiga, a DOS suite and a btop rice all speak in the same typeface, so they all read as "egui app with a theme".
2. **No grid.** Text-mode apps are laid out in pixels with egui spacing, so columns drift, text hugs borders (Liza's phonebook detail text touches the window edge), boxes don't line up with each other, and rows get cut in half (the suite's activity log).
3. **Truncation everywhere.** `cycles left this tur`, `firewall.e`, a spreadsheet column `OPT` sliced off, overlapping axis labels. Real software lays out to fit, or truncates on purpose with a marker.
4. **Chrome stacked on chrome.** A tab titled `WAREZ.WK1` over a box titled `WAREZ.WK1` with `[■]` and `[2]` badges, double rules and bracketed values. Each layer was a reference detail; together they are noise. The real apps had one frame per thing.
5. **Charts with no data design.** A strength chart that is one solid block filling the plot; axis ticks at 18/14/9/4/0; a tiny sparkline in a big empty pane. A chart is only worth drawing if its shape says something.
6. **Dead space next to cramped space.** Half of a pane empty while the next one truncates. Real layouts give space to what the user is looking at.
7. **Everything the same weight.** Labels, values, hints and headers in the same size and colour, often all caps. There's no first thing to look at.
8. **Hints written in the app's body.** `double-click a number to dial it`, `bring it to zero`, `click a header to sort` sit inside app content, in the game designer's voice. Period software put help in a status line, a legend, Balloon Help or the manual.

## The method

### 1. Pick the one real program and version

Not "a retro Mac" but **MacPaint 1.0 on System 1.1, 512×342**. Not "a DOS app" but **Lotus 1-2-3 R2.01, 80×25 text mode**. Not "btop-like" but **btop 1.2 at 120×40**. The comps tables in `docs/design.md` list candidates; choose one per screen and name it in the PR. Mixing two references on one screen is how chrome gets stacked.

### 2. Write a spec sheet before code

Look at real screenshots of the reference (search for them; the Internet Archive and GUIdebook have period screenshots; mention which ones you used). Write down, in the PR or issue:

- **Grid:** character cells (cols × rows) or pixels at an integer scale.
- **Type:** the face (Chicago, Geneva, Topaz, the VGA 8×16 ROM font, a terminal font), sizes, and where bold, inverse or underline appear.
- **Chrome:** title bars, borders, gadgets, scroll bars, menus and status lines, at their real sizes and positions.
- **Density:** how much was on one screen, and how much padding there was.
- **Voice:** labels, button words, error wording, capitalisation (System 1 says `OK` and `Cancel`; 1-2-3 says `READY` in the corner).
- **What we change for the game,** and why. Every departure is a decision, never an accident.

### 3. Build on the grid

**Text-mode apps** (the Hosaka suite, btop chrome, BBS screens, NinShell, AnarchyOS bars, the QWK reader):
- Lay out in **cells, not pixels**. Measure the font's advance and line height once, compute cols × rows for the pane, then place every glyph, rule and box by cell. Box-drawing characters or rects snapped to cell edges, never egui `Frame` margins.
- A pane never shows a partial cell or a half row. Size it to whole cells and centre the leftover pixels.
- Columns are fixed widths in cells, sized from the widest real value (8.3 filenames are 12 cells). If it doesn't fit, drop a column, don't slice one.
- Use the period's emphasis: inverse video for selection, bright for the focused field, a bar in the corner for the mode. One emphasis per meaning.
- BBS screens already work this way (78×24, `every_screen_fits_the_terminal`); copy that discipline.

**GUI apps** (Liza, Desktop/Workbench, the Sense/Net stack, CPU Shopper):
- Lay out in **pixels at a whole-number scale** of the reference's resolution (for example 1× logical pixels drawn at 2× or 3×). Every rect, line and glyph lands on that grid. No fractional positions, no egui default rounding or margins leaking in.
- Take sizes from the reference: menu bar height, title bar, close box, scroll bar width, button size and corner radius, dialog margins. If unsure, measure a screenshot.
- Proportional system fonts where the reference had them (Chicago and Geneva on the Mac, Topaz on the Amiga), not the code font.

**Fonts.** Faithful fonts are the biggest single win and need a real decision (licences, a bitmap-font route in egui, wasm size), so raise it with the user instead of quietly adding one. Until then, set sizes to the reference's metrics and keep the grid honest, so swapping the face later is cheap.

### 4. Then bend it for the game

The pillars win over faithfulness where they conflict: every colour from the theme's six roles (`bg panel faint dim fg bright`), readable on every theme and on mono tubes, click areas never move, danger is `bright` plus motion. Write the departure down. Most of the game's information should still fit the app's own idiom: on the Hosaka, the trace is a field on a form; on the Ono-Sendai it's a gradient meter; on Sense/Net it's a pattern-filled thermometer on a card.

### 5. Compare side by side

Put your screenshot next to the reference screenshot at the same scale and look for differences in this order: grid and alignment, type, chrome sizes, density, voice. Fix, then hand it to `ninsei-ux-review`.

## Rules of craft

- **One frame per thing.** A window has one title. A pane in a tab doesn't repeat the tab's name in its border. Badges and IDs only where the reference had them.
- **Hierarchy with the period's tools:** position, inverse, bold, `bright` against `dim`. At most three levels on a screen.
- **Space goes to the focus.** Size panes to their content. An empty pane is a layout bug.
- **Numbers are aligned:** right-aligned in fixed columns, with units in the header, not repeated in every cell.
- **Truncate on purpose:** 8.3 names never truncate; free text ends with the period's marker (`…`, `>` or `~1`), never mid-glyph at a clip rect.
- **Charts earn their place.** Pick a scale that shows change (nice ticks: 0, 5, 10, 15, 20), label the axes once, and don't draw a chart when one number says it.
- **Copy sounds like the software, not the designer.** Help goes where the real app put it: a status line, an F-key legend, Balloon Help, a manual card. The world's text (BBSes, posts, textfiles) can teach; the OS chrome never coaches.
- **Motion has a meaning** (see design.md), and stays within the comfort limits.

## Per-era references

- **Liza, the Sense/Net stack, Mac-style alerts:** `human-interface-guidelines-1992` (menus, windows, dialogs, alerts, controls, black-and-white-first). The 1992 HIG is later than System 1 but its rules are the same family.
- **btop chrome, NinShell, AnarchyOS, BBS screens, the suite's text mode:** `tui-design` (layout from the work, focus, footers, box drawing, sparklines). Skip its sections on raw mode, NO_COLOR and terminal capability detection: our terminals are drawn, not real.
- **Any screen, for usability:** `ux-heuristics` (Nielsen, Krug). Where it says "web" or "mobile", read "this fake app".
