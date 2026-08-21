//! Capability resolution — deliberately a pure function over observed facts.
//!
//! Every arm below corresponds to a cell of the capability matrix in
//! `docs/architecture/platform-abstraction.md` §5. When the matrix changes, this
//! changes, and the tests in `tests/capability_resolution.rs` are what keep the two
//! from drifting apart.
//!
//! The `reason` and `detail` strings are product copy, not log messages. They are
//! read by a person standing in front of a control that does not work, so they say
//! what is true and, where there is one, what to do instead.

use mira_core::{Capability, CapabilityReport, CapabilityStatus};

use crate::env::{DisplayServer, EnvFacts, LinuxPackaging, Os};

/// Resolve one capability against observed facts.
///
/// Pure: same facts in, same status out, on any machine. That is what makes the
/// Wayland and macOS-media cases testable from a developer's laptop.
#[must_use]
pub fn resolve(capability: Capability, facts: &EnvFacts) -> CapabilityStatus {
    let linux = facts.os == Os::Linux;

    match capability {
        Capability::GlobalShortcut => {
            if facts.display_server == DisplayServer::Wayland {
                unavailable(
                    "Wayland has no protocol for global shortcuts",
                    Some("mira --toggle"),
                )
            } else {
                CapabilityStatus::Full
            }
        }

        Capability::TrayIcon if linux => degraded(
            "The tray menu is the only way in on Linux",
            "Click events are never delivered to Linux tray icons, so every action lives in \
             the menu. Stock GNOME also needs a tray extension before the icon appears at all.",
        ),

        Capability::TrayClickEvents if linux => unavailable(
            "Linux tray icons do not report clicks",
            Some("Use the tray menu"),
        ),

        Capability::ProcessWorkingDirectory if facts.os == Os::Windows => degraded(
            "Windows rarely reveals a process working directory",
            "Reading it requires inspecting another process's memory, which is unreliable and \
             not something Mira will do. Anything that depends on the working directory is \
             matched by executable path instead.",
        ),

        Capability::ProcessTermination if facts.os == Os::Windows => degraded(
            "Windows cannot stop a console program gracefully",
            "There is no portable equivalent of Ctrl-C for another process. Terminate closes \
             the window if there is one and ends the process otherwise, which is closer to \
             taskkill than to a clean shutdown. The confirmation says so before anything is \
             signalled.",
        ),

        Capability::PortAttribution if facts.os == Os::Windows => degraded(
            "Ports are matched by executable path on Windows",
            "Without a process working directory, Mira attributes a listening socket to a \
             project by the program's path. That is less precise, so rows matched this way \
             say how they were matched.",
        ),

        Capability::LockDetection if linux => {
            if facts.has_logind {
                degraded(
                    "Lock detection depends on logind",
                    "Mira reads the locked hint published by systemd-logind. Some lock screens \
                     never set it, and there is no way to tell that case apart from an unlocked \
                     screen.",
                )
            } else {
                degraded(
                    "This system has no logind, so screen locking is invisible",
                    "Mira falls back to window focus and an idle timer to decide when a session \
                     has paused. Sleep and wake detection are unaffected, so battery behaviour \
                     is unchanged.",
                )
            }
        }

        Capability::MediaNowPlaying if facts.os == Os::MacOs => unavailable(
            "Apple restricts now-playing information to entitled applications",
            None,
        ),

        Capability::MediaControl => match facts.os {
            Os::MacOs => unavailable(
                "Apple restricts playback information to entitled applications",
                None,
            ),
            _ => unavailable("Mira reads what is playing but does not control it", None),
        },

        Capability::Notifications if linux => degraded(
            "Notifications need a running notification daemon",
            "Most desktops start one. Without it nothing would be shown, so Mira reports the \
             capability as off rather than firing into the void.",
        ),

        Capability::FileWatching if linux => degraded(
            "Inotify limits how many directories can be watched at once",
            "On a machine with many projects the kernel limit can be reached. Mira then \
             refreshes when a window regains focus instead, and marks the data as stale.",
        ),

        Capability::RevealInFileManager if linux => degraded(
            "Linux file managers open the folder rather than selecting the file",
            "No portable way exists to ask a Linux file manager to highlight one file, so Mira \
             opens the containing directory and the button says that is what it does.",
        ),

        Capability::DragOutFiles if linux => degraded(
            "Dragging files out depends on the desktop and the webview",
            "Support in WebKitGTK varies by distribution. Mira offers the drag but cannot \
             promise the drop will be accepted.",
        ),

        // The clipboard is written natively — `NSPasteboard`, the Win32 clipboard,
        // the X11 selection — and never by handing text to a program.
        Capability::Clipboard if linux => degraded(
            "A copied value lasts while Mira is running",
            "X11 has no clipboard daemon: the application that copied something is the one \
             that serves it to whatever pastes it. A commit id copied from Mira stays \
             available until Mira quits, unless a clipboard manager on this desktop keeps \
             its own copy.",
        ),

        // Keep Awake asks the operating system to stay awake. It never simulates a
        // keystroke, moves a pointer, or manufactures activity of any kind — see
        // ADR-0014 — so where there is no public API to ask with, the answer is that
        // the capability is off, not that Mira found another way.
        Capability::KeepAwake => match facts.os {
            Os::MacOs => CapabilityStatus::Full,
            Os::Windows => unavailable(
                "Mira does not yet hold the Windows power request that prevents sleep",
                Some("Settings → System → Power & battery"),
            ),
            Os::Linux => unavailable(
                "Mira does not yet hold a systemd-logind sleep inhibitor",
                Some("your desktop's power settings"),
            ),
        },

        Capability::AutoUpdate if linux && facts.packaging != LinuxPackaging::AppImage => degraded(
            "Only AppImage builds can update themselves",
            "Installations from a deb or rpm package are updated by the system package manager. \
             Mira does not replace files it did not install.",
        ),

        // Everything not named above works as specified here. Listing them rather than
        // using a catch-all keeps the compiler honest when a capability is added.
        Capability::TrayIcon
        | Capability::TrayClickEvents
        | Capability::LaunchApplication
        | Capability::ProcessEnumeration
        | Capability::ProcessWorkingDirectory
        | Capability::ProcessTermination
        | Capability::PortEnumeration
        | Capability::PortAttribution
        | Capability::LockDetection
        | Capability::SleepDetection
        | Capability::MediaNowPlaying
        | Capability::Notifications
        | Capability::FileWatching
        | Capability::RevealInFileManager
        | Capability::DragOutFiles
        | Capability::AutoStart
        | Capability::BatteryInfo
        | Capability::AutoUpdate
        | Capability::Clipboard => CapabilityStatus::Full,
    }
}

/// Resolve every capability, in [`Capability::ALL`] order.
#[must_use]
pub fn resolve_all(facts: &EnvFacts) -> Vec<CapabilityReport> {
    Capability::ALL
        .into_iter()
        .map(|capability| CapabilityReport::new(capability, resolve(capability, facts)))
        .collect()
}

fn degraded(reason: &str, detail: &str) -> CapabilityStatus {
    CapabilityStatus::Degraded {
        reason: reason.to_owned(),
        detail: detail.to_owned(),
    }
}

fn unavailable(reason: &str, fallback: Option<&str>) -> CapabilityStatus {
    CapabilityStatus::Unavailable {
        reason: reason.to_owned(),
        fallback: fallback.map(ToOwned::to_owned),
    }
}
