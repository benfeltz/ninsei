//! System, the deck's settings app (it replaced Prefs), drawn as a System 6-style Control
//! Panel: the panels down the left, the open one on the right. What it reports comes from core
//! (`ninsei_core::system`); every change goes out as a `WbOut` and through `Save`'s methods.

use egui::{RichText, Ui};
use ninsei_core::disk::{self, Disk, Program};
use ninsei_core::system::{self, Panel};
use ninsei_core::{Data, hub::Save};

use crate::home::{WbOut, accent, backdrop, fill, font, ink, muted, shadow};
use crate::theme::Palette;

#[cfg(test)]
#[path = "system_tests.rs"]
mod tests;

/// Liza's Control Panel list, after System 6's: a framed list of icons with their names under
/// them, the scroll bar down its right side (inactive while every panel fits), and the system's
/// version beneath.
mod cp {
    /// The list's width, scroll bar included.
    pub const LIST_W: f32 = 150.0;
    /// Icons draw at twice their pixels: 32x32, the Control Panel's size.
    pub const ICON_SCALE: f32 = 2.0;
    /// One panel's cell: icon, gap, name, gap.
    pub const CELL_H: f32 = 64.0;
    /// Space above the first cell.
    pub const TOP: f32 = 8.0;
    /// Room under the list for the version line.
    pub const VERSION_H: f32 = 22.0;
    /// Space between the list and the open panel, and on the panel's far side.
    pub const GAP: f32 = 12.0;
}
/// A level bar's size: one cell per upgrade level.
const BAR: egui::Vec2 = egui::vec2(96.0, 12.0);

/// System's state between frames: the open panel, and on Disk the picked program and a scrap
/// waiting for its confirmation.
#[derive(Clone, Debug, Default)]
pub struct SystemUi {
    pub panel: Panel,
    /// AnarchyOS: the highlighted row of the open panel (Disk uses `picked`).
    pub cursor: usize,
    /// AnarchyOS: the tile is focused, so its keys are live.
    pub keys: bool,
    pub picked: Option<Disk>,
    pub confirm_scrap: Option<u32>,
    /// Defragment in progress: its frames (from core) and when it started.
    pub defrag: Option<(Vec<Vec<Option<usize>>>, f64)>,
}

/// Defragment's pacing. Display numbers, not balance: a real defrag took time.
pub mod anim {
    /// Seconds per block moved.
    pub const STEP_SECS: f64 = 0.25;
    /// Seconds to read the disk when nothing needs moving (and before the first move).
    pub const READ_SECS: f64 = 0.8;
}

pub fn show(ui: &mut Ui, data: &Data, save: &Save, st: &mut SystemUi, out: &mut Vec<WbOut>) {
    use ninsei_core::hardware::Desktop;
    match save.firmware_def(data).desktop {
        Desktop::Liza => control_panel(ui, data, save, st, out),
        Desktop::Workbench => preferences(ui, data, save, st, out),
        Desktop::Tiling => crate::system_tui::show(ui, data, save, st, out),
    }
}

/// The open panel, in a GUI look.
fn panel_body(
    ui: &mut Ui,
    data: &Data,
    save: &Save,
    look: Look,
    st: &mut SystemUi,
    out: &mut Vec<WbOut>,
) {
    let panel = st.panel;
    ui.vertical(|ui| match panel {
        Panel::Overview => overview(ui, data, save, look, out),
        Panel::Monitor => monitor(ui, data, save, look, out),
        Panel::Theme => theme(ui, data, save, look, out),
        Panel::Disk => disk_panel(ui, data, save, look, st, out),
    });
}

/// Liza: a System 6 Control Panel. The panels in a framed list of icons down the left, the full
/// height of the window with the system's version under it; the open one on the right.
fn control_panel(ui: &mut Ui, data: &Data, save: &Save, st: &mut SystemUi, out: &mut Vec<WbOut>) {
    let fw = save.firmware_def(data);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            let fit = cp::TOP * 2.0 + Panel::ALL.len() as f32 * cp::CELL_H;
            let h = (ui.available_height() - cp::VERSION_H).max(fit);
            panel_list(ui, data, st, h);
            ui.label(dim(&fw.label));
        });
        ui.add_space(cp::GAP);
        ui.vertical(|ui| {
            ui.set_max_width(ui.available_width() - cp::GAP);
            panel_body(ui, data, save, Look::Mac, st, out);
        });
    });
}

