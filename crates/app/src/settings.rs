//! Settings from the hub's tools (System, the Shell), applied to the save. This is the half
//! that needs no window, so it's tested headlessly: the app then redraws for what changed
//! (the palette, the layout, the CRT), which needs the window.

use ninsei_core::{Data, hub::Save};

use crate::home::WbOut;

/// What applying a request did.
#[derive(Debug, PartialEq, Eq)]
pub enum Applied {
    /// The save changed; tell the Shell this.
    Said(String),
    /// The theme changed (`Save::phosphor`): repaint with it. Its label, for the Shell.
    Theme(String),
    /// A monitor, deck or firmware changed: re-apply the look (theme, layout, CRT).
    Hardware(String),
    /// Refused, and why. The save is untouched.
    Refused(String),
    /// Not a setting: the caller handles it.
    Other(WbOut),
}

/// Applies `o` to the save if it's a setting.
pub fn apply(save: &mut Save, data: &Data, o: WbOut) -> Applied {
    let hardware = |r: Result<String, String>| match r {
        Ok(msg) => Applied::Hardware(msg),
        Err(e) => Applied::Refused(e),
    };
    match o {
        WbOut::Glitches(g) => {
            save.glitches = g;
            Applied::Said(format!("damage glitches {}", g.label()))
        }
        WbOut::Intros(on) => {
            save.intros = on;
            Applied::Said(format!("cracktros {}", if on { "on" } else { "off" }))
        }
        WbOut::Theme(id) => {
            let Some(t) = data.theme(&id) else {
                return Applied::Refused(format!("no theme {id}."));
            };
            if let Some(why) = save.theme_blocker(data, &id) {
                return Applied::Refused(why);
            }
            save.phosphor = t.id.clone();
            Applied::Theme(t.label.clone())
        }
        WbOut::Monitor(id) => hardware(save.plug_monitor(data, &id)),
        WbOut::Deck(id) => hardware(save.use_deck(data, &id)),
        WbOut::Firmware(id) => hardware(save.use_firmware(data, &id)),
        other => Applied::Other(other),
    }
}
