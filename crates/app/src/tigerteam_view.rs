//! Tiger Team Pro, drawn as a Borland Turbo Vision application on a cell grid: a menu bar of
//! sections across the top, one double-framed window with a centred title and a drop shadow on
//! a flat desktop, single-line group boxes inside, and a status line of keys along the bottom.
//! The report itself comes from core's `tigerteam` module; Reconnaissance, Address Space and
//! the Console are NiGHTCOMM's pages, dressed in this frame.

use egui::{Id, Sense, Ui};
use ninsei_core::Data;
use ninsei_core::hub::{RunEnd, Save, ScanResult, Severity, Tool};
use ninsei_core::nightcomm::Tab;
use ninsei_core::tigerteam::{self, Order, Page, Profile};
use ninsei_core::tuning::HEAT_MAX;

use crate::home::{Dialing, WbOut, accent, backdrop, fill, ink, muted, shadow};
use crate::kit::Cells;
use crate::nightcomm_view::{self as nc_view, Ctx, Keys, NightComm};

/// Tiger Team Pro's state between frames.
pub struct TigerTeam {
    pub page: Page,
    /// The target picked on Assessment and Engagements.
    pub sel: Option<u32>,
    pub order: Order,
    /// The pages it shares with NiGHTCOMM keep their state here, its own terminal included.
    pub nc: NightComm,
    /// The frame it was last drawn on (none yet: it has just opened).
    drawn: Option<u64>,
    want_focus: bool,
}

impl Default for TigerTeam {
    fn default() -> Self {
        let mut nc = NightComm::default();
        nc.term_lines = vec![
            "Tiger Team Pro 3.0 Console".to_string(),
            ninsei_core::at::IDENT.to_string(),
            "OK".to_string(),
        ];
        TigerTeam {
            page: Page::default(),
            sel: None,
            order: Order::default(),
            nc,
            drawn: None,
            want_focus: false,
        }
    }
}

/// Width of the Assessment section's target list.
const TARGETS_W: usize = 30;
/// Fewer columns than this and tables drop their wide columns.
const NARROW: usize = 76;

fn severity_col(s: Severity) -> egui::Color32 {
    match s {
        Severity::High => accent(),
        Severity::Medium => ink(),
        _ => muted(),
    }
}