/// The Control Panel's list: each panel's icon over its name, the open one inverted, icon and
/// name together, in the kit's list box. Click one to open it; then ↑↓ walk the list.
fn panel_list(ui: &mut Ui, data: &Data, st: &mut SystemUi, height: f32) {
    use egui::{Rect, pos2, vec2};
    let look = crate::kit::liza::Look {
        gray: data.pattern("p50"),
        ..mac()
    };
    let at = Panel::ALL.iter().position(|&p| p == st.panel).unwrap_or(0);
    let picked = crate::kit::liza::list_box(
        ui,
        vec2(cp::LIST_W, height),
        egui::Id::new("system-panel-list"),
        Panel::ALL.len(),
        cp::CELL_H,
        cp::TOP,
        at,
        &look,
        |p, cell, i, on| {
            let panel = Panel::ALL[i];
            let (fg, bg) = if on {
                (look.paper, look.ink)
            } else {
                (look.ink, look.paper)
            };
            // The icon, pixel-aligned, centred at the top of the cell.
            let px = ninsei_core::system::panel_icon(data, panel);
            let side = ninsei_core::system::PANEL_ICON as f32 * cp::ICON_SCALE;
            let art = pos2((cell.center().x - side / 2.0).round(), cell.top().round());
            for (y, row) in px.iter().enumerate() {
                for (x, ch) in row.chars().enumerate() {
                    let col = match ch {
                        'k' => fg,
                        'w' => bg,
                        _ => continue,
                    };
                    let at = art + vec2(x as f32, y as f32) * cp::ICON_SCALE;
                    p.rect_filled(
                        Rect::from_min_size(at, egui::Vec2::splat(cp::ICON_SCALE)),
                        0.0,
                        col,
                    );
                }
            }
            // The name under it, on a solid backing (inverse when open).
            let galley = p.layout_no_wrap(panel.label().to_string(), font(), fg);
            let name = Rect::from_center_size(
                pos2(
                    cell.center().x,
                    art.y + side + 4.0 + galley.size().y / 2.0 + 1.0,
                ),
                galley.size() + vec2(6.0, 2.0),
            );
            p.rect_filled(name, 0.0, bg);
            p.galley(name.min + vec2(3.0, 1.0), galley, fg);
            panel.label().to_string()
        },
    );
    if let Some(i) = picked {
        st.panel = Panel::ALL[i];
    }
}

/// Installed software, two to a row, so a full disk of tools still leaves room for the
/// switches below it.
fn software_grid(ui: &mut Ui, software: &[(String, String)]) {
    egui::Grid::new("system-software")
        .num_columns(2)
        .spacing([24.0, 2.0])
        .show(ui, |ui| {
            for (i, (name, ver)) in software.iter().enumerate() {
                ui.label(RichText::new(format!("{name} {ver}")).color(ink()));
                if i % 2 == 1 {
                    ui.end_row();
                }
            }
        });
}

/// Desktop: Workbench 1.3's Preferences. A row of chunky gadgets across the top picks the
/// panel; the open one fills the window under it. Settings take at once, so there's no
/// Use/Cancel pair: every gadget is a live switch.
fn preferences(ui: &mut Ui, data: &Data, save: &Save, st: &mut SystemUi, out: &mut Vec<WbOut>) {
    let look = crate::workbench::look();
    ui.horizontal(|ui| {
        for p in Panel::ALL {
            if crate::kit::workbench::toggle_gadget(ui, p.label(), st.panel == p, &look).clicked() {
                st.panel = p;
            }
            ui.add_space(4.0);
        }
    });
    ui.add_space(4.0);
    let r = ui.available_rect_before_wrap();
    ui.painter()
        .hline(r.x_range(), r.top(), egui::Stroke::new(2.0, ink()));
    ui.add_space(8.0);
    panel_body(ui, data, save, Look::Amiga, st, out);
}

fn head(ui: &mut Ui, t: &str) {
    ui.label(RichText::new(t).color(ink()).strong().size(16.0));
    ui.add_space(4.0);
}

