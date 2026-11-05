//! The hub's tool windows: CarrierTerm (phonebook), RIG (deck shop), STASH (programs and
//! loadout) and HexEd.
//! They draw from the save and report what you clicked as [`WbOut`]s; the app applies those
//! through `Save`'s methods in core.

use egui::{RichText, Ui, Visuals};
use ninsei_core::hexedit::HexBuffer;
use ninsei_core::hub::{Answer, Save, ScanResult, Upgrade};
use ninsei_core::modem::Handshake;
use ninsei_core::tuning::HEAT_MAX;
use ninsei_core::{Data, binary, tuning};

use crate::hexview::{self, HexCtx, HexOut};
use crate::home::{Dialing, WbOut, accent, backdrop, fill, ink, muted, shadow};
use crate::theme::Palette;

/// Restyles egui widgets in the 1.3 palette: blue fields, white ink, orange when pressed.
pub fn wb_visuals(v: &mut Visuals) {
    let w = &mut v.widgets;
    for wv in [&mut w.noninteractive, &mut w.inactive] {
        wv.bg_fill = backdrop();
        wv.weak_bg_fill = backdrop();
        wv.bg_stroke = egui::Stroke::new(1.0, ink());
        wv.fg_stroke = egui::Stroke::new(1.0, ink());
    }
    // Hover: an accent outline with accent text, readable on any theme.
    for wv in [&mut w.hovered, &mut w.open] {
        wv.bg_fill = backdrop();
        wv.weak_bg_fill = backdrop();
        wv.bg_stroke = egui::Stroke::new(1.0, accent());
        wv.fg_stroke = egui::Stroke::new(1.0, accent());
    }
    // Pressed: inverse video.
    w.active.bg_fill = accent();
    w.active.weak_bg_fill = accent();
    w.active.bg_stroke = egui::Stroke::new(1.0, ink());
    w.active.fg_stroke = egui::Stroke::new(1.0, shadow());
    v.selection.bg_fill = accent();
    v.selection.stroke = egui::Stroke::new(1.0, shadow());
    v.extreme_bg_color = backdrop();
    v.text_edit_bg_color = Some(backdrop());
    v.faint_bg_color = backdrop();
    v.code_bg_color = backdrop();
    v.hyperlink_color = accent();
    v.warn_fg_color = accent();
    v.error_fg_color = accent();
    v.window_fill = backdrop();
    v.panel_fill = backdrop();
}

/// The hex editor's palette at home: the same four colours.
fn wb_palette() -> Palette {
    Palette {
        bg: backdrop(),
        panel: backdrop(),
        faint: shadow(),
        dim: muted(),
        fg: ink(),
        bright: accent(),
    }
}

fn dim(text: impl Into<String>) -> RichText {
    RichText::new(text).color(muted())
}

