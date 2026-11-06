//! CPU Shopper, the RIG window: an issue of a mail-order magazine. Core lays out the pages
//! (`ninsei_core::shopper`): every page is the same size, and each block on a section page owns
//! a rectangle of its grid. This only draws them, through the shared book control
//! (`pager.rs`), in the desk's colours (newsprint: one ink on the theme's paper), and routes
//! Order clicks as `WbOut::Order`. Nothing here knows what kind of item it's selling.

use egui::{Align2, FontId, Rect, Response, RichText, Sense, Stroke, Ui, pos2, vec2};
use ninsei_core::book::Turn;
use ninsei_core::hub::{Save, Upgrade};
use ninsei_core::shopper::{
    self, AdDef, AdSize, Block, Cell, GRID_COLS, GRID_ROWS, Listing, Page, ShopperDef, Stock,
    VendorDef,
};
use ninsei_core::{Data, tuning};

use crate::home::{WbOut, accent, backdrop, ink, muted};
use crate::pager::{self, Book};

/// Type sizes.
mod size {
    pub const MASTHEAD: f32 = 56.0;
    pub const HOTLINE: f32 = 26.0;
    pub const HEADLINE: f32 = 20.0;
    pub const TITLE: f32 = 16.0;
    pub const BODY: f32 = 13.0;
    pub const SMALL: f32 = 11.0;
}

/// Space between blocks on a page, and inside their borders.
const GUTTER: f32 = 6.0;
const MARGIN: f32 = 8.0;
/// The running head across the top of a section page.
const HEAD: f32 = 22.0;

pub fn show(ui: &mut Ui, data: &Data, save: &Save, out: &mut Vec<WbOut>) {
    let def = &data.shopper;
    let cat = save.catalog(data);
    let pages = shopper::layout(def, &cat);
    let id = ui.id().with("cpu-shopper-page");
    let at: usize = ui
        .data(|d| d.get_temp(id))
        .unwrap_or(0)
        .min(pages.len() - 1);
    let label = match &pages[at] {
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
    let status = format!("Bank {}¢", save.bank);
    let book = Book {
        at,
        count: pages.len(),
        label: &label,
        status: &status,
    };
    let next = pager::show(ui, book, |ui, rect| match &pages[at] {
        Page::Cover => cover(ui, rect, def, &pages, save.bank),
        Page::Contents => contents(ui, def, &pages),
        Page::Index => {
            index(ui, def, &cat, &pages);
            None
        }
        Page::Section {
            section,
            number,
            blocks,
            ..
        } => {
            section_page(
                ui, rect, data, save, def, &cat, *section, *number, blocks, out,
            );
            None
        }
    });
    ui.data_mut(|d| d.insert_temp(id, next));
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
    let r = ui.max_rect();
    let y = ui.cursor().top() + width / 2.0;
    ui.painter()
        .hline(r.left()..=r.right(), y, Stroke::new(width, ink()));
    ui.add_space(width + 3.0);
}

/// A bar in inverse video: the theme's paper on ink.
fn inverse(ui: &mut Ui, t: &str, size: f32) {
    let font = FontId::proportional(size);
    let h = size + 6.0;
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
    ui.painter().rect_filled(r, 0.0, ink());
    ui.painter()
        .text(r.center(), Align2::CENTER_CENTER, t, font, backdrop());
}

/// A dot-leader line: `left ........ right`. Clickable.
fn leader(ui: &mut Ui, left: &str, right: &str) -> Response {
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
    let mut x = l.right() + 4.0;
    while x < rr.left() - 4.0 {
        p.circle_filled(pos2(x, r.center().y + 4.0), 0.8, muted());
        x += 5.0;
    }
    resp
}

/// A ruled box filling `r`, with a margin inside: what every block on a page draws in.
fn boxed(ui: &mut Ui, r: Rect, weight: f32, add: impl FnOnce(&mut Ui)) {
    ui.painter()
        .rect_stroke(r, 0.0, Stroke::new(weight, ink()), egui::StrokeKind::Inside);
    let mut inner = pager::area(ui, r.shrink(MARGIN + weight));
    add(&mut inner);
}

fn cover(ui: &mut Ui, rect: Rect, def: &ShopperDef, pages: &[Page], bank: u32) -> Option<Turn> {
    let mut ui = pager::area(ui, rect.shrink(MARGIN));
    let ui = &mut ui;
    ui.horizontal(|ui| {
        ui.label(small(&def.issue));
        ui.add_space(16.0);
        ui.label(small(&def.strap));
        ui.add_space(16.0);
        ui.label(small(&def.newsstand));
    });
    rule(ui, 1.0);
    ui.label(bold(&def.title, size::MASTHEAD));
    inverse(ui, &def.banner, size::BODY);
    ui.add_space(8.0);
    let toc = shopper::contents(pages);
    let top = ui.cursor().top();
    let body = Rect::from_min_max(pos2(ui.max_rect().left(), top), ui.max_rect().max);
    let half = (body.width() - GUTTER) / 2.0;
    let left = Rect::from_min_size(body.min, vec2(half, body.height()));
    let right = Rect::from_min_size(pos2(left.max.x + GUTTER, top), vec2(half, body.height()));
    let mut to = None;
    let mut l = pager::area(ui, left);
    for c in &def.cover {
        let at = toc.iter().find(|(s, ..)| *s == c.section);
        if let Some(k) = &c.kicker {
            l.label(small(k));
        }
        let head = l.add(
            egui::Label::new(bold(&c.headline, size::HEADLINE))
                .sense(Sense::click())
                .wrap(),
        );
        let page = at.map_or(String::new(), |&(_, _, n)| format!(" Page {n}"));
        l.label(text(format!("{}{page}", c.line), size::BODY));
        l.add_space(6.0);
        if head.clicked() {
            to = at.map(|&(_, i, _)| Turn::To(i));
        }
    }
    let mut r = pager::area(ui, right);
    starburst(&mut r, bank);
    r.add_space(8.0);
    if let Some(t) = toc_box(&mut r, def, pages) {
        to = Some(t);
    }
    to
}

/// The bank in a starburst on the cover.
fn starburst(ui: &mut Ui, bank: u32) {
    let d = ui.available_width().min(150.0);
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), d), Sense::hover());
    let c = r.center();
    let p = ui.painter();
    let spikes = 18;
    let pts: Vec<_> = (0..spikes * 2)
        .map(|i| {
            let a = std::f32::consts::TAU * i as f32 / (spikes * 2) as f32;
            let rad = if i % 2 == 0 { d / 2.0 } else { d / 2.0 * 0.82 };
            c + vec2(a.cos(), a.sin()) * rad
        })
        .collect();
    p.add(egui::Shape::convex_polygon(pts, ink(), Stroke::NONE));
    let line = |dy: f32, t: String, s: f32| {
        p.text(
            c + vec2(0.0, dy),
            Align2::CENTER_CENTER,
            t,
            FontId::proportional(s),
            backdrop(),
        );
    };
    line(-20.0, "YOUR CREDIT".into(), size::SMALL);
    line(0.0, format!("{bank}¢"), size::HEADLINE);
    line(20.0, "spend it inside!".into(), size::SMALL);
}

