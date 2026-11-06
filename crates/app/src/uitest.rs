//! Headless UI tests: run frames on an `egui::Context`, feed keys, text and clicks, and find
//! widgets by what they say through egui's accessibility tree, so a test can press "Hang up"
//! the way a player does instead of guessing where it was drawn.

use egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, Ui, Vec2, pos2, vec2};

/// Frames on one context, a tenth of a second apart.
pub struct Harness {
    pub ctx: egui::Context,
    size: Vec2,
    t: f64,
    /// What last frame's widgets said (label or value), where they were, and whether they
    /// would take a click.
    widgets: Vec<(String, Rect, bool)>,
}

impl Harness {
    /// A screen of `w` × `h` points.
    pub fn new(w: f32, h: f32) -> Harness {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        Harness {
            ctx,
            size: vec2(w, h),
            t: 0.0,
            widgets: Vec::new(),
        }
    }

    /// One frame with these events, drawing with `ui`.
    pub fn frame(&mut self, events: Vec<Event>, mut ui: impl FnMut(&mut Ui)) {
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)),
            time: Some(self.t),
            events,
            ..RawInput::default()
        };
        self.t += 0.1;
        let mut out = self.ctx.run_ui(raw, |u| ui(u));
        out.textures_delta.clear();
        self.widgets = out
            .platform_output
            .accesskit_update
            .map(|u| {
                u.nodes
                    .iter()
                    .filter_map(|(_, n)| {
                        let text = n.label().or(n.value())?.to_string();
                        let b = n.bounds()?;
                        let rect = Rect::from_min_max(
                            pos2(b.x0 as f32, b.y0 as f32),
                            pos2(b.x1 as f32, b.y1 as f32),
                        );
                        Some((text, rect, !n.is_disabled()))
                    })
                    .collect()
            })
            .unwrap_or_default();
    }

    /// Lets `secs` pass before the next frame (frames are otherwise a tenth of a second apart).
    pub fn wait(&mut self, secs: f64) {
        self.t += secs;
    }

    /// Where last frame drew the widget that says `text` (exactly, else the first whose text
    /// starts with it).
    pub fn find(&self, text: &str) -> Option<Rect> {
        self.widgets
            .iter()
            .find(|(t, _, _)| t.trim() == text)
            .or_else(|| {
                self.widgets
                    .iter()
                    .find(|(t, _, _)| t.trim().starts_with(text))
            })
            .map(|(_, r, _)| *r)
    }

    /// Whether the first widget whose text starts with `text` takes clicks (none: not drawn).
    pub fn enabled(&self, text: &str) -> Option<bool> {
        self.widgets
            .iter()
            .find(|(t, _, _)| t.trim().starts_with(text))
            .map(|(_, _, on)| *on)
    }

    /// Every enabled widget whose text starts with `text`, in drawing order.
    pub fn all(&self, text: &str) -> Vec<Rect> {
        self.widgets
            .iter()
            .filter(|(t, _, on)| *on && t.trim().starts_with(text))
            .map(|(_, r, _)| *r)
            .collect()
    }

    /// Every widget text last frame, for a failing test to print.
    pub fn texts(&self) -> Vec<&str> {
        self.widgets.iter().map(|(t, _, _)| t.as_str()).collect()
    }
}

/// A key pressed and released.
pub fn key(key: Key) -> Vec<Event> {
    [true, false]
        .map(|pressed| Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Modifiers::NONE,
        })
        .to_vec()
}

/// Typed text.
pub fn text(s: &str) -> Vec<Event> {
    vec![Event::Text(s.into())]
}

/// A left click at `at`.
pub fn click(at: Pos2) -> Vec<Event> {
    let b = |pressed| Event::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    vec![Event::PointerMoved(at), b(true), b(false)]
}
