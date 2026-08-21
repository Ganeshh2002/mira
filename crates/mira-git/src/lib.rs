//! Read-only Git context.
//!
//! Mira reads Git and never writes it. There is no commit, stage, checkout, push,
//! pull, merge, or rebase in the 0.x line, and the absence is structural: this
//! crate exposes no method that could perform one (`prd.md` FR-3.4).
//!
//! It also never opens a network transport. `git2` is built with its HTTPS and
//! SSH features off, so a fetch is not merely unused — it is not compiled in.
//!
//! Six questions are answered here: **where does this repository stand**
//! ([`GitProvider::overview`]), **what happened lately**
//! ([`GitProvider::history`]), **how do those commits relate**
//! ([`GitProvider::graph`]), **what changed**
//! ([`GitProvider::changed_files`], [`GitProvider::file_diff`]), and **what
//! happened to one file** ([`GitProvider::file_history`]), and **which commits
//! match** ([`GitProvider::filtered_history`]). Every one is bounded by a
//! constant in this crate — [`PAGE`] for history, the five limits in [`diff`] for
//! changes, [`MAX_SCAN`] for a file trace, [`MAX_FILTER_SCAN`] for a search — so
//! no caller can ask Mira to read an entire repository.
//!
//! The graph is a picture, not a client. Nothing here can check out, merge,
//! rebase, reset, cherry-pick, create a commit, stage a path, or reach a remote,
//! and a guard test fails the build if a libgit2 write API appears in these
//! sources.
//!
//! See [ADR-0009](../../../docs/adr/0009-git-via-libgit2.md) for why libgit2, and
//! `architecture.md` §4 for why it sits behind a trait.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod diff;
pub mod filter;
pub mod graph;
pub mod history;
pub mod lanes;
pub mod libgit2;
pub mod model;
pub mod patch;
pub mod provider;
pub mod trace;
pub(crate) mod walk;

pub use diff::{
    ChangeKind, ChangedFiles, Comparison, DiffLine, DiffScope, FileChange, FileDiff,
    FilesTruncated, Hunk, LineKind, PatchTruncated, MAX_BYTES, MAX_FILES, MAX_FILE_BYTES,
    MAX_LINES, MAX_LINE_BYTES,
};
pub use filter::{
    AuthorCount, FilterCursor, FilteredHistory, HistoryFilter, KnownAuthors, KnownRefs,
    MalformedTerm, RefTip, Term, LONGEST_TERM, MAX_AUTHORS, MAX_FILTER_SCAN,
};
pub use graph::{
    CommitGraph, Edge, EdgeKind, GitRef, GraphRow, RefKind, RowKind, MAX_LANES, MAX_REFS,
};
pub use history::{CommitDetail, CommitId, CommitLookup, CommitPage, MalformedCommitId, PAGE};
pub use lanes::{layout, Layout, Node, Placement};
pub use libgit2::Libgit2;
pub use model::{Commit, GitOverview, Head, Upstream};
pub use patch::MAX_STATS_BYTES;
pub use provider::GitProvider;
pub use trace::{FileCommit, FileCursor, FileHistory, FileSubject, ScanStopped, MAX_SCAN};
