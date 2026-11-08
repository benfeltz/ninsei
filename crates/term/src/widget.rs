//! The line as an egui widget: a prompt, the text and a block cursor, painted like a terminal.
//! Key events go through the [`keys`](crate::keys) table into the [`LineEditor`]; typed text is
//! inserted. Colours and the font come from the caller (the game's theme).

use egui::{Align2, Color32, Event, FontId, Id, Rect, Sense, Ui, pos2, vec2};

use crate::keys::{self, Key, Mods};
use crate::{Edit, LineEditor, Outcome};

/// How the line is drawn.
#[derive(Clone, Debug)]
pub struct Style {
    pub font: FontId,
    pub prompt: Color32,
    pub text: Color32,
    pub cursor: Color32,
    /// Text under the block cursor.
    pub under_cursor: Color32,
    /// The `(reverse-i-search)` prompt.
    pub search: Color32,
}

/// What happened to the line this frame.
pub struct LineResponse {
    /// Everything the edits asked for, in order (a submit, a list of completions, ...).
    pub outcomes: Vec<Outcome>,
    pub focused: bool,
    pub response: egui::Response,
}

/// How long the cursor stays on and off, in seconds.
const BLINK: f64 = 0.53;

fn key_of(k: egui::Key) -> Option<Key> {
    use egui::Key as E;
    Some(match k {
        E::ArrowLeft => Key::Left,
        E::ArrowRight => Key::Right,
        E::ArrowUp => Key::Up,
        E::ArrowDown => Key::Down,
        E::Home => Key::Home,
        E::End => Key::End,
        E::Backspace => Key::Backspace,
        E::Delete => Key::Delete,
        E::Tab => Key::Tab,
        E::Enter => Key::Enter,
        E::Escape => Key::Escape,
        other => {
            let name = other.name();
            let mut chars = name.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) if c.is_ascii_alphabetic() => Key::Letter(c.to_ascii_lowercase()),
                _ => return None,
            }
        }
    })
}

/// Whether the backend sends a key's typed text just before its key event (the browser's
/// keydown does) or just after it (egui-winit does).
const TEXT_BEFORE_KEY: bool = cfg!(target_arch = "wasm32");

/// The edit a key event does, and whether it was a Ctrl/Alt chord.
fn bound(e: &Event) -> Option<(Edit, bool)> {
    let Event::Key {
        key,
        pressed: true,
        modifiers,
        ..
    } = e
    else {
        return None;
    };
    let mods = Mods {
        ctrl: modifiers.ctrl,
        alt: modifiers.alt,
        shift: modifiers.shift,
    };
    let edit = keys::bind(key_of(*key)?, mods)?;
    Some((edit, mods.ctrl || mods.alt))
}

/// The edits a frame's events ask for, in the order they arrived, so text typed before Enter
/// is part of the line Enter runs. An Alt chord also types a character on some keyboards
/// (Option+B is ∫): the one text event that chord typed, right beside its key, is skipped.
pub fn edits(events: &[Event]) -> Vec<Edit> {
    edits_in(events, TEXT_BEFORE_KEY)
}

fn edits_in(events: &[Event], text_before_key: bool) -> Vec<Edit> {
    let chord = |e: Option<&Event>| e.and_then(bound).is_some_and(|(_, chord)| chord);
    let mut out = Vec::new();
    for (i, e) in events.iter().enumerate() {
        match e {
            Event::Text(t) => {
                let typed_by_chord = if text_before_key {
                    chord(events.get(i + 1))
                } else {
                    chord(i.checked_sub(1).and_then(|j| events.get(j)))
                };
                if !typed_by_chord {
                    out.extend(t.chars().map(Edit::Insert));
                }
            }
            Event::Paste(t) => out.push(Edit::InsertStr(t.clone())),
            _ => out.extend(bound(e).map(|(edit, _)| edit)),
        }
    }
    out
}

/// What a frame's edits did to the line.
#[derive(Debug, Default, PartialEq)]
struct Applied {
    outcomes: Vec<Outcome>,
    /// Edits after a submit, for the next frame: what the submit runs may hand the keyboard to
    /// someone else (a board, a modal), and then they aren't the line's.
    rest: Vec<Edit>,
    /// Esc outside a search: the line lets go of the keyboard, and the frame's later edits
    /// aren't its own.
    let_go: bool,
}

/// Applies `edits` in order, stopping at the first submit or at an Esc that lets go.
fn apply(ed: &mut LineEditor, edits: Vec<Edit>, complete: &dyn Fn(&str) -> Vec<String>) -> Applied {
    let mut done = Applied::default();
    let mut edits = edits.into_iter();
    while let Some(e) = edits.next() {
        if e == Edit::Escape && !ed.searching() {
            done.let_go = true;
            break;
        }
        let out = ed.apply(e, complete);
        let submitted = matches!(out, Outcome::Submit(_));
        if out != Outcome::Nothing {
            done.outcomes.push(out);
        }
        if submitted {
            done.rest = edits.collect();
            break;
        }
    }
    done
}

