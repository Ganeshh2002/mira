//! `live.ports` — every listening socket on this machine, grouped.
//!
//! The one place Mira shows machine-wide data prominently, because *"what has
//! :3000"* is a question asked without a project in mind
//! (`information-architecture.md` §5).
//!
//! **This is not a workspace's Services list and must never be mistaken for
//! one.** A workspace's Services section answers "what did this workspace say
//! matters"; this answers "what is listening on this computer". The first is
//! curated and stored; this is neither — it is a reading, grouped, and gone the
//! moment the process ends.
//!
//! Read-only, and narrower on the way in than on the way out. A row carries a
//! port so it can be recognised; the way back is `at`, a position in the list
//! Mira produced, which is the same shape the workspace's services use
//! ([ADR-0020](../../../docs/adr/0020-workspace-services.md)). No port, address,
//! pid or process name is ever a parameter.
//!
//! Nothing here can stop anything. Termination is a later slice with its own
//! confirmation and refusal design, and its absence from this surface is
//! deliberate rather than pending — there is no disabled Stop button hinting at
//! one.

use std::sync::Arc;

use mira_core::{ProjectId, Result};
use mira_ports::Attribution;
use mira_processes::ProcessFacts;
use mira_projects::ProjectService;
use serde::{Deserialize, Serialize};
use tauri::State;
use ts_rs::TS;

use crate::live::{Service, ServiceObservation};
use crate::state::AppState;

/// One listening socket, as the machine-wide view shows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PortRow {
    /// Where this sits in the observation Mira took. What `live.open_service`
    /// is given — never the port.
    #[ts(type = "number")]
    pub at: u32,
    /// The port it is listening on.
    #[ts(type = "number")]
    pub port: u16,
    /// The address it is bound to, as the platform reported it.
    pub address: String,
    /// The process behind it, where the platform named one.
    ///
    /// Carries cpu, memory and uptime — and **no command line**, because argv
    /// routinely holds credentials ([ADR-0022](../../../docs/adr/0022-process-detail.md)).
    pub process: Option<ProcessFacts>,
}

/// The listeners belonging to one project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PortGroup {
    /// Which project.
    pub project_id: ProjectId,
    /// What it is called, so the group can be read without a second lookup.
    pub project: String,
    /// Its listeners, in the order Mira observed them.
    pub rows: Vec<PortRow>,
}

/// Every listening socket on the machine, grouped as the IA describes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PortsView {
    /// One group per project that owns at least one listener, in project order.
    pub projects: Vec<PortGroup>,
    /// Listeners Mira could not place, with the reason on each row's own
    /// attribution — kept apart rather than dropped, because on Windows this is
    /// most of them and an empty view beside a running dev server would be a
    /// worse lie than an uncertain one.
    pub unattributed: Vec<PortRow>,
    /// When the reading was taken; `None` means it has not been.
    #[ts(type = "number | null")]
    pub observed_at: Option<i64>,
    /// What went wrong, if the last attempt failed.
    pub error: Option<String>,
}

/// Group an observation into the machine-wide view.
///
/// Pure, and separated from the command so it can be measured and asserted
/// without a running application. `named` is every project Mira knows, by id —
/// a listener attributed to a project that has since been removed falls to
/// `unattributed` rather than being shown under a name Mira cannot produce.
#[must_use]
pub fn group(observation: &ServiceObservation, named: &[(ProjectId, String)]) -> PortsView {
    let mut projects: Vec<PortGroup> = Vec::new();
    let mut unattributed: Vec<PortRow> = Vec::new();

    for (at, service) in observation.services.iter().enumerate() {
        let row = row_of(at, service);

        let Attribution::Project { project_id, .. } = service.attribution else {
            unattributed.push(row);
            continue;
        };

        let Some((_, name)) = named.iter().find(|(id, _)| *id == project_id) else {
            unattributed.push(row);
            continue;
        };

        match projects
            .iter_mut()
            .find(|group| group.project_id == project_id)
        {
            Some(group) => group.rows.push(row),
            None => projects.push(PortGroup {
                project_id,
                project: name.clone(),
                rows: vec![row],
            }),
        }
    }

    // Project order, so the view does not reshuffle between readings.
    projects.sort_by_key(|group| group.project_id.get());

    PortsView {
        projects,
        unattributed,
        observed_at: observation.observed_at,
        error: observation.error.clone(),
    }
}

fn row_of(at: usize, service: &Service) -> PortRow {
    PortRow {
        at: u32::try_from(at).unwrap_or(u32::MAX),
        port: service.listener.port,
        address: service.listener.local_address.clone(),
        process: service.process.clone(),
    }
}

/// `live.ports` — the machine-wide view.
///
/// A read of memory and a grouping. It starts no observer and takes no reading
/// of its own: the scheduler already took one, and this is that reading arranged
/// (ADR-0011).
#[tauri::command]
pub fn live_ports(state: State<'_, Arc<AppState>>) -> Result<PortsView> {
    let named: Vec<(ProjectId, String)> = state
        .projects()
        .list()?
        .into_iter()
        .map(|project| (project.id, project.name))
        .collect();

    Ok(group(&state.live.snapshot().services, &named))
}
