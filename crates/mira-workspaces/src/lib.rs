//! Workspaces — a named way of working on one project.
//!
//! **Slice 0 establishes this boundary and nothing more.** Workspaces are a 0.2
//! feature (`docs/product/product-scope.md` §3); the crate exists now only so the
//! schema created by `0001_init.sql` has an owner and the dependency direction is
//! enforced from the start. Creating, renaming, duplicating, switching, and
//! per-workspace configuration are all absent on purpose.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use mira_core::Result;
use mira_db::WorkspaceRepo;

/// What the application shell may ask about workspaces.
pub trait WorkspaceService {
    /// How many workspaces exist across all projects.
    fn count(&self) -> Result<u32>;
}

/// The repository-backed implementation.
#[derive(Debug, Clone, Copy)]
pub struct Workspaces<R> {
    repo: R,
}

impl<R: WorkspaceRepo> Workspaces<R> {
    /// Wrap a repository.
    pub const fn new(repo: R) -> Self {
        Self { repo }
    }
}

impl<R: WorkspaceRepo> WorkspaceService for Workspaces<R> {
    fn count(&self) -> Result<u32> {
        self.repo.count()
    }
}
