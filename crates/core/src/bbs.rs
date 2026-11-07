//! A BBS session: dial-in, splash, menus, file areas, Zmodem downloads, hang-up.
//! Pure navigation logic. [`Session::render`] returns pipe-coded lines for the app to paint,
//! [`Session::key`] takes single keypresses like a real board, [`Session::tick`] runs the modem clock.

use crate::data::{BbsDef, Drop, HeaderStyle, Look};
use crate::game::{Action, ContractStatus, Game, Reading, StockFile};
use crate::modem::Handshake;
use crate::tuning::{BBS_CPS, BBS_HANGUP_SECS};

/// Screen width in columns.
pub const COLS: usize = 78;
/// Screen height in rows.
pub const ROWS: usize = 24;
/// Seconds after CONNECT the ANSI line shows.
const MODEM_ANSI_AFTER: f32 = 0.15;

/// One post on a jobs board, wherever the board was reached from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobPost {
    pub client: String,
    pub reward: u32,
    /// What to hit: a node in a run, a network in the phonebook from home.
    pub target: String,
    /// Where it is: `layer 3`, or a phone number.
    pub detail: String,
    pub brief: String,
    /// Ghost bonus: (trace cap, bonus).
    pub ghost: Option<(u32, u32)>,
    pub taken: bool,
}

/// The other end of the line: everything a BBS session reads. A board reached in a run
/// ([`Game`]) or called from home ([`crate::hub::HomeCall`]).
pub trait Line {
    fn board(&self) -> &BbsDef;
    /// Today's listing: per area, the files on offer.
    fn stock(&self) -> &[Vec<StockFile>];
    fn jobs(&self) -> Vec<JobPost>;
    /// Credits you can spend: the deck's in a run, the bank's at home.
    fn credits(&self) -> u32;
    /// Where downloads land, and how much room is left there (`Free slots`, `Stash`).
    fn room(&self) -> (&'static str, u32);
    /// Why a file can't be bought right now, if it can't.
    fn buy_blocker(&self, area: usize, file: usize) -> Option<String>;
    /// Whether you own a desktop tool: an owned tool lists as yours, not for sale.
    fn has_tool(&self, id: &str) -> bool;
    /// Whether a textfile on this board is already on your disk.
    fn has_text(&self, id: &str) -> bool;
    /// Whether you have a cracked copy of a game: it lists as yours, like a tool. A boxed
    /// copy doesn't count; the board still sells the crack.
    fn has_crack(&self, id: &str) -> bool;
}

/// Whether a file is a tool or a game you already own: the listing marks it yours.
fn owned(game: &dyn Line, item: &Drop) -> bool {
    match item {
        Drop::Tool(t) => game.has_tool(t),
        Drop::Game(g) => game.has_crack(g),
        _ => false,
    }
}

impl Line for Game {
    fn board(&self) -> &BbsDef {
        self.bbs()
    }

    fn stock(&self) -> &[Vec<StockFile>] {
        &self.bbs_stock
    }

    fn jobs(&self) -> Vec<JobPost> {
        self.bbs_jobs
            .iter()
            .map(|j| {
                let n = &self.root().nodes[j.node];
                JobPost {
                    client: j.client.clone(),
                    reward: j.reward,
                    target: n.name.clone(),
                    detail: format!("layer {}", n.layer),
                    brief: j.brief.clone(),
                    ghost: j.ghost,
                    taken: j.status != ContractStatus::Offered,
                }
            })
            .collect()
    }

    fn credits(&self) -> u32 {
        self.deck.credits
    }

    fn room(&self) -> (&'static str, u32) {
        (
            "Free slots",
            self.deck.hdd_max.saturating_sub(self.hdd.len() as u32),
        )
    }

    fn buy_blocker(&self, area: usize, file: usize) -> Option<String> {
        Game::buy_blocker(self, area, file)
    }

    fn has_tool(&self, id: &str) -> bool {
        Game::has_tool(self, id)
    }

    fn has_text(&self, id: &str) -> bool {
        Game::has_text(self, id)
    }

