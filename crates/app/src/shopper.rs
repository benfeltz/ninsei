//! CPU Shopper, the RIG window: an issue of a mail-order magazine. Core lays out the pages
//! (`ninsei_core::shopper`) from the catalog and `data/hub/shopper.ron`; this only draws them
//! in the desk's colours (newsprint: one ink on the theme's paper) and routes Order clicks as
//! `WbOut::Order`. Nothing here knows what kind of item it's selling.

use egui::{Align2, FontId, Response, RichText, Sense, Stroke, Ui, pos2, vec2};
use ninsei_core::hub::{Save, Upgrade};
use ninsei_core::shopper::{
    self, AdDef, AdSize, Block, Listing, Page, ShopperDef, Stock, VendorDef,
};
use ninsei_core::{Data, tuning};

use crate::home::{WbOut, accent, backdrop, ink, muted};

/// Type sizes.
mod size {
    pub const MASTHEAD: f32 = 64.0;
    pub const HOTLINE: f32 = 30.0;
    pub const HEADLINE: f32 = 24.0;
    pub const TITLE: f32 = 18.0;
    pub const BODY: f32 = 14.0;
    pub const SMALL: f32 = 12.0;
}

pub fn show(ui: &mut Ui, data: &Data, save: &Save, out: &mut Vec<WbOut>) {
    let def = &data.shopper;
    let cat = save.catalog(data);
    let pages = shopper::layout(def, &cat);
    let id = ui.id().with("cpu-shopper-page");
    let mut page: usize = ui
        .data(|d| d.get_temp(id))
        .unwrap_or(0)
        .min(pages.len() - 1);

    // Turning pages: buttons, or PgUp/PgDn/Home while the pointer is over the magazine and
    // nothing else has the keyboard.
    let hovered = ui.rect_contains_pointer(ui.max_rect());
    if hovered && ui.memory(|m| m.focused().is_none()) {
        ui.input(|i| {
            if i.key_pressed(egui::Key::PageDown) {
                page = (page + 1).min(pages.len() - 1);
            }
            if i.key_pressed(egui::Key::PageUp) {
                page = page.saturating_sub(1);
            }
            if i.key_pressed(egui::Key::Home) {
                page = 0;
            }
        });
    }
    let jump = nav(ui, def, &pages, page, save.bank);
    page = jump.unwrap_or(page);
    ui.add_space(6.0);

    let mut goto = None;
    egui::ScrollArea::vertical()
        .id_salt(("cpu-shopper", page))
        .auto_shrink([false, false])
        .show(ui, |ui| match &pages[page] {
            Page::Cover => goto = cover(ui, def, &pages, save.bank),
            Page::Contents => goto = contents(ui, def, &pages),
            Page::Index => index(ui, def, &cat, &pages),
            Page::Section {
                section,
                number,
                blocks,
                ..
            } => {
                running_head(ui, def, *number);
                if *section == shopper::Section::Upgrades {
                    next_run(ui, data, save);
                }
                section_page(ui, data, save, def, &cat, blocks, out);
            }
        });
    if let Some(g) = goto {
        page = g;
    }
    ui.data_mut(|d| d.insert_temp(id, page));
}

/// ◀ page ▶, Contents, and the bank. Returns a page to jump to.
fn nav(ui: &mut Ui, def: &ShopperDef, pages: &[Page], page: usize, bank: u32) -> Option<usize> {
    let mut to = None;
    ui.horizontal(|ui| {
        if ui
            .add_enabled(page > 0, egui::Button::new(" ◀ "))
            .on_hover_text("previous page (PgUp)")
            .clicked()
        {
            to = Some(page - 1);
        }
        let label = match &pages[page] {
            Page::Cover => "Cover".to_string(),
            Page::Contents => "Contents".to_string(),
            Page::Index => "Index of advertisers".to_string(),
            Page::Section {
                section, number, ..
            } => {
                let title = def.section(*section).map_or("", |s| s.title.as_str());
                format!("{title} · p.{number} of {}", def.pages)
            }
        };
        ui.label(RichText::new(label).color(muted()).monospace());
        if ui
            .add_enabled(page + 1 < pages.len(), egui::Button::new(" ▶ "))
            .on_hover_text("next page (PgDn)")
            .clicked()
        {
            to = Some(page + 1);
        }
        if ui.button("Contents").clicked() {
            to = Some(1);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(format!("Bank {bank}¢"))
                    .color(accent())
                    .strong(),
            );
        });
    });
    rule(ui, 1.0);
    to
}

