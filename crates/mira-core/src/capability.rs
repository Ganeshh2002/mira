//! The capability vocabulary.
//!
//! Every OS-touching feature in Mira is a [`Capability`] with a runtime
//! [`CapabilityStatus`], resolved on the actual machine rather than assumed from a
//! compile-time target (ADR-0005). The UI asks what it *can do*; it never asks what
//! operating system it is on.
//!
//! The types live here, in the dependency-free core, because they cross the IPC
//! boundary. Resolving them is `mira-platform`'s job.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Something Mira might be able to do, which some platforms cannot.
///
/// The list mirrors the capability matrix in
/// `docs/architecture/platform-abstraction.md` §5. That matrix is the checklist:
/// a capability without a row is a capability nobody has been honest about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Capability {
    /// Register an OS-wide keyboard shortcut.
    GlobalShortcut,
    /// Place an icon in the menu bar or notification area.
    TrayIcon,
    /// Receive click events on that icon (as opposed to menu selections only).
    TrayClickEvents,
    /// Start another application with an argv array.
    LaunchApplication,
    /// List processes and read their facts.
    ProcessEnumeration,
    /// Read a process's working directory.
    ProcessWorkingDirectory,
    /// Ask a process to stop, and force it if the user insists.
    ProcessTermination,
    /// List listening sockets.
    PortEnumeration,
    /// Attribute a listening socket to a project.
    PortAttribution,
    /// Notice that the screen was locked or unlocked.
    LockDetection,
    /// Notice that the machine slept or woke.
    SleepDetection,
    /// Read what is currently playing.
    MediaNowPlaying,
    /// Control playback.
    MediaControl,
    /// Post a system notification.
    Notifications,
    /// Watch a directory for changes.
    FileWatching,
    /// Show a file in the system file manager.
    RevealInFileManager,
    /// Drag a file out of Mira and into another application.
    DragOutFiles,
    /// Start Mira when the user logs in.
    AutoStart,
    /// Read battery state.
    BatteryInfo,
    /// Update Mira in place.
    AutoUpdate,
    /// Put a short piece of text Mira itself produced on the system clipboard.
    Clipboard,
    /// Ask the operating system not to fall asleep while the user says so.
    KeepAwake,
}

impl Capability {
    /// Every capability, in declaration order.
    ///
    /// Resolution walks this, so a capability added to the enum without a resolver
    /// arm fails the exhaustiveness check in `mira-platform` at compile time.
    pub const ALL: [Self; 22] = [
        Self::GlobalShortcut,
        Self::TrayIcon,
        Self::TrayClickEvents,
        Self::LaunchApplication,
        Self::ProcessEnumeration,
        Self::ProcessWorkingDirectory,
        Self::ProcessTermination,
        Self::PortEnumeration,
        Self::PortAttribution,
        Self::LockDetection,
        Self::SleepDetection,
        Self::MediaNowPlaying,
        Self::MediaControl,
        Self::Notifications,
        Self::FileWatching,
        Self::RevealInFileManager,
        Self::DragOutFiles,
        Self::AutoStart,
        Self::BatteryInfo,
        Self::AutoUpdate,
        Self::Clipboard,
        Self::KeepAwake,
    ];

    /// A short human label, used in Settings and in `Unsupported` errors.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::GlobalShortcut => "Global shortcut",
            Self::TrayIcon => "Tray icon",
            Self::TrayClickEvents => "Tray click events",
            Self::LaunchApplication => "Launch applications",
            Self::ProcessEnumeration => "Process enumeration",
            Self::ProcessWorkingDirectory => "Process working directory",
            Self::ProcessTermination => "Process termination",
            Self::PortEnumeration => "Port enumeration",
            Self::PortAttribution => "Port attribution",
            Self::LockDetection => "Lock detection",
            Self::SleepDetection => "Sleep and wake detection",
            Self::MediaNowPlaying => "Now playing",
            Self::MediaControl => "Playback control",
            Self::Notifications => "Notifications",
            Self::FileWatching => "File watching",
            Self::RevealInFileManager => "Reveal in file manager",
            Self::DragOutFiles => "Drag files out",
            Self::AutoStart => "Start at login",
            Self::BatteryInfo => "Battery",
            Self::AutoUpdate => "Automatic updates",
            Self::Clipboard => "Copy to clipboard",
            Self::KeepAwake => "Keep awake",
        }
    }
}

/// Whether a capability works here, and if not, what to say about it.
///
/// Every `reason` and `detail` string is **user-facing copy**, written to be read by
/// a person rather than parsed from a log. A capability that is `Unavailable` renders
/// as off *with its reason*; it is never hidden and never shown as broken
/// (`docs/architecture/platform-abstraction.md` §2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
#[ts(export)]
pub enum CapabilityStatus {
    /// Works as specified.
    Full,
    /// Works, with a reduction the user can see and should know about.
    #[serde(rename_all = "camelCase")]
    Degraded {
        /// One line, shown next to the control.
        reason: String,
        /// The longer explanation, shown on disclosure.
        detail: String,
    },
    /// Cannot work here.
    #[serde(rename_all = "camelCase")]
    Unavailable {
        /// Why not — true, specific, and not an apology.
        reason: String,
        /// What to do instead, when there is something.
        fallback: Option<String>,
    },
}

impl CapabilityStatus {
    /// Whether the feature can be offered at all.
    #[must_use]
    pub const fn is_usable(&self) -> bool {
        matches!(self, Self::Full | Self::Degraded { .. })
    }

    /// A stable machine key (`full`, `degraded`, `unavailable`) for tests and styling.
    #[must_use]
    pub const fn state(&self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Degraded { .. } => "degraded",
            Self::Unavailable { .. } => "unavailable",
        }
    }
}

/// One capability paired with its resolved status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CapabilityReport {
    /// Which capability.
    pub capability: Capability,
    /// Its label, so the UI does not maintain a second copy of the vocabulary.
    pub label: String,
    /// How it resolved on this machine.
    pub status: CapabilityStatus,
}

impl CapabilityReport {
    /// Pair a capability with a status.
    #[must_use]
    pub fn new(capability: Capability, status: CapabilityStatus) -> Self {
        Self {
            capability,
            label: capability.label().to_owned(),
            status,
        }
    }
}
