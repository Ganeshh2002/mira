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

use mira_core::service::{Port, WatchedService};
use mira_core::{
    AppId, AppKind, AppPreference, MiraError, ProjectId, Result, Workspace, WorkspaceId,
    WorkspaceServiceId,
};
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

    /// Choose which application this workspace uses for one kind.
    ///
    /// `None` clears the choice, which puts that kind back on automatic. Keyed
    /// by workspace, so this changes exactly one workspace and no other.
    fn set_workspace_preference(
        &self,
        id: WorkspaceId,
        kind: AppKind,
        application: Option<&AppId>,
        now: i64,
    ) -> Result<()>;
    /// Every service this workspace watches, in port order.
    ///
    /// Ordered by port rather than by when it was added, so the list reads the
    /// same every time and removing one does not reshuffle the rest.
    fn workspace_services(&self, id: WorkspaceId) -> Result<Vec<WatchedService>>;

    /// Start watching one port for this workspace.
    ///
    /// The port is Mira's own reading of a socket it observed, never a number
    /// the interface sent. Refuses a duplicate by naming it, because "added"
    /// that silently did nothing is worse than a sentence.
    fn watch_service(&self, id: WorkspaceId, port: Port, now: i64) -> Result<WatchedService>;

    /// Stop watching one service.
    ///
    /// Keyed by **both** the row and the workspace, so a row id belonging to a
    /// sibling workspace is `NotFound` rather than a deletion.
    fn forget_service(&self, id: WorkspaceId, service: WorkspaceServiceId) -> Result<()>;

    /// One watched service, if this workspace watches it.
    ///
    /// Keyed by both for the same reason as [`Self::forget_service`]: this is
    /// what a workspace's own row id resolves through before it becomes a port.
    fn workspace_service(
        &self,
        id: WorkspaceId,
        service: WorkspaceServiceId,
    ) -> Result<WatchedService>;
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
                workspace.preferences = preferences(conn, workspace.id)?;
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
            workspace.preferences = preferences(conn, id)?;
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

    fn set_workspace_preference(
        &self,
        id: WorkspaceId,
        kind: AppKind,
        application: Option<&AppId>,
        now: i64,
    ) -> Result<()> {
        // The workspace is read first so choosing for one that is gone is that
        // sentence, rather than a silently-inserted row with no parent.
        self.get_workspace(id)?;

        self.with_connection(|conn| {
            match application {
                Some(chosen) => conn.execute(
                    "INSERT INTO workspace_app_preferences \
                     (workspace_id, kind, application_id, chosen_at) VALUES (?1, ?2, ?3, ?4) \
                     ON CONFLICT (workspace_id, kind) \
                     DO UPDATE SET application_id = ?3, chosen_at = ?4",
                    params![id.get(), kind.as_str(), chosen.as_str(), now],
                )?,
                // Clearing is a delete rather than a sentinel row: automatic is
                // the absence of a choice, and storing "no choice" as a value
                // would give it two spellings.
                None => conn.execute(
                    "DELETE FROM workspace_app_preferences WHERE workspace_id = ?1 AND kind = ?2",
                    params![id.get(), kind.as_str()],
                )?,
            };
            Ok(())
        })?;

        self.touch_updated(id, now)
    }

    fn workspace_services(&self, id: WorkspaceId) -> Result<Vec<WatchedService>> {
        self.with_connection(|conn| services_of(conn, id))
    }

    fn watch_service(&self, id: WorkspaceId, port: Port, now: i64) -> Result<WatchedService> {
        let added = self.with_connection(|conn| {
            let taken = match conn.query_row(
                "SELECT id FROM workspace_services WHERE workspace_id = ?1 AND port = ?2",
                params![id.get(), i64::from(port.get())],
                |row| row.get::<_, i64>(0),
            ) {
                Ok(_) => true,
                Err(rusqlite::Error::QueryReturnedNoRows) => false,
                Err(error) => return Err(error),
            };
            if taken {
                return Ok(None);
            }

            conn.execute(
                "INSERT INTO workspace_services (workspace_id, port, added_at) \
                 VALUES (?1, ?2, ?3)",
                params![id.get(), i64::from(port.get()), now],
            )?;

            Ok(Some(WatchedService {
                id: WorkspaceServiceId::new(conn.last_insert_rowid()),
                workspace_id: id,
                port,
                added_at: now,
            }))
        })?;

        added.ok_or_else(|| {
            MiraError::invalid(
                "service",
                format!("This workspace is already watching port {port}."),
            )
        })
    }

    fn forget_service(&self, id: WorkspaceId, service: WorkspaceServiceId) -> Result<()> {
        // Both keys in the WHERE clause. A row id belonging to a sibling
        // workspace matches nothing, so "not this workspace's" and "not there"
        // are the same answer — which is what keeps one workspace's
        // configuration unreachable from another (ADR-0020).
        let affected = self.with_connection(|conn| {
            conn.execute(
                "DELETE FROM workspace_services WHERE id = ?1 AND workspace_id = ?2",
                params![service.get(), id.get()],
            )
        })?;

        if affected == 0 {
            return Err(MiraError::NotFound {
                what: "That service".to_owned(),
            });
        }
        Ok(())
    }

    fn workspace_service(
        &self,
        id: WorkspaceId,
        service: WorkspaceServiceId,
    ) -> Result<WatchedService> {
        let found = self.with_connection(|conn| {
            match conn.query_row(
                "SELECT id, workspace_id, port, added_at FROM workspace_services \
                 WHERE id = ?1 AND workspace_id = ?2",
                params![service.get(), id.get()],
                row_to_service,
            ) {
                Ok(found) => Ok(found),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(error) => Err(error),
            }
        })?;

        found.ok_or_else(|| MiraError::NotFound {
            what: "That service".to_owned(),
        })
    }
}

