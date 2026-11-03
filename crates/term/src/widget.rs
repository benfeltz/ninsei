//! The line as an egui widget: a prompt, the text and a block cursor, painted like a terminal.
//! Key events go through the [`keys`](crate::keys) table into the [`LineEditor`]; typed text is
//! inserted. Colours and the font come from the caller (the game's theme).

use egui::{Align2, Color32, Event, FontId, Id, Rect, Sense, Ui, pos2, vec2};

use crate::keys::{self, Key, Mods};
use crate::{LineEditor, Outcome};

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
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), row + 4.0), Sense::click());
    let response = ui.interact(rect, id, Sense::click()).union(response);
    if response.clicked() {
        response.request_focus();
    }
    let focused = response.has_focus();
    let mut outcomes = Vec::new();
    if focused {
        // Tab completes and the arrows edit, rather than moving focus; Esc leaves a search,
        // and otherwise lets go of the keyboard (so the run's hotkeys work again).
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(
                id,
                egui::EventFilter {
                    tab: true,
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    escape: ed.searching(),
                },
            )
        });
        let events = ui.input(|i| i.events.clone());
        // An Alt chord also types a character on some keyboards (Option+B is ∫): skip text that
        // arrives in a frame where a chord did something.
        let mut chorded = false;
        for e in &events {
            if let Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = e
                && let Some(k) = key_of(*key)
            {
                let mods = Mods {
                    ctrl: modifiers.ctrl,
                    alt: modifiers.alt,
                    shift: modifiers.shift,
                };
                if let Some(edit) = keys::bind(k, mods) {
                    chorded |= mods.ctrl || mods.alt;
                    let out = ed.apply(edit, complete);
                    if out != Outcome::Nothing {
                        outcomes.push(out);
                    }
                }
            }
        }
        for e in events {
            match e {
                Event::Text(t) if !chorded => {
                    for c in t.chars() {
                        ed.apply(crate::Edit::Insert(c), complete);
                    }
                }
                Event::Paste(t) => {
                    ed.apply(crate::Edit::InsertStr(t), complete);
                }
                _ => {}
            }
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
