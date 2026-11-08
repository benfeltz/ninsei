//! System on AnarchyOS: a keyboard-first text-mode settings app, like a TUI in a tiling
//! window. Tabs across the top, the open panel under them, the keys at the bottom; About is
//! neofetch, Disk is a DEFRAG-style text block map. It draws a character grid in the desk's
//! colours and sends the same `WbOut`s as the other looks. Every key has a click.

use egui::{Color32, FontId, Ui};
use ninsei_core::disk::{self, Disk, Program};
use ninsei_core::glitch::Glitches;
use ninsei_core::hardware::Desktop;
use ninsei_core::system::{self, Panel};
use ninsei_core::{Data, WarezKind, hub::Save};

use crate::home::{WbOut, accent, backdrop, fill, ink, muted, shadow};
use crate::kit::Sheet;
use crate::system::SystemUi;

const SIZE: f32 = 13.0;
/// Rows the tabs and their rule take.
const TOP: usize = 2;
/// Blocks per map row (as on the other looks), each two characters wide.
const MAP_COLS: usize = 16;
/// Narrower than this, About drops the logo (like `neofetch --off`).
const LOGO_MIN_COLS: usize = 80;

fn font() -> FontId {
    FontId::monospace(SIZE)
}

/// What a key press or a click asks for.
#[derive(Clone, Debug, PartialEq)]
enum Cmd {
    Panel(Panel),
    NextPanel,
    PrevPanel,
    Up,
    Down,
    Left,
    Right,
    Enter,
    /// A click on a row: highlight it and press Enter.
    Row(usize),
    Pick(Disk),
    Scrap,
    Yes,
    No,
    Defrag,
}

/// A row with a cursor: what Enter, ← and → send.
#[derive(Default)]
struct Item {
    enter: Option<WbOut>,
    prev: Option<WbOut>,
    next: Option<WbOut>,
}

/// The face's character grid: text goes in cells, clicks come back as `Cmd`s.
type Grid = Sheet<Cmd>;

/// A key hint in the face's colours.
fn key(g: &mut Grid, col: usize, row: usize, k: &str, what: &str, cmd: Option<Cmd>) -> usize {
    g.key(col, row, k, what, cmd, (accent(), muted()))
}

/// What the text face needs: its grid is never narrower than 40 of its columns (it lays out
/// to at least that and would cut the rest); its height scrolls, so a few rows will do.
pub const NEED: crate::kit::fit::Need = crate::kit::fit::Need {
    grids: std::borrow::Cow::Borrowed(&[crate::kit::Fit { cols: 40, rows: 8 }]),
    cells: cells_in,
    scrolls: true,
};

/// The cells of this face's own font that fit in `rect`.
fn cells_in(ui: &Ui, rect: egui::Rect) -> (usize, usize) {
    let cw = ui.fonts_mut(|f| f.glyph_width(&font(), 'M'));
    let ch = ui.fonts_mut(|f| f.row_height(&font()));
    (
        (rect.width() / cw).floor() as usize,
        (rect.height() / ch).floor() as usize,
    )
}

