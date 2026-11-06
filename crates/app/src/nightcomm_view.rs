//! NiGHTCOMM, drawn as a btop screen on a cell grid: one rounded box per thing, titles cut
//! into the borders (`┐¹iNTEL┌`), key hints in the bottom border instead of the body, and
//! heat as a gradient meter. What it shows comes from core's `nightcomm` module; this only
//! draws and turns keys and clicks into a dial or a scan.

use egui::{Event, Id, Key, Sense, Ui};
use ninsei_core::Data;
use ninsei_core::hub::{Answer, Found, RunEnd, Save, ScanResult};
use ninsei_core::nightcomm::{self, Block, Sort, Tab};
use ninsei_core::tuning::{HEAT_COOL, HEAT_MAX};

use crate::cells::Cells;
use crate::home::{WbOut, accent, fill, ink, muted, shadow};
use crate::hubtools::SCAN_FILL_SECS;

/// NiGHTCOMM's state between frames: the page, the order and filter, and what's picked.
#[derive(Default)]
pub struct NightComm {
    pub tab: Tab,
    pub sort: Sort,
    pub query: String,
    /// The phonebook entry picked on iNTEL.
    pub sel: Option<u32>,
    /// The range picked on WARDiAL and RANGES.
    pub range: Option<String>,
    drawn: u64,
    want_focus: bool,
    want_filter: bool,
}

/// Fewer columns than this and the pages drop their wide columns and side boxes.
const NARROW: usize = 76;
/// Rows the iNTEL page gives the box about the picked number.
const DETAIL_ROWS: usize = 7;
/// Cells in a block on the wardial map (two digits and a gap).
const BLOCK_W: usize = 3;
/// Blocks per row on the wardial map, and numbers per row in a scan.
const PER_ROW: usize = 10;

fn sup(n: usize) -> char {
    ['⁰', '¹', '²', '³', '⁴', '⁵', '⁶', '⁷', '⁸', '⁹'][n % 10]
}

/// A btop gradient, darkest to brightest. It starts above the empty cells' `faint`, so a lit
/// cell never disappears on a mono tube.
fn ramp() -> [egui::Color32; 3] {
    [muted(), ink(), accent()]
}

/// What the keys asked for this frame.
#[derive(Default)]
struct Keys {
    up: bool,
    down: bool,
    enter: bool,
    space: bool,
    text: Vec<char>,
}

