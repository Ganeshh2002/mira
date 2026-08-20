//! Starting applications the way macOS starts applications.
//!
//! `NSWorkspace`, not `open(1)`. The difference is not cosmetic:
//!
//! - **No command line exists.** `open -a "Some App" <path>` composes a program
//!   name and two arguments; this passes an `NSURL` for the bundle and an
//!   `NSURL` for the target through a typed API. There is no string a quoting
//!   mistake could turn into something else.
//! - **No child process.** A launched application belongs to the window server,
//!   not to Mira, so quitting Mira leaves your editor open — and Mira never
//!   holds a handle it could use to stop one.
//! - **It is what the platform does.** Activation, reuse of an already-running
//!   instance, and Launch Services' own permission checks all apply, because
//!   this is the same call the Finder makes.
//!
//! Every binding used here is a *safe* function in `objc2-app-kit`, so this
//! module contains no `unsafe` and the crate keeps `forbid(unsafe_code)`
//! (ADR-0013).

#[cfg(not(target_os = "macos"))]
use mira_core::Capability;
use mira_core::{MiraError, Result};

use crate::launch::LaunchTarget;

/// Open `target` with the application bundle at `bundle`.
///
/// # Errors
///
/// [`MiraError::NotFound`] if the bundle is not there, [`MiraError::Invalid`] if
/// the address cannot be parsed, and [`MiraError::Unsupported`] off macOS.
#[cfg(target_os = "macos")]
pub fn open_with_application(bundle: &str, target: &LaunchTarget) -> Result<()> {
    use objc2_app_kit::{NSWorkspace, NSWorkspaceOpenConfiguration};
    use objc2_foundation::{NSArray, NSString, NSURL};

    let application = NSURL::fileURLWithPath(&NSString::from_str(bundle));
    let opened = NSArray::from_retained_slice(&[url_for(target)?]);

    NSWorkspace::sharedWorkspace().openURLs_withApplicationAtURL_configuration_completionHandler(
        &opened,
        &application,
        &NSWorkspaceOpenConfiguration::configuration(),
        // The failure this reports is the application declining to start, which
        // arrives after this call has returned and after the user has already
        // been told the launch was requested. Watching for it would mean holding
        // a callback alive to show a message about a window that did not appear,
        // which the window not appearing says more clearly.
        None,
    );

    Ok(())
}

/// Open a web address with whatever the person set as their browser.
///
/// # Errors
///
/// [`MiraError::Invalid`] if the address cannot be parsed, and
/// [`MiraError::External`] if the desktop declined to open it.
#[cfg(target_os = "macos")]
pub fn open_with_default(url: &str) -> Result<()> {
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::{NSString, NSURL};

    let address = NSURL::URLWithString(&NSString::from_str(url)).ok_or_else(|| unparseable(url))?;

    if NSWorkspace::sharedWorkspace().openURL(&address) {
        return Ok(());
    }

    Err(MiraError::External {
        source: "the browser".to_owned(),
        detail: format!("{url} could not be opened."),
    })
}

/// A target as a URL the window server understands.
#[cfg(target_os = "macos")]
fn url_for(target: &LaunchTarget) -> Result<objc2::rc::Retained<objc2_foundation::NSURL>> {
    use objc2_foundation::{NSString, NSURL};

    match target {
        LaunchTarget::Directory(path) => Ok(NSURL::fileURLWithPath(&NSString::from_str(
            &path.to_string_lossy(),
        ))),
        LaunchTarget::WebAddress(url) => {
            NSURL::URLWithString(&NSString::from_str(url)).ok_or_else(|| unparseable(url))
        }
    }
}

#[cfg(target_os = "macos")]
fn unparseable(url: &str) -> MiraError {
    MiraError::invalid("target", format!("{url} is not an address."))
}

// ── Everywhere else ──────────────────────────────────────────────────────────
//
// Bundles are a macOS idea, so these are unreachable rather than unimplemented:
// `plan` only produces `Bundle` from the macOS candidate table, and only macOS
// routes a default handler through here. They exist so the code is total on
// every platform without a `cfg` anywhere above this crate.

/// Not a mechanism this platform has.
///
/// # Errors
///
/// Always [`MiraError::Unsupported`].
#[cfg(not(target_os = "macos"))]
pub fn open_with_application(_bundle: &str, _target: &LaunchTarget) -> Result<()> {
    Err(elsewhere())
}

/// Not a mechanism this platform has.
///
/// # Errors
///
/// Always [`MiraError::Unsupported`].
#[cfg(not(target_os = "macos"))]
pub fn open_with_default(_url: &str) -> Result<()> {
    Err(elsewhere())
}

#[cfg(not(target_os = "macos"))]
fn elsewhere() -> MiraError {
    MiraError::Unsupported {
        capability: Capability::LaunchApplication,
        reason: "Application bundles are a macOS mechanism.".to_owned(),
    }
}