/// RIG: buy permanent deck upgrades with banked credits.
pub fn rig(ui: &mut Ui, data: &Data, save: &Save, out: &mut Vec<WbOut>) {
    let bonus = |u| save.rig.bonus(u, &data.rig);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("bank {}¢", save.bank))
                .color(accent())
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
                ui.label(RichText::new(&def.label).color(ink()).strong());
                ui.label(RichText::new(format!("+{} {}", def.step, def.unit)).color(ink()));
                ui.label(RichText::new(pips).color(accent()))
                    .on_hover_text(&def.blurb);
                match def.cost(lvl) {
                    Some(c) => {
                        ui.label(RichText::new(format!("{c}¢")).color(if save.bank >= c {
                            ink()
                        } else {
                            accent()
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
    ui.add_space(10.0);
    hardware_shop(ui, data, save, out);
    ui.add_space(10.0);
    pane_shop(ui, data, save, out);
    ui.add_space(4.0);
    addon_shop(ui, data, save, out);
    ui.add_space(6.0);
    ui.label(dim(
        "Credits come home when you reach the exit. Flatline and only the dead man's switch pays.",
    ));
}

/// RIG's HARDWARE section: monitors and decks you don't own yet.
fn hardware_shop(ui: &mut Ui, data: &Data, save: &Save, out: &mut Vec<WbOut>) {
    use ninsei_core::hardware::Hardware;
    ui.label(RichText::new("HARDWARE").color(ink()).strong());
    let mut items: Vec<(Hardware, String, &str, String, u32)> = Vec::new();
    for m in &data.hardware.monitors {
        let kind = if m.colour {
            "colour monitor"
        } else {
            "mono monitor"
        };
        items.push((
            Hardware::Monitor(m.id.clone()),
            m.title(),
            kind,
            m.blurb.clone(),
            m.price,
        ));
    }
    for d in &data.hardware.decks {
        items.push((
            Hardware::Deck(d.id.clone()),
            d.title(),
            "deck",
            d.blurb.clone(),
            d.price,
        ));
    }
    items.retain(|(hw, ..)| !save.owns(hw));
    if items.is_empty() {
        ui.label(dim("You own everything on the shelf."));
        return;
    }
    egui::Grid::new("hardware")
        .num_columns(4)
        .spacing([14.0, 6.0])
        .show(ui, |ui| {
            for (hw, title, kind, blurb, price) in items {
                ui.label(RichText::new(title).color(ink()).strong())
                    .on_hover_text(blurb);
                ui.label(dim(kind));
                let can = save.bank >= price;
                ui.label(RichText::new(format!("{price}¢")).color(if can {
                    ink()
                } else {
                    accent()
                }));
                if ui.add_enabled(can, egui::Button::new(" Buy ")).clicked() {
                    out.push(WbOut::BuyHardware(hw));
                }
                ui.end_row();
            }
        });
    ui.label(dim("New hardware plugs straight in. Swap in Prefs."));
}

/// RIG's PANES section: extra windows for the run screen. Panes that came with a firmware
/// you own are listed as bundled.
fn pane_shop(ui: &mut Ui, data: &Data, save: &Save, out: &mut Vec<WbOut>) {
    ui.label(RichText::new("RUN PANES").color(ink()).strong());
    let bundled = |id: &str| {
        save.decks
            .iter()
            .filter_map(|d| data.hardware.deck(d))
            .flat_map(|d| d.firmware.iter().map(move |f| (d, f)))
            .find(|(_, f)| f.panes.iter().any(|p| p == id))
            .map(|(d, f)| format!("free on {} {}", d.name, f.id))
    };
    egui::Grid::new("panes")
        .num_columns(4)
        .spacing([14.0, 6.0])
        .show(ui, |ui| {
            for d in &data.panes {
                ui.label(RichText::new(&d.name).color(ink()).strong())
                    .on_hover_text(&d.blurb);
                let owned = save.panes.iter().any(|p| p == &d.id);
                ui.label(dim(bundled(&d.id).unwrap_or_default()));
                if owned {
                    ui.label(dim("owned"));
                    ui.label("");
                } else {
                    let can = save.bank >= d.price;
                    ui.label(RichText::new(format!("{}¢", d.price)).color(if can {
                        ink()
                    } else {
                        accent()
                    }));
                    if ui.add_enabled(can, egui::Button::new(" Buy ")).clicked() {
                        out.push(WbOut::BuyPane(d.id.clone()));
                    }
                }
                ui.end_row();
            }
        });
    ui.label(dim("Add a pane in a run from the view menu."));
    ui.add_space(6.0);
}

/// RIG's ADD-ONS section: upgrades for the desktops your decks run.
fn addon_shop(ui: &mut Ui, data: &Data, save: &Save, out: &mut Vec<WbOut>) {
    let desks: Vec<_> = save
        .decks
        .iter()
        .filter_map(|d| data.hardware.deck(d))
        .flat_map(|d| d.firmware.iter().map(|f| f.desktop))
        .collect();
    let items: Vec<_> = data
        .addons
        .iter()
        .filter(|a| desks.contains(&a.desktop) && !save.has_addon(&a.id))
        .collect();
    ui.label(RichText::new("DESKTOP ADD-ONS").color(ink()).strong());
    if items.is_empty() {
        ui.label(dim("Nothing more for your desktops."));
        return;
    }
    egui::Grid::new("addons")
        .num_columns(4)
        .spacing([14.0, 6.0])
        .show(ui, |ui| {
            for a in items {
                ui.label(RichText::new(&a.name).color(ink()).strong())
                    .on_hover_text(&a.blurb);
                ui.label(dim(a.desktop.name()));
                let can = save.bank >= a.price;
                ui.label(RichText::new(format!("{}¢", a.price)).color(if can {
                    ink()
                } else {
                    accent()
                }));
                if ui.add_enabled(can, egui::Button::new(" Buy ")).clicked() {
                    out.push(WbOut::BuyAddon(a.id.clone()));
                }
                ui.end_row();
            }
        });
}

/// STASH: programs kept between runs, and what goes into the next one.
pub fn stash(ui: &mut Ui, data: &Data, save: &Save, out: &mut Vec<WbOut>, edit: &mut Option<u32>) {
    let cap = save.stash_cap(&data.rig);
    let hdd = save.hdd_cap(&data.rig);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("stash {}/{cap}", save.stash.len()))
                .color(accent())
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
    ui.label(RichText::new("KEPT PROGRAMS").color(ink()).strong());
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
                                    .color(ink())
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
                                RichText::new(format!("item {} (corrupt)", item.id))
                                    .color(accent()),
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
                        if save.opens(ninsei_core::hub::Tool::HexEd)
                            && ui.small_button("hex").clicked()
                        {
                            *edit = Some(item.id);
                        }
                    });
                });
            }
        });
    ui.add_space(4.0);
    ui.label(
        RichText::new("STARTERS  (always free, never at risk)")
            .color(ink())
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

/// The Welcome to the Net stack as a HyperCard card: ink on paper, the card's picture on the
/// left and its text on the right, ◀ Home ▶ along the bottom. The Home card lists the
/// sections. Arrow keys page while the pointer is over it.
pub fn guide(ui: &mut Ui, data: &Data, at: &mut ninsei_core::cards::Place) {
    use ninsei_core::cards::{self, Place};
    let stack = &data.welcome;
    let all = stack.cards();
    let (ink, paper) = (ink(), backdrop());
    let over = ui
        .ctx()
        .pointer_hover_pos()
        .is_some_and(|p| ui.clip_rect().contains(p));
    if over {
        let (l, r, h) = ui.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowLeft),
                i.key_pressed(egui::Key::ArrowRight),
                i.key_pressed(egui::Key::Home),
            )
        });
        if l {
            *at = at.prev();
        } else if r {
            *at = at.next(stack);
        } else if h {
            *at = Place::Home;
        }
    }
    let mono = |s: &str, size: f32| RichText::new(s).monospace().size(size).color(ink);
    egui::Frame::new()
        .fill(paper)
        .stroke(egui::Stroke::new(2.0, ink))
        .corner_radius(8.0)
        .inner_margin(14.0)
        .show(ui, |ui| {
            ui.set_min_size(egui::vec2(560.0, 300.0));
            match *at {
                Place::Home => {
                    ui.label(mono(&stack.title, 20.0).strong());
                    ui.label(mono("from your friends at Sense/Net", 12.0));
                    ui.add_space(10.0);
                    for (i, s) in stack.sections.iter().enumerate() {
                        let n = s.cards.len();
                        if ui
                            .button(mono(&format!("{:<16} {n} card(s)", s.name), 14.0))
                            .clicked()
                        {
                            *at = Place::Card(stack.section_start(i));
                        }
                    }
                }
                Place::Card(i) => {
                    let Some((sec, card)) = all.get(i) else {
                        *at = Place::Home;
                        return;
                    };
                    ui.horizontal(|ui| {
                        ui.label(mono(&card.title, 18.0).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(mono(&stack.sections[*sec].name, 12.0));
                        });
                    });
                    ui.add_space(8.0);
                    ui.horizontal_top(|ui| {
                        if let Some(rows) =
                            card.picture.as_ref().and_then(|p| stack.pictures.get(p))
                        {
                            let scale = 3.0;
                            let size = crate::patterns::bitmap_size(rows, scale);
                            let (r, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                            crate::patterns::bitmap(ui.painter(), rows, r.min, scale, ink, paper);
                            ui.add_space(12.0);
                        }
                        ui.vertical(|ui| {
                            for l in &card.text {
                                ui.label(mono(&cards::fill(l, data), 13.0));
                            }
                        });
                    });
                }
            }
        });
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui.button(mono("◀", 14.0)).clicked() {
            *at = at.prev();
        }
        if ui.button(mono("Home", 14.0)).clicked() {
            *at = Place::Home;
        }
        if ui.button(mono("▶", 14.0)).clicked() {
            *at = at.next(stack);
        }
        let where_ = match *at {
            Place::Home => "Home".to_string(),
            Place::Card(i) => format!("card {} of {}", i + 1, all.len()),
        };
        ui.label(RichText::new(where_).monospace().color(muted()));
    });
}

