//! CPU Shopper: the catalog comes from each item's own file, the magazine only dresses it, and
//! every listing lands on exactly one page.

use ninsei_core::hub::{Save, Upgrade};
use ninsei_core::shopper::{self, AdSize, Block, Cell, Item, Page, Section, Stock};
use ninsei_core::*;

fn data() -> Data {
    Data::builtin().unwrap()
}

fn fresh(bank: u32) -> Save {
    let mut s = Save {
        bank,
        ..Save::default()
    };
    s.ensure_hardware(&data());
    s
}

/// Every listing index the pages show, in order.
fn shown(pages: &[Page]) -> Vec<usize> {
    let mut out = Vec::new();
    for p in pages {
        if let Page::Section { blocks, .. } = p {
            for (b, _) in blocks {
                match b {
                    Block::Ad { listing, .. } => out.push(*listing),
                    Block::List { listings, .. } => out.extend(listings),
                }
            }
        }
    }
    out
}

#[test]
fn every_listing_is_on_exactly_one_page() {
    let d = data();
    for s in [fresh(0), {
        let mut s = fresh(5000);
        s.decks = d.hardware.decks.iter().map(|d| d.id.clone()).collect();
        s
    }] {
        let cat = s.catalog(&d);
        let pages = shopper::layout(&d.shopper, &cat);
        assert_eq!(pages.first(), Some(&Page::Cover));
        assert_eq!(pages[1], Page::Contents);
        assert!(matches!(pages.last(), Some(Page::Section { .. })));
        let mut seen = shown(&pages);
        seen.sort_unstable();
        assert_eq!(seen, (0..cat.len()).collect::<Vec<_>>());
    }
}

#[test]
fn the_catalog_comes_from_each_items_own_file() {
    let d = data();
    let s = fresh(0);
    let cat = s.catalog(&d);
    let has = |i: &Item| cat.iter().any(|l| &l.item == i);
    for m in &d.hardware.monitors {
        assert!(has(&Item::Monitor(m.id.clone())), "{}", m.id);
    }
    for k in &d.hardware.decks {
        assert!(has(&Item::Deck(k.id.clone())), "{}", k.id);
    }
    for u in &d.rig {
        assert!(has(&Item::Upgrade(u.kind)));
    }
    for p in &d.panes {
        assert!(has(&Item::Pane(p.id.clone())));
    }
    // Tools only with a shop block.
    assert!(has(&Item::Tool("nightcomm".into())));
    assert!(!has(&Item::Tool("hexed".into())));
    // Starters are in your rig; prices are the files' own.
    let deck = cat
        .iter()
        .find(|l| l.item == Item::Deck("onosendai".into()))
        .unwrap();
    assert_eq!(
        deck.stock,
        Stock::Price(d.hardware.deck("onosendai").unwrap().price)
    );
    let starter = cat
        .iter()
        .find(|l| l.item == Item::Deck(d.hardware.starter_deck().id.clone()))
        .unwrap();
    assert_eq!(starter.stock, Stock::Owned);
}

#[test]
fn a_new_tool_is_in_the_next_issue_without_an_ad() {
    // Give a tool a shop block in its own file, and nothing else.
    let tools = include_str!("../../../data/hub/tools.ron").replace(
        "intro: Some(\"icedb\")",
        "intro: Some(\"icedb\"), shop: Some((price: 75, vendor: \"d1rge\"))",
    );
    let d = Data::parse(Sources {
        tools: &tools,
        ..Sources::builtin()
    })
    .unwrap();
    let s = fresh(0);
    let cat = s.catalog(&d);
    let ix = cat
        .iter()
        .position(|l| l.item == Item::Tool("icedb".into()))
        .expect("listed");
    assert_eq!(cat[ix].stock, Stock::Price(75));
    assert_eq!(cat[ix].section, Section::Software);
    let pages = shopper::layout(&d.shopper, &cat);
    let in_list = pages.iter().any(|p| match p {
        Page::Section { blocks, .. } => blocks
            .iter()
            .any(|(b, _)| matches!(b, Block::List { listings, .. } if listings.contains(&ix))),
        _ => false,
    });
    assert!(in_list, "an item without an ad is a price-list line");
}

#[test]
fn deleting_an_ad_never_drops_the_item() {
    let d = data();
    let mut def = d.shopper.clone();
    def.ads.clear();
    let cat = fresh(0).catalog(&d);
    let pages = shopper::layout(&def, &cat);
    assert_eq!(shown(&pages).len(), cat.len());
    assert!(pages.iter().all(|p| match p {
        Page::Section { blocks, .. } => blocks.iter().all(|(b, _)| matches!(b, Block::List { .. })),
        _ => true,
    }));
}