fn dim(t: impl Into<String>) -> RichText {
    RichText::new(t).color(muted())
}

/// About This Deck: the OS and deck, the specs, the software you own, and the switches.
fn overview(ui: &mut Ui, data: &Data, save: &Save, look: Look, out: &mut Vec<WbOut>) {
    let o = system::overview(save, data);
    head(ui, &o.deck);
    ui.label(RichText::new(format!("{}  ·  {}", o.os, o.firmware)).color(ink()));
    ui.add_space(6.0);

    // Decks you own, as many to a row as fit, so a shelf of models stays on the panel.
    if save.decks.len() > 1 {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 14.0;
            for d in save.decks.iter().filter_map(|id| data.hardware.deck(id)) {
                if radio(ui, look, save.deck == d.id, &d.title())
                    .on_hover_text(&d.blurb)
                    .clicked()
                {
                    out.push(WbOut::Deck(d.id.clone()));
                }
            }
        });
    }
    let deck = save.deck_def(data);
    if deck.firmware.len() > 1 {
        let fw = save.firmware_def(data);
        ui.horizontal_wrapped(|ui| {
            ui.label(dim("Firmware"));
            for f in &deck.firmware {
                if radio(ui, look, fw.id == f.id, &f.label).clicked() {
                    out.push(WbOut::Firmware(f.id.clone()));
                }
            }
        });
    }
    ui.add_space(6.0);

    egui::Grid::new("system-specs")
        .num_columns(4)
        .spacing([14.0, 4.0])
        .show(ui, |ui| {
            for s in &o.specs {
                ui.label(RichText::new(&s.label).color(ink()).strong());
                ui.label(RichText::new(&s.value).color(ink()));
                let (lvl, max) = s.level;
                level_bar(ui, lvl, max).on_hover_text(match &s.next {
                    Some(n) => format!("level {lvl} of {max}. The next upgrade adds {n}."),
                    None => format!("level {lvl} of {max}."),
                });
                ui.label(dim(&s.use_note));
                ui.end_row();
            }
        });
    ui.add_space(8.0);

    ui.label(RichText::new("Installed software").color(ink()).strong());
    if o.software.is_empty() {
        ui.label(dim("None."));
    } else {
        software_grid(ui, &o.software);
    }
    ui.add_space(8.0);

    ui.label(RichText::new("Sound and screen").color(ink()).strong());
    let mut sound = save.sound;
    if check(ui, look, &mut sound, "Sound").changed() {
        out.push(WbOut::Sound);
    }
    let mut intros = save.intros;
    if check(ui, look, &mut intros, "Cracktros")
        .on_hover_text("Programs from the boards open with their crew's intro.")
        .changed()
    {
        out.push(WbOut::Intros(intros));
    }
    ui.horizontal(|ui| {
        ui.label(RichText::new("Damage glitches").color(ink()));
        for g in ninsei_core::glitch::Glitches::ALL {
            if radio(ui, look, save.glitches == g, g.label()).clicked() {
                out.push(WbOut::Glitches(g));
            }
        }
    });
}

/// The monitors you own, the one plugged in, and the CRT.
fn monitor(ui: &mut Ui, data: &Data, save: &Save, look: Look, out: &mut Vec<WbOut>) {
    let m = save.monitor_def(data);
    head(ui, "Monitor");
    for id in &save.monitors {
        let Some(m) = data.hardware.monitor(id) else {
            continue;
        };
        ui.horizontal(|ui| {
            if radio(ui, look, save.monitor == m.id, &m.title())
                .on_hover_text(&m.blurb)
                .clicked()
            {
                out.push(WbOut::Monitor(m.id.clone()));
            }
            ui.label(dim(if m.colour { "colour" } else { "mono" }));
        });
    }
    ui.add_space(4.0);
    ui.label(dim(&m.blurb));
    ui.add_space(6.0);
    let mut crt = save.crt;
    if check(ui, look, &mut crt, "CRT effects").changed() {
        out.push(WbOut::Crt);
    }
    if let Some(t) = m.locked_theme(data) {
        let label = data.theme(t).map_or(t, |d| d.label.as_str());
        ui.add_space(6.0);
        ui.label(dim(format!("Mono tube: shows {label} only.")));
    }
}