/// The ICE database: every ICE down the left (`????` until you've met it), the picked one's
/// page on the right. Read-only; the save fills it in after each run.
pub fn icedb(ui: &mut Ui, data: &Data, save: &Save, sel: &mut Option<usize>) {
    let all = ninsei_core::icedb::entries(save, data);
    let known = all.iter().filter(|e| e.seen.is_some()).count();
    ui.label(
        RichText::new(format!("ICE DB  {known}/{} on file", all.len()))
            .monospace()
            .strong()
            .color(ink()),
    );
    ui.separator();
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(170.0);
            for (i, e) in all.iter().enumerate() {
                let on = *sel == Some(i);
                // A picked row is filled with the accent, so its text takes the selection's
                // own colour (as `wb_visuals` sets it), never the ink it would vanish into.
                let colour = match (on, e.seen.is_some()) {
                    (true, _) => shadow(),
                    (false, true) => ink(),
                    (false, false) => muted(),
                };
                let label = RichText::new(format!("{:<12}", e.name()))
                    .monospace()
                    .color(colour);
                if ui.selectable_label(on, label).clicked() {
                    *sel = Some(i);
                }
            }
        });
        ui.separator();
        ui.vertical(|ui| match sel.and_then(|i| all.get(i)) {
            Some(e) => {
                for (n, l) in e.lines(save, data).iter().enumerate() {
                    let t = RichText::new(l).monospace();
                    ui.label(if n == 0 {
                        t.strong().color(accent())
                    } else {
                        t.color(ink())
                    });
                }
            }
            None => {
                ui.label(RichText::new("pick an entry.").monospace().color(muted()));
            }
        });
    });
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
        ui.label(
            RichText::new("HexEd: no file open")
                .color(accent())
                .strong(),
        );
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

