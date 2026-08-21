//! The provider boundary.

use std::path::{Path, PathBuf};

use crate::diff::{ChangedFiles, DiffScope, FileDiff};
use crate::graph::CommitGraph;
use crate::history::{CommitId, CommitLookup, CommitPage};
use crate::model::GitOverview;

/// Read-only Git, as the rest of Mira sees it.
///
/// A trait rather than a concrete type because the implementation is expected to
/// change: `gitoxide` is the long-term destination once its status and graph APIs
/// settle, and swapping it must be a change inside this crate and nowhere else
/// (`architecture.md` §4, ADR-0009).
///
/// No method returns `Result`. "Not a repository" and "cannot be read" are states
/// the interface renders, not failures the caller has to translate.
///
/// Every method reads. There is no `commit`, `stage`, `checkout`, `push`, `pull`,
/// `merge`, `rebase` or `reset` here, and there will not be one in the 0.x line —
/// a trait with no way to express a write is how that promise is kept
/// (`prd.md` FR-3.4).
pub trait GitProvider {
    /// The worktree root containing `start`, found by walking up.
    fn discover(&self, start: &Path) -> Option<PathBuf>;

    /// Everything Mira shows about the repository at `root`.
    fn overview(&self, root: &Path) -> GitOverview;

    /// One page of history, continuing from `from` or starting at `HEAD`.
    ///
    /// Bounded: the page size belongs to the implementation
    /// ([`crate::history::PAGE`]), so no caller can ask for a whole repository.
    fn history(&self, root: &Path, from: Option<&CommitId>) -> CommitPage;

    /// One commit, in the detail its own view shows.
    fn commit(&self, root: &Path, id: &CommitId) -> CommitLookup;

    /// The same page as [`GitProvider::history`], with the shape of it.
    ///
    /// Parent ids, lanes and reference labels, computed over that page and
    /// nothing else. There is no second traversal here and no second history:
    /// the graph *is* the history, with the relationships kept
    /// ([ADR-0015](../../../docs/adr/0015-graph-lanes.md)).
    fn graph(&self, root: &Path, from: Option<&CommitId>) -> CommitGraph;

    /// What one commit changed, or what the working tree has that `HEAD` does not.
    ///
    /// Bounded by `diff::MAX_FILES`, and says when the bound bit. A merge is
    /// compared against its **first** parent and the answer records that
    /// ([ADR-0016](../../../docs/adr/0016-bounded-diffs.md)).
    fn changed_files(&self, root: &Path, scope: &DiffScope) -> ChangedFiles;

    /// One file's patch, by its **position** in the change list.
    ///
    /// An ordinal rather than a path: the caller can only name a file Mira
    /// already decided to offer. Bounded by the five limits in `diff`, each of
    /// which reports itself rather than truncating silently.
    fn file_diff(&self, root: &Path, scope: &DiffScope, at: u32) -> FileDiff;
}
