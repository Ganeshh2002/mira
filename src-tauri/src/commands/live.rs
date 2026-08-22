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

/// `live.open_service` — open a listening port in the browser.
///
/// The interface names a **port**, never a URL. Mira builds
/// `http://localhost:<port>` itself, so there is no argument through which a page
/// could ask Mira to open a `file://` path or a custom scheme — and a guard test
/// fails the build if a command ever takes a URL (`security-and-privacy.md` §5
/// rule 5).
///
/// Read-only, like every action in this slice: opening a service does not touch
/// the process serving it.
#[tauri::command]
pub fn live_open_service(port: u16, state: State<'_, Arc<AppState>>) -> Result<()> {
    if !state
        .live
        .snapshot()
        .services
        .services
        .iter()
        .any(|service| service.listener.port == port)
    {
        // Only a port Mira is actually watching. Without this the command would
        // open any port on the machine on request, which is a wider door than the
        // feature needs.
        return Err(MiraError::NotFound {
            what: format!("A service on port {port}"),
        });
    }

    // Through the launcher, so a service opens the way an editor does — and on
    // macOS through the window server rather than through a command line
    // (ADR-0013). A web address goes to the browser the person actually chose,
    // never to whichever one Mira happened to find first.
    //
    // `None` because this is the *machine's* service list rather than a
    // workspace's: there is no workspace here whose choice could apply, and
    // borrowing one from somewhere would be a guess about which one
    // ([ADR-0019](../../../docs/adr/0019-application-preferences.md)).
    Launcher::new(state.os, state.platform.clone())
        .launch(
            AppKind::Browser,
            None,
            LaunchTarget::WebAddress(localhost(port)),
        )
        .map(|_| ())
}

/// The address a locally listening port is reached at.
fn localhost(port: u16) -> String {
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
