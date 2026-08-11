//! Which backend evaluates, and what happens when one cannot.
//!
//! The engine registers backends at runtime and holds every one of them to the
//! CPU scalar reference, so this layer never has to reason about correctness —
//! only about preference. Choosing badly costs speed; it cannot cost accuracy.

use claycore::{Backend, ClayError};

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
}

impl BackendPolicy {
    /// Which backend to evaluate brick refills on, or `None` for the CPU.
    ///
    /// Deliberately not `active()`. Refill sits on the input-to-visible path,
    /// and the accelerated backends are — today, measured — slower at it than
    /// the CPU reference path: 5.61 ms against 0.77 ms for one dab on Metal,
    /// and ten times worse on a whole-model fill. See ClayCore #64.
    ///
    /// `active()` still reports what the machine offers, because that is what
    /// the status bar is telling the user about and it stays true. This is
    /// only about which one does this particular job.
    pub fn refill_backend(&self) -> Option<&Backend> {
        None
    }

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
            Some(_) => (
                Self::rank(&available),
                SelectionReason::OverrideUnavailable,
            ),
            None => (Self::rank(&available), SelectionReason::Automatic),
        };
        Self {
            available,
            active,
            reason,
            fallbacks: Vec::new(),
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
        let policy =
            BackendPolicy::from_available(vec![Backend::Cpu], Some(Backend::Cuda));
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
        assert!(err.to_string().contains("cpu"), "the message should list what is available: {err}");
        assert_eq!(policy.active(), &Backend::Cpu, "the refusal must not change the active backend");
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
