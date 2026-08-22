//! `workspaces.*_action` — what a workspace can be asked to do.
//!
//! # What this is not
//!
//! It is not a way to run anything. There is **no program, no argument list, no
//! shell string, no working directory, no template** and no text of any kind
//! that gets executed — not in a parameter, not in a column, not in a file.
//! `mira_core::action::CATALOGUE` is a fixed array compiled into the binary, and
//! `mira_core::action::find` is the only thing that turns an identity into an
//! effect. An id that names no row is refused before it is stored and refused
//! again before anything happens, so the whole space of things this module can
//! do is the six rows of that array.
//!
//! Every one of those six was already reachable from a button somewhere.
//! **Actions add no new way for Mira to touch the machine**; they let a workspace
//! say which of the existing ones are its own.
//!
//! # How an action is named
//!
//! By its **catalogue identity**, and refused before storing if it names no row —
//! the same shape as an application preference (ADR-0019), and deliberately not
//! the ordinal shape a service uses (ADR-0020). The difference is what the list
//! is made of: a service list is *observed* and moves under the caller, so a
//! position is the only stable way to point into it; a catalogue is *compiled in*
//! and cannot move, so an identity is both stable and more legible.
//!
//! # What performing one does
//!
//! Dispatches to a seam that already exists — the launcher, the file manager, the
//! workspace's own open, the scheduler's observe-now. Nothing here spawns a
//! process, writes to a repository, or signals anything, and there is no variant
//! of [`Effect`] that could (ADR-0021).

use std::sync::Arc;

use mira_core::action::{find, resolve, ActionId, ActionState, Effect, Support, WorkspaceAction};
use mira_core::{MiraError, Result, WorkspaceId};
use mira_platform::{ChosenApp, LaunchHost, Launcher, PlatformCapabilities, Shell, ShellHost};
use mira_projects::ProjectService;
use mira_workspaces::{WorkspaceService, Workspaces};
use serde::{Deserialize, Serialize};
use tauri::State;
use ts_rs::TS;

use crate::clock::now;
use crate::commands::workspaces::preference;
use crate::state::AppState;

/// One catalogue row, as something a workspace could be given.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ActionOffer {
    /// The catalogue identity. What is sent back to add or remove it.
    pub id: ActionId,
    /// What to call it.
    pub label: String,
    /// What it will do, in a sentence.
    pub describes: String,
    /// The icon beside the label. Never instead of it.
    pub icon: String,
    /// Whether this workspace already has it.
    pub chosen: bool,
}

/// What happened when an action was performed.
///
/// A sentence rather than a structure, because there is nothing for the
/// interface to *do* with the outcome except say it. A launch that started
/// nothing Mira can name still happened, and says so.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Performed {
    /// Which action.
    pub id: ActionId,
    /// What happened, in words.
    pub happened: String,
}

fn workspaces(state: &AppState) -> Workspaces<&mira_db::Db> {
    Workspaces::new(state.db.as_ref())
}

