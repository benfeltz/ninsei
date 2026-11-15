//! The run's alert card for the net's AI waking, on the deck that boxes it (the Sense/Net's
//! Stack chrome draws it). While it's open it owns the keyboard (`App::modal_up`).

use ninsei_core::stack_fight::AlertWatch;

/// What the run remembers of the alert between frames.
#[derive(Default)]
pub struct Alerts {
    pub watch: AlertWatch,
    /// The card up now: its two lines.
    pub open: Option<[String; 2]>,
}
