//! The hub's tool windows: CarrierTerm (phonebook), STASH (programs and loadout) and HexEd.
//! RIG is CPU Shopper, in `shopper.rs`.
//! They draw from the save and report what you clicked as [`WbOut`]s; the app applies those
//! through `Save`'s methods in core.

use egui::{RichText, Ui, Visuals};
use ninsei_core::directory::{self, Filter, Listed};
use ninsei_core::hexedit::HexBuffer;
use ninsei_core::hub::{Answer, Save, ScanResult, Tool};
use ninsei_core::modem::Handshake;
use ninsei_core::{Data, binary};

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
        for id in save.free(data) {
            let id = id.as_str();
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

/// CarrierTerm's directory, as the player left it: what's typed, the filter, the line picked.
#[derive(Default)]
pub struct Directory {
    pub sel: Option<Listed>,
    pub query: String,
    pub filter: Filter,
    /// The pass the directory was last drawn on (none yet: it has just opened); a gap means
    /// it just came back.
    drawn: Option<u64>,
    /// The search box should take the keys at the next chance (an enabled frame).
    want_focus: bool,
}

/// Width of the directory card beside the list.
const CARD_W: f32 = 250.0;
/// Widest the dialing card gets beside the sidebar.
const DIAL_CARD_W: f32 = 340.0;
/// Height of the card's big Dial / Call button.
const BIG_BUTTON_H: f32 = 44.0;
/// Height of the dialing card's progress bar.
const PROGRESS_H: f32 = 12.0;
/// Width of the search box.
const SEARCH_W: f32 = 220.0;
/// Name column width in the directory list, in characters.
const NAME_COLS: usize = 24;
/// Number column width, in characters (`555-0420`, plus one).
const NUMBER_COLS: usize = 9;
/// Space between columns, in characters.
const GUTTER: usize = 2;
/// The fewest characters of a listing worth showing as a column.
const LISTING_MIN_COLS: usize = 10;
/// The narrowest the directory list goes beside the card; narrower, the card goes under it.
const LIST_MIN_W: f32 = 300.0;
/// The list's height when the card sits under it.
const NARROW_LIST_H: f32 = 140.0;
/// Height kept under the list for its status line.
const FOOTER_H: f32 = 22.0;
/// The search box's widget id.
const FIND_ID: &str = "carrierterm-find";

/// Width of one monospace character in `ui`'s current style.
fn mono_char_w(ui: &Ui) -> f32 {
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    ui.fonts_mut(|f| f.glyph_width(&font, 'M')).max(1.0)
}

/// The width `ui` can draw in and be seen: a tile can lay out wider than it shows.
fn visible_width(ui: &Ui) -> f32 {
    ui.available_width()
        .min(ui.clip_rect().right() - ui.cursor().left())
}

/// Space a vertical separator takes between two columns.
fn separator_gap(ui: &Ui) -> f32 {
    // egui's separator is 6pt of spacing plus the item spacing on each side.
    6.0 + ui.spacing().item_spacing.x * 2.0
}

/// The directory's filter buttons. Returns whether one was clicked.
fn filter_buttons(ui: &mut Ui, filter: &mut Filter) -> bool {
    let mut clicked = false;
    for f in Filter::ALL {
        if ui.selectable_label(*filter == f, f.label()).clicked() {
            *filter = f;
            clicked = true;
        }
    }
    clicked
}

/// A full-width row of the directory, inverse when `on`. `live` rows can be clicked.
fn dir_row(ui: &mut Ui, line: String, on: bool, live: bool) -> egui::Response {
    let text = RichText::new(line)
        .monospace()
        .color(if on { shadow() } else { ink() });
    let b = egui::Button::selectable(on, text)
        .min_size(egui::vec2(ui.available_width(), 0.0))
        .wrap_mode(egui::TextWrapMode::Truncate);
    ui.add(if live {
        b
    } else {
        b.sense(egui::Sense::hover())
    })
}

/// The list's box: an outline the full height it's given, so a short list reads as a list.
fn list_box<R>(ui: &mut Ui, height: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .stroke(egui::Stroke::new(1.0, muted()))
        .inner_margin(2.0)
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("carrierterm-dir")
                .auto_shrink([false, false])
                .max_height(height)
                .show(ui, add)
                .inner
        })
        .inner
}

