//! Mira's SQLite store.
//!
//! This is the only crate in the workspace that speaks SQL. Everything above it
//! goes through repository traits, and the frontend reaches none of it directly —
//! there is no arbitrary-SQL channel to the webview, by design (ADR-0004).

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod db;
pub mod migrations;
pub mod projects;
pub mod workspaces;

pub use db::{target_version, Db};
pub use migrations::{Migration, MIGRATIONS};
pub use projects::{NewProject, ProjectRepo};
pub use workspaces::{NewWorkspace, WorkspaceRepo};