/// CarrierTerm: a Telix-style dialing directory. Pick a number, see what you're walking into, dial.
pub fn carrierterm(
    ui: &mut Ui,
    data: &Data,
    save: &Save,
    sel: &mut Option<u32>,
    dialing: Option<&Dialing>,
    dial: &mut Option<u32>,
    out: &mut Vec<WbOut>,
) {
    let mono = |t: String, c| RichText::new(t).monospace().color(c);
    if let Some(d) = dialing {
        let Some(e) = save.entry(d.entry) else { return };
        ui.label(
            RichText::new(format!("Dialing {}", e.name))
                .color(accent())
                .strong(),
        );
        ui.add_space(6.0);
        // Lines land on the handshake's beats, so text and sound stay in step.
        let h = Handshake::new(&e.number);
        let mut script = vec![
            (0.0, "ATZ".to_string(), ink()),
            (h.offhook / 2.0, "OK".to_string(), ink()),
            (h.offhook, format!("ATDT {}", e.number), ink()),
        ];
        script.extend(
            h.rings
                .iter()
                .map(|&r| (r, "~~ RING ~~".to_string(), ink())),
        );
        script.push((h.connect, "CONNECT 2400/ARQ".to_string(), accent()));
        script.push((
            h.connect + (h.end - h.connect) / 2.0,
            "CARRIER DETECTED. JACKING IN...".to_string(),
            accent(),
        ));
        for (at, line, col) in script {
            if d.t >= at {
                ui.label(mono(line, col));
            }
        }
        let frac = (d.t / h.end).clamp(0.0, 1.0);
        let cells = 40;
        let on = (frac * cells as f32) as usize;
        ui.add_space(6.0);
        ui.label(mono(
            format!("[{}{}]", "█".repeat(on), "░".repeat(cells - on)),
            accent(),
        ));
        return;
    }
    boards(ui, data, save, out);
    if save.phonebook.is_empty() {
        ui.label(dim("The phonebook is empty."));
        return;
    }
    if sel.is_none_or(|id| save.entry(id).is_none()) {
        *sel = save.phonebook.first().map(|e| e.id);
    }

    ui.label(mono(
        format!(
            "{:<3} {:<9} {:<22} {:<5} {}",
            "#", "NUMBER", "NAME", "TIER", "HEAT"
        ),
        ink(),
    ));
    egui::ScrollArea::vertical()
        .max_height(150.0)
        .show(ui, |ui| {
            for (i, e) in save.phonebook.iter().enumerate() {
                let tier = "▪".repeat(e.tier as usize);
                let heat: String = (0..HEAT_MAX)
                    .map(|h| if h < e.heat { '█' } else { '░' })
                    .collect();
                let line = format!(
                    "{:<3} {:<9} {:<22} {:<5} {heat}",
                    i + 1,
                    e.number,
                    e.name,
                    tier
                );
                let on = *sel == Some(e.id);
                let r = ui.selectable_label(on, mono(line, if on { shadow() } else { ink() }));
                if r.clicked() {
                    *sel = Some(e.id);
                }
                if r.double_clicked() {
                    *dial = Some(e.id);
                }
            }
        });
    ui.add_space(6.0);

    let Some(e) = sel.and_then(|id| save.entry(id)) else {
        return;
    };
    let (net, mods) = Save::site_params(data, e);
    let tier = data
        .networks
        .tier(e.tier)
        .map_or("?".to_string(), |t| t.label.clone());
    ui.label(
        RichText::new(format!("{}  ·  {}", e.name, e.business))
            .color(accent())
            .strong(),
    );
    ui.label(mono(
        format!(
            "TIER {} {tier}  ·  {} layers{}  ·  ICE +{}  ·  pay {}%  ·  trace {}%",
            e.tier,
            net.mid_layers + 2,
            match net.subnets {
                0 => String::new(),
                1 => "  ·  1 subnet".to_string(),
                n => format!("  ·  {n} subnets"),
            },
            mods.ice_bonus,
            mods.pay_pct,
            mods.trace_pct
        ),
        ink(),
    ));
    if e.traits.is_empty() {
        ui.label(dim("  no known quirks"));
    }
    for t in &e.traits {
        if let Some(def) = data.networks.trait_def(t) {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("  {}", def.label))
                        .color(ink())
                        .strong(),
                );
                ui.label(dim(&def.blurb));
            });
        }
    }
    if e.heat > 0 {
        ui.label(dim(format!(
            "  heat {}/{HEAT_MAX} from {} run(s): their ICE and trace are up.",
            e.heat, e.runs
        )));
    }
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui.button(format!(" Dial {} ", e.number)).clicked() {
            *dial = Some(e.id);
        }
        ui.label(dim("double-click a number to dial it"));
    });
}

