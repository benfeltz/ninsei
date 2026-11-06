---
name: ninsei-ux-review
description: Review ninsei's UI for taste, craft and usability from real screenshots, against docs/design.md, the deck's reference software and classic usability heuristics. Use when asked to review, critique, audit or "make better" a screen, pane, chrome, desktop, tool window, BBS screen or PR that touches crates/app or data that changes what's drawn; when the user says the UX looks bad, cheap or off; and before calling UI work done.
---

# ninsei UX review

A review judges what the player sees, so it starts from pixels, not code. The bar: **would someone who used the real program believe this screen, and can a new player read it and act?**

## 1. Get screenshots

- Capture the screens in question. Use the web build (`scripts/build-web.sh`, then `python3 -m http.server -d dist 8000`) in a browser, or `cargo run -p ninsei [-- <hexseed>]` natively with a screen capture. Use a fixed seed so before and after match.
- Cover the hard cases: **two themes** (a mono tube such as amber or paper white, and a colour one such as Synthwave), the **deck** whose firmware owns the screen, an **empty** state and a **full** one (long names, many rows, high numbers), and **damage** if it shows in a run.
- `docs/shots/` has older shots for comparison; they may be stale.
- If you can't get a screenshot, say so and review the code as a weaker fallback. Don't claim a visual verdict you didn't see.

## 2. Know what it's meant to be

- The deck and its point of view in `docs/design.md`: fantasy, feel, rules of thumb, comps.
- The one reference program the screen imitates (see `fake-app-craft`, step 1). If nobody picked one, that's the first finding.
- Find real screenshots of that program and look at them next to ours.

## 3. Check, in this order

Run the passes in order and stop to write things down as you go. Earlier passes matter more.

**A. Craft (does it look shipped?)** Use `fake-app-craft`'s list of known failures:
- Is everything on one grid (cells for text mode, integer pixels for GUI)? Any column drift, half rows, text touching borders?
- Anything truncated by a clip rect rather than on purpose?
- Is chrome stacked (tab plus box title plus badge for one thing)?
- Is there one font for everything, at sizes that don't match the reference?
- Dead space next to cramped space?
- Do charts show a shape that means something, with sensible scale and ticks?
- Is there a clear first thing to look at (at most three levels of emphasis)?

**B. Faithfulness (is it the reference?)** Compare to the reference screenshots: chrome sizes and positions, type, density, voice and wording, how selection and focus look. For Mac-style screens, run the 1992 HIG checklist (`fake-app-craft/references/human-interface-guidelines-1992/references/checklist.md`). For text-mode and terminal screens, use the TUI guide's layout, focus and footer guidance (`fake-app-craft/references/tui-design/SKILL.md`).

**C. Pillars (`docs/design.md`)**
- Every colour from the theme's roles; reads on mono tubes and on colour themes.
- Readable before pretty; motion never moves a click area; important numbers never covered.
- The deck's point of view shows: swapping decks should feel like a different computer, not a palette swap.
- Nothing hints at what the player hasn't found (tools, boards, numbers). The OS doesn't coach.
- Comfort: no full-screen flashing faster than about three times a second; intense effects have a reduced setting (`Save::glitches`).
- Keys work in a browser (no Ctrl/Alt shortcuts the browser owns; see CLAUDE.md's browser keys).

**D. Usability** Use the heuristics guide (`fake-app-craft/references/ux-heuristics/SKILL.md`: Nielsen's ten, Krug's trunk test), read as "this fake app":
- Can a player tell where they are, what they can do, and what just happened?
- Can they see why an action is refused (the preview is the rule: `Game::preview`)?
- Is every action reachable by both mouse and keys?
- Is there recognition rather than recall: do commands, numbers and programs show where the player needs them?

## 4. Report

Lead with a one-line verdict, then findings ranked by severity:

| Severity | Meaning |
|---|---|
| 4 Broken | Can't read it or can't act (clipped values, unreachable action, unreadable on a theme) |
| 3 Major | Breaks the illusion or a pillar (wrong grid, stacked chrome, off-reference, a hint for something unfound) |
| 2 Minor | Visible sloppiness (a misaligned column, odd tick values, inconsistent wording) |
| 1 Polish | Nice to have |

Each finding has: the screen and theme, what's wrong (point at the shot), what the reference does instead, and the fix (file and function where you can find it). Group findings that share a root cause, such as "no cell grid in suite.rs", instead of listing every symptom.

Finish with **the three changes that would improve the screen most**. The user cares more about the screen looking good than about a complete list.

If asked to fix, follow `fake-app-craft`, re-shoot the same seed and themes, and show before and after.
