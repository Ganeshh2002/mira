//! `keep_awake.*` — asking the operating system not to fall asleep.
//!
//! Thin by rule (`architecture.md` §5). The whole privilege the interface has
//! here is **one word out of four**: off, thirty minutes, an hour, or until turned
//! off. It cannot name a duration, a program, or a power setting, and there is no
//! parameter through which it could — which is what makes the shape of this
//! feature safe rather than its implementation
//! ([ADR-0014](../../../docs/adr/0014-keep-awake.md)).
//!
//! Reading the state is a read of memory. Setting it takes out or gives back one
//! operating-system request, and arms or disarms one deadline. Neither polls.

use std::sync::Arc;

use mira_core::Result;
use mira_platform::{KeepAwakeSpan, KeepAwakeState};
use tauri::State;

use crate::state::AppState;

/// `keep_awake.state` — what is being held right now.
///
/// Expires on the way out, so a lock whose time ran out while nobody was looking
/// is reported as off rather than as still holding.
#[tauri::command]
pub fn keep_awake_state(state: State<'_, Arc<AppState>>) -> KeepAwakeState {
    state.awake.state()
}

/// `keep_awake.set` — hold one of the four spans, or none of them.
///
/// Replaces whatever was held; it never stacks a second request on the first.
/// Where the platform has no way for Mira to ask, this is an `Unsupported` error
/// carrying the reason the capability gave, and the interface shows that reason
/// rather than a control that does nothing (`platform-abstraction.md` §2 rule 4).
#[tauri::command]
pub fn keep_awake_set(
    span: KeepAwakeSpan,
    state: State<'_, Arc<AppState>>,
) -> Result<KeepAwakeState> {
    state.awake.set(span)
}
