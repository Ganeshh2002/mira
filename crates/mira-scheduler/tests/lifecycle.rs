//! The scheduler's contract.
//!
//! This is the only clock in Mira (`architecture.md` §6), so its promises are
//! load-bearing for everything built on it: nothing runs while the gate is shut,
//! one observer's failure never touches another's, no observer ever runs twice at
//! once, and shutting down stops everything.
//!
//! Time is paused in most tests, so a five-second interval costs no wall clock and
//! the assertions are about *scheduling*, not about how fast the machine is.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::watch;

use mira_core::MiraError;
use mira_scheduler::{Gate, Observation, Scheduler};

/// An observation that counts its runs and can be told to fail or to be slow.
struct Probe {
    name: &'static str,
    interval: Duration,
    runs: AtomicUsize,
    concurrent: AtomicUsize,
    peak: AtomicUsize,
    fails: bool,
    takes: Option<Duration>,
    /// Bumped as each run finishes, so a test can await a completed observation
    /// rather than guessing how many yields the blocking pool needs.
    finished: watch::Sender<usize>,
}

impl Probe {
    fn new(name: &'static str, interval: Duration) -> Arc<Self> {
        Arc::new(Self::bare(name, interval))
    }

    fn failing(name: &'static str, interval: Duration) -> Arc<Self> {
        Arc::new(Self {
            fails: true,
            ..Self::bare(name, interval)
        })
    }

    fn slow(name: &'static str, interval: Duration, takes: Duration) -> Arc<Self> {
        Arc::new(Self {
            takes: Some(takes),
            ..Self::bare(name, interval)
        })
    }

    fn bare(name: &'static str, interval: Duration) -> Self {
        Self {
            name,
            interval,
            runs: AtomicUsize::new(0),
            concurrent: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
            fails: false,
            takes: None,
            finished: watch::channel(0).0,
        }
    }

    fn watch(&self) -> watch::Receiver<usize> {
        self.finished.subscribe()
    }

    fn runs(&self) -> usize {
        self.runs.load(Ordering::SeqCst)
    }

    fn peak_concurrency(&self) -> usize {
        self.peak.load(Ordering::SeqCst)
    }
}

impl Observation for Probe {
    fn name(&self) -> &'static str {
        self.name
    }

    fn interval(&self) -> Duration {
        self.interval
    }

    fn observe(&self) -> mira_core::Result<()> {
        let now = self.concurrent.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(now, Ordering::SeqCst);
        self.runs.fetch_add(1, Ordering::SeqCst);

        if let Some(takes) = self.takes {
            std::thread::sleep(takes);
        }

        self.concurrent.fetch_sub(1, Ordering::SeqCst);
        self.finished.send_modify(|done| *done += 1);

        if self.fails {
            return Err(MiraError::external("the probe", "it was asked to fail"));
        }
        Ok(())
    }
}

/// A gate the test opens and shuts.
#[derive(Default)]
struct Switch(Mutex<bool>);

impl Switch {
    fn open() -> Arc<Self> {
        Arc::new(Self(Mutex::new(true)))
    }

    fn shut() -> Arc<Self> {
        Arc::new(Self(Mutex::new(false)))
    }

    fn set(&self, open: bool) {
        *self.0.lock().expect("lock") = open;
    }
}

impl Gate for Switch {
    fn is_open(&self) -> bool {
        *self.0.lock().expect("lock")
    }
}

/// Move paused time forward `count` intervals, waiting for each observation.
///
/// Waiting is the point. Observations run on the blocking pool, which needs real
/// scheduling even when the clock is frozen, so a test that only advanced the
/// clock would be asserting against whatever had happened to finish.
async fn advance(every: Duration, count: u32, done: &mut watch::Receiver<usize>) {
    for _ in 0..count {
        tokio::time::advance(every).await;
        // Awaited without a timeout on purpose. With the clock paused, tokio
        // auto-advances virtual time whenever it thinks it has nothing to do —
        // and it does not know about the blocking pool, so any `timeout` here
        // would expire instantly while the observation was still running.
        done.changed().await.expect("the probe is still alive");
    }
}

/// Move the clock forward expecting *nothing* to happen.
async fn advance_quietly(every: Duration, count: u32) {
    for _ in 0..count {
        tokio::time::advance(every).await;
        tokio::task::yield_now().await;
    }
}

// ── Starting and stopping ────────────────────────────────────────────────────

#[tokio::test(start_paused = true)]
async fn an_observation_runs_once_per_interval() {
    let probe = Probe::new("probe", Duration::from_secs(5));
    let mut done = probe.watch();
    let scheduler = Scheduler::start(vec![probe.clone()], Switch::open());

    advance(Duration::from_secs(5), 3, &mut done).await;
    scheduler.shutdown().await;

    assert_eq!(probe.runs(), 3);
}

