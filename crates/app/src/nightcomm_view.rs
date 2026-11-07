//! NiGHTCOMM, drawn as a btop screen on a cell grid: one rounded box per thing, titles cut
//! into the borders (`┐¹iNTEL┌`), key hints in the bottom border instead of the body, and
//! heat as a gradient meter. What it shows comes from core's `nightcomm` module; this only
//! draws and turns keys and clicks into a dial or a scan.

use egui::{Event, Id, Key, Sense, Ui};
use ninsei_core::Data;
use ninsei_core::at;
use ninsei_core::hub::{Answer, Found, RunEnd, Save, ScanResult};
use ninsei_core::modem::Handshake;
use ninsei_core::nightcomm::{self, Block, Sort, Tab};
use ninsei_core::tuning::{HEAT_COOL, HEAT_MAX};

use crate::home::{Dialing, WbOut, accent, fill, ink, muted, shadow};
use crate::hubtools::SCAN_FILL_SECS;
use crate::kit::Cells;

/// NiGHTCOMM's state between frames: the page, the order and filter, what's picked, and the
/// TERM page's line and scrollback.
pub struct NightComm {
    pub tab: Tab,
    pub sort: Sort,
    pub query: String,
    /// The phonebook entry picked on iNTEL.
    pub sel: Option<u32>,
    /// The range picked on WARDiAL and RANGES.
    pub range: Option<String>,
    /// TERM's command line.
    pub term: ninsei_term::LineEditor,
    /// What TERM has printed, oldest first.
    pub term_lines: Vec<String>,
    /// The entry a `jack in? [Y/n]` waits on.
    pub asking: Option<u32>,
    /// A page key typed on TERM's empty line, for whichever program shows TERM to act on.
    pub(crate) page_key: Option<char>,
    /// The frame it was last drawn on (none yet: it has just opened).
    drawn: Option<u64>,
    want_focus: bool,
    want_filter: bool,
}

impl Default for NightComm {
    fn default() -> Self {
        NightComm {
            tab: Tab::default(),
            sort: Sort::default(),
            query: String::new(),
            sel: None,
            range: None,
            term: ninsei_term::LineEditor::new(TERM_HISTORY),
            term_lines: vec![
                "NiGHTCOMM 3.1 TERM".to_string(),
                at::IDENT.to_string(),
                "OK".to_string(),
            ],
            asking: None,
            page_key: None,
            drawn: None,
            want_focus: false,
            want_filter: false,
        }
    }
}

impl NightComm {
    /// Prints the modem's answer on TERM.
    pub fn term_say(&mut self, lines: &[String]) {
        self.term_lines.extend(lines.iter().cloned());
        let extra = self.term_lines.len().saturating_sub(TERM_KEEP);
        self.term_lines.drain(..extra);
    }
}

/// Commands TERM remembers for ↑ and Ctrl+R.
const TERM_HISTORY: usize = 100;
/// Lines of TERM scrollback kept.
const TERM_KEEP: usize = 300;
/// Width of TERM's modem and AT boxes.
const TERM_SIDE: usize = 26;

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
pub(crate) struct Keys {
    pub(crate) up: bool,
    pub(crate) down: bool,
    pub(crate) enter: bool,
    pub(crate) space: bool,
    pub(crate) text: Vec<char>,
}

/// This frame's keys, if the page area `keys_id` has the focus. Arrows and Enter stay with it.
pub(crate) fn read_keys(ui: &Ui, keys_id: Id) -> Keys {
    if ui.memory(|m| m.has_focus(keys_id)) {
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
    }
}

