//! `git.*` — read-only Git context for one project.

use std::path::PathBuf;
use std::time::Duration;

use mira_core::{MiraError, Project, ProjectId, Result};
use mira_git::{GitOverview, GitProvider, Libgit2};
use mira_projects::ProjectService;
use serde::Serialize;
use tauri::State;
use ts_rs::TS;

use crate::state::AppState;

/// How long one repository read may take before Mira stops waiting.
///
/// `prd.md` feature 3 sets this: a very large monorepo is inherently slow, and the
/// answer is to say so rather than to hang the view. The work is abandoned, not
/// cancelled — libgit2 finishes on its own thread and its result is dropped.
const READ_BUDGET: Duration = Duration::from_secs(2);

/// A project and the Git state of its directory, as of right now.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProjectContext {
    /// The project as stored.
    pub project: Project,
    /// Whether the directory is still on disk.
    ///
    /// A project whose folder was moved or deleted is *Missing*, never
    /// auto-removed (`prd.md` FR-1.5).
    pub directory_exists: bool,
    /// The repository state, or `None` when there is no directory to read.
    pub git: Option<GitOverview>,
}

/// `git.context` — everything the project view shows, read on demand.
///
/// On demand is the point: nothing polls, nothing watches, and this runs only for
/// the project a person is looking at.
#[tauri::command]
pub async fn git_context(
    project_id: ProjectId,
    state: State<'_, AppState>,
) -> Result<ProjectContext> {
    let project = state.projects().get(project_id)?;
    let root = PathBuf::from(&project.root_path);

    if !root.is_dir() {
        return Ok(ProjectContext {
            project,
            directory_exists: false,
            git: None,
        });
    }

    let git = read_within_budget(root, &project.name).await?;

    Ok(ProjectContext {
        project,
        directory_exists: true,
        git: Some(git),
    })
}

/// Read a repository off the UI thread, giving up after [`READ_BUDGET`].
async fn read_within_budget(root: PathBuf, name: &str) -> Result<GitOverview> {
    let reading = tauri::async_runtime::spawn_blocking(move || Libgit2.overview(&root));

    match tokio::time::timeout(READ_BUDGET, reading).await {
        Ok(Ok(overview)) => Ok(overview),
        Ok(Err(error)) => Err(MiraError::external("Git", error)),
        Err(_) => Err(MiraError::Timeout {
            operation: format!("Reading Git for \"{name}\""),
            after_ms: u64::try_from(READ_BUDGET.as_millis()).unwrap_or(u64::MAX),
        }),
    }
}
