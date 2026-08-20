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

use mira_core::{AppKind, MiraError, ProjectId, Result, Workspace, WorkspaceId};
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
