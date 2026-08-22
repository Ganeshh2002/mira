//! Mira's domain vocabulary.
//!
//! This crate is the bottom of the dependency graph: it depends on nothing else in
//! the workspace, contains no I/O, and knows nothing about Tauri, SQLite, or any
//! operating system. Everything here is a type that other crates agree on.
//!
//! See `docs/architecture/architecture.md` §3 for the dependency rule this enforces.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod capability;
pub mod error;
pub mod ids;
pub mod project;
pub mod service;
pub mod workspace;

pub use capability::{Capability, CapabilityReport, CapabilityStatus};
pub use error::MiraError;
pub use ids::{ProjectId, WorkspaceId, WorkspaceServiceId};
pub use project::Project;
pub use service::{
    resolve, Listening, NotAPort, Observed, Port, ServiceState, WatchedService, WorkspaceService,
};
pub use workspace::{AppKind, Workspace};

/// Convenience alias: every fallible operation in Mira fails with [`MiraError`].
pub type Result<T> = std::result::Result<T, MiraError>;
