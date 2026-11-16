//! The ICE database: a bestiary that fills in as you fight. Every ICE in `data/ice.ron` has an
//! entry; the ones you haven't met stay `????`. It reads `Save::ice_seen` (booked from each
//! run's fights by `Save::record`): the fight record is yours, whatever you fought. The code is
//! only the sources you own (`Save::ice_sources`): a kind's page lists the newest one, and its
//! changelog compares the ones you own. It never changes play.

use crate::binary;
use crate::data::{Data, Effect, IceClass, IceDef};
use crate::hub::{IceSeen, Save};
use crate::ice_version::{Bump, Version};

/// One entry: the ICE, what you've seen of it, and the sources of it you own.
pub struct Entry<'a> {
    pub def: &'a IceDef,
    pub seen: Option<&'a IceSeen>,
    /// The sources you own of it that still read, as saved: (version, program as booked, its
    /// text, the board it came from).
    owned: Vec<(Version, &'a str, &'a str, &'a str)>,
    /// The sources the page lists, newest version first, one per version: (version, text). Only
    /// the source of the program the save knows that version by (in force, else as last met).
    pub sources: Vec<(Version, &'a str)>,
    /// The board each of `sources` came from (`IceSource::from`; empty: none).
    from: Vec<&'a str>,
    data: &'a Data,
}

/// Every ICE, in data order, with what you know of it, and each net's AI you've met or own the
/// source of (`ai.ron`): a corp's product isn't the bestiary's to list as `????`.
pub fn entries<'a>(save: &'a Save, data: &'a Data) -> Vec<Entry<'a>> {
    data.ice
        .iter()
        .filter(|def| {
            !def.boss
                || save.ice_seen.iter().any(|s| s.id == def.id)
                || save.ice_sources.iter().any(|s| s.id == def.id)
        })
        .map(|def| {
            let seen = save.ice_seen.iter().find(|s| s.id == def.id);
            // (version, program as booked, its text, the board it came from)
            let owned: Vec<(Version, &str, &str, &str)> = save
                .ice_sources
                .iter()
                .filter(|s| s.id == def.id)
                .filter_map(|s| {
                    Some((
                        s.version,
                        s.program.as_str(),
                        s.text(data)?,
                        s.from.as_str(),
                    ))
                })
                .collect();
            // The program the save knows each version by: in force, else as last met.
            let known = |v: Version| -> Option<String> {
                save.version_program(data, &def.id, v).or_else(|| {
                    let s = seen?;
                    std::iter::once((s.version, &s.program))
                        .chain(s.past.iter().rev().map(|p| (p.version, &p.program)))
                        .find(|(w, _)| *w == v)
                        .map(|(_, p)| p.clone())
                })
            };
            // A source of another program at that number is a version that's gone (the row
            // changed under it): it lists nothing, as a fight against the number shows nothing.
            let mut picked: Vec<(Version, &str, &str)> = Vec::new();
            for &(v, program, text, from) in &owned {
                if known(v).is_none_or(|p| p == program) && !picked.iter().any(|(w, ..)| *w == v) {
                    picked.push((v, text, from));
                }
            }
            picked.sort_by_key(|s| std::cmp::Reverse(s.0));
            Entry {
                def,
                seen,
                owned,
                sources: picked.iter().map(|&(v, t, _)| (v, t)).collect(),
                from: picked.iter().map(|&(.., f)| f).collect(),
                data,
            }
        })
        .collect()
}

/// Your programs (starters in the loadout, then the stash) that break `class` at double,
/// by file name. Read from each binary, so hand-patched programs count as they are now.
pub fn matching(save: &Save, data: &Data, class: IceClass) -> Vec<String> {
    let starters = save
        .loadout
        .starters
        .iter()
        .filter_map(|id| data.warez_ix(id))
        .map(|w| binary::encode(&data.warez[w], save.key));
    let stash = save.stash.iter().filter_map(|s| s.bin());
    let mut out: Vec<String> = Vec::new();
    for bin in starters.chain(stash) {
        if let Ok(spec) = binary::check(&bin, save.key)
            && matches!(spec.effect, Effect::Break { vs: Some(c), .. } if c == class)
            && !out.contains(&spec.file())
        {
            out.push(spec.file());
        }
    }
    out
}

/// A version of a kind, as the ICE DB's table lists it: its record if you met it, and whether
/// you own its source.
struct Row {
    version: Version,
    bump: Option<Bump>,
    /// Fights against it (won, lost); `None` if you never met it.
    record: Option<(u32, u32)>,
    source: bool,
}

/// A line of an entry's page, and whether the window mutes it (what isn't there).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageLine {
    pub text: String,
    pub muted: bool,
}

