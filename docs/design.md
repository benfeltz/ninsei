# ninsei design notes

How ninsei should look and feel, and why. Read this before building UI. `CLAUDE.md` holds the code rules; this holds the taste.

## Pillars

- **You're at a computer.** The whole game is a machine on your desk: a desktop at home, a deck in a run, a CRT in front of both. Nothing floats outside that fiction. Menus are menus on that machine; damage is that machine breaking.
- **The look is hardware you own.** Colour comes from the **theme**. The CRT comes from the **monitor**, and a mono tube locks the theme. The UX comes from the **deck** and its firmware. A new look is something you buy, plug in, or flash; never a free toggle.
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
| fights | a Mac alert beside the card: the hardest hit as the ringed default button, a shield ✅ | the map dives into the node: the ICE as a wireframe over a horizon grid, a program hotbar with previews ✅ | an incident report with a strength chart and an agenda ✅ |
| damage | 1-bit glitches: tearing and noise, never an RGB split ✅ (pattern smears and inverted rows: an idea) | screen glitches: tearing, RGB split, corrupted text ✅ | the same, plus corporate error dialogs (idea below) |
| add-ons | Alarm Clock, a Calculator desk accessory under the eye, a desktop pattern, MultiFinder ✅ |  |  |


## The decks

### Sense/Net Personal Deck: the starter

**Fantasy.** Everybody's first deck, sold by the simstim giant that sells experiences to ordinary people. It's a friendly consumer machine that ships with a welcome stack and a smile, and the net seen through it is approachable, maybe a bit naive. The cracks show if you look: the fine print, the people its manual would rather you didn't meet.

**Feel.** Friendly, tidy, a little toy-like. A 1-bit world where everything is a picture: a menu bar, icons and folders, windows with striped title bars and a close box, a Trash. A run is a HyperCard stack: the network is a card you're looking at, drawn in MacPaint.

**Rules of thumb**
- **Shade with patterns, never colour.** Grey is a 50% checker, shadows are diagonal hatching, danger is dense crosshatch. Patterns are data (`data/patterns.ron`) and always draw in two theme roles, ink and paper.
- **Crisp pixels.** Patterns and icons draw pixel-aligned at a whole-number scale. Where a fine pattern shimmers through the CRT, use its `fallback`.
- **Text never sits on a pattern.** Labels get a solid backing.
- **Everything is labelled.** Menus say what they do; buttons are words. It's the calmest screen to learn on.
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
- **Function-key legend bars** along the bottom (`F1 Help  F2 Save  F3 Exit ...`), a menu bar along the top, and a status line with the date, user and record number.
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

**Adding a chrome.** A chrome is how a deck dresses the run screen. Write a `ChromeView` and override only what this deck sees differently: a pane, the tab titles, the menus, the status line, a bar under the panes, keys, an overlay. Everything else stays Classic. Secondary panes are dressed by each chrome too, so the same traffic data is btop's net box on the Ono-Sendai and a network usage report on the Hosaka.

**Adding a pane.** Extra panes are things you buy (`data/hub/panes.ron`), or that a firmware ships with. Give each one a point of view per chrome, not just a box.

**Adding a deck**
1. Write down its fantasy, feel, rules of thumb and comps here first.
2. Add it to `data/hub/hardware.ron` with firmware that reuses existing parts where they fit.
3. Build any new parts it needs (a `map`, `chrome` or `desktop` variant and its renderer). The part must work in every theme, on mono monitors too.
4. Give it a price and a blurb that sells the point of view.

## Shared rules for every part

- **Every theme, every monitor.** It must read on a mono tube (the starter's paper white, amber, green) as well as on Synthwave. Test on both.
- **Clicks land:** animations never move a click area.
- **One place for timings:** each part keeps its animation timings and strengths together (like `netmap::anim`), and they're display numbers, not balance.
- **Comfort:** no full-screen flashing faster than about 3 times a second, and anything intense has a reduced setting.
- **Native and web:** no new heavy dependencies, and it stays smooth in the browser.