/// Liza's controls in her two colours.
fn mac() -> crate::kit::liza::Look<'static> {
    crate::kit::liza::Look {
        ink: ink(),
        paper: backdrop(),
        font: font(),
        stripes: None,
        gray: None,
    }
}

/// A check box: the Mac's on Liza, Workbench's on Desktop.
fn check(ui: &mut Ui, look: Look, on: &mut bool, label: &str) -> egui::Response {
    match look {
        Look::Mac => crate::kit::liza::check_box(ui, on, label, &mac()),
        Look::Amiga => crate::kit::workbench::check_box(ui, on, label, &crate::workbench::look()),
    }
}

/// A push button: the Mac's on Liza, Workbench's on Desktop. Disabled when `ui` is.
fn push(ui: &mut Ui, look: Look, label: &str) -> egui::Response {
    match look {
        Look::Mac => crate::kit::liza::push_button(ui, label, false, &mac()),
        Look::Amiga => crate::kit::workbench::push_button(ui, label, &crate::workbench::look()),
    }
}

/// A radio button: the Mac's on Liza, a Workbench toggle gadget on Desktop.
fn radio(ui: &mut Ui, look: Look, on: bool, label: &str) -> egui::Response {
    match look {
        Look::Mac => crate::kit::liza::radio(ui, on, label, &mac()),
        // 1.3's mutually exclusive choice: a row of toggle gadgets, the chosen one pressed in.
        Look::Amiga => {
            crate::kit::workbench::toggle_gadget(ui, label, on, &crate::workbench::look())
        }
    }
}

/// Every theme with its six colours; locked to one on a mono monitor.
fn theme(ui: &mut Ui, data: &Data, save: &Save, look: Look, out: &mut Vec<WbOut>) {
    head(ui, "Theme");
    for t in &data.themes {
        let blocker = save.theme_blocker(data, &t.id);
        ui.horizontal(|ui| {
            let resp = ui
                .add_enabled_ui(blocker.is_none(), |ui| {
                    radio(ui, look, save.phosphor == t.id, &format!("{:<18}", t.label))
                })
                .inner;
            if let Some(why) = &blocker {
                resp.on_disabled_hover_text(why);
            } else if resp.clicked() {
                out.push(WbOut::Theme(t.id.clone()));
            }
            swatch(ui, t);
        });
    }
}

/// A level as a framed bar like About This Macintosh's memory bars: filled up to the level,
/// with a tick between each level.
fn level_bar(ui: &mut Ui, level: u32, max: u32) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(BAR, egui::Sense::hover());
    let p = ui.painter();
    let max = max.max(1);
    let w = r.width() / max as f32;
    let filled =
        egui::Rect::from_min_size(r.min, egui::vec2(w * level.min(max) as f32, r.height()));
    p.rect_filled(filled, 0.0, accent());
    for i in 1..max {
        let x = (r.left() + w * i as f32).round();
        p.vline(x, r.y_range(), egui::Stroke::new(1.0, ink()));
    }
    p.rect_stroke(
        r,
        0.0,
        egui::Stroke::new(1.0, ink()),
        egui::StrokeKind::Inside,
    );
    resp
}

/// A theme's six roles, darkest to brightest.
fn swatch(ui: &mut Ui, t: &ninsei_core::data::ThemeDef) {
    let p = Palette::from_def(t);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for c in [p.bg, p.panel, p.faint, p.dim, p.fg, p.bright] {
            let (r, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
            ui.painter().rect_filled(r, 0.0, c);
            ui.painter().rect_stroke(
                r,
                0.0,
                egui::Stroke::new(1.0, fill()),
                egui::StrokeKind::Inside,
            );
        }
    });
}

