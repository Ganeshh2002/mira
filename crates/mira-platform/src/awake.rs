//! Keep Awake — asking the operating system not to fall asleep.
//!
//! What this is: a person says "stay awake", and Mira holds an **operating-system
//! power request** for as long as they said. A presentation, a long build, a
//! download, a screen being watched rather than typed at.
//!
//! What this is **not**, and structurally cannot become
//! ([ADR-0014](../../../docs/adr/0014-keep-awake.md)):
//!
//! - It never synthesises a keystroke, a pointer move, or any other input event.
//!   Nothing in this crate can post an event to the window server, and a guard
//!   test fails the build if the API that would appears anywhere.
//! - It never runs a program. No `caffeinate`, no `powercfg`, no
//!   `systemd-inhibit` — there is no argv here at all.
//! - It does not hide anything from anybody. The lock is visible to the operating
//!   system's own power tooling, it is off by default, it is released when Mira
//!   quits, and it never survives a restart.
//!
//! The lock is expressed as a [`KeepAwakeSpan`] — a fixed vocabulary of four
//! choices — rather than as a number of seconds, so the caller cannot ask for an
//! arbitrary duration and there is nothing to validate.
//!
//! ## Shape
//!
//! [`Inhibit`] is the OS mechanism and the seam: production uses
//! [`SystemInhibitor`], and tests use a double that counts calls without touching
//! the machine's power settings. [`KeepAwake`] is the policy — one lock at a time,
//! a deadline expressed as a timestamp, and a capability check in front of both —
//! and it is a pure function of the clock it is handed, so every lifecycle rule is
//! assertable without waiting for real time to pass.

use std::sync::Mutex;
use std::time::Duration;

use mira_core::{Capability, CapabilityStatus, MiraError, Result};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::inhibit::SystemInhibitor;
use crate::platform::PlatformCapabilities;

/// How long the machine is being asked to stay awake.
///
/// A closed vocabulary, deliberately. The interface picks one of four words; it
/// cannot name a duration, so there is no number arriving from a webview that
/// something downstream has to bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum KeepAwakeSpan {
    /// Let the machine sleep as it normally would.
    Off,
    /// Half an hour, then off on its own.
    ThirtyMinutes,
    /// One hour, then off on its own.
    OneHour,
    /// Until the person turns it off, or Mira quits.
    UntilTurnedOff,
}

impl KeepAwakeSpan {
    /// Every span, in the order the interface offers them.
    pub const ALL: [Self; 4] = [
        Self::Off,
        Self::ThirtyMinutes,
        Self::OneHour,
        Self::UntilTurnedOff,
    ];

    /// How long this span lasts, or `None` for one that does not end on its own.
    #[must_use]
    pub const fn duration(self) -> Option<Duration> {
        match self {
            Self::Off | Self::UntilTurnedOff => None,
            Self::ThirtyMinutes => Some(Duration::from_secs(30 * 60)),
            Self::OneHour => Some(Duration::from_secs(60 * 60)),
        }
    }

    /// The label the interface shows.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::ThirtyMinutes => "30 minutes",
            Self::OneHour => "1 hour",
            Self::UntilTurnedOff => "Until turned off",
        }
    }
}

/// Whether the machine is being kept awake, and on whose terms.
///
/// A state the interface renders rather than an error it translates — including
/// `Unavailable`, which is what a platform with no public way to ask reports
/// (`platform-abstraction.md` §2 rule 4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
#[ts(export)]
pub enum KeepAwakeState {
    /// Nothing is being held. The machine sleeps as it normally would.
    Off,

    /// Sleep is being prevented.
    #[serde(rename_all = "camelCase")]
    On {
        /// Which choice produced this.
        span: KeepAwakeSpan,
        /// When it ends by itself, Unix epoch seconds UTC. `None` means it does
        /// not — it ends when the person says so, or when Mira quits.
        #[ts(type = "number | null")]
        until: Option<i64>,
    },

    /// This machine has no way for Mira to ask.
    #[serde(rename_all = "camelCase")]
    Unavailable {
        /// Why not — true, specific, and not an apology.
        reason: String,
        /// What to do instead, when there is something.
        fallback: Option<String>,
    },
}

/// The operating system's own "do not sleep" mechanism.
///
/// The seam. One implementation talks to the platform; the tests use one that
/// counts. Nothing here takes a duration: expiry is Mira's policy, held in
/// [`KeepAwake`], because a wall-clock deadline is the same idea on every
/// platform and the OS mechanisms disagree about whether they can express one.
pub trait Inhibit: Send + Sync {
    /// Begin preventing sleep. Called only when nothing is currently held.
    ///
    /// # Errors
    ///
    /// [`MiraError::Unsupported`] where this platform has no mechanism, and
    /// [`MiraError::External`] if the mechanism refused.
    fn engage(&self) -> Result<()>;

    /// Stop preventing sleep. Doing this twice is not an error.
    fn release(&self);

    /// Whether a request is outstanding right now.
    fn engaged(&self) -> bool;
}

impl<T: Inhibit + ?Sized> Inhibit for &T {
    fn engage(&self) -> Result<()> {
        (**self).engage()
    }

    fn release(&self) {
        (**self).release();
    }

    fn engaged(&self) -> bool {
        (**self).engaged()
    }
}

