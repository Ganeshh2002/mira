//! `projects.*` — the project lifecycle, as the interface drives it.
//!
//! Thin by rule (`architecture.md` §5): read the clock, call the service, map the
//! result. The one thing that happens here and nowhere else is choosing a
//! directory, because that is a native dialog and the shell is the only layer that
//! may open one.

use mira_core::{MiraError, Project, ProjectId, Result};
use mira_platform::{Shell, ShellHost};
use mira_projects::ProjectService;
use tauri::{AppHandle, Runtime, State};
use tauri_plugin_dialog::DialogExt;

use std::sync::Arc;

use crate::clock::now;
use crate::state::AppState;

/// `projects.list` — every project, most recently opened first.
///
/// Stored facts only. Git is deliberately not read here: the list would otherwise
/// run one repository scan per project on every render, which is the shape of
/// background work Slice 1 has no scheduler to gate.
#[tauri::command]
pub fn projects_list(state: State<'_, Arc<AppState>>) -> Result<Vec<Project>> {
    let projects = state.projects().list()?;
    // The scheduler's gate reads this rather than the database, so it is kept
    // current wherever the list is known to have changed.
    state.set_project_count(projects.len());
    Ok(projects)
}

/// `projects.add` — register a directory the user picks.
///
/// The path is chosen in a native dialog and never crosses the IPC boundary
/// inbound. A page cannot ask Mira to adopt `/` because a page cannot name a
/// directory at all (`security-and-privacy.md` §5).
///
/// Returns `None` when the user dismisses the picker, which is an outcome rather
/// than a failure.
#[tauri::command]
pub async fn projects_add<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<AppState>>,
) -> Result<Option<Project>> {
    let (reply, chosen) = tokio::sync::oneshot::channel();

    app.dialog()
        .file()
        .set_title("Choose a project folder")
        .pick_folder(move |picked| {
            // The receiver is gone only if the window closed while the dialog was
            // open; there is nobody left to tell, so the error is the answer.
            let _ = reply.send(picked);
        });

    let Ok(Some(picked)) = chosen.await else {
        return Ok(None);
    };

    let path = picked.into_path().map_err(|error| {
        MiraError::invalid("path", format!("That folder could not be read: {error}"))
    })?;

    let added = state.projects().add(&path, now())?;
    // A first project opens the gate: there is now something to observe.
    if let Ok(count) = mira_db::ProjectRepo::count(state.db.as_ref()) {
        state.set_project_count(count as usize);
    }
    Ok(Some(added))
}

/// `projects.open` — record that a project was opened and return it.
#[tauri::command]
pub fn projects_open(project_id: ProjectId, state: State<'_, Arc<AppState>>) -> Result<Project> {
    state.projects().open(project_id, now())
}

/// `projects.remove` — forget a project.
///
/// The directory on disk is never touched (`prd.md` AC-1.3).
#[tauri::command]
pub fn projects_remove(project_id: ProjectId, state: State<'_, Arc<AppState>>) -> Result<()> {
    state.projects().remove(project_id)?;
    // Its observations go with it, rather than lingering in a map nobody empties.
    state.live.forget(project_id);
    if let Ok(count) = mira_db::ProjectRepo::count(state.db.as_ref()) {
        state.set_project_count(count as usize);
    }
    Ok(())
}

/// `projects.reveal` — open a project's folder where the user's file manager
/// would open it.
///
/// On Linux this opens the containing folder without selecting anything, which is
/// why the capability reads Degraded there and the button says "Open Folder"
/// rather than promising a selection (`platform-abstraction.md` §4.9).
#[tauri::command]
pub fn projects_reveal(project_id: ProjectId, state: State<'_, Arc<AppState>>) -> Result<()> {
    let project = state.projects().get(project_id)?;
    let root = std::path::Path::new(&project.root_path);

    if !root.is_dir() {
        return Err(MiraError::NotFound {
            what: format!("The folder for \"{}\"", project.name),
        });
    }

    Shell::new(state.os, state.platform.clone()).reveal(root)
}
