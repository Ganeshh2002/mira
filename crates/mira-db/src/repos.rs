//! Repository traits.

use mira_core::Result;

use crate::db::Db;

/// Reads and writes of projects.
pub trait ProjectRepo {
    /// How many projects exist.
    fn count(&self) -> Result<u32>;
}

/// Reads and writes of workspaces.
pub trait WorkspaceRepo {
    /// How many workspaces exist.
    fn count(&self) -> Result<u32>;
}

// So a service can hold `&Db` without owning it.
impl<T: ProjectRepo + ?Sized> ProjectRepo for &T {
    fn count(&self) -> Result<u32> {
        (**self).count()
    }
}

impl<T: WorkspaceRepo + ?Sized> WorkspaceRepo for &T {
    fn count(&self) -> Result<u32> {
        (**self).count()
    }
}

impl ProjectRepo for Db {
    fn count(&self) -> Result<u32> {
        self.with_connection(|conn| {
            conn.query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))
        })
    }
}

impl WorkspaceRepo for Db {
    fn count(&self) -> Result<u32> {
        self.with_connection(|conn| {
            conn.query_row("SELECT COUNT(*) FROM workspaces", [], |row| row.get(0))
        })
    }
}