/// Disk: the HDD as a block map (the loadout), the box of disks beside it (the stash and the
/// free programs not installed), the RAM the installed programs will load into, and the picked
/// program's details. After MS-DOS 6 DEFRAG / Norton Speed Disk, drawn 1-bit.
fn disk_panel(
    ui: &mut Ui,
    data: &Data,
    save: &Save,
    look: Look,
    st: &mut SystemUi,
    out: &mut Vec<WbOut>,
) {
    let on = disk::installed(save, data);
    let shelf = disk::shelf(save, data);
    let now = ui.input(|i| i.time);
    let map = defrag_map(ui, save, data, st, out);
    let cap = save.hdd_cap(data) as usize;
    // A pick that's gone (scrapped, lost) is dropped.
    if st
        .picked
        .as_ref()
        .is_some_and(|p| !on.iter().chain(&shelf).any(|x| &x.at == p))
    {
        st.picked = None;
    }
    keys(ui, save, data, st, &on, &shelf, out);

    ui.horizontal(|ui| {
        ui.label(RichText::new("Disk").color(ink()).strong().size(16.0));
        ui.add_space(12.0);
        let busy = st.defrag.is_some();
        let label = if busy {
            "Defragmenting…"
        } else {
            "Defragment"
        };
        if ui
            .add_enabled_ui(!busy, |ui| push(ui, look, label))
            .inner
            .on_hover_text("Lay every program out end to end")
            .clicked()
        {
            st.defrag = Some((disk::defrag_steps(save, data), now));
            st.picked = None;
        }
    });
    ui.label(dim(format!(
        "{} of {cap} slots used  ·  {}% fragmented  ·  {} optimizer fragments",
        on.len(),
        disk::fragmentation(save, data),
        save.fragments
    )));
    ui.add_space(4.0);
    // Top: the drive (map, legend, RAM). Bottom: the box of disks beside the picked
    // program's details and its buttons.
    block_map(ui, data, look, st, &on, &shelf, &map);
    ui.add_space(4.0);
    legend(ui, data, look);
    ui.add_space(6.0);
    ram_strip(ui, save, data, &on, look);
    ui.add_space(8.0);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(BOX_W);
            box_list(ui, save, data, st, &shelf);
        });
        ui.add_space(12.0);
        ui.vertical(|ui| {
            info(ui, data, st, &on, &shelf);
            ui.add_space(6.0);
            actions(ui, save, data, look, st, out);
        });
    });
}

/// The map to show: while Defragment runs, its frame for now (and, when it ends, the
/// `WbOut::Defragment` that compacts the save); otherwise the disk as it lies.
pub(crate) fn defrag_map(
    ui: &Ui,
    save: &Save,
    data: &Data,
    st: &mut SystemUi,
    out: &mut Vec<WbOut>,
) -> Vec<Option<usize>> {
    let now = ui.input(|i| i.time);
    let mut map = disk::map(save, data);
    if let Some((frames, start)) = &st.defrag {
        let t = now - start - anim::READ_SECS;
        let step = if t < 0.0 {
            0
        } else {
            (t / anim::STEP_SECS) as usize + 1
        };
        if step >= frames.len() && t >= 0.0 {
            out.push(WbOut::Defragment);
            st.defrag = None;
        } else {
            map = frames[step.min(frames.len() - 1)].clone();
            ui.ctx().request_repaint();
        }
    }
    map
}

/// Map geometry: blocks per row and each block's size.
const MAP_COLS: usize = 16;
const BLOCK: f32 = 24.0;
/// The box of disks' width.
const BOX_W: f32 = 180.0;

/// Which desktop's look System is wearing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    /// Liza: a System 6 Control Panel, 1-bit patterns.
    Mac,
    /// Desktop: Workbench 1.3 Preferences, solid theme colours.
    Amiga,
}

/// Fills a block (or a legend swatch, or a RAM segment) for a kind of program: a 1-bit pattern
/// on Liza, a solid theme colour on Desktop (like an Amiga disk optimizer's map).
fn paint_kind(
    p: &egui::Painter,
    data: &Data,
    look: Look,
    kind: ninsei_core::WarezKind,
    r: egui::Rect,
) {
    use ninsei_core::WarezKind;
    match look {
        Look::Mac => match data.disk.fill(kind).and_then(|id| data.pattern(id)) {
            Some(pat) => crate::patterns::fill(p, pat, r, ink(), crate::home::backdrop()),
            None => {
                p.rect_filled(r, 0.0, muted());
            }
        },
        Look::Amiga => {
            let c = match kind {
                WarezKind::Breaker => accent(),
                WarezKind::Shield => ink(),
                WarezKind::Utility => fill(),
            };
            p.rect_filled(r, 0.0, c);
        }
    }
}