/// Draws NiGHTCOMM in the space `ui` offers.
pub fn show(
    ui: &mut Ui,
    data: &Data,
    save: &Save,
    nc: &mut NightComm,
    scan: Option<&(ScanResult, f32)>,
    dial: &mut Option<u32>,
    out: &mut Vec<WbOut>,
) {
    let size = ui.available_size();
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let keys_id = resp.id;
    let filter_id = Id::new("nightcomm-filter");

    // Not drawn last pass: the window just opened (or a splash ended). It takes the keys.
    let pass = ui.ctx().cumulative_pass_nr();
    if nc.drawn + 1 < pass {
        nc.want_focus = true;
    }
    nc.drawn = pass;
    if resp.clicked() {
        nc.want_focus = true;
    }
    if ui.is_enabled() {
        if nc.want_filter {
            ui.memory_mut(|m| m.request_focus(filter_id));
            nc.want_filter = false;
            nc.want_focus = false;
        } else if nc.want_focus {
            ui.memory_mut(|m| m.request_focus(keys_id));
            nc.want_focus = false;
        }
    }
    let keys = if ui.memory(|m| m.has_focus(keys_id)) {
        // Arrows and Enter stay ours instead of moving egui's focus.
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(
                keys_id,
                egui::EventFilter {
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    ..Default::default()
                },
            );
        });
        ui.input(|i| {
            let mut k = Keys::default();
            for e in &i.events {
                match e {
                    Event::Key {
                        key, pressed: true, ..
                    } => match key {
                        Key::ArrowUp => k.up = true,
                        Key::ArrowDown => k.down = true,
                        Key::Enter => k.enter = true,
                        _ => {}
                    },
                    Event::Text(t) => k.text.extend(t.chars()),
                    _ => {}
                }
            }
            k.space = k.text.contains(&' ');
            k
        })
    } else {
        Keys::default()
    };
    for c in &keys.text {
        if let Some(t) = Tab::from_key(*c) {
            nc.tab = t;
        }
    }

    let g = Cells::fit(ui, rect, muted(), shadow(), muted());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 0.0, shadow());
    if g.cols < 24 || g.rows < 8 {
        return;
    }
    // The program's box: page titles in the top border, the key legend in the bottom one.
    g.frame(&p, 0, 0, g.cols, g.rows);
    let mut c = 2;
    for (i, t) in Tab::ALL.iter().enumerate() {
        let on = *t == nc.tab;
        let label = format!("{}{}", sup(i + 1), t.title());
        let col = if on { accent() } else { muted() };
        let used = g.title(&p, c, 0, &label, col);
        let hit = ui.interact(g.at(c, 0, used, 1), Id::new(("nc-tab", i)), Sense::click());
        if hit.clicked() {
            nc.tab = *t;
            nc.want_focus = true;
        }
        c += used + 1;
    }
    let inner = (1, 1, g.cols - 2, g.rows - 2);
    let ctx = Ctx { ui, g: &g, p: &p };
    let mut legend: Vec<String> = Vec::new();
    match nc.tab {
        Tab::Intel => intel(&ctx, save, nc, &keys, inner, dial, &mut legend),
        Tab::Wardial => wardial(&ctx, data, save, nc, &keys, scan, inner, out, &mut legend),
        Tab::Ranges => ranges(&ctx, data, save, nc, &keys, inner, &mut legend),
    }
    // The legend: btop's options cut into the bottom border.
    let mut c = 2;
    for (i, l) in legend.iter().enumerate() {
        let is_filter = nc.tab == Tab::Intel && i == 1;
        let shown = if is_filter && ui.memory(|m| m.has_focus(filter_id)) {
            // Room for the filter box over the title.
            format!("/ {:<14}", "")
        } else {
            l.clone()
        };
        let used = g.title(&p, c, g.rows - 1, &shown, ink());
        if used == 0 {
            break;
        }
        if is_filter {
            let r = g.at(c + 3, g.rows - 1, used.saturating_sub(4), 1);
            if ui.memory(|m| m.has_focus(filter_id)) {
                let edit = ui.put(
                    r,
                    egui::TextEdit::singleline(&mut nc.query)
                        .id(filter_id)
                        .frame(egui::Frame::NONE)
                        .font(egui::TextStyle::Monospace)
                        .text_color(accent()),
                );
                if edit.lost_focus() {
                    nc.want_focus = true;
                }
            } else if ui
                .interact(
                    g.at(c, g.rows - 1, used, 1),
                    Id::new("nc-filter-hit"),
                    Sense::click(),
                )
                .clicked()
            {
                nc.want_filter = true;
            }
        }
        c += used + 1;
    }
    if keys.text.contains(&'/') && nc.tab == Tab::Intel {
        nc.want_filter = true;
    }
    if nc.want_focus || nc.want_filter {
        ui.ctx().request_repaint();
    }
}

/// What a page draws with.
struct Ctx<'a> {
    ui: &'a Ui,
    g: &'a Cells,
    p: &'a egui::Painter,
}

impl Ctx<'_> {
    fn click(
        &self,
        c: usize,
        r: usize,
        w: usize,
        id: impl std::hash::Hash + std::fmt::Debug,
    ) -> egui::Response {
        self.ui
            .interact(self.g.at(c, r, w, 1), Id::new(id), Sense::click())
    }

    /// An inverse-video row: the period's selection.
    fn picked(&self, c: usize, r: usize, w: usize) {
        self.g.fill(self.p, c, r, w, 1, accent());
    }

    /// A sub-box with its title cut into the top border.
    fn boxed(&self, c: usize, r: usize, w: usize, h: usize, title: &str) {
        self.g.frame(self.p, c, r, w, h);
        if !title.is_empty() {
            self.g.title(self.p, c + 2, r, title, ink());
        }
    }
}

/// Steps a selection through `ids` by the arrow keys; an unknown pick lands on the first.
fn step<T: PartialEq + Clone>(ids: &[T], cur: &mut Option<T>, keys: &Keys) {
    let at = cur.as_ref().and_then(|c| ids.iter().position(|i| i == c));
    let Some(first) = ids.first() else {
        *cur = None;
        return;
    };
    let mut i = match at {
        Some(i) => i,
        None => {
            *cur = Some(first.clone());
            0
        }
    };
    if keys.up {
        i = i.saturating_sub(1);
    }
    if keys.down {
        i = (i + 1).min(ids.len() - 1);
    }
    *cur = Some(ids[i].clone());
}