pub fn show(ui: &mut Ui, data: &Data, save: &Save, st: &mut SystemUi, out: &mut Vec<WbOut>) {
    let cw = ui.fonts_mut(|f| f.glyph_width(&font(), 'M'));
    let cols = ((ui.available_width() / cw) as usize).max(40);
    let mut g = Grid::new(cols);

    // Disk works on what's installed and in the box; a pick that's gone is dropped.
    let on = disk::installed(save, data);
    let shelf = disk::shelf(save, data);
    if st
        .picked
        .as_ref()
        .is_some_and(|p| !on.iter().chain(&shelf).any(|x| &x.at == p))
    {
        st.picked = None;
    }

    tabs(&mut g, data, save, st);
    let mut items = Vec::new();
    let hints: Vec<(&str, &str, Option<Cmd>)> = match st.panel {
        Panel::Overview => {
            overview(&mut g, data, save, st, &mut items);
            vec![
                ("↑↓", "move", None),
                ("←→", "change", None),
                ("enter", "toggle", Some(Cmd::Enter)),
            ]
        }
        Panel::Monitor => {
            monitor(&mut g, data, save, st, &mut items);
            vec![("↑↓", "move", None), ("enter", "plug in", Some(Cmd::Enter))]
        }
        Panel::Theme => {
            theme(&mut g, data, save, st, &mut items);
            vec![("↑↓", "move", None), ("enter", "use", Some(Cmd::Enter))]
        }
        Panel::Disk => {
            let map = crate::system::defrag_map(ui, save, data, st, out);
            disk_panel(&mut g, data, save, st, &on, &shelf, &map);
            vec![
                ("↑↓", "pick", None),
                ("enter", "install/remove", Some(Cmd::Enter)),
                ("x", "scrap", Some(Cmd::Scrap)),
                ("d", "defrag", Some(Cmd::Defrag)),
            ]
        }
    };
    st.cursor = st.cursor.min(items.len().saturating_sub(1));

    // The keys along the bottom.
    let foot = g.rows() + 1;
    g.rule(foot, fill());
    // Wrapped onto another line when the tile's narrow.
    let (mut c, mut fy) = (1, foot + 1);
    let all = [
        ("1-4", "panel", None),
        ("[", "prev", Some(Cmd::PrevPanel)),
        ("]", "next", Some(Cmd::NextPanel)),
    ];
    for (k, what, cmd) in all.into_iter().chain(hints) {
        if c > 1 && c + k.chars().count() + what.chars().count() + 1 > g.cols {
            (c, fy) = (1, fy + 1);
        }
        c = key(&mut g, c, fy, k, what, cmd);
    }

    // Paint the grid and read clicks.
    let mut cmds = g.paint(ui, font(), None);
    keys(ui, st, &mut cmds);
    for cmd in cmds {
        apply(
            cmd,
            data,
            save,
            st,
            &items,
            &on,
            &shelf,
            out,
            ui.input(|i| i.time),
        );
    }
}