/// Draws Tiger Team Pro in the space `ui` offers.
#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut Ui,
    data: &Data,
    save: &Save,
    tt: &mut TigerTeam,
    scan: Option<&(ScanResult, f32)>,
    dialing: Option<&Dialing>,
    dial: &mut Option<u32>,
    out: &mut Vec<WbOut>,
) {
    let size = ui.available_size();
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let keys_id = resp.id;
    let term_id = Id::new("tigerteam-console");
    let pass = ui.ctx().cumulative_pass_nr();
    if tt.drawn.is_none_or(|d| d + 1 < pass) {
        tt.want_focus = true;
    }
    tt.drawn = Some(pass);
    if resp.clicked() {
        tt.want_focus = true;
    }
    if tt.want_focus && ui.is_enabled() {
        let target = if tt.page == Page::Console {
            term_id
        } else {
            keys_id
        };
        ui.memory_mut(|m| m.request_focus(target));
        tt.want_focus = false;
    }
    let keys = nc_view::read_keys(ui, keys_id);
    for c in &keys.text {
        if let Some(pg) = Page::from_key(*c) {
            tt.page = pg;
            tt.want_focus = true;
        }
    }

    let mut g = Cells::fit(ui, rect, ink(), backdrop(), muted());
    g.vision = true;
    let p = ui.painter_at(rect);
    // The desktop.
    p.rect_filled(rect, 0.0, fill());
    // The menu bar: the sections, the current one in inverse.
    g.fill(&p, 0, 0, g.cols, 1, ink());
    let full: usize = Page::ALL
        .iter()
        .map(|pg| pg.title().chars().count() + 5)
        .sum();
    let tight = full + 1 > g.cols;
    let mut c = 1;
    for (i, pg) in Page::ALL.iter().enumerate() {
        let name = if tight { pg.short() } else { pg.title() };
        let label = format!(" {} {name} ", i + 1);
        let n = label.chars().count();
        if c + n >= g.cols {
            break;
        }
        let on = *pg == tt.page;
        if on {
            g.fill(&p, c, 0, n, 1, shadow());
        }
        g.text(&p, c, 0, &label, if on { ink() } else { shadow() });
        if ui
            .interact(g.at(c, 0, n, 1), Id::new(("tt-menu", i)), Sense::click())
            .clicked()
        {
            tt.page = *pg;
            tt.want_focus = true;
        }
        c += n + 1;
    }
    // The window: a drop shadow, then the double frame with its title centred.
    let (wc, wr, ww, wh) = (1, 2, g.cols - 3, g.rows - 4);
    g.fill(&p, wc + 2, wr + 1, ww, wh, shadow());
    g.fill(&p, wc, wr, ww, wh, backdrop());
    g.frame_double(&p, wc, wr, ww, wh);
    let title = match tt.page {
        Page::Assessment => "Assessment report",
        pg => pg.title(),
    };
    let tc = wc + (ww.saturating_sub(title.chars().count() + 2)) / 2;
    g.title(&p, tc, wr, title, ink());
    g.title(&p, wc + 2, wr, "■", ink());
    let inner = (wc + 1, wr + 1, ww - 2, wh - 2);
    let ctx = Ctx {
        ui,
        g: &g,
        p: &p,
        tool: Tool::TigerTeam,
    };
    let mut legend = vec!["1-5 Section".to_string()];
    match tt.page {
        Page::Assessment => assessment(&ctx, data, save, tt, &keys, inner, dial, &mut legend),
        Page::Engagements => engagements(&ctx, data, save, tt, &keys, inner, &mut legend),
        Page::Recon => {
            let mut more = Vec::new();
            nc_view::wardial(
                &ctx, data, save, &mut tt.nc, &keys, scan, inner, out, &mut more,
            );
            legend.extend(more);
        }
        Page::Address => {
            let mut more = Vec::new();
            nc_view::ranges(&ctx, data, save, &mut tt.nc, &keys, inner, &mut more);
            if tt.nc.tab == Tab::Wardial {
                // Enter on a range opens it in Reconnaissance.
                tt.nc.tab = Tab::Ranges;
                tt.page = Page::Recon;
            }
            legend.extend(more);
        }
        Page::Console => {
            let mut more = Vec::new();
            nc_view::term(
                &ctx,
                Tool::TigerTeam,
                save,
                &mut tt.nc,
                dialing,
                term_id,
                inner,
                out,
                &mut more,
            );
            legend.extend(more.into_iter().filter(|l| !l.starts_with("1-4")));
        }
    }
    if let Some(pg) = tt.nc.page_key.take().and_then(Page::from_key) {
        tt.page = pg;
        tt.want_focus = true;
    }
    // The status line.
    let last = g.rows - 1;
    g.fill(&p, 0, last, g.cols, 1, ink());
    let mut c = 1;
    for l in &legend {
        let n = l.chars().count();
        if c + n >= g.cols {
            break;
        }
        // Turbo Vision coloured its hot keys; on a mono tube that colour is the bar's own,
        // so the whole line reads dark on light.
        g.text(&p, c, last, l, shadow());
        c += n + 3;
    }
    // The licence's fine print, where there's room for it.
    let note = "For authorized security testing only";
    if c + note.chars().count() + 1 < g.cols {
        g.text_right(&p, g.cols - 1, last, note, shadow());
    }
    if tt.want_focus {
        ui.ctx().request_repaint();
    }
}

/// A Turbo Vision push button with its shadow. Returns whether it was clicked.
fn button(cx: &Ctx, c: usize, r: usize, label: &str, id: &str) -> bool {
    let (g, p) = (cx.g, cx.p);
    let n = label.chars().count() + 2;
    g.fill(p, c + 1, r + 1, n, 1, shadow());
    g.fill(p, c + n, r, 1, 1, shadow());
    g.fill(p, c, r, n, 1, accent());
    g.text(p, c + 1, r, label, shadow());
    cx.click(c, r, n, ("tt-button", id)).clicked()
}