impl Db {
    /// Say a workspace's stored state changed, without changing what it is.
    fn touch_updated(&self, id: WorkspaceId, now: i64) -> Result<()> {
        self.with_connection(|conn| {
            conn.execute(
                "UPDATE workspaces SET updated_at = ?2 WHERE id = ?1",
                params![id.get(), now],
            )
        })?;
        Ok(())
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

/// The applications a workspace has chosen, in [`AppKind::ALL`] order.
///
/// A row whose `kind` or `application_id` no longer parses is skipped rather than
/// failing the read: a database written by a later version of Mira should make an
/// older one show less, not refuse to open.
fn preferences(conn: &Connection, id: WorkspaceId) -> rusqlite::Result<Vec<AppPreference>> {
    let mut statement = conn.prepare(
        "SELECT kind, application_id FROM workspace_app_preferences WHERE workspace_id = ?1",
    )?;
    let rows = statement.query_map(params![id.get()], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;

    let stored: Vec<(String, String)> = rows.collect::<rusqlite::Result<_>>()?;

    Ok(AppKind::ALL
        .into_iter()
        .filter_map(|kind| {
            let (_, application) = stored.iter().find(|(row, _)| row == kind.as_str())?;
            Some(AppPreference {
                kind,
                application: application.clone().try_into().ok()?,
            })
        })
        .collect())
}

/// The services a workspace watches, in port order.
///
/// A row whose port is not a port is skipped rather than failing the read. The
/// `CHECK` constraint makes that unreachable through Mira; skipping is what a
/// hand-edited database file gets, and losing one row is better than losing the
/// workspace.
fn services_of(conn: &Connection, id: WorkspaceId) -> rusqlite::Result<Vec<WatchedService>> {
    let mut statement = conn.prepare(
        "SELECT id, workspace_id, port, added_at FROM workspace_services \
         WHERE workspace_id = ?1 ORDER BY port",
    )?;
    let rows = statement.query_map(params![id.get()], row_to_service)?;

    Ok(rows
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect())
}

/// One stored service, or `None` where the port column is not a port.
fn row_to_service(row: &Row<'_>) -> rusqlite::Result<Option<WatchedService>> {
    let Ok(port) = Port::try_from(row.get::<_, i64>(2)?) else {
        return Ok(None);
    };
    Ok(Some(WatchedService {
        id: WorkspaceServiceId::new(row.get(0)?),
        workspace_id: WorkspaceId::new(row.get(1)?),
        port,
        added_at: row.get(3)?,
    }))
}

fn row_to_workspace(row: &Row<'_>) -> rusqlite::Result<Workspace> {
    Ok(Workspace {
        id: WorkspaceId::new(row.get(0)?),
        project_id: ProjectId::new(row.get(1)?),
        name: row.get(2)?,
        description: row.get(3)?,
        applications: Vec::new(),
        preferences: Vec::new(),
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
    fn set_workspace_preference(
        &self,
        id: WorkspaceId,
        kind: AppKind,
        application: Option<&AppId>,
        now: i64,
    ) -> Result<()> {
        (**self).set_workspace_preference(id, kind, application, now)
    }
    fn workspace_services(&self, id: WorkspaceId) -> Result<Vec<WatchedService>> {
        (**self).workspace_services(id)
    }
    fn watch_service(&self, id: WorkspaceId, port: Port, now: i64) -> Result<WatchedService> {
        (**self).watch_service(id, port, now)
    }
    fn forget_service(&self, id: WorkspaceId, service: WorkspaceServiceId) -> Result<()> {
        (**self).forget_service(id, service)
    }
    fn workspace_service(
        &self,
        id: WorkspaceId,
        service: WorkspaceServiceId,
    ) -> Result<WatchedService> {
        (**self).workspace_service(id, service)
    }
}