/// The BBSes you've found: call one and it opens over the desktop.
fn boards(ui: &mut Ui, data: &Data, save: &Save, out: &mut Vec<WbOut>) {
    let known = save.known_boards(data);
    if known.is_empty() {
        return;
    }
    let mono = |t: String, c| RichText::new(t).monospace().color(c);
    ui.label(RichText::new("BBS").color(ink()).strong());
    for b in known {
        let d = &data.boards[b];
        ui.horizontal(|ui| {
            ui.label(mono(format!("{:<9} {:<27}", d.phone, d.name), ink()));
            ui.label(dim(format!("{}¢/call", d.call_cost)));
            if ui.button(" Call ").clicked() {
                out.push(WbOut::Call(d.id.clone()));
            }
        });
    }
    ui.add_space(6.0);
}

/// Finds listed beside the scan grid.
const MAX_FINDS_SHOWN: usize = 8;

/// Seconds a scan's grid takes to fill in.
pub const SCAN_FILL_SECS: f32 = 2.5;

/// WarDial: scan a known range, 100 numbers at a time, and watch the grid fill in.
pub fn wardial(
    ui: &mut Ui,
    data: &Data,
    save: &Save,
    sel: &mut Option<String>,
    scan: Option<&(ScanResult, f32)>,
    out: &mut Vec<WbOut>,
) {
    let mono = |t: String, c| RichText::new(t).monospace().color(c);
    if sel.as_ref().is_none_or(|r| !save.knows_range(r)) {
        *sel = save.ranges.first().map(|k| k.id.clone());
    }
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("bank {}¢", save.bank))
                .color(accent())
                .strong(),
        );
        if save.telco_heat > 0 {
            ui.label(
                RichText::new(format!(
                    "telco heat {}: your next call is watched",
                    save.telco_heat
                ))
                .color(accent()),
            );
        }
    });
    ui.add_space(2.0);
    for k in &save.ranges {
        let Some(r) = data.exchanges.iter().find(|r| r.id == k.id) else {
            continue;
        };
        let line = format!(
            "{:<9} {:<20} {:>3}/{:<3} {:>3}¢",
            r.pattern(),
            r.label,
            save.scanned_in(&r.id),
            r.blocks(),
            r.cost
        );
        let on = sel.as_deref() == Some(r.id.as_str());
        ui.horizontal(|ui| {
            if ui
                .selectable_label(on, mono(line, if on { shadow() } else { ink() }))
                .clicked()
            {
                *sel = Some(r.id.clone());
            }
            ui.label(dim(format!("from {}", k.source)));
        });
    }
    if let Some(r) = sel
        .as_ref()
        .and_then(|id| data.exchanges.iter().find(|r| &r.id == id))
    {
        let left = r.blocks() as usize - save.scanned_in(&r.id).min(r.blocks() as usize);
        let can = left > 0 && save.bank >= r.cost;
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    can,
                    egui::Button::new(format!(" Scan {} ({}¢) ", r.pattern(), r.cost)),
                )
                .clicked()
            {
                out.push(WbOut::Scan(r.id.clone()));
            }
            let note = if left == 0 {
                "tapped out"
            } else {
                "100 numbers a scan"
            };
            ui.label(dim(note));
        });
    }
    ui.add_space(4.0);
    let Some((res, t)) = scan else {
        ui.label(dim("Narrower ranges hit more often. Find them in vaults, BBS text files and on the street."));
        return;
    };
    let shown = ((t / SCAN_FILL_SECS) * res.cells.len() as f32).ceil() as usize;
    let pattern = data
        .exchanges
        .iter()
        .find(|r| r.id == res.range)
        .map_or(res.range.clone(), |r| r.pattern());
    ui.label(mono(
        format!("scan {pattern}  block {:02}", res.block),
        ink(),
    ));
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            for (row, chunk) in res.cells.chunks(10).enumerate() {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.label(mono(format!("{:02} ", row * 10), ink()));
                    for (i, (_, a)) in chunk.iter().enumerate() {
                        let k = row * 10 + i;
                        let (g, c) = if k >= shown {
                            ('░', fill())
                        } else {
                            let c = match a {
                                Answer::Carrier { .. } | Answer::Trap | Answer::Board { .. } => {
                                    accent()
                                }
                                _ => ink(),
                            };
                            (a.glyph(), c)
                        };
                        ui.label(mono(format!("{g} "), c));
                    }
                });
            }
        });
        ui.add_space(12.0);
        ui.vertical(|ui| {
            ui.label(dim(". voice   - busy   F fax"));
            ui.label(dim("C CARRIER   ! TRAP   B BBS"));
            ui.add_space(4.0);
            // The latest finds; earlier ones are already in the phonebook.
            let finds: Vec<&(String, Answer)> = res
                .cells
                .iter()
                .take(shown)
                .filter(|(_, a)| {
                    matches!(
                        a,
                        Answer::Carrier { .. } | Answer::Trap | Answer::Board { .. }
                    )
                })
                .collect();
            if finds.len() > MAX_FINDS_SHOWN {
                ui.label(dim(format!(
                    "+{} more above",
                    finds.len() - MAX_FINDS_SHOWN
                )));
            }
            for (number, a) in finds
                .iter()
                .skip(finds.len().saturating_sub(MAX_FINDS_SHOWN))
            {
                if let Answer::Carrier { entry, new } = a {
                    let who = save
                        .entry(*entry)
                        .map_or("?".to_string(), |e| format!("{} T{}", e.name, e.tier));
                    let tag = if *new { "-> phonebook" } else { "(known)" };
                    ui.label(mono(format!("{number} CARRIER {who} {tag}"), accent()));
                } else if let Answer::Board { board, new } = a {
                    let tag = if *new { "-> phonebook" } else { "(known)" };
                    let name = data.boards.get(*board).map_or("?", |b| b.name.as_str());
                    ui.label(mono(format!("{number} BBS {name} {tag}"), accent()));
                } else {
                    ui.label(mono(format!("{number} TRAP: telco is listening"), accent()));
                }
            }
        });
    });
}

