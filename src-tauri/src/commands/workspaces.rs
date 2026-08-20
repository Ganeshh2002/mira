//! `workspaces.*` — a named way of working on one project.
//!
//! Thin by rule (`architecture.md` §5): read the clock, call the service, map
//! the result. `workspaces.launch` is the one that does something outside Mira,
//! and it is thin in the way that matters: it accepts a **row id and a kind**,
//! and everything else — which directory, which application, which argv — is
//! resolved underneath it from Mira's own database and a table compiled into the
//! binary (`security-and-privacy.md` §5). Nothing observed passes through here — a workspace's Git state
//! and services are the *project's*, read live and served by `live.snapshot`, so
//! two workspaces on one project share one set of observations rather than each
//! holding a copy (slice brief §12).

use std::sync::Arc;

use mira_core::{AppKind, ProjectId, Result, Workspace, WorkspaceId};
use mira_platform::{AppReport, Applications, LaunchHost, LaunchTarget, Launched, Launcher};
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

/// `workspaces.launch` — open this workspace's project in an application.
///
/// The whole privilege surface of launching, in two arguments: **which
/// workspace**, and **which kind**. There is no path here and no command; the
/// directory is the project's canonical root, resolved in Rust from a folder the
/// user picked natively, and the application is the first entry in a fixed table
/// that this machine actually has. A page cannot name a program, cannot add an
/// argument, and cannot open a directory Mira was not already given.
///
/// A missing folder is a refusal rather than a launch, and the workspace is left
/// exactly as it was (`prd.md` FR-1.5).
#[tauri::command]
pub fn workspaces_launch(
    workspace_id: WorkspaceId,
    kind: AppKind,
    state: State<'_, Arc<AppState>>,
) -> Result<Launched> {
    let root = workspaces(&state).working_directory(workspace_id)?;

    Launcher::new(state.os, state.platform.clone()).launch(kind, LaunchTarget::Directory(root))
}

/// `workspaces.openable` — which kinds this machine can open a folder in.
///
/// Not the same question as `workspaces.applications`, and the difference is
/// visible: a machine whose only editor is Neovim *has* an editor and has
/// nothing Mira can open a folder in, so the Context row says "Neovim" and no
/// button appears. Reporting one answer for both would have to lie about one of
/// them.
#[tauri::command]
pub fn workspaces_openable(state: State<'_, Arc<AppState>>) -> Vec<AppReport> {
    Launcher::new(state.os, state.platform.clone()).openable()
}