/// The first row to show so `at` stays in a window of `rows`.
fn scroll_to(at: usize, rows: usize) -> usize {
    (at + 1).saturating_sub(rows.max(1))
}

fn result_glyph(e: RunEnd) -> char {
    match e {
        RunEnd::Exit => 'E',
        RunEnd::Flatline => 'F',
    }
}

#[allow(clippy::too_many_arguments)]
fn intel(
    cx: &Ctx,
    save: &Save,
    nc: &mut NightComm,
    keys: &Keys,
    (c0, r0, w, h): (usize, usize, usize, usize),
    dial: &mut Option<u32>,
    legend: &mut Vec<String>,
) {
    let (g, p) = (cx.g, cx.p);
    for c in &keys.text {
        if *c == 's' {
            nc.sort = nc.sort.next();
        }
    }
    let rows = nightcomm::intel(save, &nc.query, nc.sort);
    let ids: Vec<u32> = rows.iter().map(|e| e.id).collect();
    step(&ids, &mut nc.sel, keys);
    if keys.enter || keys.text.contains(&'d') {
        *dial = nc.sel;
    }
    legend.push(format!("s sort: {}", nc.sort.label()));
    legend.push(if nc.query.is_empty() {
        "/ filter".to_string()
    } else {
        format!("/ {}", nc.query)
    });
    legend.push("d dial".to_string());

    let narrow = w < NARROW;
    let detail = h >= DETAIL_ROWS + 5;
    let list_h = if detail { h - DETAIL_ROWS } else { h };
    // Columns, sized from their widest real value.
    let heat_w = HEAT_MAX as usize + 5;
    let listing_w = if narrow { 0 } else { 14 };
    // Everything but the name: margins, number, listing, heat, runs and the last results.
    let others = 1
        + 10
        + 2
        + if narrow { 0 } else { listing_w + 2 }
        + heat_w
        + 2
        + 6
        + ninsei_core::tuning::RESULTS_KEPT
        + 1;
    let name_w = w.saturating_sub(others);
    let x_num = c0 + 1;
    let x_name = x_num + 10;
    let x_list = x_name + name_w + 2;
    let x_heat = if narrow {
        x_name + name_w + 2
    } else {
        x_list + listing_w + 2
    };
    let x_runs = x_heat + heat_w + 2;
    let x_last = x_runs + 6;

    g.text(p, x_num, r0, "NUMBER", muted());
    g.text(p, x_name, r0, "NAME", muted());
    if !narrow {
        g.text(p, x_list, r0, "LISTED AS", muted());
    }
    let heat_head = if nc.sort == Sort::Heat {
        "HEAT▼"
    } else {
        "HEAT"
    };
    g.text(p, x_heat, r0, heat_head, muted());
    g.text_right(
        p,
        x_runs + 4,
        r0,
        if nc.sort == Sort::Runs {
            "RUNS▼"
        } else {
            "RUNS"
        },
        muted(),
    );
    g.text(p, x_last, r0, "LAST", muted());
    let visible = list_h.saturating_sub(1);
    if rows.is_empty() {
        let msg = if save.phonebook.is_empty() {
            "the phonebook is empty."
        } else {
            "nothing matches."
        };
        g.text(p, x_num, r0 + 1, msg, muted());
    }
    let at = nc
        .sel
        .and_then(|s| ids.iter().position(|i| *i == s))
        .unwrap_or(0);
    for (k, e) in rows
        .iter()
        .enumerate()
        .skip(scroll_to(at, visible))
        .take(visible)
    {
        let r = r0 + 1 + (k - scroll_to(at, visible));
        let on = nc.sel == Some(e.id);
        if on {
            cx.picked(c0, r, w);
        }
        let (fg, dim) = if on {
            (shadow(), shadow())
        } else {
            (ink(), muted())
        };
        g.text(p, x_num, r, &e.number, fg);
        g.text_max(p, x_name, r, &e.name, name_w, fg);
        if !narrow {
            g.text_max(p, x_list, r, &e.business, listing_w, dim);
        }
        if on {
            g.text(
                p,
                x_heat,
                r,
                &format!("{:<w$}", "■".repeat(e.heat as usize), w = HEAT_MAX as usize),
                fg,
            );
        } else {
            g.meter(p, x_heat, r, HEAT_MAX as usize, e.heat as usize, &ramp());
        }
        let word = nightcomm::heat_word(e.heat);
        let word_col = if on {
            fg
        } else if word == "HOT" {
            accent()
        } else {
            dim
        };
        g.text(p, x_heat + HEAT_MAX as usize + 1, r, word, word_col);
        g.text_right(p, x_runs + 4, r, &e.runs.to_string(), fg);
        let last: String = e.last.iter().map(|x| result_glyph(*x)).collect();
        g.text(p, x_last, r, if last.is_empty() { "-" } else { &last }, dim);
        let hit = cx.click(c0, r, w, ("nc-row", e.id));
        if hit.clicked() {
            nc.sel = Some(e.id);
        }
        if hit.double_clicked() {
            *dial = Some(e.id);
        }
    }
    if !detail {
        return;
    }
    // The picked number, and the watched lines beside it.
    let top = r0 + list_h;
    let side = if narrow { 0 } else { w / 3 };
    let dw = w - side;
    let Some(e) = nc.sel.and_then(|id| save.entry(id)) else {
        cx.boxed(c0, top, dw, DETAIL_ROWS, "");
        return;
    };
    cx.boxed(
        c0,
        top,
        dw,
        DETAIL_ROWS,
        &format!("{} {}", e.number, e.name),
    );
    let x = c0 + 2;
    let v = x + 8;
    g.text(p, x, top + 1, "heat", muted());
    g.meter(p, v, top + 1, HEAT_MAX as usize, e.heat as usize, &ramp());
    g.text(
        p,
        v + HEAT_MAX as usize + 1,
        top + 1,
        &format!("{} of {HEAT_MAX}  {}", e.heat, nightcomm::heat_word(e.heat)),
        ink(),
    );
    g.text(p, x, top + 2, "cools", muted());
    g.text(
        p,
        v,
        top + 2,
        &format!("-{HEAT_COOL} for every run on another number"),
        ink(),
    );
    let exits = e.last.iter().filter(|x| **x == RunEnd::Exit).count();
    let flats = e.last.len() - exits;
    g.text(p, x, top + 3, "runs", muted());
    g.text(
        p,
        v,
        top + 3,
        &match e.runs {
            0 => "never run".to_string(),
            n => format!(
                "{n}  ·  last {}: {exits} out, {flats} flatlined",
                e.last.len()
            ),
        },
        ink(),
    );
    g.text(p, x, top + 4, "listed", muted());
    g.text_max(p, v, top + 4, &e.business, dw.saturating_sub(12), ink());
    if side > 0 {
        let sx = c0 + dw;
        cx.boxed(sx, top, side, DETAIL_ROWS, "watched lines");
        let lines = nightcomm::watched(save);
        if lines.is_empty() {
            g.text(p, sx + 2, top + 1, "none found", muted());
        }
        for (i, f) in lines.iter().rev().take(DETAIL_ROWS - 2).enumerate() {
            g.text(p, sx + 2, top + 1 + i, &f.number, accent());
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn wardial(
    cx: &Ctx,
    data: &Data,
    save: &Save,
    nc: &mut NightComm,
    keys: &Keys,
    scan: Option<&(ScanResult, f32)>,
    (c0, r0, w, h): (usize, usize, usize, usize),
    out: &mut Vec<WbOut>,
    legend: &mut Vec<String>,
) {
    let (g, p) = (cx.g, cx.p);
    let known = nightcomm::ranges(save, data);
    let ids: Vec<String> = known.iter().map(|r| r.def.id.clone()).collect();
    step(&ids, &mut nc.range, keys);
    let Some(row) = known.iter().find(|r| Some(&r.def.id) == nc.range.as_ref()) else {
        return;
    };
    let def = row.def;
    let open = row.blocks - row.scanned.min(row.blocks);
    let can = open > 0 && save.bank >= def.cost;
    if (keys.space || keys.enter) && can {
        out.push(WbOut::Scan(def.id.clone()));
    }
    legend.push(if open == 0 {
        "tapped out".to_string()
    } else if save.bank < def.cost {
        format!("scan needs {}¢", def.cost)
    } else {
        format!("space scan {}¢", def.cost)
    });
    legend.push(format!("bank {}¢", save.bank));
    if save.telco_heat > 0 {
        legend.push(format!("telco heat {}", save.telco_heat));
    }

    let narrow = w < NARROW;
    // Ranges down the left.
    let lw = if narrow { w } else { 24.min(w / 3) };
    let lh = if narrow {
        (known.len() + 2).min(h / 3).max(3)
    } else {
        h
    };
    cx.boxed(c0, r0, lw, lh, "ranges");
    for (i, r) in known.iter().take(lh - 2).enumerate() {
        let y = r0 + 1 + i;
        let on = Some(&r.def.id) == nc.range.as_ref();
        if on {
            cx.picked(c0 + 1, y, lw - 2);
        }
        let fg = if on { shadow() } else { ink() };
        let indent = if r.depth > 0 { "└" } else { "" };
        g.text(p, c0 + 2, y, &format!("{indent}{}", r.def.pattern()), fg);
        g.text_right(
            p,
            c0 + lw - 2,
            y,
            &format!("{}/{}", r.scanned, r.blocks),
            if on { shadow() } else { muted() },
        );
        if cx.click(c0 + 1, y, lw - 2, ("nc-wd-range", i)).clicked() {
            nc.range = Some(r.def.id.clone());
        }
    }
    let (mx, my, mw, mh) = if narrow {
        (c0, r0 + lh, w, h - lh)
    } else {
        (c0 + lw, r0, w - lw, h)
    };
    // The block map beside the latest scan, when there's room for both.
    let map_w = PER_ROW * BLOCK_W + 3;
    let show_map = !narrow && mw >= map_w + 30;
    if show_map {
        let blocks = nightcomm::block_map(save, def);
        cx.boxed(mx, my, map_w, mh, &format!("blocks {}", def.pattern()));
        let rows = blocks.len().div_ceil(PER_ROW).min(mh.saturating_sub(4));
        for (b, state) in blocks.iter().enumerate().take(rows * PER_ROW) {
            let x = mx + 2 + (b % PER_ROW) * BLOCK_W;
            let y = my + 1 + b / PER_ROW;
            let label = format!("{:02}", b % 100);
            // Shape, not just shade, tells the states apart on a mono tube.
            match state {
                Block::Open => {
                    g.text(p, x, y, "··", muted());
                }
                Block::Dry => {
                    g.text(p, x, y, &label, muted());
                }
                Block::Watched => {
                    g.text(p, x, y, &label, accent());
                    g.text(p, x + 2, y, "!", accent());
                }
                Block::Hits { watched, .. } => {
                    g.fill(p, x, y, 2, 1, ink());
                    g.text(p, x, y, &label, shadow());
                    if *watched {
                        g.text(p, x + 2, y, "!", accent());
                    }
                }
            }
        }
        let y = my + mh - 3;
        g.text(
            p,
            mx + 2,
            y,
            &format!("{} of {} dialed", row.scanned, row.blocks),
            muted(),
        );
        g.text(
            p,
            mx + 2,
            y + 1,
            &format!("{} found  {} watched", row.hits, row.watched),
            muted(),
        );
    }
    let (sx, sw) = if show_map {
        (mx + map_w, mw - map_w)
    } else {
        (mx, mw)
    };
    let this = scan.filter(|(s, _)| s.range == def.id);
    let title = this.map_or("scan".to_string(), |(s, _)| {
        format!("scan block {:02}", s.block)
    });
    cx.boxed(sx, my, sw, mh, &title);
    let Some((res, t)) = this else {
        // No scan this session: what earlier scans of this range turned up.
        let finds: Vec<_> = save.finds.iter().filter(|f| f.range == def.id).collect();
        if finds.is_empty() {
            g.text(p, sx + 2, my + 1, "nothing dialed here yet.", muted());
        }
        for (i, f) in finds.iter().rev().take(mh.saturating_sub(2)).enumerate() {
            find_line(cx, data, save, sx + 2, my + 1 + i, sw - 4, f);
        }
        return;
    };
    // The scan's numbers fill in as the modem works through them.
    let shown = ((t / SCAN_FILL_SECS) * res.cells.len() as f32).ceil() as usize;
    if *t < SCAN_FILL_SECS {
        cx.ui.ctx().request_repaint();
    }
    let grid_rows = res.cells.len().div_ceil(PER_ROW).min(mh.saturating_sub(5));
    for (k, (_, a)) in res.cells.iter().enumerate().take(grid_rows * PER_ROW) {
        let x = sx + 2 + (k % PER_ROW) * 2;
        let y = my + 1 + k / PER_ROW;
        if k >= shown {
            g.text(p, x, y, "·", fill());
            continue;
        }
        let col = match a {
            Answer::Carrier { .. } | Answer::Board { .. } | Answer::Trap => accent(),
            _ => muted(),
        };
        let glyph = if matches!(a, Answer::NoAnswer) {
            '·'
        } else {
            a.glyph()
        };
        g.text(p, x, y, &glyph.to_string(), col);
    }
    // ToneLoc's counters.
    let done = &res.cells[..shown.min(res.cells.len())];
    let count = |f: &dyn Fn(&Answer) -> bool| done.iter().filter(|(_, a)| f(a)).count();
    let cx0 = sx + 2 + PER_ROW * 2 + 2;
    let stats = [
        ("carrier", count(&|a| matches!(a, Answer::Carrier { .. }))),
        ("board", count(&|a| matches!(a, Answer::Board { .. }))),
        ("watched", count(&|a| matches!(a, Answer::Trap))),
        ("voice", count(&|a| matches!(a, Answer::Voice))),
        ("busy", count(&|a| matches!(a, Answer::Busy))),
        ("fax", count(&|a| matches!(a, Answer::Fax))),
        ("no answer", count(&|a| matches!(a, Answer::NoAnswer))),
    ];
    if sx + sw > cx0 + 14 {
        for (i, (label, n)) in stats.iter().enumerate().take(mh.saturating_sub(2)) {
            let hot = i < 3 && *n > 0;
            g.text(p, cx0, my + 1 + i, label, muted());
            g.text_right(
                p,
                cx0 + 13,
                my + 1 + i,
                &n.to_string(),
                if hot { accent() } else { ink() },
            );
        }
    }
    // What this scan found so far, under the grid.
    let mut y = my + 1 + grid_rows + 1;
    for (number, a) in done.iter() {
        if y >= my + mh - 1 {
            break;
        }
        let what = match a {
            Answer::Carrier { .. } => Found::Network(0),
            Answer::Board { board, .. } => Found::Board(data.boards[*board].id.clone()),
            Answer::Trap => Found::Watched,
            _ => continue,
        };
        let f = save
            .finds
            .iter()
            .rev()
            .find(|f| &f.number == number)
            .cloned()
            .unwrap_or(ninsei_core::hub::Find {
                range: res.range.clone(),
                block: res.block,
                number: number.clone(),
                what,
            });
        find_line(cx, data, save, sx + 2, y, sw - 4, &f);
        y += 1;
    }
}

/// One find: the number, what answered, and its name where there is one.
fn find_line(
    cx: &Ctx,
    data: &Data,
    save: &Save,
    x: usize,
    y: usize,
    w: usize,
    f: &ninsei_core::hub::Find,
) {
    let (kind, name) = match &f.what {
        Found::Network(id) => (
            "network",
            save.entry(*id).map(|e| e.name.clone()).unwrap_or_default(),
        ),
        Found::Board(id) => (
            "board",
            data.board(id)
                .map(|b| data.boards[b].name.clone())
                .unwrap_or_default(),
        ),
        Found::Watched => ("watched", String::new()),
    };
    let col = if f.what == Found::Watched {
        accent()
    } else {
        ink()
    };
    cx.g.text(cx.p, x, y, &f.number, col);
    cx.g.text(cx.p, x + 10, y, kind, muted());
    cx.g.text_max(cx.p, x + 19, y, &name, w.saturating_sub(19), ink());
}

#[allow(clippy::too_many_arguments)]
fn ranges(
    cx: &Ctx,
    data: &Data,
    save: &Save,
    nc: &mut NightComm,
    keys: &Keys,
    (c0, r0, w, h): (usize, usize, usize, usize),
    legend: &mut Vec<String>,
) {
    let (g, p) = (cx.g, cx.p);
    let rows = nightcomm::ranges(save, data);
    let ids: Vec<String> = rows.iter().map(|r| r.def.id.clone()).collect();
    step(&ids, &mut nc.range, keys);
    if keys.enter {
        nc.tab = Tab::Wardial;
    }
    legend.push("↑↓ range".to_string());
    legend.push("enter wardial".to_string());
    let narrow = w < NARROW;
    let watched = nightcomm::watched(save);
    let wh = if watched.is_empty() {
        0
    } else {
        (watched.len() + 2).min(h / 3)
    };
    let th = h - wh;
    // Columns, sized from their widest real value.
    let x_pre = c0 + 1;
    let x_lab = x_pre + 12;
    let lab_w = 18;
    let x_own = x_lab + lab_w + 1;
    let own_w = if narrow { 0 } else { 18 };
    let x_odds = if narrow {
        x_lab + lab_w + 1
    } else {
        x_own + own_w + 1
    };
    let x_tiers = x_odds + 11;
    let x_cost = x_tiers + 9;
    let x_dial = x_cost + 6;
    let x_from = x_dial + 17;
    g.text(p, x_pre, r0, "PREFIX", muted());
    g.text(p, x_lab, r0, "RANGE", muted());
    if !narrow {
        g.text(p, x_own, r0, "OWNER", muted());
    }
    g.text(p, x_odds, r0, "CARR TRAP", muted());
    g.text(p, x_tiers, r0, "L/C/M", muted());
    g.text_right(p, x_cost + 4, r0, "COST", muted());
    if x_dial + 16 <= c0 + w {
        g.text(p, x_dial, r0, "DIALED", muted());
    }
    let wide_from = !narrow && x_from + 10 <= c0 + w;
    if wide_from {
        g.text(p, x_from, r0, "FROM", muted());
    }
    for (i, r) in rows.iter().enumerate().take(th.saturating_sub(1)) {
        let y = r0 + 1 + i;
        let on = Some(&r.def.id) == nc.range.as_ref();
        if on {
            cx.picked(c0, y, w);
        }
        let (fg, dim) = if on {
            (shadow(), shadow())
        } else {
            (ink(), muted())
        };
        let d = r.def;
        let pre = format!(
            "{}{}{}",
            " ".repeat(r.depth.saturating_sub(1)),
            if r.depth > 0 { "└" } else { "" },
            d.pattern()
        );
        g.text(p, x_pre, y, &pre, fg);
        g.text_max(p, x_lab, y, &d.label, lab_w, fg);
        if !narrow {
            g.text_max(p, x_own, y, d.owner.as_deref().unwrap_or("—"), own_w, dim);
        }
        g.text_right(p, x_odds + 3, y, &format!("{}%", d.carrier_pct), fg);
        g.text_right(p, x_odds + 8, y, &format!("{}%", d.trap_pct), fg);
        g.text(
            p,
            x_tiers,
            y,
            &format!("{}/{}/{}", d.tiers.0, d.tiers.1, d.tiers.2),
            dim,
        );
        g.text_right(p, x_cost + 4, y, &format!("{}¢", d.cost), fg);
        if x_dial + 16 <= c0 + w {
            let lit = if r.blocks == 0 {
                0
            } else {
                (r.scanned as usize * 8).div_ceil(r.blocks as usize).min(8)
            };
            if on {
                g.text(p, x_dial, y, &format!("{:<8}", "■".repeat(lit)), fg);
            } else {
                g.meter(p, x_dial, y, 8, lit, &ramp());
            }
            g.text(
                p,
                x_dial + 9,
                y,
                &format!("{}/{}", r.scanned, r.blocks),
                dim,
            );
        }
        if wide_from {
            g.text_max(p, x_from, y, r.source, c0 + w - x_from - 1, dim);
        }
        let hit = cx.click(c0, y, w, ("nc-range", i));
        if hit.clicked() {
            nc.range = Some(d.id.clone());
        }
        if hit.double_clicked() {
            nc.range = Some(d.id.clone());
            nc.tab = Tab::Wardial;
        }
    }
    if wh > 0 {
        let top = r0 + th;
        cx.boxed(c0, top, w, wh, "watched lines");
        let label = |id: &str| {
            data.exchanges
                .iter()
                .find(|r| r.id == id)
                .map_or(id.to_string(), |r| r.label.clone())
        };
        for (i, f) in watched.iter().rev().take(wh - 2).enumerate() {
            g.text(p, c0 + 2, top + 1 + i, &f.number, accent());
            g.text(p, c0 + 12, top + 1 + i, &label(&f.range), muted());
            g.text(
                p,
                c0 + 32,
                top + 1 + i,
                &format!("block {:02}", f.block),
                muted(),
            );
        }
    }
}
