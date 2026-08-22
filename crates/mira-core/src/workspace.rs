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

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
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

/// The identity of one application in Mira's catalogue.
///
/// **Not a path, not a program name, not a command.** It is a slug naming a row
/// in a table compiled into the binary — `vscode`, `iterm`, `ghostty` — and the
/// only thing it can be turned into is that row. A caller who invents one gets a
/// refusal rather than a launch, because there is nothing for an unknown id to
/// resolve to ([ADR-0019](../../../docs/adr/0019-application-preferences.md)).
///
/// Validated as it deserialises, like a commit id: lower-case ASCII letters,
/// digits and hyphens, one to [`LONGEST_APP_ID`] of them. That is defence in
/// depth — the catalogue lookup is the real wall — but a type that cannot hold a
/// path is a type nobody has to check for one.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, TS)]
#[ts(export)]
pub struct AppId(#[ts(type = "string")] String);

/// The longest an application id may be.
pub const LONGEST_APP_ID: usize = 32;

impl AppId {
    /// The id as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for AppId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for AppId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::try_from(String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

impl std::fmt::Display for AppId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for AppId {
    type Error = MalformedAppId;

    fn try_from(raw: String) -> Result<Self, Self::Error> {
        if raw.is_empty()
            || raw.len() > LONGEST_APP_ID
            || !raw
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Err(MalformedAppId);
        }

        Ok(Self(raw))
    }
}

impl std::str::FromStr for AppId {
    type Err = MalformedAppId;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::try_from(raw.to_owned())
    }
}

/// What arrived was not an application id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MalformedAppId;

impl std::fmt::Display for MalformedAppId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "an application id is 1 to {LONGEST_APP_ID} lower-case letters, digits or hyphens"
        )
    }
}

impl std::error::Error for MalformedAppId {}

/// Which application a workspace uses for one kind.
///
/// Stored per workspace, so choosing Zed here leaves every other workspace on
/// whatever it was using. Absent means *automatic*: Mira takes the first entry in
/// its own list that this machine has, which is what every workspace did before
/// anybody chose anything.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppPreference {
    /// Which kind the choice is for.
    pub kind: AppKind,
    /// The catalogue row chosen.
    pub application: AppId,
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
    /// Which application each kind uses here, where the user chose one.
    ///
    /// Per workspace by construction: the rows are keyed by workspace id, so a
    /// choice made here is invisible to every other workspace on the same
    /// project. A kind absent from this list is on automatic.
    pub preferences: Vec<AppPreference>,
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
