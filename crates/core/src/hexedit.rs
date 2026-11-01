//! The hex editor's working buffer: a copy of one program's binary, a cursor and nibble entry.
//! Nothing here touches the game; writing the buffer back is [`crate::Action::Patch`].

use crate::binary::{Bin, LEN};

/// Bytes per row in the editor.
pub const ROW: usize = 16;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HexBuffer {
    /// HDD slot this buffer was opened from.
    pub slot: usize,
    /// The bytes as last written, for diffing and revert.
    pub orig: Bin,
    pub buf: Bin,
    pub cursor: usize,
    /// In hex mode, whether the next digit goes into the low nibble.
    pub low_nibble: bool,
    /// Typing goes into the ASCII column instead of the hex column.
    pub ascii: bool,
}

impl HexBuffer {
    pub fn open(slot: usize, bin: Bin) -> Self {
        Self {
            slot,
            orig: bin,
            buf: bin,
            cursor: 0,
            low_nibble: false,
            ascii: false,
        }
    }

    pub fn dirty(&self) -> bool {
        self.buf != self.orig
    }

    pub fn changed(&self, i: usize) -> bool {
        self.buf[i] != self.orig[i]
    }

    /// Types one character. Returns false if the character means nothing here.
    pub fn input(&mut self, ch: char) -> bool {
        if self.ascii {
            if !ch.is_ascii() || ch.is_ascii_control() {
                return false;
            }
            self.buf[self.cursor] = ch as u8;
            self.step(1);
            return true;
        }
        let Some(d) = ch.to_digit(16) else {
            return false;
        };
        let d = d as u8;
        let b = &mut self.buf[self.cursor];
        if self.low_nibble {
            *b = (*b & 0xF0) | d;
            self.step(1);
        } else {
            *b = (d << 4) | (*b & 0x0F);
            self.low_nibble = true;
        }
        true
    }

    /// Moves the cursor by `delta` bytes, clamped to the buffer.
    pub fn step(&mut self, delta: isize) {
        let max = LEN as isize - 1;
        self.cursor = (self.cursor as isize + delta).clamp(0, max) as usize;
        self.low_nibble = false;
    }

    pub fn set_cursor(&mut self, i: usize) {
        self.cursor = i.min(LEN - 1);
        self.low_nibble = false;
    }

    /// Restores one byte, or the whole buffer.
    pub fn revert(&mut self) {
        self.buf = self.orig;
        self.low_nibble = false;
    }

    /// Call after a successful write so the new bytes become the baseline.
    pub fn committed(&mut self, bin: Bin) {
        self.orig = bin;
        self.buf = bin;
        self.low_nibble = false;
    }
}