/// Draws NiGHTCOMM in the space `ui` offers.
#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut Ui,
    data: &Data,
    save: &Save,
    nc: &mut NightComm,
    scan: Option<&(ScanResult, f32)>,
    dialing: Option<&Dialing>,
    dial: &mut Option<u32>,
    out: &mut Vec<WbOut>,
) {
    let size = ui.available_size();
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let keys_id = resp.id;
    let filter_id = Id::new("nightcomm-filter");
    let term_id = Id::new("nightcomm-term");

    // Not drawn last pass: the window just opened (or a splash ended). It takes the keys.
    let pass = ui.ctx().cumulative_pass_nr();
    if nc.drawn.is_none_or(|d| d + 1 < pass) {
        nc.want_focus = true;
    }
    nc.drawn = Some(pass);
    if resp.clicked() {
        nc.want_focus = true;
    }
    if ui.is_enabled() {
        if nc.want_filter {
            ui.memory_mut(|m| m.request_focus(filter_id));
            nc.want_filter = false;
            nc.want_focus = false;
        } else if nc.want_focus {
            // TERM's line takes the keys on its page; the page keys everywhere else.
            let target = if nc.tab == Tab::Term {
                term_id
            } else {
                keys_id
            };
            ui.memory_mut(|m| m.request_focus(target));
            nc.want_focus = false;
        }
    }
    let keys = read_keys(ui, keys_id);
    for c in &keys.text {
        if let Some(t) = Tab::from_key(*c) {
            nc.tab = t;
            nc.want_focus = true;
        }
    }

    let g = Cells::fit(ui, rect, muted(), shadow(), muted());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 0.0, shadow());
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
    let ctx = Ctx {
        ui,
        g: &g,
        p: &p,
        tool: ninsei_core::hub::Tool::NightComm,
    };
    let mut legend: Vec<String> = Vec::new();
    match nc.tab {
        Tab::Intel => intel(&ctx, save, nc, &keys, inner, dial, &mut legend),
        Tab::Wardial => wardial(&ctx, data, save, nc, &keys, scan, inner, out, &mut legend),
        Tab::Ranges => ranges(&ctx, data, save, nc, &keys, inner, &mut legend),
        Tab::Term => term(
            &ctx,
            ninsei_core::hub::Tool::NightComm,
            save,
            nc,
            dialing,
            term_id,
            inner,
            out,
            &mut legend,
        ),
    }
    if let Some(t) = nc.page_key.take().and_then(Tab::from_key) {
        nc.tab = t;
        nc.want_focus = true;
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
pub(crate) struct Ctx<'a> {
    pub(crate) ui: &'a Ui,
    pub(crate) g: &'a Cells,
    pub(crate) p: &'a egui::Painter,
    /// The program drawing: it salts the pages' widget ids (NiGHTCOMM and Tiger Team Pro
    /// can both be open, drawing the same pages) and says where a scan's result goes.
    pub(crate) tool: ninsei_core::hub::Tool,
}

impl Ctx<'_> {
    pub(crate) fn click(
        &self,
        c: usize,
        r: usize,
        w: usize,
        id: impl std::hash::Hash + std::fmt::Debug,
    ) -> egui::Response {
        self.ui.interact(
            self.g.at(c, r, w, 1),
            Id::new(self.tool).with(id),
            Sense::click(),
        )
    }

    /// An inverse-video row: the period's selection.
    pub(crate) fn picked(&self, c: usize, r: usize, w: usize) {
        self.g.fill(self.p, c, r, w, 1, accent());
    }

    /// A sub-box with its title cut into the top border.
    pub(crate) fn boxed(&self, c: usize, r: usize, w: usize, h: usize, title: &str) {
        self.g.frame(self.p, c, r, w, h);
        if !title.is_empty() {
            self.g.title(self.p, c + 2, r, title, ink());
        }
    }
}

