//! Typed identifiers.
//!
//! These are newtypes rather than bare integers so that a `ProjectId` cannot be
//! passed where a `WorkspaceId` is expected. The compiler carries the invariant
//! (ADR-0003), which matters most at the IPC boundary where both arrive as numbers.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

macro_rules! id_type {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
        #[ts(export)]
        // serde serialises a newtype struct as its inner value, so the wire form is a
        // bare number. `type = "number"` keeps TypeScript agreeing: the IPC boundary is
        // JSON, where an i64 arrives as a number, not a bigint. SQLite rowids stay well
        // inside the safe-integer range.
        pub struct $name(#[ts(type = "number")] i64);

        impl $name {
            /// Wrap a raw row id from the database.
            #[must_use]
            pub const fn new(raw: i64) -> Self {
                Self(raw)
            }

            /// The underlying row id.
            #[must_use]
            pub const fn get(self) -> i64 {
                self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

id_type!(
    /// Identifies a project — a directory on disk plus what Mira learned about it.
    ProjectId
);

id_type!(
    /// Identifies a workspace — a named way of working on one project.
    WorkspaceId
);

id_type!(
    /// Identifies one service a workspace watches.
    ///
    /// Issued by Mira when the service is added, and the *only* name the
    /// interface has for it afterwards. Opening or forgetting a watched service
    /// names this id, never the port behind it, so there is no request through
    /// which a page could reach a port Mira did not already put on the row
    /// (ADR-0020).
    WorkspaceServiceId
);
