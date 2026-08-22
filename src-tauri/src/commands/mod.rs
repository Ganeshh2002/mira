//! Command handlers.
//!
//! Commands are **thin**: validate, call a service, map the result. No business
//! logic lives in `src-tauri` (`architecture.md` §5). Every command returns
//! `Result<T, MiraError>`; there are no `unwrap`s on the command path.

pub mod actions;
pub mod app;
pub mod awake;
pub mod git;
pub mod live;
pub mod ports;
pub mod projects;
pub mod services;
pub mod workspaces;