/// The block map. Click a program's blocks to pick it. A picked program in the box shows the
/// blocks it would take, dashed, after the last program; hatched past the end if it won't fit.
fn block_map(
    ui: &mut Ui,
    data: &Data,
    look: Look,
    st: &mut SystemUi,
    on: &[Program],
    shelf: &[Program],
    map: &[Option<usize>],
) {
    let rows = map.len().div_ceil(MAP_COLS);
    let size = egui::vec2(MAP_COLS as f32 * BLOCK, rows as f32 * BLOCK);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Disk map"));
    let p = ui.painter_at(rect.expand(BLOCK * 2.0));
    let cell = |i: usize| {
        let (c, r) = (i % MAP_COLS, i / MAP_COLS);
        egui::Rect::from_min_size(
            rect.min + egui::vec2(c as f32 * BLOCK, r as f32 * BLOCK),
            egui::vec2(BLOCK, BLOCK),
        )
        .shrink(1.0)
    };
    let picked_on = st
        .picked
        .as_ref()
        .and_then(|pk| on.iter().position(|x| &x.at == pk));
    let mut first = vec![true; on.len()];
    for (i, b) in map.iter().enumerate() {
        let r = cell(i);
        match b {
            Some(n) => {
                let prog = &on[*n];
                paint_kind(&p, data, look, prog.kind, r);
                if first[*n] {
                    first[*n] = false;
                    // The program's initial on the first block of its run, on a paper chip.
                    let chip = egui::Rect::from_center_size(r.center(), egui::vec2(11.0, 13.0));
                    p.rect_filled(chip, 0.0, crate::home::backdrop());
                    p.text(
                        r.center(),
                        egui::Align2::CENTER_CENTER,
                        prog.spec.name.chars().next().unwrap_or('?'),
                        egui::FontId::monospace(11.0),
                        ink(),
                    );
                }
                let w = if picked_on == Some(*n) { 2.5 } else { 1.0 };
                p.rect_stroke(
                    r,
                    0.0,
                    egui::Stroke::new(w, ink()),
                    egui::StrokeKind::Inside,
                );
            }
            None => {
                p.rect_stroke(
                    r,
                    0.0,
                    egui::Stroke::new(1.0, muted()),
                    egui::StrokeKind::Inside,
                );
            }
        }
    }
    // A picked program from the box: where it would go.
    if let Some(prog) = st
        .picked
        .as_ref()
        .and_then(|pk| shelf.iter().find(|x| &x.at == pk))
    {
        let used = map.iter().filter(|b| b.is_some()).count();
        let slots_full = on.len() >= map.len() / disk::BLOCKS_PER_SLOT;
        for k in 0..prog.blocks() {
            let i = used + k;
            let r = if i < map.len() && !slots_full {
                cell(i)
            } else {
                // Past the end: hatched beside the map.
                let over = i.saturating_sub(map.len().min(used));
                let col = MAP_COLS + over % 2;
                let row = rows.saturating_sub(1);
                egui::Rect::from_min_size(
                    rect.min + egui::vec2(col as f32 * BLOCK + 4.0, row as f32 * BLOCK),
                    egui::vec2(BLOCK, BLOCK),
                )
                .shrink(1.0)
            };
            if (slots_full || i >= map.len())
                && let Some(pat) = data.pattern("diagonal")
            {
                crate::patterns::fill(&p, pat, r, ink(), crate::home::backdrop());
            }
            dashed(&p, r);
        }
    }
    if resp.clicked()
        && let Some(pos) = resp.interact_pointer_pos()
    {
        let v = pos - rect.min;
        let i = (v.y / BLOCK) as usize * MAP_COLS + (v.x / BLOCK) as usize;
        st.picked = map.get(i).copied().flatten().map(|n| on[n].at.clone());
    }
}

/// A dashed outline, for blocks a program would take.
fn dashed(p: &egui::Painter, r: egui::Rect) {
    let s = egui::Stroke::new(1.0, ink());
    for (a, b) in [
        (r.left_top(), r.right_top()),
        (r.right_top(), r.right_bottom()),
        (r.right_bottom(), r.left_bottom()),
        (r.left_bottom(), r.left_top()),
    ] {
        p.add(egui::Shape::dashed_line(&[a, b], s, 3.0, 2.0));
    }
}

