//! Capability resolution against synthetic environments.
//!
//! `docs/architecture/platform-abstraction.md` §6 requires exactly this: resolution
//! unit-tested against machines we are not running on — Wayland, logind-less Linux,
//! macOS media. Every assertion below traces to a cell of the §5 capability matrix,
//! which is the checklist these tests enforce.

use mira_core::{Capability, CapabilityStatus};
use mira_platform::{resolve, DisplayServer, EnvFacts, LinuxPackaging, Os};

fn macos() -> EnvFacts {
    EnvFacts {
        os: Os::MacOs,
        display_server: DisplayServer::Quartz,
        has_logind: false,
        packaging: LinuxPackaging::NotApplicable,
    }
}

fn windows() -> EnvFacts {
    EnvFacts {
        os: Os::Windows,
        display_server: DisplayServer::Dwm,
        has_logind: false,
        packaging: LinuxPackaging::NotApplicable,
    }
}

fn linux_x11() -> EnvFacts {
    EnvFacts {
        os: Os::Linux,
        display_server: DisplayServer::X11,
        has_logind: true,
        packaging: LinuxPackaging::Managed,
    }
}

fn linux_wayland() -> EnvFacts {
    EnvFacts {
        display_server: DisplayServer::Wayland,
        ..linux_x11()
    }
}

#[test]
fn global_shortcut_is_unavailable_on_wayland_and_names_the_fallback() {
    let status = resolve(Capability::GlobalShortcut, &linux_wayland());

    match status {
        CapabilityStatus::Unavailable { fallback, .. } => {
            assert_eq!(
                fallback.as_deref(),
                Some("mira --toggle"),
                "Wayland users must be given the documented fallback, not left stuck"
            );
        }
        other => panic!("expected Unavailable on Wayland, got {other:?}"),
    }
}

#[test]
fn global_shortcut_is_full_everywhere_else() {
    for facts in [macos(), windows(), linux_x11()] {
        assert_eq!(
            resolve(Capability::GlobalShortcut, &facts),
            CapabilityStatus::Full,
            "global shortcut should be Full on {:?}/{:?}",
            facts.os,
            facts.display_server
        );
    }
}

#[test]
fn now_playing_is_unavailable_on_macos_with_no_fallback() {
    match resolve(Capability::MediaNowPlaying, &macos()) {
        CapabilityStatus::Unavailable { reason, fallback } => {
            assert!(
                reason.to_lowercase().contains("entitled"),
                "the reason must state Apple's actual restriction, got: {reason}"
            );
            assert_eq!(
                fallback, None,
                "there is no fallback, and inventing one would be a lie"
            );
        }
        other => panic!("expected Unavailable on macOS, got {other:?}"),
    }
}

#[test]
fn now_playing_is_full_on_windows_and_linux() {
    for facts in [windows(), linux_x11(), linux_wayland()] {
        assert_eq!(
            resolve(Capability::MediaNowPlaying, &facts),
            CapabilityStatus::Full,
            "MPRIS and GSMTC are public APIs; {:?} should be Full",
            facts.os
        );
    }
}

#[test]
fn tray_click_events_never_fire_on_linux() {
    for facts in [linux_x11(), linux_wayland()] {
        assert!(
            matches!(
                resolve(Capability::TrayClickEvents, &facts),
                CapabilityStatus::Unavailable { .. }
            ),
            "libappindicator delivers no click events"
        );
    }
    assert_eq!(
        resolve(Capability::TrayClickEvents, &macos()),
        CapabilityStatus::Full
    );
    assert_eq!(
        resolve(Capability::TrayClickEvents, &windows()),
        CapabilityStatus::Full
    );
}

#[test]
fn tray_icon_is_degraded_but_usable_on_linux() {
    let status = resolve(Capability::TrayIcon, &linux_x11());
    assert_eq!(status.state(), "degraded");
    assert!(status.is_usable(), "a menu-only tray still works");
}

