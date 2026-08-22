//! What Mira watches, and how often.
//!
//! Two observers, each independent of the other and each knowing nothing about
//! the scheduler beyond the [`Observation`] contract. The scheduler decides
//! *when*; these decide *what*.
//!
//! Both are blocking: reading a repository, walking the socket table and reading
//! the process table are syscall work, and `architecture.md` §6 puts syscall work
//! on the blocking pool rather than on the async workers.

use std::sync::Arc;
use std::time::Duration;

use mira_core::{MiraError, Project, ProjectId, Result};
use mira_fs::PathMatching;
use mira_git::{GitProvider, Libgit2};
use mira_monorepo::RepositoryLayout;
use mira_ports::{
    attribute, Attribution, PackageBoundary, PortScanner, Ports, ProjectRoot, Unattributed,
};
use mira_processes::ProcessProvider;
use mira_projects::ProjectService;
use mira_scheduler::Observation;

use crate::clock::now;
use crate::live::{Live, Service};
use crate::state::AppState;

/// How often a project's repository is re-read.
///
/// Five seconds is the figure `information-architecture.md` §3 gives for the
/// always-fresh tier — long enough that a repository is not being read
/// constantly, short enough that saving a file and glancing at Mira shows the
/// change. It is a constant here rather than a number buried in a call so that
/// changing it is one edit and one conversation.
pub const GIT_INTERVAL: Duration = Duration::from_secs(5);

/// How often the machine's listening ports are re-read.
///
/// The same five seconds, and for the same reason: `roadmap.md` slice 2 asks for
/// a started dev server to appear "within 5 s".
pub const SERVICES_INTERVAL: Duration = Duration::from_secs(5);

/// Told after each round that something was observed.
///
/// A closure rather than an `AppHandle`, so nothing in this module knows Tauri
/// exists and every observer can be tested with a counter.
pub type Announce = Arc<dyn Fn() + Send + Sync>;

/// A notifier that tells nobody. For tests and for one-off observation.
#[must_use]
pub fn silent() -> Announce {
    Arc::new(|| {})
}

/// Re-reads every project's Git state and repository layout.
pub struct GitObserver {
    state: Arc<AppState>,
    announce: Announce,
}

impl GitObserver {
    /// Watch the projects this state holds.
    #[must_use]
    pub const fn new(state: Arc<AppState>, announce: Announce) -> Self {
        Self { state, announce }
    }
}

impl Observation for GitObserver {
    fn name(&self) -> &'static str {
        "Git"
    }

    fn interval(&self) -> Duration {
        GIT_INTERVAL
    }

    fn observe(&self) -> Result<()> {
        let projects = self.state.projects().list()?;
        // A project removed since the last round leaves nothing behind.
        self.state.live.retain_only(&projects);

        for project in &projects {
            let at = now();
            let root = std::path::Path::new(&project.root_path);

            if !root.is_dir() {
                // A folder that was moved or deleted is a state, not a failure.
                // Mira keeps the project and says the folder is missing.
                self.state
                    .live
                    .record_project(crate::live::ProjectObservation {
                        project_id: project.id,
                        directory_exists: false,
                        git: None,
                        layout: None,
                        error: None,
                        observed_at: at,
                    });
                continue;
            }

            let git = Libgit2.overview(root);
            let layout = mira_monorepo::detect(root, git_root(project).as_deref());

            self.state
                .live
                .record_project(crate::live::ProjectObservation {
                    project_id: project.id,
                    directory_exists: true,
                    git: Some(git),
                    layout: Some(layout),
                    error: None,
                    observed_at: at,
                });
        }

        (self.announce)();
        Ok(())
    }
}

/// Re-reads the machine's listening ports and places them.
pub struct ServiceObserver {
    state: Arc<AppState>,
    announce: Announce,
}

impl ServiceObserver {
    /// Watch the ports on this machine.
    #[must_use]
    pub const fn new(state: Arc<AppState>, announce: Announce) -> Self {
        Self { state, announce }
    }
}

