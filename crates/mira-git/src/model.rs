//! What Mira says about a repository.
//!
//! These are the types the interface renders, so every one of them is a state a
//! person can be shown — including "this is not a repository" and "this repository
//! cannot be read", which are answers rather than failures (`prd.md` FR-3.6).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Everything Slice 1 reads about a project's Git state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
#[ts(export)]
pub enum GitOverview {
    /// The directory has no repository. A neutral state, not an error.
    NotARepository,

    /// There is a repository, but it could not be read.
    #[serde(rename_all = "camelCase")]
    Unreadable {
        /// What went wrong, in words a person can act on.
        detail: String,
    },

    /// The repository was read.
    #[serde(rename_all = "camelCase")]
    Ready {
        /// Which branch, or which commit, is checked out.
        head: Head,
        /// Whether the working tree has nothing to report.
        clean: bool,
        /// How many paths differ from HEAD, counting untracked files.
        changed: u32,
        /// HEAD's commit, absent only in a repository with no commits.
        last_commit: Option<Commit>,
        /// The tracked branch and the distance to it, when one is configured.
        upstream: Option<Upstream>,
    },
}

/// What `HEAD` points at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum Head {
    /// A named branch.
    #[serde(rename_all = "camelCase")]
    Branch {
        /// The short name, as `git branch` would print it.
        name: String,
    },

    /// A commit, checked out directly.
    #[serde(rename_all = "camelCase")]
    Detached {
        /// The abbreviated commit id.
        sha: String,
    },

    /// A branch that exists but has no commits yet.
    Unborn,
}

/// One commit, reduced to what the header shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Commit {
    /// The full commit id, for copying.
    pub sha: String,
    /// The abbreviated commit id, for showing.
    pub short_sha: String,
    /// The first line of the message.
    pub subject: String,
    /// The author's name as recorded in the commit.
    pub author: String,
    /// When it was authored, Unix epoch seconds UTC.
    #[ts(type = "number")]
    pub committed_at: i64,
}

/// The tracked branch, and how far HEAD is from it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Upstream {
    /// The upstream's short name, such as `origin/main`.
    pub name: String,
    /// Commits on HEAD that the upstream does not have.
    pub ahead: u32,
    /// Commits on the upstream that HEAD does not have.
    pub behind: u32,
}
