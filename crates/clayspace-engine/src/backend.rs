//! Which backend evaluates, and what happens when one cannot.
//!
//! The engine registers backends at runtime and holds every one of them to the
//! CPU scalar reference, so this layer never has to reason about correctness —
//! only about preference. Choosing badly costs speed; it cannot cost accuracy.

use std::time::Duration;

use claycore::{Backend, ClayError};

/// What a refill has actually cost, per backend.
///
/// The reason this exists rather than a constant: the crossover is a property
/// of the *machine*, not of the library. Measured on an M-series Mac, Metal
/// wins a dab's twenty-seven bricks by about 2x. Measured on a 24-thread Linux
/// box with an RTX 5060, both CUDA and Vulkan lose to the CPU at every batch
/// size from 8 bricks to 7600 — so there is no threshold there to move a
/// constant to, and the honest answer is "never".
///
/// One constant cannot be right for both, and neither can a per-backend one:
/// the same backend is fast on one machine and slow on another, depending on
/// the toolkit, the driver and how much CPU there is to lose to. So this
/// records what the refills actually took and routes on that.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct RefillCost {
    /// Nanoseconds per brick, smoothed. `None` until the first measurement.
    per_brick: Option<f64>,
}

impl RefillCost {
    /// How much a batch of `bricks` is predicted to cost, or `None` when
    /// nothing has been measured yet.
    fn predict(&self, bricks: usize) -> Option<f64> {
        Some(self.per_brick? * bricks as f64)
    }

    /// Folds in one measurement.
    ///
    /// An exponential average rather than the last value: refills contend with
    /// whatever else the machine is doing, and one descheduled batch should not
    /// flip the routing for the rest of the session. It is weighted towards
    /// recent samples anyway, so a machine that changes — a laptop leaving a
    /// power-saving state — is followed rather than remembered wrongly.
    fn record(&mut self, bricks: usize, took: Duration) {
        if bricks == 0 {
            return;
        }
        let sample = took.as_nanos() as f64 / bricks as f64;
        self.per_brick = Some(match self.per_brick {
            Some(held) => held * 0.7 + sample * 0.3,
            None => sample,
        });
    }
}

/// Why a backend is the active one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionReason {
    /// Highest-ranked backend this platform offers.
    Automatic,
    /// The user chose it explicitly.
    Override,
    /// A stored override named a backend this machine does not have.
    OverrideUnavailable,
}

/// An engine operation that can be routed to a backend.
///
/// Fallback is recorded per kind rather than per call: OpenCL declines raycast
/// by design, and reporting that on every ray would be noise rather than news.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Operation {
    EvalPoints,
    EvalGrid,
    Raycast,
    Mesh,
}

impl Operation {
    pub fn label(self) -> &'static str {
        match self {
            Self::EvalPoints => "point evaluation",
            Self::EvalGrid => "grid evaluation",
            Self::Raycast => "raycast",
            Self::Mesh => "meshing",
        }
    }
}

/// The order this platform prefers its backends in.
///
/// CUDA leads on Linux as the more mature tier-2 path; Vulkan is a full
/// implementation behind it, so a non-NVIDIA machine still gets a GPU rather
/// than OpenCL's best-effort subset.
fn preference() -> &'static [Backend] {
    #[cfg(target_os = "macos")]
    {
        &[Backend::Metal, Backend::Cpu]
    }
    #[cfg(not(target_os = "macos"))]
    {
        &[
            Backend::Cuda,
            Backend::Vulkan,
            Backend::OpenCl,
            Backend::Cpu,
        ]
    }
}

/// Picks and remembers which backend the session runs on.
#[derive(Debug, Clone)]
pub struct BackendPolicy {
    available: Vec<Backend>,
    active: Backend,
    reason: SelectionReason,
    /// Operations that have fallen back this session, each recorded once.
    fallbacks: Vec<(Operation, Backend)>,
    /// What a refill has cost on the CPU, and on the accelerated backend.
    cpu_refill: RefillCost,
    accelerated_refill: RefillCost,
}

