# ninsei design notes

How ninsei should look and feel, and why. Read this before building UI. `CLAUDE.md` holds the code rules; this holds the taste.

## Pillars

- **You're at a computer.** The whole game is a machine on your desk: a desktop at home, a deck in a run, a CRT in front of both. Nothing floats outside that fiction. Menus are menus on that machine; damage is that machine breaking.
- **The look is hardware you own.** Colour comes from the **theme**. The CRT comes from the **monitor**, and a mono tube locks the theme. The UX comes from the **deck** and its firmware. A new look is something you buy, plug in, or flash; never a free toggle. The deck is also the **chassis**: each deck says how far each RIG line can go on it, so the climb to the endgame is a bigger machine, never a faster look. There are two tiers of machine: the starter caps low, and the decks above it share one ceiling in different shapes, so nobody gives up the look they like to climb.
- **Each deck is a point of view.** Same network, same rules, different lens. Swapping decks should feel like sitting down at someone else's computer.
- **Readable before pretty.** Motion, glow and glitch sit on top of a screen you can always read and click. Click areas never move with an animation, and important numbers are never obscured.
- **You start with nothing; it unfolds.** A new player has a deck, a friendly welcome stack and one number. Every tool, board, network and idea is found in the world, in order: the customer board leads to the first companies and to a message about an illicit board, that board's textfiles teach the next step, and so on. The OS never hints at what you haven't found; a thing you don't have simply isn't there. Each discovery should open a door, never a menu.
- **Data, then code.** Looks are data (`themes.ron`, `displays.ron`, `hub/hardware.ron`, `netmap.ron`, `bbs/<id>/`). Code adds *parts*; data picks which parts a deck uses.

## How a look is put together

```
theme (colour roles) ─┐
monitor (CRT preset) ─┼─> what you see
deck ─> firmware ─────┘
           ├─ map      Iso | Draft | Paint              (the run's network view)
           ├─ chrome   Classic | Btop | Suite | Stack   (how the run screen is dressed)
           ├─ panes    net | ...                      (secondary panes it ships with; more are bought)
           ├─ desktop  Tiling | Workbench | Liza      (the OS between runs: AnarchyOS, Desktop, Liza)
           └─ sounds   ono | hosaka | sensenet        (a sound pack in data/sounds/)
                                                        * planned for the Sense/Net deck (#46)
```

- **Colour roles.** Every part draws with the theme's six roles, darkest to brightest: `bg`, `panel`, `faint`, `dim`, `fg`, `bright`. No part brings its own colours; a deck's personality comes from shape, layout, type and motion. (The BBS window's CGA and the crash screen's red are the only exceptions.)
- **Danger** is `bright` plus motion: a faster pulse, a burst, a glitch. There's no red.
- **A new part** is one enum variant and one renderer. A deck's firmware names it in data. Never special-case a deck id in code.

