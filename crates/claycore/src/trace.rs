//! Which entry points a piece of work actually called.
//!
//! The capability table above this crate names the engine verb each tool
//! invokes, and until now nothing could check that claim: the name is a string
//! in the domain, the call is several layers below it, and the two only have to
//! agree by somebody remembering. `ENTRY_POINTS` closes half of it — the name
//! has to be a symbol the engine has. This closes the other half: the name has
//! to be a symbol the stroke *reaches*.
//!
//! The record is taken in [`crate::error::check`], which every fallible call in
//! this crate passes through, so what is recorded is the same `&'static str`
//! that would name the call in its error. There is nothing to keep in step: a
//! call renamed here is renamed in the trace on the same line.
//!
//! **Only compiled under `test-support`.** A sculpting session must not pay for
//! a bookkeeping it never reads, and a release build of the application does
//! not enable the feature. Inside a test binary the cost is one thread-local
//! read per engine call, against a call that crosses an FFI boundary and does
//! geometry on the other side of it.
//!
//! **Per thread, and deliberately.** The engine's own detail message is
//! thread-local for the same reason, and a recording shared between threads
//! would interleave the work of tests that run beside each other in one binary.
//! Record on the thread that does the work.

use std::cell::RefCell;

thread_local! {
    /// `None` when nothing is recording, which is every thread that has not
    /// asked. Reading it is the whole cost of the trace when it is off.
    static RECORDING: RefCell<Option<Vec<&'static str>>> = const { RefCell::new(None) };
}

/// Notes one engine call. Called from [`crate::error::check`] and nowhere else.
pub(crate) fn note(operation: &'static str) {
    // `try_borrow_mut` rather than `borrow_mut`: a panic here would cross the
    // FFI boundary this crate promises nothing crosses, and a trace is a
    // diagnostic. If it is somehow already borrowed, the call goes unrecorded
    // and the test that cares fails on what is missing.
    RECORDING.with(|slot| {
        if let Ok(mut slot) = slot.try_borrow_mut() {
            if let Some(calls) = slot.as_mut() {
                calls.push(operation);
            }
        }
    });
}

/// Records every engine call made on this thread while it is held.
///
/// Nested recordings are refused rather than merged: two overlapping answers to
/// "what did this call" is a question with no answer, and the refusal is a
/// panic because it can only be a mistake in the test that asked.
#[must_use = "a recording that is dropped immediately records nothing"]
pub struct Recording {
    /// Guards against a recording being taken while one is already running,
    /// and carries nothing else — the calls live in the thread-local, so that
    /// `note` does not have to find this value.
    _private: (),
}

impl Recording {
    /// Starts recording on this thread.
    pub fn start() -> Self {
        RECORDING.with(|slot| {
            let mut slot = slot.borrow_mut();
            assert!(
                slot.is_none(),
                "an engine trace is already recording on this thread"
            );
            *slot = Some(Vec::new());
        });
        Self { _private: () }
    }

    /// Every entry point called since the recording started, in order and with
    /// repeats: a stroke that stamps forty times says so, and a test that only
    /// wants to know *which* verbs ran can collect them into a set.
    pub fn calls(&self) -> Vec<&'static str> {
        RECORDING.with(|slot| {
            slot.borrow()
                .as_ref()
                .expect("a live recording has its list")
                .clone()
        })
    }

    /// Forgets what has been recorded so far, leaving the recording running.
    ///
    /// For the shape every test here has: build a fixture, which calls the
    /// engine a great deal, then ask what *one* stroke did.
    pub fn clear(&self) {
        RECORDING.with(|slot| {
            if let Some(calls) = slot.borrow_mut().as_mut() {
                calls.clear();
            }
        });
    }
}

impl Drop for Recording {
    fn drop(&mut self) {
        RECORDING.with(|slot| {
            if let Ok(mut slot) = slot.try_borrow_mut() {
                *slot = None;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Nothing is recorded until something asks, and nothing after it stops.
    ///
    /// The second half is the one worth asserting: a recording that outlived
    /// its guard would leak into whatever ran next on the thread, and a test
    /// reading it would be told about somebody else's work.
    #[test]
    fn a_trace_records_only_while_it_is_held() {
        note("clay_before_anything_asked");
        {
            let recording = Recording::start();
            note("clay_first");
            note("clay_second");
            assert_eq!(recording.calls(), vec!["clay_first", "clay_second"]);
            recording.clear();
            note("clay_third");
            assert_eq!(
                recording.calls(),
                vec!["clay_third"],
                "clearing leaves the recording running"
            );
        }
        // Nothing to read and nothing to panic on: the slot is empty again.
        note("clay_after_it_stopped");
        let recording = Recording::start();
        assert!(
            recording.calls().is_empty(),
            "a fresh recording was handed the last one's calls"
        );
    }
}
