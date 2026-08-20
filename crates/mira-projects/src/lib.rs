//! Projects — the top of Mira's spine.
//!
//! **Slice 0 establishes this boundary and nothing more.** The crate exists so the
//! dependency direction (`src-tauri` → domain → `mira-db` → `mira-core`) is real and
//! enforced from the first commit rather than retrofitted. Directory probing, type
//! markers, add, rename, remove and reordering are Slice 1 and are deliberately
//! absent — building them now would be implementing a feature this slice excludes.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use mira_core::Result;
use mira_db::ProjectRepo;

/// What the application shell may ask about projects.
pub trait ProjectService {
    /// How many projects the user has added.
    fn count(&self) -> Result<u32>;
}

/// The repository-backed implementation.
#[derive(Debug, Clone, Copy)]
pub struct Projects<R> {
    repo: R,
}

impl<R: ProjectRepo> Projects<R> {
    /// Wrap a repository.
    pub const fn new(repo: R) -> Self {
        Self { repo }
    }
}

impl<R: ProjectRepo> ProjectService for Projects<R> {
    fn count(&self) -> Result<u32> {
        self.repo.count()
    }
}
