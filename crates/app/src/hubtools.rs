//! The hub's tool windows: RIG (deck shop), STASH (programs and loadout) and HexEd.
//! They draw from the save and report what you clicked as [`WbOut`]s; the app applies those
//! through `Save`'s methods in core.

use egui::{Color32, RichText, Ui, Visuals};
use ninsei_core::hexedit::HexBuffer;
use ninsei_core::hub::{Save, Upgrade};
use ninsei_core::{Data, binary, tuning};

use crate::hexview::{self, HexCtx, HexOut};
use crate::theme::Palette;
use crate::workbench::{BLACK, BLUE, ORANGE, WHITE, WbOut};

/// Restyles egui widgets in the 1.3 palette: blue fields, white ink, orange when pressed.
pub fn wb_visuals(v: &mut Visuals) {
    let w = &mut v.widgets;
    for wv in [&mut w.noninteractive, &mut w.inactive] {
        wv.bg_fill = BLUE;
        wv.weak_bg_fill = BLUE;
        wv.bg_stroke = egui::Stroke::new(1.0, WHITE);
        wv.fg_stroke = egui::Stroke::new(1.0, WHITE);
    }
    for wv in [&mut w.hovered, &mut w.active, &mut w.open] {
        wv.bg_fill = ORANGE;
        wv.weak_bg_fill = ORANGE;
        wv.bg_stroke = egui::Stroke::new(1.0, WHITE);
        wv.fg_stroke = egui::Stroke::new(1.0, BLACK);
    }
    v.selection.bg_fill = ORANGE;
    v.selection.stroke = egui::Stroke::new(1.0, BLACK);
    v.extreme_bg_color = BLUE;
    v.text_edit_bg_color = Some(BLUE);
    v.faint_bg_color = BLUE;
    v.code_bg_color = BLUE;
    v.hyperlink_color = ORANGE;
    v.warn_fg_color = ORANGE;
    v.error_fg_color = ORANGE;
    v.window_fill = BLUE;
    v.panel_fill = BLUE;
}

/// The hex editor's palette at home: the same four colours.
fn wb_palette() -> Palette {
    Palette {
        bg: BLUE,
        panel: BLUE,
        faint: BLACK,
        dim: WHITE,
        fg: WHITE,
        bright: ORANGE,
    }
}

fn dim(text: impl Into<String>) -> RichText {
    RichText::new(text).color(Color32::from_rgb(0xAA, 0xCC, 0xFF))
}

/// RIG: buy permanent deck upgrades with banked credits.
pub fn rig(ui: &mut Ui, data: &Data, save: &Save, out: &mut Vec<WbOut>) {
    let bonus = |u| save.rig.bonus(u, &data.rig);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("bank {}¢", save.bank))
                .color(ORANGE)
                .strong(),
        );
        ui.label(dim(format!(
            "next run: CPU {}  RAM {}  HDD {}  INTEG {}  STASH {}",
            tuning::START_CPU + bonus(Upgrade::Cpu),
            tuning::START_RAM + bonus(Upgrade::Ram),
            tuning::START_HDD + bonus(Upgrade::Hdd),
            tuning::START_INTEGRITY + bonus(Upgrade::Integrity),
            save.stash_cap(&data.rig),
        )));
    });
    ui.add_space(6.0);
    egui::Grid::new("rig")
        .num_columns(5)
        .spacing([14.0, 6.0])
        .show(ui, |ui| {
            for def in &data.rig {
                let lvl = save.rig.level(def.kind);
                let pips: String = (0..def.max)
                    .map(|i| if i < lvl { '█' } else { '░' })
                    .collect();
                ui.label(RichText::new(&def.label).color(WHITE).strong());
                ui.label(RichText::new(format!("+{} {}", def.step, def.unit)).color(WHITE));
                ui.label(RichText::new(pips).color(ORANGE))
                    .on_hover_text(&def.blurb);
                match def.cost(lvl) {
                    Some(c) => {
                        ui.label(RichText::new(format!("{c}¢")).color(if save.bank >= c {
                            WHITE
                        } else {
                            ORANGE
                        }));
                        if ui
                            .add_enabled(save.bank >= c, egui::Button::new(" Buy "))
                            .clicked()
                        {
                            out.push(WbOut::Buy(def.kind));
                        }
                    }
                    None => {
                        ui.label(dim("maxed"));
                        ui.label("");
                    }
                }
                ui.end_row();
            }
        });
    ui.add_space(6.0);
    ui.label(dim(
        "Credits come home when you reach the exit. Flatline and only the dead man's switch pays.",
    ));
}

