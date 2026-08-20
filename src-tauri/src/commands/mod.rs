//! Command handlers.
//!
//! Commands are **thin**: validate, call a service, map the result. No business
//! logic lives in `src-tauri` (`architecture.md` §5). Every command returns
//! `Result<T, MiraError>`; there are no `unwrap`s on the command path.

pub mod app;
