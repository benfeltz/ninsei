//! The ICE database: a bestiary that fills in as you fight. Every ICE in `data/ice.ron` has an
//! entry; the ones you haven't met stay `????`. It reads `Save::ice_seen` (booked from each
//! run's `History::met` by `Save::record`) and never changes play.

use crate::binary;
use crate::data::{Data, Effect, IceClass, IceDef};
use crate::hub::{IceSeen, Save};

/// One entry: the ICE and what you've seen of it.
pub struct Entry<'a> {
    pub def: &'a IceDef,
    pub seen: Option<&'a IceSeen>,
    /// The program of the version its seen lines belong to (`IceSeen::version`): the one it
    /// ran (`IceSeen::program`), the row's at 1.0.0, or the version in force. `None` when the
    /// save no longer holds it.
    pub program: Option<&'a str>,
}

/// Every ICE, in data order, with what you know of it.
pub fn entries<'a>(save: &'a Save, data: &'a Data) -> Vec<Entry<'a>> {
    data.ice
        .iter()
        .map(|def| {
            let seen = save.ice_seen.iter().find(|s| s.id == def.id);
            let program = seen.and_then(|s| {
                if !s.program.is_empty() {
                    return Some(s.program.as_str());
                }
                if s.version == crate::ice_version::Version::FIRST {
                    return Some(def.program.as_str());
                }
                save.ice_program(&def.id, data)
                    .filter(|(v, _)| *v == s.version)
                    .map(|(_, p)| p)
            });
            Entry { def, seen, program }
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

/// A line of an entry's page, and whether the window mutes it (a listing line not yet seen
/// run).
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

impl Entry<'_> {
    /// Its program, decompiled from the lines you've seen it run: a heading saying how much
    /// is known, then each line numbered, the unseen ones `????`. Empty until you've met it.
    /// Each line comes with whether it's muted: a line not yet seen run.
    pub fn listing(&self) -> Vec<PageLine> {
        let (Some(s), Some(program)) = (self.seen, self.program) else {
            return Vec::new();
        };
        let rows = crate::ice_listing::decompile_def(program, &self.def.verbs, &s.lines);
        let (seen, all) = crate::ice_listing::known(&rows);
        let mut out = vec![PageLine::plain(format!(
            "listing        decompiled, {seen} of {all} lines seen"
        ))];
        out.extend(rows.iter().map(|r| PageLine {
            text: format!("  {:>3} {}", r.no, r.shown()),
            muted: r.text.is_none(),
        }));
        out
    }

    /// The name to list it under: `????` until you've met it.
    pub fn name(&self) -> &str {
        if self.seen.is_some() {
            &self.def.name
        } else {
            "????"
        }
    }

    /// The entry's page, plain text lines. Unmet ICE shows nothing but question marks.
    pub fn lines(&self, save: &Save, data: &Data) -> Vec<String> {
        self.page(save, data).into_iter().map(|l| l.text).collect()
    }

    /// The entry's page, each line with whether the window mutes it.
    pub fn page(&self, save: &Save, data: &Data) -> Vec<PageLine> {
        let Some(s) = self.seen else {
            return ["????", "", "No record. You haven't met this one yet."]
                .map(|t| PageLine::plain(t.into()))
                .into();
        };
        let d = self.def;
        let kind = match d.tier {
            2 => "black ICE",
            0 => "a rogue",
            _ => "ICE",
        };
        let range = if s.lo == s.hi {
            format!("{}", s.lo)
        } else {
            format!("{}-{}", s.lo, s.hi)
        };
        let mut out = vec![
            format!("{}  [{}]  {kind}", d.name, d.class.label()),
            String::new(),
            format!("strength seen  {range}"),
            format!("met            {}", s.met),
            format!("broken         {}", s.broken),
            String::new(),
        ]
        .into_iter()
        .map(PageLine::plain)
        .collect::<Vec<_>>();
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
        out
    }
}