#[test]
fn a_full_ad_has_its_page_and_pages_count_up() {
    let d = data();
    let cat = fresh(0).catalog(&d);
    let pages = shopper::layout(&d.shopper, &cat);
    for p in &pages {
        if let Page::Section { blocks, .. } = p {
            let full = blocks.iter().any(|(b, _)| match b {
                Block::Ad { ad, .. } => d.shopper.ads[*ad].size == AdSize::Full,
                _ => false,
            });
            if full {
                assert_eq!(blocks.len(), 1, "a full-page ad is alone");
            }
        }
    }
    // Printed pages count up through the issue; the first section opens on its own page,
    // and a section that shares a page is listed in the contents at that page.
    let numbers: Vec<u32> = pages
        .iter()
        .filter_map(|p| match p {
            Page::Section { number, .. } => Some(*number),
            _ => None,
        })
        .collect();
    assert!(numbers.windows(2).all(|w| w[0] < w[1]), "{numbers:?}");
    let toc = shopper::contents(&pages);
    let first = d.shopper.sections[0].section;
    assert_eq!(toc[0].2, d.shopper.section(first).unwrap().page);
    for (section, at, number) in &toc {
        let Page::Section {
            sections,
            number: n,
            ..
        } = &pages[*at]
        else {
            panic!("contents points at a section page");
        };
        assert!(sections.contains(section));
        assert_eq!(n, number);
    }
    // Small sections share pages: the issue is shorter than one page per section.
    assert!(
        numbers.len() < Section::ALL.len() + 2,
        "{} pages",
        numbers.len()
    );
    // The full ad's page belongs to its advertiser.
    let ono = pages.iter().find_map(|p| match p {
        Page::Section { vendor, blocks, .. }
            if blocks.iter().any(|(b, _)| {
                matches!(b, Block::Ad { listing, .. }
                if cat[*listing].item == Item::Deck("onosendai".into()))
            }) =>
        {
            Some(vendor.clone())
        }
        _ => None,
    });
    assert_eq!(ono.as_deref(), Some("onosendai"));
}

#[test]
fn the_index_lists_who_sells_and_where() {
    let d = data();
    let cat = fresh(0).catalog(&d);
    let pages = shopper::layout(&d.shopper, &cat);
    let index = shopper::advertisers(&d.shopper, &cat, &pages);
    let page_of = |id: &str| index.iter().find(|(v, _)| v == id).map(|(_, p)| p.clone());
    // Each vendor is indexed on the page its goods are on.
    let toc = shopper::contents(&pages);
    let at = |sec: Section| toc.iter().find(|(s, ..)| *s == sec).unwrap().2;
    assert!(page_of("d1rge").is_some_and(|p| p.contains(&at(Section::Software))));
    assert!(page_of("chiba").is_some_and(|p| p.contains(&at(Section::Upgrades))));
    assert!(index.iter().all(|(_, p)| !p.is_empty()));
}

#[test]
fn one_order_buys_anything_through_its_own_rules() {
    let d = data();
    let mut s = fresh(5000);
    let items = [
        Item::Upgrade(Upgrade::Ram),
        Item::Monitor("amber".into()),
        Item::Deck("onosendai".into()),
        Item::Pane("net".into()),
        Item::Tool("nightcomm".into()),
    ];
    for item in &items {
        let before = s.bank;
        let price = s
            .catalog(&d)
            .into_iter()
            .find(|l| &l.item == item)
            .and_then(|l| l.price())
            .unwrap();
        assert_eq!(s.order_blocker(&d, item), None, "{item:?}");
        s.buy_item(&d, item).unwrap();
        assert_eq!(s.bank, before - price, "{item:?}");
    }
    assert_eq!(s.rig.level(Upgrade::Ram), 1);
    assert!(s.has_tool("nightcomm"));
    assert!(s.decks.contains(&"onosendai".to_string()));
    // Once owned, it says so and won't sell twice.
    let tool = Item::Tool("nightcomm".into());
    assert!(s.order_blocker(&d, &tool).is_some());
    assert!(s.buy_item(&d, &tool).is_err());
    // An add-on for a desktop you now run (AnarchyOS, via the Ono-Sendai).
    let addon = Item::Addon("workspaces".into());
    assert!(s.catalog(&d).iter().any(|l| l.item == addon));
    s.buy_item(&d, &addon).unwrap();
    assert!(s.has_addon("workspaces"));
}