| Part | Sense/Net (starter) | Ono-Sendai (hacker) | Hosaka (corpo) |
|---|---|---|---|
| `map` | `Paint`: a MacPaint picture on a card, nodes filled with patterns ✅ | `Iso`: wireframes over a glowing grid ✅ | `Draft`: a technical drawing ✅ |
| `chrome` | `Stack`: a HyperCard stack: the Card, the Message box, pattern meters, rounded buttons ✅ | `Btop`: boxed panes, gradient meters, dot graphs, waterfalls ✅ | `Suite`: document windows, forms, a spreadsheet, report charts, an F-key legend, dialogs ✅ |
| `desktop` | `Liza`: after System 1 and the Lisa: a menu bar, Finder windows, a Trash, a happy deck at boot ✅ | `Tiling`: **AnarchyOS**, after Omarchy: tiles, workspaces, a top bar, a launcher ✅ | `Workbench`: **Desktop**, after Workbench 1.3 ✅ |
| `sounds` | `sensenet`: square beeps, a warm chord, a gentle "boing", and a run soundtrack per network tier like a 1980s Mac adventure game: square tunes over triangle basses in major keys, gentle at tier 1 and uneasy at tier 3. As the trace climbs, a quiet triangle part drops out (the countermelody, the offbeat chords or the held line in thirds) and then the tempo rises ✅ | `ono`: harsh squares, noise bursts, chiptune arpeggios, and a looping chiptune soundtrack per network tier that thins out and speeds up as the trace climbs ✅ | `hosaka`: soft sine chimes, a polite ding, a tidy shutdown tune, and hold music for the runs: a tune per network tier in what an office plays while you wait (sine tunes over a bossa or walking bass, major sevenths, a polite tempo). Tier 1 is the lobby, tier 2 the department and tier 3 the executive floor, still polite but colder. As the trace climbs, the calm goes wrong while staying polite: the chords or the tune drop out and it turns hollow, then the hold music hurries ✅ |
| volume | View > Control Panel...: the Mac's Speaker Volume slider (0 to 7) and a Sound check box, in a striped window with a close box ✅ | a speaker on the status line, as a bar module shows it, that opens a btop box over it: a meter, `- 70% +` cut into the edge, `[x] mute` ✅ | Edit > Sound...: a dialog with a Volume bar, a Mute box, OK and Cancel ✅ |
| vis pane | a Scope card: a 1-bit oscilloscope in a field ✅ | a btop box: the spectrum in columns of cells along the gradient, the level in its edge ✅ | SOUNDLVL.RPT: a sound level report, level and peak on a form, the level by band as a bar chart ✅ |
| fights | the ICE card (the ICE, its hotbar, End Turn) in the centre, and the deck's program as a stack of blocks with Step, Run, Pause in the Script pane beside it ✅ (see *Fights*) | the map dives into the node: the ICE as a wireframe over a horizon grid, a program hotbar with previews ✅, and the deck's program in the `ice.bas` tile beside it, debugged from its keys and the terminal ✅ (see *Fights*) | an incident report with a strength chart and an agenda ✅, and the deck's program as `PROC-7.BAS`, a Turbo Debugger Module window with Watches, its F-keys on the legend while it's active ✅ (see *Fights*) |
| damage | 1-bit glitches: tearing and noise, never an RGB split ✅ (pattern smears and inverted rows: an idea) | screen glitches: tearing, RGB split, corrupted text ✅ | the same, plus corporate error dialogs (idea below) |
| add-ons | Alarm Clock, a Calculator desk accessory under the eye, a desktop pattern, MultiFinder ✅ |  |  |
| music player | **TRAX** as a System 6 desk accessory with Winamp 2's transport: the tune in bold and its crew, a horizontal scroll bar for the position (click or drag to seek), round ◀◀ ▶ ■ ▶▶ buttons (▶ reads ‖ while it plays), a volume thermometer, Repeat and Shuffle check boxes, the spectrum as 1-bit bars in a sunken box, and the playlist in a list box ✅ | **TRAX**, a tile after cmus 2.x with Winamp 2's feel: the playlist (the cursor in inverse, the playing tune lit with ▶), a spectrum in cell columns along the gradient, and a status box (`▶ playing · repeat`, cmus's `artist - title`, a position bar to click, the level and the keys cut into its edge) ✅ | **TRAX** as Protracker 2.3's playback screen in a Workbench window: POS, PATTERN, LENGTH and TIME in bevelled number boxes (and the loop's row beside PATTERN), PLAY (again to pause), STOP, PREV and NEXT, REPEAT and SHUFFLE toggle gadgets, the song name, its crew and a STATUS line, Protracker's spectrum analyzer, the volume on a proportional gadget, and the playlist as a list with its gadget ✅ |
| games | a window like any program's, titled by the copy's label (`Nibbler`, `NIBBLER [2K4]`), in Liza's two colours: the field in its frame, walls and covered squares in the 50% gray pattern, blocks outlined; a **Games** menu before Special lists each copy you own ✅ | a tile, labelled in lower case like the others (`blox [phs]`), from the launcher or `run blox`; the leader's keys stay the desktop's ✅ | a window titled with its version (`Minefield 1.1`), its icon among the others ✅ |


## The decks

**Two tiers of machine.** The Sense/Net Personal Deck is the starter and caps low (CPU 3, RAM 8, HDD 8, Integrity 21). The Ono-Sendai Cyberspace VII and the Hosaka Executive 4 are both the real deck, at one ceiling (6, 12, 12, 36) and one price, and differ in look and in **shape**: the Ono ships with more CPU, the Hosaka with more RAM and HDD (`stats` in `hardware.ron`, tuned by the probe so both climbs max out near the same run). Climbing is one purchase, then RIG, and which look you climb in is yours. Owning the starter stays, like owning a monitor; nothing is sold back. The magazine sells each deck on its own page, and each page prints a **spec box**: what the deck ships with and what it expands to, line by line, read from those `stats` rows, never typed into the ad. The ads sell the shape (the Ono sells cycles, the Hosaka memory and storage, the Sense/Net friendliness). System's Overview shows each line as value of cap, and the OS is what says a case is full.

### Sense/Net Personal Deck: the starter

**Fantasy.** Everybody's first deck, sold by the simstim giant that sells experiences to ordinary people. It's a friendly consumer machine that ships with a welcome stack and a smile, and the net seen through it is approachable, maybe a bit naive. The cracks show if you look: the fine print, the people its manual would rather you didn't meet. The shop that sold it threw in a dubbed disk, a cracked NIBBLER whose cracktro gives a number Sense/Net never would: the first door off the friendly net comes from the kid at the counter, not the manual.

**Feel.** Friendly, tidy, a little toy-like. A 1-bit world where everything is a picture: a menu bar, icons and folders, windows with striped title bars and a close box, a Trash. A run is a HyperCard stack: the network is a card you're looking at, drawn in MacPaint.

**Rules of thumb**
- **Shade with patterns, never colour.** Grey is a 50% checker, shadows are diagonal hatching, danger is dense crosshatch. Patterns are data (`data/patterns.ron`) and always draw in two theme roles, ink and paper.
- **Crisp pixels.** Patterns and icons draw pixel-aligned at a whole-number scale. Where a fine pattern shimmers through the CRT, use its `fallback`.
- **Text never sits on a pattern.** Labels get a solid backing.
- **Everything is labelled.** Menus say what they do; buttons are words. (Where a period player drew transport glyphs, ◀◀ ▶ ■ ▶▶, TRAX's desk accessory does too: they were the words.) It's the calmest screen to learn on.
- **Lit ink.** A dark page (`bg`) with lit lines (`fg`): a 1-bit Mac on a phosphor tube. It ships with its own tube, the Personal Display 9″: paper white, sharp, a gentle curve and no colour fringing (`paperwhite` in `themes.ron` and `displays.ron`). The lit-paper Mac look (a bright page) was the alternative in the mockup (#70), but it's glaring on a CRT. Liza loads a strict two-colour register while it draws, so the shared tools come out 1-bit too.
- **Motion is small:** marching ants around what's selected or dangerous, a blinking insertion point, a zoom rectangle when a window opens.
- **Damage is 1-bit:** patterns smear, bitmaps tear, rows invert. Never RGB split.

**Comps**

| Reference | Take from it |
|---|---|
| HyperCard | Stacks and cards, the Message box, the Home stack, rounded buttons, ◀ ▶ navigation |
| MacPaint | The pattern palette, pattern fills, the fat-bits look |
| The Apple Lisa, Macintosh System 1 | The menu bar, the Finder, striped title bars, close boxes, a Trash, the alert box |
| Susan Kare's icons | Friendly 1-bit icons that read at a glance |
| *Neuromancer* (Sense/Net) | A media company that sells experiences: friendly on the surface |

### Ono-Sendai Cyberspace VII: the hacker deck

**Fantasy.** A cowboy's custom rig. You don't *use* the matrix, you're *in* it. The machine is tuned, riced and a little unsafe.

**Feel.** Dense, fast, keyboard-first, alive. Things glow, pulse and spin. Data is shown as shapes and motion before numbers. Never idle: a graph is always moving somewhere.

**Rules of thumb**
- Show it as a graph: braille sparklines, gradient meters, waterfalls. Numbers sit next to the graph, not instead of it.
- Glow and bloom are welcome. Let the CRT's bloom do the rest.
- Tiling over floating: tools snap into tiles with small gaps and thin borders, and the focused tile is lit `bright`.
- Keyboard first, with every action a key; the mouse still works.
- Motion has a meaning: a pulse along a route means you can go there, and a burst means something died.
- Damage shows up as the screen breaking.
- Music is a TUI player in a tile (TRAX, after cmus): keys first (space, n, p, + and -, r, s, j and k, Enter), every key a click on its legend too.

**Comps**

| Reference | Take from it |
|---|---|
| [Omarchy](https://omarchy.org/) / Hyprland, r/unixporn rices | Tiling with gaps, rounded borders, a slim top bar, a fuzzy launcher, themed everything |
| [btop](https://github.com/aristocratos/btop) | Boxed panes with titles cut into the border; braille graphs; gradient meters; dense but legible |
| [cool-retro-term](https://github.com/Swordfish90/cool-retro-term) | The CRT itself: bloom, burn-in, curvature (our monitors) |
| *Neuromancer* (Gibson) | "Lines of light ranged in the nonspace of the mind": the iso map's grid and wireframes |
| *TRON* (1982) | Vector wireframes, glowing edges, a grid floor |
| *Hackers* (1995) | Data as a city you fly through; ICE as architecture |
| *Ghost in the Shell* (1995) | Green code, dense overlays, a cold professionalism |
| *Uplink*, *Hacknet* | Hacking-game UX: trace bars, connection chains, a terminal you trust |
| *DEFCON* | Glowing vector maps, slow dread as numbers climb |
| *Cyberpunk 2077* breach protocol, *Android: Netrunner* | Neon glitch, ICE as a puzzle with personality |

### Hosaka Executive 4: the corpo deck

**Fantasy.** A company workstation, standard issue. It sees the net the way the company does: assets, documents, line items. It's calm, orderly and faintly ominous, and everything on it is filed, numbered and logged.

**Feel.** A productivity suite. Tidy layouts, forms and tables, rulers and gridlines, status bars and function-key legends. Motion is sparse and purposeful: a progress bar, a cursor, a dialog. It's easy to read and hard to love, which is the point.

**Rules of thumb**
- **Show it as a document.** Tables, forms with labelled fields, reports with headers and footers, page and sheet numbers, document IDs. The draft map's title block is the template.
- **Rulers, grids and gridlines.** Spreadsheet cells, drafting paper, column rules.
- **Its music player is a tracker's screen** (TRAX on Desktop): numbers in labelled boxes, gadgets in capitals, the song in a strip; even the soundtrack is filed.
- **A function-key legend bar** (`F1 Help  F2 Save  F3 Exit ...`), a menu bar along the top, and a status line with the date, user and record number. **A departure, on purpose:** the period's software (WordPerfect, Turbo Debugger) ran the F-key line along the bottom of the screen; ours sits directly under the menu bar, because its keys are buttons as well as a legend, and at the bottom they're missed (Ben's test of 141.6). Same bar, same keys, same rules; only its place differs.
- **Dialogs, not overlays.** Confirmations and errors come in boxed dialogs with buttons and drop shadows.
- **Very little motion.** No spin and no glow beyond what the CRT adds. Animate only progress and focus.
- **Everything is labelled and attributed:** `PREPARED BY`, `DEPT`, `REV`, `PROPERTY OF HOSAKA CORP.`
- **Damage idea (on top of the glitches):** besides the glitches, the corpo deck can surface failures as system dialogs: `GENERAL PROTECTION FAULT`, `Abort, Retry, Fail?`, a stack of error boxes as integrity drops.

**Comps**

| Reference | Take from it |
|---|---|
| Lotus 1-2-3, VisiCalc | The spreadsheet grid, a cell address bar, a top-line command menu |
| WordPerfect 5.1 | The blue screen, a bare status line, the function-key template |
| Turbo Vision (Turbo Pascal/C++ IDEs), Norton Commander | Menu bar, boxed dialogs with shadows, twin panels, an F-key bar along the bottom |
| IBM 3270 / AS/400 green screens | Form screens, PF keys, a screen ID in the corner, fields you tab between |
| Windows 3.1 Program Manager, Office 4.x | Bevelled toolbars, status bars, windows inside windows |
| AutoCAD R12 for DOS, blueprints | The draft map: title blocks, dimension lines, revision clouds |
| Bloomberg Terminal | Dense tabular data, colour-coded function keys, a serious tone |
| *Severance* (the Lumon MDR terminal) | Retro-corporate calm with dread underneath; a numbers-on-a-grid ritual |
| *Alien* (1979): MU/TH/UR, Weyland-Yutani | The company computer as an authority, not a tool |

## More decks

The two decks are a start. Each new one should be a new point of view, not a palette swap. Some ideas:

- **A military/ops deck:** tactical radar sweeps, contacts and bearings, NORAD war-room boards (*WarGames*, *DEFCON*).
- **A scene deck, hand-built:** demoscene and cracktro energy, scrolling sine text, ANSI art everywhere, Amiga copper bars.
- **A wetware/biotech deck:** organic, slow, breathing UI; signals as nerves; the network as an organism.

**Adding an OS.** A deck's OS is its `desktop` part, and it gets a name (`Desktop::name`). It's built on the shared home (the Shell and the tools), so a new OS only decides how tools are arranged, how you move between them, and what the bar or menus show. Give it add-ons in `data/hub/desktops.ron` that fit its point of view: AnarchyOS sells workspaces and a fuzzy launcher, Desktop sells a clock and a backdrop.

**Adding a chrome.** A chrome is how a deck dresses the run screen. Write a `ChromeView` and override only what this deck sees differently: a pane, the tab titles, the menus, the status line, a bar over the panes, keys, an overlay. Everything else stays Classic. Secondary panes are dressed by each chrome too, so the same traffic data is btop's net box on the Ono-Sendai and a network usage report on the Hosaka.

**Adding a pane.** Extra panes are things you buy (`data/hub/panes.ron`), or that a firmware ships with. Give each one a point of view per chrome, not just a box.

**Adding a deck**
1. Write down its fantasy, feel, rules of thumb and comps here first.
2. Add it to `data/hub/hardware.ron` with firmware that reuses existing parts where they fit.
3. Build any new parts it needs (a `map`, `chrome` or `desktop` variant and its renderer). The part must work in every theme, on mono monitors too.
4. Give it a price and a blurb that sells the point of view.

## Fights: the deck runs a program

*Designed 2026-11-07 (task 141.1, dossier at the task note). Built by 141.2 (core), 141.4–141.6 (one screen per deck) and 141.3 (ICE that answers back: 141.3.1 put the ICE's programs in core and data; 141.3.2 shows them on screen, 141.3.3 adds the verbs that attack the program) and 141.9 (ICE that patch themselves between runs: 141.9.1 the machine, the grammar, the generator and versions in the save). ✅ marks what's built: the Sense/Net's Script card (141.4), the Ono-Sendai's `ice.bas` tile (141.5) and the Hosaka's Module window (141.6). Every deck has its screen; a chrome that adds none shows the terminal's `prog` listing.*

**The Script is a pane, not part of the fight.** It is on screen for the whole run, in its own box, so the program is something you look after between fights as well as during one; Step, Run and Pause only press with ICE in front. Every chrome dresses the one Script pane (until a deck designs its own, it's the terminal's `prog` listing); 141.5 and 141.6 design its dress, not where it lives. The Stack's default layout is three columns over the Message box: the Deck over the Programs on the left (narrow), the Card with ICE and Hex in the centre (widest), the Script in front of To Do on the right (medium; To Do is the least used pane, so it sits behind). The btop chrome starts in the same shape over the terminal: deck.sys over warez/, the map (the fight view in a fight), `ice.bas` in front of todo. So does the suite over its activity log: SYSRES.FRM over WAREZ.WK1, the plan view and the incident report, `PROC-7.BAS` in front of the work orders.

**The idea.** You don't fight ICE by hand. You wrote something that does, and you watch it work. Each deck holds **one program**, kept between runs like the loadout, and every fight opens with it loaded and the cursor on Run. The run screen is already an IDE, so the fight runs like a debugger, gdb's way: Step runs the line under the ▶ and stops on the next, so the ▶ travels down the listing (it never rests on a REM, which isn't a line that does anything); Run plays a line at a time at a pace you can follow (`LINE_SECS`, a display number), turn after turn, until a STOP, a hang or the fight's end; Pause stops wherever it is, partway through a turn too, keeping its place and what the turn has spent, and Step or Run carry on from there (End Turn there ends the turn); Finish (the terminal's `finish`) runs the rest of the turn at once. The hotbar is the manual override; Continue gives the turn back. Ben's call (141.8): a whole pass per press left the ▶ on line 10 before and after, so nothing seemed to run. The program is edited in place, mid-fight if you like; what you leave is what the deck keeps. No window opens, nothing is saved by hand, and there is no library in version 1.

**The language is BASIC**, line-numbered, with four keywords. Everything else is a program you own, so **the warez are the verbs**: a shield is `RUN firewall`, a class match is `IF ICE CODEGATE` / `RUN decoder`. Nothing in code knows what a shield is.

```
10 RUN icepick
20 IF INTEG < 10
30 RUN patch
40 GOTO 10
```

| Line | Does |
|---|---|
| `RUN <program>` | runs a program on the HDD, exactly as the hotbar would; spends one cycle; resolves through `Game::preview`, so it does what the preview says or nothing |
| `IF <condition>` | guards the next line only (true: it runs; false: it's skipped) |
| `GOTO <line>` | jumps; the only loop |
| `STOP` | breaks to the debugger and hands you the turn; the breakpoint, and the only one |

Conditions read what is already on screen: `INTEG`, `TRACE`, `CPU` (cycles left), `TURN`, `ICE` (its strength), `ICE <class>` (`BARRIER`, `CODEGATE`, `SENTRY`), `NEXT <intent>` (`ATTACK`, `TRACE`, `FORTIFY`, `SPOOF`, `SEGFAULT`: the telegraphed move, what the ICE's program will do next as the fight stands now), `HAVE <program>`. Comparisons `< > =` against a number; no `AND`, `OR` or variables (two IFs in a row are an AND, two guarded lines an OR).

**The deck's stats are the program's limits**, and each is a different kind of decision:

| Stat | Is | Rule | On the deck |
|---|---|---|---|
| CPU | tempo | each `RUN` spends one; at 0 the pass ends and the ICE acts | "3 cycles a turn" |
| RAM | cleverness | each `IF` evaluated spends one GB for the turn, true or false; `GOTO` and `STOP` are free; program length never matters | "6 decisions a turn" |
| HDD | vocabulary | the programs you carry are the verbs you can write | "6 programs carried" |

One pass through the program is one turn: start at the first line and run until the program ends, a `STOP`, or a budget is empty (CPU at 0, or no GB for another IF); then the ICE acts. A Step spends as it goes (a RUN its cycle, an IF its GB), and the turn ends on the line that spends the last cycle, reaches the program's end or finds no GB: the ICE acts and the ▶ goes back to the top. A `GOTO` back to the top does not end the pass: `40 GOTO 10` keeps looping while cycles remain, so a three-cycle deck fires three programs from a one-RUN program, and a program that ends with cycles left wastes them. **Loading goes away:** every program on the HDD is a verb. Three rules keep it honest: a `RUN` that can't run is skipped and the line shows the preview's reason (a typo never halts a fight); a pass that comes back to a line without spending anything (no `RUN`'s cycle and no `IF`'s GB since it was last there, so a bare `GOTO` loop) is a hang and the deck prints `?BREAK IN <line>` and drops to the debugger, the ▶ held on that line and the break shown where you're looking (the Card's title line on the Sense/Net, the tile's bottom edge on the Ono-Sendai in btop's lowercase, the Module window's bottom edge and its LAST watch on the Hosaka; on an edge it comes after the budgets, so a narrow one cuts it first) until the next Step, Run, edit or the turn's end; running out of CPU or RAM is not an error, just the end of the turn.

**Where the first program comes from.** A fresh deck ships with one: the Security Suite's `SAFE` (`20 IF INTEG < 7` / `30 RUN patch` / `40 IF NEXT ATTACK` / `50 RUN firewall` / `60 GOTO 20`). **SAFE is defensive only.** Sense/Net sells decks to customers, and its Security Suite exists to keep hackers out of them, so SAFE patches and firewalls and never attacks. It names only what the deck ships, so it never prints "no program called …" lines, and with nothing to defend against it runs nothing: its IFs spend the turn's GB going round, so it idles until the RAM runs out, the turn ends and Run carries on. Ben's call (141.8): it used to stop with `?BREAK IN 20` every quiet turn, though each loop had spent two GB; progress is spending a budget, a RUN or an IF, so only a loop that spends nothing hangs. The player's first edit, adding `RUN icepick` to the loop, is the first lesson. The world teaches more as text you type in: the Gutter's G-FILES print the crews' autos, CorpLink prints Hosaka's procedure with a STOP at every severity. Programs as downloadable files, and several per deck, are a later version.

**Who speaks.** The trace of a pass is the deck's OS (a sentence per line in the Sense/Net's Message box, a terminal line on the Ono-Sendai, an activity-log row with a reference number on the Hosaka). `?BREAK IN 20` is the interpreter's, the one line every deck shares, as every BASIC did. A RUN that can't run uses the preview's words. A printed program sounds like its author: Sense/Net's is short and carries its copyright in a REM line, a crew's is terse and greets somebody in a REM line, Hosaka's has a header block.

**The ICE runs a program too** (141.3.1). An ICE's behaviour is a program in the same BASIC, run by the same interpreter from the ICE's side at the end of every turn, carrying on where it left off. Its words are its own, never the player's warez: the verbs on its row in `ice.ron` (`ATTACK n` hits the deck less its shield, which the hit uses up; `TRACE n`; `FORTIFY n`), with `IF`, `GOTO`, `STOP` and `REM`, and conditions that read the fight from its side: `TARGET INTEG`, `TARGET SHIELD`, `TARGET CPU` (the deck's cycles left), `TARGET RAN <program>` (the deck ran it this turn, from its program or the hotbar), `SELF STR` and `TURN`. A word outside a program's vocabulary is a bad line, so the deck can never `ATTACK` and an ICE can never `RUN`, and the player's program keeps its four keywords. A model of ICE is a vocabulary and a budget: `cpu` is actions a turn and `ram` IFs a turn, both in `ice.ron`, and its turn ends when either runs out, at its program's end (the next starts at the top), on a `STOP`, or when it goes round without spending anything, which is a stall ("Hydra stalls in 20. it does nothing."). A lagged ICE runs its turn and every verb does nothing, so it loses the move it was about to make, as it always did. Its steps are said in the log in the OS's voice, named for the ICE ("Enigma 20 TRACE 10: traces +10."), and what it does to the deck is bad news.

**NEXT is a dry run.** What the card telegraphs is the ICE's program walked from where it is to its first action, its IFs read against the fight as it stands, with no side effects and nothing from the RNG; it is asked again whenever it's shown or read, so it changes during your turn when you change what the ICE reads. Shield against Enigma (`10 IF TARGET SHIELD > 0` / `20 TRACE 10` / `30 ATTACK 4`) and its NEXT turns from ATTACK to TRACE on screen before it acts; that flip is the point, the preview being the rule. Lagged, or a program that would stall or run out of RAM first, has no NEXT. Only the next move is known: past it the ICE reads the fight as it will be then, so no screen lists moves ahead.

**The ICE on screen is what the deck has seen it run** (141.3.2). Each line an ICE's pass executes is remembered (`IceSeen::lines`, kept across runs), and every screen shows its program decompiled from those lines, the rest `????`, with a mark on the line it carries on from and NEXT's line emphasised: the Sense/Net's ICE card lists it under the hotbar ("its program: 2 of 4 lines seen"); the Ono-Sendai's fight view lists three lines under NEXT and target.ice a window under `PROG 2/4`, in btop's lowercase; the Hosaka's incident report has an OBSERVED PROCEDURE table (step, statement) straight after its form, in place of the agenda and before the strength chart. A new ICE starts as all `????` with NEXT on top, and a well-studied one is clear from its first turn. The ICE DB prints the same decompile on each met ICE's page ("decompiled, 2 of 4 lines seen"), so studying the bestiary is how you write the counter. The deck only shows what it watched run: it never prints a line the ICE hasn't executed in front of it.

**ICE that attack the program** (141.3.3). Two verbs hit your program rather than your deck, so the answer is a better program, never a number:
- **`SPOOF n`** hides the ICE's NEXT for the next n turns: every deck reads `???`, `IF NEXT …` is false (and still spends its GB), and the listing marks no NEXT line. The counter is a program that guards on INTEG instead, or the **sniffer**, RAZORWIRE's warez on Dead Sector, whose RUN clears the spoof.
- **`SEGFAULT`** garbles one line of your program. After a true `TARGET RAN <program>` in the ICE's pass, it hits the line that ran that program this turn (the last one, if it ran twice). If you ran it from the hotbar, it hits the first line that RUNs it. If your program never names it, nothing is garbled. An ICE with no `TARGET RAN` hits the last line that ran any program, or the first line that does something. Its text turns to garbage drawn from the run's RNG (same seed, same garble), it is skipped as a bad line, and the deck prints `?SYNTAX ERROR IN n` in the interpreter's voice after the ICE's own step. It never touches a meter. Only retyping the line undoes it, mid-fight or at home, since a garbled program goes home with the save.

A reader of the ICE's listing can predict both *when* and *where* (Archer SPOOFs a deck behind a firewall; Hydra's `IF TARGET RAN firewall` / `SEGFAULT` garbles your firewall line; kr4ken does both), so the counter is in your program: a spare line that runs the same program, or a different program in its place (Ben, 2026-11-08: it lets you swap out the target and counter). The probe's bots can't retype mid-fight, so the report counts what each program met (`ICE turns that hit the program`); the career bot retypes at home. CAREFUL has no guard against a spoof, because a seventh IF would starve the starter deck's 6 GB before its breakers.

**The nine ICE read you.** A barrier fortifies (Ice Wall digs in once you've spent your cycles, Bulwark rebuilds when it's hurt), a code gate traces (Enigma and Tollbooth when you shield, Tollbooth when you patch), a sentry hits (Rototurret when your integrity dips, Hunter the turn you patch); black ICE are longer programs with more RAM (Archer SPOOFs a deck behind a firewall and then traces it, and shoots when it's down, harder when you're weak; Hydra SEGFAULTs your firewall line the turn you raise it, traces while you're shielded and rewrites itself as it does) and lower base strength, since the layers they sit in add to it. Tier 1 is three or four lines. Firewalling on `NEXT ATTACK` is no longer free against an ICE that reads the shield: the answer is a better program, not a number.

**ICE patch themselves** (141.9; 141.9.1 built the machinery, so nothing is patched yet). Once the ICE DB has decompiled a kind, that kind would be solved on every net for ever, so the sysops read their logs and ship a new version, written by the game, never by hand:
- **The ICE's machine is its cap.** Its row in `ice.ron` is a small computer: `cpu` (actions a turn), `ram` (IFs a turn), `hdd` (the most lines its program may hold; REMs are free) and its verbs, each with a `range` for the number a line may carry (`ATTACK 3..8`). Every version fits the same machine: versions climb for ever and never raise it, and a stronger ICE is one on a bigger machine. Today every row's HDD is its program plus two lines, room for one more IF and its line.
- **Versions are semver on the kind** (`Rototurret 1.2.3`; 1.0.0 is its row). A **patch** moves a block or changes one number inside its range; a **minor** adds a read and its response, or removes one; a **major** replaces a read with a different one, so what the ICE counters changes. A block is an IF and the line it guards, or a line on its own, and it moves, comes and goes whole, since a false IF skips the line after it. A minor never removes a line on its own, so every version still acts.
- **What a version may say is data** (`data/ice_grammar.ron`): per class, the reads a new block may open with (`TARGET INTEG < n` with a range for n, `TARGET RAN patch`) and the verbs that may follow, weighted the way the class fights (a barrier digs in, a code gate runs the clock, a sentry hits). The fairness rule is there too: which verbs attack your program (SPOOF, SEGFAULT) and how many such lines a tier may hold (none for tier 1, one for black ICE). A new read is never one the program already has.
- **Between runs, never during one.** A version is in force from the next jack-in, the same on every net, and every ICE of a kind runs the same version all run. The next one is drawn from the save's key, the kind and the version it replaces, so the same save writes the same versions, and nothing comes from a run's RNG or the clock. Every draw passes the check the row's own program passed; one that doesn't is drawn again, and when nothing of that size fits, a smaller change is made, or none.

**The probe.** The careful bot's strategy is written in the game's BASIC (`probe::CAREFUL`: defend first, then the breaker made for the ICE, then the strongest it carries), and the probe plays through the same interpreter. It is the bot's own program, not the kit's: it buys the specialists, so it names them. If the bot can't say it in the language, players can't either. The dumb bot is `10 RUN icepick` / `20 GOTO 10`, the program that loses from tier 2. The third is `probe::SAFE_PICK`, the starter's SAFE with `55 RUN icepick` typed in, the first program a player writes: ICE that read the deck make it lose at tier 2 (about half its runs on the starter deck, 56% out in careers). The report gives each program's exits, traced, ICE and black ICE deaths and turns per fight per tier, and plays the careers again with SAFE_PICK.

**Per deck.** Same program, three computers; each screen names the one real program it imitates and keeps the hotbar as the override.

| | Sense/Net | Ono-Sendai | Hosaka |
|---|---|---|---|
| reference | HyperCard 1.2's button script editor and the Home stack, 1-bit | gdb's TUI in a btop box, with the C64 screen editor's way of retyping a line | Turbo Debugger 2.0: the Module window, Watches, F-keys |
| the program | ✅ a **stack of operator blocks** (RUN, IF, GOTO, STOP) dragged in from a palette row, each with fields for its parameters; a program menu lists only the disk; the ▶ in the gutter is the line about to run; a click selects a block (a dotted marquee) and the palette's ▲ ▼ X move or clear it, as HyperCard's Objects did, so a row is only the block and fits a ~340px column | ✅ `ice.bas`: the listing in a btop box on whole cells, the line about to run lit with ▶ in the gutter, a STOP's ●, a bad line dim with its fault after it, the cursor in inverse while the tile has the keys, `cpu 2/3  ram 4/6` cut into the bottom edge (the deck's limits outside a fight); a line is retyped where it stands, C64 style, with the terminal's line editor; the keys at the tile's foot, lit while they press | ✅ `PROC-7.BAS` in a Module window on the suite's cells: the line about to run in inverse with ▶, a STOP's ■ in the gutter, a bad line dim with its fault after it, the cursor boxed while the window is active (its double rule lit), `CPU 3/3  RAM 6/6` in the bottom edge; a line is retyped as a field in its row |
| the debugger | the button row under the palette row: Step (a line), Run, Pause, as rounded Mac buttons; a hang's `?BREAK IN n` at the left of the card's title line | ✅ the terminal is the prompt (the pass prints there; `step` a line, `finish` the rest of the turn, `play`, `pause`, `prog` are its commands; a hang's `?break in n` is cut into the tile's bottom edge), no box of its own; with the tile focused (a click; Esc gives the keys back): `r` run, `s` step, `c` continue, `p` pause, `j`/`k` or arrows move, `e`/Enter retype, `o` open a line, `d` delete; Space and the digits stay the hotbar's; the terminal types whenever the tile isn't focused | ✅ the Watches under the listing in the same window (TURN, CPU, IFS, INTEG, TRACE, NEXT, ICE, LAST: the last step as the log said it; blank outside an incident); a click makes the window active and the F-key legend (under the menu bar, not along the bottom as in Turbo Debugger: its keys are buttons, and up there they're seen) becomes the debugger's, Turbo Debugger's way: F1 Help, F2 Edit, F3 Insert, F4 Pause, F8 Step (Turbo Debugger's own), F9 Run (and carry on after a STOP), F10 End turn, with ↑ ↓, Enter, Ins, Del (or Backspace, a Mac's delete) and Esc; with another window active, F4 Hex and F8 Jack as before. Never F5, F6, F7, F11, F12 (F7 is caret browsing); keys that press nothing now are dim on the legend |
| the trace | the Message box, a sentence per line | a terminal line | an activity-log row with a reference number |

**Set aside.** `SHIELD`, `SCRUB` and `BEST BREAKER` as keywords (the warez are the verbs); a breakpoints pane (STOP is a line; a gutter mark is a view of it, never a second mechanism); downloaded editor and debugger (they are the deck's own software, dressed per chrome; "tools are downloaded" stays about the desktop's apps); a program library (deferred); a pure deckbuilder or puzzle fights without automation.

## Desktop music

Between runs the desk plays the tunes you own, on every deck, through **TRAX**, a player you download like any tool. Winamp 2 is the feel on every desktop (a playlist, transport, a position bar, the level, a spectrum); each desktop's period program is the look (cmus on AnarchyOS; the Liza and Desktop dresses come in 137.2 and 137.3). Tunes are music disks: the crews' releases on the boards' files, a publisher's compilation in CPU Shopper, and the one or two a new deck ships with. They're synthesized like everything else.

The run's tune wins. Jacking in pauses the desk's music where it was, and jacking out picks it up there; a cracktro takes the speaker while its tune plays and gives it back after; a dial never touches it (the modem is its own channel, and you listened to music while it screamed). The player keeps its tune, repeat and shuffle on the save; where it is in a tune lasts the session.

## Shared rules for every part

- **Every theme, every monitor.** It must read on a mono tube (the starter's paper white, amber, green) as well as on Synthwave. Test on both.
- **Clicks land:** animations never move a click area.
- **One place for timings:** each part keeps its animation timings and strengths together (like `netmap::anim`), and they're display numbers, not balance.
- **Comfort:** no full-screen flashing faster than about 3 times a second, and anything intense has a reduced setting.
- **Native and web:** no new heavy dependencies, and it stays smooth in the browser.
