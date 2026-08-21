//! Putting a short piece of text on the system clipboard.
//!
//! Native on all three platforms — `NSPasteboard`, the Win32 clipboard, the X11
//! selection — through `arboard`. Nothing here runs a program: there is no
//! `pbcopy`, no `clip.exe`, no `xclip`, and no argv in this file at all
//! (`security-and-privacy.md` §5 rule 1).
//!
//! The narrower guarantee, and the reason this is a capability rather than a
//! helper: **the only thing Mira ever writes to the clipboard is a value Mira
//! itself read from a repository.** The interface asks to copy *a commit*, and the
//! commit id is resolved in Rust before anything is written, so there is no path
//! by which a page could use Mira to place text of its own choosing on the
//! clipboard of the person running it (`commands::git::git_copy_commit`).

use mira_core::{Capability, CapabilityStatus, MiraError, Result};

use crate::platform::PlatformCapabilities;

/// The longest value Mira will ever put on the clipboard.
///
/// A full commit id is 40 characters. The cap is here so that "Mira copies short
/// facts" is enforced rather than merely true today — a future caller cannot turn
/// this into a way to move a file's contents through the clipboard.
pub const LONGEST: usize = 128;

/// What Mira may ask the clipboard to hold.
pub trait ClipboardHost {
    /// Put `value` on the clipboard.
    ///
    /// # Errors
    ///
    /// [`MiraError::Unsupported`] where the capability is off, [`MiraError::Invalid`]
    /// for anything longer than [`LONGEST`] or containing a control character, and
    /// [`MiraError::External`] if the desktop refused.
    fn copy(&self, value: &str) -> Result<()>;
}

/// This machine's clipboard.
#[derive(Debug, Clone, Copy)]
pub struct Clipboard<P> {
    platform: P,
}

impl<P: PlatformCapabilities> Clipboard<P> {
    /// Bind the clipboard to this machine's capabilities.
    pub const fn new(platform: P) -> Self {
        Self { platform }
    }
}

/// Whether `value` is the shape of thing Mira copies.
///
/// Short, and one line. A commit id passes; anything carrying a newline or a
/// control character does not, because a clipboard payload that can contain a
/// newline is a clipboard payload that can be pasted into a terminal as two
/// commands.
#[must_use]
pub fn is_copyable(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= LONGEST
        && !value.chars().any(char::is_control)
        && value.trim() == value
}

impl<P: PlatformCapabilities> ClipboardHost for Clipboard<P> {
    fn copy(&self, value: &str) -> Result<()> {
        let status = self.platform.status(Capability::Clipboard);
        if !status.is_usable() {
            return Err(MiraError::Unsupported {
                capability: Capability::Clipboard,
                reason: match status {
                    CapabilityStatus::Unavailable { reason, .. } => reason,
                    _ => "This machine has no clipboard Mira can write to.".to_owned(),
                },
            });
        }

        if !is_copyable(value) {
            return Err(MiraError::invalid(
                "value",
                "Mira copies short, single-line facts such as a commit id.",
            ));
        }

        arboard::Clipboard::new()
            .and_then(|mut clipboard| clipboard.set_text(value.to_owned()))
            .map_err(|error| MiraError::External {
                source: "the clipboard".to_owned(),
                detail: format!("Nothing was copied: {error}"),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{is_copyable, LONGEST};

    #[test]
    fn a_commit_id_is_copyable() {
        assert!(is_copyable("ad50fc7"));
        assert!(is_copyable("ad50fc71f70b122469a9772cb772dc86b2c83521"));
    }

    #[test]
    fn nothing_with_a_newline_in_it_is_copyable() {
        // A clipboard value that can carry a newline is one that can be pasted
        // into a terminal as two commands.
        assert!(!is_copyable("ad50fc7\nrm -rf ~"));
        assert!(!is_copyable("ad50fc7\r\nwhoami"));
        assert!(!is_copyable("ad50fc7\u{0}"));
    }

    #[test]
    fn nothing_long_is_copyable() {
        assert!(!is_copyable(&"a".repeat(LONGEST + 1)));
    }

    #[test]
    fn nothing_empty_or_padded_is_copyable() {
        assert!(!is_copyable(""));
        assert!(!is_copyable(" ad50fc7"));
        assert!(!is_copyable("ad50fc7 "));
    }
}
