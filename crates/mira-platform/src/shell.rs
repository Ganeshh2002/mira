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

/// The only schemes Mira will hand to the desktop.
///
/// `security-and-privacy.md` §5 rule 5. `file://` would open a local path,
/// `javascript:` and `data:` execute, and a custom scheme hands control to
/// whichever application registered it. Mira opens web addresses and nothing
/// else, and in practice opens exactly one shape of them: the `http://localhost`
/// it built itself from an observed port.
const OPENABLE: [&str; 2] = ["http://", "https://"];

/// Whether `url` is a web address Mira is willing to open.
///
/// Deliberately a prefix check on the raw string rather than a parse: the
/// question is not "is this a valid URL" but "will the desktop be handed
/// something other than a web address", and a leading space or a scheme Mira
/// does not know both answer that with no.
#[must_use]
pub fn is_openable(url: &str) -> bool {
    OPENABLE.iter().any(|scheme| url.starts_with(scheme))
}

/// What Mira may ask the desktop to do.
pub trait ShellHost {
    /// Open a directory in the platform's file manager.
    ///
    /// # Errors
    ///
    /// [`MiraError::Unsupported`] where the capability is off, and
    /// [`MiraError::External`] if the program could not be started.
    fn reveal(&self, path: &Path) -> Result<()>;

    /// Open a web address in the user's browser.
    ///
    /// # Errors
    ///
    /// [`MiraError::Invalid`] for anything that is not `http` or `https`, and
    /// [`MiraError::External`] if the program could not be started.
    fn open_url(&self, url: &str) -> Result<()>;
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

/// The program and arguments that open `url`, per operating system.
///
/// The same three openers as [`reveal_command`]: on every platform Mira supports,
/// handing a web address to the desktop opener is what opens the browser the
/// person actually chose.
#[must_use]
pub fn open_url_command(os: Os, url: &str) -> (&'static str, Vec<OsString>) {
    let program = match os {
        Os::MacOs => "open",
        Os::Windows => "explorer",
        Os::Linux => "xdg-open",
    };

    (program, vec![OsString::from(url)])
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
        spawn(
            program,
            &args,
            "the file manager",
            &PathBuf::from(path).display().to_string(),
        )
    }

    fn open_url(&self, url: &str) -> Result<()> {
        if !is_openable(url) {
            return Err(MiraError::invalid(
                "url",
                format!("Mira only opens web addresses, and {url} is not one."),
            ));
        }

        let (program, args) = open_url_command(self.os, url);
        spawn(program, &args, "the browser", url)
    }
}

/// Start a program and leave it alone.
///
/// Not waited on: the browser or file manager outlives this call, and `explorer`
/// reports a non-zero exit code even when it succeeds, so checking the status
/// would invent failures that did not happen.
fn spawn(program: &str, args: &[OsString], what: &str, subject: &str) -> Result<()> {
    Command::new(program)
        .args(args)
        .spawn()
        .map(|_| ())
        .map_err(|error| MiraError::External {
            source: what.to_owned(),
            detail: format!("{subject} could not be opened: {error}"),
        })
}

fn reason(status: &mira_core::CapabilityStatus) -> String {
    match status {
        mira_core::CapabilityStatus::Unavailable { reason, .. } => reason.clone(),
        _ => "Opening a folder is not available on this machine".to_owned(),
    }
}