#[tokio::test(start_paused = true)]
async fn nothing_runs_before_the_first_interval_elapses() {
    // Starting the scheduler is not an invitation to do work immediately. The
    // first observation is one interval away, so launching Mira costs nothing.
    let probe = Probe::new("probe", Duration::from_secs(5));
    let scheduler = Scheduler::start(vec![probe.clone()], Switch::open());

    tokio::time::advance(Duration::from_secs(4)).await;
    tokio::task::yield_now().await;

    assert_eq!(probe.runs(), 0);
    scheduler.shutdown().await;
}

#[tokio::test(start_paused = true)]
async fn observers_keep_their_own_intervals() {
    let fast = Probe::new("fast", Duration::from_secs(1));
    let slow = Probe::new("slow", Duration::from_secs(10));
    let mut fast_done = fast.watch();
    let mut slow_done = slow.watch();
    let scheduler = Scheduler::start(vec![fast.clone(), slow.clone()], Switch::open());

    advance(Duration::from_secs(1), 10, &mut fast_done).await;
    slow_done
        .changed()
        .await
        .expect("the slow observer ran once");
    scheduler.shutdown().await;

    assert_eq!(fast.runs(), 10);
    assert_eq!(slow.runs(), 1);
}

#[tokio::test(start_paused = true)]
async fn shutdown_stops_every_observer() {
    let probe = Probe::new("probe", Duration::from_secs(5));
    let mut done = probe.watch();
    let scheduler = Scheduler::start(vec![probe.clone()], Switch::open());

    advance(Duration::from_secs(5), 2, &mut done).await;
    scheduler.shutdown().await;
    let after_shutdown = probe.runs();

    advance_quietly(Duration::from_secs(5), 5).await;

    assert_eq!(
        probe.runs(),
        after_shutdown,
        "a shut-down scheduler does no further work, whatever the clock does"
    );
}

#[tokio::test(start_paused = true)]
async fn shutting_down_twice_is_not_an_error() {
    let scheduler = Scheduler::start(
        vec![Probe::new("probe", Duration::from_secs(1))],
        Switch::open(),
    );

    scheduler.shutdown().await;
}

#[tokio::test(start_paused = true)]
async fn a_scheduler_with_nothing_to_observe_starts_and_stops_cleanly() {
    let scheduler = Scheduler::start(Vec::new(), Switch::open());
    advance_quietly(Duration::from_secs(5), 3).await;
    scheduler.shutdown().await;
}

// ── The gate ─────────────────────────────────────────────────────────────────

#[tokio::test(start_paused = true)]
async fn a_shut_gate_means_no_work_at_all() {
    // The idle-CPU budget is this test. With no projects, or with every window
    // hidden, the clock still ticks and nothing is observed.
    let probe = Probe::new("probe", Duration::from_secs(5));
    let scheduler = Scheduler::start(vec![probe.clone()], Switch::shut());

    advance_quietly(Duration::from_secs(5), 10).await;
    scheduler.shutdown().await;

    assert_eq!(probe.runs(), 0);
}

#[tokio::test(start_paused = true)]
async fn work_resumes_when_the_gate_opens_again() {
    let probe = Probe::new("probe", Duration::from_secs(5));
    let mut done = probe.watch();
    let switch = Switch::shut();
    let scheduler = Scheduler::start(vec![probe.clone()], switch.clone());

    advance_quietly(Duration::from_secs(5), 3).await;
    assert_eq!(probe.runs(), 0);

    switch.set(true);
    advance(Duration::from_secs(5), 2, &mut done).await;
    scheduler.shutdown().await;

    assert_eq!(
        probe.runs(),
        2,
        "no backlog is replayed; the missed ticks are gone"
    );
}

// ── Isolation ────────────────────────────────────────────────────────────────

#[tokio::test(start_paused = true)]
async fn one_observer_failing_leaves_the_others_alone() {
    let broken = Probe::failing("broken", Duration::from_secs(5));
    let healthy = Probe::new("healthy", Duration::from_secs(5));
    let mut done = healthy.watch();
    let scheduler = Scheduler::start(vec![broken.clone(), healthy.clone()], Switch::open());

    advance(Duration::from_secs(5), 3, &mut done).await;
    scheduler.shutdown().await;

    assert_eq!(
        healthy.runs(),
        3,
        "a neighbour's failure is not this one's problem"
    );
}

#[tokio::test(start_paused = true)]
async fn an_observer_that_fails_is_tried_again_next_interval() {
    // Port enumeration can fail transiently. Giving up on the first error would
    // mean a machine that hiccuped once shows no services until Mira restarts.
    let broken = Probe::failing("broken", Duration::from_secs(5));
    let mut done = broken.watch();
    let scheduler = Scheduler::start(vec![broken.clone()], Switch::open());

    advance(Duration::from_secs(5), 4, &mut done).await;
    scheduler.shutdown().await;

    assert_eq!(broken.runs(), 4);
}

