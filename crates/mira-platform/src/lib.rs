//! Mira's platform boundary.
//!
//! This is the **only** crate allowed to contain `cfg(target_os)`, and the only
//! crate that knows an operating system exists (ADR-0005, enforced by a guard test).
//! Everything above it asks about capabilities instead.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod env;
pub mod platform;
pub mod resolve;
pub mod shell;
pub mod surface;

pub use env::{DisplayServer, EnvFacts, LinuxPackaging, Os};
pub use platform::{Platform, PlatformCapabilities};
pub use resolve::{resolve, resolve_all};
pub use shell::{is_openable, open_url_command, reveal_command, Shell, ShellHost};
pub use surface::{surface_treatment, SurfaceTreatment};
