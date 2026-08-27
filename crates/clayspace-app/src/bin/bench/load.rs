//! What else the machine was doing when the numbers were taken.
//!
//! A benchmark saturates cores by design, so the load *during* a run says
//! nothing useful about competition — by then the run is most of it. The
//! reading that matters is the one taken before anything starts: if the
//! machine is already busy then, every figure after it was measured against
//! someone else's work, and a baseline recorded from it is wrong forever.
//!
//! # Where the thresholds come from
//!
//! Measured on the 24-core Linux reference machine, not chosen for roundness:
//!
//! - a `pytest` run and a database alongside the suite, one-minute load around
//!   5, moved `brush.sdf.mover` by under 2% across three runs — indistinguishable
//!   from a quiet box;
//! - an unrelated process spiking the load to 15 dragged a single brick refill
//!   from a steady 2650 ms to 3314 ms, a 25% error, which is far larger than
//!   the 1.5x tolerance the gate applies.
//!
//! So the harmful reading was about 0.6 runnable threads per core and the
//! harmless one about 0.2. The thresholds sit between them, scaled per core so
//! that a four-core laptop and a many-core workstation are held to the same
//! standard: a load of 5 means something very different on each.

use std::process::Command;

/// Warn above this many runnable threads per core; the figures are probably
/// still usable, but a surprise should be read with the load in mind.
const WARN_PER_CORE: f64 = 0.25;

/// Refuse to *record a baseline* above this. A run can still be measured and
/// compared — only writing a new baseline is blocked, because that is the one
/// mistake a later quiet run cannot undo.
const REFUSE_PER_CORE: f64 = 0.5;

/// The machine's one-minute load, and what it has to spread across.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Load {
    pub one_minute: f64,
    pub cores: usize,
}

impl Load {
    /// Reads the load, or `None` where this platform does not report one.
    ///
    /// Both paths are ordinary safe code: the workspace forbids `unsafe`
    /// outside the two engine crates, which rules out `getloadavg` directly.
    pub fn sample() -> Option<Self> {
        let cores = std::thread::available_parallelism().ok()?.get();
        let one_minute = if cfg!(target_os = "linux") {
            let text = std::fs::read_to_string("/proc/loadavg").ok()?;
            text.split_whitespace().next()?.parse().ok()?
        } else if cfg!(target_os = "macos") {
            // `vm.loadavg` reads `{ 1.83 1.94 2.05 }`.
            let out = Command::new("sysctl")
                .args(["-n", "vm.loadavg"])
                .output()
                .ok()?;
            let text = String::from_utf8(out.stdout).ok()?;
            text.split_whitespace().nth(1)?.parse().ok()?
        } else {
            return None;
        };
        Some(Self { one_minute, cores })
    }

    pub fn per_core(&self) -> f64 {
        self.one_minute / self.cores as f64
    }

    /// Quiet enough that a figure can be taken at face value.
    pub fn is_quiet(&self) -> bool {
        self.per_core() < WARN_PER_CORE
    }

    /// Busy enough that recording a baseline from it would bake in the noise.
    pub fn too_busy_to_record(&self) -> bool {
        self.per_core() >= REFUSE_PER_CORE
    }

    pub fn describe(&self) -> String {
        format!(
            "load {:.2} across {} cores ({:.2} per core)",
            self.one_minute,
            self.cores,
            self.per_core()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(one_minute: f64) -> Load {
        Load {
            one_minute,
            cores: 24,
        }
    }

    #[test]
    fn the_load_that_measured_clean_is_treated_as_quiet() {
        // The pytest-and-database run: three repeats inside 2% of each other.
        assert!(at(5.0).is_quiet());
        assert!(!at(5.0).too_busy_to_record());
    }

    #[test]
    fn the_load_that_cost_twenty_five_percent_refuses_a_baseline() {
        // The load-15 spike, which moved a refill from 2650 ms to 3314 ms.
        assert!(!at(15.0).is_quiet());
        assert!(at(15.0).too_busy_to_record());
    }

    #[test]
    fn the_threshold_is_per_core_not_absolute() {
        // A load of 5 is nothing on 24 cores and serious on 4. An absolute
        // threshold would call these the same, which is the bug this avoids.
        let workstation = Load {
            one_minute: 5.0,
            cores: 24,
        };
        let laptop = Load {
            one_minute: 5.0,
            cores: 4,
        };
        assert!(workstation.is_quiet());
        assert!(laptop.too_busy_to_record());
    }

    #[test]
    fn a_sampled_load_is_never_negative() {
        if let Some(load) = Load::sample() {
            assert!(load.one_minute >= 0.0, "{}", load.describe());
            assert!(load.cores >= 1);
        }
    }
}
