//! A project — a directory on disk plus what Mira learned about it.
//!
//! The top of Mira's spine (`information-architecture.md` §1). Everything else in
//! the product attaches to one of these.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ids::ProjectId;

/// One project, as stored and as shown.
///
/// Deliberately absent: whether the directory still exists. That is a runtime
/// state Mira observes on load, not a column that can go stale
/// (`data-model.md` §3.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Project {
    /// Stable identity, independent of the directory's name or location.
    pub id: ProjectId,
    /// What the user calls it. Inferred from the directory on add.
    pub name: String,
    /// The canonical absolute path of the directory.
    pub root_path: String,
    /// Whether a Git worktree was found at or above the root.
    pub is_git: bool,
    /// The worktree root, when it differs from `root_path`.
    pub git_root: Option<String>,
    /// Detected project types: `node`, `rust`, `go`, and so on.
    pub markers: Vec<String>,
    /// When the project was last opened, Unix epoch seconds UTC.
    #[ts(type = "number | null")]
    pub last_opened_at: Option<i64>,
    /// When it was added.
    #[ts(type = "number")]
    pub created_at: i64,
    /// When any of its stored fields last changed.
    #[ts(type = "number")]
    pub updated_at: i64,
}