#[allow(clippy::too_many_arguments)]
fn assessment(
    cx: &Ctx,
    data: &Data,
    save: &Save,
    tt: &mut TigerTeam,
    keys: &Keys,
    (c0, r0, w, h): (usize, usize, usize, usize),
    dial: &mut Option<u32>,
    legend: &mut Vec<String>,
) {
    let (g, p) = (cx.g, cx.p);
    legend.push("↑↓ Target".to_string());
    legend.push("Enter Engage".to_string());
    let targets = tigerteam::targets(save, data);
    let ids: Vec<u32> = targets.iter().map(|t| t.entry.id).collect();
    nc_view::step(&ids, &mut tt.sel, keys);
    if keys.enter {
        *dial = tt.sel;
    }
    if targets.is_empty() {
        g.text(p, c0 + 2, r0 + 1, "No targets on file.", muted());
        return;
    }
    // The targets, most exposed first: down the left, or across the top of a narrow window.
    let narrow = w < NARROW;
    let (lw, lh) = if narrow {
        (w - 2, (targets.len() + 2).clamp(3, (h / 3).max(3)))
    } else {
        (TARGETS_W.min(w / 3).max(16), h)
    };
    cx.boxed(c0 + 1, r0, lw, lh, "Targets");
    let rows = lh.saturating_sub(2);
    let at = tt
        .sel
        .and_then(|s| ids.iter().position(|i| *i == s))
        .unwrap_or(0);
    let start = nc_view::scroll_to(at, rows);
    for (k, t) in targets.iter().enumerate().skip(start).take(rows) {
        let y = r0 + 1 + k - start;
        let on = tt.sel == Some(t.entry.id);
        if on {
            cx.picked(c0 + 2, y, lw - 2);
        }
        let fg = if on { shadow() } else { ink() };
        g.text_max(p, c0 + 3, y, &t.entry.name, lw - 9, fg);
        let col = if on {
            shadow()
        } else if t.exposure == tigerteam::Exposure::High {
            accent()
        } else {
            muted()
        };
        g.text_right(p, c0 + lw - 1, y, t.exposure.label(), col);
        if cx
            .click(c0 + 2, y, lw - 2, ("tt-target", t.entry.id))
            .clicked()
        {
            tt.sel = Some(t.entry.id);
        }
    }
    let Some(t) = targets.iter().find(|t| Some(t.entry.id) == tt.sel) else {
        return;
    };
    let area = if narrow {
        (c0 + 1, r0 + lh, w - 2, h - lh)
    } else {
        (c0 + lw + 2, r0, w - lw - 3, h)
    };
    report(cx, t, area, dial);
}

