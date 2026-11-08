# The world

Who's who on the Net, and who knows what. Read this before writing a board, a post, a textfile, a cracktro, a splash, a magazine page or a contract brief. `docs/design.md` says how each deck looks at the world; this says what the world is.

## The rule: everything speaks from where it is

Every text in the game has an author inside the world, and it knows only what that author would know. The rule is in `CLAUDE.md`; this table is it applied.

| Author | Knows | Would never say |
|---|---|---|
| A board's sysop (bulletins, area names, file blurbs, the goodbye line) | the board's own menus, its callers, its stock, the other boards by reputation | your desktop, your deck's settings, a key to press on your machine |
| A poster on a board | what they've seen and heard, rumours, a number they were given | the game's rules as rules |
| A textfile (G-FILES, PHREAK TEXTS) | a craft: wardialing, ranges, heat, the telco; written to teach | the names of our apps, panes or themes |
| A crew's cracktro | its crew, its greetz, the board it calls home, the program it cracked | the publisher's side of the story, your save |
| A publisher's splash | its product, its version, its slogan | any board, any crew, any phone number but a 1-800 |
| CPU Shopper (editors, advertisers) | products, prices, 1-800 hotlines, what's reviewed this issue | your deck, your settings app, a key to press, a board's 555 number |
| Sense/Net Customer Care | its deck, its welcome stack, its partners, its terms of service | the illicit boards (it deletes posts about them) |
| CorpLink's help desk | policy, requisitions, memos, who's logged in | that anyone outside the company is reading |
| The OS (menus, windows, System) | its own windows and the tools you own | what a magazine printed, what you haven't found |
| A contract client (MSG nodes, the Jobs board) | its grudge and its target | more than it needs you to know |

When a screen needs to tell the player something its author couldn't know, it's the wrong screen: put it where the in-world author would.

## The boards

Three boards turn up on networks, one per tier, and one customer line you call from home. Each sells a step up from the last. They are folders under `data/bbs/`; this is who runs them.

### THE NEON GUTTER (`gutter`, 555-0199)

