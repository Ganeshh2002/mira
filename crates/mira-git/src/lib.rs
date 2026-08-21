//! Read-only Git context.
//!
//! Mira reads Git and never writes it. There is no commit, stage, checkout, push,
//! pull, merge, or rebase in the 0.x line, and the absence is structural: this
//! crate exposes no method that could perform one (`prd.md` FR-3.4).
//!
//! It also never opens a network transport. `git2` is built with its HTTPS and
//! SSH features off, so a fetch is not merely unused — it is not compiled in.
//!
//! Three questions are answered here: **where does this repository stand**
//! ([`GitProvider::overview`]), **what happened lately**
//! ([`GitProvider::history`]), and **how do those commits relate**
//! ([`GitProvider::graph`]). All three are paged, and the page size is this
//! crate's ([`PAGE`]), so no caller can ask Mira to walk an entire repository.
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

pub mod graph;
pub mod history;
pub mod lanes;
pub mod libgit2;
pub mod model;
pub mod provider;
mod walk;

pub use graph::{
    CommitGraph, Edge, EdgeKind, GitRef, GraphRow, RefKind, RowKind, MAX_LANES, MAX_REFS,
};
pub use history::{CommitDetail, CommitId, CommitLookup, CommitPage, MalformedCommitId, PAGE};
pub use lanes::{layout, Layout, Node, Placement};
pub use libgit2::Libgit2;
pub use model::{Commit, GitOverview, Head, Upstream};
pub use provider::GitProvider;
