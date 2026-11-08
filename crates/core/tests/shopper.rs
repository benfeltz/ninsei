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
    // Owned, but still printed at its price: the magazine doesn't know your rig.
    let price = d.hardware.starter_deck().price;
    assert_eq!(starter.stock, Stock::Owned(price));
    assert_eq!(starter.list_price(), Some(price));
    assert_eq!(starter.price(), None, "an owned item can't be ordered");
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
                // A full-page ad has its page to itself, apart from a deck's own add-ons.
                let others: Vec<&Block> = blocks
                    .iter()
                    .map(|(b, _)| b)
                    .filter(|b| !matches!(b, Block::Ad { .. }))
                    .collect();
                assert!(others.len() <= 1, "a full-page ad is alone: {others:?}");
                for b in others {
                    let Block::List { listings, .. } = b else {
                        panic!("only a list rides with a full-page ad");
                    };
                    assert!(
                        listings
                            .iter()
                            .all(|&l| matches!(cat[l].item, Item::Addon(_)))
                    );
                }
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
    // Small sections share pages: the issue is shorter than one page per section plus one
    // per full-page ad.
    let full = d
        .shopper
        .ads
        .iter()
        .filter(|a| a.size == AdSize::Full)
        .count();
    assert!(
        numbers.len() < Section::ALL.len() + full - 1,
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
fn every_add_on_is_listed_but_only_yours_can_be_ordered() {
    let d = data();
    let mut s = fresh(5000);
    let cat = s.catalog(&d);
    let desks = s.desktops_owned(&d);
    for a in &d.addons {
        let l = cat
            .iter()
            .find(|l| l.item == Item::Addon(a.id.clone()))
            .expect("every add-on is listed");
        if desks.contains(&a.desktop) {
            assert_eq!(l.stock, Stock::Price(a.price), "{}", a.id);
        } else {
            // Listed with CALL NOW: no price, no order.
            assert_eq!(l.stock, Stock::Call, "{}", a.id);
            assert!(s.order_blocker(&d, &l.item).is_some());
            let bank = s.bank;
            assert!(s.buy_item(&d, &l.item).is_err());
            assert_eq!(s.bank, bank);
        }
    }
    let broke = fresh(10);
    assert!(
        broke
            .order_blocker(&d, &Item::Deck("hosaka".into()))
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
fn every_deck_has_a_full_page_with_its_picture_and_its_add_ons() {
    let d = data();
    let cat = fresh(0).catalog(&d);
    let pages = shopper::layout(&d.shopper, &cat);
    for deck in &d.hardware.decks {
        let art = deck.art.as_deref().expect("every deck has a picture");
        assert!(d.art(art).is_some(), "{art}");
        let item = Item::Deck(deck.id.clone());
        let page = pages
            .iter()
            .find_map(|p| {
                match p {
                Page::Section { blocks, .. }
                    if blocks.iter().any(|(b, _)| {
                        matches!(b, Block::Ad { listing, .. } if cat[*listing].item == item)
                    }) =>
                {
                    Some(blocks)
                }
                _ => None,
            }
            })
            .expect("the deck has a page");
        let decks_here = page
            .iter()
            .filter(|(b, _)| matches!(b, Block::Ad { listing, .. } if matches!(cat[*listing].item, Item::Deck(_))))
            .count();
        assert_eq!(decks_here, 1, "{} shares its page", deck.id);
        // Its add-ons ride on its page: the ones for its first firmware's desktop.
        let os = deck.firmware[0].desktop;
        let here: Vec<&str> = page
            .iter()
            .filter_map(|(b, _)| match b {
                Block::List { listings, .. } => Some(listings),
                _ => None,
            })
            .flatten()
            .filter_map(|&l| match &cat[l].item {
                Item::Addon(id) => Some(id.as_str()),
                _ => None,
            })
            .collect();
        for a in d.addons.iter().filter(|a| a.desktop == os) {
            if d.hardware
                .decks
                .iter()
                .position(|x| x.firmware[0].desktop == os)
                == d.hardware.decks.iter().position(|x| x.id == deck.id)
            {
                assert!(
                    here.contains(&a.id.as_str()),
                    "{} on {}'s page",
                    a.id,
                    deck.id
                );
            }
        }
    }
}

#[test]
fn bad_art_is_caught_on_load() {
    let ok = data::ArtDef::parse("x", "#.#\n.#.\n");
    assert!(ok.validate().is_ok());
    for (text, want) in [("#.#\n.#\n", "widths"), ("#x#\n", "`#`"), ("", "empty")] {
        let err = data::ArtDef::parse("x", text).validate().unwrap_err();
        assert!(err.contains(want), "{err}");
    }
    let hw = include_str!("../../../data/hub/hardware.ron")
        .replace("art: Some(\"deck-hosaka\")", "art: Some(\"nowhere\")");
    let err = Data::parse(Sources {
        hardware: &hw,
        ..Sources::builtin()
    })
    .unwrap_err();
    assert!(err.contains("nowhere"), "{err}");
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
        // Add-ons ride on their deck's own page, so only other deck pages may come between.
        let decks_only = |w: &[usize]| {
            (w[0] + 1..w[1]).all(|i| {
                matches!(&pages[i], Page::Section { sections, .. } if sections == &[Section::Decks])
            })
        };
        assert!(
            on.windows(2).all(|w| w[1] == w[0] + 1 || decks_only(w)),
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

#[test]
fn every_deck_in_the_data_is_listed_and_orderable() {
    let d = data();
    let mut s = fresh(0);
    let decks: Vec<_> = s
        .catalog(&d)
        .into_iter()
        .filter(|l| l.section == Section::Decks)
        .collect();
    // The magazine prints exactly the decks in the data, each once, in the data's order.
    let printed: Vec<Item> = decks.iter().map(|l| l.item.clone()).collect();
    let data_decks: Vec<Item> = d
        .hardware
        .decks
        .iter()
        .map(|m| Item::Deck(m.id.clone()))
        .collect();
    assert_eq!(printed, data_decks);
    // Every deck has its own full page.
    let def = &d.shopper;
    for l in &decks {
        assert_eq!(
            def.ad(&l.item).map(|a| a.size),
            Some(AdSize::Full),
            "{}",
            l.name
        );
    }
    // The cheapest deck you don't own orders once the bank holds its price.
    let cheapest = decks
        .iter()
        .filter(|l| matches!(l.stock, Stock::Price(_)))
        .min_by_key(|l| l.price())
        .unwrap();
    s.bank = cheapest.price().unwrap();
    assert_eq!(s.order_blocker(&d, &cheapest.item), None);
    s.buy_item(&d, &cheapest.item).unwrap();
    assert!(s.owns(&match &cheapest.item {
        Item::Deck(id) => ninsei_core::hardware::Hardware::Deck(id.clone()),
        _ => unreachable!(),
    }));
}

#[test]
fn a_deck_ad_prints_its_stats_and_nothing_else_does() {
    let d = data();
    let cat = fresh(0).catalog(&d);
    for deck in &d.hardware.decks {
        let l = cat
            .iter()
            .find(|l| l.item == Item::Deck(deck.id.clone()))
            .unwrap();
        assert_eq!(l.specs.len(), deck.stats.len(), "{}", deck.id);
        for (spec, row) in l.specs.iter().zip(&deck.stats) {
            assert_eq!(spec.line, row.line);
            assert_eq!((spec.ships, spec.max), (row.base, row.max), "{}", deck.id);
            let rig = d.rig.iter().find(|u| u.kind == row.line).unwrap();
            assert_eq!(spec.label, rig.label, "the magazine's word for the line");
            let unit = |n: u32| match rig.unit.as_str() {
                "" => n.to_string(),
                u => format!("{n} {u}"),
            };
            assert_eq!(spec.ships_text(), unit(row.base));
            assert_eq!(spec.max_text(), unit(row.max));
        }
        // CPU, RAM and HDD are always in the box.
        for u in [Upgrade::Cpu, Upgrade::Ram, Upgrade::Hdd] {
            assert!(l.specs.iter().any(|s| s.line == u), "{} {u:?}", deck.id);
        }
    }
    // A product's numbers, never the reader's rig: RIG levels don't move them.
    let mut rich = fresh(0);
    rich.rig.ram = 3;
    let again = rich.catalog(&d);
    for (a, b) in cat.iter().zip(&again) {
        assert_eq!(a.specs, b.specs, "{}", a.name);
    }
    for l in cat.iter().filter(|l| !matches!(l.item, Item::Deck(_))) {
        assert!(l.specs.is_empty(), "{} has no spec box", l.name);
    }
}

#[test]
fn a_vendors_small_print_runs_every_term_in() {
    let d = data();
    for v in &d.shopper.vendors {
        match v.small_print() {
            None => assert!(v.terms.is_empty(), "{}", v.id),
            Some(p) => {
                assert!(p.starts_with("TERMS: "), "{p}");
                for t in &v.terms {
                    assert!(p.contains(t.as_str()), "{}: {t}", v.id);
                }
            }
        }
    }
}
