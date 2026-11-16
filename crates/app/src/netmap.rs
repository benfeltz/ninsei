//! The main editor pane: the network. Three views share what's visible, what's clickable and
//! what each node is called: floating isometric wireframes (`netmap_iso`), a flat technical
//! draft (`netmap_draft`) or a 1-bit MacPaint picture (`netmap_paint`). Returns the node the
//! player clicked.

use egui::{Color32, Rect, Sense, Ui};
use ninsei_core::netview::{MapNode, MapView};
use ninsei_core::{Game, NodeKind, Phase};

use crate::theme::Palette;

/// The map's animation timings and strengths, in one place. Display numbers, not balance.
pub mod anim {
    /// How long a node's ICE takes to blow apart when it falls.
    pub const BURST_SECS: f64 = 1.0;
    /// How far the pieces fly, in shape radii.
    pub const BURST_SPREAD: f32 = 1.6;
    /// How long the map takes to build itself, layer by layer, when a run starts.
    pub const SWEEP_SECS: f64 = 0.5;
    /// Trace (%) above which the grid's pulse quickens.
    pub const TRACE_FAST: u32 = 75;
    /// The grid's pulse, in radians a second: calm, then fast.
    pub const PULSE_SLOW: f32 = 1.6;
    pub const PULSE_FAST: f32 = 5.5;
    /// A hovered route lifts this far (world units) and spins this many times faster.
    pub const HOVER_LIFT: f32 = 0.18;
    pub const HOVER_SPIN: f32 = 3.0;
}

/// What the map remembers between frames: when this network appeared, and which ICE has
/// fallen since, so new derezzes can burst. Kept in egui's memory, not in the game.
#[derive(Clone, Default)]
struct Memory {
    /// Seed, inside a subnet or not, and node count: a new network means a fresh map.
    key: Option<(u64, bool, usize)>,
    hops: u32,
    start: f64,
    cleared: Vec<bool>,
    /// (node, time it fell).
    bursts: Vec<(usize, f64)>,
}

/// This frame's view of the map's animations.
pub struct Anim {
    /// Seconds, for spins, pulses and blinks.
    pub t: f64,
    /// Seconds since this network appeared.
    age: f64,
    bursts: Vec<(usize, f64)>,
}

impl Anim {
    /// Reads and updates the map's memory for this frame.
    pub fn frame(ui: &Ui, game: &Game) -> Anim {
        let t = ui.input(|i| i.time);
        let id = ui.id().with("netmap-anim");
        let net = &game.net;
        let key = (game.seed, game.in_subnet(), net.nodes.len());
        let now: Vec<bool> = net.nodes.iter().map(|n| n.cleared).collect();
        let mem = ui.ctx().data_mut(|d| {
            let m: &mut Memory = d.get_temp_mut_or_default(id);
            if m.key != Some(key) || game.hops < m.hops {
                *m = Memory {
                    key: Some(key),
                    hops: game.hops,
                    start: t,
                    cleared: now.clone(),
                    bursts: Vec::new(),
                };
            }
            for (i, n) in net.nodes.iter().enumerate() {
                let was = m.cleared.get(i).copied().unwrap_or(true);
                if n.cleared && !was && matches!(n.kind, NodeKind::Ice | NodeKind::BlackIce) {
                    m.bursts.push((i, t));
                }
            }
            m.bursts.retain(|&(_, at)| t - at < anim::BURST_SECS);
            m.cleared = now;
            m.hops = game.hops;
            m.clone()
        });
        Anim {
            t,
            age: t - mem.start,
            bursts: mem.bursts,
        }
    }

    /// How far node `id` is through blowing apart (0..1), if it's bursting.
    pub fn burst(&self, id: usize) -> Option<f32> {
        self.bursts
            .iter()
            .find(|&&(n, _)| n == id)
            .map(|&(_, at)| (((self.t - at) / anim::BURST_SECS) as f32).clamp(0.0, 1.0))
    }