/// CarrierTerm: a friendly dialing directory. Search the numbers you know, pick one, and dial
/// it (a network) or call it (a board). Who they are and how to reach them; never what's
/// behind a number, which is NiGHTCOMM's business.
#[allow(clippy::too_many_arguments)]
pub fn carrierterm(
    ui: &mut Ui,
    data: &Data,
    save: &Save,
    dir: &mut Directory,
    dialing: Option<&Dialing>,
    dial: &mut Option<u32>,
    hang_up: &mut bool,
    out: &mut Vec<WbOut>,
) {
    // Side by side when there's room for both; in a narrow tile the card goes under the list.
    let avail = visible_width(ui);
    let wide = avail >= CARD_W + LIST_MIN_W;
    if let Some(d) = dialing {
        *hang_up = dialing_view(ui, data, save, dir, d, wide);
        return;
    }
    // Not drawn last pass: the window just opened, or a splash or a call just ended. The
    // search box takes the keys, so ↑↓ and Enter work straight away.
    let pass = ui.ctx().cumulative_pass_nr();
    if dir.drawn.is_none_or(|d| d + 1 < pass) {
        dir.want_focus = true;
    }
    dir.drawn = Some(pass);
    let find = egui::Id::new(FIND_ID);
    // `/` searches, when nothing else has the keys and the pointer is over the directory.
    let over = ui.clip_rect();
    let slash = ui.input(|i| {
        i.pointer.hover_pos().is_some_and(|p| over.contains(p))
            && i.events
                .iter()
                .any(|e| matches!(e, egui::Event::Text(t) if t == "/"))
    });
    if slash && ui.memory(|m| m.focused().is_none()) {
        dir.want_focus = true;
    }
    // Keep the request until it can land: a disabled frame would hand the focus back.
    if dir.want_focus && ui.is_enabled() {
        ui.memory_mut(|m| m.request_focus(find));
        dir.want_focus = false;
    }

    let mut keys = (false, false, false);
    let mut refocus = false;
    ui.horizontal(|ui| {
        ui.label(dim("Find"));
        let r = ui.add(
            egui::TextEdit::singleline(&mut dir.query)
                .id(find)
                .hint_text("name, number or listing")
                .desired_width(SEARCH_W.min(avail - 60.0).max(80.0)),
        );
        // Arrows and Enter drive the list while the search box has the keys.
        let (focus, left) = (r.has_focus(), r.lost_focus());
        keys = ui.input(|i| {
            (
                focus && i.key_pressed(egui::Key::ArrowUp),
                focus && i.key_pressed(egui::Key::ArrowDown),
                left && i.key_pressed(egui::Key::Enter),
            )
        });
        if wide {
            ui.add_space(8.0);
            refocus |= filter_buttons(ui, &mut dir.filter);
        }
    });
    // Narrow: the filters get their own row rather than wrapping one under the box.
    if !wide {
        ui.horizontal(|ui| refocus |= filter_buttons(ui, &mut dir.filter));
    }
    ui.separator();

    let shown = directory::search(save, data, &dir.query, dir.filter);
    if !dir.sel.is_some_and(|s| shown.iter().any(|l| l.key == s)) {
        dir.sel = shown.first().map(|l| l.key);
    }
    let (up, down, enter) = keys;
    let moved = up || down;
    if moved {
        dir.sel = directory::step(&shown, dir.sel, if up { -1 } else { 1 });
    }
    let mut go = None;
    if enter {
        go = dir.sel;
    }
    let searching = ui.memory(|m| m.has_focus(find));

    let list_w = if wide {
        avail - CARD_W - separator_gap(ui)
    } else {
        avail
    };
    // Columns that fit: the listing is dropped first, then names are cut short.
    let cols = (list_w / mono_char_w(ui)) as usize;
    let show_listing = cols >= NUMBER_COLS + GUTTER + NAME_COLS + GUTTER + LISTING_MIN_COLS;
    let name_cols = NAME_COLS.min(cols.saturating_sub(NUMBER_COLS + GUTTER + 1).max(8));
    let line = |number: &str, name: &str, listed: &str| {
        let name: String = name.chars().take(name_cols).collect();
        let g = " ".repeat(GUTTER);
        let row = format!("{number:<NUMBER_COLS$}{g}{name:<name_cols$}");
        if show_listing {
            format!("{row}{g}{listed}")
        } else {
            row
        }
    };
    let mut list = |ui: &mut Ui, sel: &mut Option<Listed>| {
        ui.label(
            RichText::new(line("NUMBER", "NAME", "LISTED AS"))
                .monospace()
                .color(muted()),
        );
        let height = if wide {
            (ui.available_height() - FOOTER_H - 8.0).max(80.0)
        } else {
            NARROW_LIST_H
        };
        list_box(ui, height, |ui| {
            if shown.is_empty() {
                ui.label(dim("Nothing matches."));
            }
            for l in &shown {
                let on = *sel == Some(l.key);
                let r = dir_row(ui, line(&l.number, &l.name, &l.listed_as), on, true);
                if on && moved {
                    r.scroll_to_me(None);
                }
                if r.clicked() {
                    *sel = Some(l.key);
                    refocus = true;
                }
                if r.double_clicked() {
                    go = Some(l.key);
                }
            }
        });
        let total = directory::listings(save, data).len();
        let keys = if searching {
            "↑↓ choose  ·  Enter dial"
        } else {
            "/ to search"
        };
        ui.label(dim(format!(
            "{} of {total} numbers  ·  {keys}",
            shown.len()
        )));
    };
    // The card's button: the line it dials, if pressed.
    let show_card = |ui: &mut Ui, sel: Option<Listed>| {
        ui.set_width(CARD_W.min(visible_width(ui)));
        sel.filter(|&key| card(ui, data, save, key))
    };
    let mut sel = dir.sel;
    let pick = if wide {
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(list_w);
                list(ui, &mut sel);
            });
            ui.separator();
            ui.vertical(|ui| show_card(ui, sel)).inner
        })
        .inner
    } else {
        list(ui, &mut sel);
        ui.separator();
        show_card(ui, sel)
    };
    dir.sel = sel;
    go = go.or(pick);
    // Clicking a row or a filter leaves the keys with the search box, like a list box's.
    // Not after Enter: a board's window opens and takes the keys.
    if refocus && go.is_none() {
        dir.want_focus = true;
    }

    match go {
        Some(Listed::Entry(id)) => *dial = Some(id),
        Some(Listed::Board(b)) => out.push(WbOut::Call(data.boards[b].id.clone())),
        None => {}
    }
}

