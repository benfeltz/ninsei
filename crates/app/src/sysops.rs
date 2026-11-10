//! The sysops' clock (141.9.4): the app's running time, at work or idle alike, handed to core's
//! pace (`ninsei_core::ice_study::Pace`) as milliseconds. Core never sees a clock: it's handed
//! elapsed time. The time is egui's (`InputState::time`), so the web build has it too, with no
//! `SystemTime`.

use ninsei_core::ice_study::Pace;

/// The most drafts a frame works: time a hitch or a hidden window piled up is worked off over
/// the frames after it, never all in one.
pub const DRAFTS_PER_FRAME: u32 = 2;

/// Running time not yet handed to the sysops.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Clock {
    /// When it was last read, in seconds of running time.
    last: Option<f64>,
    pace: Pace,
}

impl Clock {
    /// The steps of time due at running time `now` (seconds) since it was last read. With
    /// nobody at work (`busy` false) time banks nothing.
    pub fn steps(&mut self, now: f64, busy: bool) -> u32 {
        let last = self.last.replace(now).unwrap_or(now);
        let ms = ((now - last).max(0.0) * 1000.0) as u64;
        self.pace.steps(ms, busy)
    }

    /// Seconds until the next step.
    pub fn wait(&self) -> f64 {
        self.pace.wait_ms() as f64 / 1000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ninsei_core::tuning::STUDY_DRAFT_MS;

    const DRAFT: f64 = STUDY_DRAFT_MS as f64 / 1000.0;

    #[test]
    fn running_time_turns_into_steps_and_the_remainder_carries() {
        let mut c = Clock::default();
        assert_eq!(c.steps(10.0, true), 0, "the first read starts the clock");
        assert_eq!(c.steps(10.0 + DRAFT * 0.6, true), 0);
        assert_eq!(c.steps(10.0 + DRAFT * 1.2, true), 1);
        assert!((c.wait() - DRAFT * 0.8).abs() < 1e-3, "{}", c.wait());
        assert_eq!(c.steps(10.0 + DRAFT * 2.0, true), 1);
        assert_eq!(
            c.steps(10.0 + DRAFT * 9.0, true),
            7,
            "a long gap, all of it"
        );
    }

    #[test]
    fn time_with_nobody_at_work_banks_nothing() {
        let mut c = Clock::default();
        c.steps(0.0, false);
        assert_eq!(c.steps(DRAFT * 50.0, false), 0);
        assert_eq!(
            c.steps(DRAFT * 50.0, true),
            0,
            "woken now: nothing owed yet"
        );
        assert_eq!(c.steps(DRAFT * 51.0, true), 1);
    }
}