impl BackendPolicy {
    /// Which backend to evaluate a batch of brick refills on, or `None` for
    /// the CPU.
    ///
    /// Routed by batch size, which is what the engine's own header asks for.
    /// A device submission has a fixed cost — about 0.25 ms on an M-series Mac
    /// — that the batch has to earn back, so a handful of residual bricks are
    /// cheaper on the CPU however fast the GPU is once it starts.
    ///
    /// This used to return `None` unconditionally: before ClayCore 0.28.0 the
    /// Metal path paid a full round trip *per brick* and sat 7–10× behind the
    /// CPU at every size (#64). Batched into one dispatch it is now roughly
    /// 2× ahead at a dab and far more on a whole-model fill, so the decision
    /// is a threshold rather than a refusal.
    ///
    /// `active()` still reports what the machine offers, because that is what
    /// the status bar tells the user about. This is only about which backend
    /// does this particular job.
    pub fn refill_backend(&self, bricks: usize) -> Option<&Backend> {
        if self.active == Backend::Cpu {
            return None;
        }
        // Below the threshold the CPU wins on every machine measured, because
        // what is being avoided is the fixed cost of a device submission
        // rather than the throughput of the batch. Kept as a guard so a
        // handful of residual bricks never pays for a dispatch.
        if bricks < Self::GPU_CROSSOVER_BRICKS {
            return None;
        }
        match (
            self.cpu_refill.predict(bricks),
            self.accelerated_refill.predict(bricks),
        ) {
            // Both measured: route on what they actually cost, but only move
            // *away* from the accelerated backend when the CPU is clearly
            // ahead. A sample is one timing on a machine doing other things,
            // and the failure modes are not symmetric — sending a batch to a
            // slightly slower device costs a little, while abandoning a device
            // that is genuinely faster costs on every edit for the rest of the
            // session. The margin is what keeps a near-tie on the default.
            (Some(cpu), Some(accelerated)) => {
                (accelerated <= cpu * Self::CPU_MARGIN).then_some(&self.active)
            }
            // Not measured yet. The accelerated backend is tried, which is
            // both the old behaviour and what produces the sample that
            // replaces it — see `needs_refill_calibration`.
            _ => Some(&self.active),
        }
    }

    /// Whether the routing is still running on the constant rather than on
    /// measurement.
    ///
    /// True until both backends have been timed once. The caller answers it by
    /// splitting one eligible batch — a slice on the CPU, the rest on the
    /// accelerated backend — which costs a fraction of a batch and settles the
    /// question for the session.
    pub fn needs_refill_calibration(&self) -> bool {
        self.active != Backend::Cpu
            && (self.cpu_refill.per_brick.is_none() || self.accelerated_refill.per_brick.is_none())
    }

    /// Records what a refill cost, so the next one routes on evidence.
    ///
    /// `backend` is what [`BackendPolicy::refill_backend`] returned, so `None`
    /// is the CPU.
    pub fn record_refill(&mut self, backend: Option<&Backend>, bricks: usize, took: Duration) {
        match backend {
            Some(_) => self.accelerated_refill.record(bricks, took),
            None => self.cpu_refill.record(bricks, took),
        }
    }

    /// Discards what has been measured, so the next refills are what decide.
    ///
    /// For the warm-up: the first call into a device pays costs no later call
    /// does, and a rate built from it describes the start-up rather than the
    /// work.
    pub fn forget_refill_costs(&mut self) {
        self.cpu_refill = RefillCost::default();
        self.accelerated_refill = RefillCost::default();
    }

    /// What a refill is predicted to cost per brick on each backend, in
    /// nanoseconds — the CPU first. For diagnostics.
    pub fn refill_cost_per_brick(&self) -> (Option<f64>, Option<f64>) {
        (self.cpu_refill.per_brick, self.accelerated_refill.per_brick)
    }