**The scene board.** The hangout on tier-1 LANs and the default board everywhere. Sysop **m0th**, who runs it from Ninsei on a 14400 line and writes the bulletins in l33t. The regulars are **c4se** (Chiba, wrote HexEd's checksum), **ratz** (Night City, never reads her mail), **zero_kool** (Hamburg, always in a door game), and whoever's new.

What it's for: games, demos, cracks, the starter tools and the first breakers. WAREZ TOOLS is free and always stocked (the wardialer, the hex editor, the QWK reader, the ICE bestiary: m0th's own and his friends'). FREEWARE gives a new deck its first breaker. G-FILES teach the craft. TEXTFILES sell leaked phone lists, OPTIMIZERS sell fragments, and the ICEBREAKERS and DECK MODS areas carry the basics at honest prices. GAMES carries the crews' cracks, cheap, two a call, the file named in 8.3 with the crew's tag (`NIBB-2K4.ZIP`, `BLOX-PHS.ZIP`, `MINE-PHS.ZIP`) and described the way m0th would ("PHOSPHOR, the real one"); the bulletin says so ("Kids, keep it clean"), and c4se is usually playing NIBBLER.

The overlap is on purpose: the wardialer is a hacker's tool, and every kid on the Gutter starts with it. The Gutter is where you learn; Dead Sector is where you go once you have.

Voice: friendly, cocky, rules in three lines. "Don't leech. Upload or pay creds." Blue bars, l33t headers, filenames and numbers kept readable. It would never stock hot hardware or a core prefix, and it would never post a corp address.

### DEAD SECTOR (`deadsector`, 555-0666)

**The hackers' and phreaks' board.** An anarchy board at 2400 baud on tier-2 corp nets, and anywhere a pirate runs it off a backbone (the *pirate board* trait). Sysop **d1rge**, location `???`, who is "Watching you". Regulars: **lag** (Sprawl, upload stalled since forever), **acid_burn** (Chiba), and ghosts.

What it's for: hot breakers (used, ripped, modded: wyrm, scrubber, lagspike), hot hardware that fell off a truck, the first core prefixes, and NiGHTCOMM, the terminal the scene runs. Its textfiles are phreaking, not tutorials. No games, no kids, no freeware: if it's free here, it's bait.

Voice: paranoid, short, red on black, box-drawn rules. "No names. No faces. No real numbers." It knows the Gutter and tolerates it. It would never run a door game, never explain anything twice, and never admit it exists ("We never met").

### CORPLINK EMPLOYEE SERVICES (`corplink`, 555-0100)

**The office.** The staff intranet on tier-3 megacorp backbones, PCBoard Corporate Edition, 28800 baud, and you're a guest login (`GUEST-0451`, billed). The sysop is the **IT Help Desk**; the Office of Information Security writes the memos. Staff online: **J.TANAKA** in Accounts, **R.OKONKWO** in Security, a nightly backup job.

What it's for: licensed software (TORCH, Mirror Enterprise, log maintenance, a process freezer, a QA worm nobody should run), hardware requisitions on forms, internal directories that are really core ranges, and a site licence for Tiger Team Pro. Priced like an expense report.

Voice: policy. Inverse blue headers, no l33t, a cafeteria menu. It would never call a breaker a breaker, never use a handle, and never acknowledge that its guest accounts are how it gets robbed.

### SENSE/NET CUSTOMERLINK (`sensenet`, 555-0001)

**The customer line.** Free, in every new deck's phonebook, never a network's board. Run by **Customer Care** from Sense/Net HQ. Customers: **dot_matrix** (just got a deck, wants bigger text), **pixelpusher** (modems), the Ninsei Sense/Net User Group (first Tuesday, the noodle bar on 4th), and **n1ghtowl**, location unlisted, whose post about the lines being tame gives the Gutter's number and gets deleted.

What it's for: the first two doors. BUSINESS PARTNERS puts the first corporate dial-ups in your phonebook; n1ghtowl's post is the way to the Gutter. Support Files are a modem guide, tips and manners.

Voice: cheerful, nine to five. "Thank you for choosing Sense/Net!" It would never name an illicit board, never mention cracks, and never know about the disk somebody at the shop dubbed for you (task 47.6).

## The crews

A crack is signed. Each crew has a voice, a signature in its scroller, a style of tune, and people it greets. Greetz name only handles, crews and boards that exist on this page.

**the NiGHTMOTHS.** m0th's crew, the Gutter's house crew. They crack tools, not games, and give them away: WarDial, HexEd, ICE DB and the QWK reader all carry their mark (`data/intros/`). Signature: a big block-letter logo on a sine, the Gutter's ad underneath, square-wave leads over a triangle bass. Greet c4se, ratz, zero_kool, the Dead Sector crew, "kr4ken (no hard feelings)" and the phreaks of 555-7. They feud with kr4ken, politely.

**PHOSPHOR.** The art crew. The best ANSI and the best tunes on the scene, and the first to crack a game: a PHOSPHOR crack is the one everybody copies. Signature: a logo that fills the screen, colour cycling, a fast arpeggio lead at 150+ bpm. They greet the NiGHTMOTHS and other artists, never the kids. Chiba. Shipped: BLOX and MINEFIELD (`data/intros/blox-crack.ron`, `minefield-crack.ron`, tag PHS): the logo cycling across the top, a piece falling down a well or a field opened square by square until it finds a mine, credits signed by the crew alone, at 168 and 156 bpm.

**THE 2400 CLUB.** Kids with slow modems and fast fingers. Their cracks are late and sloppy, their frames cramped, their greetz lists twenty names long with spelling. Signature: a bouncing one-line logo, a two-track tune that rushes. They greet everyone, including people who don't know them, and want PHOSPHOR to notice. m0th lets them on the Gutter. Dead Sector doesn't. Shipped: NIBBLER (`data/intros/nibbler-crack.ron`, tag 2K4), "krakked" late, in a cramped frame with a snake that doesn't fit it, eighteen greetz with asides ("PHOSPHOR (plz call!!)", "DEAD SECTOR (let us in!!)") and a two-track tune at 188 bpm; it begs PHOSPHOR to call and asks for an ANSI artist, paying in warez.

**RAZORWIRE.** d1rge's people. They crack breakers and deck firmware for Dead Sector, and NiGHTCOMM is theirs ("d1rge & DEAD SECTOR" on the scroller). Never games, never greetz to kids; they greet the 555 phreaks, "everyone who ever hit a trap line", and "n0 0n3". Signature: a thin frame, red, a bass-heavy tune that doesn't hurry.

**kr4ken.** Not a crew: a handle everyone greets and nobody has met. Keep it that way.

## The publishers and vendors

Boxed software is legitimate, friendly and knows nothing about the scene.

**Paper Tiger Software.** Boxed games by mail order, the home-software house in CPU Shopper's Games section (`papertiger` in `shopper.ron`): a Box ad for BLOX 2.0 ("Seven shapes. Just one more game!"), the other games as dot-leader lines. Warranty cards, a 1-800 line (1-800-PAPER-TG), "fun for the whole deck". Its splashes are a tagline and a logo: no crew, no greetz, no board. Each is the logo in a frame, the product and its version, a one-line pitch, Loading then Ready, and a soft sine jingle (`data/intros/*-splash.ron`).

The magazine's advertisers (`data/hub/shopper.ron`), each with a voice:

| Vendor | Sells | Voice |
|---|---|---|
| Ono-Sendai Direct | the Cyberspace VII | "JACK IN TODAY"; looks and feel only, no performance claims |
| Hosaka Corporate Sales | the Executive 4 | volume licensing, installed by a technician |
| Sense/Net Home Computing | the Personal Deck | "the computer for the rest of us" |
| Chiba Micro Supply | RAM, CPU, monitors, panes | fast delivery, prices climb as your rig grows, all sales final |
| Desk Accessory Warehouse | desktop add-ons | only for the desktops you run |
| Tiger Team Systems | Tiger Team Pro | "for authorized security testing only" |
| Paper Tiger Software | boxed games | "fun for the whole deck"; warranty cards, never a crew or a board |
| d1rge mail order | NiGHTCOMM, in a plain wrapper | "no questions"; the one advertiser who's also a sysop, and the magazine doesn't know |

A magazine can name products, boards by reputation and 1-800 numbers; it never prints a 555 number, never names your settings app, and never tells you a key to press.

## The decks

A deck is a computer on your desk, sold by a company with a point of view (`docs/design.md` says how each one looks; `data/hub/hardware.ron` holds them). The world knows them as products.

**Sense/Net Personal Deck** (Liza 1.0). Everybody's first deck, from the simstim giant that sells experiences to ordinary people. It comes in the box with a welcome stack, a Security Suite (a firewall, a patch kit and a plain icepick, free, yours for good), the Personal Display 9" and one free line, CustomerLink. Its OS is **Liza**: a menu bar, a Finder, a Trash, everything labelled. Sense/Net's face is Customer Care: cheerful, nine to five, and it would rather you never met the people on its message boards. The deck is clean when it ships; anything cracked on it, somebody else put there.

**Ono-Sendai Cyberspace VII** (Cyberspace 7.0, or 6.2 with the classic panes). The cowboy's deck, "JACK IN TODAY", by mail order from Ono-Sendai Direct. It sees the net as wireframes over a grid and runs **AnarchyOS**, a tiling desktop with workspaces, a launcher and a bar; the run screen is a btop rice with panes you buy. The scene uses it and so do the crews: a PHOSPHOR cracktro looks best on it. Ono-Sendai swaps firmware free, forever. It sells a line: the **Cyberspace V** (the cowboy's first real deck, a smaller board in the same case), the VII, and the **Cyberspace IX** (every slot wired, every bay open), all on the same firmware.