/// The card's title: the name, then what it's listed as and its number on a line below.
fn card_title(ui: &mut Ui, name: &str, sub: String) {
    ui.label(RichText::new(name).color(accent()).strong().size(18.0));
    ui.label(dim(sub));
}

/// A big button across the card.
fn big_button(ui: &mut Ui, text: String) -> bool {
    ui.add_sized(
        [ui.available_width(), BIG_BUTTON_H],
        egui::Button::new(RichText::new(text).color(ink()).strong().size(16.0)),
    )
    .clicked()
}

/// The directory card for `key`: who it is and how to reach them, over a big Dial or Call
/// button. Returns whether the button was pressed.
fn card(ui: &mut Ui, data: &Data, save: &Save, key: Listed) -> bool {
    let row = |ui: &mut Ui, k: &str, v: String| {
        ui.label(dim(k));
        ui.label(RichText::new(v).color(ink()));
        ui.end_row();
    };
    let (name, number, verb) = match key {
        Listed::Entry(id) => {
            let Some(e) = save.entry(id) else {
                return false;
            };
            (e.name.clone(), e.number.clone(), "Dial")
        }
        Listed::Board(b) => {
            let d = &data.boards[b];
            (d.name.clone(), d.phone.clone(), "Call")
        }
    };
    ui.label(dim("DIRECTORY CARD"));
    ui.label(RichText::new(&name).color(accent()).strong().size(18.0));
    ui.add_space(4.0);
    egui::Grid::new("carrierterm-card")
        .num_columns(2)
        .spacing([10.0, 4.0])
        .show(ui, |ui| match key {
            Listed::Entry(id) => {
                let Some(e) = save.entry(id) else { return };
                row(ui, "Number", e.number.clone());
                row(ui, "Listed as", e.business.clone());
                row(ui, "Cost", "free (local)".into());
                row(
                    ui,
                    "Calls",
                    match e.runs {
                        0 => "never".into(),
                        1 => "once".into(),
                        n => format!("{n} times"),
                    },
                );
            }
            Listed::Board(b) => {
                let d = &data.boards[b];
                row(ui, "Number", d.phone.clone());
                row(ui, "Listed as", directory::BOARD_LISTING.into());
                row(ui, "Sysop", d.sysop.clone());
                row(ui, "Speed", format!("{} baud", d.baud));
                row(
                    ui,
                    "Cost",
                    match d.call_cost {
                        0 => "free (local)".into(),
                        c => format!("{c}¢ a call"),
                    },
                );
            }
        });
    ui.add_space(10.0);
    big_button(ui, format!("{verb} {number}"))
}