fn text(t: impl Into<String>, size: f32) -> RichText {
    RichText::new(t).size(size).color(ink())
}

fn bold(t: impl Into<String>, size: f32) -> RichText {
    text(t, size).strong()
}

fn small(t: impl Into<String>) -> RichText {
    RichText::new(t).size(size::SMALL).color(muted())
}

fn rule(ui: &mut Ui, width: f32) {
    let r = ui.available_rect_before_wrap();
    let y = ui.cursor().top();
    ui.painter()
        .hline(r.left()..=r.right(), y, Stroke::new(width, ink()));
    ui.add_space(width + 2.0);
}

/// A ruled box, like a print ad's border.
fn boxed<R>(ui: &mut Ui, weight: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .stroke(Stroke::new(weight, ink()))
        .inner_margin(10.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// A bar in inverse video: the theme's paper on ink.
fn inverse(ui: &mut Ui, t: &str, size: f32) {
    let font = FontId::proportional(size);
    let h = size + 8.0;
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
    ui.painter().rect_filled(r, 0.0, ink());
    ui.painter()
        .text(r.center(), Align2::CENTER_CENTER, t, font, backdrop());
}

/// A dot-leader line: `left ........ right`. Clickable.
fn leader(ui: &mut Ui, left: &str, right: &str, strong: bool) -> Response {
    let font = FontId::proportional(size::BODY);
    let h = size::BODY + 6.0;
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click());
    let p = ui.painter();
    let col = if resp.hovered() { accent() } else { ink() };
    let l = p.text(
        r.left_center(),
        Align2::LEFT_CENTER,
        left,
        font.clone(),
        col,
    );
    let rr = p.text(r.right_center(), Align2::RIGHT_CENTER, right, font, col);
    if strong {
        p.hline(l.left()..=l.right(), l.bottom(), Stroke::new(1.0, col));
    }
    let (a, b) = (l.right() + 4.0, rr.left() - 4.0);
    let mut x = a;
    while x < b {
        p.circle_filled(pos2(x, r.center().y + 4.0), 0.8, muted());
        x += 5.0;
    }
    resp
}

fn running_head(ui: &mut Ui, def: &ShopperDef, number: u32) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.label(bold(format!("Page {number}"), size::SMALL));
        ui.add_space(20.0);
        ui.label(bold(format!("{}, {}", def.title, def.month), size::SMALL));
    });
    rule(ui, 2.0);
}

/// The upgrades pages say what your rig will be next run.
fn next_run(ui: &mut Ui, data: &Data, save: &Save) {
    let bonus = |u| save.rig.bonus(u, &data.rig);
    ui.label(small(format!(
        "Your rig next run: CPU {}  RAM {}  HDD {}  INTEG {}  STASH {}",
        tuning::START_CPU + bonus(Upgrade::Cpu),
        tuning::START_RAM + bonus(Upgrade::Ram),
        tuning::START_HDD + bonus(Upgrade::Hdd),
        tuning::START_INTEGRITY + bonus(Upgrade::Integrity),
        save.stash_cap(&data.rig),
    )));
    ui.add_space(4.0);
}

