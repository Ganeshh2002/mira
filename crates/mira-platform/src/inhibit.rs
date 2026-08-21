//! The operating system's own "please stay awake", per platform.
//!
//! ## macOS
//!
//! `NSProcessInfo`'s activity API — the public, documented way for an application
//! to say *I am doing something for the person in front of me; do not go to sleep
//! yet*. Two flags are asked for, `NSActivityIdleSystemSleepDisabled` and
//! `NSActivityIdleDisplaySleepDisabled`, which is idle sleep of the machine and of
//! the screen. Closing the lid still sleeps the machine, because closing the lid
//! is not idleness and no application should be able to override it.
//!
//! The **block** form is used rather than `beginActivity` / `endActivity`, and the
//! reason is worth writing down: `endActivity:` is an `unsafe` binding in `objc2`
//! (it takes an activity token whose type it cannot check), while
//! `performActivityWithOptions:reason:usingBlock:` is safe — it begins the
//! activity, runs the block, and ends the activity itself. So the request is held
//! for exactly as long as one block runs, and the block is a thread parked on a
//! channel until Mira releases it. That keeps `#![forbid(unsafe_code)]` intact for
//! this crate, and it makes "the request is released" a consequence of the thread
//! ending rather than of a call somebody has to remember to make.
//!
//! One thread exists while a lock is held and none exists otherwise. It sleeps on
//! a channel; it is not a timer and it does not poll.
//!
//! ## Windows and Linux
//!
//! No mechanism yet, and therefore no mechanism — `Capability::KeepAwake` resolves
//! `Unavailable` there with the reason, the interface renders that reason, and
//! nothing is faked in the meantime ([ADR-0014](../../../docs/adr/0014-keep-awake.md)).
//!
//! What is *not* done on any platform, now or later: no synthetic keyboard or
//! pointer event, and no program run to do the job. There is no argv in this file.

use std::sync::mpsc::Sender;
use std::sync::Mutex;
use std::thread::JoinHandle;

use mira_core::Result;

use crate::awake::Inhibit;

/// Why the machine is being kept awake, as the operating system records it.
///
/// macOS shows this string in its own power tooling (`pmset -g assertions`), which
/// is the point: the request is visible to the machine's owner and to anyone
/// administering it, and is written to be read there.
#[cfg(target_os = "macos")]
const REASON: &str = "Mira: Keep Awake is on because you turned it on";

/// One held request: the thread holding it, and the way to let it go.
#[derive(Debug)]
struct Session {
    /// Dropping this ends the block, which ends the activity.
    stop: Sender<()>,
    thread: JoinHandle<()>,
}

/// This machine's power interface.
#[derive(Debug, Default)]
pub struct SystemInhibitor {
    session: Mutex<Option<Session>>,
}

impl SystemInhibitor {
    /// Ask this machine, through whatever public interface it offers.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl Inhibit for SystemInhibitor {
    fn engage(&self) -> Result<()> {
        let Ok(mut session) = self.session.lock() else {
            return Ok(());
        };
        if session.is_some() {
            // Already held. Never two requests for one lock.
            return Ok(());
        }

        *session = Some(hold()?);
        Ok(())
    }

    fn release(&self) {
        let Ok(mut guard) = self.session.lock() else {
            return;
        };
        let Some(session) = guard.take() else {
            return;
        };

        // Dropping the sender wakes the parked block, which returns, which is
        // what ends the activity. The join is what makes "released" true by the
        // time this call returns rather than shortly afterwards.
        drop(session.stop);
        let _ = session.thread.join();
    }

    fn engaged(&self) -> bool {
        self.session.lock().is_ok_and(|session| session.is_some())
    }
}

/// Take out one request, and return the handle that releases it.
#[cfg(target_os = "macos")]
fn hold() -> Result<Session> {
    use block2::RcBlock;
    use mira_core::MiraError;
    use objc2_foundation::{NSActivityOptions, NSProcessInfo, NSString};

    // Idle sleep of the machine and of the screen. Not lid-close, not the power
    // button, not a scheduled shutdown: those are the person's own instructions
    // and an application does not get to overrule them.
    let options = NSActivityOptions::IdleSystemSleepDisabled
        | NSActivityOptions::IdleDisplaySleepDisabled
        | NSActivityOptions::UserInitiated;

    let (stop, parked) = std::sync::mpsc::channel::<()>();
    let (started, ready) = std::sync::mpsc::channel::<()>();

    let thread = std::thread::Builder::new()
        .name("mira-keep-awake".to_owned())
        .spawn(move || {
            let reason = NSString::from_str(REASON);
            let block = RcBlock::new(move || park(&started, &parked));
            NSProcessInfo::processInfo()
                .performActivityWithOptions_reason_usingBlock(options, &reason, &block);
        })
        .map_err(|error| MiraError::External {
            source: "the operating system".to_owned(),
            detail: format!("Keep Awake could not be started: {error}"),
        })?;

    // Wait for the activity to actually have begun, so `engage` returning means
    // the machine is being kept awake rather than about to be.
    let _ = ready.recv();

    Ok(Session { stop, thread })
}

/// Hold the block open until the sender is dropped.
///
/// Blocking on a channel, not sleeping on a clock: this thread consumes nothing
/// while it waits, and it ends the moment [`Inhibit::release`] drops the sender.
#[cfg(target_os = "macos")]
fn park(started: &Sender<()>, parked: &std::sync::mpsc::Receiver<()>) {
    let _ = started.send(());
    // `Err` is the disconnect that release causes, and is the ordinary path out.
    let _ = parked.recv();
}

/// Not a mechanism this platform offers Mira yet.
///
/// Unreachable in practice: `Capability::KeepAwake` resolves `Unavailable` off
/// macOS, and [`crate::awake::KeepAwake`] refuses before reaching here. It exists
/// so the code is total on every platform without a `cfg` anywhere above this
/// crate (ADR-0005).
#[cfg(not(target_os = "macos"))]
fn hold() -> Result<Session> {
    Err(mira_core::MiraError::Unsupported {
        capability: mira_core::Capability::KeepAwake,
        reason: "Mira has no way to ask this operating system to stay awake.".to_owned(),
    })
}
