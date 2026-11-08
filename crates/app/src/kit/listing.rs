//! A program listing that a pane holds the keyboard for: the cursor, the window over the rows,
//! the line being retyped where it stands, and the keys every listing shares. What it shows and
//! what an edit does are core's (`ninsei_core::listing`); the pane draws the rows in its own
//! look and names its own keys (`Bind`). Nothing here names a tool or a deck.
//!
//! The pane's keyboard focus is an egui focus id: a click takes it, with the arrows, Tab and
//! Esc locked to it. While it has it, the keys it binds and every typed character are taken out
//! of the frame, so the terminal types only when the pane isn't focused. A retyped row is a
//! `ninsei_term` line editor under its own focus id, which holds the keyboard like any typing.

use std::ops::Range;

use egui::{Event, Id, Key, Rect, Response, UiBuilder, vec2};
use ninsei_core::listing::{self, Gutter, Row};
use ninsei_core::{Action, Game};

use super::{Cells, List};

/// What a listing remembers between frames, in egui memory.
#[derive(Clone, Debug, Default)]
pub struct View {
    /// The line the cursor is on.
    pub cursor: Option<u32>,
    /// The window over the rows: core's one scroll rule, the wheel's leftover travel.
    pub list: List,
    /// The line being retyped where it stands, if one is.
    pub edit: Option<Edit>,
}

/// A line being retyped: its number (a new one, for a line just opened) and the editor holding
/// its text. Enter commits it as one `Action::Program`; Esc or Ctrl+C puts it back.
#[derive(Clone, Debug)]
pub struct Edit {
    pub no: u32,
    pub line: ninsei_term::LineEditor,
}

/// What a key does to a listing.
#[derive(Clone, Debug, PartialEq)]
pub enum Bind {
    /// A game action (the debugger's: step, play, pause).
    Act(Action),
    Up,
    Down,
    /// Retype the line under the cursor.
    Edit,
    /// Open a new line after the cursor.
    Open,
    /// Delete the line under the cursor.
    Delete,
    /// Give the keyboard back.
    Release,
}

/// One pane's listing, named by its focus id.
pub struct Listing {
    id: Id,
}

impl Listing {
    pub fn new(name: &'static str) -> Listing {
        Listing { id: Id::new(name) }
    }

    /// The pane's keyboard focus.
    pub fn id(&self) -> Id {
        self.id
    }

    /// The retyped row's keyboard focus.
    pub fn edit_id(&self) -> Id {
        self.id.with("edit")
    }

    fn view_id(&self) -> Id {
        self.id.with("view")
    }

    /// The view as last kept, its cursor back on a line that's there.
    pub fn view(&self, ctx: &egui::Context, g: &Game) -> View {
        let mut v: View = ctx.data(|d| d.get_temp(self.view_id())).unwrap_or_default();
        if v.cursor.is_none_or(|n| g.program.line(n).is_none()) {
            v.cursor = listing::home(g);
        }
        v
    }

    pub fn keep(&self, ctx: &egui::Context, v: View) {
        ctx.data_mut(|d| d.insert_temp(self.view_id(), v));
    }

    /// Whether the pane holds the keyboard (not a retyped row: that's typing).
    pub fn has_keys(&self, ctx: &egui::Context) -> bool {
        ctx.memory(|m| m.has_focus(self.id))
    }

    /// Takes the keyboard for the pane: its arrows, Tab and Esc are its own.
    pub fn take_keys(&self, ctx: &egui::Context) {
        let id = self.id;
        ctx.memory_mut(|m| {
            m.request_focus(id);
            m.set_focus_lock_filter(
                id,
                egui::EventFilter {
                    tab: true,
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    escape: true,
                },
            );
        });
    }

    /// Starts retyping line `no` from `text`; the row takes the keyboard.
    pub fn start_edit(&self, ctx: &egui::Context, v: &mut View, no: u32, text: &str) {
        let mut line = ninsei_term::LineEditor::new(1);
        line.set_text(text);
        v.edit = Some(Edit { no, line });
        let id = self.edit_id();
        ctx.memory_mut(|m| m.request_focus(id));
    }