    fn has_crack(&self, id: &str) -> bool {
        Game::has_crack(self, id)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Screen {
    Dialing {
        t: f32,
    },
    Splash,
    Main,
    Bulletins,
    Bulletin(usize),
    Who,
    Jobs,
    Areas,
    Area(usize),
    Download {
        area: usize,
        file: usize,
        t: f32,
    },
    /// Reading a textfile from an area's listing.
    Read {
        area: usize,
        file: usize,
    },
    /// The message bases.
    Messages,
    /// One base's posts.
    Base(usize),
    /// Reading a post.
    Post {
        base: usize,
        post: usize,
    },
    Hangup {
        t: f32,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing for the game to do.
    None,
    Act(Action),
    /// The line dropped; close the window.
    Closed,
}

#[derive(Clone, Debug)]
pub struct Session {
    pub screen: Screen,
    /// One-line message under the current screen (errors, transfer results).
    pub status: Option<String>,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    pub fn new() -> Self {
        Self {
            screen: Screen::Dialing { t: 0.0 },
            status: None,
        }
    }

    /// Advances modem time by `dt` seconds.
    pub fn tick(&mut self, game: &dyn Line, dt: f32) -> Outcome {
        match &mut self.screen {
            Screen::Dialing { t } => {
                *t += dt;
                if *t >= Handshake::new(&game.board().phone).end {
                    self.screen = Screen::Splash;
                }
            }
            Screen::Download { area, file, t } => {
                *t += dt;
                let (area, file) = (*area, *file);
                if *t >= transfer_secs(game, area, file) {
                    self.screen = Screen::Area(area);
                    // The game may still refuse (e.g. credits spent elsewhere); it logs why.
                    if let Some(why) = game.buy_blocker(area, file) {
                        self.status = Some(format!("|12Transfer aborted: {why}"));
                        return Outcome::None;
                    }
                    let name = &file_def(game, area, file).file;
                    self.status = Some(format!("|10Transfer complete. {name} received. |08CRC OK"));
                    return Outcome::Act(Action::Buy { area, file });
                }
            }
            Screen::Hangup { t } => {
                *t += dt;
                if *t >= BBS_HANGUP_SECS {
                    return Outcome::Closed;
                }
            }
            _ => {}
        }
        Outcome::None
    }

    /// The hotkeys the current screen accepts, with labels (for clickable buttons).
    pub fn keys(&self, game: &dyn Line) -> Vec<(char, String)> {
        let b = game.board();
        match self.screen {
            Screen::Dialing { .. } | Screen::Download { .. } | Screen::Hangup { .. } => Vec::new(),
            Screen::Splash => vec![('\n', "continue".into())],
            Screen::Main => {
                let mut v = vec![('F', "files".into())];
                if !b.messages.is_empty() {
                    v.push(('M', "messages".into()));
                }
                if b.jobs {
                    v.push(('J', "jobs".into()));
                }
                v.extend([
                    ('B', "bulletins".into()),
                    ('W', "who".into()),
                    ('G', "bye".into()),
                ]);
                v
            }
            Screen::Messages => numbered(b.messages.len(), "Q"),
            Screen::Base(i) => numbered(b.messages.get(i).map_or(0, |m| m.posts.len()), "Q"),
            Screen::Post { .. } => vec![('Q', "back".into())],
            Screen::Bulletins => numbered(b.bulletins.len(), "Q"),
            Screen::Jobs => numbered(game.jobs().len(), "Q"),
            Screen::Areas => numbered(b.areas.len(), "Q"),
            Screen::Area(a) => numbered(game.stock()[a].len(), "Q"),
            Screen::Read { area, file } => {
                let mut v = Vec::new();
                if read_kept(game, area, file) == Some(false) {
                    v.push(('D', "download".into()));
                }
                v.push(('Q', "back".into()));
                v
            }
            Screen::Bulletin(_) | Screen::Who => vec![('Q', "back".into())],
        }
    }

    /// Handles one keypress. Letters are case-insensitive; Escape counts as `Q`.
    pub fn key(&mut self, game: &dyn Line, key: char) -> Outcome {
        let k = key.to_ascii_uppercase();
        let back = k == 'Q' || k == '\x1b';
        let digit = k.to_digit(10).map(|d| d as usize);
        match self.screen {
            Screen::Dialing { .. } | Screen::Download { .. } | Screen::Hangup { .. } => {}
            Screen::Splash => self.go(Screen::Main),
            Screen::Main => match k {
                'F' => self.go(Screen::Areas),
                'M' if !game.board().messages.is_empty() => self.go(Screen::Messages),
                'J' if game.board().jobs => self.go(Screen::Jobs),
                'B' => self.go(Screen::Bulletins),
                'W' => self.go(Screen::Who),
                'G' | '\x1b' => self.go(Screen::Hangup { t: 0.0 }),
                _ => self.status = Some(format!("|12Unknown command '{}'.", printable(key))),
            },
            Screen::Bulletins => match digit {
                Some(n) if (1..=game.board().bulletins.len()).contains(&n) => {
                    self.go(Screen::Bulletin(n - 1));
                    return read(game, Reading::Bulletin(n - 1));
                }
                _ if back => self.go(Screen::Main),
                _ => {}
            },
            Screen::Bulletin(_) => {
                if back || k == '\n' {
                    self.go(Screen::Bulletins);
                }
            }
            Screen::Who => {
                if back || k == '\n' {
                    self.go(Screen::Main);
                }
            }
            Screen::Messages => match digit {
                Some(n) if (1..=game.board().messages.len()).contains(&n) => {
                    self.go(Screen::Base(n - 1))
                }
                _ if back => self.go(Screen::Main),
                _ => {}
            },
            Screen::Base(base) => match digit {
                Some(n) if (1..=game.board().messages[base].posts.len()).contains(&n) => {
                    let post = n - 1;
                    self.go(Screen::Post { base, post });
                    return read(game, Reading::Post { base, post });
                }
                _ if back => self.go(Screen::Messages),
                _ => {}
            },
            Screen::Post { base, .. } => {
                if back || k == '\n' {
                    self.go(Screen::Base(base));
                }
            }
            Screen::Read { area, file } => {
                if back || k == '\n' {
                    self.go(Screen::Area(area));
                } else if k == 'D' && read_kept(game, area, file) == Some(false) {
                    match game.buy_blocker(area, file) {
                        Some(why) => self.status = Some(format!("|12{why}")),
                        None => self.go(Screen::Download { area, file, t: 0.0 }),
                    }
                }
            }
            Screen::Jobs => match digit {
                Some(n) if (1..=game.jobs().len()).contains(&n) => {
                    if !game.jobs()[n - 1].taken {
                        self.status = Some("|10Job accepted. It's on your todo list.".into());
                        return Outcome::Act(Action::TakeJob { job: n - 1 });
                    }
                    self.status = Some("|12That job's already taken.".into());
                }
                _ if back => self.go(Screen::Main),
                _ => {}
            },
            Screen::Areas => match digit {
                Some(n) if (1..=game.board().areas.len()).contains(&n) => {
                    self.go(Screen::Area(n - 1))
                }
                _ if back => self.go(Screen::Main),
                _ => {}
            },
            Screen::Area(a) => match digit {
                Some(n) if (1..=game.stock()[a].len()).contains(&n) => {
                    let file = n - 1;
                    if matches!(file_def(game, a, file).item, Drop::Text(_)) {
                        self.go(Screen::Read { area: a, file });
                        return Outcome::None;
                    }
                    match game.buy_blocker(a, file) {
                        Some(why) => self.status = Some(format!("|12{why}")),
                        None => self.go(Screen::Download {
                            area: a,
                            file,
                            t: 0.0,
                        }),
                    }
                }
                _ if back => self.go(Screen::Areas),
                _ => {}
            },
        }
        Outcome::None
    }

    fn go(&mut self, s: Screen) {
        self.screen = s;
        self.status = None;
    }

    /// The whole screen as pipe-coded lines, at most [`ROWS`] of them, in the board's look.
    pub fn render(&self, game: &dyn Line) -> Vec<String> {
        let b = game.board();
        let l = &b.look;
        let [fr, k, lb, dm, tx, mo, ok, wn, aside] = [
            l.frame, l.key, l.label, l.dim, l.text, l.money, l.ok, l.warn, l.aside,
        ]
        .map(pipe);
        // `[K]` in the board's colours.
        let hot = |key: &str| format!("{fr}[{k}{key}{fr}]");
        let prompt = |what: &str| format!("   {tx}{what} {fr}{} {k}_", l.prompt);
        let pick = |what: &str| prompt(&format!("{what} {} {tx}or {}{tx}uit", hot("#"), hot("Q")));
        let mut out: Vec<String> = Vec::new();
        match self.screen {
            Screen::Dialing { t } => {
                let h = Handshake::new(&b.phone);
                out.push("|07ATZ".into());
                if t >= h.offhook {
                    out.push(format!("|07ATDT {}", b.phone));
                }
                for &ring in &h.rings {
                    if t >= ring {
                        out.push("|08~~ RING ~~".into());
                    }
                }
                if t >= h.connect {
                    out.push(format!("|15CONNECT {}/ARQ/V42BIS", b.baud));
                }
                if t >= h.connect + MODEM_ANSI_AFTER {
                    out.push("|07Negotiating ANSI... |10OK".into());
                    out.push(format!("|08{}", l.software));
                }
            }
            Screen::Splash => {
                out.extend(b.splash.lines().map(String::from));
                out.push(String::new());
                out.push(centered(&format!("{tx}Press {k}[ENTER]{tx} to continue")));
            }
            Screen::Main => {
                header(&mut out, b, "MAIN MENU");
                out.push(String::new());
                let m = &l.menu;
                let mut items = vec![('F', &m.files)];
                if !b.messages.is_empty() {
                    items.push(('M', &m.messages));
                }
                if b.jobs {
                    items.push(('J', &m.jobs));
                }
                items.extend([('B', &m.bulletins), ('W', &m.who), ('G', &m.goodbye)]);
                for (key, (label, note)) in items {
                    out.push(format!(
                        "   {} {lb}{label:<20}{dm}{note}",
                        hot(&key.to_string())
                    ));
                }
                out.push(String::new());
                out.push(format!(
                    "   {dm}Handle: {tx}{}   {dm}Credits: {mo}{}¢   {dm}{}: {tx}{}",
                    l.handle,
                    game.credits(),
                    game.room().0,
                    game.room().1
                ));
                out.push(String::new());
                out.push(prompt("Command"));
            }
            Screen::Bulletins => {
                header(&mut out, b, "BULLETINS");
                out.push(String::new());
                for (i, bl) in b.bulletins.iter().enumerate() {
                    out.push(format!("   {} {tx}{}", hot(&(i + 1).to_string()), bl.title));
                }
                out.push(String::new());
                out.push(pick("Read which?"));
            }
            Screen::Bulletin(i) => {
                let bl = &b.bulletins[i];
                header(&mut out, b, &format!("BULLETIN #{}", i + 1));
                out.push(format!("   {mo}{}", bl.title));
                out.push(format!("   {dm}{}", "─".repeat(bl.title.chars().count())));
                out.extend(bl.body.iter().map(|line| format!("   {tx}{line}")));
                out.push(String::new());
                out.push(format!("   {} {tx}back", hot("Q")));
            }
            Screen::Messages => {
                header(&mut out, b, &l.menu.messages.0.to_uppercase());
                out.push(String::new());
                for (i, m) in b.messages.iter().enumerate() {
                    out.push(format!(
                        "   {} {lb}{:<26}{dm}{} message(s)",
                        hot(&(i + 1).to_string()),
                        m.name,
                        m.posts.len()
                    ));
                }
                out.push(String::new());
                out.push(pick("Read which base?"));
            }
            Screen::Base(i) => {
                let m = &b.messages[i];
                header(&mut out, b, &m.name);
                out.push(String::new());
                out.push(format!("   {dm}{:<4}{:<16}{}", "#", "FROM", "SUBJECT"));
                out.push(format!("   {dm}{}", "─".repeat(60)));
                for (n, p) in m.posts.iter().enumerate() {
                    let subject: String = p.subject.chars().take(50).collect();
                    out.push(format!(
                        "   {} {k}{:<16}{tx}{subject}",
                        hot(&(n + 1).to_string()),
                        p.from.chars().take(15).collect::<String>()
                    ));
                }
                out.push(String::new());
                out.push(pick("Read which?"));
            }
            Screen::Post { base, post } => {
                let m = &b.messages[base];
                let p = &m.posts[post];
                header(&mut out, b, &format!("{} #{}", m.name, post + 1));
                out.push(format!(
                    "   {dm}From: {k}{}   {dm}Subj: {mo}{}",
                    p.from, p.subject
                ));
                out.push(format!("   {dm}{}", "─".repeat(60)));
                out.extend(p.body.iter().map(|line| format!("   {tx}{line}")));
                out.push(String::new());
                out.push(format!("   {} {tx}back", hot("Q")));
            }
            Screen::Who => {
                header(&mut out, b, "WHO'S ONLINE");
                out.push(String::new());
                out.push(format!(
                    "   {dm}{:<6}{:<14}{:<16}{}",
                    "NODE", "HANDLE", "FROM", "DOING"
                ));
                out.push(format!("   {dm}{}", "─".repeat(60)));
                for (i, u) in b.users.iter().enumerate() {
                    out.push(format!(
                        "   {tx}{:<6}{k}{:<14}{tx}{:<16}{aside}{}",
                        i + 1,
                        u.handle,
                        u.location,
                        u.doing
                    ));
                }
                let n = b.users.len() + 1;
                out.push(format!(
                    "   {tx}{n:<6}{mo}{:<14}{tx}{:<16}{aside}{}",
                    l.handle, "unknown", "This screen"
                ));
                out.push(String::new());
                out.push(format!("   {} {tx}back", hot("Q")));
            }
            Screen::Jobs => {
                header(&mut out, b, "JOBS BOARD");
                out.push(String::new());
                let jobs = game.jobs();
                if jobs.is_empty() {
                    out.push(format!(
                        "   {dm}Nothing posted. Check back next time you dial in."
                    ));
                }
                for (i, j) in jobs.iter().enumerate() {
                    let taken = j.taken;
                    let (num, name) = if taken { (&dm, &dm) } else { (&k, &lb) };
                    let line = format!(
                        "   {fr}[{num}{}{fr}] {name}{:<24}{mo}{:>5}¢ {dm}target {tx}{} {dm}{}",
                        i + 1,
                        j.client,
                        j.reward,
                        j.target,
                        j.detail
                    );
                    out.push(crate::ansi::clip(&line, COLS));
                    let brief: String = j.brief.chars().take(68).collect();
                    out.push(format!("       {tx}{brief}"));
                    match (taken, j.ghost) {
                        (true, _) => out.push(format!("       {dm}[taken]")),
                        (false, Some((cap, bonus))) => out.push(format!(
                            "       {aside}ghost bonus {mo}+{bonus}¢ {aside}if trace stays at or under {cap}%"
                        )),
                        (false, None) => {}
                    }
                    out.push(String::new());
                }
                out.push(pick("Take which?"));
            }
            Screen::Areas => {
                header(&mut out, b, "FILE AREAS");
                out.push(String::new());
                for (i, a) in b.areas.iter().enumerate() {
                    let left = game.stock()[i]
                        .iter()
                        .filter(|s| !s.bought && !owned(game, &a.files[s.def].item))
                        .count();
                    out.push(format!(
                        "   {} {lb}{:<26}{dm}{left} file(s)",
                        hot(&(i + 1).to_string()),
                        a.name
                    ));
                }
                out.push(String::new());
                out.push(pick("Area?"));
            }
            Screen::Area(a) => {
                let area = &b.areas[a];
                header(&mut out, b, &format!("AREA {}: {}", a + 1, area.name));
                out.push(String::new());
                out.push(format!(
                    "   {dm}{:<3}{:<14}{:>8}{:>7}  {}",
                    "#", "FILENAME", "SIZE", "CRED", "DESCRIPTION"
                ));
                out.push(format!("   {dm}{}", "─".repeat(70)));
                for (i, s) in game.stock()[a].iter().enumerate() {
                    let f = &area.files[s.def];
                    let size = file_size(&f.file);
                    let owned = owned(game, &f.item);
                    if s.bought || owned {
                        let note = if owned { "[yours]" } else { "[downloaded]" };
                        out.push(format!(
                            "   {dm}{:<3}{:<14}{:>7}k{:>7}  {note}",
                            i + 1,
                            f.file,
                            size / 1024,
                            "--"
                        ));
                    } else {
                        let price = if game.credits() >= f.price { &mo } else { &wn };
                        let kept = matches!(&f.item, Drop::Text(t) if game.has_text(t));
                        let cost = if kept {
                            "on disk".to_string()
                        } else if f.price == 0 {
                            "free".to_string()
                        } else {
                            format!("{}¢", f.price)
                        };
                        out.push(format!(
                            "   {k}{:<3}{lb}{:<14}{tx}{:>7}k{price}{cost:>7}  {tx}{}{}",
                            i + 1,
                            f.file,
                            size / 1024,
                            f.desc,
                            drop_tag(&f.item, l)
                        ));
                    }
                }
                out.push(String::new());
                out.push(format!("   {dm}Credits: {mo}{}¢", game.credits()));
                out.push(pick("Download"));
            }
            Screen::Download { area, file, t } => {
                let f = file_def(game, area, file);
                let size = file_size(&f.file);
                let total = transfer_secs(game, area, file);
                let frac = (t / total).clamp(0.0, 1.0);
                header(&mut out, b, "ZMODEM TRANSFER");
                out.push(String::new());
                out.push(format!(
                    "   {dm}Protocol {tx}Zmodem      {dm}File {k}{}",
                    f.file
                ));
                out.push(format!(
                    "   {dm}Size     {tx}{size} bytes   {dm}Cost {mo}{}¢",
                    f.price
                ));
                out.push(String::new());
                let cells = 50;
                let on = (frac * cells as f32) as usize;
                out.push(format!(
                    "   {dm}[{ok}{}{dm}{}{dm}] {k}{:>3}%",
                    "█".repeat(on),
                    "░".repeat(cells - on),
                    (frac * 100.0) as u32
                ));
                let sent = (size as f32 * frac) as u32;
                out.push(format!(
                    "   {dm}Bytes {tx}{sent:>7}   {dm}CPS {tx}{:>6}   {dm}ETA {tx}{:.1}s",
                    BBS_CPS as u32,
                    (total - t).max(0.0)
                ));
                out.push(String::new());
                out.push(format!("   {dm}rz waiting to receive.**B0100000023be50"));
            }
            Screen::Read { area, file } => {
                let f = file_def(game, area, file);
                header(&mut out, b, &f.file);
                let text = match &f.item {
                    Drop::Text(id) => b.texts.iter().find(|t| t.id == *id),
                    _ => None,
                };
                if let Some(t) = text {
                    out.push(format!("   {mo}{}", t.title));
                    out.push(format!("   {dm}{}", "─".repeat(t.title.chars().count())));
                    out.extend(t.lines.iter().map(|line| format!("   {tx}{line}")));
                }
                out.push(String::new());
                if read_kept(game, area, file) == Some(false) {
                    out.push(format!(
                        "   {} {tx}download to disk   {} {tx}back",
                        hot("D"),
                        hot("Q")
                    ));
                } else {
                    out.push(format!("   {} {tx}back   {dm}on your disk", hot("Q")));
                }
            }
            Screen::Hangup { t } => {
                out.push(format!("|07{}", l.goodbye.replace("{board}", &b.name)));
                if t > 0.2 {
                    out.push("|08+++".into());
                }
                if t > 0.5 {
                    out.push("|08ATH0".into());
                }
                if t > 0.8 {
                    out.push("|15NO CARRIER".into());
                }
            }
        }
        if let Some(s) = &self.status {
            while out.len() < ROWS - 2 {
                out.push(String::new());
            }
            out.push(format!("   {s}"));
        }
        out.truncate(ROWS);
        // A l33t board speaks 31337. The modem (dialing, hang-up) and the splash art don't.
        if l.leet
            && !matches!(
                self.screen,
                Screen::Dialing { .. } | Screen::Hangup { .. } | Screen::Splash
            )
        {
            out = out.iter().map(|l| crate::leet::translate(l)).collect();
        }
        out
    }
}

/// Reading a bulletin or post: its effect, if it has one, goes to the game.
fn read(game: &dyn Line, r: Reading) -> Outcome {
    match r.effect(game.board()) {
        Some(_) => Outcome::Act(Action::Read(r)),
        None => Outcome::None,
    }
}

/// The pipe code for CGA colour `c`.
fn pipe(c: u8) -> String {
    format!("|{c:02}")
}

fn numbered(n: usize, back: &str) -> Vec<(char, String)> {
    let mut v: Vec<(char, String)> = (1..=n.min(9))
        .filter_map(|i| char::from_digit(i as u32, 10).map(|c| (c, String::new())))
        .collect();
    v.push((back.chars().next().unwrap_or('Q'), "back".into()));
    v
}

fn printable(c: char) -> String {
    if c.is_control() {
        format!("^{}", ((c as u8) + b'@') as char)
    } else {
        c.to_string()
    }
}

/// The header across the top of a menu screen, exactly [`COLS`] wide, in the board's style.
fn header(out: &mut Vec<String>, b: &BbsDef, title: &str) {
    let l = &b.look;
    let (fr, k, bar) = (pipe(l.frame), pipe(l.key), format!("|{}", 16 + l.bar));
    let left = format!(" {} ", b.name);
    let right = format!(" {title} ");
    let used = left.chars().count() + right.chars().count();
    out.push(match l.header {
        HeaderStyle::Bar => format!(
            "{fr}▄▄{bar}{k}{left}|16{fr}{}{bar}{k}{right}|16{fr}▄▄",
            "▄".repeat(COLS.saturating_sub(used + 4))
        ),
        HeaderStyle::Rule => format!(
            "{fr}══[{k}{left}{fr}]{}[{k}{right}{fr}]══",
            "═".repeat(COLS.saturating_sub(used + 8))
        ),
        HeaderStyle::Inverse => format!(
            "{bar}{k}{left}{}{right}|16",
            " ".repeat(COLS.saturating_sub(used))
        ),
    });
}

fn centered(line: &str) -> String {
    let pad = COLS.saturating_sub(crate::ansi::visible_len(line)) / 2;
    format!("{}{line}", " ".repeat(pad))
}

/// For a textfile in the listing: whether it's already on your disk. `None` if it isn't one.
fn read_kept(game: &dyn Line, area: usize, file: usize) -> Option<bool> {
    match &file_def(game, area, file).item {
        Drop::Text(id) => Some(game.has_text(id)),
        _ => None,
    }
}

fn file_def(game: &dyn Line, area: usize, file: usize) -> &crate::data::BbsFileDef {
    let s = game.stock()[area][file];
    &game.board().areas[area].files[s.def]
}

/// A stable fake file size from the filename.
pub fn file_size(name: &str) -> u32 {
    let h = name.bytes().fold(0x811c_9dc5u32, |h, b| {
        (h ^ u32::from(b)).wrapping_mul(0x0100_0193)
    });
    40_000 + h % 90_000
}

fn transfer_secs(game: &dyn Line, area: usize, file: usize) -> f32 {
    file_size(&file_def(game, area, file).file) as f32 / BBS_CPS
}

fn drop_tag(item: &Drop, l: &Look) -> String {
    match item {
        Drop::Warez(_) => String::new(),
        Drop::Hint(_) => format!(" {}[TXT]", pipe(l.ok)),
        Drop::Text(_) => format!(" {}[READ]", pipe(l.ok)),
        Drop::Tool(_) | Drop::Game(_) => format!(" {}[EXE]", pipe(l.key)),
        _ => format!(" {}[MOD]", pipe(l.aside)),
    }
}
