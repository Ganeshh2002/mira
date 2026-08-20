//! Projects — the top of Mira's spine.
//!
//! A project is a directory the user pointed at, plus what Mira learned by looking
//! at it once: whether Git is there, and which type markers sit at its root. This
//! crate owns that lifecycle. It reaches Git through [`GitProvider`] and the
//! filesystem through `mira-fs`, so it holds no `cfg(target_os)` and needs no
//! repository on disk to test.
//!
//! **The clock is a parameter, not a dependency.** Every method that records a time
//! takes it from the caller. The application shell reads the clock once per command;
//! the domain stays a pure function of its inputs.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod markers;

use std::path::{Path, PathBuf};

use mira_core::{MiraError, Project, ProjectId, Result};
use mira_db::{NewProject, ProjectRepo};
use mira_fs::{canonical_dir, PathMatching};
use mira_git::GitProvider;

pub use markers::MARKERS;

/// What the application shell may ask about projects.
pub trait ProjectService {
    /// How many projects the user has added.
    fn count(&self) -> Result<u32>;

    /// Every project, most recently opened first.
    fn list(&self) -> Result<Vec<Project>>;

    /// One project by id.
    fn get(&self, id: ProjectId) -> Result<Project>;

    /// Register a directory the user chose.
    fn add(&self, chosen: &Path, now: i64) -> Result<Project>;

    /// Record that a project was opened, and return it.
    fn open(&self, id: ProjectId, now: i64) -> Result<Project>;

    /// Forget a project. The directory on disk is never touched.
    fn remove(&self, id: ProjectId) -> Result<()>;
}

/// The repository-backed implementation.
#[derive(Debug, Clone, Copy)]
pub struct Projects<R, G> {
    repo: R,
    git: G,
    matching: PathMatching,
}

impl<R: ProjectRepo, G: GitProvider> Projects<R, G> {
    /// Wrap a repository and a Git provider.
    ///
    /// `matching` says whether this filesystem distinguishes `Aviora` from
    /// `aviora`; it comes from `mira-platform`, because that is the only crate
    /// allowed to know which operating system this is (ADR-0005).
    pub const fn new(repo: R, git: G, matching: PathMatching) -> Self {
        Self {
            repo,
            git,
            matching,
        }
    }
}

impl<R: ProjectRepo, G: GitProvider> ProjectService for Projects<R, G> {
    fn count(&self) -> Result<u32> {
        self.repo.count()
    }

    fn list(&self) -> Result<Vec<Project>> {
        self.repo.list()
    }

    fn get(&self, id: ProjectId) -> Result<Project> {
        self.repo.get(id)
    }

    fn add(&self, chosen: &Path, now: i64) -> Result<Project> {
        let root = canonical_dir(chosen)?;

        // Ask before inserting, so a repeat add explains itself by naming the
        // project already there rather than surfacing a UNIQUE constraint.
        if let Some(existing) = self.existing_at(&root)? {
            return Err(MiraError::invalid(
                "path",
                format!(
                    "{} is already open as \"{}\".",
                    root.display(),
                    existing.name
                ),
            ));
        }

        // libgit2 reports a worktree with a trailing separator. Paths compare by
        // components, so it changes no behaviour — but it is shown to a person,
        // and re-collecting the components is what drops it.
        let worktree: Option<PathBuf> = self
            .git
            .discover(&root)
            .map(|found| found.components().collect());
        let git_root = worktree
            .as_deref()
            .filter(|found| !self.matching.same_path(found, &root))
            .map(|found| found.display().to_string());

        self.repo.insert(
            &NewProject {
                name: name_for(&root),
                root_path: root.display().to_string(),
                is_git: worktree.is_some(),
                git_root,
                markers: markers::detect(&root),
            },
            now,
        )
    }

    fn open(&self, id: ProjectId, now: i64) -> Result<Project> {
        self.repo.touch_opened(id, now)?;
        self.repo.get(id)
    }

    fn remove(&self, id: ProjectId) -> Result<()> {
        self.repo.remove(id)
    }
}

impl<R: ProjectRepo, G: GitProvider> Projects<R, G> {
    /// The project registered for this directory, under this filesystem's rules.
    ///
    /// The `root_path UNIQUE` index compares bytes, which is the right answer on
    /// Linux and the wrong one on macOS and Windows, where `~/Code/Aviora` and
    /// `~/code/aviora` are one directory. The scan closes that gap.
    fn existing_at(&self, root: &Path) -> Result<Option<Project>> {
        if self.matching == PathMatching::CaseSensitive {
            return self.repo.find_by_root(&root.display().to_string());
        }

        Ok(self
            .repo
            .list()?
            .into_iter()
            .find(|project| self.matching.same_path(Path::new(&project.root_path), root)))
    }
}

/// What to call a project, inferred from its directory (`prd.md` feature 1).
fn name_for(root: &Path) -> String {
    root.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| root.display().to_string())
}
