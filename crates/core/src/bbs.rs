//! A BBS session: dial-in, splash, menus, file areas, Zmodem downloads, hang-up.
//! Pure navigation logic. [`Session::render`] returns pipe-coded lines for the app to paint,
//! [`Session::key`] takes single keypresses like a real board, [`Session::tick`] runs the modem clock.

use crate::data::{BbsDef, Drop, HeaderStyle, Look};
use crate::game::{Action, ContractStatus, Game};
use crate::modem::Handshake;
use crate::tuning::{BBS_CPS, BBS_HANGUP_SECS};

/// Screen width in columns.
pub const COLS: usize = 78;
/// Screen height in rows.
pub const ROWS: usize = 24;
/// Seconds after CONNECT the ANSI line shows.
const MODEM_ANSI_AFTER: f32 = 0.15;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Screen {
    Dialing { t: f32 },
    Splash,
    Main,
    Bulletins,
    Bulletin(usize),
    Who,
    Jobs,
    Areas,
    Area(usize),
    Download { area: usize, file: usize, t: f32 },
    Hangup { t: f32 },
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
    pub fn tick(&mut self, game: &Game, dt: f32) -> Outcome {
        match &mut self.screen {
            Screen::Dialing { t } => {
                *t += dt;
                if *t >= Handshake::new(&game.bbs().phone).end {
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
    pub fn keys(&self, game: &Game) -> Vec<(char, String)> {
        let b = game.bbs();
        match self.screen {
            Screen::Dialing { .. } | Screen::Download { .. } | Screen::Hangup { .. } => Vec::new(),
            Screen::Splash => vec![('\n', "continue".into())],
            Screen::Main => vec![
                ('F', "files".into()),
                ('J', "jobs".into()),
                ('B', "bulletins".into()),
                ('W', "who".into()),
                ('G', "bye".into()),
            ],
            Screen::Bulletins => numbered(b.bulletins.len(), "Q"),
            Screen::Jobs => numbered(game.bbs_jobs.len(), "Q"),
            Screen::Areas => numbered(b.areas.len(), "Q"),
            Screen::Area(a) => numbered(game.bbs_stock[a].len(), "Q"),
            Screen::Bulletin(_) | Screen::Who => vec![('Q', "back".into())],
        }
    }

    /// Handles one keypress. Letters are case-insensitive; Escape counts as `Q`.
    pub fn key(&mut self, game: &Game, key: char) -> Outcome {
        let k = key.to_ascii_uppercase();
        let back = k == 'Q' || k == '\x1b';
        let digit = k.to_digit(10).map(|d| d as usize);
        match self.screen {
            Screen::Dialing { .. } | Screen::Download { .. } | Screen::Hangup { .. } => {}
            Screen::Splash => self.go(Screen::Main),
            Screen::Main => match k {
                'F' => self.go(Screen::Areas),
                'J' => self.go(Screen::Jobs),
                'B' => self.go(Screen::Bulletins),
                'W' => self.go(Screen::Who),
                'G' | '\x1b' => self.go(Screen::Hangup { t: 0.0 }),
                _ => self.status = Some(format!("|12Unknown command '{}'.", printable(key))),
            },
            Screen::Bulletins => match digit {
                Some(n) if (1..=game.bbs().bulletins.len()).contains(&n) => {
                    self.go(Screen::Bulletin(n - 1))
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
            Screen::Jobs => match digit {
                Some(n) if (1..=game.bbs_jobs.len()).contains(&n) => {
                    if game.bbs_jobs[n - 1].status == ContractStatus::Offered {
                        self.status = Some("|10Job accepted. It's on your todo list.".into());
                        return Outcome::Act(Action::TakeJob { job: n - 1 });
                    }
                    self.status = Some("|12That job's already taken.".into());
                }
                _ if back => self.go(Screen::Main),
                _ => {}
            },
            Screen::Areas => match digit {
                Some(n) if (1..=game.bbs().areas.len()).contains(&n) => {
                    self.go(Screen::Area(n - 1))
                }
                _ if back => self.go(Screen::Main),
                _ => {}
            },
            Screen::Area(a) => match digit {
                Some(n) if (1..=game.bbs_stock[a].len()).contains(&n) => {
                    let file = n - 1;
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
    pub fn render(&self, game: &Game) -> Vec<String> {
        let b = game.bbs();
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
                for (key, (label, note)) in [
                    ('F', &m.files),
                    ('J', &m.jobs),
                    ('B', &m.bulletins),
                    ('W', &m.who),
                    ('G', &m.goodbye),
                ] {
                    out.push(format!(
                        "   {} {lb}{label:<20}{dm}{note}",
                        hot(&key.to_string())
                    ));
                }
                out.push(String::new());
                out.push(format!(
                    "   {dm}Handle: {tx}{}   {dm}Credits: {mo}{}¢   {dm}Free slots: {tx}{}",
                    l.handle,
                    game.deck.credits,
                    game.deck.hdd_max.saturating_sub(game.hdd.len() as u32)
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
                if game.bbs_jobs.is_empty() {
                    out.push(format!(
                        "   {dm}Nothing posted. Check back next time you dial in."
                    ));
                }
                for (i, j) in game.bbs_jobs.iter().enumerate() {
                    let n = &game.root().nodes[j.node];
                    let taken = j.status != ContractStatus::Offered;
                    let (num, name) = if taken { (&dm, &dm) } else { (&k, &lb) };
                    out.push(format!(
                        "   {fr}[{num}{}{fr}] {name}{:<24}{mo}{:>5}¢ {dm}target {tx}{} {dm}layer {}",
                        i + 1,
                        j.client,
                        j.reward,
                        n.name,
                        n.layer
                    ));
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
                    let left = game.bbs_stock[i].iter().filter(|f| !f.bought).count();
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
                for (i, s) in game.bbs_stock[a].iter().enumerate() {
                    let f = &area.files[s.def];
                    let size = file_size(&f.file);
                    if s.bought {
                        out.push(format!(
                            "   {dm}{:<3}{:<14}{:>7}k{:>7}  [downloaded]",
                            i + 1,
                            f.file,
                            size / 1024,
                            "--"
                        ));
                    } else {
                        let price = if game.credits_ok(f.price) { &mo } else { &wn };
                        out.push(format!(
                            "   {k}{:<3}{lb}{:<14}{tx}{:>7}k{price}{:>6}¢  {tx}{}{}",
                            i + 1,
                            f.file,
                            size / 1024,
                            f.price,
                            f.desc,
                            drop_tag(&f.item, l)
                        ));
                    }
                }
                out.push(String::new());
                out.push(format!("   {dm}Credits: {mo}{}¢", game.deck.credits));
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

fn file_def(game: &Game, area: usize, file: usize) -> &crate::data::BbsFileDef {
    let s = game.bbs_stock[area][file];
    &game.bbs().areas[area].files[s.def]
}

/// A stable fake file size from the filename.
pub fn file_size(name: &str) -> u32 {
    let h = name.bytes().fold(0x811c_9dc5u32, |h, b| {
        (h ^ u32::from(b)).wrapping_mul(0x0100_0193)
    });
    40_000 + h % 90_000
}

fn transfer_secs(game: &Game, area: usize, file: usize) -> f32 {
    file_size(&file_def(game, area, file).file) as f32 / BBS_CPS
}

fn drop_tag(item: &Drop, l: &Look) -> String {
    match item {
        Drop::Warez(_) => String::new(),
        Drop::Hint(_) => format!(" {}[TXT]", pipe(l.ok)),
        _ => format!(" {}[MOD]", pipe(l.aside)),
    }
}
