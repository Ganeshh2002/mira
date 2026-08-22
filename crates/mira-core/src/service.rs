//! Services a workspace watches.
//!
//! A workspace is a way of working on a project, and part of what somebody
//! working means is *these are the things that should be up*. Slice 2 observes
//! every listening socket on the machine and slice 4 attributes them to
//! projects; neither says which ones a person cares about, so a workspace on a
//! monorepo shows twelve services when two of them are the work.
//!
//! This module is the narrowing. It holds three things and the relationship
//! between them:
//!
//! - a [`Port`], validated once so nothing else has to;
//! - a [`WatchedService`] — what a workspace *stated*: one port, and when it was
//!   added;
//! - a [`ServiceState`] — what Mira *observed*, resolved fresh on every read.
//!
//! The split is `data-model.md` §1 rule 2 again. The stored row has a port and
//! nothing else: no process name, no pid, no address, no label, no command. All
//! of those are observation, they belong to the project underneath, and they are
//! read live — which is why a service that has stopped can be shown honestly as
//! a port that is not answering, rather than as a stale copy of what used to be
//! there.
//!
//! Nothing here starts, stops, or signals anything. Resolving is a pure function
//! of two lists ([`resolve`]), so watching a service creates no observer, no
//! timer and no work per workspace (ADR-0011, ADR-0020).

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use ts_rs::TS;

use crate::ids::{ProjectId, WorkspaceId, WorkspaceServiceId};

// ── A port ───────────────────────────────────────────────────────────────────

/// The lowest port a socket can listen on. Port 0 means "any", which is a
/// request rather than an address, and is never something to watch.
const LOWEST_PORT: u16 = 1;

/// A TCP port Mira will talk about.
///
/// Validated where it deserialises, so `0` and anything outside `u16` fail on
/// the wire rather than three layers down — and every other layer may assume a
/// `Port` is a port. This is the "validate port constraints centrally" rule with
/// one place to look.
///
/// Note what this type is *not*: a way for the interface to name one. Nothing
/// the frontend sends carries a `Port`; a workspace names a service by an
/// ordinal in a list Mira offered, or by the row id Mira issued when it was
/// added. The type exists so that the value Mira reads from its own database is
/// checked before it is used (ADR-0020).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, TS)]
#[ts(export)]
pub struct Port(#[ts(type = "number")] u16);

impl Serialize for Port {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u16(self.0)
    }
}

impl<'de> Deserialize<'de> for Port {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::try_from(u16::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

impl Port {
    /// The number, for building an address or writing a row.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for Port {
    type Error = NotAPort;

    fn try_from(raw: u16) -> Result<Self, Self::Error> {
        if raw < LOWEST_PORT {
            return Err(NotAPort);
        }
        Ok(Self(raw))
    }
}

impl TryFrom<i64> for Port {
    type Error = NotAPort;

    /// From a database column, where the value is an `INTEGER` of any width.
    fn try_from(raw: i64) -> Result<Self, Self::Error> {
        u16::try_from(raw)
            .map_err(|_| NotAPort)
            .and_then(Self::try_from)
    }
}

impl std::fmt::Display for Port {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// What arrived was not a port.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotAPort;

impl std::fmt::Display for NotAPort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("A port is a number from 1 to 65535.")
    }
}

impl std::error::Error for NotAPort {}

// ── What a workspace stated ──────────────────────────────────────────────────

/// One service a workspace watches, as stored.
///
/// Four fields, and the absences are the design. There is no label, no process
/// name, no address and no command — a stored row is a *port this workspace
/// cares about*, and everything else about it is observation that belongs to the
/// project and is read fresh (`data-model.md` §1 rule 2).
///
/// The consequence is visible in the interface and is meant to be: a watched
/// service that has stopped is shown as its port and nothing more, because
/// everything Mira could have added would have been a memory of something that
/// is no longer true.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WatchedService {
    /// Stable identity, issued by Mira when the service was added. This is what
    /// the interface hands back to open or to stop watching — never a port.
    pub id: WorkspaceServiceId,
    /// The workspace that watches it.
    pub workspace_id: WorkspaceId,
    /// The port, as Mira read it from its own observation at the time.
    pub port: Port,
    /// When it was added.
    #[ts(type = "number")]
    pub added_at: i64,
}

