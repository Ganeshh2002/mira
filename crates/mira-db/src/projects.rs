//! The project repository.
//!
//! Every statement here is written out in full rather than built from fragments:
//! the frontend cannot reach SQL (ADR-0004), and neither can a caller of this
//! module — values arrive only as bound parameters.

use mira_core::{MiraError, Project, ProjectId, Result};
use rusqlite::{params, Connection, Row};

use crate::db::Db;

/// What the service layer knows about a project before it has an id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewProject {
    /// The inferred display name.
    pub name: String,
    /// The canonical absolute path.
    pub root_path: String,
    /// Whether a Git worktree was found.
    pub is_git: bool,
    /// The worktree root, when it differs from the project root.
    pub git_root: Option<String>,
    /// Detected type markers.
    pub markers: Vec<String>,
}

const COLUMNS: &str =
    "id, name, root_path, is_git, git_root, last_opened_at, created_at, updated_at";

/// Reads and writes of projects.
pub trait ProjectRepo {
    /// How many projects exist.
    fn count(&self) -> Result<u32>;

    /// Every project, most recently opened first (`prd.md` FR-1.6).
    fn list(&self) -> Result<Vec<Project>>;

    /// One project by id.
    fn get(&self, id: ProjectId) -> Result<Project>;

    /// The project registered for a canonical root, if there is one.
    fn find_by_root(&self, root_path: &str) -> Result<Option<Project>>;

    /// Store a new project and its markers, and read it back.
    fn insert(&self, new: &NewProject, now: i64) -> Result<Project>;

    /// Record that a project was opened.
    fn touch_opened(&self, id: ProjectId, now: i64) -> Result<()>;

    /// Forget a project. The directory on disk is never touched (FR-1.4).
    fn remove(&self, id: ProjectId) -> Result<()>;
}

impl ProjectRepo for Db {
    fn count(&self) -> Result<u32> {
        self.with_connection(|conn| {
            conn.query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))
        })
    }

    fn list(&self) -> Result<Vec<Project>> {
        self.with_connection(|conn| {
            let sql = format!(
                "SELECT {COLUMNS} FROM projects \
                 ORDER BY sort_order ASC, last_opened_at DESC, id DESC"
            );
            let mut statement = conn.prepare(&sql)?;
            let rows = statement.query_map([], row_to_project)?;

            let mut projects = Vec::new();
            for project in rows {
                let mut project = project?;
                project.markers = markers(conn, project.id)?;
                projects.push(project);
            }
            Ok(projects)
        })
    }

    fn get(&self, id: ProjectId) -> Result<Project> {
        let found = self.with_connection(|conn| {
            let sql = format!("SELECT {COLUMNS} FROM projects WHERE id = ?1");
            let mut project = match conn.query_row(&sql, params![id.get()], row_to_project) {
                Ok(project) => project,
                Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
                Err(error) => return Err(error),
            };
            project.markers = markers(conn, id)?;
            Ok(Some(project))
        })?;

        found.ok_or_else(|| MiraError::NotFound {
            what: "That project".to_owned(),
        })
    }

    fn find_by_root(&self, root_path: &str) -> Result<Option<Project>> {
        self.with_connection(|conn| {
            let sql = format!("SELECT {COLUMNS} FROM projects WHERE root_path = ?1");
            let mut project = match conn.query_row(&sql, params![root_path], row_to_project) {
                Ok(project) => project,
                Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
                Err(error) => return Err(error),
            };
            project.markers = markers(conn, project.id)?;
            Ok(Some(project))
        })
    }

    fn insert(&self, new: &NewProject, now: i64) -> Result<Project> {
        let id = self.with_connection(|conn| {
            conn.execute(
                "INSERT INTO projects \
                 (name, root_path, is_git, git_root, last_opened_at, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?5)",
                params![
                    new.name,
                    new.root_path,
                    i32::from(new.is_git),
                    new.git_root,
                    now
                ],
            )?;
            let id = conn.last_insert_rowid();

            for marker in &new.markers {
                conn.execute(
                    "INSERT OR IGNORE INTO project_markers (project_id, marker, detected_at) \
                     VALUES (?1, ?2, ?3)",
                    params![id, marker, now],
                )?;
            }
            Ok(id)
        })?;

        self.get(ProjectId::new(id))
    }

    fn touch_opened(&self, id: ProjectId, now: i64) -> Result<()> {
        let updated = self.with_connection(|conn| {
            conn.execute(
                "UPDATE projects SET last_opened_at = ?2, updated_at = ?2 WHERE id = ?1",
                params![id.get(), now],
            )
        })?;

        missing_is_not_found(updated)
    }

    fn remove(&self, id: ProjectId) -> Result<()> {
        let removed = self.with_connection(|conn| {
            conn.execute("DELETE FROM projects WHERE id = ?1", params![id.get()])
        })?;

        missing_is_not_found(removed)
    }
}

fn missing_is_not_found(affected: usize) -> Result<()> {
    if affected == 0 {
        return Err(MiraError::NotFound {
            what: "That project".to_owned(),
        });
    }
    Ok(())
}

fn markers(conn: &Connection, id: ProjectId) -> rusqlite::Result<Vec<String>> {
    let mut statement =
        conn.prepare("SELECT marker FROM project_markers WHERE project_id = ?1 ORDER BY marker")?;
    let rows = statement.query_map(params![id.get()], |row| row.get::<_, String>(0))?;
    rows.collect()
}

fn row_to_project(row: &Row<'_>) -> rusqlite::Result<Project> {
    Ok(Project {
        id: ProjectId::new(row.get(0)?),
        name: row.get(1)?,
        root_path: row.get(2)?,
        is_git: row.get::<_, i32>(3)? != 0,
        git_root: row.get(4)?,
        markers: Vec::new(),
        last_opened_at: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

// So a service can hold `&Db` without owning it.
impl<T: ProjectRepo + ?Sized> ProjectRepo for &T {
    fn count(&self) -> Result<u32> {
        (**self).count()
    }
    fn list(&self) -> Result<Vec<Project>> {
        (**self).list()
    }
    fn get(&self, id: ProjectId) -> Result<Project> {
        (**self).get(id)
    }
    fn find_by_root(&self, root_path: &str) -> Result<Option<Project>> {
        (**self).find_by_root(root_path)
    }
    fn insert(&self, new: &NewProject, now: i64) -> Result<Project> {
        (**self).insert(new, now)
    }
    fn touch_opened(&self, id: ProjectId, now: i64) -> Result<()> {
        (**self).touch_opened(id, now)
    }
    fn remove(&self, id: ProjectId) -> Result<()> {
        (**self).remove(id)
    }
}
