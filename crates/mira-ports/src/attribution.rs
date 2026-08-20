//! Which project a listening process belongs to.
//!
//! One rule governs everything here: **never guess.** A service Mira cannot place
//! is shown as unattributed, with the reason. A service placed in the wrong
//! project is worse than one left unplaced — it is a confident statement about
//! where your work is running, and being wrong about that costs more than saying
//! nothing.
//!
//! So attribution rests on exactly one fact: the **working directory** of the
//! process that owns the socket. Not the process name — a `node` on :3000 is not
//! evidence of anything, and every project has one. Not the executable path,
//! which points at a shared interpreter rather than at your code.
//!
//! Where the platform does not expose a working directory, the answer is "cannot
//! be placed here, and here is why" (`platform-abstraction.md` §5).

use std::path::Path;

use mira_core::ProjectId;
use mira_fs::{contains, PathMatching};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One package boundary inside a project, from the monorepo detection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PackageBoundary {
    /// The package's own name.
    pub name: String,
    /// Its path relative to the project root, with `/` separators.
    pub path: String,
}

/// A project as attribution sees it: a root, and the packages inside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRoot {
    /// Which project.
    pub id: ProjectId,
    /// Its canonical absolute root.
    pub root: String,
    /// The package boundaries detected inside it, if any.
    pub packages: Vec<PackageBoundary>,
}

/// Why a service could not be placed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Unattributed {
    /// The operating system did not say which process owns the socket.
    NoOwningProcess,
    /// This platform does not expose another process's working directory.
    NoWorkingDirectory,
    /// It is running somewhere that is not inside any project Mira knows.
    OutsideEveryProject,
}

impl Unattributed {
    /// The sentence shown under the service.
    #[must_use]
    pub const fn explain(self) -> &'static str {
        match self {
            Self::NoOwningProcess => {
                "The operating system did not say which process is listening here."
            }
            Self::NoWorkingDirectory => {
                "This system does not let Mira see where that process is running from."
            }
            Self::OutsideEveryProject => "It is running outside every project you have added.",
        }
    }
}

/// Where a service belongs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum Attribution {
    /// Placed in a project, and possibly in one of its packages.
    #[serde(rename_all = "camelCase")]
    Project {
        /// Which project.
        project_id: ProjectId,
        /// The innermost package it is running in, when it is running in one.
        package: Option<PackageBoundary>,
    },

    /// Not placed, and the reason why.
    #[serde(rename_all = "camelCase")]
    Unattributed {
        /// What stopped Mira placing it.
        reason: Unattributed,
    },
}

/// Place a process by the directory it is running in.
///
/// `cwd` is `None` where the platform will not report one. Where several projects
/// contain the directory — a package added as its own project, inside the
/// monorepo that is also a project — the **innermost** wins, because that is the
/// one the person was working in.
#[must_use]
pub fn attribute(cwd: Option<&Path>, roots: &[ProjectRoot], matching: PathMatching) -> Attribution {
    let Some(cwd) = cwd else {
        return Attribution::Unattributed {
            reason: Unattributed::NoWorkingDirectory,
        };
    };

    let found = roots
        .iter()
        .filter(|project| contains(Path::new(&project.root), cwd, matching))
        // Longest root first: the innermost project is the most specific true
        // answer, and every candidate here is genuinely a container.
        .max_by_key(|project| Path::new(&project.root).components().count());

    let Some(project) = found else {
        return Attribution::Unattributed {
            reason: Unattributed::OutsideEveryProject,
        };
    };

    Attribution::Project {
        project_id: project.id,
        package: innermost_package(project, cwd, matching),
    }
}

/// The deepest declared package containing `cwd`, if any.
fn innermost_package(
    project: &ProjectRoot,
    cwd: &Path,
    matching: PathMatching,
) -> Option<PackageBoundary> {
    let root = Path::new(&project.root);

    project
        .packages
        .iter()
        .filter(|package| contains(&root.join(&package.path), cwd, matching))
        .max_by_key(|package| Path::new(&package.path).components().count())
        .cloned()
}
