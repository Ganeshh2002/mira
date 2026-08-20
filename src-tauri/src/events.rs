//! The events Mira pushes to the interface.
//!
//! `architecture.md` §5: events carry **change notifications, not payloads**. The
//! interface hears that something moved and asks for what it needs. That keeps a
//! busy watcher's serialisation cost near zero and stops the event channel
//! quietly becoming a second, unversioned API.

/// Something was observed. The interface re-reads the live snapshot.
pub const LIVE: &str = "mira://live";