/// One target's report.
fn report(
    cx: &Ctx,
    t: &Profile,
    (x, y, w, h): (usize, usize, usize, usize),
    dial: &mut Option<u32>,
) {
    let (g, p) = (cx.g, cx.p);
    let e = t.entry;
    // Room for the Engage button beside the record, or it goes under it.
    let beside = w >= 50;
    g.text_max(p, x, y, &e.name, w.saturating_sub(20), accent());
    g.text_right(p, x + w, y, &format!("{}  {}", e.number, t.tier), ink());
    g.text_max(
        p,
        x,
        y + 1,
        &format!(
            "Listed as {}  ·  Board on site: {}",
            e.business,
            t.board.unwrap_or("none")
        ),
        w,
        muted(),
    );
    // Four figures in boxes, like a report's summary row.
    let stats = [
        ("TOPOLOGY", format!("{} layers", t.layers)),
        ("DEFENCES", format!("ICE +{}", t.ice_bonus)),
        ("TRACE", format!("{}%", t.trace_pct)),
        ("ASSET VALUE", format!("{}%", t.pay_pct)),
    ];
    let bw = w / stats.len();
    let boxes = bw >= 14;
    for (i, (label, value)) in stats.iter().enumerate() {
        if boxes {
            let bx = x + i * bw;
            cx.boxed(bx, y + 3, bw - 1, 4, "");
            g.text_max(p, bx + 1, y + 4, label, bw - 3, muted());
            g.text_max(p, bx + 1, y + 5, value, bw - 3, ink());
        } else {
            // Too narrow for four boxes: two figures a line.
            let (col, row) = (i % 2, i / 2);
            let bx = x + col * (w / 2);
            g.text(p, bx, y + 3 + row, label, muted());
            g.text(p, bx + 12, y + 3 + row, value, ink());
        }
    }
    // Findings, worst first.
    let fy = if boxes { y + 8 } else { y + 6 };
    let sev_w = 9;
    g.text(p, x, fy, "ID", muted());
    g.text(p, x + 6, fy, "FINDING", muted());
    g.text(p, x + w - sev_w, fy, "SEVERITY", muted());
    if t.findings.is_empty() {
        g.text(p, x + 6, fy + 1, "No findings.", muted());
    }
    let room = h.saturating_sub(fy - y + if beside { 6 } else { 7 });
    for (i, f) in t.findings.iter().enumerate().take(room) {
        let r = fy + 1 + i;
        g.text(p, x, r, &f.id, ink());
        g.text_max(p, x + 6, r, &f.text, w.saturating_sub(sev_w + 7), ink());
        g.text(
            p,
            x + w - sev_w,
            r,
            f.severity.label(),
            severity_col(f.severity),
        );
    }
    // Alert level and the record, then the button: beside them, or under them when narrow.
    let ay = if beside { y + h - 4 } else { y + h - 5 };
    let exits = e.last.iter().filter(|r| **r == RunEnd::Exit).count();
    g.text(p, x, ay, "Alert level", muted());
    g.meter(
        p,
        x + 13,
        ay,
        HEAT_MAX as usize,
        e.heat as usize,
        &[muted(), ink(), accent()],
    );
    g.text(
        p,
        x + 14 + HEAT_MAX as usize,
        ay,
        &format!("{} of {HEAT_MAX}", e.heat),
        ink(),
    );
    g.text(p, x, ay + 1, "Engagements", muted());
    g.text(
        p,
        x + 13,
        ay + 1,
        &match e.runs {
            0 => "none on file".to_string(),
            n => format!("{n}  ·  last {}: {exits} completed", e.last.len()),
        },
        ink(),
    );
    g.text(p, x, ay + 2, "Exposure", muted());
    g.text(
        p,
        x + 13,
        ay + 2,
        t.exposure.label(),
        if t.exposure == tigerteam::Exposure::High {
            accent()
        } else {
            ink()
        },
    );
    let label = "Engage";
    let (bx, by) = if beside {
        ((x + w).saturating_sub(label.len() + 4), ay + 1)
    } else {
        (x, ay + 3)
    };
    if button(cx, bx, by, label, "engage") {
        *dial = Some(e.id);
    }
}

