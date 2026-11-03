//! The line being typed, and everything a key can do to it.

/// Something a key does to the line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Edit {
    Insert(char),
    /// Pasted text (newlines become spaces).
    InsertStr(String),
    Left,
    Right,
    WordLeft,
    WordRight,
    Home,
    End,
    Backspace,
    Delete,
    /// `Ctrl+U`: cut everything before the cursor.
    KillToStart,
    /// `Ctrl+K`: cut everything after it.
    KillToEnd,
    /// `Ctrl+W` / `Alt+Backspace`: cut the word before it.
    KillWordBack,
    /// `Alt+D`: cut the word after it.
    KillWordForward,
    /// `Ctrl+Y`: paste what was last cut.
    Yank,
    /// `↑` / `Ctrl+P`: the previous command.
    Prev,
    /// `↓` / `Ctrl+N`: the next one (or back to what you were typing).
    Next,
    /// `Ctrl+R`: search history backwards (again: the next older match).
    SearchBack,
    /// `Tab`: complete the word before the cursor (twice: list the choices).
    Complete,
    /// `Enter`: run the line.
    Submit,
    /// `Ctrl+C`: drop the line.
    Cancel,
    /// `Ctrl+L`: clear the screen.
    ClearScreen,
    /// `Esc`: leave a search, keeping its match.
    Escape,
}

/// What an edit asks of the terminal around the editor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Nothing,
    /// Run this line (already in history).
    Submit(String),
    /// The line was dropped with `Ctrl+C`; this is what it said.
    Cancelled(String),
    ClearScreen,
    /// More than one way to complete: show these.
    List(Vec<String>),
}

/// What to draw: the prompt (a search replaces it), the text and the cursor (in chars).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct View {
    /// Set while searching: `(reverse-i-search)'jack': `, shown instead of the prompt.
    pub search: Option<String>,
    pub text: String,
    pub cursor: usize,
}

#[derive(Clone, Debug, Default)]
struct Search {
    query: String,
    /// The history entry matched, if any.
    hit: Option<usize>,
}

/// One terminal's line, history and kill ring.
#[derive(Clone, Debug)]
pub struct LineEditor {
    line: Vec<char>,
    cursor: usize,
    kill: String,
    history: Vec<String>,
    /// Which history entry is shown while walking it with ↑/↓.
    recall: Option<usize>,
    /// What was being typed before walking history.
    draft: Vec<char>,
    search: Option<Search>,
    /// The last edit was a Tab that couldn't decide (a second one lists the choices).
    tabbed: bool,
    max_history: usize,
}

impl Default for LineEditor {
    fn default() -> Self {
        Self::new(200)
    }
}

impl LineEditor {
    /// An empty line that remembers up to `max_history` commands.
    pub fn new(max_history: usize) -> Self {
        Self {
            line: Vec::new(),
            cursor: 0,
            kill: String::new(),
            history: Vec::new(),
            recall: None,
            draft: Vec::new(),
            search: None,
            tabbed: false,
            max_history: max_history.max(1),
        }
    }

    pub fn text(&self) -> String {
        self.line.iter().collect()
    }

    /// Replaces the line, cursor at the end (e.g. F8 puts in `jack `).
    pub fn set_text(&mut self, text: &str) {
        self.line = text.chars().collect();
        self.cursor = self.line.len();
        self.search = None;
        self.recall = None;
    }

    pub fn clear(&mut self) {
        self.set_text("");
    }

    pub fn history(&self) -> &[String] {
        &self.history
    }

    /// Restores history (e.g. from a save), oldest first.
    pub fn set_history(&mut self, history: Vec<String>) {
        let skip = history.len().saturating_sub(self.max_history);
        self.history = history.into_iter().skip(skip).collect();
        self.recall = None;
    }

    pub fn searching(&self) -> bool {
        self.search.is_some()
    }

    pub fn view(&self) -> View {
        match &self.search {
            Some(s) => {
                let text = s.hit.map_or(String::new(), |i| self.history[i].clone());
                let cursor = s
                    .hit
                    .and_then(|i| self.history[i].find(&s.query))
                    .map_or(0, |b| self.history[s.hit.unwrap_or(0)][..b].chars().count());
                View {
                    search: Some(format!("(reverse-i-search)'{}': ", s.query)),
                    text,
                    cursor,
                }
            }
            None => View {
                search: None,
                text: self.text(),
                cursor: self.cursor,
            },
        }
    }

