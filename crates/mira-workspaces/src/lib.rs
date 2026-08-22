//! Workspaces — a named way of working on one project.
//!
//! `information-architecture.md` §1 puts this in the middle of Mira's spine, and
//! the distinction is worth restating because three nearby words mean three
//! different things:
//!
//! - a **project** is where the code lives — a directory, stated by the user;
//! - a **workspace** is what they are working on there — a name, with no
//!   directory of its own and no requirement that one exist;
//! - a **package** is a boundary the repository declares, found by reading its
//!   manifests and never stored.
//!
//! Nothing observed lives here. Two workspaces on one project see the *same*
//! Git state and the same services, because those belong to the project
//! underneath and are read live (`data-model.md` §1 rule 2).

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::path::PathBuf;

use mira_core::action::ActionId;
use mira_core::service::{Listening, Observed, Port, WatchedService, WorkspaceService as Watched};
use mira_core::{
    resolve, AppId, AppKind, MiraError, ProjectId, Result, Workspace, WorkspaceId,
    WorkspaceServiceId,
};
use mira_db::{NewWorkspace, ProjectRepo, WorkspaceRepo};

/// What the application shell may ask about workspaces.
pub trait WorkspaceService {
    /// How many workspaces exist, across every project.
    fn count(&self) -> Result<u32>;

    /// One project's workspaces, most recently opened first.
    fn list_for(&self, project_id: ProjectId) -> Result<Vec<Workspace>>;

    /// One workspace by id.
    fn get(&self, id: WorkspaceId) -> Result<Workspace>;

    /// Create a workspace against a project.
    fn create(
        &self,
        project_id: ProjectId,
        name: &str,
        description: Option<&str>,
        now: i64,
    ) -> Result<Workspace>;

    /// Change a workspace's name and description.
    fn rename(
        &self,
        id: WorkspaceId,
        name: &str,
        description: Option<&str>,
        now: i64,
    ) -> Result<Workspace>;

    /// Record that a workspace was opened, and return it.
    fn open(&self, id: WorkspaceId, now: i64) -> Result<Workspace>;

    /// Forget a workspace. Its project is untouched.
    fn remove(&self, id: WorkspaceId) -> Result<()>;

    /// Replace the kinds of application this workspace works with.
    fn set_applications(&self, id: WorkspaceId, kinds: &[AppKind], now: i64) -> Result<Workspace>;

    /// Choose which application this workspace uses for one kind.
    ///
    /// `None` puts that kind back on automatic. The id is **not** checked here:
    /// whether an id names something Mira can start is a question about the
    /// machine, and this crate does not know there is one. The command layer
    /// resolves it against the platform catalogue before it ever arrives
    /// ([ADR-0019](../../../docs/adr/0019-application-preferences.md)).
    fn prefer(
        &self,
        id: WorkspaceId,
        kind: AppKind,
        application: Option<&AppId>,
        now: i64,
    ) -> Result<Workspace>;

    /// The directory this workspace's applications open at.
    ///
    /// A workspace has no directory of its own, so this is its project's
    /// canonical root — resolved here, in Rust, from a row the user registered
    /// through the native picker. It is the *only* way a path reaches the
    /// launcher, which is what makes "the interface cannot name a directory"
    /// hold all the way down (`security-and-privacy.md` §5).
    ///
    /// # Errors
    ///
    /// [`MiraError::NotFound`] if the workspace is gone, its project is gone, or
    /// the folder has been moved or deleted — the last of which leaves the
    /// workspace entirely intact (`prd.md` FR-1.5).
    fn working_directory(&self, id: WorkspaceId) -> Result<PathBuf>;

    /// What this workspace watches, resolved against what Mira last observed.
    ///
    /// Two reads and a pure function: the stored rows, and the listeners the
    /// caller already has. Nothing here starts an observer or a timer, so a
    /// workspace that watches five services costs the same as one that watches
    /// none the moment nobody is looking at it (ADR-0011).
    ///
    /// # Errors
    ///
    /// [`MiraError::NotFound`] if the workspace is gone.
    fn services(&self, id: WorkspaceId, listening: &[Listening]) -> Result<Vec<Watched>>;

