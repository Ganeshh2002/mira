//! Listening ports, and the rule that places them.
//!
//! Two halves. [`scan`] reads the socket table through native platform
//! interfaces; [`attribution`] decides, from a process's working directory alone,
//! which project a listener belongs to — or refuses to decide and says why.
//!
//! Read-only throughout. There is no method here that closes a socket, signals a
//! process, or stops a service; those are a later slice with their own security
//! design, and their absence from this interface is what keeps that ordering
//! real (slice brief §9).

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod attribution;
pub mod scan;

pub use attribution::{attribute, Attribution, PackageBoundary, ProjectRoot, Unattributed};
pub use scan::{Listener, PortScanner, Ports};