**Hosaka Executive 4** (PlanView 4.0). The corporate workstation, "the office of tomorrow", sold by Hosaka Corporate Sales on volume licences and installed by a technician. It sees the net as a plan, every node drafted and filed, and runs **Desktop**, a Workbench-style OS; a run is a productivity suite with forms, a spreadsheet and a report. Corp staff and contractors with expense accounts use it. It is the deck CorpLink expects to see, and the deck a Tiger Team auditor carries. Hosaka sells a line too: the **Executive 2** (the branch-office model), the 4, and the **Executive 6** (the boardroom model, full expansion and a service contract), all on PlanView 4.0.

**A deck is a chassis.** A model's look is its brand's firmware; what it adds is room. Each model ships with the same numbers and tops out somewhere: upgrades you buy, deck mods from a board and loot only go as far as the case takes them, and a bigger model takes more. The makers sell that in their own words ("upgradeable", "every slot wired", "full expansion in every bay"); the magazine prints the models and the prices, never what's in your case, and the OS is the one that tells you yours is full. No model is faster at the same numbers, and no in-world text claims otherwise. Monitors are real period tubes (Sense/Net's paper-white, a Zenith amber, a Hercules green, Apple's Monitor II, Commodore's 1702, IBM's 8513) and they change what you see, never what happens.