/// IN THIS ISSUE: each section and the index, with their pages.
fn toc_box(ui: &mut Ui, def: &ShopperDef, pages: &[Page]) -> Option<Turn> {
    let mut to = None;
    ui.label(small("IN THIS ISSUE"));
    for (s, i, n) in shopper::contents(pages) {
        let title = def.section(s).map_or("", |d| d.title.as_str());
        if leader(ui, title, &n.to_string()).clicked() {
            to = Some(Turn::To(i));
        }
    }
    if leader(ui, "Index of advertisers", &def.pages.to_string()).clicked() {
        to = Some(Turn::To(pages.len() - 1));
    }
    to
}

fn contents(ui: &mut Ui, def: &ShopperDef, pages: &[Page]) -> Option<Turn> {
    let rect = ui.max_rect().shrink(MARGIN);
    let mut ui = pager::area(ui, rect);
    ui.label(bold("CONTENTS", size::HEADLINE));
    rule(&mut ui, 2.0);
    let to = toc_box(&mut ui, def, pages);
    ui.add_space(8.0);
    ui.label(small(
        "Every price is the seller's own. Order from any page: it goes straight to your rig.",
    ));
    to
}

fn index(ui: &mut Ui, def: &ShopperDef, cat: &[Listing], pages: &[Page]) {
    let rect = ui.max_rect().shrink(MARGIN);
    let mut ui = pager::area(ui, rect);
    ui.label(bold("INDEX OF ADVERTISERS", size::HEADLINE));
    rule(&mut ui, 2.0);
    for (v, nums) in shopper::advertisers(def, cat, pages) {
        let name = def.vendor(&v).map_or(v.clone(), |d| d.name.clone());
        let nums: Vec<String> = nums.iter().map(u32::to_string).collect();
        leader(&mut ui, &name, &nums.join(", "));
    }
}

/// Where a grid cell lands on the page body.
fn cell_rect(body: Rect, c: Cell) -> Rect {
    let w = (body.width() - GUTTER * (GRID_COLS - 1) as f32) / GRID_COLS as f32;
    let h = (body.height() - GUTTER * (GRID_ROWS - 1) as f32) / GRID_ROWS as f32;
    let min = body.min + vec2(c.col as f32 * (w + GUTTER), c.row as f32 * (h + GUTTER));
    let size = vec2(
        c.cols as f32 * w + (c.cols - 1) as f32 * GUTTER,
        c.rows as f32 * h + (c.rows - 1) as f32 * GUTTER,
    );
    Rect::from_min_size(min, size)
}

