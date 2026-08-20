//! The connection and the migration runner.
//!
//! Mira hand-rolls this rather than inheriting sqlx's, which is one of the accepted
//! costs in ADR-0004. It is about a hundred lines, and the promises it keeps are in
//! `docs/architecture/data-model.md` §4: forward-only, numbered, tracked in
//! `PRAGMA user_version`, each applied in a transaction, and a newer database
//! refused rather than upgraded.

use std::path::Path;
use std::sync::Mutex;

use mira_core::{MiraError, Result};
use rusqlite::Connection;

use crate::migrations::{latest_version, Migration, MIGRATIONS};

/// Mira's SQLite database: one file, one connection, one mutex.
#[derive(Debug)]
pub struct Db {
    conn: Mutex<Connection>,
    schema_version: i32,
}

impl Db {
    /// Open (or create) the database at `path` and bring it up to date.
    pub fn open(path: &Path) -> Result<Self> {
        Self::open_with(path, MIGRATIONS)
    }

    /// An in-memory database, for tests.
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(sqlite)?;
        Self::from_connection(conn, MIGRATIONS)
    }

    /// Open `path` and apply `migrations`.
    ///
    /// Taking the migration set as an argument is what lets the tests build a
    /// previous version's database and upgrade it, which §4 requires of every
    /// migration.
    pub fn open_with(path: &Path, migrations: &[Migration]) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| MiraError::external("the application data directory", e))?;
        }
        let conn = Connection::open(path).map_err(sqlite)?;
        Self::from_connection(conn, migrations)
    }

    fn from_connection(mut conn: Connection, migrations: &[Migration]) -> Result<Self> {
        // Per connection, per data-model.md §2. Foreign keys default to OFF in
        // SQLite, so every constraint in the schema is inert without this line.
        conn.execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(sqlite)?;

        // WAL is a durable property of the file. An in-memory database answers
        // "memory" and cannot change, which is not an error worth failing on.
        let _ = conn.query_row("PRAGMA journal_mode = WAL", [], |row| {
            row.get::<_, String>(0)
        });

        let schema_version = migrate(&mut conn, migrations)?;

        Ok(Self {
            conn: Mutex::new(conn),
            schema_version,
        })
    }

    /// The applied schema version.
    #[must_use]
    pub const fn schema_version(&self) -> i32 {
        self.schema_version
    }

    /// Borrow the connection for one operation.
    ///
    /// Access is serialised through a single connection behind a mutex: SQLite is
    /// fast enough here that a pool would add contention bugs and no measurable
    /// speed (`docs/architecture/architecture.md` §6).
    pub fn with_connection<T>(
        &self,
        f: impl FnOnce(&Connection) -> rusqlite::Result<T>,
    ) -> Result<T> {
        let guard = self
            .conn
            .lock()
            .map_err(|_| MiraError::external("SQLite", "the database lock was poisoned"))?;
        f(&guard).map_err(sqlite)
    }

    /// Fold the write-ahead log back into the database file and truncate it.
    ///
    /// Mira exits through `app.exit`, which does not run destructors, so nothing
    /// drops this connection and SQLite never performs its usual close-time
    /// checkpoint. Without this, recent writes stay in `mira.db-wal` — recoverable,
    /// but not what Settings promises when it says everything Mira knows is in one
    /// file. Shutdown calls it; a failure is worth reporting but not worth blocking
    /// an exit over, because the log is replayed on the next open either way.
    pub fn checkpoint(&self) -> Result<()> {
        self.with_connection(|conn| {
            conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))
        })
    }
}

/// Apply every pending migration and return the resulting version.
fn migrate(conn: &mut Connection, migrations: &[Migration]) -> Result<i32> {
    let current: i32 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(sqlite)?;
    let target = latest_version(migrations);

    if current > target {
        // Opening it read-write and "fixing" it would corrupt data written by a
        // schema this build does not understand. Refuse, and say why.
        return Err(MiraError::External {
            source: "the Mira database".to_owned(),
            detail: format!(
                "This database was written by a newer version of Mira (schema {current}; \
                 this build understands {target}). Update Mira to open it."
            ),
        });
    }

    for migration in migrations.iter().filter(|m| m.version > current) {
        let tx = conn.transaction().map_err(sqlite)?;
        tx.execute_batch(migration.sql)
            .map_err(|e| MiraError::External {
                source: format!("migration {:04} ({})", migration.version, migration.name),
                detail: e.to_string(),
            })?;
        // Transactional: a rollback takes the version with it, so a half-applied
        // migration cannot leave the database claiming to be upgraded.
        tx.execute_batch(&format!("PRAGMA user_version = {};", migration.version))
            .map_err(sqlite)?;
        tx.commit().map_err(sqlite)?;
    }

    Ok(target)
}

fn sqlite(error: rusqlite::Error) -> MiraError {
    MiraError::external("SQLite", error)
}

/// Latest version this build knows how to produce.
#[must_use]
pub fn target_version() -> i32 {
    latest_version(MIGRATIONS)
}