/// Which pattern means which kind of program.
fn legend(ui: &mut Ui, data: &Data, look: Look) {
    ui.horizontal(|ui| {
        for (kind, label) in [
            (ninsei_core::WarezKind::Breaker, "Breaker"),
            (ninsei_core::WarezKind::Shield, "Shield"),
            (ninsei_core::WarezKind::Utility, "Utility"),
        ] {
            let (r, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
            paint_kind(ui.painter(), data, look, kind, r);
            ui.painter().rect_stroke(
                r,
                0.0,
                egui::Stroke::new(1.0, ink()),
                egui::StrokeKind::Inside,
            );
            ui.label(dim(label));
            ui.add_space(6.0);
        }
        let (r, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
        ui.painter().rect_stroke(
            r,
            0.0,
            egui::Stroke::new(1.0, muted()),
            egui::StrokeKind::Inside,
        );
        ui.label(dim("Free"));
    });
}

/// RAM at jack-in: the installed programs that load, in order, then the ones that won't fit.
fn ram_strip(ui: &mut Ui, save: &Save, data: &Data, on: &[Program], look: Look) {
    let (strip, ram_max) = disk::ram_preview(save, data);
    let used: u32 = strip.iter().filter(|r| r.loads).map(|r| r.ram).sum();
    ui.label(RichText::new(format!("RAM at jack-in: {used} of {ram_max} GB")).color(ink()));
    let w = MAP_COLS as f32 * BLOCK;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 18.0), egui::Sense::hover());
    let p = ui.painter();
    let per = w / ram_max.max(1) as f32;
    let mut x = rect.left();
    for r in strip.iter().filter(|r| r.loads) {
        let seg = egui::Rect::from_min_size(
            egui::pos2(x, rect.top()),
            egui::vec2(per * r.ram as f32, rect.height()),
        );
        paint_kind(p, data, look, on[r.program].kind, seg.shrink(1.0));
        p.rect_stroke(
            seg,
            0.0,
            egui::Stroke::new(1.0, ink()),
            egui::StrokeKind::Inside,
        );
        x += per * r.ram as f32;
    }
    p.rect_stroke(
        rect,
        0.0,
        egui::Stroke::new(1.0, ink()),
        egui::StrokeKind::Inside,
    );
    let wont: Vec<String> = strip
        .iter()
        .filter(|r| !r.loads)
        .map(|r| on[r.program].spec.file())
        .collect();
    if !wont.is_empty() {
        ui.label(dim(format!(
            "Won't fit in RAM, stays on disk: {}",
            wont.join(", ")
        )));
    }
}

/// The box of disks beside the drive: stash programs and free programs not installed.
fn box_list(ui: &mut Ui, save: &Save, data: &Data, st: &mut SystemUi, shelf: &[Program]) {
    let cap = save.stash_cap(&data.rig);
    ui.label(
        RichText::new(format!("Stash  {} of {cap}", save.stash.len()))
            .color(ink())
            .strong(),
    );
    egui::Frame::new()
        .stroke(egui::Stroke::new(1.0, ink()))
        .inner_margin(4.0)
        .show(ui, |ui| {
            ui.set_width(BOX_W - 10.0);
            ui.set_min_height(90.0);
            if shelf.is_empty() {
                ui.label(dim("Empty."));
            }
            for prog in shelf {
                let picked = st.picked.as_ref() == Some(&prog.at);
                let tag = match prog.at {
                    Disk::Free(_) => " (free)",
                    Disk::Stash(_) => "",
                };
                let text = RichText::new(format!("{}{tag}", prog.spec.file())).color(if picked {
                    shadow()
                } else {
                    ink()
                });
                if ui.selectable_label(picked, text).clicked() {
                    st.picked = Some(prog.at.clone());
                }
            }
        });
}