    /// The same, when the socket table could not be read.
    ///
    /// A separate call rather than an `Option` parameter, because "Mira has not
    /// looked" and "Mira looked and failed" are two different sentences a person
    /// needs to be told, and neither of them is "not running".
    ///
    /// # Errors
    ///
    /// [`MiraError::NotFound`] if the workspace is gone.
    fn services_unobserved(&self, id: WorkspaceId, observed: Observed<'_>) -> Result<Vec<Watched>>;

    /// Start watching a port this workspace's project was observed serving.
    ///
    /// The port is not a parameter the caller composes — it comes from `at`, a
    /// position in `offered`, which is the list Mira produced from its own
    /// observation. An ordinal past the end is a stale selection, not an attempt
    /// at anything.
    ///
    /// # Errors
    ///
    /// [`MiraError::NotFound`] if the workspace is gone or `at` names nothing;
    /// [`MiraError::Invalid`] if the workspace already watches that port, or if
    /// the offer at `at` belongs to a different project.
    fn watch(
        &self,
        id: WorkspaceId,
        at: u32,
        offered: &[Listening],
        now: i64,
    ) -> Result<WatchedService>;

    /// Stop watching one service.
    ///
    /// # Errors
    ///
    /// [`MiraError::NotFound`] if the workspace is gone, or if the service is
    /// not one this workspace watches — including when it is a sibling's.
    fn forget(&self, id: WorkspaceId, service: WorkspaceServiceId) -> Result<()>;

    /// The port behind one of this workspace's watched services.
    ///
    /// The one place a row id becomes a number, and it is here rather than in
    /// the command layer so that the workspace check and the lookup cannot be
    /// separated.
    ///
    /// # Errors
    ///
    /// [`MiraError::NotFound`] if the workspace is gone or does not watch it.
    fn port_of(&self, id: WorkspaceId, service: WorkspaceServiceId) -> Result<Port>;

    /// The catalogue identities this workspace has, in catalogue order.
    ///
    /// Returned as stored, including any id this version of Mira has no row
    /// for. Resolving one to a state — and refusing an unknown one — is the
    /// command layer's job, because this crate does not know a catalogue exists.
    ///
    /// # Errors
    ///
    /// [`MiraError::NotFound`] if the workspace is gone.
    fn actions(&self, id: WorkspaceId) -> Result<Vec<ActionId>>;

    /// Give this workspace one action, or take it away.
    ///
    /// The caller has already resolved `action` against the compiled catalogue;
    /// an id that names nothing never reaches here.
    ///
    /// # Errors
    ///
    /// [`MiraError::NotFound`] if the workspace is gone.
    fn set_action(
        &self,
        id: WorkspaceId,
        action: &ActionId,
        wanted: bool,
        now: i64,
    ) -> Result<Vec<ActionId>>;
}

/// The repository-backed implementation.
#[derive(Debug, Clone, Copy)]
pub struct Workspaces<R> {
    repo: R,
}

impl<R> Workspaces<R> {
    /// Wrap a repository.
    pub const fn new(repo: R) -> Self {
        Self { repo }
    }
}

impl<R: WorkspaceRepo + ProjectRepo> WorkspaceService for Workspaces<R> {
    fn count(&self) -> Result<u32> {
        WorkspaceRepo::count(&self.repo)
    }

    fn list_for(&self, project_id: ProjectId) -> Result<Vec<Workspace>> {
        self.repo.list_for(project_id)
    }

    fn get(&self, id: WorkspaceId) -> Result<Workspace> {
        self.repo.get_workspace(id)
    }

    fn create(
        &self,
        project_id: ProjectId,
        name: &str,
        description: Option<&str>,
        now: i64,
    ) -> Result<Workspace> {
        let name = named(name)?;
        // Reading the project first turns "no such project" into that sentence,
        // rather than into a foreign-key violation from two layers down.
        self.repo.get(project_id)?;
        self.refuse_duplicate(project_id, &name, None)?;

        self.repo.create(
            &NewWorkspace {
                project_id,
                name,
                description: described(description),
            },
            now,
        )
    }

    fn rename(
        &self,
        id: WorkspaceId,
        name: &str,
        description: Option<&str>,
        now: i64,
    ) -> Result<Workspace> {
        let name = named(name)?;
        let existing = self.repo.get_workspace(id)?;
        self.refuse_duplicate(existing.project_id, &name, Some(id))?;

        self.repo
            .rename_workspace(id, &name, described(description).as_deref(), now)?;
        self.repo.get_workspace(id)
    }

    fn open(&self, id: WorkspaceId, now: i64) -> Result<Workspace> {
        self.repo.touch_workspace(id, now)?;
        self.repo.get_workspace(id)
    }

    fn remove(&self, id: WorkspaceId) -> Result<()> {
        self.repo.remove_workspace(id)
    }

    fn set_applications(&self, id: WorkspaceId, kinds: &[AppKind], now: i64) -> Result<Workspace> {
        // Read first so a workspace that is gone says so, rather than a delete
        // and three inserts quietly affecting nothing.
        self.repo.get_workspace(id)?;
        self.repo.set_workspace_applications(id, kinds, now)?;
        self.repo.get_workspace(id)
    }

