//! The migration runner.
//!
//! `docs/architecture/data-model.md` §4 makes five promises about migrations:
//! forward-only, numbered, tracked in `PRAGMA user_version`, applied in a
//! transaction, and idempotent. Each is a test here. The last promise — that a
//! newer database is refused rather than corrupted — matters most, because getting
//! it wrong destroys user data silently.

use mira_db::{Db, Migration, ProjectRepo, WorkspaceRepo, MIGRATIONS};
use rusqlite::Connection;
use tempfile::TempDir;

fn table_names(db: &Db) -> Vec<String> {
    db.with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT name FROM sqlite_master WHERE type = 'table' \
             AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect()
    })
    .expect("reading the schema")
}

fn user_version(path: &std::path::Path) -> i32 {
    let conn = Connection::open(path).expect("open");
    conn.query_row("PRAGMA user_version", [], |r| r.get(0))
        .expect("user_version")
}

#[test]
fn a_fresh_database_reports_the_latest_version() {
    let db = Db::open_in_memory().expect("open");
    assert_eq!(db.schema_version(), mira_db::target_version());
    assert_eq!(
        db.schema_version(),
        2,
        "0001_init, then 0002_workspace_context"
    );
}

#[test]
fn the_schema_creates_every_table_in_the_data_model() {
    let db = Db::open_in_memory().expect("open");
    let tables = table_names(&db);

    for expected in [
        "app_preferences",
        "applications",
        "commands",
        "docker_refs",
        "expected_ports",
        "preferences",
        "project_active_workspace",
        "project_markers",
        "projects",
        "sessions",
        "session_pauses",
        "shelf_items",
        "ssh_hosts",
        "themes",
        "workspaces",
        // 0002: which kinds of application a workspace works with. Intent; the
        // application itself is discovered on the machine, never stored.
        "workspace_applications",
    ] {
        assert!(
            tables.iter().any(|t| t == expected),
            "table {expected} is missing; tables were {tables:?}"
        );
    }
    assert_eq!(tables.len(), 16, "16 tables, no more: {tables:?}");
}

#[test]
fn the_baseline_creates_no_automation_or_account_tables() {
    let tables = table_names(&Db::open_in_memory().expect("open"));

    for forbidden in [
        "automations",
        "automation_actions",
        "users",
        "accounts",
        "organizations",
        "subscriptions",
        "sync_state",
        "telemetry",
    ] {
        assert!(
            !tables.iter().any(|t| t == forbidden),
            "{forbidden} must not exist: automation is 0.6+ and accounts are ADR-0006"
        );
    }
}

#[test]
fn opening_an_existing_database_twice_changes_nothing() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("mira.db");

    let first = Db::open(&path).expect("first open");
    let tables_before = table_names(&first);
    drop(first);

    let second = Db::open(&path).expect("second open");
    assert_eq!(second.schema_version(), mira_db::target_version());
    assert_eq!(
        table_names(&second),
        tables_before,
        "re-opening must not re-run or duplicate anything"
    );
}

#[test]
fn data_survives_a_reopen() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("mira.db");

    {
        let db = Db::open(&path).expect("open");
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO projects (name, root_path, created_at, updated_at) \
                 VALUES ('mira', '/tmp/mira', 1, 1)",
                [],
            )
        })
        .expect("insert");
    }

    let db = Db::open(&path).expect("reopen");
    assert_eq!(
        ProjectRepo::count(&db).expect("count"),
        1,
        "the row must still be there"
    );
}

#[test]
fn foreign_keys_are_enforced_on_every_connection() {
    let db = Db::open_in_memory().expect("open");

    let result = db.with_connection(|conn| {
        conn.execute(
            "INSERT INTO workspaces (project_id, name, created_at, updated_at) \
             VALUES (999, 'ghost', 1, 1)",
            [],
        )
    });

    let error = result.expect_err("a workspace pointing at a missing project must be refused");
    assert!(
        error.to_string().to_uppercase().contains("FOREIGN KEY"),
        "it must fail the foreign key, not merely fail: {error}"
    );
}

#[test]
fn one_default_workspace_per_project_is_a_database_guarantee() {
    let db = Db::open_in_memory().expect("open");
    db.with_connection(|conn| {
        conn.execute(
            "INSERT INTO projects (id, name, root_path, created_at, updated_at) \
             VALUES (1, 'p', '/p', 1, 1)",
            [],
        )?;
        conn.execute(
            "INSERT INTO workspaces (project_id, name, is_default, created_at, updated_at) \
             VALUES (1, 'a', 1, 1, 1)",
            [],
        )
    })
    .expect("first default workspace");

    let second = db.with_connection(|conn| {
        conn.execute(
            "INSERT INTO workspaces (project_id, name, is_default, created_at, updated_at) \
             VALUES (1, 'b', 1, 1, 1)",
            [],
        )
    });

    assert!(
        second.is_err(),
        "the partial unique index must refuse a second default"
    );
}

#[test]
fn a_database_from_a_newer_mira_is_refused_rather_than_corrupted() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("mira.db");

    Db::open(&path).expect("create at current version");
    Connection::open(&path)
        .expect("open")
        .execute_batch("PRAGMA user_version = 99")
        .expect("bump to a future version");

    let error = Db::open(&path).expect_err("a newer database must not be opened");
    let message = error.to_string().to_lowercase();
    assert!(
        message.contains("newer"),
        "the message must tell the user what happened, got: {error}"
    );
    assert_eq!(
        user_version(&path),
        99,
        "refusing must leave the database untouched"
    );
}

