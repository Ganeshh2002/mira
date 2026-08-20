//! Observed runtime state.
//!
//! `data-model.md` §1 rule 2 draws the line this module sits on: **projects,
//! workspaces and preferences are things a person stated and are written to
//! disk; Git state, listening ports and running processes are observed and are
//! not.** Everything here lives in memory for as long as the process does, and
//! nothing here has a table.
//!
//! Every observation carries when it happened and whether it failed, because the
//! interface has to be able to say "updated 3 s ago" and "port information
//! unavailable" rather than showing a stale answer as a current one.

use std::collections::HashMap;
use std::sync::RwLock;

use mira_core::{MiraError, Project, ProjectId};
use mira_git::GitOverview;
use mira_monorepo::RepositoryLayout;
use mira_ports::{Attribution, Listener};
use mira_processes::ProcessFacts;
use serde::Serialize;
use ts_rs::TS;

/// One listening socket, its process, and where it belongs.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Service {
    /// The socket.
    pub listener: Listener,
    /// The process that owns it, where the platform reports one.
    pub process: Option<ProcessFacts>,
    /// Which project it belongs to, or why Mira will not say.
    pub attribution: Attribution,
}

/// What Mira last saw of one project.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProjectObservation {
    /// Which project.
    pub project_id: ProjectId,
    /// Whether the directory is still on disk.
    pub directory_exists: bool,
    /// The repository state, absent when the directory is gone.
    pub git: Option<GitOverview>,
    /// How the directory sits in the repository around it.
    pub layout: Option<RepositoryLayout>,
    /// What went wrong, if the last attempt failed.
    ///
    /// Present alongside a previous `git`: the interface shows the older reading
    /// *and* says it could not be refreshed, which is more useful than either
    /// alone and is never mistaken for current.
    pub error: Option<String>,
    /// When this was read, Unix epoch seconds UTC.
    #[ts(type = "number")]
    pub observed_at: i64,
}

/// What Mira last saw of the machine's listening ports.
#[derive(Debug, Clone, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ServiceObservation {
    /// The services found.
    pub services: Vec<Service>,
    /// What went wrong, if the last attempt failed.
    pub error: Option<String>,
    /// When this was read; `None` means it has not been read yet.
    #[ts(type = "number | null")]
    pub observed_at: Option<i64>,
}

/// Everything observed, as one message to the interface.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LiveSnapshot {
    /// One entry per project that has been observed at least once.
    pub projects: Vec<ProjectObservation>,
    /// The machine's listening ports.
    pub services: ServiceObservation,
}

/// The in-memory store the observers write and the commands read.
///
/// A read-write lock rather than a channel: readers are the command handlers,
/// which are frequent and short, and writers are two observers on a five-second
/// interval. There is no contention to manage.
#[derive(Debug, Default)]
pub struct Live {
    projects: RwLock<HashMap<i64, ProjectObservation>>,
    services: RwLock<ServiceObservation>,
}

impl Live {
    /// An empty store. Nothing has been observed yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record what was seen of one project.
    pub fn record_project(&self, observation: ProjectObservation) {
        if let Ok(mut projects) = self.projects.write() {
            projects.insert(observation.project_id.get(), observation);
        }
    }

    /// Record a failed read of one project, keeping the previous reading.
    ///
    /// The older answer stays visible with the failure beside it. Dropping it
    /// would replace something true-but-old with nothing at all, and the
    /// timestamp already tells a person how much to trust it.
    pub fn record_project_error(&self, project_id: ProjectId, error: &MiraError, now: i64) {
        let Ok(mut projects) = self.projects.write() else {
            return;
        };

        match projects.get_mut(&project_id.get()) {
            Some(existing) => {
                existing.error = Some(error.to_string());
                existing.observed_at = now;
            }
            None => {
                projects.insert(
                    project_id.get(),
                    ProjectObservation {
                        project_id,
                        directory_exists: false,
                        git: None,
                        layout: None,
                        error: Some(error.to_string()),
                        observed_at: now,
                    },
                );
            }
        }
    }

    /// Record what was seen of the machine's ports.
    pub fn record_services(&self, services: Vec<Service>, now: i64) {
        if let Ok(mut observed) = self.services.write() {
            *observed = ServiceObservation {
                services,
                error: None,
                observed_at: Some(now),
            };
        }
    }

    /// Record that the ports could not be read.
    ///
    /// The service list is emptied here, unlike a project's Git state: a stale
    /// list of servers is actively misleading, because the whole question it
    /// answers is *what is running right now*.
    pub fn record_services_error(&self, error: &MiraError, now: i64) {
        if let Ok(mut observed) = self.services.write() {
            *observed = ServiceObservation {
                services: Vec::new(),
                error: Some(error.to_string()),
                observed_at: Some(now),
            };
        }
    }

    /// Forget a project that is no longer on the list.
    ///
    /// Called when a project is removed, so its observations disappear with it
    /// rather than lingering in a map nobody empties.
    pub fn forget(&self, project_id: ProjectId) {
        if let Ok(mut projects) = self.projects.write() {
            projects.remove(&project_id.get());
        }
    }

    /// Drop every observation for a project that is no longer known.
    pub fn retain_only(&self, keep: &[Project]) {
        if let Ok(mut projects) = self.projects.write() {
            projects.retain(|id, _| keep.iter().any(|project| project.id.get() == *id));
        }
    }

    /// The layout last observed for a project, for attributing services to it.
    #[must_use]
    pub fn layout_of(&self, project_id: ProjectId) -> Option<RepositoryLayout> {
        self.projects
            .read()
            .ok()?
            .get(&project_id.get())?
            .layout
            .clone()
    }

    /// Everything observed so far.
    #[must_use]
    pub fn snapshot(&self) -> LiveSnapshot {
        let mut projects: Vec<ProjectObservation> = self
            .projects
            .read()
            .map(|projects| projects.values().cloned().collect())
            .unwrap_or_default();
        projects.sort_by_key(|observation| observation.project_id.get());

        let services = self
            .services
            .read()
            .map(|services| services.clone())
            .unwrap_or_default();

        LiveSnapshot { projects, services }
    }
}