#[allow(clippy::too_many_arguments)]
fn engagements(
    cx: &Ctx,
    data: &Data,
    save: &Save,
    tt: &mut TigerTeam,
    keys: &Keys,
    (c0, r0, w, h): (usize, usize, usize, usize),
    legend: &mut Vec<String>,
) {
    let (g, p) = (cx.g, cx.p);
    for c in &keys.text {
        if c.eq_ignore_ascii_case(&'s') {
            tt.order = tt.order.next();
        }
    }
    legend.push(format!("S Sort: {}", tt.order.label()));
    legend.push("Enter Assess".to_string());
    let rows = tigerteam::engagements(save, data, tt.order);
    let ids: Vec<u32> = rows.iter().map(|t| t.entry.id).collect();
    nc_view::step(&ids, &mut tt.sel, keys);
    if keys.enter && tt.sel.is_some() {
        tt.page = Page::Assessment;
    }
    let narrow = w < NARROW;
    // Columns, sized from their widest real value (MEGACORP is 8). In a narrow window the
    // findings go first, then layers, trace and alert, so what's left always fits.
    let x_num = c0 + 2;
    let x_name = x_num + 10;
    let find_w = if narrow { 0 } else { 18 };
    let room = w.saturating_sub(2 + 10 + 2 + find_w);
    let core = 9 + 5 + 7 + 6;
    let spare = room.saturating_sub(core + 12);
    let show_alert = spare >= 6;
    let show_trace = spare >= 13;
    let show_layers = spare >= 20;
    let tail = core
        + if show_layers { 7 } else { 0 }
        + if show_trace { 7 } else { 0 }
        + if show_alert { 6 } else { 0 };
    let name_w = room.saturating_sub(tail).max(8);
    let x_tier = x_name + name_w + 1;
    let x_lay = x_tier + 9;
    let x_ice = x_lay + if show_layers { 7 } else { 0 };
    let x_trace = x_ice + 5;
    let x_val = x_trace + if show_trace { 7 } else { 0 };
    let x_alert = x_val + 7;
    let x_exp = x_alert + if show_alert { 6 } else { 0 };
    let x_find = x_exp + 6;
    let head = |x: usize, s: &str, sort: Option<Order>| {
        let s = if sort == Some(tt.order) {
            format!("{s}▼")
        } else {
            s.to_string()
        };
        g.text(p, x, r0, &s, muted());
    };
    head(x_num, "NUMBER", None);
    head(x_name, "TARGET", Some(Order::Name));
    head(x_tier, "TIER", Some(Order::Tier));
    if show_layers {
        head(x_lay, "LAYERS", None);
    }
    head(x_ice, "ICE", None);
    if show_trace {
        head(x_trace, "TRACE", None);
    }
    head(x_val, "VALUE", Some(Order::Value));
    if show_alert {
        head(x_alert, "ALERT", Some(Order::Alert));
    }
    head(x_exp, "EXPO", Some(Order::Exposure));
    if !narrow {
        head(x_find, "FINDINGS", None);
    }
    let visible = h.saturating_sub(1);
    let at = tt
        .sel
        .and_then(|s| ids.iter().position(|i| *i == s))
        .unwrap_or(0);
    let start = nc_view::scroll_to(at, visible);
    for (k, t) in rows.iter().enumerate().skip(start).take(visible) {
        let y = r0 + 1 + k - start;
        let on = tt.sel == Some(t.entry.id);
        if on {
            cx.picked(c0 + 1, y, w - 2);
        }
        let (fg, dim) = if on {
            (shadow(), shadow())
        } else {
            (ink(), muted())
        };
        g.text(p, x_num, y, &t.entry.number, fg);
        g.text_max(p, x_name, y, &t.entry.name, name_w, fg);
        g.text(p, x_tier, y, &t.tier, dim);
        if show_layers {
            g.text_right(p, x_lay + 6, y, &t.layers.to_string(), fg);
        }
        g.text_right(p, x_ice + 4, y, &format!("+{}", t.ice_bonus), fg);
        if show_trace {
            g.text_right(p, x_trace + 6, y, &format!("{}%", t.trace_pct), fg);
        }
        g.text_right(p, x_val + 6, y, &format!("{}%", t.pay_pct), fg);
        if show_alert {
            g.text_right(p, x_alert + 5, y, &t.entry.heat.to_string(), fg);
        }
        let ecol = if on {
            fg
        } else if t.exposure == tigerteam::Exposure::High {
            accent()
        } else {
            dim
        };
        g.text(p, x_exp, y, t.exposure.label(), ecol);
        if !narrow {
            let worst = t.findings.first().map_or("none".to_string(), |f| {
                format!("{} · {}", t.findings.len(), f.severity.label())
            });
            g.text_max(p, x_find, y, &worst, c0 + w - x_find - 1, dim);
        }
        let hit = cx.click(c0 + 1, y, w - 2, ("tt-eng", t.entry.id));
        if hit.clicked() {
            tt.sel = Some(t.entry.id);
        }
        if hit.double_clicked() {
            tt.sel = Some(t.entry.id);
            tt.page = Page::Assessment;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::home::Home;
    use crate::home::tests::{deck, frame};
    use crate::uitest::{Harness, key, text};
    use egui::Key;

    fn licensed(data: &Data) -> Save {
        let mut s = deck(data);
        s.add_starters(data);
        s.tools = vec!["tigerteam".into()];
        s.bank = 10_000;
        s
    }

    /// Tiger Team Pro open for a couple of frames, so it has the keys.
    fn opened(data: &Data, save: &Save) -> (Harness, Home) {
        let (mut h, mut home) = (Harness::new(1280.0, 800.0), Home::new(Vec::new()));
        for _ in 0..2 {
            frame(&mut h, &mut home, data, save, Tool::TigerTeam, Vec::new());
        }
        (h, home)
    }

    /// Presses `events` in Tiger Team Pro, then lets a frame pass. Returns what it sent.
    fn press(
        h: &mut Harness,
        home: &mut Home,
        data: &Data,
        save: &Save,
        events: Vec<egui::Event>,
    ) -> Vec<WbOut> {
        let mut out = frame(h, home, data, save, Tool::TigerTeam, events);
        out.extend(frame(h, home, data, save, Tool::TigerTeam, Vec::new()));
        out
    }

    #[test]
    fn digits_pick_the_section() {
        let data = Data::builtin().unwrap();
        let save = licensed(&data);
        let (mut h, mut home) = opened(&data, &save);
        // Console last: from its empty line, `1` comes back to Assessment.
        for (i, page) in Page::ALL
            .iter()
            .enumerate()
            .skip(1)
            .chain([(0, &Page::ALL[0])])
        {
            press(&mut h, &mut home, &data, &save, text(&(i + 1).to_string()));
            assert_eq!(home.tigerteam.page, *page, "{}", i + 1);
        }
    }

    #[test]
    fn engagements_sort_and_enter_opens_the_assessment() {
        let data = Data::builtin().unwrap();
        let save = licensed(&data);
        let (mut h, mut home) = opened(&data, &save);
        press(&mut h, &mut home, &data, &save, text("2"));
        press(&mut h, &mut home, &data, &save, text("s"));
        assert_eq!(home.tigerteam.order, Order::default().next());
        press(&mut h, &mut home, &data, &save, key(Key::ArrowDown));
        let picked = home.tigerteam.sel.expect("a target is picked");
        press(&mut h, &mut home, &data, &save, key(Key::Enter));
        assert_eq!(home.tigerteam.page, Page::Assessment);
        assert_eq!(home.tigerteam.sel, Some(picked), "on the target picked");
        assert!(
            home.dialing.is_none(),
            "Enter on Engagements assesses, it doesn't dial"
        );
    }

    #[test]
    fn recon_scans_as_tiger_team_pro() {
        let data = Data::builtin().unwrap();
        let save = licensed(&data);
        let (mut h, mut home) = opened(&data, &save);
        press(&mut h, &mut home, &data, &save, text("3"));
        let out = press(&mut h, &mut home, &data, &save, text(" "));
        assert!(
            out.iter()
                .any(|o| matches!(o, WbOut::Scan(_, Some(Tool::TigerTeam)))),
            "the scan comes back to Tiger Team Pro"
        );
        assert_eq!(
            home.nightcomm.range, None,
            "NiGHTCOMM's own pages aren't touched"
        );
    }

    #[test]
    fn the_console_talks_to_the_modem_as_tiger_team_pro() {
        let data = Data::builtin().unwrap();
        let save = licensed(&data);
        let (mut h, mut home) = opened(&data, &save);
        press(&mut h, &mut home, &data, &save, text("5"));
        press(&mut h, &mut home, &data, &save, text("ATI"));
        let out = press(&mut h, &mut home, &data, &save, key(Key::Enter));
        assert!(
            out.iter()
                .any(|o| matches!(o, WbOut::At(Tool::TigerTeam, l) if l == "ATI")),
            "sent as Tiger Team Pro"
        );
        assert!(home.tigerteam.nc.term_lines.iter().any(|l| l == "> ATI"));
        assert_eq!(
            home.nightcomm.term_lines,
            NightComm::default().term_lines,
            "NiGHTCOMM's TERM hears nothing of it"
        );
    }
}