#[test]
fn lock_detection_is_degraded_on_linux_whether_or_not_logind_is_present() {
    let with_logind = resolve(Capability::LockDetection, &linux_x11());
    let without_logind = resolve(
        Capability::LockDetection,
        &EnvFacts {
            has_logind: false,
            ..linux_x11()
        },
    );

    assert_eq!(with_logind.state(), "degraded");
    assert_eq!(without_logind.state(), "degraded");
    assert_ne!(
        with_logind, without_logind,
        "a machine without logind deserves a different explanation, not the same one"
    );
}

#[test]
fn lock_detection_is_full_on_macos_and_windows() {
    assert_eq!(
        resolve(Capability::LockDetection, &macos()),
        CapabilityStatus::Full
    );
    assert_eq!(
        resolve(Capability::LockDetection, &windows()),
        CapabilityStatus::Full
    );
}

#[test]
fn windows_reports_its_real_process_and_port_limitations() {
    assert_eq!(
        resolve(Capability::ProcessWorkingDirectory, &windows()).state(),
        "degraded"
    );
    assert_eq!(
        resolve(Capability::ProcessTermination, &windows()).state(),
        "degraded",
        "Windows has no true graceful stop for console apps and the UI must say so"
    );
    assert_eq!(
        resolve(Capability::PortAttribution, &windows()).state(),
        "degraded",
        "attribution falls back to executable path without a working directory"
    );
}

#[test]
fn port_enumeration_is_full_everywhere() {
    for facts in [macos(), windows(), linux_x11(), linux_wayland()] {
        assert_eq!(
            resolve(Capability::PortEnumeration, &facts),
            CapabilityStatus::Full
        );
    }
}

#[test]
fn reveal_in_file_manager_is_degraded_on_linux_because_it_cannot_select() {
    let status = resolve(Capability::RevealInFileManager, &linux_x11());
    assert_eq!(status.state(), "degraded");
    let CapabilityStatus::Degraded { reason, .. } = status else {
        unreachable!()
    };
    assert!(
        reason.to_lowercase().contains("folder"),
        "the copy must say it opens the folder, not that it selects the file: {reason}"
    );
}

#[test]
fn auto_update_is_full_only_for_appimage_on_linux() {
    let appimage = EnvFacts {
        packaging: LinuxPackaging::AppImage,
        ..linux_x11()
    };
    assert_eq!(
        resolve(Capability::AutoUpdate, &appimage),
        CapabilityStatus::Full
    );
    assert_eq!(
        resolve(Capability::AutoUpdate, &linux_x11()).state(),
        "degraded",
        "deb and rpm installs update through the package manager"
    );
}

#[test]
fn every_capability_resolves_on_every_environment() {
    for facts in [macos(), windows(), linux_x11(), linux_wayland()] {
        for capability in Capability::ALL {
            let status = resolve(capability, &facts);
            match &status {
                CapabilityStatus::Full => {}
                CapabilityStatus::Degraded { reason, detail } => {
                    assert!(
                        !reason.trim().is_empty(),
                        "{capability:?} has an empty reason"
                    );
                    assert!(
                        !detail.trim().is_empty(),
                        "{capability:?} has an empty detail"
                    );
                }
                CapabilityStatus::Unavailable { reason, .. } => {
                    assert!(
                        !reason.trim().is_empty(),
                        "{capability:?} has an empty reason"
                    );
                }
            }
        }
    }
}

#[test]
fn user_facing_strings_are_sentences_not_log_lines() {
    for facts in [macos(), windows(), linux_x11(), linux_wayland()] {
        for capability in Capability::ALL {
            let text = match resolve(capability, &facts) {
                CapabilityStatus::Full => continue,
                CapabilityStatus::Degraded { reason, .. }
                | CapabilityStatus::Unavailable { reason, .. } => reason,
            };
            let first = text.chars().next().expect("non-empty");
            assert!(
                first.is_uppercase(),
                "{capability:?} reason should read as prose: {text}"
            );
            assert!(
                !text.contains('_') && !text.contains("::"),
                "{capability:?} reason leaks an identifier: {text}"
            );
        }
    }
}
