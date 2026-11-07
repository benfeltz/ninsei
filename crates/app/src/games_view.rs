//! A game's window: core's engine (`ninsei_core::games`) on the kit's cell grid. The field in a
//! frame, the game's own status line under it; the engine ticks at its row's `tick_ms` on the
//! frames' own time (no clock), keys reach it while its window has the keyboard, and a click
//! lands on a cell. Every cell reads by its shape (a glyph, a fill, a pattern or inverse),
//! never by colour alone, so a game plays on a mono tube.

use egui::{Color32, Key, Pos2, Rect, Sense, Shape, Stroke, Ui, pos2, vec2};
use ninsei_core::Data;
use ninsei_core::data::GameDef;
use ninsei_core::games::{Arcade, Cell, Input};

use crate::home::{accent, backdrop, ink, muted};
use crate::kit::{Cells, Fit};

/// A game in its window: the engine, and the time it hasn't ticked yet.
pub struct Session {
    pub game: Box<dyn Arcade>,
    /// Which round of the game this is (`Save::game_rounds`); only the tests ask.
    #[cfg_attr(not(test), allow(dead_code))]
    pub round: u64,
    /// Milliseconds since the last tick.
    acc: f32,
    /// Paused because the window lost the keyboard (not by the player's `p`): it plays on
    /// when the keys come back.
    auto_paused: bool,
}

impl Session {
    /// Round `round` of `def` on the save seeded `seed`.
    pub fn new(def: &GameDef, seed: u64, round: u64) -> Session {
        Session {
            game: ninsei_core::games::new_game(def, seed, round),
            round,
            acc: 0.0,
            auto_paused: false,
        }
    }
}

/// How many text cells wide one of the game's cells is drawn at: two for a square-ish block,
/// or one in a narrow room.
const WIDE: usize = 2;
/// The status lines under the field, and the narrowest the window gets so they fit: the
/// game's status wraps between its parts (`SCORE 40`, `LINES 2`, ...), each a few cells.
const STATUS_ROWS: usize = 3;
const MIN_COLS: usize = 24;

/// The grids a game's window works in: the field `w` cells a square wide in a frame, then its
/// status line. Square-ish first; narrow when that's all there is.
pub fn grids(def: &GameDef) -> Vec<Fit> {
    [WIDE, 1]
        .into_iter()
        .map(|w| grid(def, w))
        .collect::<Vec<_>>()
}

fn grid(def: &GameDef, w: usize) -> Fit {
    Fit {
        cols: field_cols(def.cols as usize, w).max(MIN_COLS),
        rows: def.rows as usize + 2 + STATUS_ROWS,
    }
}

/// The field's width in text cells, frame and all.
fn field_cols(cols: usize, w: usize) -> usize {
    cols * w + 2
}

/// The status lines: the game's own status, wrapped between its parts into `cols`, then
/// what Space does once it's over. At most [`STATUS_ROWS`], none wider than `cols`.
pub fn status_rows(status: &str, over: bool, cols: usize) -> Vec<String> {
    let mut parts: Vec<&str> = status
        .split("  ")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    if over {
        parts.push("SPACE PLAYS AGAIN");
    }
    let mut rows: Vec<String> = Vec::new();
    for part in parts {
        match rows.last_mut() {
            Some(row) if row.chars().count() + 2 + part.chars().count() <= cols => {
                row.push_str("  ");
                row.push_str(part);
            }
            _ => rows.push(part.to_string()),
        }
    }
    rows.truncate(STATUS_ROWS);
    rows
}

/// What the player did this frame, as the engine's inputs: arrows steer, Space is the main
/// button, P pauses. Browser-safe keys only: no Ctrl, Alt, Tab or function keys.
pub fn inputs(ui: &Ui) -> Vec<Input> {
    ui.input(|i| {
        i.events
            .iter()
            .filter_map(|e| match e {
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } if !modifiers.ctrl && !modifiers.alt && !modifiers.command => match key {
                    Key::ArrowUp => Some(Input::Up),
                    Key::ArrowDown => Some(Input::Down),
                    Key::ArrowLeft => Some(Input::Left),
                    Key::ArrowRight => Some(Input::Right),
                    Key::Space => Some(Input::Fire),
                    Key::P => Some(Input::Pause),
                    _ => None,
                },
                _ => None,
            })
            .collect()
    })
}

/// What happened in the window this frame, for [`crate::home::Home`] to act on.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Played {
    /// The player asked for another round (Space once it's over).
    pub again: bool,
}