fn cover(ui: &mut Ui, def: &ShopperDef, pages: &[Page], bank: u32) -> Option<usize> {
    ui.horizontal(|ui| {
        ui.label(small(&def.issue));
        ui.label(small(&def.strap));
        ui.label(small(&def.newsstand));
    });
    rule(ui, 1.0);
    ui.label(bold(&def.title, size::MASTHEAD));
    inverse(ui, &def.banner, size::BODY);
    ui.add_space(8.0);
    let toc = shopper::contents(pages);
    let mut to = None;
    ui.columns(2, |cols| {
        for c in &def.cover {
            let at = toc.iter().find(|(s, ..)| *s == c.section);
            if let Some(k) = &c.kicker {
                cols[0].label(small(k));
            }
            let head = cols[0].add(
                egui::Label::new(bold(&c.headline, size::HEADLINE))
                    .sense(Sense::click())
                    .wrap(),
            );
            let page = at.map_or(String::new(), |&(_, _, n)| format!(" Page {n}"));
            cols[0].label(text(format!("{}{page}", c.line), size::BODY));
            cols[0].add_space(8.0);
            if head.clicked() {
                to = at.map(|&(_, i, _)| i);
            }
        }
        starburst(&mut cols[1], bank);
        cols[1].add_space(10.0);
        if let Some(i) = toc_box(&mut cols[1], def, pages) {
            to = Some(i);
        }
    });
    to
}

/// The bank in a starburst on the cover.
fn starburst(ui: &mut Ui, bank: u32) {
    let d = ui.available_width().min(200.0);
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), d), Sense::hover());
    let c = r.center();
    let p = ui.painter();
    // A jagged star: alternating radii around the centre.
    let spikes = 18;
    let pts: Vec<_> = (0..spikes * 2)
        .map(|i| {
            let a = std::f32::consts::TAU * i as f32 / (spikes * 2) as f32;
            let rad = if i % 2 == 0 { d / 2.0 } else { d / 2.0 * 0.82 };
            c + vec2(a.cos(), a.sin()) * rad
        })
        .collect();
    p.add(egui::Shape::convex_polygon(pts, ink(), Stroke::NONE));
    p.text(
        c - vec2(0.0, 26.0),
        Align2::CENTER_CENTER,
        "YOUR CREDIT",
        FontId::proportional(size::BODY),
        backdrop(),
    );
    p.text(
        c,
        Align2::CENTER_CENTER,
        format!("{bank}¢"),
        FontId::proportional(size::HOTLINE),
        backdrop(),
    );
    p.text(
        c + vec2(0.0, 26.0),
        Align2::CENTER_CENTER,
        "spend it inside!",
        FontId::proportional(size::SMALL),
        backdrop(),
    );
}

/// IN THIS ISSUE: each section and the index, with their pages. Returns a page to open.
fn toc_box(ui: &mut Ui, def: &ShopperDef, pages: &[Page]) -> Option<usize> {
    let mut to = None;
    boxed(ui, 1.0, |ui| {
        ui.label(small("IN THIS ISSUE"));
        for (s, i, n) in shopper::contents(pages) {
            let title = def.section(s).map_or("", |d| d.title.as_str());
            if leader(ui, title, &n.to_string(), false).clicked() {
                to = Some(i);
            }
        }
        if leader(ui, "Index of advertisers", &def.pages.to_string(), false).clicked() {
            to = Some(pages.len() - 1);
        }
    });
    to
}

fn contents(ui: &mut Ui, def: &ShopperDef, pages: &[Page]) -> Option<usize> {
    ui.label(bold("CONTENTS", size::HEADLINE));
    rule(ui, 2.0);
    let to = toc_box(ui, def, pages);
    ui.add_space(8.0);
    ui.label(small(
        "Every price is the seller's own. Order from any page: it goes straight to your rig.",
    ));
    to
}

fn index(ui: &mut Ui, def: &ShopperDef, cat: &[Listing], pages: &[Page]) {
    ui.label(bold("INDEX OF ADVERTISERS", size::HEADLINE));
    rule(ui, 2.0);
    for (v, nums) in shopper::advertisers(def, cat, pages) {
        let name = def.vendor(&v).map_or(v.clone(), |d| d.name.clone());
        let nums: Vec<String> = nums.iter().map(u32::to_string).collect();
        leader(ui, &name, &nums.join(", "), false);
    }
}

