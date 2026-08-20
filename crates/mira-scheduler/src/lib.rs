//! The only clock in Mira.
//!
//! `architecture.md` §6: **one scheduler owns every recurring task, and no module
//! starts its own timer.** That rule is what makes the idle-CPU budget a property
//! of the design rather than a hope — if the only clock refuses to fire, it is
//! impossible for anything to poll.
//!
//! The engine here knows nothing about Git, ports, or processes. It knows three
//! things: a list of [`Observation`]s, each with its own interval; a [`Gate`] that
//! says whether recurring work should happen at all right now; and how to stop.
//!
//! Four promises, each held by a test:
//!
//! 1. **Nothing runs while the gate is shut.** With no projects, or with every
//!    window hidden, the clock ticks and no work is done. Missed ticks are not
//!    replayed — a backlog would turn "you came back" into a thundering herd.
//! 2. **One observer never runs twice at once.** Each observer is a loop that
//!    awaits its own observation before sleeping again, so overlap is impossible
//!    by construction rather than prevented by a lock.
//! 3. **Failures are isolated.** An observer that errors is logged, kept, and
//!    tried again next interval. Its neighbours never notice.
//! 4. **Shutdown is complete.** Work already in flight is allowed to finish, no
//!    new work starts, and the call returns only once every task has ended.
//!
//! Observations are **blocking** functions run on the blocking pool: reading a
//! repository, enumerating sockets and walking the process table are syscall
//! work, and `architecture.md` §6 puts syscall work on `spawn_blocking` rather
//! than on the async worker threads.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;
use tokio::task::JoinHandle;

/// One thing worth observing on a schedule.
///
/// Implementations do the work synchronously and report failure by returning it;
/// the scheduler decides when they run and what a failure means.
pub trait Observation: Send + Sync + 'static {
    /// A short name, used when reporting a failure.
    fn name(&self) -> &'static str;

    /// How long to wait between runs.
    ///
    /// Read once, when the observer starts. Intervals are configuration, not
    /// something that changes under the scheduler's feet.
    fn interval(&self) -> Duration;

    /// Do the work. Blocking; runs on the blocking pool.
    ///
    /// # Errors
    ///
    /// Whatever went wrong. The scheduler isolates it: this observer is tried
    /// again next interval and no other observer is affected.
    fn observe(&self) -> mira_core::Result<()>;
}

/// Whether recurring work should happen at all right now.
///
/// The gate is the difference between a scheduler and a busy loop. It is checked
/// after every sleep and before every observation, so closing it stops all work
/// within one interval without cancelling anything.
pub trait Gate: Send + Sync + 'static {
    /// `false` means: the clock may tick, but do nothing with it.
    fn is_open(&self) -> bool;
}

/// A gate that is always open. Useful in tests and wherever gating is not wanted.
#[derive(Debug, Clone, Copy)]
pub struct AlwaysOpen;

impl Gate for AlwaysOpen {
    fn is_open(&self) -> bool {
        true
    }
}

/// Every recurring task in the process.
#[derive(Debug)]
pub struct Scheduler {
    stop: watch::Sender<bool>,
    tasks: Vec<JoinHandle<()>>,
}

impl Scheduler {
    /// Start one task per observation.
    ///
    /// Each begins by waiting out its first interval, so starting the scheduler
    /// costs nothing: launching Mira does not trigger a burst of work.
    ///
    /// # Panics
    ///
    /// Must be called from inside a Tokio runtime, because it spawns. An
    /// application whose set-up runs on the main thread — Tauri's does — has to
    /// enter the runtime first:
    ///
    /// ```ignore
    /// let _guard = tauri::async_runtime::handle().inner().enter();
    /// let scheduler = Scheduler::start(observations, gate);
    /// ```
    #[must_use]
    pub fn start(observations: Vec<Arc<dyn Observation>>, gate: Arc<dyn Gate>) -> Self {
        let (stop, _) = watch::channel(false);

        let tasks = observations
            .into_iter()
            .map(|observation| {
                let stop = stop.subscribe();
                let gate = Arc::clone(&gate);
                tokio::spawn(run(observation, gate, stop))
            })
            .collect();

        Self { stop, tasks }
    }

    /// Stop every task, and wait for the ones already working to finish.
    ///
    /// Returning means nothing is still observing. That is what makes quitting
    /// safe: there is no half-finished read racing the database's last write.
    pub async fn shutdown(self) {
        // A send failure means every receiver is already gone, which is the state
        // this is trying to reach.
        let _ = self.stop.send(true);

        for task in self.tasks {
            let _ = task.await;
        }
    }
}

/// One observer's whole life: sleep, check the gate, observe, repeat.
async fn run(
    observation: Arc<dyn Observation>,
    gate: Arc<dyn Gate>,
    mut stop: watch::Receiver<bool>,
) {
    let interval = observation.interval();

    loop {
        tokio::select! {
            // Biased so a pending stop always wins a race with an elapsed timer,
            // which is what stops one last observation slipping past shutdown.
            biased;
            _ = stop.changed() => return,
            () = tokio::time::sleep(interval) => {}
        }

        if *stop.borrow() {
            return;
        }
        if !gate.is_open() {
            continue;
        }

        let work = Arc::clone(&observation);
        // Awaited, not detached: the next sleep does not begin until this run has
        // finished, so one observer can never overlap itself and a slow one
        // simply runs less often.
        let outcome = tokio::task::spawn_blocking(move || work.observe()).await;

        match outcome {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                eprintln!("Mira could not observe {}: {error}", observation.name());
            }
            Err(joined) => {
                eprintln!(
                    "Mira's {} observer stopped unexpectedly: {joined}",
                    observation.name()
                );
            }
        }
    }
}