/// Everything this machine and this workspace can currently support.
///
/// **Gathered once per request**, which is the one thing about this module that
/// was measured before it was written. Deciding all six actions costs 0.65 µs;
/// asking the machine what it can open costs 7.0 µs and a `is_dir()` another
/// 1.0 µs. Asking per action would be 47.9 µs instead of 8.0 — six times the
/// work to answer the same question six times (ADR-0021).
///
/// At 8 µs there is nothing worth keeping between requests, so there is no cache
/// and therefore no clock (ADR-0011).
fn support(state: &AppState, id: WorkspaceId) -> Result<Support> {
    let service = workspaces(state);
    let workspace = service.get(id)?;
    let launcher = Launcher::new(state.os, state.platform.clone());

    // The kinds this machine can open a folder in, each named by the application
    // *this workspace* chose — so an action's label is the same answer the Open
    // with row gives, not the first thing Mira happened to find (ADR-0019).
    let mut openable = Vec::new();
    for report in launcher.openable() {
        let preferred = preference(state, id, report.kind)?;
        if let ChosenApp::Ready { name, .. }
        | ChosenApp::Automatic {
            application: Some(name),
        } = launcher.chosen(report.kind, preferred.as_ref())
        {
            openable.push((report.kind, name));
        }
    }

    // The project's own root, read from Mira's database. A folder that has been
    // moved or deleted is a state every action that reaches it must respect, so
    // it is asked once here rather than discovered by a failure after a click.
    let root = state
        .projects()
        .get(workspace.project_id)
        .map(|project| std::path::PathBuf::from(project.root_path))
        .unwrap_or_default();

    // The workspace's own running services, named the way the Services section
    // names them. A service belonging to a sibling workspace is not here,
    // because this reads that workspace's rows and no others (ADR-0020).
    let observation = state.live.snapshot().services;
    let listening = observation.listening();
    let running: Vec<String> = service
        .services_unobserved(id, observation.observed(&listening))?
        .into_iter()
        .filter(|watched| watched.state.is_running())
        .map(|watched| format!(":{}", watched.watched.port))
        .collect();

    Ok(Support {
        openable,
        folder_exists: root.is_dir(),
        reveal: match state
            .platform
            .status(mira_core::Capability::RevealInFileManager)
        {
            mira_core::CapabilityStatus::Unavailable { reason, .. } => Some(reason),
            _ => None,
        },
        running,
    })
}