    /// Applies one edit. `complete` gets the text before the cursor and returns the words that
    /// could finish its last word.
    pub fn apply(&mut self, e: Edit, complete: &dyn Fn(&str) -> Vec<String>) -> Outcome {
        let tab = e == Edit::Complete;
        let out = if self.search.is_some() {
            self.in_search(e, complete)
        } else {
            self.edit(e, complete)
        };
        if !tab {
            self.tabbed = false;
        }
        out
    }

    fn edit(&mut self, e: Edit, complete: &dyn Fn(&str) -> Vec<String>) -> Outcome {
        let n = self.line.len();
        match e {
            Edit::Insert(c) => {
                if !c.is_control() {
                    self.line.insert(self.cursor, c);
                    self.cursor += 1;
                }
            }
            Edit::InsertStr(s) => {
                for c in s.chars() {
                    let c = if c == '\n' || c == '\r' || c == '\t' {
                        ' '
                    } else {
                        c
                    };
                    if !c.is_control() {
                        self.line.insert(self.cursor, c);
                        self.cursor += 1;
                    }
                }
            }
            Edit::Left => self.cursor = self.cursor.saturating_sub(1),
            Edit::Right => self.cursor = (self.cursor + 1).min(n),
            Edit::WordLeft => self.cursor = self.word_start(self.cursor),
            Edit::WordRight => self.cursor = self.word_end(self.cursor),
            Edit::Home => self.cursor = 0,
            Edit::End => self.cursor = n,
            Edit::Backspace => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.line.remove(self.cursor);
                }
            }
            Edit::Delete => {
                if self.cursor < n {
                    self.line.remove(self.cursor);
                }
            }
            Edit::KillToStart => self.cut(0, self.cursor),
            Edit::KillToEnd => self.cut(self.cursor, n),
            Edit::KillWordBack => self.cut(self.word_start(self.cursor), self.cursor),
            Edit::KillWordForward => self.cut(self.cursor, self.word_end(self.cursor)),
            Edit::Yank => {
                let k: Vec<char> = self.kill.chars().collect();
                let len = k.len();
                self.line.splice(self.cursor..self.cursor, k);
                self.cursor += len;
            }
            Edit::Prev => self.recall_by(-1),
            Edit::Next => self.recall_by(1),
            Edit::SearchBack => {
                self.search = Some(Search {
                    query: String::new(),
                    hit: None,
                });
            }
            Edit::Complete => return self.complete(complete),
            Edit::Submit => return self.submit(),
            Edit::Cancel => {
                let was = self.text();
                self.clear();
                return Outcome::Cancelled(was);
            }
            Edit::ClearScreen => return Outcome::ClearScreen,
            Edit::Escape => {}
        }
        Outcome::Nothing
    }

    fn in_search(&mut self, e: Edit, complete: &dyn Fn(&str) -> Vec<String>) -> Outcome {
        let Some(s) = self.search.as_mut() else {
            return Outcome::Nothing;
        };
        match e {
            Edit::Insert(c) => {
                s.query.push(c);
                let from = s.hit.map_or(self.history.len(), |h| h + 1);
                s.hit = find_back(&self.history, &s.query, from);
                Outcome::Nothing
            }
            Edit::Backspace => {
                s.query.pop();
                s.hit = find_back(&self.history, &s.query, self.history.len());
                Outcome::Nothing
            }
            Edit::SearchBack => {
                let from = s.hit.unwrap_or(self.history.len());
                if let Some(h) = find_back(&self.history, &s.query, from) {
                    s.hit = Some(h);
                }
                Outcome::Nothing
            }
            Edit::Cancel => {
                // Back to what was being typed.
                self.search = None;
                Outcome::Nothing
            }
            other => {
                // Anything else takes the match into the line, then does its usual thing
                // (Enter runs it; arrows move in it; Esc just keeps it).
                let hit = s.hit.map(|i| self.history[i].clone());
                self.search = None;
                if let Some(h) = hit {
                    self.set_text(&h);
                }
                if other == Edit::Escape {
                    Outcome::Nothing
                } else {
                    self.edit(other, complete)
                }
            }
        }
    }

    fn cut(&mut self, from: usize, to: usize) {
        if from >= to {
            return;
        }
        self.kill = self.line.drain(from..to).collect();
        self.cursor = from;
    }

    /// Start of the word before `at` (skipping spaces first).
    fn word_start(&self, mut at: usize) -> usize {
        while at > 0 && self.line[at - 1].is_whitespace() {
            at -= 1;
        }
        while at > 0 && !self.line[at - 1].is_whitespace() {
            at -= 1;
        }
        at
    }

    /// End of the word after `at` (skipping spaces first).
    fn word_end(&self, mut at: usize) -> usize {
        let n = self.line.len();
        while at < n && self.line[at].is_whitespace() {
            at += 1;
        }
        while at < n && !self.line[at].is_whitespace() {
            at += 1;
        }
        at
    }

    fn recall_by(&mut self, step: i32) {
        if self.history.is_empty() {
            return;
        }
        let last = self.history.len() - 1;
        let next = match (self.recall, step < 0) {
            (None, true) => {
                self.draft = self.line.clone();
                Some(last)
            }
            (None, false) => None,
            (Some(i), true) => Some(i.saturating_sub(1)),
            (Some(i), false) if i < last => Some(i + 1),
            (Some(_), false) => None,
        };
        self.recall = next;
        self.line = match next {
            Some(i) => self.history[i].chars().collect(),
            None => std::mem::take(&mut self.draft),
        };
        self.cursor = self.line.len();
    }

    fn complete(&mut self, complete: &dyn Fn(&str) -> Vec<String>) -> Outcome {
        let before: String = self.line[..self.cursor].iter().collect();
        let start = before
            .char_indices()
            .rev()
            .find(|(_, c)| c.is_whitespace())
            .map_or(0, |(i, c)| i + c.len_utf8());
        let word = &before[start..];
        let mut cands: Vec<String> = complete(&before)
            .into_iter()
            .filter(|c| c.starts_with(word))
            .collect();
        cands.sort();
        cands.dedup();
        let start_chars = before[..start].chars().count();
        match cands.len() {
            0 => Outcome::Nothing,
            1 => {
                let mut rest: Vec<char> = cands[0].chars().collect();
                rest.push(' ');
                self.line.splice(start_chars..self.cursor, rest.clone());
                self.cursor = start_chars + rest.len();
                Outcome::Nothing
            }
            _ => {
                let common = common_prefix(&cands);
                if common.chars().count() > word.chars().count() {
                    let c: Vec<char> = common.chars().collect();
                    self.line.splice(start_chars..self.cursor, c.clone());
                    self.cursor = start_chars + c.len();
                    Outcome::Nothing
                } else if self.tabbed {
                    self.tabbed = false;
                    Outcome::List(cands)
                } else {
                    self.tabbed = true;
                    Outcome::Nothing
                }
            }
        }
    }

    fn submit(&mut self) -> Outcome {
        let line = self.text();
        self.clear();
        self.draft.clear();
        let t = line.trim();
        if !t.is_empty() && self.history.last().map(String::as_str) != Some(t) {
            self.history.push(t.to_string());
            if self.history.len() > self.max_history {
                self.history.remove(0);
            }
        }
        Outcome::Submit(line)
    }
}

/// The newest history entry before index `before` that contains `query`.
fn find_back(history: &[String], query: &str, before: usize) -> Option<usize> {
    if query.is_empty() {
        return None;
    }
    history[..before.min(history.len())]
        .iter()
        .rposition(|h| h.contains(query))
}

fn common_prefix(words: &[String]) -> String {
    let Some(first) = words.first() else {
        return String::new();
    };
    let mut n = first.len();
    for w in &words[1..] {
        n = n.min(
            first
                .char_indices()
                .zip(w.chars())
                .take_while(|((_, a), b)| a == b)
                .last()
                .map_or(0, |((i, a), _)| i + a.len_utf8()),
        );
    }
    first[..n].to_string()
}
