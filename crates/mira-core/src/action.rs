//! What a workspace can be asked to do.
//!
//! A workspace already says *where the code is* (its project), *which
//! applications it works with* ([ADR-0019](../../../docs/adr/0019-application-preferences.md))
//! and *which services matter* ([ADR-0020](../../../docs/adr/0020-workspace-services.md)).
//! This is the fourth thing: the handful of actions that belong to that working
//! context, gathered in one place instead of scattered across the surface.
//!
//! # What a workspace action is not
//!
//! It is **not a command**. There is no program here, no argument list, no shell
//! string, no working directory, no template and no text of any kind that gets
//! run. The word "command" does not appear in this module, and the `commands`
//! table `0001_init.sql` created — which has a `program TEXT NOT NULL` column —
//! stays empty, with a guard test asserting that nothing reads or writes it.
//!
//! What an action *is*: one row of a **catalogue compiled into the binary**, each
//! row naming an [`Effect`] that is one of Mira's existing typed seams. A
//! workspace stores the catalogue's own identity and nothing else. Adding an
//! action to the catalogue is a code change; there is no path by which a person,
//! a page, a project directory or a database file can introduce one
//! ([ADR-0021](../../../docs/adr/0021-workspace-actions.md)).
//!
//! The consequence worth stating plainly: **this feature adds no new way for
//! Mira to affect the machine.** Every effect below already had a button
//! somewhere. What is new is that a workspace can say which ones are its own.

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use ts_rs::TS;

use crate::workspace::AppKind;

/// The longest a catalogue identity may be.
pub const LONGEST_ACTION_ID: usize = 32;

/// How a workspace names one action.
///
/// The same shape as an application's catalogue id: lowercase ASCII, digits and
/// hyphens, one to thirty-two bytes. Validated where it deserialises, so a slash,
/// a space, a quote, a semicolon, a newline or a null cannot be inside one —
/// which means a stored action id cannot be a path, a program or anything with
/// syntax in it, whatever else goes wrong.
///
/// Validation is only half of it, and the smaller half. `open-editor` is a
/// perfectly well-formed id and so is `rm-rf`; the difference is that one names a
/// row in [`CATALOGUE`] and the other names nothing at all. [`find`] is the only
/// way an id becomes anything.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, TS)]
#[ts(export)]
pub struct ActionId(#[ts(type = "string")] String);

impl Serialize for ActionId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ActionId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::try_from(String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

impl ActionId {
    /// The id as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ActionId {
    type Error = MalformedActionId;

    fn try_from(raw: String) -> Result<Self, Self::Error> {
        let usable = !raw.is_empty()
            && raw.len() <= LONGEST_ACTION_ID
            && raw
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');

        if usable {
            Ok(Self(raw))
        } else {
            Err(MalformedActionId)
        }
    }
}

impl std::str::FromStr for ActionId {
    type Err = MalformedActionId;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::try_from(raw.to_owned())
    }
}

impl std::fmt::Display for ActionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// What arrived was not an action id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MalformedActionId;

impl std::fmt::Display for MalformedActionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "An action is named by a catalogue id: lowercase letters, digits and \
             hyphens, up to {LONGEST_ACTION_ID} characters."
        )
    }
}

impl std::error::Error for MalformedActionId {}

// ── The catalogue ────────────────────────────────────────────────────────────

/// What performing an action does.
///
/// **This enum is the whole privilege surface of the feature**, and it is
/// deliberately short enough to read in one go. Every variant is an existing
/// typed seam that already had a button somewhere in Mira; none of them takes a
/// parameter that the interface supplies, and none of them writes to a
/// repository, deletes anything, signals a process, or runs a program.
///
/// The absences are the design and a guard test enumerates them: there is no
/// `Run`, no `Stop`, no `Kill`, no `Script`, no `Shell`, no `Exec`. Adding one
/// would be a decision with its own ADR and its own confirmation flow, not a
/// variant somebody slipped in — which is why the variant list is asserted
/// exactly rather than merely scanned for bad words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "does", rename_all = "camelCase")]
#[ts(export)]
pub enum Effect {
    /// Open the project's own root directory in an application of this kind.
    ///
    /// The directory is resolved in Rust from the project row; the application
    /// is the one this workspace chose, or the first one Mira finds
    /// (ADR-0013, ADR-0019).
    #[serde(rename_all = "camelCase")]
    OpenIn {
        /// Editor, terminal or browser.
        kind: AppKind,
    },

