//! The crossing to the interface thread.
//!
//! `Observable` holds a `Cell` and the engine's safe wrapper is `Send +
//! !Sync`, so a connection thread cannot hold a ViewModel or a document
//! behind a mutex — that is a borrow-check fact, not a preference. What it can
//! hold is a queue: a parsed request becomes a job, the job is pushed here,
//! the application's event loop drains it between frames with a real
//! `&mut dyn Session`, and the answer comes back down a channel the connection
//! thread is waiting on.
//!
//! Three properties matter and are what the tests check. A job's answer is
//! sent from *inside* the drain, which is what makes "the tool returned" mean
//! "the change happened". Jobs come out in the order they went in, across
//! however many clients, so two agents cannot interleave within one call. And
//! the drain takes a bound, so a burst from an agent delays itself rather than
//! starving the redraw.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;

use crate::session::{Frame, Refusal, RefusalCode, Session};

/// What a job produced.
///
/// The pixels come back unencoded on purpose: PNG and base64 are the
/// connection thread's work, because a megabyte-and-a-half encode inside a
/// frame is a dropped frame for a result nobody is watching in real time.
#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    pub value: Value,
    pub frame: Option<Frame>,
}

impl Answer {
    pub fn value(value: Value) -> Self {
        Self { value, frame: None }
    }

    pub fn with_frame(value: Value, frame: Frame) -> Self {
        Self {
            value,
            frame: Some(frame),
        }
    }
}

pub type Outcome = Result<Answer, Refusal>;

type Work = Box<dyn FnOnce(&mut dyn Session) -> Outcome + Send>;

struct Job {
    work: Work,
    reply: mpsc::Sender<Outcome>,
}

#[derive(Default)]
struct Inner {
    jobs: Mutex<VecDeque<Job>>,
    waker: Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
    closed: AtomicBool,
    /// How many jobs have been applied this session, which is what the status
    /// area and the diagnostics report count.
    applied: AtomicU64,
}

/// Both halves of the crossing. Cloning shares one queue.
#[derive(Clone, Default)]
pub struct JobQueue {
    inner: Arc<Inner>,
}

impl JobQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// How the application is woken.
    ///
    /// The event loop runs on `ControlFlow::Wait` deliberately — an idle
    /// application that redraws forever is the failure `Observable` exists to
    /// prevent — so without this a command would sit in the queue until
    /// somebody moved the mouse.
    pub fn set_waker(&self, waker: impl Fn() + Send + Sync + 'static) {
        *self.inner.waker.lock().expect("the waker is not poisoned") = Some(Box::new(waker));
    }

    /// Queues work and waits for the interface thread to do it.
    ///
    /// Called from a connection thread. The bound is not optional: an
    /// application that has stopped answering must cost a client an error
    /// rather than a thread that never returns.
    pub fn submit<F>(&self, bound: Duration, work: F) -> Outcome
    where
        F: FnOnce(&mut dyn Session) -> Outcome + Send + 'static,
    {
        if self.inner.closed.load(Ordering::SeqCst) {
            return Err(Refusal::new(
                RefusalCode::Failed,
                "the application is closing",
            ));
        }

        let (reply, answer) = mpsc::channel();
        self.inner
            .jobs
            .lock()
            .expect("the queue is not poisoned")
            .push_back(Job {
                work: Box::new(work),
                reply,
            });

        if let Some(waker) = self
            .inner
            .waker
            .lock()
            .expect("the waker is not poisoned")
            .as_ref()
        {
            waker();
        }

        match answer.recv_timeout(bound) {
            Ok(outcome) => outcome,
            Err(RecvTimeoutError::Timeout) => Err(Refusal::new(
                RefusalCode::Failed,
                format!(
                    "the application did not answer within {} ms; it may be holding a \
                     gesture or running work that outlasts the bound",
                    bound.as_millis()
                ),
            )),
            // The queue was closed, or the job was dropped with the drain.
            Err(RecvTimeoutError::Disconnected) => Err(Refusal::new(
                RefusalCode::Failed,
                "the application closed before it answered",
            )),
        }
    }

    /// Does up to `limit` jobs, in arrival order. Returns how many it did.
    ///
    /// Called on the interface thread. The bound is what keeps a burst from an
    /// agent from starving the redraw: the rest wait for the next pass, which
    /// delays them without dropping them.
    pub fn drain(&self, session: &mut dyn Session, limit: usize) -> usize {
        let mut done = 0;
        while done < limit {
            let job = {
                let mut jobs = self.inner.jobs.lock().expect("the queue is not poisoned");
                match jobs.pop_front() {
                    Some(job) => job,
                    None => break,
                }
            };
            let outcome = (job.work)(session);
            // A client that gave up waiting has dropped its receiver. The work
            // has already happened by then; there is nobody to tell.
            let _ = job.reply.send(outcome);
            done += 1;
            self.inner.applied.fetch_add(1, Ordering::Relaxed);
        }
        done
    }

    /// Whether anything is waiting, so the event loop can decide to keep
    /// draining rather than going back to sleep.
    pub fn pending(&self) -> usize {
        self.inner
            .jobs
            .lock()
            .expect("the queue is not poisoned")
            .len()
    }

    /// How many jobs this session has applied.
    pub fn applied(&self) -> u64 {
        self.inner.applied.load(Ordering::Relaxed)
    }

    /// Refuses further work and releases everything waiting.
    pub fn close(&self) {
        self.inner.closed.store(true, Ordering::SeqCst);
        self.inner
            .jobs
            .lock()
            .expect("the queue is not poisoned")
            .clear();
    }

    pub fn is_closed(&self) -> bool {
        self.inner.closed.load(Ordering::SeqCst)
    }
}