/// A call under way. Beside the directory (shrunk to a sidebar, held still) when there's
/// room, alone in a narrow tile.
fn dialing_view(
    ui: &mut Ui,
    data: &Data,
    save: &Save,
    dir: &Directory,
    d: &Dialing,
    wide: bool,
) -> bool {
    let mut hang_up = false;
    if wide {
        let side_w = LIST_MIN_W.min(visible_width(ui) - CARD_W - separator_gap(ui));
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(side_w);
                let cols = (side_w / mono_char_w(ui)) as usize;
                let name_cols = cols.saturating_sub(NUMBER_COLS + GUTTER + 1).max(8);
                let mut shown = directory::search(save, data, &dir.query, dir.filter);
                // The number being dialed is always in view, whatever the search left.
                if !shown.iter().any(|l| l.key == Listed::Entry(d.entry)) {
                    shown = directory::listings(save, data);
                }
                let height = (ui.available_height() - 8.0).max(80.0);
                list_box(ui, height, |ui| {
                    for l in &shown {
                        let name: String = l.name.chars().take(name_cols).collect();
                        let g = " ".repeat(GUTTER);
                        let on = l.key == Listed::Entry(d.entry);
                        let r = dir_row(
                            ui,
                            format!("{:<NUMBER_COLS$}{g}{name}", l.number),
                            on,
                            false,
                        );
                        if on {
                            r.scroll_to_me(None);
                        }
                    }
                });
            });
            ui.separator();
            ui.vertical(|ui| {
                ui.set_width(DIAL_CARD_W.min(visible_width(ui)));
                hang_up = dialing_card(ui, save, d);
            });
        });
    } else {
        ui.vertical(|ui| {
            ui.set_width(DIAL_CARD_W.min(visible_width(ui)));
            hang_up = dialing_card(ui, save, d);
        });
    }
    hang_up
}

/// The call as a checklist over a progress bar, with Hang up, all one card wide. The lines
/// land on the handshake's beats, so text and sound stay in step. Returns whether Hang up
/// was pressed.
fn dialing_card(ui: &mut Ui, save: &Save, d: &Dialing) -> bool {
    let Some(e) = save.entry(d.entry) else {
        return false;
    };
    let mono = |t: String, c| RichText::new(t).monospace().color(c);
    ui.label(dim("CALLING"));
    card_title(ui, &e.name, format!("{}  ·  {}", e.business, e.number));
    ui.add_space(8.0);
    let h = Handshake::new(&e.number);
    let ring = h.rings.first().copied().unwrap_or(h.offhook);
    let steps = [
        (0.0, "ATZ: modem reset".to_string()),
        (h.offhook, format!("ATDT {}", e.number)),
        (ring, "Ringing…".to_string()),
        (h.connect, "CONNECT 2400/ARQ".to_string()),
        (
            h.connect + (h.end - h.connect) / 2.0,
            "Carrier detected, jacking in".to_string(),
        ),
    ];
    for (i, (at, text)) in steps.iter().enumerate() {
        let next = steps.get(i + 1).map_or(h.end, |s| s.0);
        let (mark, col) = if d.t >= next {
            ("■", ink())
        } else if d.t >= *at {
            ("»", accent())
        } else {
            ("·", muted())
        };
        ui.label(mono(format!("{mark} {text}"), col));
    }
    ui.add_space(8.0);
    // The call's progress: an outlined track filling with the accent, as wide as the card.
    let frac = (d.t / h.end).clamp(0.0, 1.0);
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, PROGRESS_H), egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(
        egui::Rect::from_min_size(rect.min, egui::vec2(w * frac, PROGRESS_H)),
        0.0,
        accent(),
    );
    p.rect_stroke(
        rect,
        0.0,
        egui::Stroke::new(1.0, ink()),
        egui::StrokeKind::Inside,
    );
    ui.add_space(10.0);
    big_button(ui, "Hang up".into())
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
                out.push(WbOut::Scan(r.id.clone(), Some(Tool::Terminal)));
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