    /// This frame's keys while the pane holds the keyboard, through `bind` (a key with no
    /// modifier, to what it does; `None` leaves it for the run). Moves act on a held key's
    /// repeats; everything else acts once but takes its repeats too, so a held Enter never
    /// reaches the hotkeys. The keys taken, the characters they typed and any typing before a
    /// retype started come out of the frame. Returns the game actions.
    pub fn keys(
        &self,
        ctx: &egui::Context,
        g: &Game,
        bind: impl Fn(Key) -> Option<Bind>,
    ) -> Vec<Action> {
        if !self.has_keys(ctx) {
            return Vec::new();
        }
        let mut v = self.view(ctx, g);
        let k = listing::keys(g, v.cursor);
        let mut out = Vec::new();
        let mut taken = Vec::new();
        let events = ctx.input(|i| i.events.clone());
        // Where a retype started this frame: the typing after it is the line's.
        let mut edit_from = None;
        for (i, e) in events.iter().enumerate() {
            let Event::Key {
                key,
                pressed: true,
                repeat,
                modifiers,
                ..
            } = e
            else {
                continue;
            };
            if !modifiers.is_none() || v.edit.is_some() {
                continue;
            }
            let Some(b) = bind(*key) else {
                continue;
            };
            taken.push(i);
            let once = !*repeat;
            match b {
                Bind::Up => v.cursor = listing::moved(g, v.cursor, -1),
                Bind::Down => v.cursor = listing::moved(g, v.cursor, 1),
                _ if !once => {}
                Bind::Act(a) => out.push(a),
                Bind::Edit => {
                    if let Some(no) = v.cursor.filter(|_| k.edit) {
                        self.start_edit(ctx, &mut v, no, &listing::editing_text(g, no));
                        edit_from = Some(i);
                    }
                }
                Bind::Open => {
                    if let Some(no) = listing::open_after(g, v.cursor) {
                        self.start_edit(ctx, &mut v, no, "");
                        edit_from = Some(i);
                    }
                }
                Bind::Delete => {
                    if let Some(no) = v.cursor
                        && let Some(a) = listing::delete(g, no)
                    {
                        let lines = &g.program.lines;
                        let before = lines.iter().map(|l| l.no).take_while(|&n| n < no).last();
                        v.cursor = g.program.after(no).or(before);
                        out.push(a);
                    }
                }
                Bind::Release => {
                    let id = self.id;
                    ctx.memory_mut(|m| m.surrender_focus(id));
                }
            }
        }
        let drop = dropped(&events, &taken, edit_from);
        ctx.input_mut(|inp| {
            let mut i = 0;
            inp.events.retain(|_| {
                i += 1;
                !drop.contains(&(i - 1))
            })
        });
        self.keep(ctx, v);
        out
    }

    /// The rows to draw: the program's, with a row for a line just opened before it's in the
    /// program.
    pub fn rows(&self, g: &Game, v: &View) -> Vec<Row> {
        let mut rows = listing::rows(g);
        if let Some(e) = &v.edit
            && g.program.line(e.no).is_none()
        {
            let at = rows
                .iter()
                .position(|r| r.no.is_none_or(|n| n > e.no))
                .unwrap_or(rows.len());
            rows.insert(
                at,
                Row {
                    no: Some(e.no),
                    text: String::new(),
                    gutter: Gutter::None,
                    dim: false,
                    fault: None,
                },
            );
        }
        rows
    }

    /// The rows in view in `n` rows of `c`: the wheel scrolls them whole rows at a time, and
    /// the window keeps in view the row being retyped, else the cursor while the pane has the
    /// keys, else the line about to run.
    pub fn window(
        &self,
        ui: &mut egui::Ui,
        c: &Cells,
        v: &mut View,
        rows: &[Row],
        focused: bool,
        n: usize,
    ) -> Range<usize> {
        let follow = if let Some(e) = &v.edit {
            Some(e.no)
        } else if focused {
            v.cursor
        } else {
            rows.iter()
                .find(|r| r.gutter == Gutter::Current)
                .and_then(|r| r.no)
        };
        let sel = follow.and_then(|no| rows.iter().position(|r| r.no == Some(no)));
        if let Some(dy) = super::scrollbar::take_wheel(ui, c.rect, self.id) {
            v.list.wheel(dy / c.ch);
        }
        v.list.view(rows.len(), sel, n)
    }

