//! The events Mira pushes to the interface.
//!
//! `architecture.md` §5: events carry **change notifications, not payloads**. The
//! interface hears that something moved and asks for what it needs. That keeps a
//! busy watcher's serialisation cost near zero and stops the event channel
//! quietly becoming a second, unversioned API.

/// Something was observed. The interface re-reads the live snapshot.
pub const LIVE: &str = "mira://live";

/// Keep Awake changed without anybody pressing anything.
///
/// Sent when a span reaches its end. Every other change is the answer to a
/// command the interface already made, so it already knows — this exists for the
/// one moment the backend knows something the interface does not.
pub const KEEP_AWAKE: &str = "mira://keep-awake";