/// Edits held over from a frame that submitted, and that frame's pass number.
#[derive(Clone)]
struct Held(u64, Vec<Edit>);

/// Draws the line and applies this frame's keys if it has focus. `complete` gets the text
/// before the cursor and returns words that could finish its last word.
pub fn line(
    ui: &mut Ui,
    id: Id,
    ed: &mut LineEditor,
    prompt: &str,
    style: &Style,
    complete: &dyn Fn(&str) -> Vec<String>,
) -> LineResponse {
    let row = ui.fonts_mut(|f| f.row_height(&style.font));
    // Take the space without a widget of its own (a second, unnamed focusable widget there would
    // take the clicks and the focus), then the line is the one widget, under `id`.
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), row + 4.0), Sense::hover());
    let response = ui.interact(rect, id, Sense::click());
    if response.clicked() {
        response.request_focus();
    }
    let mut focused = response.has_focus();
    let mut outcomes = Vec::new();
    let pass = ui.ctx().cumulative_pass_nr();
    let held = ui.data_mut(|d| {
        let held = d.get_temp::<Held>(id);
        d.remove::<Held>(id);
        held
    });
    if focused {
        // Tab completes and the arrows edit, rather than moving focus. Esc is the line's too,
        // so egui doesn't drop the focus before this frame's typing is in: it leaves a search,
        // and otherwise the line lets go of the keyboard (so the run's hotkeys work again).
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(
                id,
                egui::EventFilter {
                    tab: true,
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    escape: true,
                },
            )
        });
        // What last frame held over after its submit comes first.
        let mut todo = match held {
            Some(Held(at, edits)) if at + 1 == pass => edits,
            _ => Vec::new(),
        };
        todo.extend(ui.input(|i| edits(&i.events)));
        let done = apply(ed, todo, complete);
        outcomes = done.outcomes;
        if done.let_go {
            ui.memory_mut(|m| m.surrender_focus(id));
            focused = false;
        } else if !done.rest.is_empty() {
            ui.data_mut(|d| d.insert_temp(id, Held(pass, done.rest)));
            ui.ctx().request_repaint();
        }
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs_f64(BLINK));
    }

    // Draw: prompt (or the search prompt), the text, the cursor.
    let painter = ui.painter_at(rect);
    let view = ed.view();
    let y = rect.center().y;
    let mut x = rect.left();
    let (lead, lead_col) = match &view.search {
        Some(s) => (s.as_str(), style.search),
        None => (prompt, style.prompt),
    };
    if !lead.is_empty() {
        let g = painter.layout_no_wrap(format!("{lead} "), style.font.clone(), lead_col);
        x += g.size().x;
        painter.galley(pos2(rect.left(), y - g.size().y / 2.0), g, lead_col);
    }
    let galley = painter.layout_no_wrap(view.text.clone(), style.font.clone(), style.text);
    let cw = ui.fonts_mut(|f| f.glyph_width(&style.font, 'M'));
    painter.galley(pos2(x, y - galley.size().y / 2.0), galley, style.text);
    let on = ((ui.input(|i| i.time) / BLINK) as i64) % 2 == 0;
    if focused && on {
        let cx = x + view.cursor as f32 * cw;
        let cell = Rect::from_min_size(pos2(cx, y - row / 2.0), vec2(cw.max(2.0), row));
        painter.rect_filled(cell, 0.0, style.cursor);
        if let Some(c) = view.text.chars().nth(view.cursor) {
            painter.text(
                cell.center(),
                Align2::CENTER_CENTER,
                c,
                style.font.clone(),
                style.under_cursor,
            );
        }
    } else if !focused && view.text.is_empty() {
        // An outline cursor says where to click.
        let cell = Rect::from_min_size(pos2(x, y - row / 2.0), vec2(cw.max(2.0), row));
        painter.rect_stroke(
            cell,
            0.0,
            egui::Stroke::new(1.0, style.cursor.gamma_multiply(0.5)),
            egui::StrokeKind::Inside,
        );
    }
    LineResponse {
        outcomes,
        focused,
        response,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(key: egui::Key, modifiers: egui::Modifiers) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    fn enter() -> Event {
        key(egui::Key::Enter, egui::Modifiers::NONE)
    }

    fn text(t: &str) -> Event {
        Event::Text(t.into())
    }

    /// Applies one frame's events to `ed`.
    fn frame(ed: &mut LineEditor, events: &[Event]) -> Applied {
        apply(ed, edits(events), &|_| Vec::new())
    }

    #[test]
    fn half_line_is_not_submitted_early() {
        let mut ed = LineEditor::default();
        let done = frame(&mut ed, &[text("d"), text("ial 1"), enter()]);
        assert_eq!(done.outcomes, vec![Outcome::Submit("dial 1".into())]);
        assert_eq!(ed.text(), "");
    }

    #[test]
    fn edits_after_a_submit_wait_for_the_next_frame() {
        let mut ed = LineEditor::default();
        let done = frame(&mut ed, &[text("ls"), enter(), text("x"), enter()]);
        assert_eq!(done.outcomes, vec![Outcome::Submit("ls".into())]);
        assert_eq!(ed.text(), "", "nothing typed after the submit went in");
        assert_eq!(done.rest, vec![Edit::Insert('x'), Edit::Submit]);
        // Next frame: the held edits, then that frame's own.
        let next = apply(&mut ed, done.rest, &|_| Vec::new());
        assert_eq!(next.outcomes, vec![Outcome::Submit("x".into())]);
    }

    #[test]
    fn esc_lets_go_after_the_typing_before_it() {
        let mut ed = LineEditor::default();
        let esc = key(egui::Key::Escape, egui::Modifiers::NONE);
        let done = frame(&mut ed, &[text("run 1"), enter(), esc.clone()]);
        assert_eq!(done.outcomes, vec![Outcome::Submit("run 1".into())]);
        assert_eq!(done.rest, vec![Edit::Escape]);
        let next = apply(&mut ed, done.rest, &|_| Vec::new());
        assert!(next.let_go);
        let done = frame(&mut ed, &[text("ls"), esc.clone(), text("x")]);
        assert!(done.let_go);
        assert_eq!(ed.text(), "ls", "typing after Esc isn't the line's");
        // In a search, Esc only leaves the search.
        let ctrl_r = key(egui::Key::R, egui::Modifiers::CTRL);
        let l = key(egui::Key::L, egui::Modifiers::NONE);
        let done = frame(&mut ed, &[enter(), ctrl_r, l, text("l"), esc]);
        assert_eq!(done.outcomes, vec![Outcome::Submit("ls".into())]);
        let next = apply(&mut ed, done.rest, &|_| Vec::new());
        assert!(!next.let_go);
        assert!(!ed.searching());
        assert_eq!(ed.text(), "ls", "the search kept its match");
    }

    #[test]
    fn a_chord_drops_only_the_text_it_typed() {
        let alt = egui::Modifiers::ALT;
        let alt_b = key(egui::Key::B, alt);
        let b = key(egui::Key::B, egui::Modifiers::NONE);
        // Native: each key's text comes after its key.
        let native = [
            b.clone(),
            text("b"),
            alt_b.clone(),
            text("∫"),
            b.clone(),
            text("b"),
        ];
        assert_eq!(
            edits_in(&native, false),
            [Edit::Insert('b'), Edit::WordLeft, Edit::Insert('b')]
        );
        // The browser: before it.
        let web = [text("b"), b.clone(), text("∫"), alt_b, text("b"), b];
        assert_eq!(
            edits_in(&web, true),
            [Edit::Insert('b'), Edit::WordLeft, Edit::Insert('b')]
        );
    }

    #[test]
    fn typing_around_a_chord_stays_in_the_line() {
        // `dial gut`, Alt+Backspace (kill word, which types nothing), `2`, Enter, in one frame.
        // Each typed key's text is beside its own key event (after it natively, before it in
        // the browser), never beside the chord's.
        let alt_bs = key(egui::Key::Backspace, egui::Modifiers::ALT);
        let two = key(egui::Key::Num2, egui::Modifiers::NONE);
        let t = key(egui::Key::T, egui::Modifiers::NONE);
        let native = vec![
            t.clone(),
            text("dial gut"),
            alt_bs.clone(),
            two.clone(),
            text("2"),
            enter(),
        ];
        let web = vec![text("dial gut"), t, alt_bs, text("2"), two, enter()];
        for (events, text_before_key) in [(native, false), (web, true)] {
            let mut ed = LineEditor::default();
            let done = apply(&mut ed, edits_in(&events, text_before_key), &|_| Vec::new());
            assert_eq!(done.outcomes, vec![Outcome::Submit("dial 2".into())]);
        }
    }

    #[test]
    fn a_pasted_newline_is_a_space() {
        // The editor flattens a paste into the line; Enter runs all of it.
        let mut ed = LineEditor::default();
        let paste = Event::Paste("b\nc".into());
        let done = frame(&mut ed, &[text("a"), paste, text("d"), enter()]);
        assert_eq!(done.outcomes, vec![Outcome::Submit("ab cd".into())]);
    }

    #[test]
    fn a_held_key_repeats() {
        let mut ed = LineEditor::default();
        let mut held = key(egui::Key::Backspace, egui::Modifiers::NONE);
        if let Event::Key { repeat, .. } = &mut held {
            *repeat = true;
        }
        let done = frame(&mut ed, &[text("abc"), held.clone(), held, enter()]);
        assert_eq!(done.outcomes, vec![Outcome::Submit("a".into())]);
    }
}
