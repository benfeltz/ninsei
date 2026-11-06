//! Paging through a book: Welcome to the Net's cards and CPU Shopper's pages share one control
//! (`crates/app/src/pager.rs`). Page 0 is the book's Home (the Home card, the cover).

/// A turn of the page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Turn {
    Prev,
    Next,
    Home,
    /// Straight to a page (a link, a contents line).
    To(usize),
}

/// Where a turn lands in a book of `count` pages. It never runs off either end.
pub fn turn(at: usize, count: usize, t: Turn) -> usize {
    let last = count.saturating_sub(1);
    match t {
        Turn::Prev => at.saturating_sub(1),
        Turn::Next => (at + 1).min(last),
        Turn::Home => 0,
        Turn::To(p) => p.min(last),
    }
    .min(last)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turns_stay_in_the_book() {
        assert_eq!(turn(0, 5, Turn::Prev), 0);
        assert_eq!(turn(4, 5, Turn::Next), 4);
        assert_eq!(turn(2, 5, Turn::Next), 3);
        assert_eq!(turn(3, 5, Turn::Home), 0);
        assert_eq!(turn(1, 5, Turn::To(99)), 4);
        assert_eq!(
            turn(9, 5, Turn::Prev),
            4,
            "a stale page snaps back into the book"
        );
        assert_eq!(turn(0, 0, Turn::Next), 0);
    }
}
