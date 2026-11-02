//! Secondary panes as things you own: the catalog in `data/hub/panes.ron`. A pane is bought in
//! RIG (`Save::panes`) or comes with a deck's firmware (`FirmwareDef::panes`). The app maps each
//! id to its renderer, and each chrome dresses it its own way.

use serde::Deserialize;

use crate::data::Data;
use crate::hub::Save;

/// One line of `data/hub/panes.ron`.
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PaneDef {
    pub id: String,
    pub name: String,
    pub price: u32,
    pub blurb: String,
}

impl PaneDef {
    pub fn validate(defs: &[PaneDef]) -> Result<(), String> {
        for (i, d) in defs.iter().enumerate() {
            if defs[..i].iter().any(|o| o.id == d.id) {
                return Err(format!("pane `{}` is listed twice", d.id));
            }
            if d.price == 0 {
                return Err(format!(
                    "pane `{}` is free; give it a price (firmware can bundle it)",
                    d.id
                ));
            }
        }
        Ok(())
    }
}

impl Save {
    /// Whether pane `id` is available now: bought, or bundled with the running firmware.
    pub fn has_pane(&self, data: &Data, id: &str) -> bool {
        self.panes.iter().any(|p| p == id) || self.firmware_def(data).panes.iter().any(|p| p == id)
    }

    /// Buys a secondary pane with banked credits.
    pub fn buy_pane(&mut self, data: &Data, id: &str) -> Result<String, String> {
        let d = data
            .panes
            .iter()
            .find(|d| d.id == id)
            .ok_or("no such pane.")?;
        if self.panes.iter().any(|p| p == id) {
            return Err("you already have that.".into());
        }
        if self.bank < d.price {
            return Err(format!(
                "{} is {}¢. the bank has {}¢.",
                d.name, d.price, self.bank
            ));
        }
        self.bank -= d.price;
        self.panes.push(d.id.clone());
        Ok(format!(
            "installed {} (-{}¢). add it from view.",
            d.name, d.price
        ))
    }
}
