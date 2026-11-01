//! A l33tsp34k translator for any text in the game: the BBS speaks it, and so can encounters
//! (`leet: true` in `data/events.ron`) or anything else that calls [`translate`].
//!
//! It's tuned to stay readable: one character in, one character out, so layouts keep their width.
//! Pipe colour codes (`|07`) and digits pass through; so do hotkeys in brackets and 8.3 filenames.

/// Translates text into l33tsp34k: a→4, e→3, i→1, o→0, and a word-final s→z, case kept.
/// Every swap is one character for one, so screens keep their width. Hotkeys in brackets
/// (`[F]`, `[ENTER]`), filenames (`DECODR22.ZIP`), digits and pipe colour codes are left alone.
pub fn translate(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut token = String::new();
    let mut in_bracket = false;
    let flush = |token: &mut String, out: &mut String, in_bracket: &mut bool| {
        if is_filename(token) {
            out.push_str(token);
        } else {
            let chars: Vec<char> = token.chars().collect();
            for (i, &c) in chars.iter().enumerate() {
                match c {
                    '[' => *in_bracket = true,
                    ']' => *in_bracket = false,
                    _ => {}
                }
                // A run of s's that ends the word counts as word-final ("Press" -> "Pr3zz").
                let rest_is_s = chars[i + 1..]
                    .iter()
                    .take_while(|n| n.is_alphabetic())
                    .all(|n| n.eq_ignore_ascii_case(&'s'));
                let next_is_letter = !rest_is_s;
                let prev_is_letter = i > 0 && chars[i - 1].is_alphabetic();
                let swapped = if *in_bracket || c == '[' {
                    c
                } else {
                    match c {
                        'a' | 'A' => '4',
                        'e' | 'E' => '3',
                        'i' | 'I' => '1',
                        'o' | 'O' => '0',
                        's' if prev_is_letter && !next_is_letter => 'z',
                        'S' if prev_is_letter && !next_is_letter => 'Z',
                        other => other,
                    }
                };
                out.push(swapped);
            }
        }
        token.clear();
    };
    for c in line.chars() {
        if c.is_whitespace() {
            flush(&mut token, &mut out, &mut in_bracket);
            out.push(c);
        } else {
            token.push(c);
        }
    }
    flush(&mut token, &mut out, &mut in_bracket);
    out
}

/// `DECODR22.ZIP`-style names (after any pipe codes): left as they are.
fn is_filename(token: &str) -> bool {
    let plain = crate::ansi::plain(token);
    let t = plain.trim_end_matches(['.', ',', ':', ')']);
    let Some((name, ext)) = t.rsplit_once('.') else {
        return false;
    };
    let shouty = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_' || c == '-')
    };
    shouty(name) && shouty(ext) && ext.len() <= 3
}
