//! Repository traits.
//!
//! Projects live in [`crate::projects`]; this module holds what is left.

use mira_core::Result;

use crate::db::Db;

/// Reads and writes of workspaces.
///
/// Workspaces are a 0.2 concept (`product-scope.md` §2). The boundary exists so the
/// project/workspace/session distinction stays real while only projects are
/// user-facing, per `information-architecture.md` §1.
pub trait WorkspaceRepo {
    /// How many workspaces exist.
    fn count(&self) -> Result<u32>;
}

// So a service can hold `&Db` without owning it.
impl<T: WorkspaceRepo + ?Sized> WorkspaceRepo for &T {
    fn count(&self) -> Result<u32> {
        (**self).count()
    }
}

impl WorkspaceRepo for Db {
    fn count(&self) -> Result<u32> {
        self.with_connection(|conn| {
            conn.query_row("SELECT COUNT(*) FROM workspaces", [], |row| row.get(0))
        })
    }
}