#[test]
fn add_ons_are_only_for_desktops_you_run_and_a_broke_bank_is_refused() {
    let d = data();
    let s = fresh(10);
    let cat = s.catalog(&d);
    let desks = s.desktops_owned(&d);
    for l in cat.iter().filter(|l| l.section == Section::Addons) {
        let Item::Addon(id) = &l.item else { panic!() };
        let a = d.addons.iter().find(|a| &a.id == id).unwrap();
        assert!(desks.contains(&a.desktop));
    }
    assert!(
        s.order_blocker(&d, &Item::Deck("hosaka".into()))
            .is_some_and(|e| e.contains("bank"))
    );
    let mut maxed = fresh(0);
    maxed.rig.cpu = d.rig.iter().find(|u| u.kind == Upgrade::Cpu).unwrap().max;
    let cpu = maxed
        .catalog(&d)
        .into_iter()
        .find(|l| l.item == Item::Upgrade(Upgrade::Cpu))
        .unwrap();
    assert_eq!(cpu.stock, Stock::SoldOut);
}

#[test]
fn a_bad_magazine_is_caught_on_load() {
    let src = include_str!("../../../data/hub/shopper.ron");
    let parse = |shopper: &str| {
        Data::parse(Sources {
            shopper,
            ..Sources::builtin()
        })
    };
    assert!(parse(src).is_ok());
    let cases = [
        (src.replace("Deck(\"hosaka\")", "Deck(\"nope\")"), "isn't for sale"),
        (src.replace("1-800-CHIBA-24", "555-0199"), "dialable"),
        (
            src.replace("(section: Panes,    title: \"Run panes\",           page: 600, vendor: \"chiba\"),", ""),
            "Panes",
        ),
        (src.replace("vendor: \"deskacc\"),", "vendor: \"nobody\"),"), "nobody"),
    ];
    for (bad, want) in cases {
        let err = parse(&bad).unwrap_err();
        assert!(err.contains(want), "{err}");
    }
    let tools = include_str!("../../../data/hub/tools.ron").replace(
        "vendor: \"d1rge\", review: true",
        "vendor: \"ghost\", review: true",
    );
    let err = Data::parse(Sources {
        tools: &tools,
        ..Sources::builtin()
    })
    .unwrap_err();
    assert!(err.contains("ghost"), "{err}");
}

#[test]
fn every_block_fills_its_own_cells_on_the_page() {
    let d = data();
    let mut rich = fresh(5000);
    rich.decks = d.hardware.decks.iter().map(|d| d.id.clone()).collect();
    // With and without ads: every block sits inside the grid and none overlap.
    let mut bare = d.shopper.clone();
    bare.ads.clear();
    for def in [&d.shopper, &bare] {
        let cat = rich.catalog(&d);
        for p in shopper::layout(def, &cat) {
            let Page::Section { blocks, .. } = p else {
                continue;
            };
            let cells: Vec<Cell> = blocks.iter().map(|(_, c)| *c).collect();
            for c in &cells {
                assert!(c.cols >= 1 && c.rows >= 1);
                assert!(c.col + c.cols <= shopper::GRID_COLS);
                assert!(c.row + c.rows <= shopper::GRID_ROWS);
            }
            for (i, a) in cells.iter().enumerate() {
                for b in &cells[i + 1..] {
                    let apart = a.col + a.cols <= b.col
                        || b.col + b.cols <= a.col
                        || a.row + a.rows <= b.row
                        || b.row + b.rows <= a.row;
                    assert!(apart, "{a:?} overlaps {b:?}");
                }
            }
            for (b, c) in &blocks {
                if let Block::List { listings, .. } = b {
                    assert!(
                        listings.len() <= shopper::list_lines(c.rows),
                        "a list fits its cells"
                    );
                }
            }
        }
    }
}

#[test]
fn a_section_never_runs_backwards_across_pages() {
    let d = data();
    let cat = fresh(5000).catalog(&d);
    let pages = shopper::layout(&d.shopper, &cat);
    // Once a section has gone to a later page, no earlier page gets more of it.
    for s in Section::ALL {
        let on: Vec<usize> = pages
            .iter()
            .enumerate()
            .filter(|(_, p)| matches!(p, Page::Section { sections, .. } if sections.contains(&s)))
            .map(|(i, _)| i)
            .collect();
        assert!(
            on.windows(2).all(|w| w[1] == w[0] + 1),
            "{s:?} on pages {on:?}"
        );
    }
    // Both software listings end up on one page together.
    let soft: Vec<usize> = pages
        .iter()
        .enumerate()
        .filter(|(_, p)| {
            matches!(p, Page::Section { blocks, .. }
            if blocks.iter().any(|(b, _)| matches!(b, Block::Ad { listing, .. }
                if cat[*listing].section == Section::Software)))
        })
        .map(|(i, _)| i)
        .collect();
    assert_eq!(soft.len(), 1, "software ads share a page: {soft:?}");
}
