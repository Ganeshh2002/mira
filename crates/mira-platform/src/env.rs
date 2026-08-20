//! What Mira observed about the machine it is running on.
//!
//! [`EnvFacts`] is a plain data struct so that capability resolution is a *pure
//! function over observations* — which is what makes it testable against synthetic
//! environments (`docs/architecture/platform-abstraction.md` §6) instead of only on
//! the developer's own laptop.
//!
//! Probing the real machine happens in [`EnvFacts::detect`]. That is the only place
//! in Mira where `cfg(target_os)` is allowed to decide anything.

use std::path::Path;

/// The operating system family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Os {
    /// macOS.
    MacOs,
    /// Windows.
    Windows,
    /// Any Linux distribution.
    Linux,
}

impl Os {
    /// A label for the shell and for Settings.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::MacOs => "macOS",
            Self::Windows => "Windows",
            Self::Linux => "Linux",
        }
    }
}

/// The windowing system in use.
///
/// This is the distinction a compile-time target cannot make: the same Linux binary
/// runs under X11 and under Wayland, and global shortcuts work on exactly one of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DisplayServer {
    /// macOS Quartz.
    Quartz,
    /// The Windows desktop.
    Dwm,
    /// X11, including XWayland sessions that identify as X11.
    X11,
    /// A Wayland compositor.
    Wayland,
    /// Linux, but the session type could not be determined.
    Unknown,
}

impl DisplayServer {
    /// A label for the shell and for Settings.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Quartz => "Quartz",
            Self::Dwm => "Desktop Window Manager",
            Self::X11 => "X11",
            Self::Wayland => "Wayland",
            Self::Unknown => "unknown session",
        }
    }
}

/// How this Linux build was installed, which decides whether it can update itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LinuxPackaging {
    /// Running from an AppImage.
    AppImage,
    /// Installed by a package manager, or not determinable.
    Managed,
    /// Not Linux.
    NotApplicable,
}

/// Observations about the current machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvFacts {
    /// Which OS family.
    pub os: Os,
    /// Which windowing system.
    pub display_server: DisplayServer,
    /// Whether systemd-logind is present, which decides how lock detection behaves.
    pub has_logind: bool,
    /// How this build was packaged.
    pub packaging: LinuxPackaging,
}

impl EnvFacts {
    /// Whether this filesystem tells `Aviora` and `aviora` apart.
    ///
    /// Linux does; macOS and Windows normally do not. This is the fact `mira-fs`
    /// needs to decide whether two spellings name one project, and it lives here
    /// because it is the sort of thing only the platform layer may know
    /// (ADR-0005).
    ///
    /// It is a statement about the common case, not about every mount: a
    /// case-sensitive APFS volume exists, and a project added on one is compared
    /// case-insensitively. The consequence is bounded — Mira may refuse to add a
    /// second project whose path differs only in case — and the alternative,
    /// probing every volume, costs more than the mistake.
    #[must_use]
    pub const fn paths_are_case_sensitive(&self) -> bool {
        matches!(self.os, Os::Linux)
    }

    /// Observe the real machine.
    ///
    /// Cheap: environment variables and one `stat`. No network, no subprocess, no
    /// D-Bus connection. It runs once at startup.
    #[must_use]
    pub fn detect() -> Self {
        Self::detect_with(
            |key| std::env::var(key).ok(),
            |path| Path::new(path).exists(),
        )
    }

    /// [`EnvFacts::detect`] with its two sources of truth injected, so the probing
    /// logic itself can be tested without setting process-wide environment
    /// variables (which race across parallel test threads).
    pub fn detect_with<E, P>(env: E, path_exists: P) -> Self
    where
        E: Fn(&str) -> Option<String>,
        P: Fn(&str) -> bool,
    {
        #[cfg(target_os = "macos")]
        let os = Os::MacOs;
        #[cfg(target_os = "windows")]
        let os = Os::Windows;
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let os = Os::Linux;

        let display_server = match os {
            Os::MacOs => DisplayServer::Quartz,
            Os::Windows => DisplayServer::Dwm,
            Os::Linux => detect_linux_display_server(&env),
        };

        let has_logind = matches!(os, Os::Linux) && path_exists("/run/systemd/seats");

        let packaging = match os {
            Os::Linux if env("APPIMAGE").is_some() => LinuxPackaging::AppImage,
            Os::Linux => LinuxPackaging::Managed,
            _ => LinuxPackaging::NotApplicable,
        };

        Self {
            os,
            display_server,
            has_logind,
            packaging,
        }
    }
}

/// Detect the Linux windowing system from session environment variables.
///
/// `XDG_SESSION_TYPE` is authoritative when present. `WAYLAND_DISPLAY` is the
/// fallback signal, exactly as `docs/architecture/platform-abstraction.md` §4.1
/// specifies — and it is checked *before* `DISPLAY`, because an XWayland session
/// sets both and global shortcuts still do not work there.
fn detect_linux_display_server<E>(env: &E) -> DisplayServer
where
    E: Fn(&str) -> Option<String>,
{
    let is_set = |key: &str| env(key).is_some_and(|value| !value.is_empty());

    match env("XDG_SESSION_TYPE")
        .unwrap_or_default()
        .to_lowercase()
        .as_str()
    {
        "wayland" => DisplayServer::Wayland,
        "x11" => DisplayServer::X11,
        _ if is_set("WAYLAND_DISPLAY") => DisplayServer::Wayland,
        _ if is_set("DISPLAY") => DisplayServer::X11,
        _ => DisplayServer::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn from(pairs: &[(&str, &str)]) -> DisplayServer {
        let owned: Vec<(String, String)> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        detect_linux_display_server(&|key: &str| {
            owned.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
        })
    }

    #[test]
    fn xdg_session_type_wins_when_it_is_set() {
        assert_eq!(
            from(&[("XDG_SESSION_TYPE", "wayland")]),
            DisplayServer::Wayland
        );
        assert_eq!(from(&[("XDG_SESSION_TYPE", "x11")]), DisplayServer::X11);
    }

    #[test]
    fn session_type_is_matched_case_insensitively() {
        assert_eq!(
            from(&[("XDG_SESSION_TYPE", "Wayland")]),
            DisplayServer::Wayland
        );
    }

    #[test]
    fn wayland_display_is_the_fallback_signal() {
        assert_eq!(
            from(&[("WAYLAND_DISPLAY", "wayland-0")]),
            DisplayServer::Wayland
        );
    }

    #[test]
    fn an_empty_wayland_display_is_not_a_wayland_session() {
        assert_eq!(
            from(&[("WAYLAND_DISPLAY", ""), ("DISPLAY", ":0")]),
            DisplayServer::X11
        );
    }

    #[test]
    fn display_alone_means_x11() {
        assert_eq!(from(&[("DISPLAY", ":0")]), DisplayServer::X11);
    }

    #[test]
    fn a_wayland_session_running_xwayland_still_reads_as_wayland() {
        assert_eq!(
            from(&[("XDG_SESSION_TYPE", "wayland"), ("DISPLAY", ":0")]),
            DisplayServer::Wayland,
            "XWayland sets DISPLAY, but global shortcuts still do not work"
        );
    }

    #[test]
    fn nothing_set_is_unknown_rather_than_a_guess() {
        assert_eq!(from(&[]), DisplayServer::Unknown);
    }
}
