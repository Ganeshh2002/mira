//! Handing a path to the desktop.
//!
//! One action in Slice 1: open a project's directory where the user's file manager
//! would open it. Three programs, one argument each, no shell — the path is passed
//! as an argv element, so a directory whose name contains a semicolon is a
//! directory with a semicolon in its name and nothing else
//! (`security-and-privacy.md` §5 rule 1).

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use mira_core::{Capability, MiraError, Result};

use crate::env::Os;
use crate::platform::PlatformCapabilities;

/// What Mira may ask the desktop to do.
pub trait ShellHost {
    /// Open a directory in the platform's file manager.
    ///
    /// # Errors
    ///
    /// [`MiraError::Unsupported`] where the capability is off, and
    /// [`MiraError::External`] if the program could not be started.
    fn reveal(&self, path: &Path) -> Result<()>;
}

/// The program and arguments that open `path`, per operating system.
///
/// Returned as data so every platform's answer can be asserted from any one of
/// them. The programs are literals; nothing about them is derived from input.
///
/// Linux gets `xdg-open`, which opens the containing folder without selecting
/// anything — the reason `RevealInFileManager` resolves to Degraded there
/// (`platform-abstraction.md` §4.9).
#[must_use]
pub fn reveal_command(os: Os, path: &Path) -> (&'static str, Vec<OsString>) {
    let program = match os {
        Os::MacOs => "open",
        Os::Windows => "explorer",
        Os::Linux => "xdg-open",
    };

    (program, vec![OsString::from(path)])
}

/// The real desktop.
#[derive(Debug, Clone)]
pub struct Shell<P> {
    os: Os,
    platform: P,
}

impl<P: PlatformCapabilities> Shell<P> {
    /// Bind the shell to this machine's capabilities.
    pub const fn new(os: Os, platform: P) -> Self {
        Self { os, platform }
    }
}

impl<P: PlatformCapabilities> ShellHost for Shell<P> {
    fn reveal(&self, path: &Path) -> Result<()> {
        let status = self.platform.status(Capability::RevealInFileManager);
        if !status.is_usable() {
            return Err(MiraError::Unsupported {
                capability: Capability::RevealInFileManager,
                reason: reason(&status),
            });
        }

        let (program, args) = reveal_command(self.os, path);

        // Spawned and left alone: the file manager outlives this call, and
        // `explorer` reports a non-zero exit code even when it succeeds, so waiting
        // on the status would invent failures that did not happen.
        Command::new(program)
            .args(&args)
            .spawn()
            .map(|_| ())
            .map_err(|error| MiraError::External {
                source: "the file manager".to_owned(),
                detail: format!(
                    "{} could not be opened: {error}",
                    PathBuf::from(path).display()
                ),
            })
    }
}

fn reason(status: &mira_core::CapabilityStatus) -> String {
    match status {
        mira_core::CapabilityStatus::Unavailable { reason, .. } => reason.clone(),
        _ => "Opening a folder is not available on this machine".to_owned(),
    }
}