    /// How built a layer is (0..1) as the map sweeps in at the start of a run.
    pub fn reveal(&self, layer: usize, layers: usize) -> f32 {
        let at = anim::SWEEP_SECS * layer as f64 / layers.max(1) as f64;
        (((self.age - at) / (anim::SWEEP_SECS / 3.0)) as f32).clamp(0.0, 1.0)
    }

    /// Whether anything is still moving that needs another frame.
    pub fn busy(&self) -> bool {
        !self.bursts.is_empty() || self.age < anim::SWEEP_SECS * 1.5
    }

    /// Trace pressure: how strongly the grid lights up (0..1), pulsing faster past
    /// `TRACE_FAST`.
    pub fn pressure(&self, trace: u32) -> f32 {
        let f = (trace as f32 / 100.0).clamp(0.0, 1.0).powf(1.5);
        let speed = if trace >= anim::TRACE_FAST {
            anim::PULSE_FAST
        } else {
            anim::PULSE_SLOW
        };
        f * (0.7 + 0.3 * (self.t as f32 * speed).sin())
    }
}

/// Where a callout of `size` goes: the first of `spots` (rects' centres, in the order a map
/// prefers them) that lies inside `within` and clear of everything in `avoid`; else the first
/// inside `within`, else the first. Whole pixels.
pub fn place_clear(spots: &[egui::Pos2], size: egui::Vec2, avoid: &[Rect], within: Rect) -> Rect {
    let rects: Vec<Rect> = spots
        .iter()
        .map(|&c| {
            let r = Rect::from_center_size(c, size);
            Rect::from_min_size(r.min.round(), r.size().round())
        })
        .collect();
    let inside = |r: &&Rect| within.contains_rect(**r);
    rects
        .iter()
        .filter(inside)
        .find(|r| !avoid.iter().any(|a| a.expand(2.0).intersects(**r)))
        .or_else(|| rects.iter().find(inside))
        .or(rects.first())
        .copied()
        .unwrap_or(Rect::NOTHING)
}

/// Mixes two theme colours.
pub fn mix(a: Color32, b: Color32, k: f32) -> Color32 {
    let k = k.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * k).round() as u8;
    Color32::from_rgba_premultiplied(
        m(a.r(), b.r()),
        m(a.g(), b.g()),
        m(a.b(), b.b()),
        m(a.a(), b.a()),
    )
}

/// The BBS node in its board's colours: the board's frame and label CGA colours, mapped onto
/// the theme the same way the BBS window's phosphor mode does. (edge, label)
pub fn board_ink(game: &Game, p: &Palette) -> (Color32, Color32) {
    let l = &game.bbs().look;
    let pick = |c: u8, or: Color32| {
        // Never fainter than `dim`, so a dark board colour still reads as a lit node.
        match crate::bbs_view::mono(c, p) {
            m if m == Color32::TRANSPARENT => or,
            m if m == p.faint => p.dim,
            m => m,
        }
    };
    (pick(l.frame, p.fg), pick(l.label, p.bright))
}

pub fn show(ui: &mut Ui, game: &Game, p: &Palette, view: MapView) -> Option<usize> {
    match view {
        MapView::Iso => crate::netmap_iso::show(ui, game, p),
        MapView::Draft => crate::netmap_draft::show(ui, game, p),
        MapView::Paint => crate::netmap_paint::show(ui, game, p),
    }
}

pub use ninsei_core::netview::{target, visible};

/// A filled arrowhead centred on `at`, pointing along `dir`, `len` long: how every map style
/// marks a one-way link (and Draft a live route's end). Snapped to whole pixels.
pub fn arrowhead(painter: &egui::Painter, at: egui::Pos2, dir: egui::Vec2, len: f32, c: Color32) {
    let d = dir.normalized();
    let n = egui::vec2(-d.y, d.x);
    let tip = at + d * (len / 2.0);
    let back = at - d * (len / 2.0);
    let snap = |q: egui::Pos2| egui::pos2(q.x.round(), q.y.round());
    painter.add(egui::Shape::convex_polygon(
        vec![
            snap(tip),
            snap(back + n * (len / 2.0)),
            snap(back - n * (len / 2.0)),
        ],
        c,
        egui::Stroke::NONE,
    ));
}