/// What Mira may ask about staying awake.
pub trait KeepAwakeHost {
    /// What is being held right now.
    fn state(&self) -> KeepAwakeState;

    /// Hold `span`, replacing whatever was held before.
    ///
    /// [`KeepAwakeSpan::Off`] releases. Anything else takes a request out, and
    /// takes exactly one: asking twice replaces the first rather than stacking a
    /// second on top of it.
    ///
    /// # Errors
    ///
    /// [`MiraError::Unsupported`] where the capability is off, and
    /// [`MiraError::External`] if the operating system refused.
    fn set(&self, span: KeepAwakeSpan, now: i64) -> Result<KeepAwakeState>;

    /// Release the lock if its deadline has passed, and report the state after.
    ///
    /// Called when a deadline fires, and again whenever the state is read, so a
    /// lock that outlived its span is never reported as still holding.
    fn expire(&self, now: i64) -> KeepAwakeState;

    /// Release whatever is held, deadline or not. What quitting calls.
    fn release(&self);
}

/// One lock, held at a time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Held {
    span: KeepAwakeSpan,
    until: Option<i64>,
}

/// Keeping this machine awake.
#[derive(Debug)]
pub struct KeepAwake<P, I = SystemInhibitor> {
    platform: P,
    inhibitor: I,
    held: Mutex<Option<Held>>,
}

impl<P: PlatformCapabilities> KeepAwake<P, SystemInhibitor> {
    /// Keep this machine awake through its own power interface.
    #[must_use]
    pub fn new(platform: P) -> Self {
        Self::with(platform, SystemInhibitor::new())
    }
}

impl<P: PlatformCapabilities, I: Inhibit> KeepAwake<P, I> {
    /// Keep awake through some other mechanism. The seam tests use.
    pub const fn with(platform: P, inhibitor: I) -> Self {
        Self {
            platform,
            inhibitor,
            held: Mutex::new(None),
        }
    }

    /// The lock currently recorded, having first dropped an expired one.
    fn current(&self, now: i64) -> Option<Held> {
        let mut held = self.held.lock().unwrap_or_else(|poisoned| {
            // A panic elsewhere must not leave the machine pinned awake. The
            // lock's contents are two plain values, so there is nothing to be
            // inconsistent about; taking them back is the safe answer.
            self.held.clear_poison();
            poisoned.into_inner()
        });

        let expired = held.is_some_and(|lock| lock.until.is_some_and(|until| now >= until));
        if expired {
            *held = None;
            self.inhibitor.release();
        }

        *held
    }

    /// The capability's status, or `None` when it is usable.
    fn refusal(&self) -> Option<KeepAwakeState> {
        match self.platform.status(Capability::KeepAwake) {
            CapabilityStatus::Full | CapabilityStatus::Degraded { .. } => None,
            CapabilityStatus::Unavailable { reason, fallback } => {
                Some(KeepAwakeState::Unavailable { reason, fallback })
            }
        }
    }
}

impl<P: PlatformCapabilities, I: Inhibit> KeepAwakeHost for KeepAwake<P, I> {
    fn state(&self) -> KeepAwakeState {
        if let Some(unavailable) = self.refusal() {
            return unavailable;
        }
        // Read without a clock: the caller that has one calls `expire` first.
        // Everything that matters for correctness happens there, so a read that
        // cannot supply the time still answers rather than lying by omission.
        match self.held.lock() {
            Ok(held) => held.map_or(KeepAwakeState::Off, |lock| KeepAwakeState::On {
                span: lock.span,
                until: lock.until,
            }),
            Err(_) => KeepAwakeState::Off,
        }
    }

    fn set(&self, span: KeepAwakeSpan, now: i64) -> Result<KeepAwakeState> {
        if let Some(KeepAwakeState::Unavailable { reason, .. }) = self.refusal() {
            return Err(MiraError::Unsupported {
                capability: Capability::KeepAwake,
                reason,
            });
        }

        if span == KeepAwakeSpan::Off {
            self.release();
            return Ok(KeepAwakeState::Off);
        }

        // Engage before recording, and only when nothing is engaged. That is what
        // makes "one request at a time" a property of this function rather than
        // of the platform code beneath it: changing 30 minutes to an hour moves
        // the deadline and leaves the same single request in place.
        if !self.inhibitor.engaged() {
            self.inhibitor.engage()?;
        }

        let until = span
            .duration()
            .map(|window| now.saturating_add(i64::try_from(window.as_secs()).unwrap_or(i64::MAX)));

        if let Ok(mut held) = self.held.lock() {
            *held = Some(Held { span, until });
        }

        Ok(KeepAwakeState::On { span, until })
    }

    fn expire(&self, now: i64) -> KeepAwakeState {
        if let Some(unavailable) = self.refusal() {
            return unavailable;
        }

        self.current(now)
            .map_or(KeepAwakeState::Off, |lock| KeepAwakeState::On {
                span: lock.span,
                until: lock.until,
            })
    }

    fn release(&self) {
        if let Ok(mut held) = self.held.lock() {
            *held = None;
        }
        // Released unconditionally rather than only when something was recorded:
        // quitting must leave the machine as it was found even if the record and
        // the operating system had somehow disagreed.
        self.inhibitor.release();
    }
}
