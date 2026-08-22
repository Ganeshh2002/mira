//! The embedded, forward-only migration set.

/// One numbered migration, embedded in the binary at compile time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Migration {
    /// Its number. Must be contiguous from 1 and never reused.
    pub version: i32,
    /// A short name, used in error messages.
    pub name: &'static str,
    /// The SQL to apply.
    pub sql: &'static str,
}

/// Every migration Mira ships, in order.
///
/// Applied migrations are **never edited** — a mistake is corrected by adding a new
/// one (`docs/architecture/data-model.md` §4).
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "init",
        sql: include_str!("../migrations/0001_init.sql"),
    },
    Migration {
        version: 2,
        name: "workspace_context",
        sql: include_str!("../migrations/0002_workspace_context.sql"),
    },
    Migration {
        version: 3,
        name: "application_preferences",
        sql: include_str!("../migrations/0003_application_preferences.sql"),
    },
    Migration {
        version: 4,
        name: "workspace_services",
        sql: include_str!("../migrations/0004_workspace_services.sql"),
    },
];

/// The version a fully migrated database reports.
#[must_use]
pub fn latest_version(migrations: &[Migration]) -> i32 {
    migrations.iter().map(|m| m.version).max().unwrap_or(0)
}
