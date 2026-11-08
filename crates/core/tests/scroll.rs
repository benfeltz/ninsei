//! The scroll rule: windows over lists, and what their borders say.

use ninsei_core::scroll::{position, window};

#[test]
fn an_empty_list_or_no_rows_shows_from_the_top() {
    assert_eq!(window(0, None, 5, 10), 0);
    assert_eq!(window(0, Some(3), 5, 10), 0);
    assert_eq!(window(40, Some(30), 5, 0), 0);
}

#[test]
fn the_window_follows_the_selection_only_as_far_as_it_must() {
    // Inside the window: it stays put.
    assert_eq!(window(40, Some(12), 10, 8), 10);
    // Below it: the selection becomes the last row.
    assert_eq!(window(40, Some(20), 10, 8), 13);
    // Above it: the selection becomes the first row.
    assert_eq!(window(40, Some(4), 10, 8), 4);
    // A selection past the end counts as the last item.
    assert_eq!(window(40, Some(99), 0, 8), 32);
}

#[test]
fn the_window_never_scrolls_past_the_end() {
    assert_eq!(window(40, None, 39, 8), 32);
    assert_eq!(window(5, None, 3, 8), 0, "a short list shows from the top");
    assert_eq!(window(5, Some(4), 3, 8), 0);
    assert_eq!(window(40, None, 7, 8), 7, "no selection keeps its place");
}

#[test]
fn the_border_says_where_you_are() {
    assert_eq!(position(40, Some(2), 0, 8).as_deref(), Some("3/40"));
    assert_eq!(position(40, None, 0, 8).as_deref(), Some("↓ 32 more"));
    assert_eq!(
        position(40, None, 32, 8),
        None,
        "at the bottom, nothing below"
    );
    assert_eq!(position(8, Some(2), 0, 8), None, "it all fits");
    assert_eq!(position(0, None, 0, 8), None);
}
