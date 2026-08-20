# ADR-0004 — SQLite via rusqlite, owned by Rust

**Status:** Accepted · 2026-08-19

## Context

Mira stores a small amount of durable state: projects, workspaces, sessions, shelf
references, app preferences, commands, settings. Hundreds of kilobytes, one user, one
machine, no sync. It must survive upgrades, be inspectable, and be deletable in one
gesture. It must also never become a way for the UI to gain privileges it was denied
elsewhere.

## Decision

**SQLite**, embedded via **`rusqlite` with the bundled feature**, owned entirely by the
`mira-db` crate behind repository traits, exposed to the frontend only as typed Tauri
commands. Forward-only numbered migrations tracked in `PRAGMA user_version`.

Explicitly **not** `tauri-plugin-sql`.

## Alternatives considered

**`tauri-plugin-sql`** — the obvious path, and the one most Tauri apps take. It exposes
`Database.load()` and `execute(sql, params)` to the frontend over sqlx, with `migrate!`
embedding migrations at compile time. Rejected for two reasons:

1. **It inverts the security model.** [ADR-0006](0006-no-account-no-cloud.md) and the
   security design give the webview *no* ambient authority — no fs, no shell, no http.
   Handing it an arbitrary-SQL channel makes any content-injection bug (a crafted
   filename, a hostile commit message rendered wrong) a full read/write path to every
   piece of stored state. Everything else about the app is designed so that cannot
   happen.
2. **It dissolves the domain boundary.** Queries would live in React components, so the
   schema becomes frontend API and the repository layer stops existing. Migrations would
   then have to preserve compatibility with query strings scattered across the UI.

**sqlx (in Rust only).** Compile-time-checked queries are attractive, but it pulls in an
async runtime for a database we access synchronously in microseconds, and its
compile-time checking wants a live database at build time — friction for contributors.
`rusqlite` is synchronous, tiny, and matches the access pattern.

**JSON or TOML files.** Simple until the first partial write or the first concurrent
access, with no transactions and no migration story. Rejected.

**`tauri-plugin-store`** (key-value). Fine for preferences, insufficient for relational
data (projects → workspaces → sessions) with foreign keys and constraints. We get its
benefit from a `preferences` table instead.

**An embedded key-value store (sled, redb).** No SQL, no external inspectability. SQLite's
ubiquity — any contributor can open the file with any tool — is a real advantage for a
local-first app.

## Consequences

**Good.** Transactions, foreign keys, and constraints enforce invariants in the schema
rather than in code (e.g. one default workspace per project as a partial unique index).
The database is a single inspectable file the user can delete. The bundled build removes
"which SQLite does this OS have" from the support matrix. The frontend cannot run SQL,
which preserves the whole privilege model.

**Bad.** Every query needs a command, a type, and a repository method — more ceremony than
calling SQL from the UI, deliberately. `rusqlite` is synchronous, so database work runs on
a blocking task. Bundling SQLite adds roughly a megabyte. We hand-roll the migration
runner (~100 lines) instead of inheriting sqlx's.

**Follows from this.** Migrations are never edited after release; a mistake is fixed by a
new migration. A newer database opened by an older Mira is refused, not upgraded.