impl PageLine {
    fn plain(text: String) -> PageLine {
        PageLine { text, muted: false }
    }
}

impl<'a> Entry<'a> {
    /// Whether you know it: you've met it or own a source of it.
    pub fn known(&self) -> bool {
        self.seen.is_some() || !self.sources.is_empty()
    }

    /// Every version of it you've met (each number and program once, as `IceSeen::meet` keeps
    /// them) or own the source of, newest first.
    fn rows(&self) -> Vec<Row> {
        let mut out: Vec<Row> = Vec::new();
        let mut met: Vec<(Version, &str)> = Vec::new();
        if let Some(s) = self.seen {
            let all = std::iter::once((s.version, s.bump, s.program.as_str(), s.won, s.lost))
                .chain(
                    s.past
                        .iter()
                        .rev()
                        .map(|p| (p.version, p.bump, p.program.as_str(), p.won, p.lost)),
                );
            for (version, bump, program, won, lost) in all {
                if met.contains(&(version, program)) {
                    continue;
                }
                met.push((version, program));
                out.push(Row {
                    version,
                    bump,
                    record: Some((won, lost)),
                    // Owned only if it's the source of the program met at that number.
                    source: self
                        .owned
                        .iter()
                        .any(|(v, p, ..)| *v == version && *p == program),
                });
            }
        }
        for (v, _) in &self.sources {
            if !out.iter().any(|r| r.version == *v && r.source) && !met.iter().any(|m| m.0 == *v) {
                out.push(Row {
                    version: *v,
                    bump: None,
                    record: None,
                    source: true,
                });
            }
        }
        out.sort_by_key(|r| std::cmp::Reverse(r.version));
        out
    }

    /// Owned source `k` (newest first) listed whole, marked against the next older one you own
    /// (`ice_listing::changes`), under a heading naming its version and what the marks are
    /// against.
    fn source_listing(&self, k: usize, heading: &str) -> Vec<PageLine> {
        let (v, program) = self.sources[k];
        let prev = self.sources.get(k + 1);
        let rows = crate::ice_listing::changes(&self.def.verbs, prev.map(|p| p.1), program);
        let against = prev.map_or(String::new(), |p| format!(", against {}", p.0));
        // A net's AI's program is its maker's release: its name carries the version.
        let shown = if self.def.boss {
            "on file".to_string()
        } else {
            v.to_string()
        };
        let mut out = vec![PageLine::plain(format!("{heading}{shown}{against}"))];
        // The board it came from puts its header above it and a remark under each read it
        // knows: muted, as the fight's decks dim them. A removed line takes none.
        let lines: Vec<(PageLine, Option<String>)> = rows
            .iter()
            .map(|r| {
                let line = PageLine::plain(format!(" {}{:>3} {}", r.mark.glyph(), r.no, r.text));
                let stmt = (r.mark != crate::ice_listing::Mark::Removed).then(|| r.text.clone());
                (line, stmt)
            })
            .collect();
        let notes = crate::ice_listing::notes(self.data, self.from[k], self.def, v);
        let lines = match notes {
            Some((header, sources)) => {
                let rem = |t: String| {
                    let line = PageLine {
                        text: crate::ice_listing::rem_line(&t),
                        muted: true,
                    };
                    (line, None)
                };
                crate::ice_listing::interleave(lines, header, sources, |l| l.1.as_deref(), rem)
            }
            None => lines,
        };
        out.extend(lines.into_iter().map(|(l, _)| l));
        out
    }

