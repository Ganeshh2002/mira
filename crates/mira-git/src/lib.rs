//! Read-only Git context.
//!
//! Mira reads Git and never writes it. There is no commit, stage, checkout, push,
//! pull, merge, or rebase in the 0.x line, and the absence is structural: this
//! crate exposes no method that could perform one (`prd.md` FR-3.4).
//!
//! See [ADR-0009](../../../docs/adr/0009-git-via-libgit2.md) for why libgit2, and
//! `architecture.md` §4 for why it sits behind a trait.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod libgit2;
pub mod model;
pub mod provider;

pub use libgit2::Libgit2;
pub use model::{Commit, GitOverview, Head, Upstream};
pub use provider::GitProvider;