/// Keys, while the tile is focused (or, outside a tile, while the pointer's over it) and
/// nothing else is typing. No Ctrl or Alt: the browser owns those, and no Tab: it moves the
/// focus (in egui and in the page).
fn keys(ui: &Ui, st: &SystemUi, cmds: &mut Vec<Cmd>) {
    let live = st.keys || ui.rect_contains_pointer(ui.max_rect());
    if !live || st.blocked || ui.ctx().text_edit_focused() {
        return;
    }
    ui.input(|i| {
        use egui::Key;
        let k = |key| i.key_pressed(key);
        for (key, p) in [Key::Num1, Key::Num2, Key::Num3, Key::Num4]
            .into_iter()
            .zip(Panel::ALL)
        {
            if k(key) {
                cmds.push(Cmd::Panel(p));
            }
        }
        let map = [
            (Key::CloseBracket, Cmd::NextPanel),
            (Key::OpenBracket, Cmd::PrevPanel),
            (Key::ArrowUp, Cmd::Up),
            (Key::K, Cmd::Up),
            (Key::ArrowDown, Cmd::Down),
            (Key::J, Cmd::Down),
            (Key::ArrowLeft, Cmd::Left),
            (Key::ArrowRight, Cmd::Right),
            (Key::Enter, Cmd::Enter),
            (Key::Space, Cmd::Enter),
            (Key::X, Cmd::Scrap),
            (Key::Delete, Cmd::Scrap),
            (Key::Y, Cmd::Yes),
            (Key::N, Cmd::No),
            (Key::D, Cmd::Defrag),
        ];
        for (key, cmd) in map {
            if k(key) {
                cmds.push(cmd);
            }
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn apply(
    cmd: Cmd,
    data: &Data,
    save: &Save,
    st: &mut SystemUi,
    items: &[Item],
    on: &[Program],
    shelf: &[Program],
    out: &mut Vec<WbOut>,
    now: f64,
) {
    let goto = |st: &mut SystemUi, p: Panel| {
        st.panel = p;
        st.cursor = 0;
        st.confirm_scrap = None;
    };
    match cmd {
        Cmd::Panel(p) => return goto(st, p),
        Cmd::NextPanel => {
            let i = Panel::ALL.iter().position(|&p| p == st.panel).unwrap_or(0);
            return goto(st, Panel::ALL[(i + 1) % Panel::ALL.len()]);
        }
        Cmd::PrevPanel => {
            let i = Panel::ALL.iter().position(|&p| p == st.panel).unwrap_or(0);
            let n = Panel::ALL.len();
            return goto(st, Panel::ALL[(i + n - 1) % n]);
        }
        _ => {}
    }
    if st.panel == Panel::Disk {
        return disk_cmd(cmd, data, save, st, on, shelf, out, now);
    }
    let last = items.len().saturating_sub(1);
    let pick = |st: &SystemUi, f: fn(&Item) -> &Option<WbOut>| {
        items.get(st.cursor).and_then(|it| f(it).clone())
    };
    let send = match cmd {
        Cmd::Up => {
            st.cursor = st.cursor.saturating_sub(1);
            None
        }
        Cmd::Down => {
            st.cursor = (st.cursor + 1).min(last);
            None
        }
        Cmd::Row(i) => {
            st.cursor = i.min(last);
            pick(st, |it| &it.enter)
        }
        Cmd::Enter => pick(st, |it| &it.enter),
        Cmd::Left => pick(st, |it| &it.prev),
        Cmd::Right => pick(st, |it| &it.next),
        _ => None,
    };
    out.extend(send);
}

#[allow(clippy::too_many_arguments)]
fn disk_cmd(
    cmd: Cmd,
    data: &Data,
    save: &Save,
    st: &mut SystemUi,
    on: &[Program],
    shelf: &[Program],
    out: &mut Vec<WbOut>,
    now: f64,
) {
    let all: Vec<&Disk> = on.iter().chain(shelf).map(|p| &p.at).collect();
    let at = st
        .picked
        .as_ref()
        .and_then(|p| all.iter().position(|x| *x == p));
    if st.confirm_scrap.is_some() {
        match cmd {
            Cmd::Yes => {
                out.extend(st.confirm_scrap.take().map(WbOut::Scrap));
                st.picked = None;
            }
            Cmd::No | Cmd::Scrap => st.confirm_scrap = None,
            _ => {}
        }
        return;
    }
    if st.defrag.is_some() {
        return;
    }
    match cmd {
        Cmd::Down | Cmd::Up if !all.is_empty() => {
            let n = match (cmd, at) {
                (_, None) => 0,
                (Cmd::Down, Some(a)) => (a + 1).min(all.len() - 1),
                (_, Some(a)) => a.saturating_sub(1),
            };
            st.picked = Some(all[n].clone());
        }
        Cmd::Pick(d) => st.picked = Some(d),
        Cmd::Enter => {
            if let Some(p) = st.picked.clone() {
                if save.is_installed(&p) {
                    out.push(WbOut::Remove(p));
                } else if save.install_blocker(data, &p).is_none() {
                    out.push(WbOut::Install(p));
                }
            }
        }
        Cmd::Scrap => {
            if let Some(Disk::Stash(id)) = st.picked {
                st.confirm_scrap = Some(id);
            }
        }
        Cmd::Defrag => {
            st.defrag = Some((disk::defrag_steps(save, data), now));
            st.picked = None;
        }
        _ => {}
    }
}

/// The tab line: each panel with its number key, the open one inverted; the firmware right.
fn tabs(g: &mut Grid, data: &Data, save: &Save, st: &SystemUi) {
    let mut c = 1;
    for (i, p) in Panel::ALL.into_iter().enumerate() {
        let name = match p {
            Panel::Overview => "about",
            Panel::Monitor => "monitor",
            Panel::Theme => "theme",
            Panel::Disk => "disk",
        };
        let label = format!(" {} {name} ", i + 1);
        let start = c;
        c = if st.panel == p {
            g.inv(c, 0, &label, shadow(), accent())
        } else {
            g.put(c, 0, &label, ink())
        };
        g.hit(start, 0, c - start, Cmd::Panel(p));
        c += 1;
    }
    let fw = format!("system // {}", save.firmware_def(data).label.to_lowercase());
    let at = g.cols.saturating_sub(fw.chars().count() + 1).max(c + 2);
    g.put(at, 0, &fw, muted());
    g.rule(1, fill());
}

/// One row with a cursor: `▸` and the accent when highlighted. Clicking it is Enter.
fn row(g: &mut Grid, y: usize, i: usize, st: &SystemUi, label: &str) -> usize {
    let on = st.cursor == i;
    g.put(1, y, if on { "▸" } else { " " }, accent());
    let end = g.put(3, y, label, if on { accent() } else { ink() });
    g.hit(1, y, end.max(30) - 1, Cmd::Row(i));
    end
}

fn check(b: bool) -> &'static str {
    if b { "[x]" } else { "[ ]" }
}

fn radio(b: bool) -> &'static str {
    if b { "(•)" } else { "( )" }
}

/// The neighbour of `cur` in `list`, one step `by` (wrapping).
fn step<T: Clone + PartialEq>(list: &[T], cur: &T, by: isize) -> Option<T> {
    if list.len() < 2 {
        return None;
    }
    let i = list.iter().position(|x| x == cur).unwrap_or(0) as isize;
    let n = list.len() as isize;
    Some(list[((i + by).rem_euclid(n)) as usize].clone())
}

/// About: neofetch. The OS logo left, `runner@deck` and the specs right, the theme's colours
/// under them, then the switches.
fn overview(g: &mut Grid, data: &Data, save: &Save, st: &SystemUi, items: &mut Vec<Item>) {
    let o = system::overview(save, data);
    // Beside the specs when there's room, above them when not (fastfetch's top logo).
    let logo = system::logo(data, Desktop::Tiling);
    let beside = g.cols >= LOGO_MIN_COLS;
    for (r, l) in logo.iter().enumerate() {
        g.put(2, TOP + 1 + r, l, accent());
    }
    let (x, mut y) = if beside {
        (2 + system::LOGO_MAX.0 + 3, TOP + 1)
    } else {
        (1, TOP + 2 + logo.len())
    };
    let c = g.put(x, y, "runner", accent());
    let c = g.put(c, y, "@", ink());
    let end = g.put(c, y, &save.deck, accent());
    y += 1;
    g.put(x, y, &"─".repeat(end - x), muted());
    y += 1;
    let label_w = 11;
    for (k, v) in [("OS", &o.os), ("Deck", &o.deck), ("Firmware", &o.firmware)] {
        g.put(x, y, k, accent());
        g.put(x + label_w, y, v, ink());
        y += 1;
    }
    // The values line up, and the meters start a cell past the longest ("6 of 12 slots").
    let value_w = o
        .specs
        .iter()
        .map(|s| s.value.chars().count() + 1)
        .max()
        .unwrap_or(0)
        .max(8);
    for s in &o.specs {
        g.put(x, y, &s.label, accent());
        g.put(x + label_w, y, &s.value, ink());
        // The level as a meter: one cell per upgrade.
        let (lvl, max) = s.level;
        let mut c = x + label_w + value_w;
        for i in 0..max {
            c = g.put(c, y, "■", if i < lvl { accent() } else { fill() });
        }
        g.put(c + 2, y, &s.use_note, muted());
        y += 1;
    }
    // Software, wrapped like neofetch's Packages line.
    g.put(x, y, "Software", accent());
    if o.software.is_empty() {
        g.put(x + label_w, y, "none", muted());
        y += 1;
    } else {
        let mut c = x + label_w;
        for (i, (name, ver)) in o.software.iter().enumerate() {
            let s = format!(
                "{name} {ver}{}",
                if i + 1 < o.software.len() { "," } else { "" }
            );
            if c + s.chars().count() > g.cols && c > x + label_w {
                y += 1;
                c = x + label_w;
            }
            c = g.put(c, y, &s, ink()) + 1;
        }
        y += 1;
    }
    // The colour blocks: the theme's six roles, darkest to brightest.
    y += 1;
    if let Some(t) = data.theme(&save.phosphor) {
        let p = crate::theme::Palette::from_def(t);
        let mut c = x;
        for col in [p.bg, p.panel, p.faint, p.dim, p.fg, p.bright] {
            c = g.put(c, y, "███", col);
        }
    }
    y = y.max(TOP + 1 + logo.len()) + 2;

    // The switches.
    g.put(1, y, "settings", muted());
    y += 1;
    let add = |g: &mut Grid,
               items: &mut Vec<Item>,
               y: &mut usize,
               label: &str,
               value: &str,
               item: Item| {
        let i = items.len();
        let end = row(g, *y, i, st, &format!("{label:<12}"));
        g.put(end, *y, value, ink());
        items.push(item);
        *y += 1;
    };
    add(
        g,
        items,
        &mut y,
        "sound",
        check(save.sound),
        Item {
            enter: Some(WbOut::Sound),
            ..Item::default()
        },
    );
    add(
        g,
        items,
        &mut y,
        "cracktros",
        check(save.intros),
        Item {
            enter: Some(WbOut::Intros(!save.intros)),
            ..Item::default()
        },
    );
    let gl = Glitches::ALL.to_vec();
    let glitch_i = items.len();
    add(
        g,
        items,
        &mut y,
        "glitches",
        "",
        Item {
            enter: step(&gl, &save.glitches, 1).map(WbOut::Glitches),
            prev: step(&gl, &save.glitches, -1).map(WbOut::Glitches),
            next: step(&gl, &save.glitches, 1).map(WbOut::Glitches),
        },
    );
    // Every setting shown, the current one inverted, like a TUI's segmented switch.
    let mut c = 3 + 12;
    for gg in Glitches::ALL {
        let label = format!(" {} ", gg.label().to_lowercase());
        c = if save.glitches == gg {
            g.inv(
                c,
                y - 1,
                &label,
                shadow(),
                if st.cursor == glitch_i {
                    accent()
                } else {
                    ink()
                },
            )
        } else {
            g.put(c, y - 1, &label, muted())
        };
    }
    if save.decks.len() > 1 {
        let title = data
            .hardware
            .deck(&save.deck)
            .map_or(save.deck.clone(), |d| d.title());
        let pos = save.decks.iter().position(|d| *d == save.deck).unwrap_or(0);
        add(
            g,
            items,
            &mut y,
            "deck",
            &format!("‹ {title} › {}/{}", pos + 1, save.decks.len()),
            Item {
                enter: step(&save.decks, &save.deck, 1).map(WbOut::Deck),
                prev: step(&save.decks, &save.deck, -1).map(WbOut::Deck),
                next: step(&save.decks, &save.deck, 1).map(WbOut::Deck),
            },
        );
    }
    let deck = save.deck_def(data);
    if deck.firmware.len() > 1 {
        let ids: Vec<String> = deck.firmware.iter().map(|f| f.id.clone()).collect();
        let fw = save.firmware_def(data);
        add(
            g,
            items,
            &mut y,
            "firmware",
            &format!("‹ {} ›", fw.label),
            Item {
                enter: step(&ids, &fw.id, 1).map(WbOut::Firmware),
                prev: step(&ids, &fw.id, -1).map(WbOut::Firmware),
                next: step(&ids, &fw.id, 1).map(WbOut::Firmware),
            },
        );
    }
}

/// The monitors you own, the CRT switch, and what the highlighted one is.
fn monitor(g: &mut Grid, data: &Data, save: &Save, st: &SystemUi, items: &mut Vec<Item>) {
    let mut y = TOP + 1;
    g.put(1, y, "monitors", muted());
    y += 1;
    let owned: Vec<_> = save
        .monitors
        .iter()
        .filter_map(|id| data.hardware.monitor(id))
        .collect();
    for m in &owned {
        let i = items.len();
        let end = row(
            g,
            y,
            i,
            st,
            &format!("{} {:<34}", radio(save.monitor == m.id), m.title()),
        );
        g.put(
            end + 1,
            y,
            if m.colour { "colour" } else { "mono" },
            muted(),
        );
        items.push(Item {
            enter: Some(WbOut::Monitor(m.id.clone())),
            ..Item::default()
        });
        y += 1;
    }
    y += 1;
    let crt_i = items.len();
    row(g, y, crt_i, st, &format!("{} crt effects", check(save.crt)));
    items.push(Item {
        enter: Some(WbOut::Crt),
        ..Item::default()
    });
    y += 2;
    // What the highlighted monitor is (or the one plugged in, on the CRT row).
    let m = owned
        .get(st.cursor)
        .copied()
        .unwrap_or_else(|| save.monitor_def(data));
    y = wrap(g, 3, y, &m.blurb, muted());
    if let Some(t) = save.monitor_def(data).locked_theme(data) {
        let label = data.theme(t).map_or(t, |d| d.label.as_str());
        g.put(3, y + 1, &format!("mono tube: shows {label} only"), ink());
    }
}

/// Every theme with its six colours; a locked one says why when it's highlighted.
fn theme(g: &mut Grid, data: &Data, save: &Save, st: &SystemUi, items: &mut Vec<Item>) {
    let mut y = TOP + 1;
    g.put(1, y, "themes", muted());
    y += 1;
    let mut why = None;
    for t in &data.themes {
        let i = items.len();
        let blocker = save.theme_blocker(data, &t.id);
        let label = format!("{} {:<20}", radio(save.phosphor == t.id), t.label);
        let end = if blocker.is_some() {
            // Locked: no cursor colour, just muted.
            g.put(1, y, if st.cursor == i { "▸" } else { " " }, accent());
            let e = g.put(3, y, &label, muted());
            g.hit(1, y, e - 1, Cmd::Row(i));
            e
        } else {
            row(g, y, i, st, &label)
        };
        let p = crate::theme::Palette::from_def(t);
        let mut c = end + 1;
        for col in [p.bg, p.panel, p.faint, p.dim, p.fg, p.bright] {
            c = g.put(c, y, "██", col);
        }
        if st.cursor == i {
            why = blocker.clone();
        }
        items.push(Item {
            enter: blocker.is_none().then(|| WbOut::Theme(t.id.clone())),
            ..Item::default()
        });
        y += 1;
    }
    if let Some(why) = why {
        wrap(g, 3, y + 1, &why, ink());
    }
}

/// Writes `s` word-wrapped from (col, y); returns the row after it.
fn wrap(g: &mut Grid, col: usize, mut y: usize, s: &str, fg: Color32) -> usize {
    let w = g.cols.saturating_sub(col + 1).max(20);
    let mut line = String::new();
    for word in s.split_whitespace() {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > w {
            g.put(col, y, &line, fg);
            y += 1;
            line.clear();
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        g.put(col, y, &line, fg);
        y += 1;
    }
    y
}

/// A kind of program's block character and colour.
fn glyph(kind: WarezKind) -> (char, Color32) {
    match kind {
        WarezKind::Breaker => ('▓', accent()),
        WarezKind::Shield => ('▒', ink()),
        WarezKind::Utility => ('░', ink()),
    }
}

/// Disk: DEFRAG in text. The map (two characters a block, the first block of each program
/// carrying its initial), the legend, the RAM strip, then the disk in load order, the box
/// beside it and the picked program's details.
#[allow(clippy::too_many_arguments)]
fn disk_panel(
    g: &mut Grid,
    data: &Data,
    save: &Save,
    st: &SystemUi,
    on: &[Program],
    shelf: &[Program],
    map: &[Option<usize>],
) {
    let cap = save.hdd_cap(data) as usize;
    let mut y = TOP + 1;
    let c = g.put(1, y, "hdd", accent());
    if st.defrag.is_some() {
        g.inv(c + 2, y, " defragmenting… ", shadow(), accent());
        y += 1;
    }
    g.put(
        c + 2,
        y,
        &format!(
            "{} of {cap} slots · {}% fragmented · {} optimizer fragments",
            on.len(),
            disk::fragmentation(save, data),
            save.fragments
        ),
        ink(),
    );
    y += 1;

    // The map, framed.
    let w = MAP_COLS * 2;
    g.put(1, y, &format!("┌{}┐", "─".repeat(w)), muted());
    let rows = map.len().div_ceil(MAP_COLS);
    let picked_on = st
        .picked
        .as_ref()
        .and_then(|pk| on.iter().position(|x| &x.at == pk));
    let used = map.iter().filter(|b| b.is_some()).count();
    let preview = st
        .picked
        .as_ref()
        .and_then(|pk| shelf.iter().find(|x| &x.at == pk))
        .filter(|_| on.len() < cap)
        .map_or(0..0, |p| used..used + p.blocks());
    let mut first = vec![true; on.len()];
    for r in 0..rows {
        let yy = y + 1 + r;
        g.put(1, yy, "│", muted());
        g.put(2 + w, yy, "│", muted());
        for k in 0..MAP_COLS {
            let i = r * MAP_COLS + k;
            let col = 2 + k * 2;
            match map.get(i).copied().flatten() {
                Some(n) => {
                    let prog = &on[n];
                    let (ch, fg) = glyph(prog.kind);
                    let head = if first[n] {
                        first[n] = false;
                        prog.spec.name.chars().next().unwrap_or('?')
                    } else {
                        ch
                    };
                    let s: String = [head, ch].iter().collect();
                    if picked_on == Some(n) {
                        g.inv(col, yy, &s, backdrop(), accent());
                    } else if head != ch {
                        g.inv(col, yy, &head.to_string(), backdrop(), fg);
                        g.put(col + 1, yy, &ch.to_string(), fg);
                    } else {
                        g.put(col, yy, &s, fg);
                    }
                    g.hit_as(col, yy, 2, Cmd::Pick(prog.at.clone()), prog.spec.file());
                }
                None if preview.contains(&i) => {
                    g.put(col, yy, "::", accent());
                }
                None if i < map.len() => {
                    g.put(col, yy, "· ", fill());
                }
                None => {}
            }
        }
    }
    g.put(1, y + 1 + rows, &format!("└{}┘", "─".repeat(w)), muted());

    // Beside the map: the legend and the RAM strip.
    let x = 5 + w;
    let mut ly = y + 1;
    for (kind, name) in [
        (WarezKind::Breaker, "breaker"),
        (WarezKind::Shield, "shield"),
        (WarezKind::Utility, "utility"),
    ] {
        let (ch, fg) = glyph(kind);
        g.put(x, ly, &format!("{ch}{ch}"), fg);
        g.put(x + 3, ly, name, muted());
        ly += 1;
    }
    g.put(x, ly, "· ", fill());
    g.put(x + 3, ly, "free", muted());
    y += rows + 3;

    // The disk, the box, and the pick.
    // The pick's details right of the lists, or under them in a narrow tile.
    let (dx, bx, ix) = (1, 22, 44);
    let below = g.cols < ix + 34;
    g.put(dx, y, "disk", muted());
    g.put(
        bx,
        y,
        &format!("box {} of {}", save.stash.len(), save.stash_cap(&data.rig)),
        muted(),
    );
    y += 1;
    let list = |g: &mut Grid, x: usize, progs: &[Program]| {
        for (k, prog) in progs.iter().enumerate() {
            let picked = st.picked.as_ref() == Some(&prog.at);
            let free = matches!(prog.at, Disk::Free(_));
            let name = format!("{}{}", prog.spec.file(), if free { "*" } else { "" });
            g.put(x, y + k, if picked { "▸" } else { " " }, accent());
            let (ch, fg) = glyph(prog.kind);
            g.put(x + 2, y + k, &ch.to_string(), fg);
            let end = if picked {
                g.inv(x + 4, y + k, &name, shadow(), accent())
            } else {
                g.put(x + 4, y + k, &name, ink())
            };
            g.hit_as(x, y + k, end - x, Cmd::Pick(prog.at.clone()), name);
        }
        if progs.is_empty() {
            g.put(x + 2, y, "empty", fill());
        }
    };
    list(g, dx, on);
    list(g, bx, shelf);
    let listed = on.len().max(shelf.len()).max(1);
    if shelf.iter().any(|p| matches!(p.at, Disk::Free(_))) {
        g.put(bx + 2, y + listed, "* free", fill());
    }
    if below {
        info(g, data, save, st, on, shelf, 3, y + listed + 2);
    } else {
        info(g, data, save, st, on, shelf, ix, y);
    }
    g.grow_to(y + listed + 1);
}

/// The picked program, from its binary, and what Enter would do with it.
#[allow(clippy::too_many_arguments)]
fn info(
    g: &mut Grid,
    data: &Data,
    save: &Save,
    st: &SystemUi,
    on: &[Program],
    shelf: &[Program],
    x: usize,
    mut y: usize,
) {
    let Some(prog) = st
        .picked
        .as_ref()
        .and_then(|pk| on.iter().chain(shelf).find(|x| &x.at == pk))
    else {
        g.put(x, y, "↑↓ to pick a program", fill());
        return;
    };
    let s = &prog.spec;
    g.put(x, y, &s.file(), accent());
    y += 1;
    let w = g.cols.saturating_sub(x + 1).max(20);
    let mut sub = Grid::new(w);
    let n = wrap(&mut sub, 0, 0, &data.warez[prog.warez].blurb, muted());
    let n = wrap(&mut sub, 0, n, &s.effect.describe(), ink());
    g.stamp(sub, x, y);
    y += n;
    let mut line = format!("{:?} · cpu {}", prog.kind, s.cpu).to_lowercase();
    if let Some(pw) = prog.power() {
        line.push_str(&format!(" · power {pw}"));
    }
    if s.opt > 0 {
        line.push_str(&format!(" · opt {}", s.opt));
    }
    g.put(x, y, &line, muted());
    y += 2;
    if let Some(id) = st.confirm_scrap {
        let c = g.put(x, y, "scrap it for good?", ink());
        let c = key(g, c + 2, y, "y", "yes", Some(Cmd::Yes));
        key(g, c, y, "n", "no", Some(Cmd::No));
        let _ = id;
        return;
    }
    let c = if save.is_installed(&prog.at) {
        key(g, x, y, "enter", "remove", Some(Cmd::Enter))
    } else {
        match save.install_blocker(data, &prog.at) {
            None => key(g, x, y, "enter", "install", Some(Cmd::Enter)),
            Some(why) => {
                wrap(g, x, y, &why, ink());
                return;
            }
        }
    };
    if matches!(prog.at, Disk::Stash(_)) {
        key(g, c, y, "x", "scrap", Some(Cmd::Scrap));
    }
}