// ── No overlap ───────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn an_observer_never_runs_twice_at_once() {
    // Real time here: the point is that a slow observation delays its own next
    // run rather than piling up behind it. A repository that takes longer than
    // the interval to read must not spawn a second read on top of the first.
    let slow = Probe::slow("slow", Duration::from_millis(10), Duration::from_millis(60));
    let scheduler = Scheduler::start(vec![slow.clone()], Switch::open());

    tokio::time::sleep(Duration::from_millis(400)).await;
    scheduler.shutdown().await;

    assert!(slow.runs() > 1, "it ran repeatedly: {}", slow.runs());
    assert_eq!(
        slow.peak_concurrency(),
        1,
        "two runs of one observer overlapped"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_slow_observer_does_not_delay_a_fast_one() {
    let slow = Probe::slow(
        "slow",
        Duration::from_millis(10),
        Duration::from_millis(120),
    );
    let fast = Probe::new("fast", Duration::from_millis(10));
    let scheduler = Scheduler::start(vec![slow.clone(), fast.clone()], Switch::open());

    tokio::time::sleep(Duration::from_millis(300)).await;
    scheduler.shutdown().await;

    assert!(
        fast.runs() > slow.runs(),
        "observers are independent: fast {} vs slow {}",
        fast.runs(),
        slow.runs()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn shutdown_waits_for_work_already_in_flight() {
    // Quitting mid-observation must not leave a half-finished read behind. The
    // observation is allowed to finish; the next one never starts.
    let slow = Probe::slow("slow", Duration::from_millis(10), Duration::from_millis(80));
    let scheduler = Scheduler::start(vec![slow.clone()], Switch::open());

    tokio::time::sleep(Duration::from_millis(40)).await;
    scheduler.shutdown().await;

    assert_eq!(
        slow.peak_concurrency(),
        1,
        "nothing was left running after shutdown returned"
    );
    let settled = slow.runs();
    tokio::time::sleep(Duration::from_millis(120)).await;
    assert_eq!(slow.runs(), settled);
}

// ── The one-shot ─────────────────────────────────────────────────────────────
//
// `Deadline` is the other half of "one crate owns every clock": something that
// happens once, later, and usually never — a Keep Awake span reaching its end
// (ADR-0014). These are the promises the feature above it depends on.

#[tokio::test(start_paused = true)]
async fn a_deadline_fires_once_when_its_time_comes() {
    let fired = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&fired);

    let _deadline = mira_scheduler::Deadline::in_time(Duration::from_secs(1800), move || {
        count.fetch_add(1, Ordering::SeqCst);
    });

    tokio::time::sleep(Duration::from_secs(1799)).await;
    assert_eq!(fired.load(Ordering::SeqCst), 0, "not before it is due");

    tokio::time::sleep(Duration::from_secs(2)).await;
    assert_eq!(fired.load(Ordering::SeqCst), 1);

    // And never again. A one-shot is not a slow interval.
    tokio::time::sleep(Duration::from_secs(7200)).await;
    assert_eq!(fired.load(Ordering::SeqCst), 1);
}

#[tokio::test(start_paused = true)]
async fn cancelling_a_deadline_stops_it_firing() {
    // What turning Keep Awake off does, and what changing the span does before it
    // arms the new one.
    let fired = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&fired);

    let deadline = mira_scheduler::Deadline::in_time(Duration::from_secs(1800), move || {
        count.fetch_add(1, Ordering::SeqCst);
    });
    deadline.cancel();

    tokio::time::sleep(Duration::from_secs(3600)).await;
    assert_eq!(fired.load(Ordering::SeqCst), 0);
}

#[tokio::test(start_paused = true)]
async fn dropping_a_deadline_cancels_it_too() {
    // The property that makes the lifecycle safe by construction: whatever holds
    // the deadline going away is enough, so there is no cancel call to forget on a
    // path somebody has not thought about — quitting, for instance.
    let fired = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&fired);

    {
        let _deadline = mira_scheduler::Deadline::in_time(Duration::from_secs(60), move || {
            count.fetch_add(1, Ordering::SeqCst);
        });
    }

    tokio::time::sleep(Duration::from_secs(600)).await;
    assert_eq!(fired.load(Ordering::SeqCst), 0);
}

#[tokio::test(start_paused = true)]
async fn a_deadline_is_not_gated_and_does_not_wait_for_a_scheduler() {
    // Deliberate: a gate answers *should recurring work happen now*, and a lock
    // that stopped counting because every window was hidden would outlive the time
    // the person chose. There is no gate parameter, and this is what asserts it.
    let fired = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&fired);

    let _deadline = mira_scheduler::Deadline::in_time(Duration::from_secs(30), move || {
        count.fetch_add(1, Ordering::SeqCst);
    });

    tokio::time::sleep(Duration::from_secs(31)).await;
    assert_eq!(
        fired.load(Ordering::SeqCst),
        1,
        "a deadline is nobody's observation"
    );
}