fn section_page(
    ui: &mut Ui,
    data: &Data,
    save: &Save,
    def: &ShopperDef,
    cat: &[Listing],
    blocks: &[Block],
    out: &mut Vec<WbOut>,
) {
    // Big ads span the page; boxes and price lists sit two to a row.
    let wide = |b: &Block| match b {
        Block::Ad { ad, .. } => def.ads[*ad].size != AdSize::Box,
        Block::List { .. } => false,
    };
    let mut i = 0;
    while i < blocks.len() {
        if wide(&blocks[i]) {
            block(ui, data, save, def, cat, &blocks[i], out);
            i += 1;
        } else {
            let pair: Vec<&Block> = blocks[i..]
                .iter()
                .take_while(|b| !wide(b))
                .take(2)
                .collect();
            let n = pair.len();
            ui.columns(2, |cols| {
                for (c, b) in pair.into_iter().enumerate() {
                    block(&mut cols[c], data, save, def, cat, b, out);
                }
            });
            i += n;
        }
        ui.add_space(8.0);
    }
}

fn block(
    ui: &mut Ui,
    data: &Data,
    save: &Save,
    def: &ShopperDef,
    cat: &[Listing],
    b: &Block,
    out: &mut Vec<WbOut>,
) {
    match b {
        Block::Ad { listing, ad } => {
            let l = &cat[*listing];
            let a = &def.ads[*ad];
            let vendor = def.vendor(def.vendor_of(l));
            match a.size {
                AdSize::Full => full_ad(ui, data, save, l, a, vendor, out),
                AdSize::Half | AdSize::Box => small_ad(ui, data, save, l, a, vendor, out),
            }
        }
        Block::List { title, listings } => boxed(ui, 2.0, |ui| {
            inverse(ui, title, size::BODY);
            ui.add_space(4.0);
            for &l in listings {
                list_line(ui, data, save, &cat[l], out);
            }
        }),
    }
}

/// What the price spot says: the price, IN YOUR RIG, or SOLD OUT.
fn price_text(l: &Listing) -> String {
    match l.stock {
        Stock::Price(p) => format!("{p}¢"),
        Stock::Owned => "IN YOUR RIG".into(),
        Stock::SoldOut => "SOLD OUT".into(),
    }
}

fn review_tag(ui: &mut Ui, l: &Listing) {
    if l.review {
        ui.label(small("[REVIEW COPY]"));
    }
}

/// The Order button, or why there isn't one.
fn order(ui: &mut Ui, data: &Data, save: &Save, l: &Listing, label: &str, out: &mut Vec<WbOut>) {
    match l.stock {
        Stock::Price(p) => {
            let blocker = save.order_blocker(data, &l.item);
            let resp = ui.add_enabled(
                blocker.is_none(),
                egui::Button::new(bold(format!("{label} · {p}¢"), size::BODY)),
            );
            if let Some(why) = blocker {
                resp.on_disabled_hover_text(why);
            } else if resp.clicked() {
                out.push(WbOut::Order(l.item.clone()));
            }
        }
        Stock::Owned => {
            ui.label(bold("IN YOUR RIG", size::BODY));
        }
        Stock::SoldOut => {
            // A stamp across the box.
            ui.label(bold("SOLD OUT", size::HEADLINE));
        }
    }
}

fn bullets(ui: &mut Ui, list: &[String]) {
    for b in list {
        ui.label(text(format!("★ {b}"), size::BODY));
    }
}