    /// Show the project's folder where the file manager would show it.
    RevealProject,

    /// Open this workspace's running service in the browser.
    ///
    /// Only when **exactly one** is running. Two running services make the
    /// action ambiguous, and an ambiguous action is refused rather than guessed
    /// at (ADR-0021).
    OpenService,

    /// Record that this workspace is the one being worked in.
    ///
    /// Records *when*, and nothing else — the same thing selecting it does, and
    /// deliberately not more (ADR-0012).
    MarkOpened,

    /// Read the project's Git state and the machine's ports again, now.
    ///
    /// The refresh that already exists, asked for from here. It starts no timer
    /// and creates no observer: it makes the one scheduler take its reading
    /// early (ADR-0011).
    Observe,
}

/// One row of the catalogue.
///
/// Static, because the catalogue is compiled in. A row has no fields a program,
/// a path or an argument could be written into, and a guard test asserts that as
/// well as asserting positively what *is* here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Action {
    /// How a workspace names it. Stable, and never reused for something else.
    pub id: &'static str,
    /// What the interface calls it.
    pub label: &'static str,
    /// **What it will actually do**, in a sentence a person can check.
    ///
    /// Shown beside the action rather than hidden behind it, because the whole
    /// safety story of this feature is that you can see what an action is before
    /// you press it — and because an action list nobody can read is how a
    /// convenience becomes a hazard.
    pub describes: &'static str,
    /// The icon beside the label. Never instead of it.
    pub icon: &'static str,
    /// What it does.
    pub effect: Effect,
}

/// Every action Mira knows, in the order they are shown.
///
/// **Intentionally small.** Six rows, each an existing seam. The point of this
/// slice is the architecture — a compiled catalogue, an identity, a refusal for
/// anything else — not the breadth of the list. Growing it is a row here and
/// nothing else, which is what makes it safe to grow slowly.
pub const CATALOGUE: [Action; 6] = [
    Action {
        id: "open-editor",
        label: "Open in the editor",
        describes: "Opens this project's folder in the editor this workspace uses.",
        icon: "editor",
        effect: Effect::OpenIn {
            kind: AppKind::Editor,
        },
    },
    Action {
        id: "open-terminal",
        label: "Open a terminal here",
        describes: "Opens a terminal at this project's folder. Nothing is typed into it.",
        icon: "terminal",
        effect: Effect::OpenIn {
            kind: AppKind::Terminal,
        },
    },
    Action {
        id: "open-service",
        label: "Open the running service",
        describes: "Opens this workspace's running service in the browser, when exactly one is up.",
        icon: "service",
        effect: Effect::OpenService,
    },
    Action {
        id: "reveal-project",
        label: "Show the project folder",
        describes: "Shows this project's folder where your file manager would show it.",
        icon: "folder",
        effect: Effect::RevealProject,
    },
    Action {
        id: "mark-opened",
        label: "Mark as opened",
        describes: "Records that you are working here. Nothing is started and nothing is restored.",
        icon: "clock",
        effect: Effect::MarkOpened,
    },
    Action {
        id: "refresh",
        label: "Read everything again",
        describes: "Reads this project's Git state and the machine's ports now, instead of at the next tick.",
        icon: "refresh",
        effect: Effect::Observe,
    },
];

/// The catalogue row `id` names, if there is one.
///
/// **The only way an id becomes anything.** An id that names no row resolves to
/// nothing, which is what makes an invented one — or one left behind by a
/// version of Mira that had it — a refusal rather than a different action.
#[must_use]
pub fn find(id: &ActionId) -> Option<&'static Action> {
    CATALOGUE.iter().find(|action| action.id == id.as_str())
}

// ── Resolving one against this machine ───────────────────────────────────────

/// What this machine and this workspace can currently support.
///
/// Gathered **once per request** by the caller and handed here, rather than
/// asked per action. That ordering was measured rather than assumed: probing the
/// machine for what it can open is three orders of magnitude more expensive than
/// deciding whether an action is available, so asking once and deciding six
/// times is the whole difference (ADR-0021).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Support {
    /// The kinds this machine can open a directory in, with what each is called.
    pub openable: Vec<(AppKind, String)>,
    /// Whether the project's folder is still on disk.
    pub folder_exists: bool,
    /// Whether the desktop's file manager can be reached, and why not.
    pub reveal: Option<String>,
    /// This workspace's running services, named as the interface would name them.
    pub running: Vec<String>,
}