    /// How many bricks a batch needs before an accelerated backend pays.
    ///
    /// Measured by the engine at brick dim 8 on an M-series Mac, which is the
    /// cache configuration this application uses: below about sixteen bricks
    /// the CPU wins, at a dab's twenty-seven Metal is roughly twice as fast.
    ///
    /// This is now only the **floor and the starting guess**. What it protects
    /// is the fixed cost of a device submission, which is a property of the
    /// call rather than of the machine, so it holds everywhere: a handful of
    /// residual bricks is never worth a dispatch.
    ///
    /// Above it the decision is measured, because the crossover turned out not
    /// to be a property of the library at all. On a 24-thread Linux box with
    /// an RTX 5060, both CUDA and Vulkan lose to the CPU at every batch size
    /// from 8 bricks to 7600 — 3.5x and 2.5x — so there was no threshold to
    /// move this number to, and routing a dab to the GPU cost 3x on startup.
    /// One constant cannot be right for both machines, and a per-backend one
    /// cannot either: the same backend is fast on one and slow on another
    /// depending on the toolkit, the driver and how much CPU there is to lose
    /// to. See `RefillCost` and `backend_choice.rs`.
    pub const GPU_CROSSOVER_BRICKS: usize = 16;

    /// How much cheaper the CPU has to measure before it takes the work.
    ///
    /// The accelerated backend keeps a batch unless the CPU beats it by more
    /// than a quarter. Deliberately not symmetric: see
    /// [`BackendPolicy::refill_backend`].
    const CPU_MARGIN: f64 = 1.25;

    /// Discovers what this machine offers and applies `stored_override`.
    ///
    /// The CPU backend is compiled in unconditionally by the engine, so this
    /// cannot fail for want of a candidate.
    pub fn discover(stored_override: Option<Backend>) -> Result<Self, ClayError> {
        Ok(Self::from_available(claycore::backends()?, stored_override))
    }

    /// The same decision over a supplied list, so it can be tested without a
    /// machine that happens to have the right hardware.
    pub fn from_available(available: Vec<Backend>, stored_override: Option<Backend>) -> Self {
        let (active, reason) = match stored_override {
            Some(wanted) if available.contains(&wanted) => (wanted, SelectionReason::Override),
            Some(_) => (Self::rank(&available), SelectionReason::OverrideUnavailable),
            None => (Self::rank(&available), SelectionReason::Automatic),
        };
        Self {
            available,
            active,
            reason,
            fallbacks: Vec::new(),
            cpu_refill: RefillCost::default(),
            accelerated_refill: RefillCost::default(),
        }
    }

    fn rank(available: &[Backend]) -> Backend {
        preference()
            .iter()
            .find(|candidate| available.contains(candidate))
            .cloned()
            // Every build registers CPU, but a machine that somehow offers
            // something else entirely should use it rather than nothing.
            .unwrap_or_else(|| available.first().cloned().unwrap_or(Backend::Cpu))
    }

    pub fn active(&self) -> &Backend {
        &self.active
    }

    pub fn available(&self) -> &[Backend] {
        &self.available
    }

    pub fn reason(&self) -> SelectionReason {
        self.reason
    }

    /// Which operations have run on a fallback path this session.
    pub fn fallbacks(&self) -> &[(Operation, Backend)] {
        &self.fallbacks
    }

    /// Overrides the active backend. Fails if the machine does not offer it.
    pub fn set_override(&mut self, backend: Backend) -> Result<(), UnavailableBackend> {
        if !self.available.contains(&backend) {
            return Err(UnavailableBackend {
                wanted: backend,
                available: self.available.clone(),
            });
        }
        self.active = backend;
        self.reason = SelectionReason::Override;
        Ok(())
    }

    /// Returns to automatic selection.
    pub fn clear_override(&mut self) {
        self.active = Self::rank(&self.available);
        self.reason = SelectionReason::Automatic;
    }

