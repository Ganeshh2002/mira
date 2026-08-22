//! `live.*` — what the observers last saw.
//!
//! Both commands are reads of memory. Neither starts a timer, and neither is a
//! second path into the database: persisted state is `projects.*`, observed state
//! is here, and the split is the one `data-model.md` §1 rule 2 draws.

use std::sync::Arc;

use mira_core::AppKind;
use mira_core::{MiraError, Result};
use mira_platform::{LaunchHost, LaunchTarget, Launcher};
use tauri::State;

use crate::live::LiveSnapshot;
use crate::observers;
use crate::state::AppState;

/// `live.snapshot` — everything observed so far.
///
/// Cheap: a lock and a clone. The interface calls this whenever the backend says
/// something moved, and gets both the readings and when they were taken, so it
/// can show freshness rather than implying it.
#[tauri::command]
pub fn live_snapshot(state: State<'_, Arc<AppState>>) -> LiveSnapshot {
    state.live.snapshot()
}

/// `live.refresh` — observe now, rather than at the next tick.
///
/// The refresh action, and the first paint. Without it the window would open onto
/// an empty five seconds, because the scheduler waits out its first interval
/// before doing anything.
///
/// A failure is returned *and* recorded, so the snapshot that follows explains
/// itself even if the caller drops the error.
#[tauri::command]
pub async fn live_refresh(state: State<'_, Arc<AppState>>) -> Result<LiveSnapshot> {
    let observing = Arc::clone(&state);

    // On the blocking pool: this reads repositories, the socket table and the
    // process table, none of which belongs on the webview's thread.
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        let result = observers::observe_once(&observing);
        (result, observing.live.snapshot())
    })
    .await;

    match outcome {
        Ok((Ok(()), snapshot)) => Ok(snapshot),
        // The snapshot is returned even when an observer failed: one broken
        // observer must not blank the other's reading.
        Ok((Err(_), snapshot)) => Ok(snapshot),
        Err(joined) => Err(mira_core::MiraError::external("Mira", joined)),
    }
}

/// `live.open_service` — open one of the observed services in the browser.
///
/// The interface names a **position** in the list `live.snapshot` returned —
/// never a port, and never a URL. Two things follow. A page cannot ask Mira to
/// open a port nobody is serving, because there is no parameter that could say
/// which; and the whole space of addresses this command can produce is the set
/// of loopback addresses Mira is already watching, built in Rust
/// (`security-and-privacy.md` §5 rule 5).
///
/// This used to take the port itself, checked against the observed list. The
/// check was real, but the parameter was still a number of the caller's
/// choosing, and slice 4c's rule is that no raw port crosses the boundary in
/// either direction of a request. An ordinal past the end is a stale selection —
/// the list moved between the render and the click — and says so
/// (ADR-0020).
///
/// Read-only, like every action in this slice: opening a service does not touch
/// the process serving it.
#[tauri::command]
pub fn live_open_service(at: u32, state: State<'_, Arc<AppState>>) -> Result<()> {
    let snapshot = state.live.snapshot();

    let service = usize::try_from(at)
        .ok()
        .and_then(|at| snapshot.services.services.get(at))
        .ok_or_else(|| MiraError::NotFound {
            what: "That service".to_owned(),
        })?;

    // Through the launcher, so a service opens the way an editor does — and on
    // macOS through the window server rather than through a command line
    // (ADR-0013).
    Launcher::new(state.os, state.platform.clone())
        .launch(
            AppKind::Browser,
            LaunchTarget::WebAddress(localhost(service.listener.port)),
        )
        .map(|_| ())
}

/// The address a locally listening port is reached at.
///
/// The one construction site for every address a browser is ever handed, and
/// a guard test counts it: `LaunchTarget::WebAddress` may only be built from
/// this call. `workspaces.open_service` shares it rather than writing a second
/// one, because a second place to build a URL is a second place to get it
/// wrong (`security-and-privacy.md` §5 rule 5).
pub fn localhost(port: u16) -> String {
    format!("http://localhost:{port}")
}

#[cfg(test)]
mod tests {
    use super::localhost;

    #[test]
    fn a_port_becomes_a_loopback_web_address() {
        assert_eq!(localhost(3000), "http://localhost:3000");
    }

    #[test]
    fn the_address_is_one_the_platform_will_open() {
        assert!(mira_platform::is_openable(&localhost(5173)));
    }

    #[test]
    fn the_address_is_the_only_thing_a_browser_is_ever_handed() {
        // The port is a `u16`, so the whole space of addresses this command can
        // produce is `http://localhost:0` through `http://localhost:65535`.
        for port in [0, 1, 3000, 65535] {
            let built = localhost(port);
            assert!(built.starts_with("http://localhost:"));
            assert!(mira_platform::is_openable(&built));
        }
    }
}