/// Plays and draws `s` in `ui`: `keys` says whether its window has the keyboard. The room is
/// at least one of [`grids`] (the desktop checked, `Home::fit`).
pub fn body(ui: &mut Ui, data: &Data, def: &GameDef, s: &mut Session, keys: bool) -> Played {
    let mut played = Played::default();
    let game = &mut s.game;
    // Losing the keyboard pauses the game; getting it back plays on, unless the player had
    // paused it.
    if !keys && !game.paused() && !game.over() {
        game.step(Input::Pause, 0);
        s.auto_paused = true;
    } else if keys && s.auto_paused {
        if game.paused() {
            game.step(Input::Pause, 0);
        }
        s.auto_paused = false;
    }
    if keys {
        // Its keys are the game's: nothing behind it types them.
        ui.ctx().memory_mut(|m| m.stop_text_input());
        for input in inputs(ui) {
            if game.over() && input == Input::Fire {
                played.again = true;
            }
            game.step(input, 0);
        }
    }

    // The face: centred across the room, from its top.
    let room = ui.available_rect_before_wrap();
    let cell = Cells::cell_size(ui.ctx());
    let have = (room.width() / cell.x).floor() as usize;
    let w = if have >= grid(def, WIDE).cols {
        WIDE
    } else {
        1
    };
    let dims = grid(def, w);
    let size = vec2(dims.cols as f32 * cell.x, dims.rows as f32 * cell.y);
    let left = (room.left() + ((room.width() - size.x) / 2.0).max(0.0)).round();
    let (rect, resp) =
        ui.allocate_exact_size(vec2(room.width().max(size.x), size.y), Sense::click());
    let min = pos2(left, rect.top().round());
    let g = Cells::grid(ui, min, (dims.cols, dims.rows), ink(), backdrop(), muted());

    // The field's frame, centred in the grid.
    let x0 = (dims.cols - field_cols(game.cols(), w)) / 2;
    // A click on a square: the main button opens it, the second flags it.
    if keys && !game.over() {
        let at = |p: Option<Pos2>| {
            let (c, r) = g.cell_at(p?)?;
            let (c, r) = (c.checked_sub(x0 + 1)? / w, r.checked_sub(1)?);
            (c < game.cols() && r < game.rows()).then_some((c, r))
        };
        let pos = resp.interact_pointer_pos();
        if resp.clicked()
            && let Some((col, row)) = at(pos)
        {
            game.step(Input::Click { col, row }, 0);
        } else if resp.secondary_clicked()
            && let Some((col, row)) = at(pos)
        {
            game.step(Input::Flag { col, row }, 0);
        }
    }

    // The clock: whole ticks of the game's own length, on the frames' time.
    let running = keys && !game.paused() && !game.over();
    if running {
        s.acc += ui.input(|i| i.stable_dt).min(0.1) * 1000.0;
        let tick = def.tick_ms.max(1) as f32;
        let ticks = (s.acc / tick).floor();
        s.acc -= ticks * tick;
        if ticks > 0.0 {
            game.step(Input::None, ticks as u32);
        }
        ui.ctx().request_repaint();
    } else {
        s.acc = 0.0;
    }

    draw(ui, data, &g, x0, w, game.as_ref());
    played
}

/// The frame, every cell, the cursor and the status line.
fn draw(ui: &Ui, data: &Data, g: &Cells, x0: usize, w: usize, game: &dyn Arcade) {
    let p = ui.painter_at(g.rect);
    let (cols, rows) = (game.cols(), game.rows());
    p.rect_filled(g.rect, 0.0, backdrop());
    g.frame(&p, x0, 0, field_cols(cols, w), rows + 2);
    let gray = data.pattern("p50");
    for (i, cell) in game.cells().into_iter().enumerate() {
        let (c, r) = (i % cols, i / cols);
        let rect = g.at(x0 + 1 + c * w, 1 + r, w, 1);
        paint(&p, rect, cell, gray);
    }
    if let Some((c, r)) = game.cursor()
        && !game.over()
    {
        // A ring in ink with paper inside it, so it shows on a pattern, a fill or paper.
        let rect = g.at(x0 + 1 + c * w, 1 + r, w, 1).shrink(0.5);
        p.rect_stroke(rect, 0.0, Stroke::new(2.0, ink()), egui::StrokeKind::Inside);
        p.rect_stroke(
            rect.shrink(2.0),
            0.0,
            Stroke::new(1.0, backdrop()),
            egui::StrokeKind::Inside,
        );
    }
    for (i, line) in status_rows(&game.status(), game.over(), g.cols)
        .iter()
        .enumerate()
    {
        g.text(&p, 0, rows + 2 + i, line, ink());
    }
}

