//! Coloured text for the BBS screen. Understands Mystic/Renegade pipe codes
//! (`|00`-`|15` foreground, `|16`-`|23` background) and ANSI SGR escapes (`ESC[1;33m`).
//! Colours are the 16 CGA indices; the app decides how to paint them.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub fg: u8,
    pub bg: u8,
    pub text: String,
}

pub const DEFAULT_FG: u8 = 7;
pub const DEFAULT_BG: u8 = 0;

/// Parses one line into colour spans. `fg`/`bg` carry state between lines, as on a real terminal.
pub fn parse_line(line: &str, fg: &mut u8, bg: &mut u8) -> Vec<Span> {
    let mut spans: Vec<Span> = Vec::new();
    let push = |spans: &mut Vec<Span>, fg: u8, bg: u8, ch: char| match spans.last_mut() {
        Some(s) if s.fg == fg && s.bg == bg => s.text.push(ch),
        _ => spans.push(Span {
            fg,
            bg,
            text: ch.to_string(),
        }),
    };
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '|'
            && i + 2 < chars.len()
            && let (Some(a), Some(b)) = (chars[i + 1].to_digit(10), chars[i + 2].to_digit(10))
        {
            let n = (a * 10 + b) as u8;
            if n < 16 {
                *fg = n;
                i += 3;
                continue;
            } else if n < 24 {
                *bg = n - 16;
                i += 3;
                continue;
            }
        }
        if c == '\x1b' && chars.get(i + 1) == Some(&'[') {
            let start = i + 2;
            if let Some(len) = chars[start..]
                .iter()
                .position(|&ch| ch.is_ascii_alphabetic())
            {
                let end = start + len;
                if chars[end] == 'm' {
                    let params: String = chars[start..end].iter().collect();
                    sgr(&params, fg, bg);
                }
                i = end + 1;
                continue;
            }
        }
        push(&mut spans, *fg, *bg, c);
        i += 1;
    }
    spans
}

fn sgr(params: &str, fg: &mut u8, bg: &mut u8) {
    let codes: Vec<u32> = if params.is_empty() {
        vec![0]
    } else {
        params.split(';').filter_map(|p| p.parse().ok()).collect()
    };
    for c in codes {
        match c {
            0 => {
                *fg = DEFAULT_FG;
                *bg = DEFAULT_BG;
            }
            1 => *fg |= 8,
            22 => *fg &= 7,
            30..=37 => *fg = (*fg & 8) | (c - 30) as u8,
            39 => *fg = DEFAULT_FG,
            40..=47 => *bg = (c - 40) as u8,
            49 => *bg = DEFAULT_BG,
            90..=97 => *fg = 8 | (c - 90) as u8,
            _ => {}
        }
    }
}

/// Visible width of a coded line.
pub fn visible_len(line: &str) -> usize {
    let (mut f, mut b) = (DEFAULT_FG, DEFAULT_BG);
    parse_line(line, &mut f, &mut b)
        .iter()
        .map(|s| s.text.chars().count())
        .sum()
}

/// Strips colour codes.
pub fn plain(line: &str) -> String {
    let (mut f, mut b) = (DEFAULT_FG, DEFAULT_BG);
    parse_line(line, &mut f, &mut b)
        .into_iter()
        .map(|s| s.text)
        .collect()
}
