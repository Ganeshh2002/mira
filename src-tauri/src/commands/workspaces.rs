//! `workspaces.*` — a named way of working on one project.
//!
//! Thin by rule (`architecture.md` §5): read the clock, call the service, map
//! the result. Nothing observed passes through here — a workspace's Git state
//! and services are the *project's*, read live and served by `live.snapshot`, so
//! two workspaces on one project share one set of observations rather than each
//! holding a copy (slice brief §12).

use std::sync::Arc;

use mira_core::{AppKind, ProjectId, Result, Workspace, WorkspaceId};
use mira_platform::{AppReport, Applications};
use mira_workspaces::{WorkspaceService, Workspaces};
use tauri::State;

use crate::clock::now;
use crate::state::AppState;

fn workspaces(state: &AppState) -> Workspaces<&mira_db::Db> {
    Workspaces::new(state.db.as_ref())
}

/// `workspaces.list` — one project's workspaces, most recently opened first.
#[tauri::command]
pub fn workspaces_list(
    project_id: ProjectId,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<Workspace>> {
    workspaces(&state).list_for(project_id)
}

/// `workspaces.create` — add a workspace to a project.
///
/// A name and a project, and that is all that is required. Packages,
/// applications and services are not asked for: a workspace is useful the moment
/// it exists, because everything it shows comes from the project underneath it.
#[tauri::command]
pub fn workspaces_create(
    project_id: ProjectId,
    name: String,
    description: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<Workspace> {
    workspaces(&state).create(project_id, &name, description.as_deref(), now())
}

/// `workspaces.rename` — change a workspace's name and description.
#[tauri::command]
pub fn workspaces_rename(
    workspace_id: WorkspaceId,
    name: String,
    description: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<Workspace> {
    workspaces(&state).rename(workspace_id, &name, description.as_deref(), now())
}

/// `workspaces.open` — make a workspace the one being worked in.
///
/// Opening records *when*, and nothing else. It does not launch an editor, start
/// a server, or restore a window: the project's live context is already being
/// observed, so opening a workspace is a change of view rather than an action on
/// the machine (slice brief §10).
#[tauri::command]
pub fn workspaces_open(
    workspace_id: WorkspaceId,
    state: State<'_, Arc<AppState>>,
) -> Result<Workspace> {
    workspaces(&state).open(workspace_id, now())
}

/// `workspaces.remove` — forget a workspace. Its project is untouched.
#[tauri::command]
pub fn workspaces_remove(workspace_id: WorkspaceId, state: State<'_, Arc<AppState>>) -> Result<()> {
    workspaces(&state).remove(workspace_id)
}

/// `workspaces.set_applications` — which kinds of application this workspace
/// works with.
///
/// Kinds, not applications. The whole list every time, so removing one is
/// sending a list without it.
#[tauri::command]
pub fn workspaces_set_applications(
    workspace_id: WorkspaceId,
    kinds: Vec<AppKind>,
    state: State<'_, Arc<AppState>>,
) -> Result<Workspace> {
    workspaces(&state).set_applications(workspace_id, &kinds, now())
}

/// `workspaces.applications` — whether this machine has an editor, a terminal
/// and a browser.
///
/// Observed, not stored: the association is the user's and travels with the
/// workspace, while availability is read fresh on whichever machine Mira is
/// running on. That is what lets a workspace say "Editor · not installed"
/// instead of quietly forgetting it wanted one.
#[tauri::command]
pub fn workspaces_applications(state: State<'_, Arc<AppState>>) -> Vec<AppReport> {
    Applications::for_os(state.os).survey()
}
