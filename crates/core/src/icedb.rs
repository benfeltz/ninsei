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
}

/// Every ICE, in data order, with what you know of it.
pub fn entries<'a>(save: &'a Save, data: &'a Data) -> Vec<Entry<'a>> {
    data.ice
        .iter()
        .map(|def| Entry {
            def,
            seen: save.ice_seen.iter().find(|s| s.id == def.id),
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

impl Entry<'_> {
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
        let Some(s) = self.seen else {
            return vec![
                "????".into(),
                String::new(),
                "No record. You haven't met this one yet.".into(),
            ];
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
        let moves: Vec<&str> = d.verbs.iter().map(|v| v.word()).collect();
        let mut out = vec![
            format!("{}  [{}]  {kind}", d.name, d.class.label()),
            String::new(),
            format!("strength seen  {range}"),
            format!("met            {}", s.met),
            format!("broken         {}", s.broken),
            String::new(),
            format!("moves          {}", moves.join(", ")),
        ];
        if d.yields.is_some() {
            out.push("  gives up when it's losing".into());
        }
        out.push(String::new());
        let m = matching(save, data, d.class);
        if m.is_empty() {
            out.push(format!(
                "x2 vs {}: nothing you carry. find a breaker for its class.",
                d.class.label()
            ));
        } else {
            out.push(format!("x2 vs {}: {}", d.class.label(), m.join(", ")));
        }
        out
    }
}
