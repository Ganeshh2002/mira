//! `app.*` — what the shell knows about itself.

use mira_core::{CapabilityReport, MiraError, Result};
use mira_platform::PlatformCapabilities;
use mira_projects::{ProjectService, Projects};
use mira_workspaces::{WorkspaceService, Workspaces};
use serde::Serialize;
use tauri::{AppHandle, Runtime, State};
use ts_rs::TS;

use crate::state::AppState;
use crate::windows;

/// Proof that the foundation is wired end to end.
///
/// Every field here crosses one architectural boundary: the schema version comes
/// from `mira-db`, the counts through the repository and service traits, the
/// capabilities from `mira-platform`. If this command answers, the spine works.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FoundationStatus {
    /// The application name.
    pub app_name: String,
    /// Its version, from `Cargo.toml`.
    pub app_version: String,
    /// The operating system, as a person would name it.
    pub platform: String,
    /// The windowing system — the distinction a compile-time target cannot make.
    pub session: String,
    /// Where the SQLite file lives.
    pub database_path: String,
    /// The applied schema version.
    pub schema_version: i32,
    /// How many projects exist. Zero until Slice 1 can add one.
    pub project_count: u32,
    /// How many workspaces exist. Zero until 0.2.
    pub workspace_count: u32,
    /// The chord Mira tried to register.
    pub shortcut_chord: String,
    /// Whether that registration succeeded here.
    pub shortcut_registered: bool,
    /// Every capability and its honest status on this machine.
    pub capabilities: Vec<CapabilityReport>,
}

/// `app.foundation_status` — read the shell's own state.
#[tauri::command]
pub fn app_foundation_status(state: State<'_, AppState>) -> Result<FoundationStatus> {
    let projects = Projects::new(state.db.as_ref());
    let workspaces = Workspaces::new(state.db.as_ref());

    Ok(FoundationStatus {
        app_name: "Mira".to_owned(),
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        platform: state.platform.facts().os.label().to_owned(),
        session: state.platform.facts().display_server.label().to_owned(),
        database_path: state.database_path.display().to_string(),
        schema_version: state.db.schema_version(),
        project_count: projects.count()?,
        workspace_count: workspaces.count()?,
        shortcut_chord: state.shortcut_chord.clone(),
        shortcut_registered: state.shortcut_registered,
        capabilities: state.platform.report(),
    })
}

/// `app.open_settings` — the settings entry point, mirroring the tray item.
///
/// The window is built on the first call rather than at start-up, so a surface
/// nobody opens costs nothing (`windows::open_settings`).
#[tauri::command]
pub fn app_open_settings<R: Runtime>(app: AppHandle<R>) -> Result<()> {
    windows::open_settings(&app).map_err(|error| MiraError::External {
        source: "the window system".to_owned(),
        detail: format!("Settings could not be opened: {error}"),
    })
}