pub fn kind(game: &Game, id: usize) -> MapNode {
    MapNode::of(&game.net, id, &game.data)
}

/// Makes a node's screen area clickable when it's a route, with a tooltip. Returns the node
/// if it was clicked, and whether it's hovered.
pub fn interact(ui: &mut Ui, game: &Game, id: usize, r: Rect) -> (Option<usize>, bool) {
    let can_go = game.reachable().contains(&id);
    let sense = if can_go {
        Sense::click()
    } else {
        Sense::hover()
    };
    let resp = ui.interact(r, ui.id().with(("node", id)), sense);
    let hovered = resp.hovered();
    let Some(tip) = ninsei_core::netview::route_tip(game, id).filter(|_| can_go) else {
        return (None, hovered);
    };
    let resp = resp.on_hover_text(tip);
    (resp.clicked().then_some(id), hovered)
}

/// The line along the bottom: what to do now.
pub fn hint(game: &Game) -> &'static str {
    match game.phase {
        Phase::Exploring if game.in_subnet() => {
            "inside a subnet. the UPLINK takes you back to the main network"
        }
        Phase::Exploring => "click a lit node (or `jack <n>`) to route there",
        Phase::Combat => "ICE holds the line. break it in target.ice",
        Phase::Encounter => "someone wants to talk",
        Phase::Flatlined => "FLATLINED. `newrun` to jack in again",
        Phase::JackedOut => "JACKED OUT. `newrun` to go deeper",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::uitest::{Harness, click};
    use ninsei_core::netview;
    use ninsei_core::{Action, Data};

    const VIEWS: [MapView; 3] = [MapView::Iso, MapView::Draft, MapView::Paint];

    /// A bare run standing one layer in on `seed`, every node but the ends an empty cleared
    /// vault, every link two way.
    fn one_in(seed: u64) -> Game {
        let mut g = Game::new(Data::builtin().unwrap(), seed);
        g.net.orient(seed, 0);
        for n in &mut g.net.nodes {
            if !matches!(n.kind, NodeKind::Entry | NodeKind::Exit) {
                (n.kind, n.ice, n.cleared) = (NodeKind::Vault, None, true);
            }
        }
        let a = g.net.nodes[g.at].links[0];
        g.apply(Action::Move(a));
        g
    }

    fn palette(id: &str) -> Palette {
        let d = Data::builtin().unwrap();
        Palette::from_def(d.themes.iter().find(|t| t.id == id).unwrap())
    }

    fn spot(view: MapView, rect: Rect, g: &Game, id: usize) -> egui::Pos2 {
        match view {
            MapView::Iso => crate::netmap_iso::spot_in(rect, g, id),
            MapView::Draft => crate::netmap_draft::spot_in(rect, g, id),
            MapView::Paint => crate::netmap_paint::spot_in(rect, g, id),
        }
    }

    /// Draws `view` until the map has swept in, then once more with a click on node `id`:
    /// what the map says was clicked, and the last frame.
    fn click_on(view: MapView, g: &Game, id: usize) -> (Option<usize>, Harness) {
        let p = palette("amber");
        let mut h = Harness::new(1000.0, 640.0);
        let mut rect = Rect::NOTHING;
        for _ in 0..12 {
            h.frame(Vec::new(), |ui| {
                rect = ui.available_rect_before_wrap();
                show(ui, g, &p, view);
            });
        }
        let at = spot(view, rect, g, id);
        let mut got = None;
        h.frame(click(at), |ui| {
            got = got.or(show(ui, g, &p, view));
        });
        (got, h)
    }

    #[test]
    fn a_click_on_the_node_you_came_from_moves_back_there_on_every_map() {
        for view in VIEWS {
            for seed in 0..4 {
                let mut g = one_in(seed);
                let entry = g.net.entry();
                assert!(g.reachable().contains(&entry));
                let (got, _) = click_on(view, &g, entry);
                assert_eq!(got, Some(entry), "{view:?} seed {seed}");
                g.apply(Action::Move(entry));
                assert_eq!(g.at, entry, "{view:?}: the click's move goes back");
            }
        }
    }

    #[test]
    fn a_one_way_link_back_takes_no_click_on_any_map() {
        for view in VIEWS {
            let mut g = one_in(2);
            let (entry, here) = (g.net.entry(), g.at);
            g.net.nodes[entry].one_way = vec![here];
            assert!(!g.reachable().contains(&entry));
            let (got, _) = click_on(view, &g, entry);
            assert_eq!(got, None, "{view:?}");
            // A route forward still takes one.
            let ahead = g.net.nodes[here].links[0];
            assert_eq!(click_on(view, &g, ahead).0, Some(ahead), "{view:?}");
        }
    }

    /// Filled triangles painted last frame, with their fill.
    fn triangles(h: &Harness) -> Vec<Color32> {
        h.painted()
            .iter()
            .filter_map(|s| match s {
                egui::Shape::Path(p) if p.points.len() == 3 && p.closed => Some(p.fill),
                _ => None,
            })
            .collect()
    }

    /// Draws `view` in theme `theme` until it has swept in; the last frame.
    fn drawn(view: MapView, g: &Game, theme: &str) -> Harness {
        let p = palette(theme);
        let mut h = Harness::new(1000.0, 640.0);
        for _ in 0..12 {
            h.frame(Vec::new(), |ui| {
                show(ui, g, &p, view);
            });
        }
        h
    }

    #[test]
    fn every_map_marks_a_one_way_link_with_an_arrow_in_the_theme_s_ink() {
        for theme in ["amber", "synthwave"] {
            let p = palette(theme);
            let roles = [p.faint, p.dim, p.fg, p.bright];
            for view in VIEWS {
                let two = one_in(5);
                let mut one = two.clone();
                let here = one.at;
                one.net.nodes[here].one_way = one.net.nodes[here].links.clone();
                let n = one.net.nodes[here].links.len();
                let (before, after) = (
                    triangles(&drawn(view, &two, theme)),
                    triangles(&drawn(view, &one, theme)),
                );
                assert_eq!(
                    after.len(),
                    before.len() + n,
                    "{view:?} {theme}: an arrow on each one-way link out of here"
                );
                // What the arrows added, colour by colour, is the theme's ink.
                let mut new = after.clone();
                for c in &before {
                    if let Some(i) = new.iter().position(|x| x == c) {
                        new.swap_remove(i);
                    }
                }
                assert!(
                    new.iter().all(|c| roles.contains(c)),
                    "{view:?} {theme}: arrows in the theme's ink: {new:?}"
                );
            }
        }
    }

    /// `one_in(seed)` with the net's AI awake on node `at`; `now` false moves it out of sight,
    /// leaving `at` where it was last known.
    fn with_ai(seed: u64, at: usize, now: bool) -> Game {
        use ninsei_core::boss::Hunter;
        use ninsei_core::game::BossState;
        let d = Data::builtin().unwrap();
        let ice = d.boss_for(1, 0).unwrap();
        let kit = ninsei_core::Kit {
            tools: ninsei_core::data::TOOLS
                .iter()
                .map(|t| t.to_string())
                .collect(),
            boss: Some((ice, 25)),
            ..ninsei_core::Kit::default()
        };
        let mut g = Game::start(d, seed, &kit);
        g.net.orient(seed, 0);
        for n in &mut g.net.nodes {
            if !matches!(n.kind, NodeKind::Entry | NodeKind::Exit) {
                (n.kind, n.ice, n.cleared) = (NodeKind::Vault, None, true);
            }
        }
        let a = g.net.nodes[g.at].links[0];
        g.apply(Action::Move(a));
        let far = ninsei_core::boss::exit(&g.net).unwrap();
        let mut h = Hunter::wake(ice, &g.data.ice[ice], if now { at } else { far }, g.at);
        h.prog = ninsei_core::basic::parse_with("10 HOLD", g.data.ice[ice].vocab());
        let b = g.boss.as_mut().unwrap();
        b.state = BossState::Awake(h);
        b.known = Some(at);
        g
    }

    #[test]
    fn every_map_draws_the_ai_as_the_node_it_holds_and_captions_where_it_was() {
        for theme in ["amber", "synthwave"] {
            for view in VIEWS {
                let quiet = one_in(3);
                let next = quiet.net.nodes[quiet.at].links[0];
                let seen = with_ai(3, next, true);
                let last = with_ai(3, next, false);
                let (m_now, m_last) = (
                    netview::ai_mark(&seen).unwrap(),
                    netview::ai_mark(&last).unwrap(),
                );
                assert!(m_now.now && !m_last.now && m_last.node == next);
                let said = |g: &Game| -> Vec<String> {
                    drawn(view, g, theme)
                        .painted_text()
                        .into_iter()
                        .map(|(t, _)| t)
                        .collect()
                };
                // Where it is: its name where the node's tag goes, and no caption beside it.
                assert_eq!(netview::ai_caption(&m_now, view), None);
                let now = said(&seen);
                assert!(now.contains(&m_now.label), "{view:?} {theme}: {now:?}");
                assert!(
                    !now.iter().any(|t| t.contains("is on this card")),
                    "{view:?} {theme}: {now:?}"
                );
                let last_cap = netview::ai_caption(&m_last, view).unwrap();
                assert!(
                    said(&last).contains(&last_cap),
                    "{view:?} {theme}: {last_cap}"
                );
                let none = said(&quiet);
                assert!(
                    !none
                        .iter()
                        .any(|t| t.contains(&m_now.label) || t == &last_cap),
                    "{view:?} {theme}: nothing to mark"
                );
            }
        }
    }

    #[test]
    fn every_map_marks_an_ai_heard_in_the_fog_by_name() {
        for view in VIEWS {
            let quiet = one_in(3);
            let next = quiet.net.nodes[quiet.at].links[0];
            let far = quiet.net.nodes[next].links[0];
            let mut g = with_ai(3, far, true);
            g.boss.as_mut().unwrap().last = vec![ninsei_core::boss::Step {
                line: 10,
                did: ninsei_core::boss::Did::Scanned {
                    from: far,
                    reach: 9,
                    found: true,
                },
            }];
            let m = netview::ai_mark(&g).unwrap();
            assert!(m.now && !m.held, "{view:?}: {m:?}");
            let cap = netview::ai_caption(&m, view).unwrap();
            let said: Vec<String> = drawn(view, &g, "amber")
                .painted_text()
                .into_iter()
                .map(|(t, _)| t)
                .collect();
            assert!(said.contains(&cap), "{view:?}: {said:?}");
        }
    }

    #[test]
    fn the_draft_never_stamps_a_node_the_ai_holds_void() {
        let quiet = one_in(3);
        let next = quiet.net.nodes[quiet.at].links[0];
        let mut g = with_ai(3, next, true);
        // ICE beaten there before; then the AI walked onto it in sight.
        let ice = g.data.ice.iter().position(|i| !i.boss).unwrap();
        let n = &mut g.net.nodes[next];
        (n.kind, n.ice, n.cleared) = (NodeKind::Ice, Some(ice), true);
        let said: Vec<String> = drawn(MapView::Draft, &g, "amber")
            .painted_text()
            .into_iter()
            .map(|(t, _)| t)
            .collect();
        assert!(!said.iter().any(|t| t == "VOID"), "{said:?}");
        // Without the AI there, the beaten ICE is stamped.
        let mut bare = g.clone();
        bare.boss = None;
        let said: Vec<String> = drawn(MapView::Draft, &bare, "amber")
            .painted_text()
            .into_iter()
            .map(|(t, _)| t)
            .collect();
        assert!(said.iter().any(|t| t == "VOID"), "{said:?}");
    }

    #[test]
    fn the_paint_writes_the_ai_s_name_in_the_node_it_holds_never_under_it() {
        let mut checked = 0;
        for seed in 0..4 {
            let g = one_in(seed);
            let mid: Vec<usize> = g.net.layers[1..3].iter().flatten().copied().collect();
            for at in mid {
                let mut a = with_ai(seed, at, true);
                let m = netview::ai_mark(&a).unwrap();
                if !m.holds(at) {
                    continue;
                }
                checked += 1;
                a.net.nodes[at].name = "self-0000".into();
                let label = m.label;
                let painted = drawn(MapView::Paint, &a, "amber").painted_text();
                let find = |t: &str| painted.iter().find(|(s, _)| s == t).map(|(_, r)| *r);
                let (name, host) = (find(&label).unwrap(), find("self-0000").unwrap());
                assert!(
                    name.bottom() < host.top(),
                    "seed {seed} node {at}: {name:?} over or under {host:?}"
                );
            }
        }
        assert!(checked > 3, "{checked}");
    }

    #[test]
    fn the_ai_s_caption_stays_on_the_map_at_its_edges() {
        let bounds = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 640.0));
        for view in VIEWS {
            let g = one_in(3);
            // The left column and the top of every column: the edges a caption could leave by.
            let mut edge: Vec<usize> = g.net.layers[0].clone();
            edge.extend(g.net.layers.iter().filter_map(|l| l.first().copied()));
            for at in edge {
                let a = with_ai(3, at, false);
                let m = netview::ai_mark(&a).unwrap();
                let cap = netview::ai_caption(&m, view).unwrap();
                let h = drawn(view, &a, "amber");
                let r = h
                    .painted_text()
                    .into_iter()
                    .find(|(t, _)| *t == cap)
                    .map(|(_, r)| r)
                    .unwrap_or_else(|| panic!("{view:?}: no {cap}"));
                assert!(
                    bounds.contains_rect(r),
                    "{view:?} node {at}: {cap} at {r:?}"
                );
            }
        }
    }

    #[test]
    fn the_ai_s_callout_keeps_off_the_other_nodes_labels() {
        for view in [MapView::Draft, MapView::Paint] {
            for seed in 0..4 {
                let g = one_in(seed);
                // Every node in the second and third layers: the crowded middle.
                let mid: Vec<usize> = g.net.layers[1..3].iter().flatten().copied().collect();
                for at in mid {
                    let mut a = with_ai(seed, at, false);
                    a.net.nodes[at].name = "self-0000".into();
                    let m = netview::ai_mark(&a).unwrap();
                    let cap = netview::ai_caption(&m, view).unwrap();
                    let painted = drawn(view, &a, "amber").painted_text();
                    let r = painted.iter().find(|(t, _)| *t == cap).unwrap().1;
                    // Whatever else is written on the map, but the node's own words.
                    let own = [
                        a.net.nodes[at].name.clone(),
                        at.to_string(),
                        netview::tag(&a, at),
                    ];
                    for (t, other) in &painted {
                        if *t == cap || own.contains(t) || t.starts_with("click") || t == "_" {
                            continue;
                        }
                        assert!(
                            !r.intersects(*other),
                            "{view:?} seed {seed} node {at}: {cap} {r:?} over {t:?} {other:?}"
                        );
                    }
                }
            }
        }
    }
}
