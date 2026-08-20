//! The provider boundary.

use std::path::{Path, PathBuf};

use crate::model::GitOverview;

/// Read-only Git, as the rest of Mira sees it.
///
/// A trait rather than a concrete type because the implementation is expected to
/// change: `gitoxide` is the long-term destination once its status and graph APIs
/// settle, and swapping it must be a change inside this crate and nowhere else
/// (`architecture.md` §4, ADR-0009).
///
/// Neither method returns `Result`. "Not a repository" and "cannot be read" are
/// states the interface renders, not failures the caller has to translate.
pub trait GitProvider {
    /// The worktree root containing `start`, found by walking up.
    fn discover(&self, start: &Path) -> Option<PathBuf>;

    /// Everything Mira shows about the repository at `root`.
    fn overview(&self, root: &Path) -> GitOverview;
}
