//! The one error type that crosses the IPC boundary.
//!
//! Every variant carries something the UI can *show a human*
//! (`docs/architecture/architecture.md` §5). There is no `Other(String)` escape
//! hatch and no variant whose only content is a backtrace, because both produce the
//! error message design-system §9 forbids: an apology followed by a stack trace.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::capability::Capability;

/// Anything that can go wrong, in a shape the interface can render.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum MiraError {
    /// The thing asked for is not there.
    #[serde(rename_all = "camelCase")]
    NotFound {
        /// What was looked for, named the way the user named it.
        what: String,
    },

    /// The operating system said no.
    #[serde(rename_all = "camelCase")]
    PermissionDenied {
        /// What was refused.
        what: String,
        /// The action that would fix it, if there is one.
        hint: String,
    },

    /// This platform cannot do it, and Mira will not pretend otherwise.
    #[serde(rename_all = "camelCase")]
    Unsupported {
        /// The capability that is off.
        capability: Capability,
        /// Its user-facing reason string.
        reason: String,
    },

    /// It took too long and was abandoned rather than left hanging.
    #[serde(rename_all = "camelCase")]
    Timeout {
        /// What was being attempted.
        operation: String,
        /// How long Mira waited.
        #[ts(type = "number")]
        after_ms: u64,
    },

    /// Something outside Mira failed — SQLite, a socket, another process.
    #[serde(rename_all = "camelCase")]
    External {
        /// Which subsystem.
        source: String,
        /// What it reported.
        detail: String,
    },

    /// The input was wrong.
    #[serde(rename_all = "camelCase")]
    Invalid {
        /// Which field.
        field: String,
        /// What is wrong with it.
        detail: String,
    },
}

impl MiraError {
    /// An [`MiraError::External`] from any error, naming the subsystem.
    pub fn external(source: impl Into<String>, detail: impl std::fmt::Display) -> Self {
        Self::External {
            source: source.into(),
            detail: detail.to_string(),
        }
    }

    /// An [`MiraError::Invalid`] naming the offending field.
    pub fn invalid(field: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::Invalid {
            field: field.into(),
            detail: detail.into(),
        }
    }
}

impl std::fmt::Display for MiraError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound { what } => write!(f, "{what} was not found"),
            Self::PermissionDenied { what, hint } => write!(f, "{what} was refused. {hint}"),
            Self::Unsupported { capability, reason } => {
                write!(f, "{} is unavailable. {reason}", capability.label())
            }
            Self::Timeout {
                operation,
                after_ms,
            } => write!(f, "{operation} did not finish within {after_ms} ms"),
            Self::External { source, detail } => write!(f, "{source} failed: {detail}"),
            Self::Invalid { field, detail } => write!(f, "{field} is not valid: {detail}"),
        }
    }
}

impl std::error::Error for MiraError {}
