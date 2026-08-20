//! When recurring work is allowed to happen.
//!
//! The scheduler's gate, and the whole of Mira's idle-CPU budget. Two conditions,
//! both of which have to hold:
//!
//! 1. **There is something to observe.** With no projects added, there is no
//!    repository to read and no service to attribute, so the clock ticks and
//!    nothing happens.
//! 2. **A window is visible.** `information-architecture.md` §3: nothing is
//!    computed while no window is visible. A Mira sitting in the tray is a Mira
//!    doing nothing, which is what makes it reasonable to leave running all day.
//!
//! Visibility is asked of the window rather than tracked through events. Events
//! can be missed — a window shown by the tray, a Space switch, a compositor that
//! reports focus differently — and a gate that has drifted out of sync either
//! burns CPU forever or never updates again. Asking costs one call per tick.

use std::sync::Arc;

use mira_scheduler::Gate;
use tauri::{AppHandle, Manager, Runtime};

use crate::state::AppState;
use crate::windows::MAIN;

/// The gate Mira actually runs behind.
pub struct WhenVisible<R: Runtime> {
    app: AppHandle<R>,
    state: Arc<AppState>,
}

impl<R: Runtime> WhenVisible<R> {
    /// Gate observation on this application's windows and projects.
    #[must_use]
    pub const fn new(app: AppHandle<R>, state: Arc<AppState>) -> Self {
        Self { app, state }
    }
}

impl<R: Runtime> Gate for WhenVisible<R> {
    fn is_open(&self) -> bool {
        if !self.state.has_projects() {
            return false;
        }

        // Any window counts, not just the main one: Settings shows the capability
        // list, and a person reading it should not be looking at frozen data.
        self.app
            .webview_windows()
            .values()
            .any(|window| window.is_visible().unwrap_or(false))
            || self
                .app
                .get_webview_window(MAIN)
                .is_some_and(|window| window.is_visible().unwrap_or(false))
    }
}