## CPU Shopper

**The magazine.** `data/hub/shopper.ron` dresses it: a mail-order computer magazine after Computer Shopper (1986), VOL. 9 NO. 11, 2¢ at the newsstand, "over 880 pages, 412 advertisers, 1,900 products". RIG is one of its issues: you don't open a shop, you flip through a magazine and order from an ad.

**Who writes it.** Editors who have never seen your deck. They test hardware ("our lab jacks in"), run shootouts, and write cover lines; the advertisers write the ads in their own voices (the vendor table above). Sections, each exactly once: Decks, Upgrades, Monitors, Desktop add-ons, Run panes, Mail-order software, and Games (Paper Tiger's price list and its BLOX ad; the cover's NEW line, "BLOX 2.0 IS HERE", points there).

**What it knows.** Products, prices, versions, 1-800 hotlines, which deck an add-on fits, what's a REVIEW COPY this issue. It prices nothing itself: every item is priced in its own data file and turns up in the next issue on its own, as a dot-leader line until somebody writes it an ad.

**What it never says.** A 555 number, a board's name as a place to go, your settings app, a key to press, anything about your save. Prices that climb as your rig grows are Chiba Micro Supply's terms, printed as terms. d1rge mail order advertises NiGHTCOMM in a plain wrapper, and the magazine runs the ad without asking who d1rge is.

**Buying.** You order from the page; it arrives on your desktop (software), in your deck (upgrades) or on your desk (monitors, decks). Buying never plugs in or switches to what you bought: a new deck sits beside the old one until you switch to it. Credits only, from your bank.

## The companies

The corporations behind the networks (`data/hub/networks.ron`), as the phonebook, a leaked list or a contract brief would name them. Deck makers: **Sense/Net** (the simstim giant, consumer decks), **Ono-Sendai** (the hacker's deck), **Hosaka** (the corpo deck). Monitor makers are real period names (Zenith, Hercules, Apple, Commodore, IBM).

| Company | Business | Face |
|---|---|---|
| Ninsei Community College | school | the `always` starter net: lightly watched, nothing worth taking, its board is the Gutter |
| Ono Drycleaning, Kowloon Noodle Co., Redline Couriers, Velvet Static | laundry, food, delivery, a nightclub | small LANs; the Gutter's tier |
| Brightwater Mutual | insurance | claim records it would rather nobody audits; leaks its lines through temps (555-7) |
| Yamatai Dynamics | robotics | an R&D directory worth a core prefix; hires runners against rival labs |
| Lazarus Clinic | medical | patient files; a client that wants a node derezzed, no copies |
| Chiba Freight Co-op | logistics | cargo manifests; a client |
| Hollow Pines Credit | banking | |
| Saito Pharmaceutical | pharma | lab lines sold on Dead Sector |
| Glasswing Media | news | |
| Mirrorshade Optics | optics | |
| Tessier Holdings | real estate | |
| Northgate Penal | prisons | |
| Kestrel Aerospace | defense | the black site; the telco watches its prefixes |
| Ironbark Utilities | power grid | |

Traits are how a company's habits show on a run (`networks.ron`): graveyard shifts, night shifts, a pirate board on the backbone. A textfile can hint at them in the company's own terms; the OS never explains them.

## Numbers

The 555 exchange is the whole world. Ranges are the digits you know (`555-7` is Brightwater's block). Two boards never share a number. A new deck knows exactly two: CustomerLink (555-0001) and the college (555-0420); everything else is read, scanned, looted or given. Hotlines in the magazine are 1-800 decoration and never dial.
