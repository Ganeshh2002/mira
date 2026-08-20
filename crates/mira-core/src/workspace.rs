//! A workspace — a way of working on one project.
//!
//! The middle of Mira's spine (`information-architecture.md` §1), and the
//! distinction worth keeping sharp, because four nearby words mean four
//! different things:
//!
//! - a **project** is *where the code lives*: a directory on disk;
//! - a **workspace** is *what you are working on there*: a name a person chose,
//!   with no directory of its own and no requirement that one exist;
//! - a **package** is a boundary the *repository* declares — `apps/web` — found
//!   by reading manifests, never invented and never stored;
//! - a **session** is *one stretch of actually working*, observed by Mira rather
//!   than created by anyone.
//!
//! A workspace holds only what a person stated. Git state, listening ports and
//! running processes belong to the project underneath it and are observed live;
//! two workspaces on one project see the same observations rather than each
//! keeping a copy (`data-model.md` §1 rule 2).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ids::{ProjectId, WorkspaceId};

/// A kind of application a workspace works with.
///
/// A *kind*, not an application. "This workspace uses an editor" is something the
/// user stated and Mira stores; "the editor here is VS Code" is something Mira
/// discovers on the machine it is running on. Keeping them apart is what lets a
/// workspace move between machines and still be true — and what lets Mira say
/// "Editor · not installed" instead of quietly forgetting the association.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum AppKind {
    /// Where the code is edited.
    Editor,
    /// Where commands are run.
    Terminal,
    /// Where the running application is looked at.
    Browser,
}

impl AppKind {
    /// Every kind, in the order they are shown.
    pub const ALL: [Self; 3] = [Self::Editor, Self::Terminal, Self::Browser];

    /// The word the interface uses.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Editor => "Editor",
            Self::Terminal => "Terminal",
            Self::Browser => "Browser",
        }
    }

    /// The value stored in the database, and parsed back from it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Editor => "editor",
            Self::Terminal => "terminal",
            Self::Browser => "browser",
        }
    }

    /// The kind a stored row names, if it names one Mira knows.
    #[must_use]
    pub fn parse(stored: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == stored)
    }
}

/// One workspace, as stored and as shown.
///
/// Deliberately absent: anything observed. There is no branch here, no port, no
/// process — those belong to the project and are read live, so that opening a
/// workspace shows the truth rather than a copy of it taken at some earlier time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Workspace {
    /// Stable identity.
    pub id: WorkspaceId,
    /// The project it is a way of working on.
    pub project_id: ProjectId,
    /// What the user calls it.
    pub name: String,
    /// What it is for, if the user said. A good name usually says enough.
    pub description: Option<String>,
    /// The kinds of application the user associated with it.
    pub applications: Vec<AppKind>,
    /// When it was last opened; `None` means never.
    #[ts(type = "number | null")]
    pub last_opened_at: Option<i64>,
    /// When it was created.
    #[ts(type = "number")]
    pub created_at: i64,
    /// When any of its stored fields last changed.
    #[ts(type = "number")]
    pub updated_at: i64,
}