/// STASH: programs kept between runs, and what goes into the next one.
pub fn stash(ui: &mut Ui, data: &Data, save: &Save, out: &mut Vec<WbOut>, edit: &mut Option<u32>) {
    let cap = save.stash_cap(&data.rig);
    let hdd = save.hdd_cap(&data.rig);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("stash {}/{cap}", save.stash.len()))
                .color(ORANGE)
                .strong(),
        );
        ui.label(dim(format!(
            "loadout {}/{hdd} HDD   frags {}   bank {}¢",
            save.loadout_size(),
            save.fragments,
            save.bank
        )));
    });
    ui.add_space(4.0);
    ui.label(RichText::new("KEPT PROGRAMS").color(WHITE).strong());
    if save.stash.is_empty() {
        ui.label(dim("  empty. reach an exit and keep what you bring out."));
    }
    egui::ScrollArea::vertical()
        .max_height(150.0)
        .show(ui, |ui| {
            for item in &save.stash {
                let spec = item.bin().and_then(|b| binary::check(&b, save.key).ok());
                ui.horizontal(|ui| {
                    let mut take = save.loadout.stash.contains(&item.id);
                    if ui.checkbox(&mut take, "take").changed() {
                        out.push(WbOut::ToggleStash(item.id));
                    }
                    match &spec {
                        Some(s) => {
                            let opt = if s.opt > 0 {
                                format!(" O{}", s.opt)
                            } else {
                                String::new()
                            };
                            ui.label(
                                RichText::new(format!("{:<14}{opt}", s.file()))
                                    .color(WHITE)
                                    .monospace(),
                            );
                            ui.label(dim(format!(
                                "{}  c{} r{}",
                                s.effect.describe(),
                                s.cpu,
                                s.ram
                            )));
                        }
                        None => {
                            ui.label(
                                RichText::new(format!("item {} (corrupt)", item.id)).color(ORANGE),
                            );
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button("scrap")
                            .on_hover_text("delete it for good")
                            .clicked()
                        {
                            out.push(WbOut::Scrap(item.id));
                        }
                        if ui.small_button("hex").clicked() {
                            *edit = Some(item.id);
                        }
                    });
                });
            }
        });
    ui.add_space(4.0);
    ui.label(
        RichText::new("STARTERS  (always free, never at risk)")
            .color(WHITE)
            .strong(),
    );
    ui.horizontal_wrapped(|ui| {
        for id in tuning::START_WAREZ {
            let mut on = save.loadout.starters.iter().any(|s| s == id);
            let name = data
                .warez_ix(id)
                .map_or(id.to_string(), |w| format!("{}.exe", data.warez[w].id));
            if ui.checkbox(&mut on, name).changed() {
                out.push(WbOut::ToggleStarter(id.to_string()));
            }
        }
    });
    ui.add_space(6.0);
    ui.label(dim(
        "Stash programs you take leave the stash. They come home only if you do.",
    ));
}

/// HexEd at home: pick a stash program, then edit it with no time pressure.
pub fn hexed(
    ui: &mut Ui,
    save: &Save,
    hex: &mut Option<(u32, HexBuffer)>,
    out: &mut Vec<WbOut>,
    edit: &mut Option<u32>,
) {
    let Some((id, h)) = hex.as_mut() else {
        ui.label(RichText::new("HexEd: no file open").color(ORANGE).strong());
        ui.label(dim("Programs in your STASH can be edited here. Pick one:"));
        ui.add_space(4.0);
        for item in &save.stash {
            let name = item
                .bin()
                .and_then(|b| binary::check(&b, save.key).ok())
                .map_or(format!("item {}", item.id), |s| s.file());
            if ui.button(format!(" STASH:{name} ")).clicked() {
                *edit = Some(item.id);
            }
        }
        if save.stash.is_empty() {
            ui.label(dim("(the stash is empty)"));
        }
        return;
    };
    let id = *id;
    let spec = binary::check(&h.orig, save.key).ok();
    let cx = HexCtx {
        file: format!("STASH:{}", spec.as_ref().map_or("?".into(), |s| s.file())),
        key: save.key,
        fragments: save.fragments,
        opt: spec.map_or(0, |s| s.opt),
        locked: None,
    };
    let mut close = false;
    for o in hexview::editor(ui, h, &cx, &wb_palette()) {
        match o {
            HexOut::Write(bin) => out.push(WbOut::Patch(id, bin)),
            HexOut::Optimize => out.push(WbOut::Optimize(id)),
            HexOut::Close => close = true,
        }
    }
    if close {
        *hex = None;
    }
}