    /// A click on a row (the pane's `tile` response, rows drawn from the top of `c`, `n` of
    /// them) puts the cursor there; a double-click retypes it.
    #[allow(clippy::too_many_arguments)]
    pub fn click(
        &self,
        ctx: &egui::Context,
        tile: &Response,
        c: &Cells,
        v: &mut View,
        rows: &[Row],
        n: usize,
        g: &Game,
    ) {
        if tile.clicked()
            && let Some(pos) = tile.interact_pointer_pos()
            && let Some((_, y)) = c.cell_at(pos)
            && let Some(r) = rows.get(v.list.top + y).filter(|_| y < n)
            && let Some(no) = r.no
        {
            v.cursor = Some(no);
            if tile.double_clicked() && v.edit.is_none() {
                self.start_edit(ctx, v, no, &listing::editing_text(g, no));
            }
        }
    }

    /// The row being retyped: the line editor over `at`, the row's text cells. Enter commits
    /// the one line (the action it makes, if anything changed); Esc, Ctrl+C or a click
    /// elsewhere puts the old text back. Committing or backing out hands the pane the keys.
    pub fn edit_row(
        &self,
        ui: &mut egui::Ui,
        at: Rect,
        v: &mut View,
        g: &Game,
        style: &ninsei_term::widget::Style,
    ) -> Option<Action> {
        let edit = v.edit.as_mut()?;
        let h = ui.fonts_mut(|f| f.row_height(&style.font)) + 4.0;
        let rect = Rect::from_center_size(at.center(), vec2(at.width(), h));
        let mut child = ui.new_child(UiBuilder::new().max_rect(rect));
        let r = ninsei_term::widget::line(
            &mut child,
            self.edit_id(),
            &mut edit.line,
            "",
            style,
            &|_| Vec::new(),
        );
        let no = edit.no;
        let submitted = r.outcomes.iter().find_map(|o| match o {
            ninsei_term::Outcome::Submit(t) => Some(t.clone()),
            _ => None,
        });
        // Ctrl+C backs out as Esc does (the editor has already cleared its line).
        let cancelled = r
            .outcomes
            .iter()
            .any(|o| matches!(o, ninsei_term::Outcome::Cancelled(_)));
        let ctx = ui.ctx().clone();
        let mut out = None;
        if let Some(text) = submitted {
            out = listing::retype(g, no, &text);
            v.edit = None;
            if !text.trim().is_empty() {
                v.cursor = Some(no);
            }
            self.take_keys(&ctx);
        } else if cancelled {
            v.edit = None;
            self.take_keys(&ctx);
        } else if !r.focused {
            // Esc let go of the keyboard (it comes back to the pane), or a click went
            // elsewhere (it stays there): the old text stands.
            v.edit = None;
            if ctx.memory(|m| m.focused().is_none()) {
                self.take_keys(&ctx);
            }
        }
        out
    }
}

/// The cells a line number takes: as wide as the widest, never under two.
pub fn number_w(rows: &[Row]) -> usize {
    rows.iter()
        .filter_map(|r| r.no)
        .map(|n| n.to_string().len())
        .max()
        .unwrap_or(0)
        .max(2)
}

/// The events (by index) a listing takes out of a frame: the keys it took, the character each
/// of those typed (beside its key: after it natively, before it in a browser), and any other
/// typing before a retype started (the pane has the keyboard; that typing has nowhere to go).
/// Typing after a retype started stays for the line.
pub fn dropped(events: &[Event], taken: &[usize], edit_from: Option<usize>) -> Vec<usize> {
    let typed = |i: usize, key: Key| {
        let name = key.name().to_ascii_lowercase();
        matches!(events.get(i), Some(Event::Text(t)) if *t == name)
    };
    let mut out: Vec<usize> = taken.to_vec();
    for &t in taken {
        if let Some(Event::Key { key, .. }) = events.get(t) {
            if typed(t + 1, *key) {
                out.push(t + 1);
            } else if t > 0 && typed(t - 1, *key) {
                out.push(t - 1);
            }
        }
    }
    let until = edit_from.unwrap_or(events.len());
    out.extend((0..until).filter(|&i| matches!(events[i], Event::Text(_))));
    out
}
