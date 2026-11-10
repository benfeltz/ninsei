//! The sysops' clock (141.9.4): the app's running time, at work or idle alike, turned into
//! drafts for the sysops' studies (`ninsei_core::ice_study`). Core never sees a clock: it's
//! handed a number of drafts. The time is egui's (`InputState::time`), so the web build has it
//! too, with no `SystemTime`.

use ninsei_core::tuning::STUDY_DRAFT_MS;

/// The most drafts a frame works: time a hitch or a hidden window piled up is worked off over
/// the frames after it, never all in one.
pub const DRAFTS_PER_FRAME: u32 = 2;

/// Running time not yet worked into drafts.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Clock {
    /// When it was last read, in seconds of running time.
    last: Option<f64>,
    /// Milliseconds owed to the sysops.
    carry: f64,
}

impl Clock {
    /// The drafts due at running time `now` (seconds), at most [`DRAFTS_PER_FRAME`]; the rest
    /// carries to the next frame. With nobody at work (`busy` false) time banks nothing, so a
    /// study woken later starts from scratch, not from the hours nobody was studying.
    pub fn drafts(&mut self, now: f64, busy: bool) -> u32 {
        let last = self.last.replace(now).unwrap_or(now);
        if !busy {
            self.carry = 0.0;
            return 0;
        }
        self.carry += (now - last).max(0.0) * 1000.0;
        let draft = f64::from(STUDY_DRAFT_MS);
        let n = ((self.carry / draft) as u32).min(DRAFTS_PER_FRAME);
        self.carry -= f64::from(n) * draft;
        n
    }

    /// Seconds until the next draft is due (none when one is already owed).
    pub fn wait(&self) -> f64 {
        ((f64::from(STUDY_DRAFT_MS) - self.carry) / 1000.0).max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DRAFT: f64 = STUDY_DRAFT_MS as f64 / 1000.0;

    #[test]
    fn running_time_turns_into_drafts_and_the_remainder_carries() {
        let mut c = Clock::default();
        assert_eq!(c.drafts(10.0, true), 0, "the first read starts the clock");
        assert_eq!(c.drafts(10.0 + DRAFT * 0.6, true), 0);
        assert_eq!(c.drafts(10.0 + DRAFT * 1.2, true), 1);
        assert!((c.wait() - DRAFT * 0.8).abs() < 1e-6, "{}", c.wait());
        assert_eq!(c.drafts(10.0 + DRAFT * 2.0, true), 1);
    }

    #[test]
    fn a_frame_works_at_most_its_share_and_the_rest_waits() {
        let mut c = Clock::default();
        c.drafts(0.0, true);
        assert_eq!(c.drafts(DRAFT * 7.0, true), DRAFTS_PER_FRAME);
        assert_eq!(c.wait(), 0.0, "more are owed");
        let mut total = DRAFTS_PER_FRAME;
        for _ in 0..10 {
            total += c.drafts(DRAFT * 7.0, true);
        }
        assert_eq!(total, 7, "all of it, over the frames after");
    }

    #[test]
    fn time_with_nobody_at_work_banks_nothing() {
        let mut c = Clock::default();
        c.drafts(0.0, false);
        assert_eq!(c.drafts(DRAFT * 50.0, false), 0);
        assert_eq!(
            c.drafts(DRAFT * 50.0, true),
            0,
            "woken now: nothing owed yet"
        );
        assert_eq!(c.drafts(DRAFT * 51.0, true), 1);
    }
}
