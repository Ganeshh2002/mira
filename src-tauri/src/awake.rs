//! Keep Awake's lifecycle, bound to the application.
//!
//! `mira-platform` owns the policy — one request at a time, a deadline expressed
//! as a timestamp, a capability check in front of both — and knows nothing about
//! when time passes. This is the half that does: it binds that policy to the one
//! clock Mira has, and to the event that tells the interface something moved.
//!
//! Three lifecycle rules live here, and each is visible in one place:
//!
//! 1. **A span that ends, ends.** Choosing thirty minutes arms a
//!    [`Deadline`] — a single wake-up owned by `mira-scheduler`, because that is
//!    the only crate in Mira permitted a clock (`architecture.md` §6). It is not a
//!    poll and not a loop: one sleep, one release, and the task is gone.
//! 2. **It is independent of the scheduler.** Arming a deadline neither starts nor
//!    stops observation, and the observers' gate has no say over it — a lock must
//!    end at the time the person chose even if every window is hidden.
//! 3. **Nothing outlives the process.** The lock is an operating-system request
//!    owned by this process and a timestamp in memory. There is no table for it
//!    and no file, so a Mira that is restarted starts Off — which is the property
//!    that matters most for a feature that changes how a machine behaves.

use std::sync::{Arc, Mutex};

use mira_core::Result;
use mira_platform::{
    Inhibit, KeepAwake, KeepAwakeHost, KeepAwakeSpan, KeepAwakeState, Platform, SystemInhibitor,
};
use mira_scheduler::Deadline;

use crate::clock::now;
use crate::observers::Announce;

/// Keeping this machine awake, for as long as somebody asked.
///
/// Generic over the operating-system mechanism, with the real one as the default,
/// so a test can drive the whole lifecycle — arm, re-arm, expire, release — without
/// touching a real power setting.
pub struct Awake<I = SystemInhibitor> {
    /// The policy and the operating-system request beneath it.
    ///
    /// Held in an `Arc` so an armed deadline can reach it without the deadline
    /// having to reach back into this struct, which would be a cycle.
    keep: Arc<KeepAwake<Platform, I>>,
    /// Told when the lock changes on its own, so the interface can re-read.
    announce: Announce,
    /// The armed wake-up, if the current span has an end. Dropping it disarms it.
    deadline: Mutex<Option<Deadline>>,
}

impl Awake<SystemInhibitor> {
    /// Bind Keep Awake to this machine and this notifier.
    #[must_use]
    pub fn new(platform: Platform, announce: Announce) -> Self {
        Self::with(platform, SystemInhibitor::new(), announce)
    }
}

impl<I: Inhibit + 'static> Awake<I> {
    /// Bind Keep Awake to some other mechanism. The seam tests use.
    #[must_use]
    pub fn with(platform: Platform, inhibitor: I, announce: Announce) -> Self {
        Self {
            keep: Arc::new(KeepAwake::with(platform, inhibitor)),
            announce,
            deadline: Mutex::new(None),
        }
    }

    /// What is being held right now.
    ///
    /// Expires first, so a lock whose time ran out is never reported as still
    /// holding. The deadline is what normally ends a span; this is the belt to
    /// its braces, for the case where the task did not survive to fire.
    #[must_use]
    pub fn state(&self) -> KeepAwakeState {
        self.keep.expire(now())
    }

    /// Hold `span`, replacing whatever was held before.
    ///
    /// # Errors
    ///
    /// [`mira_core::MiraError::Unsupported`] where the capability is off, and
    /// [`mira_core::MiraError::External`] if the operating system refused.
    pub fn set(&self, span: KeepAwakeSpan) -> Result<KeepAwakeState> {
        // Disarmed *first*. From here on no deadline belonging to the previous
        // span can fire, so there is no window in which an expiring half-hour
        // could release the hour that just replaced it.
        self.disarm();

        let state = match self.keep.set(span, now()) {
            Ok(state) => state,
            Err(error) => {
                // The old deadline is gone. A lock still held here would be held
                // for ever, so giving the machine back is the safe answer to a
                // failure — a Keep Awake that stayed on by accident is the worst
                // outcome this feature has.
                self.keep.release();
                return Err(error);
            }
        };

        if let Some(window) = span.duration() {
            let keep = Arc::clone(&self.keep);
            let announce = Arc::clone(&self.announce);
            let ended = move || {
                // Released outright rather than re-checked against the clock.
                // This deadline was armed for exactly this span and is disarmed
                // whenever the span changes, so its firing *is* the span ending.
                // Asking the wall clock again would mean a backwards adjustment —
                // NTP, a manual change, a suspend — could leave the lock held
                // with nothing left to release it.
                keep.release();
                announce();
            };

            // A deadline is spawned, and spawning needs a runtime to spawn onto.
            // An async command already has one; the tray menu and set-up do not,
            // and spawning from there would panic. So: use the runtime we are in
            // if we are in one, and Tauri's otherwise. Preferring the ambient one
            // is also what lets a test drive this against a paused clock.
            let armed = if tokio::runtime::Handle::try_current().is_ok() {
                Deadline::in_time(window, ended)
            } else {
                let runtime = tauri::async_runtime::handle();
                let _entered = runtime.inner().enter();
                Deadline::in_time(window, ended)
            };

            if let Ok(mut deadline) = self.deadline.lock() {
                *deadline = Some(armed);
            }
        }

        Ok(state)
    }

    /// Release everything. What quitting calls.
    ///
    /// Idempotent, and safe to call when nothing was ever held: the point is that
    /// the machine is left exactly as Mira found it.
    pub fn shutdown(&self) {
        self.disarm();
        self.keep.release();
    }

    /// Drop the armed wake-up, if there is one.
    fn disarm(&self) {
        if let Ok(mut deadline) = self.deadline.lock() {
            if let Some(armed) = deadline.take() {
                armed.cancel();
            }
        }
    }
}

impl<I: Inhibit> std::fmt::Debug for Awake<I> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The lock, not the mechanism: what is held is the interesting fact, and
        // an inhibitor's innards are a thread handle nobody can read anyway.
        f.debug_struct("Awake")
            .field("held", &self.keep.state())
            .finish()
    }
}
