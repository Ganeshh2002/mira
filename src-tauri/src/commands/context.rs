//! `projects.context` — everything the project view shows, read on demand.

use std::path::{Path, PathBuf};
use std::time::Duration;

use mira_core::{MiraError, Project, ProjectId, Result};
use mira_git::{GitOverview, GitProvider, Libgit2};
use mira_monorepo::RepositoryLayout;
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
    /// How the directory sits in the repository around it: on its own, as the
    /// root of a monorepo, or as one package inside one.
    ///
    /// Computed here rather than stored: a workspace is edited by the people
    /// working in it, and a cached package list is a list that goes wrong
    /// silently (`data-model.md` §1 rule 2).
    pub layout: Option<RepositoryLayout>,
}

/// `projects.context` — everything the project view shows, read on demand.
///
/// On demand is the point: nothing polls, nothing watches, and this runs only for
/// the project a person is looking at.
#[tauri::command]
pub async fn projects_context(
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
            layout: None,
        });
    }

    // The worktree root bounds the search upwards: a workspace declared in another
    // repository says nothing about this one. `git_root` is stored only when it
    // differs from the project root, so the project root is the fallback.
    let git_root = project
        .git_root
        .as_deref()
        .map(PathBuf::from)
        .or_else(|| project.is_git.then(|| root.clone()));

    let (git, layout) = read_within_budget(root, git_root, &project.name).await?;

    Ok(ProjectContext {
        project,
        directory_exists: true,
        git: Some(git),
        layout: Some(layout),
    })
}

/// Read the repository off the UI thread, giving up after [`READ_BUDGET`].
///
/// Git and the workspace layout are read together because they answer one
/// question — what is this directory — and both are filesystem work that has no
/// business on the webview's thread.
async fn read_within_budget(
    root: PathBuf,
    git_root: Option<PathBuf>,
    name: &str,
) -> Result<(GitOverview, RepositoryLayout)> {
    let reading = tauri::async_runtime::spawn_blocking(move || {
        let git = Libgit2.overview(&root);
        let layout = mira_monorepo::detect(&root, git_root.as_deref().map(Path::new));
        (git, layout)
    });

    match tokio::time::timeout(READ_BUDGET, reading).await {
        Ok(Ok(read)) => Ok(read),
        Ok(Err(error)) => Err(MiraError::external("Git", error)),
        Err(_) => Err(MiraError::Timeout {
            operation: format!("Reading \"{name}\""),
            after_ms: u64::try_from(READ_BUDGET.as_millis()).unwrap_or(u64::MAX),
        }),
    }
}