impl Observation for ServiceObserver {
    fn name(&self) -> &'static str {
        "listening ports"
    }

    fn interval(&self) -> Duration {
        SERVICES_INTERVAL
    }

    fn observe(&self) -> Result<()> {
        let at = now();

        let listeners = match Ports::new().listening() {
            Ok(listeners) => listeners,
            Err(error) => {
                // Recorded rather than returned, so the interface can say "port
                // information unavailable" instead of showing an empty list that
                // looks like "nothing is running".
                self.state.live.record_services_error(&error, at);
                return Err(error);
            }
        };

        let pids: Vec<u32> = listeners
            .iter()
            .filter_map(|listener| listener.pid)
            .collect();
        // The state's kept reader, not a fresh one. CPU share is a rate, and a
        // rate needs the previous sample — building a provider here would throw
        // it away every tick and report zero forever (ADR-0022).
        let facts = self.state.processes.facts_for(&pids);
        let roots = self.roots()?;

        let services = listeners
            .into_iter()
            .map(|listener| {
                let process = listener
                    .pid
                    .and_then(|pid| facts.iter().find(|fact| fact.pid == pid))
                    .cloned();

                let attribution = match (&listener.pid, &process) {
                    (None, _) => Attribution::Unattributed {
                        reason: Unattributed::NoOwningProcess,
                    },
                    (Some(_), None) => Attribution::Unattributed {
                        reason: Unattributed::NoOwningProcess,
                    },
                    (Some(_), Some(facts)) => attribute(
                        facts.working_directory.as_deref().map(std::path::Path::new),
                        &roots,
                        self.state.matching,
                    ),
                };

                Service {
                    listener,
                    process,
                    attribution,
                }
            })
            .collect();

        self.state.live.record_services(services, at);
        (self.announce)();
        Ok(())
    }
}

impl ServiceObserver {
    /// Every project, with the package boundaries last observed inside it.
    ///
    /// Boundaries come from the Git observer's cached layout rather than being
    /// detected here. Both observers run on the same interval, so the answer is
    /// as fresh as anything else on screen, and a directory walk per project per
    /// port scan would double the cost of both.
    fn roots(&self) -> Result<Vec<ProjectRoot>> {
        Ok(self
            .state
            .projects()
            .list()?
            .into_iter()
            .map(|project| ProjectRoot {
                id: project.id,
                root: project.root_path,
                packages: packages_of(&self.state.live, project.id),
            })
            .collect())
    }
}

fn packages_of(live: &Live, project_id: ProjectId) -> Vec<PackageBoundary> {
    match live.layout_of(project_id) {
        Some(RepositoryLayout::MonorepoRoot { packages, .. }) => packages
            .into_iter()
            .map(|package| PackageBoundary {
                name: package.name,
                path: package.path,
            })
            .collect(),
        // A project that *is* a package has no packages of its own, and a
        // standalone project has none either. In both cases a service inside it
        // belongs to the project, with no finer boundary to report.
        _ => Vec::new(),
    }
}

fn git_root(project: &Project) -> Option<std::path::PathBuf> {
    project
        .git_root
        .as_deref()
        .map(std::path::PathBuf::from)
        .or_else(|| {
            project
                .is_git
                .then(|| std::path::PathBuf::from(&project.root_path))
        })
}

/// The matching rule, exposed so a caller can build the same roots for a
/// one-off refresh.
#[must_use]
pub const fn matching_of(state: &AppState) -> PathMatching {
    state.matching
}

/// Run both observations once, now, reporting only that something failed.
///
/// Used at start-up and by the interface's refresh action, so the first paint is
/// not an empty window waiting five seconds for the first tick.
pub fn observe_once(state: &Arc<AppState>) -> std::result::Result<(), MiraError> {
    let git = GitObserver::new(Arc::clone(state), silent()).observe();
    let services = ServiceObserver::new(Arc::clone(state), silent()).observe();

    git.and(services)
}
