//! Path safety.
//!
//! Mira's whole security posture rests on two questions this crate answers:
//! *which directory is this, really* and *is this file inside a root the user
//! registered*. Both are pure functions over paths, so they are testable on any
//! machine and contain no `cfg(target_os)` — the one OS-dependent fact,
//! whether the filesystem cares about case, arrives as a [`PathMatching`] the
//! caller obtained from `mira-platform` (ADR-0005).
//!
//! See `docs/architecture/security-and-privacy.md` §5.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::path::{Component, Path, PathBuf};

use mira_core::{MiraError, Result};

/// Whether this filesystem distinguishes `Aviora` from `aviora`.
///
/// Linux says yes; macOS and Windows normally say no. Mira never guesses — the
/// platform layer decides and passes the answer down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathMatching {
    /// Linux: two spellings are two directories.
    CaseSensitive,
    /// macOS and Windows: two spellings are one directory.
    CaseInsensitive,
}

impl PathMatching {
    /// The rule for a filesystem `mira-platform` described.
    #[must_use]
    pub const fn from_case_sensitivity(case_sensitive: bool) -> Self {
        if case_sensitive {
            Self::CaseSensitive
        } else {
            Self::CaseInsensitive
        }
    }

    /// Whether two paths name the same location under this rule.
    #[must_use]
    pub fn same_path(self, a: &Path, b: &Path) -> bool {
        let a: Vec<_> = a.components().collect();
        let b: Vec<_> = b.components().collect();
        a.len() == b.len() && self.components_match(&a, &b)
    }

    fn components_match(self, prefix: &[Component<'_>], candidate: &[Component<'_>]) -> bool {
        prefix
            .iter()
            .zip(candidate)
            .all(|(want, got)| self.component_eq(*want, *got))
    }

    fn component_eq(self, a: Component<'_>, b: Component<'_>) -> bool {
        match self {
            Self::CaseSensitive => a == b,
            Self::CaseInsensitive => {
                let (a, b) = (a.as_os_str(), b.as_os_str());
                a.eq_ignore_ascii_case(b)
                    || a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
            }
        }
    }
}

/// Resolve a user-chosen directory to one canonical absolute path.
///
/// This is what gives a project its identity: `~/code/aviora`, `./aviora`, and
/// `aviora/../aviora` are the same project, and the database's `root_path UNIQUE`
/// only means "one project per directory" because every path passes through here
/// first.
///
/// # Errors
///
/// [`MiraError::Invalid`] if the path is empty or is not a directory,
/// [`MiraError::NotFound`] if nothing is there, and
/// [`MiraError::PermissionDenied`] if the operating system refuses to look.
pub fn canonical_dir(path: &Path) -> Result<PathBuf> {
    if path.as_os_str().is_empty() {
        return Err(MiraError::invalid("path", "No directory was chosen."));
    }

    let resolved = std::fs::canonicalize(path).map_err(|error| describe(path, &error))?;

    let metadata = std::fs::metadata(&resolved).map_err(|error| describe(path, &error))?;
    if !metadata.is_dir() {
        return Err(MiraError::invalid(
            "path",
            format!("{} is a file. A project is a directory.", display(path)),
        ));
    }

    Ok(resolved)
}

/// Whether `candidate` is `root` or lives beneath it.
///
/// Compares whole path components rather than string prefixes, because
/// `/home/dev/aviora-secrets` starts with `/home/dev/aviora` as text and is a
/// different directory in every way that matters.
#[must_use]
pub fn contains(root: &Path, candidate: &Path, matching: PathMatching) -> bool {
    let root: Vec<_> = root.components().collect();
    let candidate: Vec<_> = candidate.components().collect();

    candidate.len() >= root.len() && matching.components_match(&root, &candidate)
}

fn describe(path: &Path, error: &std::io::Error) -> MiraError {
    match error.kind() {
        std::io::ErrorKind::NotFound => MiraError::NotFound {
            what: display(path),
        },
        std::io::ErrorKind::PermissionDenied => MiraError::PermissionDenied {
            what: display(path),
            hint: "Grant Mira access to this folder, or choose another one.".to_owned(),
        },
        _ => MiraError::external("the filesystem", error),
    }
}

fn display(path: &Path) -> String {
    path.display().to_string()
}
