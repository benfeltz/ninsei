//! The key table: which key and modifiers do which [`Edit`], after readline's emacs mode, the
//! way egui_term keeps a bindings layout. Plain data, so it's tested and the browser's limits
//! are written down in one place.

use crate::Edit;

/// A key, as far as the editor cares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// A letter (lowercase), for Ctrl/Alt chords.
    Letter(char),
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    Backspace,
    Delete,
    Tab,
    Enter,
    Escape,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

impl Mods {
    pub const NONE: Mods = Mods {
        ctrl: false,
        alt: false,
        shift: false,
    };
    pub const CTRL: Mods = Mods {
        ctrl: true,
        alt: false,
        shift: false,
    };
    pub const ALT: Mods = Mods {
        ctrl: false,
        alt: true,
        shift: false,
    };
}

/// What `key` with `mods` does, if anything. Typed characters aren't here: they're inserts.
pub fn bind(key: Key, mods: Mods) -> Option<Edit> {
    use Key::*;
    let (c, a) = (mods.ctrl, mods.alt);
    Some(match key {
        Left if c => Edit::WordLeft,
        Right if c => Edit::WordRight,
        Left => Edit::Left,
        Right => Edit::Right,
        Up => Edit::Prev,
        Down => Edit::Next,
        Home => Edit::Home,
        End => Edit::End,
        Backspace if a || c => Edit::KillWordBack,
        Backspace => Edit::Backspace,
        Delete if a => Edit::KillWordForward,
        Delete => Edit::Delete,
        Tab if !c && !a => Edit::Complete,
        Enter => Edit::Submit,
        Escape => Edit::Escape,
        Letter(l) if c && !a => match l {
            'a' => Edit::Home,
            'e' => Edit::End,
            'b' => Edit::Left,
            'f' => Edit::Right,
            'h' => Edit::Backspace,
            'd' => Edit::Delete,
            'u' => Edit::KillToStart,
            'k' => Edit::KillToEnd,
            'w' => Edit::KillWordBack,
            'y' => Edit::Yank,
            'p' => Edit::Prev,
            'n' => Edit::Next,
            'r' => Edit::SearchBack,
            'c' => Edit::Cancel,
            'l' => Edit::ClearScreen,
            _ => return None,
        },
        Letter(l) if a && !c => match l {
            'b' => Edit::WordLeft,
            'f' => Edit::WordRight,
            'd' => Edit::KillWordForward,
            _ => return None,
        },
        _ => return None,
    })
}

/// Ctrl+letter chords a browser would act on itself, which the page stops while a terminal has
/// the keyboard (`web/index.html`). Chrome never lets a page have Ctrl+W, Ctrl+T or Ctrl+N, so
/// each of those has another key: Alt+Backspace (kill word), ↓ (next).
pub const BROWSER_CTRL: &[char] = &['a', 'b', 'd', 'e', 'f', 'h', 'k', 'l', 'p', 'r', 'u', 'y'];

/// Chords a browser keeps no matter what.
pub const BROWSER_KEEPS: &[char] = &['w', 't', 'n'];