#[test]
fn a_failing_migration_leaves_the_version_unchanged() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("mira.db");

    let broken: &[Migration] = &[
        MIGRATIONS[0],
        Migration {
            version: 2,
            name: "broken",
            sql: "CREATE TABLE good (id INTEGER); THIS IS NOT SQL;",
        },
    ];

    Db::open_with(&path, broken).expect_err("a broken migration must fail loudly");

    assert_eq!(
        user_version(&path),
        1,
        "version 1 applied, version 2 rolled back entirely"
    );

    let conn = Connection::open(&path).expect("open");
    let leaked: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='good'",
            [],
            |r| r.get(0),
        )
        .expect("query");
    assert_eq!(
        leaked, 0,
        "a half-applied migration must not leave tables behind"
    );
}

#[test]
fn pending_migrations_apply_in_order_to_an_older_database() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("mira.db");

    // Build the previous version's database, then upgrade it, exactly as
    // data-model.md §4 requires every migration to be tested.
    Db::open_with(&path, &MIGRATIONS[..1]).expect("baseline");
    Connection::open(&path)
        .expect("open")
        .execute(
            "INSERT INTO projects (name, root_path, created_at, updated_at) \
             VALUES ('kept', '/kept', 1, 1)",
            [],
        )
        .expect("seed a row that must survive");

    let upgraded: &[Migration] = &[
        MIGRATIONS[0],
        Migration {
            version: 2,
            name: "add-note",
            sql: "ALTER TABLE projects ADD COLUMN note TEXT;",
        },
    ];
    let db = Db::open_with(&path, upgraded).expect("upgrade");

    assert_eq!(db.schema_version(), 2);
    assert_eq!(
        ProjectRepo::count(&db).expect("count"),
        1,
        "data must survive the upgrade"
    );
}

#[test]
fn migration_numbers_are_contiguous_from_one() {
    let versions: Vec<i32> = MIGRATIONS.iter().map(|m| m.version).collect();
    let expected: Vec<i32> = (1..=versions.len() as i32).collect();
    assert_eq!(
        versions, expected,
        "migrations are forward-only and numbered without gaps"
    );
}

#[test]
fn repositories_read_through_the_same_connection() {
    let db = Db::open_in_memory().expect("open");
    assert_eq!(ProjectRepo::count(&db).expect("projects"), 0);
    assert_eq!(WorkspaceRepo::count(&db).expect("workspaces"), 0);

    db.with_connection(|conn| {
        conn.execute(
            "INSERT INTO projects (id, name, root_path, created_at, updated_at) \
             VALUES (1, 'p', '/p', 1, 1)",
            [],
        )?;
        conn.execute(
            "INSERT INTO workspaces (project_id, name, created_at, updated_at) \
             VALUES (1, 'w', 1, 1)",
            [],
        )
    })
    .expect("seed");

    assert_eq!(ProjectRepo::count(&db).expect("projects"), 1);
    assert_eq!(WorkspaceRepo::count(&db).expect("workspaces"), 1);
}

#[test]
fn checkpointing_folds_the_write_ahead_log_back_into_the_database() {
    // Mira exits through `app.exit`, which does not run destructors, so nothing
    // drops the connection and SQLite never gets its usual close-time checkpoint.
    // Left alone, every byte written since the last automatic checkpoint sits in
    // `mira.db-wal` — which makes "everything Mira knows is in this one file",
    // the sentence Settings shows a person, untrue. Shutdown calls this.
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("mira.db");
    let db = Db::open(&path).expect("open");

    db.with_connection(|conn| {
        conn.execute(
            "INSERT INTO projects (id, name, root_path, created_at, updated_at) \
             VALUES (1, 'p', '/p', 1, 1)",
            [],
        )
    })
    .expect("write");

    let wal = path.with_file_name("mira.db-wal");
    assert!(
        std::fs::metadata(&wal).map(|m| m.len()).unwrap_or(0) > 0,
        "the write-ahead log should hold the write before a checkpoint"
    );

    db.checkpoint().expect("checkpoint");

    assert_eq!(
        std::fs::metadata(&wal).map(|m| m.len()).unwrap_or(0),
        0,
        "a truncating checkpoint leaves an empty log and a complete database file"
    );

    drop(db);
    let reopened = Db::open(&path).expect("reopen");
    assert_eq!(
        ProjectRepo::count(&reopened).expect("count"),
        1,
        "the checkpointed write survives"
    );
}

#[test]
fn a_version_one_database_gains_the_workspace_context_without_losing_anything() {
    // `data-model.md` §4: every migration is tested against the previous
    // version's database. A person upgrading Mira must not lose the projects and
    // workspaces they already had.
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("mira.db");

    let baseline: Vec<Migration> = MIGRATIONS
        .iter()
        .filter(|migration| migration.version == 1)
        .copied()
        .collect();
    let old = Db::open_with(&path, &baseline).expect("open at v1");
    assert_eq!(old.schema_version(), 1);
    old.with_connection(|conn| {
        conn.execute(
            "INSERT INTO projects (id, name, root_path, created_at, updated_at) \
             VALUES (1, 'Aviora', '/home/dev/aviora', 1, 1)",
            [],
        )?;
        conn.execute(
            "INSERT INTO workspaces (id, project_id, name, created_at, updated_at) \
             VALUES (1, 1, 'Web Development', 1, 1)",
            [],
        )
    })
    .expect("seed at v1");
    drop(old);

    let upgraded = Db::open(&path).expect("upgrade");

    assert_eq!(upgraded.schema_version(), mira_db::target_version());
    let workspace = upgraded
        .get_workspace(mira_core::WorkspaceId::new(1))
        .expect("the workspace survived");
    assert_eq!(workspace.name, "Web Development");
    assert_eq!(
        workspace.description, None,
        "a column added by a migration starts empty, not invented"
    );
    assert_eq!(workspace.last_opened_at, None);
    assert!(workspace.applications.is_empty());
}
