//! System, the deck's settings app (it replaced Prefs), drawn as a System 1-style Control
//! Panel: the panels down the left, the open one on the right. What it reports comes from core
//! (`ninsei_core::system`); every change goes out as a `WbOut` and through `Save`'s methods.

use egui::{RichText, Ui};
use ninsei_core::system::{self, Panel};
use ninsei_core::{Data, hub::Save};

use crate::home::{WbOut, accent, fill, ink, muted, shadow};
use crate::theme::Palette;

/// Room the panel list takes on the left, and its height.
const LIST_W: f32 = 150.0;
const LIST_H: f32 = 300.0;
/// A level bar's size: one cell per upgrade level.
const BAR: egui::Vec2 = egui::vec2(96.0, 12.0);

pub fn show(ui: &mut Ui, data: &Data, save: &Save, panel: &mut Panel, out: &mut Vec<WbOut>) {
    let fw = save.firmware_def(data);
    ui.horizontal_top(|ui| {
        // The panel list, framed like the Control Panel's, with the system's version under it.
        ui.vertical(|ui| {
            ui.set_width(LIST_W);
            egui::Frame::new()
                .stroke(egui::Stroke::new(1.0, ink()))
                .inner_margin(4.0)
                .show(ui, |ui| {
                    ui.set_width(LIST_W - 10.0);
                    ui.set_min_height(LIST_H);
                    for p in Panel::ALL {
                        let on = *panel == p;
                        let text =
                            RichText::new(p.label()).color(if on { shadow() } else { ink() });
                        if ui.selectable_label(on, text).clicked() {
                            *panel = p;
                        }
                    }
                });
            ui.label(dim(&fw.label));
        });
        ui.add_space(8.0);
        ui.vertical(|ui| match panel {
            Panel::Overview => overview(ui, data, save, out),
            Panel::Monitor => monitor(ui, data, save, out),
            Panel::Theme => theme(ui, data, save, out),
        });
    });
}

fn head(ui: &mut Ui, t: &str) {
    ui.label(RichText::new(t).color(ink()).strong().size(16.0));
    ui.add_space(4.0);
}

fn dim(t: impl Into<String>) -> RichText {
    RichText::new(t).color(muted())
}

/// About This Deck: the OS and deck, the specs, the software you own, and the switches.
fn overview(ui: &mut Ui, data: &Data, save: &Save, out: &mut Vec<WbOut>) {
    let o = system::overview(save, data);
    head(ui, &o.deck);
    ui.label(RichText::new(format!("{}  ·  {}", o.os, o.firmware)).color(ink()));
    ui.add_space(6.0);

    // Decks you own, and the firmware of the one plugged in.
    // Decks you own, one to a line, like the Monitor panel's list.
    if save.decks.len() > 1 {
        for id in &save.decks {
            if let Some(d) = data.hardware.deck(id)
                && ui
                    .radio(save.deck == d.id, d.title())
                    .on_hover_text(&d.blurb)
                    .clicked()
            {
                out.push(WbOut::Deck(d.id.clone()));
            }
        }
    }
    let deck = save.deck_def(data);
    if deck.firmware.len() > 1 {
        let fw = save.firmware_def(data);
        ui.horizontal_wrapped(|ui| {
            ui.label(dim("Firmware"));
            for f in &deck.firmware {
                if ui.radio(fw.id == f.id, &f.label).clicked() {
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
    }
    for (name, ver) in &o.software {
        ui.label(RichText::new(format!("{name} {ver}")).color(ink()));
    }
    ui.add_space(8.0);

    ui.label(RichText::new("Sound and screen").color(ink()).strong());
    let mut sound = save.sound;
    if ui.checkbox(&mut sound, "Sound").changed() {
        out.push(WbOut::Sound);
    }
    let mut intros = save.intros;
    if ui
        .checkbox(&mut intros, "Cracktros")
        .on_hover_text("Programs from the boards open with their crew's intro.")
        .changed()
    {
        out.push(WbOut::Intros(intros));
    }
    ui.horizontal(|ui| {
        ui.label(RichText::new("Damage glitches").color(ink()));
        for g in ninsei_core::glitch::Glitches::ALL {
            if ui.radio(save.glitches == g, g.label()).clicked() {
                out.push(WbOut::Glitches(g));
            }
        }
    });
}

/// The monitors you own, the one plugged in, and the CRT.
fn monitor(ui: &mut Ui, data: &Data, save: &Save, out: &mut Vec<WbOut>) {
    let m = save.monitor_def(data);
    head(ui, &m.title());
    ui.label(dim(&m.blurb));
    ui.add_space(6.0);
    for id in &save.monitors {
        let Some(m) = data.hardware.monitor(id) else {
            continue;
        };
        ui.horizontal(|ui| {
            if ui
                .radio(save.monitor == m.id, m.title())
                .on_hover_text(&m.blurb)
                .clicked()
            {
                out.push(WbOut::Monitor(m.id.clone()));
            }
            ui.label(dim(if m.colour { "colour" } else { "mono" }));
        });
    }
    ui.add_space(6.0);
    let mut crt = save.crt;
    if ui.checkbox(&mut crt, "CRT effects").changed() {
        out.push(WbOut::Crt);
    }
    if let Some(t) = m.locked_theme(data) {
        let label = data.theme(t).map_or(t, |d| d.label.as_str());
        ui.add_space(6.0);
        ui.label(dim(format!("Mono tube: shows {label} only.")));
    }
}

/// Every theme with its six colours; locked to one on a mono monitor.
fn theme(ui: &mut Ui, data: &Data, save: &Save, out: &mut Vec<WbOut>) {
    head(ui, "Theme");
    for t in &data.themes {
        let blocker = save.theme_blocker(data, &t.id);
        ui.horizontal(|ui| {
            let resp = ui
                .add_enabled_ui(blocker.is_none(), |ui| {
                    ui.radio(save.phosphor == t.id, format!("{:<18}", t.label))
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
