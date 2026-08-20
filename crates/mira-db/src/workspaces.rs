//! The workspace repository.
//!
//! Deletion policy, since it is the one thing here a person could be surprised
//! by: **a project's workspaces go with the project.** `ON DELETE CASCADE` in the
//! schema, and deliberate — a workspace is a way of working on a project, so
//! without the project there is nothing left for it to describe. Keeping orphans
//! would leave rows the user can neither see nor reach.
//!
//! That is not the same as a project whose *folder* went missing. The folder is
//! observed; the project and its workspaces are stated, and both stay
//! (`prd.md` FR-1.5).

use mira_core::{AppKind, MiraError, ProjectId, Result, Workspace, WorkspaceId};
use rusqlite::{params, Connection, Row};

use crate::db::Db;

/// What the service layer knows about a workspace before it has an id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewWorkspace {
    /// The project it belongs to.
    pub project_id: ProjectId,
    /// What to call it.
    pub name: String,
    /// What it is for, if the user said.
    pub description: Option<String>,
}

const COLUMNS: &str = "id, project_id, name, description, last_opened_at, created_at, updated_at";

/// Reads and writes of workspaces.
pub trait WorkspaceRepo {
    /// How many workspaces exist, across every project.
    fn count(&self) -> Result<u32>;

    /// One project's workspaces, most recently opened first.
    fn list_for(&self, project_id: ProjectId) -> Result<Vec<Workspace>>;

    /// One workspace by id.
    fn get_workspace(&self, id: WorkspaceId) -> Result<Workspace>;

    /// Store a new workspace and read it back.
    fn create(&self, new: &NewWorkspace, now: i64) -> Result<Workspace>;

    /// Change a workspace's name and description.
    fn rename_workspace(
        &self,
        id: WorkspaceId,
        name: &str,
        description: Option<&str>,
        now: i64,
    ) -> Result<()>;

    /// Record that a workspace was opened.
    fn touch_workspace(&self, id: WorkspaceId, now: i64) -> Result<()>;

    /// Forget a workspace. The project it belonged to is untouched.
    fn remove_workspace(&self, id: WorkspaceId) -> Result<()>;

    /// Replace the kinds of application a workspace works with.
    fn set_workspace_applications(
        &self,
        id: WorkspaceId,
        kinds: &[AppKind],
        now: i64,
    ) -> Result<()>;
}

impl WorkspaceRepo for Db {
    fn count(&self) -> Result<u32> {
        self.with_connection(|conn| {
            conn.query_row("SELECT COUNT(*) FROM workspaces", [], |row| row.get(0))
        })
    }

    fn list_for(&self, project_id: ProjectId) -> Result<Vec<Workspace>> {
        self.with_connection(|conn| {
            let sql = format!(
                "SELECT {COLUMNS} FROM workspaces WHERE project_id = ?1 \
                 ORDER BY last_opened_at IS NULL, last_opened_at DESC, name ASC"
            );
            let mut statement = conn.prepare(&sql)?;
            let rows = statement.query_map(params![project_id.get()], row_to_workspace)?;

            let mut workspaces = Vec::new();
            for workspace in rows {
                let mut workspace = workspace?;
                workspace.applications = applications(conn, workspace.id)?;
                workspaces.push(workspace);
            }
            Ok(workspaces)
        })
    }

    fn get_workspace(&self, id: WorkspaceId) -> Result<Workspace> {
        let found = self.with_connection(|conn| {
            let sql = format!("SELECT {COLUMNS} FROM workspaces WHERE id = ?1");
            let mut workspace = match conn.query_row(&sql, params![id.get()], row_to_workspace) {
                Ok(workspace) => workspace,
                Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
                Err(error) => return Err(error),
            };
            workspace.applications = applications(conn, id)?;
            Ok(Some(workspace))
        })?;

        found.ok_or_else(|| MiraError::NotFound {
            what: "That workspace".to_owned(),
        })
    }

    fn create(&self, new: &NewWorkspace, now: i64) -> Result<Workspace> {
        let id = self.with_connection(|conn| {
            conn.execute(
                "INSERT INTO workspaces (project_id, name, description, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?4)",
                params![new.project_id.get(), new.name, new.description, now],
            )?;
            Ok(conn.last_insert_rowid())
        })?;

        self.get_workspace(WorkspaceId::new(id))
    }

