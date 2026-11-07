//! The one scroll rule every list on screen follows: which rows a window of `rows` shows over
//! `len` items, kept on the selection, and what its border says about where you are. Pure, so
//! a list in any tool on any desktop scrolls the same way.

/// The first item to show in a window of `rows` over `len` items, starting from `top` (last
/// frame's first row): moved just enough to keep `sel` in view, and never past the end, so
/// the window is always full when there are enough items. No rows, or nothing to show, is 0.
pub fn window(len: usize, sel: Option<usize>, top: usize, rows: usize) -> usize {
    if rows == 0 || len == 0 {
        return 0;
    }
    let mut top = top;
    if let Some(s) = sel.map(|s| s.min(len - 1)) {
        if s < top {
            top = s;
        } else if s >= top + rows {
            top = s + 1 - rows;
        }
    }
    top.min(len.saturating_sub(rows))
}

/// What a list's border says about where you are, btop style: `3/40` with a selection (one
/// based), `↓ 12 more` without one when items are below the window, and nothing when the
/// whole list fits.
pub fn position(len: usize, sel: Option<usize>, top: usize, rows: usize) -> Option<String> {
    if len <= rows {
        return None;
    }
    match sel {
        Some(s) => Some(format!("{}/{len}", s.min(len - 1) + 1)),
        None => {
            let below = len.saturating_sub(top + rows);
            (below > 0).then(|| format!("↓ {below} more"))
        }
    }
}