    fn prefer(
        &self,
        id: WorkspaceId,
        kind: AppKind,
        application: Option<&AppId>,
        now: i64,
    ) -> Result<Workspace> {
        self.repo
            .set_workspace_preference(id, kind, application, now)?;
        self.repo.get_workspace(id)
    }

    fn services(&self, id: WorkspaceId, listening: &[Listening]) -> Result<Vec<Watched>> {
        self.services_unobserved(id, Observed::Seen(listening))
    }

    fn services_unobserved(&self, id: WorkspaceId, observed: Observed<'_>) -> Result<Vec<Watched>> {
        // The workspace is read first so that a workspace that is gone says so,
        // rather than resolving an empty list into "nothing is running".
        let workspace = self.repo.get_workspace(id)?;
        let watched = self.repo.workspace_services(id)?;

        Ok(resolve(&watched, workspace.project_id, observed))
    }

    fn watch(
        &self,
        id: WorkspaceId,
        at: u32,
        offered: &[Listening],
        now: i64,
    ) -> Result<WatchedService> {
        let workspace = self.repo.get_workspace(id)?;

        let offer = usize::try_from(at)
            .ok()
            .and_then(|at| offered.get(at))
            .ok_or_else(|| MiraError::NotFound {
                what: "That service".to_owned(),
            })?;

        // The offer must belong to this workspace's project. The caller builds
        // `offered` from the project's own observations, so this cannot fail
        // through the interface — it fails if a future caller ever passes the
        // machine's whole list, which is the mistake worth making impossible
        // rather than merely unlikely.
        if offer.project_id != Some(workspace.project_id) {
            return Err(MiraError::invalid(
                "service",
                "That service is not one this workspace's project is running.".to_owned(),
            ));
        }

        self.repo.watch_service(id, offer.port, now)
    }

    fn forget(&self, id: WorkspaceId, service: WorkspaceServiceId) -> Result<()> {
        self.repo.get_workspace(id)?;
        self.repo.forget_service(id, service)
    }

    fn port_of(&self, id: WorkspaceId, service: WorkspaceServiceId) -> Result<Port> {
        self.repo.get_workspace(id)?;
        Ok(self.repo.workspace_service(id, service)?.port)
    }

    fn actions(&self, id: WorkspaceId) -> Result<Vec<ActionId>> {
        // Read the workspace first so that one that is gone says so, rather than
        // resolving an empty list into "this workspace has no actions".
        self.repo.get_workspace(id)?;
        self.repo.workspace_actions(id)
    }

    fn set_action(
        &self,
        id: WorkspaceId,
        action: &ActionId,
        wanted: bool,
        now: i64,
    ) -> Result<Vec<ActionId>> {
        self.repo.get_workspace(id)?;
        self.repo.set_workspace_action(id, action, wanted, now)?;
        self.repo.workspace_actions(id)
    }

    fn working_directory(&self, id: WorkspaceId) -> Result<PathBuf> {
        let workspace = self.repo.get_workspace(id)?;
        let project = self.repo.get(workspace.project_id)?;

        // `canonical_dir` is the same resolution a project went through when it
        // was added, so a moved folder fails here exactly as it fails there.
        // Its `NotFound` names the path; this names the project, because that is
        // the thing the person recognises.
        mira_fs::canonical_dir(std::path::Path::new(&project.root_path)).map_err(
            |error| match error {
                MiraError::NotFound { .. } | MiraError::PermissionDenied { .. } => {
                    MiraError::NotFound {
                        what: format!("The folder for \"{}\"", project.name),
                    }
                }
                other => other,
            },
        )
    }
}

impl<R: WorkspaceRepo> Workspaces<R> {
    /// Refuse a name a sibling already uses, naming the sibling.
    ///
    /// Asked before the insert so the message is about workspaces rather than
    /// about a UNIQUE index — and `except` lets a workspace keep its own name
    /// while its description changes.
    fn refuse_duplicate(
        &self,
        project_id: ProjectId,
        name: &str,
        except: Option<WorkspaceId>,
    ) -> Result<()> {
        let clash = self
            .repo
            .list_for(project_id)?
            .into_iter()
            .find(|workspace| {
                workspace.name.eq_ignore_ascii_case(name) && Some(workspace.id) != except
            });

        match clash {
            Some(workspace) => Err(MiraError::invalid(
                "name",
                format!(
                    "This project already has a workspace called \"{}\".",
                    workspace.name
                ),
            )),
            None => Ok(()),
        }
    }
}

/// A name with the whitespace taken off, or a refusal.
fn named(name: &str) -> Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(MiraError::invalid(
            "name",
            "A workspace needs a name — something you would recognise it by.",
        ));
    }
    Ok(trimmed.to_owned())
}

/// A description, or nothing. Whitespace is nothing.
fn described(description: Option<&str>) -> Option<String> {
    description
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}