/// Install, Remove and Scrap for the picked program, with scrap's confirmation.
fn actions(
    ui: &mut Ui,
    save: &Save,
    data: &Data,
    look: Look,
    st: &mut SystemUi,
    out: &mut Vec<WbOut>,
) {
    let Some(at) = st.picked.clone() else {
        ui.label(dim("Pick a program."));
        return;
    };
    if let Some(id) = st.confirm_scrap {
        ui.label(RichText::new("Scrap it for good?").color(ink()).strong());
        ui.horizontal(|ui| {
            if push(ui, look, "Cancel").clicked() {
                st.confirm_scrap = None;
            }
            if push(ui, look, "Scrap").clicked() {
                out.push(WbOut::Scrap(id));
                st.confirm_scrap = None;
                st.picked = None;
            }
        });
        return;
    }
    ui.horizontal(|ui| {
        if save.is_installed(&at) {
            if push(ui, look, "◂ Remove")
                .on_hover_text("Back into the box (Enter)")
                .clicked()
            {
                out.push(WbOut::Remove(at.clone()));
            }
        } else {
            let blocker = save.install_blocker(data, &at);
            let resp = ui
                .add_enabled_ui(blocker.is_none(), |ui| push(ui, look, "Install ▸"))
                .inner;
            match blocker {
                Some(why) => {
                    resp.on_disabled_hover_text(why);
                }
                None => {
                    if resp
                        .on_hover_text("Onto the disk: it's in the next run (Enter)")
                        .clicked()
                    {
                        out.push(WbOut::Install(at.clone()));
                    }
                }
            }
        }
        if let Disk::Stash(id) = at
            && push(ui, look, "Scrap…")
                .on_hover_text("Delete it for good (Del)")
                .clicked()
        {
            st.confirm_scrap = Some(id);
        }
    });
}

/// The picked program, from its binary.
fn info(ui: &mut Ui, data: &Data, st: &SystemUi, on: &[Program], shelf: &[Program]) {
    let Some(prog) = st
        .picked
        .as_ref()
        .and_then(|pk| on.iter().chain(shelf).find(|x| &x.at == pk))
    else {
        return;
    };
    let s = &prog.spec;
    ui.label(RichText::new(s.file()).color(ink()).strong());
    ui.label(dim(&data.warez[prog.warez].blurb));
    ui.label(RichText::new(s.effect.describe()).color(ink()));
    let mut line = format!("{:?} · RAM {} · CPU {}", prog.kind, s.ram, s.cpu);
    if let Some(pw) = prog.power() {
        line.push_str(&format!(" · power {pw}"));
    }
    if s.opt > 0 {
        line.push_str(&format!(" · opt {}", s.opt));
    }
    ui.label(dim(line));
}

/// Keys, while the pointer is over the panel and nothing else is typing: ↑↓ move the pick
/// through the disk then the box, Enter installs or removes it, Delete asks to scrap it.
fn keys(
    ui: &mut Ui,
    save: &Save,
    data: &Data,
    st: &mut SystemUi,
    on: &[Program],
    shelf: &[Program],
    out: &mut Vec<WbOut>,
) {
    // The keys belong to whatever has the focus (a text field, the panel list), not to Disk.
    if !ui.rect_contains_pointer(ui.max_rect()) || ui.memory(|m| m.focused().is_some()) {
        return;
    }
    let all: Vec<&Disk> = on.iter().chain(shelf).map(|p| &p.at).collect();
    if all.is_empty() {
        return;
    }
    let at = st
        .picked
        .as_ref()
        .and_then(|p| all.iter().position(|x| *x == p));
    ui.input(|i| {
        use egui::Key;
        if i.key_pressed(Key::ArrowDown) {
            let n = at.map_or(0, |a| (a + 1).min(all.len() - 1));
            st.picked = Some(all[n].clone());
        } else if i.key_pressed(Key::ArrowUp) {
            let n = at.map_or(0, |a| a.saturating_sub(1));
            st.picked = Some(all[n].clone());
        } else if i.key_pressed(Key::Enter)
            && let Some(p) = st.picked.clone()
        {
            if save.is_installed(&p) {
                out.push(WbOut::Remove(p));
            } else if save.install_blocker(data, &p).is_none() {
                out.push(WbOut::Install(p));
            }
        } else if i.key_pressed(Key::Delete)
            && let Some(Disk::Stash(id)) = st.picked
        {
            st.confirm_scrap = Some(id);
        }
    });
}