    fn rename_workspace(
        &self,
        id: WorkspaceId,
        name: &str,
        description: Option<&str>,
        now: i64,
    ) -> Result<()> {
        let updated = self.with_connection(|conn| {
            conn.execute(
                "UPDATE workspaces SET name = ?2, description = ?3, updated_at = ?4 \
                 WHERE id = ?1",
                params![id.get(), name, description, now],
            )
        })?;

        missing_is_not_found(updated)
    }

    fn touch_workspace(&self, id: WorkspaceId, now: i64) -> Result<()> {
        let updated = self.with_connection(|conn| {
            conn.execute(
                "UPDATE workspaces SET last_opened_at = ?2, updated_at = ?2 WHERE id = ?1",
                params![id.get(), now],
            )
        })?;

        missing_is_not_found(updated)
    }

    fn remove_workspace(&self, id: WorkspaceId) -> Result<()> {
        let removed = self.with_connection(|conn| {
            conn.execute("DELETE FROM workspaces WHERE id = ?1", params![id.get()])
        })?;

        missing_is_not_found(removed)
    }

    fn set_workspace_applications(
        &self,
        id: WorkspaceId,
        kinds: &[AppKind],
        now: i64,
    ) -> Result<()> {
        self.with_connection(|conn| {
            // Replace rather than merge: the caller sends the whole set, so
            // removing a kind is sending a list without it. A merge would make
            // removal need its own call and its own way to go wrong.
            conn.execute(
                "DELETE FROM workspace_applications WHERE workspace_id = ?1",
                params![id.get()],
            )?;
            for kind in kinds {
                conn.execute(
                    "INSERT OR IGNORE INTO workspace_applications (workspace_id, kind, added_at) \
                     VALUES (?1, ?2, ?3)",
                    params![id.get(), kind.as_str(), now],
                )?;
            }
            Ok(())
        })
    }
}

fn missing_is_not_found(affected: usize) -> Result<()> {
    if affected == 0 {
        return Err(MiraError::NotFound {
            what: "That workspace".to_owned(),
        });
    }
    Ok(())
}

/// The kinds a workspace works with, in [`AppKind::ALL`] order.
///
/// Ordered by the vocabulary rather than by insertion, so the list reads the same
/// every time and toggling one off and on again does not move it.
fn applications(conn: &Connection, id: WorkspaceId) -> rusqlite::Result<Vec<AppKind>> {
    let mut statement =
        conn.prepare("SELECT kind FROM workspace_applications WHERE workspace_id = ?1")?;
    let rows = statement.query_map(params![id.get()], |row| row.get::<_, String>(0))?;

    let stored: Vec<String> = rows.collect::<rusqlite::Result<_>>()?;
    Ok(AppKind::ALL
        .into_iter()
        .filter(|kind| stored.iter().any(|row| row == kind.as_str()))
        .collect())
}

fn row_to_workspace(row: &Row<'_>) -> rusqlite::Result<Workspace> {
    Ok(Workspace {
        id: WorkspaceId::new(row.get(0)?),
        project_id: ProjectId::new(row.get(1)?),
        name: row.get(2)?,
        description: row.get(3)?,
        applications: Vec::new(),
        last_opened_at: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

// So a service can hold `&Db` without owning it.
impl<T: WorkspaceRepo + ?Sized> WorkspaceRepo for &T {
    fn count(&self) -> Result<u32> {
        (**self).count()
    }
    fn list_for(&self, project_id: ProjectId) -> Result<Vec<Workspace>> {
        (**self).list_for(project_id)
    }
    fn get_workspace(&self, id: WorkspaceId) -> Result<Workspace> {
        (**self).get_workspace(id)
    }
    fn create(&self, new: &NewWorkspace, now: i64) -> Result<Workspace> {
        (**self).create(new, now)
    }
    fn rename_workspace(
        &self,
        id: WorkspaceId,
        name: &str,
        description: Option<&str>,
        now: i64,
    ) -> Result<()> {
        (**self).rename_workspace(id, name, description, now)
    }
    fn touch_workspace(&self, id: WorkspaceId, now: i64) -> Result<()> {
        (**self).touch_workspace(id, now)
    }
    fn remove_workspace(&self, id: WorkspaceId) -> Result<()> {
        (**self).remove_workspace(id)
    }
    fn set_workspace_applications(
        &self,
        id: WorkspaceId,
        kinds: &[AppKind],
        now: i64,
    ) -> Result<()> {
        (**self).set_workspace_applications(id, kinds, now)
    }
}
