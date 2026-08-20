//! The one clock in the process.
//!
//! The domain crates take a timestamp as a parameter so they stay pure and their
//! tests stay deterministic (`mira-projects`). This is where the real time is
//! read, once per command, at the outermost edge.

use std::time::{SystemTime, UNIX_EPOCH};

/// The current time, Unix epoch seconds UTC — the units `data-model.md` §2 uses.
///
/// A clock set before 1970 yields 0 rather than a negative timestamp; the row is
/// then merely old, which is a far better failure than an ordering that inverts.
#[must_use]
pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}