/// One of the game's cells in `rect`, by its shape.
fn paint(
    p: &egui::Painter,
    rect: Rect,
    cell: Cell,
    gray: Option<&ninsei_core::pattern::PatternDef>,
) {
    let inset = rect.shrink2(vec2(rect.width() * 0.08, rect.height() * 0.08));
    let pattern = |rect: Rect| match gray {
        Some(def) => crate::patterns::fill(p, def, rect, ink(), backdrop()),
        None => {
            p.rect_filled(rect, 0.0, muted());
        }
    };
    let glyph = |s: &str, color: Color32| {
        p.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            s,
            egui::TextStyle::Monospace.resolve(&p.ctx().global_style()),
            color,
        );
    };
    match cell {
        Cell::Empty => {}
        // Walls and covered squares in the gray pattern: the only two, never side by side.
        Cell::Wall => pattern(rect),
        Cell::Covered => pattern(inset),
        Cell::Body => {
            p.rect_filled(inset, 0.0, ink());
        }
        // The head: solid with an eye knocked out of it.
        Cell::Head => {
            p.rect_filled(rect, 0.0, ink());
            let eye =
                Rect::from_center_size(rect.center(), vec2(rect.height(), rect.height()) * 0.3);
            p.rect_filled(eye, 0.0, backdrop());
        }
        // Food: a diamond.
        Cell::Food => {
            let (c, rx, ry) = (rect.center(), rect.height() * 0.32, rect.height() * 0.38);
            p.add(Shape::convex_polygon(
                vec![
                    pos2(c.x, c.y - ry),
                    pos2(c.x + rx, c.y),
                    pos2(c.x, c.y + ry),
                    pos2(c.x - rx, c.y),
                ],
                accent(),
                Stroke::NONE,
            ));
        }
        // A block: filled, with a paper line inside its edge so neighbours read apart.
        Cell::Block(n) => {
            let fill = if n % 2 == 0 { ink() } else { accent() };
            p.rect_filled(inset, 0.0, fill);
            p.rect_stroke(
                inset.shrink(2.0),
                0.0,
                Stroke::new(1.0, backdrop()),
                egui::StrokeKind::Inside,
            );
        }
        // A flag: inverse, with a pennant knocked out.
        Cell::Flag => {
            p.rect_filled(inset, 0.0, ink());
            pennant(p, inset, backdrop());
        }
        Cell::WrongFlag => {
            p.rect_filled(inset, 0.0, ink());
            glyph("X", backdrop());
        }
        Cell::Number(0) => {}
        Cell::Number(n) => glyph(&n.to_string(), ink()),
        // A mine: a round body with spikes, on paper.
        Cell::Mine => {
            let c = rect.center();
            let r = rect.height() * 0.26;
            for (dx, dy) in [(1.0, 0.0), (0.0, 1.0), (0.7, 0.7), (0.7, -0.7)] {
                let d = vec2(dx, dy) * r * 1.5;
                p.line_segment([c - d, c + d], Stroke::new(1.5, ink()));
            }
            p.circle_filled(c, r, ink());
        }
    }
}

