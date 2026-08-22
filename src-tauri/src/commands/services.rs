//! `workspaces.*_service` — which of a project's services a workspace watches.
//!
//! A workspace is a way of working on a project, and part of what somebody
//! working means is *these are the things that should be up*. The observers see
//! every listening socket on the machine and attribute what they can to
//! projects; nothing in that says which ones a person cares about, so a
//! workspace on a monorepo shows twelve services when two of them are the work.
//!
//! **Nothing in this module accepts a port, an address, a URL, a process name or
//! a pid.** A service is added by naming a *position* in the list Mira offered
//! (the same shape as a file in a change set, `security-and-privacy.md` §5 rule
//! 31), and is opened or forgotten by the **row id Mira issued** when it was
//! added. The port lives on Mira's side of the boundary in both directions: it
//! is read from an observation Mira took, written to Mira's own database, and
//! turned into `http://localhost:<port>` in Rust.
//!
//! No observer, no timer, no cache. Watching is stored rows; state is a pure
//! function of those rows and the reading the scheduler already took (ADR-0011,
//! ADR-0020).

use std::sync::Arc;

use mira_core::service::{Listening, WorkspaceService as Watched};
use mira_core::{AppKind, Result, WorkspaceId, WorkspaceServiceId};
use mira_platform::{LaunchHost, LaunchTarget, Launched, Launcher};
use mira_workspaces::{WorkspaceService, Workspaces};
use serde::{Deserialize, Serialize};
use tauri::State;
use ts_rs::TS;

use crate::clock::now;
use crate::commands::live::localhost;
use crate::state::AppState;

/// One of the project's observed services, as something a workspace could add.
///
/// Carries the port so the list can be read — a person picks a service by
/// recognising `:5173 vite`, and hiding the number would make the chooser
/// useless. The port travels **outward** only: it is a reading being displayed,
/// and the way back is `at`, which is a position in this list rather than a
/// value the caller composed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ServiceOffer {
    /// Where this sits in the list. What [`workspaces_watch_service`] is given.
    #[ts(type = "number")]
    pub at: u32,
    /// The port it is listening on.
    #[ts(type = "number")]
    pub port: u16,
    /// The address it is bound to.
    pub address: String,
    /// The owning process's name, where the platform reports one.
    pub process: Option<String>,
    /// Whether this workspace already watches it.
    ///
    /// Sent rather than filtered out, so that `at` indexes a list that only
    /// changes when the *machine* changes. A list that also reshuffled when a
    /// selection changed would make every ordinal a race with the person using
    /// it.
    pub watched: bool,
}

fn workspaces(state: &AppState) -> Workspaces<&mira_db::Db> {
    Workspaces::new(state.db.as_ref())
}

/// Everything observed for one workspace's project, in the order Mira lists it.
///
/// The single source of the ordinals. Both the offer list and the act of
/// watching resolve `at` against *this* function, so the two cannot drift apart.
fn offered(state: &AppState, workspace_id: WorkspaceId) -> Result<Vec<Listening>> {
    let project_id = workspaces(state).get(workspace_id)?.project_id;

    Ok(state
        .live
        .snapshot()
        .services
        .listening()
        .into_iter()
        .filter(|listening| listening.project_id == Some(project_id))
        .collect())
}