/// `workspaces.actions` — what this workspace can be asked to do, and whether
/// each can be done right now.
///
/// Resolved on every call. An action whose application has been uninstalled, or
/// whose folder has moved, is a sentence on the row rather than an error after a
/// click.
#[tauri::command]
pub fn workspaces_actions(
    workspace_id: WorkspaceId,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<WorkspaceAction>> {
    let chosen = workspaces(&state).actions(workspace_id)?;
    let support = support(&state, workspace_id)?;

    Ok(resolve(&chosen, &support))
}

/// `workspaces.action_catalogue` — every action Mira has, with what this
/// workspace already uses marked.
///
/// The menu. It is a list Mira produced from an array compiled into the binary,
/// which is what lets a choice come back as an identity: the interface can only
/// ever ask for something that was already on it.
#[tauri::command]
pub fn workspaces_action_catalogue(
    workspace_id: WorkspaceId,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<ActionOffer>> {
    let chosen = workspaces(&state).actions(workspace_id)?;

    Ok(mira_core::CATALOGUE
        .iter()
        .filter_map(|action| {
            let id: ActionId = action.id.parse().ok()?;
            Some(ActionOffer {
                chosen: chosen.contains(&id),
                id,
                label: action.label.to_owned(),
                describes: action.describes.to_owned(),
                icon: action.icon.to_owned(),
            })
        })
        .collect())
}

/// `workspaces.set_action` — give this workspace an action, or take it away.
///
/// **An id that names no catalogue row is refused before it is stored**, so the
/// database can only ever hold identities Mira itself compiled in. There is no
/// path here, no program name and no command, and no column one could be written
/// into even if there were.
///
/// Removing is allowed for an id Mira no longer has a row for — that is how a
/// workspace clears an action left behind by an older version. Adding one is not.
#[tauri::command]
pub fn workspaces_set_action(
    workspace_id: WorkspaceId,
    action: ActionId,
    wanted: bool,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<WorkspaceAction>> {
    if wanted && find(&action).is_none() {
        // The catalogue is the validation. An id is only an action because
        // Mira's own compiled array says so.
        return Err(MiraError::invalid(
            "action",
            format!("Mira has no action called {action}."),
        ));
    }

    workspaces(&state).set_action(workspace_id, &action, wanted, now())?;
    workspaces_actions(workspace_id, state)
}

/// `workspaces.perform_action` — do one of this workspace's actions.
///
/// Two arguments, and everything else resolved beneath the boundary. The
/// interface names **which workspace** and **which catalogue identity**; which
/// directory, which application, which address and which service all come from
/// Mira's own rows and its own compiled tables.
///
/// Refused rather than attempted when the action is not this workspace's, when
/// the identity names no row, and when the state says it cannot be done — so an
/// ambiguous action (two services running, no way to know which was meant) is
/// impossible rather than a guess.
#[tauri::command]
pub fn workspaces_perform_action(
    workspace_id: WorkspaceId,
    action: ActionId,
    state: State<'_, Arc<AppState>>,
) -> Result<Performed> {
    // The workspace's own list, not the catalogue. An action this workspace was
    // never given is `NotFound` here even though the catalogue has it, so one
    // workspace's configuration cannot be reached through another's id.
    if !workspaces(&state).actions(workspace_id)?.contains(&action) {
        return Err(MiraError::NotFound {
            what: "That action".to_owned(),
        });
    }

    let resolved = resolve(
        std::slice::from_ref(&action),
        &support(&state, workspace_id)?,
    )
    .into_iter()
    .next()
    .ok_or_else(|| MiraError::NotFound {
        what: "That action".to_owned(),
    })?;

    match &resolved.state {
        ActionState::Unknown => {
            return Err(MiraError::invalid(
                "action",
                format!("Mira has no action called {action} any more."),
            ))
        }
        ActionState::Unavailable { reason } => {
            return Err(MiraError::Unsupported {
                capability: mira_core::Capability::LaunchApplication,
                reason: reason.clone(),
            })
        }
        ActionState::Ready { .. } => {}
    }

    let effect = resolved.effect.ok_or_else(|| MiraError::NotFound {
        what: "That action".to_owned(),
    })?;

    perform(&state, workspace_id, effect, &resolved).map(|happened| Performed {
        id: action,
        happened,
    })
}

/// Dispatch to the seam the effect names.
///
/// One arm per [`Effect`] variant, and the match is exhaustive — which is what
/// makes "the effect list is the privilege list" true rather than aspirational.
/// Every arm calls something that already existed; none of them constructs a
/// program, a path or an address of its own.
fn perform(
    state: &Arc<AppState>,
    workspace_id: WorkspaceId,
    effect: Effect,
    resolved: &WorkspaceAction,
) -> Result<String> {
    match effect {
        // Through `workspaces.launch`'s own body, so the one place that builds
        // a launch directory stays one place (`security-and-privacy.md` §5
        // rule 11). Actions add no second way to name a folder.
        Effect::OpenIn { kind } => {
            crate::commands::workspaces::launch_at_root(state, workspace_id, kind).map(|started| {
                match started.application {
                    Some(name) => format!("Opened in {name}."),
                    None => format!("Opened in your {}.", kind.label().to_lowercase()),
                }
            })
        }

        Effect::RevealProject => {
            let root = workspaces(state).working_directory(workspace_id)?;
            Shell::new(state.os, state.platform.clone())
                .reveal(&root)
                .map(|()| "Showed the folder.".to_owned())
        }

        // Resolution already established that exactly one is running. Which one
        // is read again here rather than carried, so the thing opened is a row
        // keyed by this workspace — the same path `workspaces.open_service`
        // takes, sharing its one address construction site (ADR-0020).
        Effect::OpenService => {
            let observation = state.live.snapshot().services;
            let listening = observation.listening();
            let mut running = workspaces(state)
                .services_unobserved(workspace_id, observation.observed(&listening))?
                .into_iter()
                .filter(|watched| watched.state.is_running());

            let only = running.next().ok_or_else(|| MiraError::NotFound {
                what: "A running service in this workspace".to_owned(),
            })?;
            if running.next().is_some() {
                // Unreachable through resolution, which refuses two. Kept
                // because "the check and the act are in different functions" is
                // exactly where a substitution would appear.
                return Err(MiraError::invalid(
                    "action",
                    "More than one of this workspace's services is running.".to_owned(),
                ));
            }

            crate::commands::services::open_watched(state, workspace_id, only.watched.id)
                .map(|_| format!("Opened :{} in the browser.", only.watched.port))
        }

        Effect::MarkOpened => workspaces(state)
            .open(workspace_id, now())
            .map(|workspace| format!("{} is the workspace you are working in.", workspace.name)),

        // The scheduler's own reading, taken early. No timer is created and no
        // observer is added — this is the refresh that already exists (ADR-0011).
        Effect::Observe => {
            crate::observers::observe_once(state)?;
            let _ = resolved;
            Ok("Read this project and the machine's ports again.".to_owned())
        }
    }
}