#[allow(clippy::too_many_arguments)]
fn section_page(
    ui: &mut Ui,
    rect: Rect,
    data: &Data,
    save: &Save,
    def: &ShopperDef,
    cat: &[Listing],
    section: shopper::Section,
    number: u32,
    blocks: &[(Block, Cell)],
    out: &mut Vec<WbOut>,
) {
    // The running head: the magazine on the left of the rule, the page on the right. On the
    // upgrades pages it also says what your rig will be next run.
    let head = Rect::from_min_size(rect.min, vec2(rect.width(), HEAD));
    let p = ui.painter_at(head);
    let font = FontId::proportional(size::SMALL);
    let left = if section == shopper::Section::Upgrades {
        next_run(data, save)
    } else {
        format!("{}, {}", def.title, def.month)
    };
    p.text(
        head.left_center(),
        Align2::LEFT_CENTER,
        left,
        font.clone(),
        ink(),
    );
    p.text(
        head.right_center(),
        Align2::RIGHT_CENTER,
        format!("Page {number}"),
        font,
        ink(),
    );
    p.hline(head.x_range(), head.max.y - 1.0, Stroke::new(2.0, ink()));
    let body = Rect::from_min_max(pos2(rect.min.x, head.max.y + GUTTER), rect.max);
    for (b, c) in blocks {
        let r = cell_rect(body, *c);
        block(ui, r, data, save, def, cat, b, out);
    }
}

/// What your rig will be next run, for the upgrades pages' running head.
fn next_run(data: &Data, save: &Save) -> String {
    let bonus = |u| save.rig.bonus(u, &data.rig);
    format!(
        "Your rig next run: CPU {}  RAM {}  HDD {}  INTEG {}  STASH {}",
        tuning::START_CPU + bonus(Upgrade::Cpu),
        tuning::START_RAM + bonus(Upgrade::Ram),
        tuning::START_HDD + bonus(Upgrade::Hdd),
        tuning::START_INTEGRITY + bonus(Upgrade::Integrity),
        save.stash_cap(&data.rig),
    )
}

#[allow(clippy::too_many_arguments)]
fn block(
    ui: &mut Ui,
    r: Rect,
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
                AdSize::Full => boxed(ui, r, 3.0, |ui| full_ad(ui, data, save, l, a, vendor, out)),
                AdSize::Half | AdSize::Box => {
                    boxed(ui, r, 2.0, |ui| small_ad(ui, data, save, l, a, vendor, out))
                }
            }
        }
        Block::List { title, listings } => boxed(ui, r, 2.0, |ui| {
            inverse(ui, title, size::BODY);
            ui.add_space(2.0);
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

/// The ad's foot: its price (or callout), then the Order button, or why there isn't one.
fn order(ui: &mut Ui, data: &Data, save: &Save, l: &Listing, a: &AdDef, out: &mut Vec<WbOut>) {
    match l.stock {
        Stock::Price(p) => {
            ui.horizontal(|ui| {
                let blocker = save.order_blocker(data, &l.item);
                let resp = ui.add_enabled(
                    blocker.is_none(),
                    egui::Button::new(bold(format!("ORDER · {p}¢"), size::BODY)),
                );
                if let Some(why) = blocker {
                    resp.on_disabled_hover_text(why);
                } else if resp.clicked() {
                    out.push(WbOut::Order(l.item.clone()));
                }
                if let Some(c) = &a.callout {
                    ui.label(bold(c, size::TITLE));
                }
            });
        }
        Stock::Owned => {
            ui.label(bold("IN YOUR RIG", size::TITLE));
        }
        Stock::SoldOut => {
            ui.label(bold("SOLD OUT", size::HEADLINE));
        }
    }
}

fn bullets(ui: &mut Ui, list: &[String]) {
    for b in list {
        ui.label(text(format!("★ {b}"), size::BODY));
    }
}

fn name_line(ui: &mut Ui, l: &Listing, head: &str, size: f32) {
    ui.horizontal(|ui| {
        ui.label(bold(head, size));
        if l.review {
            ui.label(small("[REVIEW COPY]"));
        }
    });
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
    name_line(
        ui,
        l,
        a.headline.as_deref().unwrap_or(&l.name),
        size::HOTLINE,
    );
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
    order(ui, data, save, l, a, out);
    if let Some(v) = vendor.filter(|v| !v.terms.is_empty()) {
        ui.add_space(6.0);
        rule(ui, 2.0);
        ui.label(bold("TERMS:", size::SMALL));
        ui.label(text(
            v.terms
                .iter()
                .map(|t| format!("★ {t}"))
                .collect::<Vec<_>>()
                .join("   "),
            size::SMALL,
        ));
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
    if let Some(v) = vendor {
        ui.label(small(v.name.to_uppercase()));
    }
    let head = a.headline.as_deref().unwrap_or(&l.name);
    name_line(ui, l, head, if big { size::HEADLINE } else { size::TITLE });
    ui.label(text(&l.blurb, size::BODY));
    if !l.detail.is_empty() {
        ui.label(small(&l.detail));
    }
    bullets(ui, &a.bullets);
    if let Some(n) = &a.note {
        ui.label(small(n));
    }
    ui.add_space(4.0);
    order(ui, data, save, l, a, out);
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
    let resp = leader(ui, &name, &price_text(l));
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
