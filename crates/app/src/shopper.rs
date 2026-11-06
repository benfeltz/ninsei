//! CPU Shopper, the RIG window: an issue of a mail-order magazine. Core lays out the pages
//! (`ninsei_core::shopper`): every page is the same size, and each block on a section page owns
//! a rectangle of its grid. This only draws them, through the shared book control
//! (`pager.rs`), in the desk's colours (newsprint: one ink on the theme's paper), and routes
//! Order clicks as `WbOut::Order`. Nothing here knows what kind of item it's selling.

use egui::{Align2, FontId, Rect, Response, RichText, Sense, Stroke, Ui, pos2, vec2};
use ninsei_core::book::Turn;
use ninsei_core::hub::{Save, Upgrade};
use ninsei_core::shopper::{
    self, AdDef, AdSize, Block, Cell, GRID_COLS, GRID_ROWS, Item, Listing, Page, ShopperDef, Stock,
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
/// An ad's foot row: the price and the Order button.
const FOOT: f32 = 32.0;

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
        // The page number is the magazine's own (its running head); the bar names the section.
        Page::Section { section, .. } => def
            .section(*section)
            .map_or(String::new(), |s| s.title.clone()),
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
        Page::Contents => contents(ui, rect, def, &cat, &pages),
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

/// Print: every word on the page is set in the proportional face, like a magazine. The bar
/// under the page and the window around it stay in the machine's own font.
fn text(t: impl Into<String>, size: f32) -> RichText {
    RichText::new(t)
        .family(egui::FontFamily::Proportional)
        .size(size)
        .color(ink())
}

fn bold(t: impl Into<String>, size: f32) -> RichText {
    text(t, size).strong()
}

fn small(t: impl Into<String>) -> RichText {
    text(t, size::SMALL).color(muted())
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
    leader_in(ui, left, right, false)
}

/// A dot-leader line; `greyed` prints the price (the right side) in the muted ink, for items
/// you can't order again.
fn leader_in(ui: &mut Ui, left: &str, right: &str, greyed: bool) -> Response {
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
    let rcol = if greyed { muted() } else { col };
    let rr = p.text(r.right_center(), Align2::RIGHT_CENTER, right, font, rcol);
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

/// IN THIS ISSUE: each section and its page.
fn toc_box(ui: &mut Ui, def: &ShopperDef, pages: &[Page]) -> Option<Turn> {
    let mut to = None;
    ui.label(small("IN THIS ISSUE"));
    for (s, i, n) in shopper::contents(pages) {
        let title = def.section(s).map_or("", |d| d.title.as_str());
        if leader(ui, title, &n.to_string()).clicked() {
            to = Some(Turn::To(i));
        }
    }
    to
}

/// The contents page: IN THIS ISSUE and the INDEX OF ADVERTISERS side by side, each as tall as
/// its lines, then the issue's FEATURES (the cover stories) filling the rest of the page.
fn contents(
    ui: &mut Ui,
    rect: Rect,
    def: &ShopperDef,
    cat: &[Listing],
    pages: &[Page],
) -> Option<Turn> {
    let rect = rect.shrink(MARGIN);
    let mut head = pager::area(ui, rect);
    head.label(bold(
        format!("{} · {} · {}", def.title, def.month, def.issue),
        size::HEADLINE,
    ));
    rule(&mut head, 2.0);
    let top = head.cursor().top() + GUTTER;
    let toc = shopper::contents(pages);
    let index = shopper::advertisers(def, cat, pages);
    // A box is its label, its leader lines and its border.
    // Each row is its own height plus egui's spacing between items.
    let gap = ui.spacing().item_spacing.y;
    let line = size::BODY + 6.0 + gap;
    let lines = toc.len().max(index.len()) as f32;
    let boxes_h = size::SMALL + 6.0 + gap + lines * line + 2.0 * (MARGIN + 2.0) + 4.0;
    let half = (rect.width() - GUTTER) / 2.0;
    let left = Rect::from_min_size(pos2(rect.left(), top), vec2(half, boxes_h));
    let right = Rect::from_min_size(pos2(left.right() + GUTTER, top), vec2(half, boxes_h));
    let mut to = None;
    boxed(ui, left, 2.0, |ui| to = toc_box(ui, def, pages));
    boxed(ui, right, 2.0, |ui| {
        ui.label(small("INDEX OF ADVERTISERS"));
        for (v, nums) in &index {
            let name = def.vendor(v).map_or(v.clone(), |d| d.name.clone());
            let nums: Vec<String> = nums.iter().map(u32::to_string).collect();
            leader(ui, &name, &nums.join(", "));
        }
    });
    let features = Rect::from_min_max(pos2(rect.left(), left.bottom() + GUTTER), rect.max);
    boxed(ui, features, 2.0, |ui| {
        ui.label(small("FEATURES"));
        ui.add_space(4.0);
        for c in &def.cover {
            let at = toc.iter().find(|(s, ..)| *s == c.section);
            let page = at.map_or(String::new(), |&(_, _, n)| n.to_string());
            if leader(ui, &c.headline, &page).clicked() {
                to = at.map(|&(_, i, _)| Turn::To(i));
            }
            ui.label(text(&c.line, size::BODY));
            ui.add_space(6.0);
        }
    });
    to
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

/// What the price spot says: the price (greyed when you own it), SOLD OUT or CALL NOW.
fn price_text(l: &Listing) -> String {
    match l.stock {
        Stock::Price(p) | Stock::Owned(p) => format!("{p}¢"),
        Stock::SoldOut => "SOLD OUT".into(),
        Stock::Call => "CALL NOW".into(),
    }
}

/// The ad's foot, pinned to the bottom of the ad like a print ad: the price in big type (or the
/// callout, with the list price under it) and the Order button, or why there isn't one.
fn order(ui: &mut Ui, data: &Data, save: &Save, l: &Listing, a: &AdDef, out: &mut Vec<WbOut>) {
    ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
        // One fixed-height row, centred, so big type never runs past the ad's border.
        let row = vec2(ui.available_width(), FOOT);
        let centred = egui::Layout::left_to_right(egui::Align::Center);
        ui.allocate_ui_with_layout(row, centred, |ui| match l.stock {
            Stock::Price(p) => {
                let blocker = save.order_blocker(data, &l.item);
                let resp = ui.add_enabled(
                    blocker.is_none(),
                    egui::Button::new(bold("ORDER", size::TITLE)),
                );
                if let Some(why) = blocker {
                    resp.on_disabled_hover_text(why);
                } else if resp.clicked() {
                    out.push(WbOut::Order(l.item.clone()));
                }
                ui.add_space(8.0);
                match &a.callout {
                    Some(c) => {
                        ui.label(bold(c, size::HEADLINE));
                        ui.label(small(format!("or {p}¢ list")));
                    }
                    None => {
                        ui.label(bold(format!("SPECIAL PRICE {p}¢"), size::HEADLINE));
                    }
                }
            }
            Stock::Owned(p) => {
                // The magazine doesn't know you have it: the price as printed, greyed, and an
                // ORDER button that won't take.
                ui.add_enabled(false, egui::Button::new(bold("ORDER", size::TITLE)))
                    .on_disabled_hover_text("you already have this.");
                ui.add_space(8.0);
                let price = match &a.callout {
                    Some(_) => format!("{p}¢"),
                    None => format!("SPECIAL PRICE {p}¢"),
                };
                ui.label(
                    RichText::new(price)
                        .family(egui::FontFamily::Proportional)
                        .size(size::HEADLINE)
                        .color(muted()),
                );
            }
            Stock::SoldOut => {
                ui.label(bold("SOLD OUT", size::HEADLINE));
            }
            Stock::Call => {
                ui.label(bold("CALL NOW", size::HEADLINE));
            }
        });
    });
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
    // The body: the copy on the left, the product's picture on the right (like the print
    // ads), with the price and ORDER pinned under both.
    let art = match &l.item {
        Item::Deck(id) => data
            .hardware
            .deck(id)
            .and_then(|d| d.art.as_deref())
            .and_then(|a| data.art(a)),
        _ => None,
    };
    let body = Rect::from_min_max(
        pos2(ui.max_rect().left(), ui.cursor().top()),
        ui.max_rect().max - vec2(0.0, FOOT + GUTTER),
    );
    let copy = match art {
        Some(art) => {
            let pic = Rect::from_min_max(
                pos2(body.left() + body.width() * 0.55, body.top()),
                body.max,
            );
            hero(ui, pic, &art.rows);
            Rect::from_min_max(body.min, pos2(pic.left() - GUTTER, body.bottom()))
        }
        None => body,
    };
    let mut c = pager::area(ui, copy);
    let ui_copy = &mut c;
    name_line(
        ui_copy,
        l,
        a.headline.as_deref().unwrap_or(&l.name),
        size::HOTLINE,
    );
    ui_copy.label(text(&l.blurb, size::BODY));
    if !l.detail.is_empty() {
        ui_copy.label(small(&l.detail));
    }
    ui_copy.add_space(4.0);
    bullets(ui_copy, &a.bullets);
    if let Some(n) = &a.note {
        ui_copy.label(small(n));
    }
    if let Some(v) = vendor.filter(|v| !v.terms.is_empty()) {
        ui_copy.add_space(6.0);
        ui_copy.label(bold("TERMS:", size::SMALL));
        for t in &v.terms {
            ui_copy.label(text(format!("★ {t}"), size::SMALL));
        }
    }
    order(ui, data, save, l, a, out);
}

/// A product picture (`data/art/`), as big as fits `r` at a whole-number scale, centred, in the
/// page's ink on its paper.
fn hero(ui: &mut Ui, r: Rect, rows: &[String]) {
    let w = rows.first().map_or(0, |r| r.chars().count()) as f32;
    let h = rows.len() as f32;
    if w == 0.0 || h == 0.0 {
        return;
    }
    let scale = (r.width() / w).min(r.height() / h).floor().max(1.0);
    let size = vec2(w, h) * scale;
    let at = r.center() - size / 2.0;
    let at = pos2(at.x.round(), at.y.round());
    crate::patterns::bitmap(ui.painter(), rows, at, scale, ink(), backdrop());
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
    let owned = matches!(l.stock, Stock::Owned(_));
    let resp = leader_in(ui, &name, &price_text(l), owned);
    let blocker = match l.stock {
        Stock::Price(_) | Stock::Call | Stock::Owned(_) => save.order_blocker(data, &l.item),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::home::Home;
    use crate::home::tests::deck;
    use crate::uitest::{Harness, click, key};
    use egui::{Key, Pos2, Rect, UiBuilder};
    use ninsei_core::hub::Tool;
    use ninsei_core::shopper::Item;

    /// One frame of `tool`'s window, sized to hold a book, with these events.
    fn frame(
        h: &mut Harness,
        home: &mut Home,
        data: &Data,
        save: &Save,
        tool: Tool,
        events: Vec<egui::Event>,
    ) -> Vec<WbOut> {
        let mut out = Vec::new();
        h.frame(events, |ui| {
            // The magazine at its page size; other tools (System) get the screen.
            let r = match tool {
                Tool::Rig => Rect::from_min_size(Pos2::ZERO, pager::SIZE + egui::vec2(16.0, 16.0)),
                _ => ui.max_rect(),
            };
            let mut child = ui.new_child(UiBuilder::new().max_rect(r));
            home.tool_body(&mut child, tool, data, save, &mut out);
        });
        out
    }

    fn press_on(h: &mut Harness, label: &str) -> Vec<egui::Event> {
        let at = h
            .find(label)
            .unwrap_or_else(|| panic!("no {label:?}: {:?}", h.texts()))
            .center();
        click(at)
    }

    #[test]
    fn the_bar_and_the_arrow_keys_turn_pages() {
        let data = Data::builtin().unwrap();
        let save = deck(&data);
        let (mut h, mut home) = (Harness::new(1280.0, 800.0), Home::new(Vec::new()));
        frame(&mut h, &mut home, &data, &save, Tool::Rig, Vec::new());
        assert_eq!(
            h.enabled("◀"),
            Some(false),
            "the cover has nothing before it"
        );
        assert_eq!(h.enabled("Home"), Some(false));
        let next = press_on(&mut h, "▶");
        frame(&mut h, &mut home, &data, &save, Tool::Rig, next);
        frame(&mut h, &mut home, &data, &save, Tool::Rig, Vec::new());
        assert_eq!(h.enabled("◀"), Some(true), "▶ turned the page");
        let home_btn = press_on(&mut h, "Home");
        frame(&mut h, &mut home, &data, &save, Tool::Rig, home_btn);
        frame(&mut h, &mut home, &data, &save, Tool::Rig, Vec::new());
        assert_eq!(h.enabled("◀"), Some(false), "Home goes back to the cover");
        // Keys work while the pointer is over the book.
        let over = vec![egui::Event::PointerMoved(egui::pos2(300.0, 200.0))];
        frame(&mut h, &mut home, &data, &save, Tool::Rig, over);
        frame(
            &mut h,
            &mut home,
            &data,
            &save,
            Tool::Rig,
            key(Key::ArrowRight),
        );
        frame(&mut h, &mut home, &data, &save, Tool::Rig, Vec::new());
        assert_eq!(h.enabled("◀"), Some(true), "→ turns the page");
        frame(
            &mut h,
            &mut home,
            &data,
            &save,
            Tool::Rig,
            key(Key::ArrowLeft),
        );
        frame(&mut h, &mut home, &data, &save, Tool::Rig, Vec::new());
        assert_eq!(h.enabled("◀"), Some(false), "← turns it back");
    }

    #[test]
    fn order_sends_the_item_and_owned_items_wont_take_an_order() {
        let data = Data::builtin().unwrap();
        let mut save = deck(&data);
        save.bank = 1_000_000;
        let (mut h, mut home) = (Harness::new(1280.0, 800.0), Home::new(Vec::new()));
        let count = shopper::layout(&data.shopper, &save.catalog(&data)).len();
        frame(&mut h, &mut home, &data, &save, Tool::Rig, Vec::new());
        // Leaf through until a page has an ORDER button that takes, and press it.
        let mut ordered = None;
        for _ in 0..count {
            if let Some(&r) = h.all("ORDER").first() {
                let out = frame(
                    &mut h,
                    &mut home,
                    &data,
                    &save,
                    Tool::Rig,
                    click(r.center()),
                );
                ordered = out.into_iter().find_map(|o| match o {
                    WbOut::Order(item) => Some(item),
                    _ => None,
                });
                break;
            }
            let next = press_on(&mut h, "▶");
            frame(&mut h, &mut home, &data, &save, Tool::Rig, next);
            frame(&mut h, &mut home, &data, &save, Tool::Rig, Vec::new());
        }
        let item = ordered.expect("an ORDER button sends its item");
        assert_eq!(
            save.order_blocker(&data, &item),
            None,
            "{item:?} can be bought"
        );
        // Once it's yours, the same button won't take.
        save.buy_item(&data, &item).unwrap();
        frame(&mut h, &mut home, &data, &save, Tool::Rig, Vec::new());
        let r = h.all("ORDER");
        let out = match r.first() {
            Some(r) => frame(
                &mut h,
                &mut home,
                &data,
                &save,
                Tool::Rig,
                click(r.center()),
            ),
            None => Vec::new(),
        };
        assert!(
            !out.iter()
                .any(|o| matches!(o, WbOut::Order(i) if *i == item)),
            "{item:?} is owned: no second order"
        );
    }

    #[test]
    fn a_bought_deck_waits_in_system_until_you_pick_it() {
        let data = Data::builtin().unwrap();
        let mut save = deck(&data);
        save.bank = 1_000_000;
        save.ensure_hardware(&data);
        let new = data
            .hardware
            .decks
            .iter()
            .find(|d| d.id != save.deck_def(&data).id)
            .unwrap();
        save.buy_item(&data, &Item::Deck(new.id.clone())).unwrap();
        assert_ne!(save.deck, new.id, "buying doesn't switch");
        let (mut h, mut home) = (Harness::new(1280.0, 800.0), Home::new(Vec::new()));
        frame(&mut h, &mut home, &data, &save, Tool::System, Vec::new());
        let pick = press_on(&mut h, &new.title());
        let out = frame(&mut h, &mut home, &data, &save, Tool::System, pick);
        assert!(
            out.iter()
                .any(|o| matches!(o, WbOut::Deck(d) if *d == new.id)),
            "picking it in System plugs it in"
        );
    }

    #[test]
    fn a_bought_monitor_waits_in_system_until_you_plug_it_in() {
        let data = Data::builtin().unwrap();
        let mut save = deck(&data);
        save.bank = 1_000_000;
        save.ensure_hardware(&data);
        let new = data
            .hardware
            .monitors
            .iter()
            .find(|m| m.id != save.monitor_def(&data).id)
            .unwrap();
        save.buy_item(&data, &Item::Monitor(new.id.clone()))
            .unwrap();
        assert_ne!(save.monitor, new.id, "buying doesn't plug it in");
        let (mut h, mut home) = (Harness::new(1280.0, 800.0), Home::new(Vec::new()));
        home.system = ninsei_core::system::Panel::Monitor;
        frame(&mut h, &mut home, &data, &save, Tool::System, Vec::new());
        let pick = press_on(&mut h, &new.title());
        let out = frame(&mut h, &mut home, &data, &save, Tool::System, pick);
        assert!(
            out.iter()
                .any(|o| matches!(o, WbOut::Monitor(m) if *m == new.id)),
            "picking it in System plugs it in"
        );
    }
}