/// `workspaces.services` — what this workspace watches, and where each stands.
///
/// Resolved on every call against the reading the scheduler already took. A
/// service that has stopped comes back as its port and a state, never as a
/// remembered copy of what used to be listening there.
#[tauri::command]
pub fn workspaces_services(
    workspace_id: WorkspaceId,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<Watched>> {
    let observation = state.live.snapshot().services;
    let listening = observation.listening();

    workspaces(&state).services_unobserved(workspace_id, observation.observed(&listening))
}

/// `workspaces.service_offers` — the project's services, as things to add.
///
/// Every service Mira attributed to this workspace's project, whether or not it
/// is already watched. Unattributed services are not offered: a socket Mira
/// cannot place is not this project's to claim, and offering it would be the
/// interface inviting somebody to attach an unrelated process to their work.
#[tauri::command]
pub fn workspaces_service_offers(
    workspace_id: WorkspaceId,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<ServiceOffer>> {
    let watched = workspaces(&state).services(workspace_id, &[])?;

    Ok(offered(&state, workspace_id)?
        .into_iter()
        .enumerate()
        .map(|(at, listening)| ServiceOffer {
            at: u32::try_from(at).unwrap_or(u32::MAX),
            port: listening.port.get(),
            address: listening.address,
            process: listening.process,
            watched: watched
                .iter()
                .any(|service| service.watched.port == listening.port),
        })
        .collect())
}

/// `workspaces.watch_service` — start watching one of the project's services.
///
/// `at` is a position in the list [`workspaces_service_offers`] returned, and
/// that is the whole privilege: the interface can ask for a service Mira already
/// decided to offer, and nothing else. An ordinal past the end is a stale
/// selection rather than an attempt at anything, and comes back as
/// `NotFound` (`security-and-privacy.md` §5 rule 31, applied to a port).
#[tauri::command]
pub fn workspaces_watch_service(
    workspace_id: WorkspaceId,
    at: u32,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<Watched>> {
    let offers = offered(&state, workspace_id)?;
    workspaces(&state).watch(workspace_id, at, &offers, now())?;

    workspaces_services(workspace_id, state)
}

/// `workspaces.forget_service` — stop watching one.
///
/// Named by the row id Mira issued. A row id belonging to a *sibling* workspace
/// matches nothing, because the delete is keyed by both the row and the
/// workspace — so one workspace's configuration is not merely hidden from
/// another, it is unreachable (ADR-0020).
#[tauri::command]
pub fn workspaces_forget_service(
    workspace_id: WorkspaceId,
    service_id: WorkspaceServiceId,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<Watched>> {
    workspaces(&state).forget(workspace_id, service_id)?;

    workspaces_services(workspace_id, state)
}

/// `workspaces.open_service` — open a watched service in the browser.
///
/// Two row ids in, and everything else resolved beneath the boundary: the port
/// comes from the workspace's own row, the address is built in Rust, and the
/// browser is reached through the same launcher an editor is
/// (`security-and-privacy.md` §5 rule 5, ADR-0013).
///
/// **A service that is not running is refused rather than opened.** Handing a
/// browser a loopback address with nothing behind it produces a connection error
/// in a window somebody has to close; saying so here is the same refusal, one
/// step earlier and in Mira's own words.
#[tauri::command]
pub fn workspaces_open_service(
    workspace_id: WorkspaceId,
    service_id: WorkspaceServiceId,
    state: State<'_, Arc<AppState>>,
) -> Result<Launched> {
    open_watched(&state, workspace_id, service_id)
}

/// Open one of a workspace's watched services, by the row id Mira issued.
///
/// The body of the command above, shared with the action catalogue's
/// "open the running service" so that both take the same path: the port comes
/// from a row keyed by **this** workspace, the address is built in Rust from
/// that port, and the browser is the one this workspace chose. A second
/// implementation would be a second place for an address to come from, which is
/// the thing `security-and-privacy.md` §5 rule 5 exists to prevent.
pub fn open_watched(
    state: &AppState,
    workspace_id: WorkspaceId,
    service_id: WorkspaceServiceId,
) -> Result<Launched> {
    let observation = state.live.snapshot().services;
    let listening = observation.listening();

    let watched = workspaces(state)
        .services_unobserved(workspace_id, observation.observed(&listening))?
        .into_iter()
        .find(|service| service.watched.id == service_id)
        .ok_or_else(|| mira_core::MiraError::NotFound {
            what: "That service".to_owned(),
        })?;

    if !watched.state.is_running() {
        return Err(mira_core::MiraError::NotFound {
            what: format!("Anything listening on port {}", watched.watched.port),
        });
    }

    // Through `port_of` rather than through the value just read, so the number
    // handed to the launcher came from the database keyed by both ids — the same
    // check that makes a sibling's service unreachable.
    let port = workspaces(state).port_of(workspace_id, service_id)?;

    // The workspace's own browser choice, not the machine's first find. A
    // workspace that chose Firefox opens its service in Firefox, and one that
    // chose nothing gets the desktop's default handler — the same answer the
    // Open with row gives (ADR-0019).
    let preferred = crate::commands::workspaces::preference(state, workspace_id, AppKind::Browser)?;

    Launcher::new(state.os, state.platform.clone()).launch(
        AppKind::Browser,
        preferred.as_ref(),
        LaunchTarget::WebAddress(localhost(port.get())),
    )
}
