---
name: ninsei-game-design
description: Game design lenses for ninsei, a cyberspace hacking roguelike told through fake applications. Use when proposing, shaping or critiquing a mechanic, progression step, tool, board, encounter, contract, deck, shop item or balance change, or when a feature request needs a design opinion before code ("should we", "how should X work", "is this fun", "what should unlock Y").
---

# ninsei game design

## What ninsei is

You sit at a computer. Between runs it's your desktop: a phonebook, boards, tools, a shop magazine. A run is dialing a number and routing through a layered network to the exit before trace hits 100%, fighting ICE, looting vaults, taking jobs. Your deck decides how all of it looks. Read `docs/design.md` (the pillars and decks) and `README.md` (the loop as players see it).

**The interface is the game.** Every mechanic reaches the player as something a piece of software shows or does: a field on a form, a line on a BBS, a file you download, a number in a phonebook. A mechanic that can't be shown in the deck's own idiom isn't ready.

## Lenses

Use these to shape and critique a proposal. Each is a question to answer in writing.

1. **The decision.** What does the player choose, between what, with what information on screen? If there's no choice, it's flavour (fine, but call it that). Per hop and per fight turn, there should be a real choice about trace, integrity, cycles or loot.
2. **Where it's found.** Things are discovered in the world, in order, never handed over by a menu: a board's textfile points at a tool, a post gives a number, a wardial finds a board. Name the door that leads to this and the door it opens next. The OS never hints at what you don't have.
3. **How it looks on each deck.** Sketch it in Sense/Net (labelled, friendly, 1-bit), Ono-Sendai (a graph, a key, motion) and Hosaka (a form, a report, a dialog). If it only makes sense on one, it's that deck's part, not a rule.
4. **The preview is the rule.** If the player can act on it, the game can say beforehand what will happen or why it can't (`Game::preview`). No hidden dice the UI can't explain.
5. **Risk you can read.** Trace and integrity are the tension. New risk must show up in the trace or integrity numbers or an explicit warning before it bites.
6. **The run vs the career.** Does this change a single run (a tactical choice) or the career (a progression step)? Career items come through RIG, boards, kits or networks data. Run items come from loot, BBS stock or encounters.
7. **Period truth.** Would this have existed, or plausibly existed, around 1984 to 1992 cyberpunk computing (BBSes, phreaking, warez, cracktros, mail order, corporate mainframes)? Lean on it: real customs make mechanics feel discovered rather than designed.
8. **Remove one thing.** What does the proposal make redundant, and can it go?

## Where things go

- **Numbers** live in `crates/core/src/tuning.rs`. After changing balance, run the probe (`cargo run --release -p ninsei-core --example balance [careers]`) and quote before and after. `runs_always_end` must keep passing.
- **Content** is data in `data/` (boards, events, ICE, warez, loot, networks, tools, starts). `Data::validate` must reject bad references.
- **A new way to progress** means teaching the probe's career bot to use it, or the probe stops telling the truth.
- **Every encounter needs a Leave choice**, and no choice may strand the runner.

## Output

For a proposal, write a short design note:
- the decision it creates
- where it's found and what it unlocks
- how it looks per deck
- its numbers and where they live
- how the probe will see it
- what it replaces

Recommend one version; list alternatives in a line each. For a critique, name the lens that fails and suggest the smallest change that fixes it. Hand UI questions to `fake-app-craft` and `ninsei-ux-review`.