    /// The newest source you own, listed whole and marked against the one before it you own;
    /// with none, a line saying there's none.
    pub fn listing(&self) -> Vec<PageLine> {
        if self.sources.is_empty() {
            return vec![PageLine {
                text: "source         none".into(),
                muted: true,
            }];
        }
        self.source_listing(0, "source         ")
    }

    /// The changelog under the listing: a row per version you've met or own the source of (its
    /// number, how it was made, the fights it won and lost, whether you own its source),
    /// newest first, then each older source you own, marked against the one before it. Empty
    /// until there are two versions to list.
    pub fn changelog(&self) -> Vec<PageLine> {
        let rows = self.rows();
        if rows.len() < 2 {
            return Vec::new();
        }
        let mut out = vec![
            PageLine::plain(String::new()),
            PageLine::plain("versions       bump     won  lost  src".into()),
        ];
        out.extend(rows.iter().map(|r| {
            let (won, lost) = r
                .record
                .map_or(("-".to_string(), "-".to_string()), |(w, l)| {
                    (w.to_string(), l.to_string())
                });
            PageLine::plain(format!(
                "  {:<12} {:<6} {won:>5} {lost:>5}  {}",
                r.version.to_string(),
                r.bump.map_or("-", |b| b.word()),
                if r.source { "yes" } else { "-" }
            ))
        }));
        for k in 1..self.sources.len() {
            out.push(PageLine::plain(String::new()));
            out.extend(self.source_listing(k, ""));
        }
        out
    }

    /// The name to list it under: `????` until you've met it or own its source.
    pub fn name(&self) -> &str {
        if self.known() { &self.def.name } else { "????" }
    }

    /// The entry's page, plain text lines. Unknown ICE shows nothing but question marks.
    pub fn lines(&self, save: &Save, data: &Data) -> Vec<String> {
        self.page(save, data).into_iter().map(|l| l.text).collect()
    }

    /// The entry's page, each line with whether the window mutes it.
    pub fn page(&self, save: &Save, data: &Data) -> Vec<PageLine> {
        let none = "No record. You haven't met this one yet.";
        if !self.known() {
            return ["????", "", none].map(|t| PageLine::plain(t.into())).into();
        }
        let d = self.def;
        // A net's AI's name carries its product's version (`WARDEN 4.2`), and its kind its maker.
        let company = data.boss_model_of(d).map(|m| m.company.as_str());
        let mut out = vec![
            match company {
                Some(company) => {
                    format!("{}  [{}]  {company} network AI", d.name, d.class.label())
                }
                None => {
                    let kind = match d.tier {
                        2 => "black ICE",
                        0 => "a rogue",
                        _ => "ICE",
                    };
                    // The version last met, or the newest source when you've never met it.
                    let version = self.seen.map_or_else(|| self.sources[0].0, |s| s.version);
                    format!("{} {version}  [{}]  {kind}", d.name, d.class.label())
                }
            },
            String::new(),
        ];
        match self.seen {
            Some(s) => {
                let range = if s.lo == s.hi {
                    format!("{}", s.lo)
                } else {
                    format!("{}-{}", s.lo, s.hi)
                };
                out.extend([
                    format!("strength seen  {range}"),
                    format!("met            {}", s.met),
                    format!("broken         {}", s.broken),
                ]);
            }
            None => out.push(none.into()),
        }
        out.push(String::new());
        let mut out: Vec<PageLine> = out.into_iter().map(PageLine::plain).collect();
        out.extend(self.listing());
        if d.yields.is_some() {
            out.push(PageLine::plain("  gives up when it's losing".into()));
        }
        out.push(PageLine::plain(String::new()));
        let m = matching(save, data, d.class);
        out.push(PageLine::plain(if m.is_empty() {
            format!(
                "x2 vs {}: nothing you carry. find a breaker for its class.",
                d.class.label()
            )
        } else {
            format!("x2 vs {}: {}", d.class.label(), m.join(", "))
        }));
        out.extend(self.changelog());
        out
    }
}