/// A flag's pennant and pole in `rect`.
fn pennant(p: &egui::Painter, rect: Rect, color: Color32) {
    let h = rect.height();
    let pole = rect.center().x - h * 0.15;
    let (top, bottom) = (rect.top() + h * 0.18, rect.bottom() - h * 0.18);
    p.line_segment(
        [pos2(pole, top), pos2(pole, bottom)],
        Stroke::new(1.5, color),
    );
    p.add(Shape::convex_polygon(
        vec![
            pos2(pole, top),
            pos2(pole + h * 0.4, top + h * 0.17),
            pos2(pole, top + h * 0.34),
        ],
        color,
        Stroke::NONE,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::home::tests::{deck, frame_keys};
    use crate::home::{Home, WbOut};
    use crate::uitest::{Harness, key};
    use egui::Id;
    use ninsei_core::hub::{GameCopy, Save, Tool};

    /// A deck that owns both copies of every game.
    fn gamer(data: &Data) -> Save {
        let mut s = deck(data);
        for g in &data.games {
            s.learn_game(&g.id, GameCopy::Cracked);
            s.learn_game(&g.id, GameCopy::Boxed);
        }
        s
    }

    /// One frame of `tool`'s window, 880 x 500, with the keyboard or without.
    fn frame_with(
        h: &mut Harness,
        home: &mut Home,
        data: &Data,
        save: &Save,
        tool: Tool,
        keys: bool,
        events: Vec<egui::Event>,
    ) -> Vec<WbOut> {
        let rect = Rect::from_min_size(Pos2::ZERO, vec2(880.0, 500.0));
        frame_keys(h, home, data, save, tool, rect, keys, events)
    }

    /// One frame of `tool`'s window with the keyboard.
    fn frame(
        h: &mut Harness,
        home: &mut Home,
        data: &Data,
        save: &Save,
        tool: Tool,
        events: Vec<egui::Event>,
    ) -> Vec<WbOut> {
        frame_with(h, home, data, save, tool, true, events)
    }

    fn tool(id: &str) -> Tool {
        Tool::game(id, GameCopy::Cracked).unwrap()
    }

    /// A game's window, open and with the keyboard, one frame in.
    fn open(id: &str) -> (Harness, Home, Data, Save, Vec<WbOut>) {
        let data = Data::builtin().unwrap();
        let save = gamer(&data);
        let mut h = Harness::new(1280.0, 800.0);
        let mut home = Home::new(Vec::new());
        let out = frame(&mut h, &mut home, &data, &save, tool(id), Vec::new());
        (h, home, data, save, out)
    }

    #[test]
    fn opening_a_game_starts_a_round_and_counts_it() {
        let (_, home, _, _, out) = open("nibbler");
        assert!(out.contains(&WbOut::GameStarted("nibbler".into())));
        assert!(home.game(tool("nibbler")).is_some_and(|g| !g.over()));
    }

    #[test]
    fn arrows_and_p_reach_the_game_while_it_has_the_keyboard() {
        let (mut h, mut home, data, save, _) = open("minefield");
        let t = tool("minefield");
        let at = home.game(t).unwrap().cursor().unwrap();
        frame(&mut h, &mut home, &data, &save, t, key(Key::ArrowRight));
        frame(&mut h, &mut home, &data, &save, t, key(Key::ArrowDown));
        assert_eq!(home.game(t).unwrap().cursor(), Some((at.0 + 1, at.1 + 1)));
        frame(&mut h, &mut home, &data, &save, t, key(Key::P));
        assert!(home.game(t).unwrap().paused(), "p pauses");
        frame(&mut h, &mut home, &data, &save, t, key(Key::P));
        assert!(!home.game(t).unwrap().paused(), "and plays on");
    }

    #[test]
    fn without_the_keyboard_the_game_pauses_and_hears_nothing() {
        let (mut h, mut home, data, save, _) = open("minefield");
        let t = tool("minefield");
        let at = home.game(t).unwrap().cursor();
        frame_with(
            &mut h,
            &mut home,
            &data,
            &save,
            t,
            false,
            key(Key::ArrowRight),
        );
        let g = home.game(t).unwrap();
        assert!(g.paused(), "it pauses when the keys go elsewhere");
        assert_eq!(g.cursor(), at, "and no key reaches it");
        frame(&mut h, &mut home, &data, &save, t, Vec::new());
        assert!(
            !home.game(t).unwrap().paused(),
            "the keys come back, it plays on"
        );
        // Paused by the player, it stays paused when the keys come back.
        frame(&mut h, &mut home, &data, &save, t, key(Key::P));
        frame_with(&mut h, &mut home, &data, &save, t, false, Vec::new());
        frame(&mut h, &mut home, &data, &save, t, Vec::new());
        assert!(home.game(t).unwrap().paused());
    }

    #[test]
    fn the_game_ticks_on_the_frames_time() {
        let (mut h, mut home, data, save, _) = open("nibbler");
        let t = tool("nibbler");
        let before = home.game(t).unwrap().cells();
        for _ in 0..20 {
            frame(&mut h, &mut home, &data, &save, t, Vec::new());
        }
        assert_ne!(home.game(t).unwrap().cells(), before, "the snake moved");
        // Paused, time stands still.
        frame(&mut h, &mut home, &data, &save, t, key(Key::P));
        let paused = home.game(t).unwrap().cells();
        for _ in 0..20 {
            frame(&mut h, &mut home, &data, &save, t, Vec::new());
        }
        assert_eq!(home.game(t).unwrap().cells(), paused);
    }

    #[test]
    fn a_click_opens_the_square_under_it_and_a_right_click_flags_it() {
        let (mut h, mut home, data, save, _) = open("minefield");
        let t = tool("minefield");
        let def = data.game("minefield").unwrap();
        // Where the window put the field: centred in 880 points, from the top, two cells a
        // square inside a one-cell frame.
        let cell = Cells::cell_size(&h.ctx);
        let size = grid(def, WIDE);
        let left = ((880.0 - size.cols as f32 * cell.x) / 2.0).round();
        let x0 = (size.cols - field_cols(def.cols as usize, WIDE)) as f32;
        let square = |c: usize, r: usize| {
            pos2(
                left + (x0 + 1.0 + c as f32 * 2.0 + 1.0) * cell.x,
                (1.0 + r as f32 + 0.5) * cell.y,
            )
        };
        let cols = def.cols as usize;
        let covered = |home: &Home, c: usize, r: usize| {
            home.game(t).unwrap().cells()[r * cols + c] == Cell::Covered
        };
        let right = |at: Pos2| {
            let b = |pressed| egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Secondary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            vec![egui::Event::PointerMoved(at), b(true), b(false)]
        };
        frame(&mut h, &mut home, &data, &save, t, right(square(2, 3)));
        assert_eq!(home.game(t).unwrap().cells()[3 * cols + 2], Cell::Flag);
        assert!(covered(&home, 5, 6));
        frame(
            &mut h,
            &mut home,
            &data,
            &save,
            t,
            crate::uitest::click(square(5, 6)),
        );
        assert!(!covered(&home, 5, 6), "the click opened it");
    }

    #[test]
    fn space_plays_again_once_it_is_over() {
        let (mut h, mut home, data, save, _) = open("blox");
        let t = tool("blox");
        // Space drops each piece straight down: the well fills and the game ends.
        for _ in 0..200 {
            if home.game(t).unwrap().over() {
                break;
            }
            frame(&mut h, &mut home, &data, &save, t, key(Key::Space));
        }
        assert!(home.game(t).unwrap().over());
        let out = frame(&mut h, &mut home, &data, &save, t, key(Key::Space));
        assert!(out.contains(&WbOut::GameStarted("blox".into())));
        assert!(!home.game(t).unwrap().over(), "a new round");
    }

    #[test]
    fn a_window_opened_again_starts_a_new_round() {
        let (mut h, mut home, data, save, _) = open("minefield");
        let t = tool("minefield");
        frame(&mut h, &mut home, &data, &save, t, key(Key::ArrowRight));
        let moved = home.game(t).unwrap().cursor();
        // The desktop closes its window, then it's opened again.
        home.keep_open(0.0, |_| false);
        let out = frame(&mut h, &mut home, &data, &save, t, Vec::new());
        assert!(out.contains(&WbOut::GameStarted("minefield".into())));
        assert_ne!(home.game(t).unwrap().cursor(), moved);
    }

    #[test]
    fn a_window_hidden_or_too_small_for_a_while_keeps_its_game() {
        let (mut h, mut home, data, save, _) = open("minefield");
        let t = tool("minefield");
        frame(&mut h, &mut home, &data, &save, t, key(Key::ArrowRight));
        let moved = home.game(t).unwrap().cursor();
        // Not drawn at all (another workspace, a full-screen tile), still open.
        for _ in 0..3 {
            home.keep_open(0.0, |x| x == t);
            h.frame(Vec::new(), |_| {});
        }
        // Drawn too small: the notice instead of the game.
        let tiny = Rect::from_min_size(Pos2::ZERO, vec2(60.0, 40.0));
        h.frame(Vec::new(), |ui| {
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(tiny));
            let drawn = home.tool_in(
                &mut child,
                t,
                true,
                &data,
                &save,
                &mut Vec::new(),
                None,
                None,
                "t",
            );
            assert!(!drawn, "the notice, not the game");
        });
        home.keep_open(0.0, |x| x == t);
        let out = frame(&mut h, &mut home, &data, &save, t, Vec::new());
        assert!(out.is_empty(), "no new round: {out:?}");
        assert_eq!(home.game(t).unwrap().cursor(), moved, "the same game");
    }

    #[test]
    fn the_game_takes_the_keyboard_from_a_text_field() {
        let (mut h, mut home, data, save, _) = open("nibbler");
        let field = Id::new("some text field");
        h.ctx.memory_mut(|m| m.request_focus(field));
        frame(&mut h, &mut home, &data, &save, tool("nibbler"), Vec::new());
        assert!(!h.ctx.memory(|m| m.has_focus(field)));
    }

    #[test]
    fn each_copy_plays_its_own_game() {
        let (mut h, mut home, data, save, _) = open("minefield");
        let boxed = Tool::game("minefield", GameCopy::Boxed).unwrap();
        frame(&mut h, &mut home, &data, &save, boxed, Vec::new());
        frame(&mut h, &mut home, &data, &save, boxed, key(Key::ArrowRight));
        assert_ne!(
            home.game(boxed).unwrap().cursor(),
            home.game(tool("minefield")).unwrap().cursor()
        );
    }

    #[test]
    fn the_status_wraps_between_its_parts_and_is_never_cut() {
        let long = "SCORE 99999  LINES 999  LEVEL 99  GAME OVER";
        for cols in MIN_COLS..80 {
            let rows = status_rows(long, true, cols);
            assert!(rows.len() <= STATUS_ROWS, "{cols}: {rows:?}");
            assert!(
                rows.iter().all(|r| r.chars().count() <= cols),
                "{cols}: {rows:?}"
            );
            assert_eq!(
                rows.join("  "),
                format!("{long}  SPACE PLAYS AGAIN"),
                "nothing lost"
            );
        }
        assert_eq!(
            status_rows("SCORE 0  LENGTH 3", false, 40),
            ["SCORE 0  LENGTH 3"]
        );
    }

    #[test]
    fn the_shell_runs_a_game_you_own_and_no_other() {
        let data = Data::builtin().unwrap();
        let mut home = Home::new(Vec::new());
        home.take_opens();
        home.run("run blox", &data, &deck(&data), &mut Vec::new());
        assert!(home.take_opens().is_empty());
        assert!(home.shell_has("blox: object not found"));
        home.run("run blox", &data, &gamer(&data), &mut Vec::new());
        assert_eq!(
            home.take_opens(),
            [Tool::game("blox", GameCopy::Boxed).unwrap()]
        );
    }

    /// `gamer`, with intros on.
    fn with_intros(data: &Data) -> Save {
        Save {
            intros: true,
            ..gamer(data)
        }
    }

    #[test]
    fn a_crack_opens_with_its_crews_cracktro_and_a_box_with_its_splash() {
        use crate::home::TuneCue;
        let data = Data::builtin().unwrap();
        let save = with_intros(&data);
        let def = data.game("nibbler").unwrap();
        for (copy, intro) in [
            (GameCopy::Cracked, &def.crack),
            (GameCopy::Boxed, &def.splash),
        ] {
            let t = Tool::game("nibbler", copy).unwrap();
            let mut h = Harness::new(1280.0, 800.0);
            let mut home = Home::new(Vec::new());
            frame(&mut h, &mut home, &data, &save, t, Vec::new());
            assert_eq!(home.tune, Some(TuneCue::Play(intro.clone())), "{copy:?}");
            assert!(
                home.game(t).is_none(),
                "{copy:?}: the game waits for its intro"
            );
            // It plays to its end, then the game starts.
            let secs = data.intro(intro).unwrap().secs as f64;
            h.wait(secs + 1.0);
            frame(&mut h, &mut home, &data, &save, t, Vec::new());
            assert!(
                home.game(t).is_some(),
                "{copy:?}: the game on the frame its intro ends, no blank frame"
            );
        }
    }

    #[test]
    fn a_key_over_the_window_skips_the_intro_and_cuts_its_tune() {
        use crate::home::TuneCue;
        let data = Data::builtin().unwrap();
        let save = with_intros(&data);
        let t = tool("blox");
        let mut h = Harness::new(1280.0, 800.0);
        let mut home = Home::new(Vec::new());
        frame(&mut h, &mut home, &data, &save, t, Vec::new());
        let mut skip = vec![egui::Event::PointerMoved(pos2(400.0, 200.0))];
        skip.extend(key(Key::Space));
        frame(&mut h, &mut home, &data, &save, t, skip);
        assert_eq!(home.tune, Some(TuneCue::Stop));
        frame(&mut h, &mut home, &data, &save, t, Vec::new());
        assert!(
            home.game(t).is_some_and(|g| !g.over()),
            "straight into the game"
        );
    }

    #[test]
    fn closing_the_window_mid_intro_cuts_its_tune() {
        use crate::home::TuneCue;
        let data = Data::builtin().unwrap();
        let save = with_intros(&data);
        let mut h = Harness::new(1280.0, 800.0);
        let mut home = Home::new(Vec::new());
        frame(
            &mut h,
            &mut home,
            &data,
            &save,
            tool("minefield"),
            Vec::new(),
        );
        home.tune = None;
        let now = h.ctx.input(|i| i.time);
        home.keep_open(now, |x| x == tool("minefield"));
        assert_eq!(home.tune, None, "still open, still playing");
        home.keep_open(now, |_| false);
        assert_eq!(home.tune, Some(TuneCue::Stop));
    }

    #[test]
    fn closing_a_window_whose_intro_ran_out_unseen_cuts_no_tune() {
        use crate::home::TuneCue;
        let data = Data::builtin().unwrap();
        let save = with_intros(&data);
        let mut h = Harness::new(1280.0, 800.0);
        let mut home = Home::new(Vec::new());
        frame(
            &mut h,
            &mut home,
            &data,
            &save,
            tool("minefield"),
            Vec::new(),
        );
        assert!(matches!(home.tune, Some(TuneCue::Play(_))));
        // Never drawn again (the notice, a hidden workspace) till long after its intro ended;
        // meanwhile another intro's tune plays.
        home.tune = None;
        home.keep_open(1000.0, |_| false);
        assert_eq!(home.tune, None, "another intro's tune plays on");
    }

    #[test]
    fn with_intros_off_the_game_starts_at_once_and_plays_no_tune() {
        let (_, home, ..) = open("nibbler");
        assert!(home.tune.is_none());
        assert!(home.game(tool("nibbler")).is_some());
    }

    #[test]
    fn an_intro_in_a_room_smaller_than_its_screen_shrinks_to_fit_and_is_never_cut() {
        let data = Data::builtin().unwrap();
        let save = with_intros(&data);
        // Narrow and tall (Blox's field), wide and short (a stack tile Minefield fits in).
        for (id, size) in [
            ("blox", vec2(330.0, 500.0)),
            ("minefield", vec2(660.0, 360.0)),
        ] {
            let t = tool(id);
            let mut h = Harness::new(1280.0, 800.0);
            let rect = Rect::from_min_size(Pos2::ZERO, size);
            let mut home = Home::new(Vec::new());
            crate::home::tests::frame_at(&mut h, &mut home, &data, &save, t, rect, Vec::new());
            crate::home::tests::frame_at(&mut h, &mut home, &data, &save, t, rect, Vec::new());
            assert!(home.game(t).is_none(), "{id}: the intro is up");
            assert!(h.cut_text().is_empty(), "{id} cut: {:?}", h.cut_text());
            let all = h
                .painted()
                .iter()
                .map(|s| s.visual_bounding_rect())
                .fold(Rect::NOTHING, |a, b| a.union(b));
            assert!(
                rect.expand(1.0).contains_rect(all),
                "{id}: drawn over {all:?}"
            );
        }
    }

    /// Where the snake's head is.
    fn head(home: &Home, t: Tool) -> (i32, i32) {
        let g = home.game(t).unwrap();
        let i = g.cells().iter().position(|c| *c == Cell::Head).unwrap();
        ((i % g.cols()) as i32, (i / g.cols()) as i32)
    }

    /// Frames until the head moves, and which way it went.
    fn step_of(h: &mut Harness, home: &mut Home, data: &Data, save: &Save, t: Tool) -> (i32, i32) {
        let from = head(home, t);
        for _ in 0..50 {
            frame(h, home, data, save, t, Vec::new());
            let to = head(home, t);
            if to != from {
                return (to.0 - from.0, to.1 - from.1);
            }
        }
        panic!("the snake never moved");
    }

    #[test]
    fn arrows_steer_the_snake() {
        let (mut h, mut home, data, save, _) = open("nibbler");
        let t = tool("nibbler");
        let going = step_of(&mut h, &mut home, &data, &save, t);
        // Turn across the way it's going.
        let (k, want) = if going.0 != 0 {
            (Key::ArrowUp, (0, -1))
        } else {
            (Key::ArrowLeft, (-1, 0))
        };
        frame(&mut h, &mut home, &data, &save, t, key(k));
        assert_eq!(step_of(&mut h, &mut home, &data, &save, t), want);
    }

    /// The desktops drawn: `liza.rs`, `workbench.rs` and `tiling.rs` each have a test that
    /// paints them and finds no unowned game.
    #[test]
    fn the_lists_the_desktops_draw_from_hold_only_the_copies_you_own() {
        let data = Data::builtin().unwrap();
        let mut save = deck(&data);
        save.learn_game("blox", GameCopy::Boxed);
        let unowned = |t: &Tool| matches!(t, Tool::Game(id, _) if id.as_str() != "blox");
        let shown: Vec<Tool> = save.icons(&data).into_iter().map(|i| i.tool).collect();
        assert!(!shown.iter().any(unowned), "Save::icons");
        assert_eq!(
            Tool::games(&data, &save),
            [Tool::game("blox", GameCopy::Boxed).unwrap()]
        );
        assert!(
            !Tool::games(&data, &save).iter().any(unowned),
            "Tool::games"
        );
        let words = ninsei_core::hub::shell_complete("run ", &save, &data);
        assert!(words.contains(&"blox".to_string()));
        assert!(
            !words
                .iter()
                .any(|w| w.contains("nibbler") || w.contains("minefield"))
        );
        // And the crack of the game you own is no more there than the others.
        assert!(!save.opens(Tool::game("blox", GameCopy::Cracked).unwrap()));
    }

    #[test]
    fn the_key_that_skips_the_intro_never_reaches_the_game() {
        let data = Data::builtin().unwrap();
        let save = with_intros(&data);
        let t = tool("minefield");
        let mut h = Harness::new(1280.0, 800.0);
        let mut home = Home::new(Vec::new());
        frame(&mut h, &mut home, &data, &save, t, Vec::new());
        assert!(home.game(t).is_none(), "the cracktro is up");
        // Space skips it (the window has the keys), and isn't Fire on the first square.
        frame(&mut h, &mut home, &data, &save, t, key(Key::Space));
        frame(&mut h, &mut home, &data, &save, t, Vec::new());
        let g = home.game(t).expect("the game after the skip");
        assert!(
            g.cells().iter().all(|c| *c == Cell::Covered),
            "nothing opened"
        );
    }

    #[test]
    fn two_copies_opened_before_the_save_counts_deal_two_rounds() {
        let (mut h, mut home, data, save, _) = open("minefield");
        let boxed = Tool::game("minefield", GameCopy::Boxed).unwrap();
        // The save still says round 0: the crack's start hasn't been counted on it yet.
        assert_eq!(save.game_round("minefield"), 0);
        frame(&mut h, &mut home, &data, &save, boxed, Vec::new());
        assert_eq!(home.round(tool("minefield")), Some(0));
        assert_eq!(home.round(boxed), Some(1), "a field of its own");
        // Once the save has counted both, the next round is the third.
        let mut counted = save.clone();
        counted.start_game("minefield");
        counted.start_game("minefield");
        frame(&mut h, &mut home, &data, &counted, boxed, Vec::new());
        home.keep_open(0.0, |_| false);
        frame(&mut h, &mut home, &data, &counted, boxed, Vec::new());
        assert_eq!(home.round(boxed), Some(2));
    }

    #[test]
    fn only_a_game_with_the_keyboard_has_its_intro_skipped() {
        let data = Data::builtin().unwrap();
        let save = with_intros(&data);
        let t = tool("minefield");
        let mut h = Harness::new(1280.0, 800.0);
        let mut home = Home::new(Vec::new());
        frame_with(&mut h, &mut home, &data, &save, t, false, Vec::new());
        // A key typed with the pointer over it, and a click on it, while another window has
        // the keyboard (a board in front, a window over it): the cracktro plays on.
        let at = pos2(400.0, 200.0);
        let mut typed = vec![egui::Event::PointerMoved(at)];
        typed.extend(key(Key::Space));
        frame_with(&mut h, &mut home, &data, &save, t, false, typed);
        frame_with(
            &mut h,
            &mut home,
            &data,
            &save,
            t,
            false,
            crate::uitest::click(at),
        );
        frame_with(&mut h, &mut home, &data, &save, t, false, Vec::new());
        assert!(home.game(t).is_none(), "the intro is still up");
        // With the keyboard, a click skips it.
        frame_with(
            &mut h,
            &mut home,
            &data,
            &save,
            t,
            true,
            crate::uitest::click(at),
        );
        frame_with(&mut h, &mut home, &data, &save, t, true, Vec::new());
        assert!(home.game(t).is_some());
    }

    /// Acceptance (47.4): a cracked copy opens with its crew's cracktro, a boxed one with its
    /// publisher's splash, for every game; with intros off, neither plays.
    #[test]
    fn acceptance_every_copy_opens_with_its_own_intro_or_none_with_intros_off() {
        use crate::home::TuneCue;
        let data = Data::builtin().unwrap();
        for intros in [true, false] {
            let save = Save {
                intros,
                ..gamer(&data)
            };
            for t in Tool::games(&data, &save) {
                let (def, copy) = t.game_def(&data).unwrap();
                let want = match copy {
                    GameCopy::Cracked => &def.crack,
                    GameCopy::Boxed => &def.splash,
                };
                let mut h = Harness::new(1280.0, 800.0);
                let mut home = Home::new(Vec::new());
                frame(&mut h, &mut home, &data, &save, t, Vec::new());
                if intros {
                    assert_eq!(home.tune, Some(TuneCue::Play(want.clone())), "{t:?}");
                    assert!(home.game(t).is_none(), "{t:?}: its intro first");
                } else {
                    assert_eq!(home.tune, None, "{t:?}: no intro, no tune");
                    assert!(home.game(t).is_some(), "{t:?}: straight to the game");
                }
            }
        }
    }

    /// Acceptance (47.4): every game plays from the keyboard: what the arrows and Space do
    /// changes its field.
    #[test]
    fn acceptance_every_game_plays_from_the_keyboard() {
        let data = Data::builtin().unwrap();
        for g in &data.games {
            let (mut h, mut home, data, save, _) = open(&g.id);
            let t = tool(&g.id);
            let before = home.game(t).unwrap().cells();
            for k in [Key::ArrowUp, Key::ArrowLeft, Key::Space, Key::Space] {
                frame(&mut h, &mut home, &data, &save, t, key(k));
            }
            assert_ne!(
                home.game(t).unwrap().cells(),
                before,
                "{}: the keys did nothing",
                g.id
            );
        }
    }
}