/// Where a watched service stands, right now.
///
/// Five states rather than two, because "not running" and "Mira has not looked"
/// are different things to be told, and because a port that something *else*
/// has taken is neither running nor free. Collapsing them would mean an
/// interface that says "not running" when the truth is "cannot say" — the same
/// failure as reporting "no results" for a search that ran out of budget
/// (ADR-0018).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum ServiceState {
    /// Listening now, from inside this workspace's project.
    #[serde(rename_all = "camelCase")]
    Running {
        /// The address it is bound to, as the platform reported it.
        address: String,
        /// What the owning process is called, where the platform says.
        process: Option<String>,
        /// The owning process id, where the platform says.
        #[ts(type = "number | null")]
        pid: Option<u32>,
    },

    /// Mira looked, and nothing is listening on this port.
    ///
    /// The expected-but-not-running state, and deliberately not an error: a
    /// service you have not started yet is the normal condition of a morning.
    NotRunning,

    /// Something is listening here, and it is not this project's.
    ///
    /// Never silently reported as running. A watched service resolves to the
    /// thing that was watched or to a state saying why not; substituting an
    /// unrelated process because it happens to hold the same number is the
    /// failure this variant exists to prevent.
    #[serde(rename_all = "camelCase")]
    Taken {
        /// What the owning process is called, where the platform says.
        process: Option<String>,
    },

    /// Mira has not read the socket table yet, so it cannot say.
    NeverObserved,

    /// Mira tried to read the socket table and could not.
    #[serde(rename_all = "camelCase")]
    Unreadable {
        /// What the platform said, shown as-is.
        reason: String,
    },
}

impl ServiceState {
    /// Whether this is a service somebody could open in a browser right now.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        matches!(self, Self::Running { .. })
    }
}

/// A watched service and where it stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WorkspaceService {
    /// What the workspace stated.
    pub watched: WatchedService,
    /// What Mira observed.
    pub state: ServiceState,
}

// ── What Mira observed ───────────────────────────────────────────────────────

/// One listening socket, reduced to what resolving needs.
///
/// Not a wire type and not stored: the observers build these from a live
/// snapshot on each read, so this module needs no dependency on the crates that
/// do the observing and stays a pure function of its inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listening {
    /// The port.
    pub port: Port,
    /// The project Mira attributed it to, where it could attribute one.
    pub project_id: Option<ProjectId>,
    /// The address it is bound to.
    pub address: String,
    /// The owning process's name.
    pub process: Option<String>,
    /// The owning process id.
    pub pid: Option<u32>,
}

/// What the last look at the socket table produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Observed<'a> {
    /// Never read.
    NotYet,
    /// Read, and this is what was listening.
    Seen(&'a [Listening]),
    /// Attempted and failed, with what the platform said.
    Failed(&'a str),
}

/// Resolve what a workspace watches against what Mira last saw.
///
/// Pure, and the only place a stored row becomes a state. Called on every read
/// rather than kept: a workspace watching a service creates no observer and no
/// timer, and fifty workspaces resolving twenty services each against four
/// thousand listening sockets cost 1.9 ms in total — so there is nothing here
/// worth caching, and therefore no clock to own (ADR-0011, ADR-0020).
///
/// `project_id` is the watching workspace's project, and it is what makes
/// [`ServiceState::Taken`] possible: a listener on the right port but the wrong
/// project is not this workspace's service.
#[must_use]
pub fn resolve(
    watched: &[WatchedService],
    project_id: ProjectId,
    observed: Observed<'_>,
) -> Vec<WorkspaceService> {
    watched
        .iter()
        .map(|watched| WorkspaceService {
            watched: *watched,
            state: state_of(*watched, project_id, observed),
        })
        .collect()
}

/// Where one watched service stands.
///
/// A linear scan of the observed listeners, and it is a scan because it was
/// measured against the obvious alternative rather than assumed. Building one
/// map of the listeners and looking each watched service up is O(watched +
/// listening) and *looks* like the right answer; it loses, because the map has
/// to be built over every listening socket on the machine whether the workspace
/// watches one service or twenty, and a hash of a `u16` costs more than
/// comparing one.
///
/// | watched | listening | scan | index |
/// |---|---|---|---|
/// | 4 | 64 | **0.069 µs** | 0.336 µs |
/// | 4 | 512 | **0.565 µs** | 2.666 µs |
/// | 20 | 512 | 2.517 µs | **2.394 µs** |
/// | 20 | 4 096 | 37.143 µs | **19.264 µs** |
///
/// The crossover is around twenty watched services and five hundred listening
/// sockets, which is not a machine anybody develops on. Below it the scan is
/// three to five times cheaper, and above it both are still microseconds
/// (ADR-0020).
fn state_of(
    watched: WatchedService,
    project_id: ProjectId,
    observed: Observed<'_>,
) -> ServiceState {
    let listeners = match observed {
        Observed::NotYet => return ServiceState::NeverObserved,
        Observed::Failed(reason) => {
            return ServiceState::Unreadable {
                reason: reason.to_owned(),
            }
        }
        Observed::Seen(listeners) => listeners,
    };

    let Some(listening) = listeners
        .iter()
        .find(|listening| listening.port == watched.port)
    else {
        return ServiceState::NotRunning;
    };

    // The attribution check. A listener Mira could not place is not claimed for
    // this project either — "something is here and Mira cannot tell whose" is
    // `Taken`, not `Running`, because opening it would be opening whatever
    // happened to be on the number.
    if listening.project_id != Some(project_id) {
        return ServiceState::Taken {
            process: listening.process.clone(),
        };
    }

    ServiceState::Running {
        address: listening.address.clone(),
        process: listening.process.clone(),
        pid: listening.pid,
    }
}