/// Steps a selection through `ids` by the arrow keys; an unknown pick lands on the first.
pub(crate) fn step<T: PartialEq + Clone>(ids: &[T], cur: &mut Option<T>, keys: &Keys) {
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
pub(crate) fn scroll_to(at: usize, rows: usize) -> usize {
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
pub(crate) fn wardial(
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
        out.push(WbOut::Scan(def.id.clone(), Some(cx.tool)));
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
pub(crate) fn ranges(
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

/// The modem's lights, as on a US Robotics Sportster's front panel.
const LIGHTS: [&str; 6] = ["MR", "TR", "CD", "OH", "RD", "SD"];

/// TERM's quick-reference card, like the one that came in the modem's box.
const CARD: [(&str, &str); 7] = [
    ("ATDT n", "dial"),
    ("ATDT n!", "watched line"),
    ("ATDS k", "phonebook #k"),
    ("ATH", "hang up"),
    ("ATZ", "reset"),
    ("ATM0 M1", "speaker"),
    ("ATI", "modem info"),
];

#[allow(clippy::too_many_arguments)]
pub(crate) fn term(
    cx: &Ctx,
    from: ninsei_core::hub::Tool,
    save: &Save,
    nc: &mut NightComm,
    dialing: Option<&Dialing>,
    term_id: Id,
    (c0, r0, w, h): (usize, usize, usize, usize),
    out: &mut Vec<WbOut>,
    legend: &mut Vec<String>,
) {
    let (g, p) = (cx.g, cx.p);
    legend.push("↑↓ history".to_string());
    legend.push("tab complete".to_string());
    legend.push("1-4 pages from an empty line".to_string());
    let narrow = w < NARROW;
    let side = if narrow { 0 } else { TERM_SIDE };
    let tw = w - side;
    cx.boxed(c0, r0, tw, h, "terminal");
    // The call under way prints as the handshake reaches each beat.
    let mut lines: Vec<(String, egui::Color32)> =
        nc.term_lines.iter().map(|l| (l.clone(), ink())).collect();
    let call = dialing.and_then(|d| save.entry(d.entry).map(|e| (d, e)));
    if let Some((d, e)) = call {
        let hs = Handshake::new(&e.number);
        // `DIALING` is already printed: the line picks up at the first ring.
        let mut beats: Vec<(f32, String)> =
            hs.rings.iter().map(|r| (*r, "RING".to_string())).collect();
        beats.push((hs.connect, "CONNECT 2400/ARQ".to_string()));
        beats.push((
            hs.connect + (hs.end - hs.connect) / 2.0,
            "CARRIER DETECTED. JACKING IN...".to_string(),
        ));
        lines.extend(
            beats
                .into_iter()
                .filter(|(at, _)| d.t >= *at)
                .map(|(_, l)| (l, accent())),
        );
        cx.ui.ctx().request_repaint();
    }
    let rows = h.saturating_sub(3);
    let start = lines.len().saturating_sub(rows);
    for (i, (l, col)) in lines[start..].iter().enumerate() {
        let col = if l.starts_with("> ") {
            muted()
        } else if l.starts_with("!!") {
            accent()
        } else {
            *col
        };
        g.text_max(p, c0 + 2, r0 + 1 + i, l, tw - 4, col);
    }
    // The command line, on the box's last row.
    let line_rect = g.at(c0 + 2, r0 + h - 2, tw - 4, 1);
    let had_text = !nc.term.text().is_empty();
    let focused = cx.ui.memory(|m| m.has_focus(term_id));
    // A page key on an empty line switches pages instead of typing.
    let mut page = None;
    if focused && !had_text {
        cx.ui.input(|i| {
            for e in &i.events {
                if let Event::Text(t) = e
                    && let Some(c) = t.chars().next().filter(|c| ('1'..='9').contains(c))
                {
                    page = Some(c);
                }
            }
        });
    }
    let style = ninsei_term::widget::Style {
        font: egui::TextStyle::Monospace.resolve(cx.ui.style()),
        prompt: accent(),
        text: ink(),
        cursor: ink(),
        under_cursor: shadow(),
        search: accent(),
    };
    let prompt = if nc.asking.is_some() { "[Y/n]" } else { ">" };
    let complete = |before: &str| at::complete(before, save);
    let mut child = Ui::new(
        cx.ui.ctx().clone(),
        term_id.with("ui"),
        egui::UiBuilder::new()
            .layer_id(cx.ui.layer_id())
            .max_rect(line_rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.set_clip_rect(line_rect.intersect(cx.ui.clip_rect()));
    let r = ninsei_term::widget::line(&mut child, term_id, &mut nc.term, prompt, &style, &complete);
    if r.response.clicked() {
        nc.want_focus = true;
    }
    if let Some(c) = page {
        nc.term.clear();
        nc.page_key = Some(c);
        return;
    }
    for o in r.outcomes {
        match o {
            ninsei_term::Outcome::Submit(line) => {
                nc.term_say(&[format!("{prompt} {line}")]);
                out.push(WbOut::At(from, line));
            }
            ninsei_term::Outcome::Cancelled(was) => {
                nc.term_say(&[format!("{prompt} {was}^C")]);
                nc.asking = None;
            }
            ninsei_term::Outcome::ClearScreen => nc.term_lines.clear(),
            ninsei_term::Outcome::List(words) => nc.term_say(&[words.join("   ")]),
            ninsei_term::Outcome::Nothing => {}
        }
    }
    if side == 0 {
        return;
    }
    // The modem's front panel.
    let sx = c0 + tw;
    cx.boxed(sx, r0, side, 5, "modem");
    let connected = call.is_some_and(|(d, e)| d.t >= Handshake::new(&e.number).connect);
    let blink = (cx.ui.input(|i| i.time) * 6.0) as i64 % 2 == 0;
    for (i, name) in LIGHTS.iter().enumerate() {
        let lit = match *name {
            "MR" => true,
            "TR" => focused,
            "CD" => connected,
            "OH" => call.is_some(),
            "RD" | "SD" => call.is_some() && blink == (*name == "RD"),
            _ => false,
        };
        let x = sx + 2 + i * 4;
        let dot = g.at(x, r0 + 1, 2, 1);
        let radius = g.ch * 0.22;
        if lit {
            p.circle_filled(dot.center(), radius, accent());
        } else {
            p.circle_stroke(dot.center(), radius, egui::Stroke::new(1.0, muted()));
        }
        g.text(p, x, r0 + 2, name, muted());
    }
    let status = if connected {
        "carrier"
    } else if call.is_some() {
        "off hook"
    } else {
        "on hook"
    };
    g.text(
        p,
        sx + 2,
        r0 + 3,
        &format!(
            "{status} · speaker {}",
            if save.sound { "on" } else { "off" }
        ),
        ink(),
    );
    let ch = h - 5;
    cx.boxed(sx, r0 + 5, side, ch, "AT");
    for (i, (cmd, what)) in CARD.iter().enumerate().take(ch.saturating_sub(2)) {
        g.text(p, sx + 2, r0 + 6 + i, cmd, ink());
        g.text_max(p, sx + 11, r0 + 6 + i, what, side - 13, muted());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::home::Home;
    use crate::home::tests::{deck, frame};
    use crate::uitest::{Harness, key, text};
    use ninsei_core::hub::Tool;

    /// NiGHTCOMM open for a couple of frames, so it has the keys.
    fn opened(data: &Data, save: &Save) -> (Harness, Home) {
        let (mut h, mut home) = (Harness::new(1280.0, 800.0), Home::new(Vec::new()));
        for _ in 0..2 {
            frame(&mut h, &mut home, data, save, Tool::NightComm, Vec::new());
        }
        (h, home)
    }

    fn nightcomm_deck(data: &Data) -> Save {
        let mut s = deck(data);
        s.add_starters(data);
        s.tools = vec!["nightcomm".into()];
        s.bank = 10_000;
        s
    }

    #[test]
    fn digits_pick_the_page() {
        let data = Data::builtin().unwrap();
        let save = nightcomm_deck(&data);
        let (mut h, mut home) = opened(&data, &save);
        for (c, tab) in [("2", Tab::Wardial), ("3", Tab::Ranges), ("1", Tab::Intel)] {
            frame(&mut h, &mut home, &data, &save, Tool::NightComm, text(c));
            assert_eq!(home.nightcomm.tab, tab, "{c}");
        }
    }

    #[test]
    fn s_turns_the_sort_and_slash_filters() {
        let data = Data::builtin().unwrap();
        let save = nightcomm_deck(&data);
        let (mut h, mut home) = opened(&data, &save);
        frame(&mut h, &mut home, &data, &save, Tool::NightComm, text("s"));
        assert_eq!(home.nightcomm.sort, Sort::default().next());
        for events in [text("/"), Vec::new(), text("gut")] {
            frame(&mut h, &mut home, &data, &save, Tool::NightComm, events);
        }
        assert_eq!(
            home.nightcomm.query, "gut",
            "/ opens the filter and it takes the typing"
        );
        assert_eq!(
            home.nightcomm.sort,
            Sort::default().next(),
            "typed s's don't sort"
        );
    }

    #[test]
    fn arrows_pick_a_number_and_d_dials_it() {
        let data = Data::builtin().unwrap();
        let save = nightcomm_deck(&data);
        let (mut h, mut home) = opened(&data, &save);
        let first = home.nightcomm.sel.expect("a number is picked");
        frame(
            &mut h,
            &mut home,
            &data,
            &save,
            Tool::NightComm,
            key(egui::Key::ArrowDown),
        );
        let picked = home.nightcomm.sel.unwrap();
        assert_ne!(picked, first, "down moves on");
        frame(&mut h, &mut home, &data, &save, Tool::NightComm, text("d"));
        assert_eq!(home.dialing.map(|d| d.entry), Some(picked));
    }

    #[test]
    fn space_on_wardial_scans_the_picked_range() {
        let data = Data::builtin().unwrap();
        let save = nightcomm_deck(&data);
        let (mut h, mut home) = opened(&data, &save);
        frame(&mut h, &mut home, &data, &save, Tool::NightComm, text("2"));
        let out = frame(&mut h, &mut home, &data, &save, Tool::NightComm, text(" "));
        let range = home.nightcomm.range.clone().expect("a range is picked");
        assert!(
            out.iter()
                .any(|o| matches!(o, WbOut::Scan(r, Some(Tool::NightComm)) if *r == range)),
            "space scans {range}"
        );
        let mut broke = save.clone();
        broke.bank = 0;
        let out = frame(&mut h, &mut home, &data, &broke, Tool::NightComm, text(" "));
        assert!(
            !out.iter().any(|o| matches!(o, WbOut::Scan(..))),
            "no scan you can't pay for"
        );
    }

    /// The lines TERM sent to the modem.
    fn sent(out: &[WbOut]) -> Vec<&str> {
        out.iter()
            .filter_map(|o| match o {
                WbOut::At(Tool::NightComm, line) => Some(line.as_str()),
                _ => None,
            })
            .collect()
    }

    /// NiGHTCOMM open on TERM, with the command line taking the keys.
    fn on_term(data: &Data, save: &Save) -> (Harness, Home) {
        let (mut h, mut home) = opened(data, save);
        for events in [text("4"), Vec::new(), Vec::new()] {
            frame(&mut h, &mut home, data, save, Tool::NightComm, events);
        }
        assert_eq!(home.nightcomm.tab, Tab::Term);
        (h, home)
    }

    #[test]
    fn a_command_typed_on_term_goes_to_the_modem() {
        let data = Data::builtin().unwrap();
        let save = nightcomm_deck(&data);
        let (mut h, mut home) = on_term(&data, &save);
        frame(
            &mut h,
            &mut home,
            &data,
            &save,
            Tool::NightComm,
            text("ATDT 555-0420"),
        );
        let out = frame(
            &mut h,
            &mut home,
            &data,
            &save,
            Tool::NightComm,
            key(Key::Enter),
        );
        assert_eq!(sent(&out), ["ATDT 555-0420"]);
        assert!(
            home.nightcomm
                .term_lines
                .iter()
                .any(|l| l == "> ATDT 555-0420"),
            "it echoes the command"
        );
        assert_eq!(home.nightcomm.term.text(), "", "and clears the line");
    }

    #[test]
    fn a_digit_turns_the_page_only_from_an_empty_line() {
        let data = Data::builtin().unwrap();
        let save = nightcomm_deck(&data);
        let (mut h, mut home) = on_term(&data, &save);
        frame(
            &mut h,
            &mut home,
            &data,
            &save,
            Tool::NightComm,
            text("ATDS"),
        );
        frame(&mut h, &mut home, &data, &save, Tool::NightComm, text("2"));
        assert_eq!(
            home.nightcomm.tab,
            Tab::Term,
            "a digit in a command is typed"
        );
        assert_eq!(home.nightcomm.term.text(), "ATDS2");
        let mut ctrl_u = key(Key::U);
        for e in &mut ctrl_u {
            if let Event::Key { modifiers, .. } = e {
                *modifiers = egui::Modifiers::CTRL;
            }
        }
        frame(&mut h, &mut home, &data, &save, Tool::NightComm, ctrl_u);
        assert_eq!(home.nightcomm.term.text(), "", "Ctrl+U kills the line");
        frame(&mut h, &mut home, &data, &save, Tool::NightComm, text("2"));
        assert_eq!(
            home.nightcomm.tab,
            Tab::Wardial,
            "on an empty line it turns the page"
        );
    }
}
