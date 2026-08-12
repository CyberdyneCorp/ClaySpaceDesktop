//! What blocked the interface, and for how long.
//!
//! A sculpting application is judged on one number: whether the surface moves
//! while the pointer does. Sixty frames a second is 16.7 ms, so anything the
//! interface thread does for longer than about 16 ms is a dropped frame, and
//! the useful question is not *whether* that happens but *which operation did
//! it* — a stall nobody can name is a stall nobody can fix.
//!
//! The threshold and the bookkeeping live here, with no clock: durations are
//! passed in. That is what lets the rules be tested without sleeping.

use std::time::Duration;

/// One frame's worth of work, at 60 Hz.
pub const FRAME: Duration = Duration::from_micros(16_667);

/// An operation that held the interface thread too long.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stall {
    /// What was running, in words a bug report can carry.
    pub operation: String,
    pub took: Duration,
    /// How many times this operation has stalled this session.
    pub count: u32,
}

impl Stall {
    pub fn describe(&self) -> String {
        if self.count > 1 {
            format!(
                "{} {:.0} ms (×{})",
                self.operation,
                self.took.as_secs_f64() * 1000.0,
                self.count
            )
        } else {
            format!(
                "{} {:.0} ms",
                self.operation,
                self.took.as_secs_f64() * 1000.0
            )
        }
    }
}

/// The session's stalls, worst first.
#[derive(Debug, Clone)]
pub struct FrameLog {
    threshold: Duration,
    stalls: Vec<Stall>,
    /// How many stalls have happened, including ones already merged.
    total: u32,
}

impl Default for FrameLog {
    fn default() -> Self {
        Self::with_threshold(FRAME)
    }
}

impl FrameLog {
    pub fn with_threshold(threshold: Duration) -> Self {
        Self {
            threshold,
            stalls: Vec::new(),
            total: 0,
        }
    }

    pub fn threshold(&self) -> Duration {
        self.threshold
    }

    /// Every operation that has stalled, worst first.
    pub fn stalls(&self) -> &[Stall] {
        &self.stalls
    }

    pub fn total(&self) -> u32 {
        self.total
    }

    pub fn is_empty(&self) -> bool {
        self.stalls.is_empty()
    }

    /// Records how long an operation took, and says whether it stalled.
    ///
    /// One entry per operation, keeping the *worst* time and counting the
    /// occurrences. A list with one line per stall would be dominated by
    /// whatever runs most often, which is exactly the operation least worth
    /// looking at: a re-mesh that goes over by a millisecond four hundred
    /// times is not the problem, and the 6-second consolidation buried under
    /// it is.
    pub fn record(&mut self, operation: &str, took: Duration) -> bool {
        if took < self.threshold {
            return false;
        }
        self.total = self.total.saturating_add(1);

        match self
            .stalls
            .iter_mut()
            .find(|stall| stall.operation == operation)
        {
            Some(known) => {
                known.count = known.count.saturating_add(1);
                known.took = known.took.max(took);
            }
            None => self.stalls.push(Stall {
                operation: operation.to_string(),
                took,
                count: 1,
            }),
        }
        self.stalls.sort_by(|a, b| b.took.cmp(&a.took));
        true
    }

    /// The worst stall this session.
    pub fn worst(&self) -> Option<&Stall> {
        self.stalls.first()
    }

    /// The stalls as report lines.
    pub fn lines(&self) -> Vec<String> {
        self.stalls.iter().map(Stall::describe).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }

    #[test]
    fn work_that_fits_in_a_frame_is_not_recorded() {
        let mut log = FrameLog::default();
        assert!(!log.record("re-mesh", ms(4)));
        assert!(!log.record("re-mesh", ms(16)));
        assert!(log.is_empty());
        assert_eq!(log.total(), 0);
    }

    #[test]
    fn work_that_overruns_a_frame_is_recorded_with_its_name() {
        let mut log = FrameLog::default();
        assert!(log.record("consolidar", ms(6400)));
        assert_eq!(log.stalls().len(), 1);
        assert_eq!(log.stalls()[0].operation, "consolidar");
        assert_eq!(log.stalls()[0].took, ms(6400));
        assert_eq!(log.stalls()[0].count, 1);
    }

    #[test]
    fn an_operation_that_stalls_repeatedly_is_one_line_with_a_count() {
        // A list with one line per stall is dominated by whatever runs most
        // often, which is the operation least worth looking at.
        let mut log = FrameLog::default();
        for took in [ms(20), ms(45), ms(22)] {
            log.record("re-mesh", took);
        }
        assert_eq!(log.stalls().len(), 1);
        assert_eq!(log.stalls()[0].count, 3);
        assert_eq!(log.stalls()[0].took, ms(45), "the worst time was not kept");
        assert_eq!(log.total(), 3);
    }

    #[test]
    fn the_worst_offender_is_first_however_often_the_others_run() {
        let mut log = FrameLog::default();
        for _ in 0..400 {
            log.record("re-mesh", ms(18));
        }
        log.record("consolidar", ms(6400));

        assert_eq!(log.worst().expect("a worst").operation, "consolidar");
        assert_eq!(log.stalls()[1].operation, "re-mesh");
    }

    #[test]
    fn a_repeated_stall_says_how_many_times() {
        let mut log = FrameLog::default();
        log.record("re-mesh", ms(20));
        assert_eq!(log.lines(), ["re-mesh 20 ms"]);
        log.record("re-mesh", ms(30));
        assert_eq!(log.lines(), ["re-mesh 30 ms (×2)"]);
    }

    #[test]
    fn the_threshold_is_one_frame_at_sixty_hertz() {
        assert_eq!(FrameLog::default().threshold(), FRAME);
        assert!(FRAME.as_millis() >= 16 && FRAME.as_millis() < 17);
    }

    #[test]
    fn a_tighter_threshold_catches_more() {
        // For a test or a diagnostic session that wants to see everything.
        let mut log = FrameLog::with_threshold(ms(1));
        assert!(log.record("re-mesh", ms(4)));
        assert_eq!(log.stalls().len(), 1);
    }
}