    /// Runs `attempt` on the active backend, falling back to CPU for this
    /// operation alone if the backend declines it.
    ///
    /// A backend that does not implement an operation is routing information,
    /// not a fault: the selected backend stays active for everything it does
    /// support, and the user is not told about it.
    pub fn route<T>(
        &mut self,
        operation: Operation,
        mut attempt: impl FnMut(&Backend) -> Result<T, ClayError>,
    ) -> Result<T, ClayError> {
        match attempt(&self.active) {
            Err(e) if e.is_unsupported() && self.active != Backend::Cpu => {
                self.record_fallback(operation);
                attempt(&Backend::Cpu)
            }
            other => other,
        }
    }

    fn record_fallback(&mut self, operation: Operation) {
        if !self.fallbacks.iter().any(|(op, _)| *op == operation) {
            self.fallbacks.push((operation, self.active.clone()));
        }
    }
}

/// A backend was asked for that this machine does not have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnavailableBackend {
    pub wanted: Backend,
    pub available: Vec<Backend>,
}

impl std::fmt::Display for UnavailableBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names: Vec<_> = self.available.iter().map(ToString::to_string).collect();
        write!(
            f,
            "this machine does not offer the {} backend; it has {}",
            self.wanted,
            names.join(", ")
        )
    }
}

impl std::error::Error for UnavailableBackend {}

impl SelectionReason {
    /// Why this backend, in the words the diagnostics panel shows.
    pub fn label(self) -> &'static str {
        match self {
            Self::Automatic => "escolha automática",
            Self::Override => "escolhido manualmente",
            Self::OverrideUnavailable => "escolha manual indisponível; automática",
        }
    }
}