fn full_ad(
    ui: &mut Ui,
    data: &Data,
    save: &Save,
    l: &Listing,
    a: &AdDef,
    vendor: Option<&VendorDef>,
    out: &mut Vec<WbOut>,
) {
    boxed(ui, 3.0, |ui| {
        if let Some(v) = vendor {
            ui.vertical_centered(|ui| {
                ui.label(bold(format!("{} Hotline:", v.name), size::TITLE));
                if let Some(h) = &v.hotline {
                    ui.label(bold(h, size::HOTLINE));
                }
                if let Some(s) = &v.slogan {
                    ui.label(bold(format!("★ {s} ★"), size::HEADLINE));
                }
            });
            rule(ui, 3.0);
        }
        ui.label(bold(
            a.headline.as_deref().unwrap_or(&l.name),
            size::HOTLINE,
        ));
        review_tag(ui, l);
        ui.label(text(&l.blurb, size::BODY));
        if !l.detail.is_empty() {
            ui.label(small(&l.detail));
        }
        ui.add_space(4.0);
        bullets(ui, &a.bullets);
        if let Some(n) = &a.note {
            ui.label(small(n));
        }
        ui.add_space(6.0);
        price_line(ui, l, a);
        order(ui, data, save, l, "ORDER", out);
        if let Some(v) = vendor.filter(|v| !v.terms.is_empty()) {
            ui.add_space(6.0);
            rule(ui, 3.0);
            ui.label(bold("TERMS:", size::SMALL));
            for t in &v.terms {
                ui.label(text(format!("★ {t}"), size::SMALL));
            }
        }
    });
}

/// "SPECIAL PRICE 400¢", or the ad's callout with the list price under it.
fn price_line(ui: &mut Ui, l: &Listing, a: &AdDef) {
    match (&a.callout, l.stock) {
        (Some(c), Stock::Price(p)) => {
            ui.label(bold(c, size::HEADLINE));
            ui.label(small(format!("or {p}¢ list")));
        }
        (_, Stock::Price(p)) => {
            ui.label(bold(format!("SPECIAL PRICE {p}¢"), size::HEADLINE));
        }
        _ => {}
    }
}

fn small_ad(
    ui: &mut Ui,
    data: &Data,
    save: &Save,
    l: &Listing,
    a: &AdDef,
    vendor: Option<&VendorDef>,
    out: &mut Vec<WbOut>,
) {
    let big = a.size == AdSize::Half;
    boxed(ui, 2.0, |ui| {
        if let Some(v) = vendor {
            ui.label(small(v.name.to_uppercase()));
        }
        let head = a.headline.as_deref().unwrap_or(&l.name);
        ui.label(bold(head, if big { size::HEADLINE } else { size::TITLE }));
        review_tag(ui, l);
        ui.label(text(&l.blurb, size::BODY));
        if !l.detail.is_empty() {
            ui.label(small(&l.detail));
        }
        bullets(ui, &a.bullets);
        if let Some(n) = &a.note {
            ui.label(small(n));
        }
        ui.add_space(4.0);
        if big {
            price_line(ui, l, a);
        }
        order(ui, data, save, l, "ORDER", out);
    });
}

/// One line of a price list: click it to order.
fn list_line(ui: &mut Ui, data: &Data, save: &Save, l: &Listing, out: &mut Vec<WbOut>) {
    let name = if l.review {
        format!("{} [REVIEW COPY]", l.name)
    } else if l.detail.is_empty() {
        l.name.clone()
    } else {
        format!("{} ({})", l.name, l.detail)
    };
    let resp = leader(ui, &name, &price_text(l), false);
    let blocker = match l.stock {
        Stock::Price(_) => save.order_blocker(data, &l.item),
        _ => None,
    };
    let hint = match (&blocker, l.stock) {
        (Some(why), _) => format!("{}\n{why}", l.blurb),
        (None, Stock::Price(_)) => format!("{}\nclick to order", l.blurb),
        _ => l.blurb.clone(),
    };
    let resp = resp.on_hover_text(hint);
    if resp.clicked() && blocker.is_none() && matches!(l.stock, Stock::Price(_)) {
        out.push(WbOut::Order(l.item.clone()));
    }
}