/// Prefs: the theme (desktop and runs alike) and the CRT. Themes come from `data/themes.ron`.
pub fn prefs(ui: &mut Ui, data: &Data, save: &Save, out: &mut Vec<WbOut>) {
    let swatch = |ui: &mut Ui, t: &ninsei_core::data::ThemeDef| {
        let p = Palette::from_def(t);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for c in [p.bg, p.panel, p.faint, p.dim, p.fg, p.bright] {
                let (r, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                ui.painter().rect_filled(r, 0.0, c);
            }
        });
    };
    let monitor = save.monitor_def(data);
    let deck = save.deck_def(data);

    ui.label(RichText::new("MONITOR").color(ink()).strong());
    ui.label(dim("Your monitor sets the CRT look. RIG sells more."));
    for id in &save.monitors {
        let Some(m) = data.hardware.monitor(id) else {
            continue;
        };
        ui.horizontal(|ui| {
            if ui
                .radio(save.monitor == m.id, format!("{:<22}", m.title()))
                .on_hover_text(&m.blurb)
                .clicked()
            {
                out.push(WbOut::Monitor(m.id.clone()));
            }
            ui.label(dim(if m.colour { "colour" } else { "mono" }));
        });
    }
    let mut crt = save.crt;
    if ui.checkbox(&mut crt, "CRT effects").changed() {
        out.push(WbOut::Crt);
    }
    let mut sound = save.sound;
    if ui.checkbox(&mut sound, "Modem sounds").changed() {
        out.push(WbOut::Sound);
    }
    let mut intros = save.intros;
    if ui
        .checkbox(&mut intros, "Cracktros")
        .on_hover_text(
            "Programs from the boards open with their crew's intro. Any key or click skips it.",
        )
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
    })
    .response
    .on_hover_text("The screen breaks down as your deck takes damage. Reduced keeps the colour fringe and the hit flash.");
    ui.add_space(8.0);

    ui.label(RichText::new("THEME").color(ink()).strong());
    let lock = monitor.locked_theme(data);
    ui.label(dim(match lock {
        Some(_) => format!(
            "The {} is a mono tube: one colour. A colour monitor shows any theme.",
            monitor.title()
        ),
        None => "One theme for everything: this desktop and your runs.".to_string(),
    }));
    ui.add_space(2.0);
    for t in &data.themes {
        let ok = lock.is_none_or(|l| l == t.id);
        ui.horizontal(|ui| {
            ui.add_enabled_ui(ok, |ui| {
                if ui
                    .radio(save.phosphor == t.id, format!("{:<16}", t.label))
                    .clicked()
                {
                    out.push(WbOut::Theme(t.id.clone()));
                }
            });
            swatch(ui, t);
        });
    }
    ui.add_space(8.0);

    ui.label(RichText::new("DECK").color(ink()).strong());
    ui.label(dim("Your deck sets how runs look. RIG sells more."));
    for id in &save.decks {
        let Some(d) = data.hardware.deck(id) else {
            continue;
        };
        if ui
            .radio(save.deck == d.id, d.title())
            .on_hover_text(&d.blurb)
            .clicked()
        {
            out.push(WbOut::Deck(d.id.clone()));
        }
    }
    let fw = save.firmware_def(data);
    if deck.firmware.len() > 1 {
        ui.horizontal(|ui| {
            ui.label(dim("  firmware"));
            for f in &deck.firmware {
                if ui.radio(fw.id == f.id, &f.label).clicked() {
                    out.push(WbOut::Firmware(f.id.clone()));
                }
            }
        });
    }
    let map = match fw.map {
        ninsei_core::netview::MapView::Iso => "isometric wireframes",
        ninsei_core::netview::MapView::Draft => "technical draft",
        ninsei_core::netview::MapView::Paint => "1-bit picture",
    };
    ui.label(dim(format!(
        "  {}: runs {}, network map as {map}",
        fw.label,
        fw.desktop.name()
    )));
}