impl BackendPolicy {
    /// This build and this machine, as a report.
    ///
    /// Built here rather than in the composition root because everything in it
    /// but the graphics adapter is this layer's own knowledge, and a report
    /// assembled from several places is one that goes stale in one of them.
    /// The renderer is filled in afterwards by whoever has a device.
    pub fn diagnostics(&self) -> clayspace_model::Diagnostics {
        clayspace_model::Diagnostics {
            app_version: format!("ClaySpaceDesktop {}", env!("CARGO_PKG_VERSION")),
            engine_version: format!("claycore {}", claycore::version()),
            engine_revision: claycore::revision().to_string(),
            platform: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
            backends: self.available.iter().map(ToString::to_string).collect(),
            active_backend: self.active.to_string(),
            selection: self.reason.label().to_string(),
            fallbacks: self
                .fallbacks
                .iter()
                .map(|(operation, backend)| clayspace_model::Fallback {
                    operation: operation.label().to_string(),
                    declined_by: backend.to_string(),
                })
                .collect(),
            renderer: None,
            stalls: Vec::new(),
            render: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> Vec<Backend> {
        vec![Backend::Cpu, Backend::Metal, Backend::Cuda, Backend::Vulkan]
    }

    #[test]
    fn cpu_is_chosen_when_it_is_all_there_is() {
        let policy = BackendPolicy::from_available(vec![Backend::Cpu], None);
        assert_eq!(policy.active(), &Backend::Cpu);
        assert_eq!(policy.reason(), SelectionReason::Automatic);
    }

    #[test]
    fn the_platform_preference_is_followed() {
        let policy = BackendPolicy::from_available(all(), None);
        let expected = if cfg!(target_os = "macos") {
            Backend::Metal
        } else {
            Backend::Cuda
        };
        assert_eq!(policy.active(), &expected);
    }

    #[test]
    fn a_non_nvidia_machine_still_gets_a_gpu() {
        // Vulkan implements the full interface, so this must not fall to
        // OpenCL's best-effort subset or to CPU.
        let policy = BackendPolicy::from_available(
            vec![Backend::Cpu, Backend::Vulkan, Backend::OpenCl],
            None,
        );
        if cfg!(target_os = "macos") {
            // Metal is absent here, so CPU is correct on this platform.
            assert_eq!(policy.active(), &Backend::Cpu);
        } else {
            assert_eq!(policy.active(), &Backend::Vulkan);
        }
    }

    #[test]
    fn an_override_is_honoured() {
        let mut policy = BackendPolicy::from_available(all(), None);
        policy.set_override(Backend::Cpu).expect("cpu is available");
        assert_eq!(policy.active(), &Backend::Cpu);
        assert_eq!(policy.reason(), SelectionReason::Override);
    }

    #[test]
    fn an_unavailable_override_falls_back_and_says_so() {
        let policy = BackendPolicy::from_available(vec![Backend::Cpu], Some(Backend::Cuda));
        assert_eq!(policy.active(), &Backend::Cpu);
        assert_eq!(
            policy.reason(),
            SelectionReason::OverrideUnavailable,
            "a stored override that is gone must be reported, not silently ignored"
        );
    }

    #[test]
    fn setting_an_unavailable_override_is_refused_with_the_alternatives() {
        let mut policy = BackendPolicy::from_available(vec![Backend::Cpu], None);
        let err = policy
            .set_override(Backend::Cuda)
            .expect_err("cuda is not available");
        assert!(
            err.to_string().contains("cpu"),
            "the message should list what is available: {err}"
        );
        assert_eq!(
            policy.active(),
            &Backend::Cpu,
            "the refusal must not change the active backend"
        );
    }

    #[test]
    fn clearing_an_override_returns_to_the_preference() {
        let mut policy = BackendPolicy::from_available(all(), None);
        let automatic = policy.active().clone();
        policy.set_override(Backend::Cpu).expect("cpu");
        policy.clear_override();
        assert_eq!(policy.active(), &automatic);
        assert_eq!(policy.reason(), SelectionReason::Automatic);
    }

    #[test]
    fn an_unsupported_operation_falls_back_for_that_operation_only() {
        let mut policy = BackendPolicy::from_available(all(), None);
        let selected = policy.active().clone();
        if selected == Backend::Cpu {
            return; // Nothing to fall back from.
        }

        let mut seen = Vec::new();
        let result = policy.route(Operation::Raycast, |backend| {
            seen.push(backend.clone());
            if *backend == Backend::Cpu {
                Ok(42)
            } else {
                Err(make_unsupported())
            }
        });

        assert_eq!(result.expect("the fallback should succeed"), 42);
        assert_eq!(seen, vec![selected.clone(), Backend::Cpu]);
        assert_eq!(
            policy.active(),
            &selected,
            "falling back for one operation must not change the active backend"
        );
        assert_eq!(policy.fallbacks().len(), 1);
    }

    #[test]
    fn a_repeated_fallback_is_recorded_once() {
        let mut policy = BackendPolicy::from_available(all(), None);
        if *policy.active() == Backend::Cpu {
            return;
        }
        for _ in 0..5 {
            let _ = policy.route(Operation::Raycast, |backend| {
                if *backend == Backend::Cpu {
                    Ok(())
                } else {
                    Err(make_unsupported())
                }
            });
        }
        assert_eq!(
            policy.fallbacks().len(),
            1,
            "a fallback should be news once, not on every call"
        );
    }

    #[test]
    fn a_real_failure_is_not_turned_into_a_fallback() {
        let mut policy = BackendPolicy::from_available(all(), None);
        let result: Result<(), _> = policy.route(Operation::EvalPoints, |_| Err(invalid()));
        assert!(result.is_err(), "a genuine error must reach the caller");
        assert!(
            policy.fallbacks().is_empty(),
            "an ordinary failure is not an unsupported operation"
        );
    }

    /// The error a backend returns when it does not implement an operation.
    fn make_unsupported() -> ClayError {
        ClayError::for_testing(claycore::ErrorKind::Unsupported, "test")
    }

    /// An ordinary failure, which must not be mistaken for one.
    fn invalid() -> ClayError {
        ClayError::for_testing(claycore::ErrorKind::InvalidArgument, "test")
    }
}