impl std::fmt::Debug for JobQueue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobQueue")
            .field("pending", &self.pending())
            .field("applied", &self.applied())
            .field("closed", &self.is_closed())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::FakeSession;
    use serde_json::json;
    use std::sync::atomic::AtomicUsize;
    use std::thread;

    #[test]
    fn a_job_is_answered_from_inside_the_drain() {
        let queue = JobQueue::new();
        let waiting = queue.clone();
        let client = thread::spawn(move || {
            waiting.submit(Duration::from_secs(5), |session| {
                session.apply(clayspace_vm::Command::Undo)?;
                Ok(Answer::value(json!({"ok": true})))
            })
        });

        let mut session = FakeSession::new();
        // Nothing has been applied until the drain runs, which is the property
        // that makes a tool's answer mean the change happened.
        while queue.pending() == 0 {
            thread::yield_now();
        }
        assert!(session.applied.is_empty());
        queue.drain(&mut session, 8);

        let answer = client.join().unwrap().unwrap();
        assert_eq!(answer.value, json!({"ok": true}));
        assert_eq!(session.applied.len(), 1);
    }

    #[test]
    fn jobs_come_out_in_the_order_they_went_in() {
        let queue = JobQueue::new();
        let order = Arc::new(Mutex::new(Vec::new()));

        let mut clients = Vec::new();
        for n in 0..8 {
            // A client per submission, each on its own thread, so this is the
            // several-clients case and not one caller's loop.
            let client_queue = queue.clone();
            let order = Arc::clone(&order);
            clients.push(thread::spawn(move || {
                client_queue.submit(Duration::from_secs(5), move |_| {
                    order.lock().unwrap().push(n);
                    Ok(Answer::value(json!(n)))
                })
            }));
            // One at a time: the guarantee is about *arrival* order, and
            // waiting for each to land is how a test states an arrival order
            // at all rather than asserting on a race.
            while queue.pending() < n + 1 {
                thread::yield_now();
            }
        }

        let mut session = FakeSession::new();
        queue.drain(&mut session, 64);
        for client in clients {
            client.join().unwrap().unwrap();
        }
        assert_eq!(*order.lock().unwrap(), (0..8).collect::<Vec<_>>());
    }

    #[test]
    fn the_drain_takes_a_bound_so_a_burst_cannot_starve_the_redraw() {
        let queue = JobQueue::new();
        for _ in 0..10 {
            let queue = queue.clone();
            thread::spawn(move || {
                queue.submit(Duration::from_secs(5), |_| Ok(Answer::value(json!(1))))
            });
        }
        while queue.pending() < 10 {
            thread::yield_now();
        }

        let mut session = FakeSession::new();
        assert_eq!(queue.drain(&mut session, 4), 4);
        assert_eq!(queue.pending(), 6);
        assert_eq!(queue.drain(&mut session, 100), 6);
        assert_eq!(queue.pending(), 0);
    }

    #[test]
    fn submitting_wakes_the_application() {
        let queue = JobQueue::new();
        let wakes = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&wakes);
        queue.set_waker(move || {
            counted.fetch_add(1, Ordering::SeqCst);
        });

        let waiting = queue.clone();
        let client = thread::spawn(move || {
            waiting.submit(Duration::from_millis(50), |_| Ok(Answer::value(json!(1))))
        });
        // The push and the wake are two steps, so waiting on the queue's depth
        // would be waiting on the wrong one of them.
        let waited = std::time::Instant::now();
        while wakes.load(Ordering::SeqCst) == 0 {
            assert!(waited.elapsed() < Duration::from_secs(5), "never woken");
            thread::yield_now();
        }
        assert_eq!(wakes.load(Ordering::SeqCst), 1);

        let mut session = FakeSession::new();
        queue.drain(&mut session, 1);
        client.join().unwrap().unwrap();
    }

    #[test]
    fn an_application_that_never_drains_costs_an_error_and_not_a_hung_thread() {
        let queue = JobQueue::new();
        let refusal = queue
            .submit(Duration::from_millis(20), |_| Ok(Answer::value(json!(1))))
            .unwrap_err();
        assert_eq!(refusal.code, RefusalCode::Failed);
        assert!(refusal.message.contains("20 ms"), "{}", refusal.message);
    }

    #[test]
    fn a_closed_queue_refuses_rather_than_waiting() {
        let queue = JobQueue::new();
        queue.close();
        let refusal = queue
            .submit(Duration::from_secs(5), |_| Ok(Answer::value(json!(1))))
            .unwrap_err();
        assert!(refusal.message.contains("closing"), "{}", refusal.message);
    }
}