/// Where one of a workspace's actions stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum ActionState {
    /// It can be performed now.
    #[serde(rename_all = "camelCase")]
    Ready {
        /// What it will reach, when that is worth naming: the application, the
        /// service. Shown so the button is never a surprise.
        detail: Option<String>,
    },

    /// Mira knows this action and cannot perform it here.
    ///
    /// Rendered as a sentence rather than a greyed-out button
    /// (`information-architecture.md` §5). An action that would be ambiguous —
    /// two services running, and no way to know which one was meant — is
    /// unavailable for that reason, which is how "make ambiguous actions
    /// impossible" is spelled here.
    #[serde(rename_all = "camelCase")]
    Unavailable {
        /// Why, in words.
        reason: String,
    },

    /// A stored id this version of Mira has no row for.
    ///
    /// Never resolved to a different action. A workspace that was given an
    /// action a later Mira removed says so and offers to forget it; substituting
    /// the nearest match would be Mira doing something nobody asked for.
    Unknown,
}

impl ActionState {
    /// Whether performing it would do anything.
    #[must_use]
    pub const fn is_ready(&self) -> bool {
        matches!(self, Self::Ready { .. })
    }
}

/// One of a workspace's actions, as the interface receives it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WorkspaceAction {
    /// The catalogue identity. What the interface hands back to perform it.
    pub id: ActionId,
    /// What to call it. Empty for an id with no row — there is nothing to call it.
    pub label: String,
    /// What it will do, in a sentence.
    pub describes: String,
    /// The icon beside the label.
    pub icon: String,
    /// What it does, where Mira knows.
    pub effect: Option<Effect>,
    /// Where it stands right now.
    pub state: ActionState,
}

/// Resolve a workspace's chosen actions against what this machine supports.
///
/// Pure. Nothing here starts, stops, observes or writes; it decides what may be
/// offered. Called on every read rather than cached, because it costs
/// nanoseconds against a `Support` the caller already gathered (ADR-0021).
#[must_use]
pub fn resolve(chosen: &[ActionId], support: &Support) -> Vec<WorkspaceAction> {
    chosen.iter().map(|id| one(id, support)).collect()
}

fn one(id: &ActionId, support: &Support) -> WorkspaceAction {
    let Some(action) = find(id) else {
        return WorkspaceAction {
            id: id.clone(),
            label: String::new(),
            describes: String::new(),
            icon: String::new(),
            effect: None,
            state: ActionState::Unknown,
        };
    };

    WorkspaceAction {
        id: id.clone(),
        label: action.label.to_owned(),
        describes: action.describes.to_owned(),
        icon: action.icon.to_owned(),
        effect: Some(action.effect),
        state: state_of(action.effect, support),
    }
}

fn state_of(effect: Effect, support: &Support) -> ActionState {
    match effect {
        Effect::OpenIn { kind } => {
            if !support.folder_exists {
                return unavailable("This project's folder is not where Mira left it.");
            }
            match support
                .openable
                .iter()
                .find(|(openable, _)| *openable == kind)
            {
                Some((_, name)) => ready(Some(name.clone())),
                None => unavailable(format!(
                    "There is no {} on this machine that Mira can open a folder in.",
                    kind.label().to_lowercase()
                )),
            }
        }

        Effect::RevealProject => {
            if !support.folder_exists {
                return unavailable("This project's folder is not where Mira left it.");
            }
            match &support.reveal {
                Some(reason) => unavailable(reason.clone()),
                None => ready(None),
            }
        }

        // The ambiguity rule. One running service is an action; three running
        // services is a question Mira cannot answer, and guessing would open
        // something nobody chose. Refused with the count, so the sentence tells
        // you what to do instead.
        Effect::OpenService => match support.running.len() {
            0 => unavailable(
                "None of this workspace's services are running. \
                 Add one it watches, or start it.",
            ),
            1 => ready(support.running.first().cloned()),
            many => unavailable(format!(
                "{many} of this workspace's services are running, so this action \
                 cannot say which one you mean. Open the one you want from the \
                 Services list."
            )),
        },

        // Neither reaches outside Mira, so neither has a way to be unavailable.
        Effect::MarkOpened | Effect::Observe => ready(None),
    }
}

fn ready(detail: Option<String>) -> ActionState {
    ActionState::Ready { detail }
}

fn unavailable(reason: impl Into<String>) -> ActionState {
    ActionState::Unavailable {
        reason: reason.into(),
    }
}
