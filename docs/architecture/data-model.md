# Aviora Mira — Data Model

Status: **in progress.** SQLite, local, single-user, single-file. Slices 0 and 1 use
`projects` and `project_markers`; the rest of the schema is created and unused.

Schema below is the design target for Slice 1–12. Migrations are additive from `0001`;
this document is kept in sync with the migration files, which are the truth once code
exists.

---

## 1. Principles

1. **Local-first, single-user.** There is no `users` table, no `organizations`, no
   `subscriptions`, no `sync_state`, no `device_id`, no `tenant_id`. Adding any of them
   requires an ADR overturning [ADR-0006](../adr/0006-no-account-no-cloud.md).
2. **Store intent, not observation.** Projects, workspaces, shelf references, and
   preferences are things the user *stated*. Ports in use, processes, containers, system
   metrics, and Git state — including commit history — are observed live and never
   written to disk. The one exception is sessions, which are observed, optional, and
   deletable.

   The rule extends to things Mira is *holding* as well as things it has seen. A Keep
   Awake lock is an operating-system power request owned by the running process plus a
   timestamp in memory: there is no table for it and no column, so quitting releases it
   and restarting starts off. A guard test fails the build if a migration ever mentions
   one ([ADR-0014](../adr/0014-keep-awake.md)).

3. **References, never copies.** The Shelf stores paths. Mira never copies file content
   into its database.
4. **No secrets.** No passwords, tokens, keys, passphrases, or environment values are
   stored — ever. SSH `IdentityFile` is stored as a *path string* and never opened.
5. **Migration-friendly.** Additive changes, `user_version` tracking, forward-only, every
   migration idempotent and tested against the previous version's database.
6. **Deletable.** One file. Deleting it returns Mira to first-run. No hidden state
   elsewhere except OS-standard window geometry.

## 2. Storage

| Item | Value |
|---|---|
| Engine | SQLite via `rusqlite` (bundled) |
| File | `<app_data_dir>/mira.db` |
| macOS | `~/Library/Application Support/dev.aviora.mira/mira.db` |
| Windows | `%APPDATA%\dev.aviora.mira\mira.db` |
| Linux | `~/.local/share/dev.aviora.mira/mira.db` (XDG) |
| Journal | WAL |
| Foreign keys | `PRAGMA foreign_keys = ON` on every connection |
| Access | Single connection behind a mutex, in `mira-db` only |

Conventions: `INTEGER PRIMARY KEY` surrogate ids; timestamps are `INTEGER` Unix epoch
seconds UTC (`_at` suffix); booleans are `INTEGER 0|1` with a `CHECK`; enums are `TEXT`
with a `CHECK` constraint, because readable database dumps matter more than two bytes.

---

## 3. Schema

### 3.1 Projects

```sql
CREATE TABLE projects (
  id             INTEGER PRIMARY KEY,
  name           TEXT    NOT NULL,
  root_path      TEXT    NOT NULL UNIQUE,   -- canonical absolute path
  color          TEXT    NOT NULL DEFAULT 'slate',
  icon           TEXT,                      -- optional glyph/emoji identifier
  is_git         INTEGER NOT NULL DEFAULT 0 CHECK (is_git IN (0,1)),
  git_root       TEXT,                      -- worktree root if it differs from root_path
  sort_order     INTEGER NOT NULL DEFAULT 0,
  last_opened_at INTEGER,
  created_at     INTEGER NOT NULL,
  updated_at     INTEGER NOT NULL
);
CREATE INDEX idx_projects_sort ON projects (sort_order, last_opened_at DESC);

-- Detected type markers (package.json, Cargo.toml, …). Facts, cheap to recompute,
-- cached only to keep the project list fast.
CREATE TABLE project_markers (
  project_id  INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  marker      TEXT    NOT NULL,             -- 'node' | 'rust' | 'go' | 'python' | …
  detected_at INTEGER NOT NULL,
  PRIMARY KEY (project_id, marker)
);
```

`root_path UNIQUE` enforces one project per directory. A missing directory is a runtime
state, not a column — Mira stats the path on load.

`git_root` carries the monorepo case: a project may be a package inside a larger
repository, and then the worktree root differs from the project root. It is stored
because it is cheap and stable. What is **not** stored is the package list —
which directories a workspace declares is an observation, it changes whenever
someone edits `pnpm-workspace.yaml`, and a cached copy is one that goes wrong
silently (§1 rule 2). Packages are detected on demand alongside the Git read; see
[ADR-0010](../adr/0010-monorepo-detection.md). **No table holds a detected package,
and turning one into a Project is the user's decision, never a side effect of
looking at a repository.**

### 3.2 Workspaces

```sql
CREATE TABLE workspaces (
  id          INTEGER PRIMARY KEY,
  project_id  INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  name        TEXT    NOT NULL,
  subpath     TEXT,                          -- relative to project root; NULL = root
  is_default  INTEGER NOT NULL DEFAULT 0 CHECK (is_default IN (0,1)),
  sort_order  INTEGER NOT NULL DEFAULT 0,
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL,
  UNIQUE (project_id, name)
);
CREATE UNIQUE INDEX idx_workspace_one_default
  ON workspaces (project_id) WHERE is_default = 1;

-- The workspace currently selected per project (exactly one row per project).
CREATE TABLE project_active_workspace (
  project_id   INTEGER PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
  workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  changed_at   INTEGER NOT NULL
);
```

The partial unique index is what makes "exactly one default workspace per project" a
database guarantee rather than a code convention.

**Migration 0002** adds `description` and `last_opened_at`, and one table:

```sql
-- Which kinds of application a workspace works with. Intent; *which* editor is
-- on this machine is discovered by mira-platform and never written here.
CREATE TABLE workspace_applications (
  workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  kind         TEXT    NOT NULL CHECK (kind IN ('editor','terminal','browser')),
  added_at     INTEGER NOT NULL,
  PRIMARY KEY (workspace_id, kind)
);
```

Three things are **not** here and will not be ([ADR-0012](../adr/0012-workspace-semantics.md)):
a branch, a port, a process. A workspace has no runtime state of its own — Git and
services belong to the project underneath it, are observed once, and are read by every
workspace on that project, so two workspaces can never disagree about one repository.

**Migration 0003** adds `workspace_services` (§3.5). It is not a counter-example: what is
stored is *which of the project's services this workspace cares about*, which is intent,
and the state of each one is resolved live on every read
([ADR-0020](../adr/0020-workspace-services.md)).

`subpath` is from `0001` and is still unused. A workspace is deliberately *not* a place;
if the column is still unused at the end of 0.2 it should be dropped by a migration
rather than left as a suggestion.

**Deletion policy.** A project's workspaces go with it, by `ON DELETE CASCADE`. A
workspace describes a project, so without the project there is nothing left for it to
describe, and the removal confirmation names what goes. A project whose *folder* went
missing keeps everything: the folder is observed, the project and its workspaces are
stated (FR-1.5).

### 3.3 Sessions

```sql
CREATE TABLE sessions (
  id             INTEGER PRIMARY KEY,
  project_id     INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  workspace_id   INTEGER          REFERENCES workspaces(id) ON DELETE SET NULL,
  started_at     INTEGER NOT NULL,
  ended_at       INTEGER,                    -- NULL = open
  active_seconds INTEGER NOT NULL DEFAULT 0, -- excludes locked/asleep intervals
  media_label    TEXT,                       -- only if the user attached it
  note           TEXT
);
CREATE INDEX idx_sessions_project ON sessions (project_id, started_at DESC);

-- Lock/sleep pauses. Interval boundaries only — never what the user did.
CREATE TABLE session_pauses (
  id         INTEGER PRIMARY KEY,
  session_id INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
  paused_at  INTEGER NOT NULL,
  resumed_at INTEGER,
  cause      TEXT    NOT NULL CHECK (cause IN ('lock','sleep','idle','manual'))
);
```

Session recording is a preference. With it off, no row is written. Sessions are
individually and bulk deletable from Settings → Privacy.

### 3.4 Applications and commands

```sql
-- Global registry of launchable applications (detected or user-added).
CREATE TABLE applications (
  id           INTEGER PRIMARY KEY,
  kind         TEXT    NOT NULL CHECK (kind IN ('editor','terminal','browser','other')),
  name         TEXT    NOT NULL,
  program      TEXT    NOT NULL,   -- executable path, bundle id, or .desktop id
  launch_form  TEXT    NOT NULL CHECK (launch_form IN ('exec','bundle','desktop','flatpak','snap')),
  args_template TEXT,              -- JSON array of argv parts with {path} {file} {line} {url}
  supports_line     INTEGER NOT NULL DEFAULT 0 CHECK (supports_line IN (0,1)),
  supports_reuse    INTEGER NOT NULL DEFAULT 0 CHECK (supports_reuse IN (0,1)),
  is_detected  INTEGER NOT NULL DEFAULT 1 CHECK (is_detected IN (0,1)),
  created_at   INTEGER NOT NULL
);

-- Per-project / per-workspace app preferences. workspace_id NULL = project-level default.
CREATE TABLE app_preferences (
  id             INTEGER PRIMARY KEY,
  project_id     INTEGER          REFERENCES projects(id)   ON DELETE CASCADE,
  workspace_id   INTEGER          REFERENCES workspaces(id) ON DELETE CASCADE,
  kind           TEXT    NOT NULL CHECK (kind IN ('editor','terminal','browser','other')),
  application_id INTEGER NOT NULL REFERENCES applications(id) ON DELETE CASCADE,
  CHECK (project_id IS NOT NULL OR workspace_id IS NOT NULL)
);

-- User-defined commands surfaced as buttons on a workspace.
CREATE TABLE commands (
  id           INTEGER PRIMARY KEY,
  workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  label        TEXT    NOT NULL,
  program      TEXT    NOT NULL,
  args         TEXT    NOT NULL DEFAULT '[]',  -- JSON array; NEVER a shell string
  cwd          TEXT,                            -- relative to workspace root
  run_in       TEXT    NOT NULL DEFAULT 'terminal'
               CHECK (run_in IN ('terminal','detached')),
  sort_order   INTEGER NOT NULL DEFAULT 0,
  created_at   INTEGER NOT NULL
);
```

`args` is a JSON **array**, not a string, at the schema level. The data model itself
forbids "just put the whole command line in here", which is how shell injection gets in.

### 3.5 Ports

Only *expected* ports are stored — a statement of intent. Live ports are never persisted.

**Migration 0003** is what a workspace actually watches:

```sql
CREATE TABLE workspace_services (
  id           INTEGER PRIMARY KEY,
  workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  port         INTEGER NOT NULL CHECK (port BETWEEN 1 AND 65535),
  added_at     INTEGER NOT NULL,
  UNIQUE (workspace_id, port)
);
CREATE INDEX idx_workspace_services_workspace ON workspace_services (workspace_id);
```

Four columns, and the absences are the design. No label, no process name, no pid, no
address, no scheme, no path, no command. Everything except the port is *observation*, it
belongs to the project, and it is read live on every request (§1 rule 2) — so a watched
service that has stopped is shown as its port and nothing more, rather than as a memory
of what used to be listening there.

There is no separate "expected port" record. **A workspace expects a service because it
saw it once:** the row that says *5173 matters here* is the same row whether or not
anything is listening, and the difference between expected and running is a state
resolved on each read against the scheduler's own observation
([ADR-0020](../adr/0020-workspace-services.md)).

`UNIQUE (workspace_id, port)` rather than `UNIQUE (port)`: two workspaces on one project
may intentionally watch the same shared dev server.

**Deletion policy.** A workspace's services go with the workspace, which goes with the
project — two `ON DELETE CASCADE` hops, both tested. A service left behind by a deleted
workspace is a row nobody can see or reach.

The `0001` table below **stays empty**:

```sql
CREATE TABLE expected_ports (
  id           INTEGER PRIMARY KEY,
  workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  port         INTEGER NOT NULL CHECK (port BETWEEN 1 AND 65535),
  label        TEXT,
  scheme       TEXT    NOT NULL DEFAULT 'http' CHECK (scheme IN ('http','https')),
  path         TEXT    NOT NULL DEFAULT '/',
  sort_order   INTEGER NOT NULL DEFAULT 0,
  UNIQUE (workspace_id, port)
);
```

It was designed before the security rules were written, and `scheme` + `path` exist to be
concatenated into a URL. A stored path of `@example.invalid/` turns
`http://localhost:3000` into `http://localhost:3000@example.invalid/` — a request to a
remote host wearing a loopback address. Mira builds `http://localhost:<port>` in Rust
from a port and nothing else, so neither column has anywhere to be used, and a guard test
fails the build if any code reads or writes this table. Left in place rather than dropped
because dropping is not additive and an unreachable empty table is not a risk.

### 3.6 SSH

```sql
CREATE TABLE ssh_hosts (
  id            INTEGER PRIMARY KEY,
  project_id    INTEGER          REFERENCES projects(id) ON DELETE CASCADE,
  alias         TEXT    NOT NULL,           -- Host from ~/.ssh/config, or user-entered
  hostname      TEXT,
  username      TEXT,
  port          INTEGER CHECK (port IS NULL OR port BETWEEN 1 AND 65535),
  identity_path TEXT,                        -- PATH ONLY. The file is never opened.
  source        TEXT    NOT NULL CHECK (source IN ('ssh_config','manual')),
  probe_enabled INTEGER NOT NULL DEFAULT 0 CHECK (probe_enabled IN (0,1)),
  created_at    INTEGER NOT NULL
);
```

No password, passphrase, key material, or `known_hosts` content. `probe_enabled`
defaults to 0: no network activity until the user asks, per host.

### 3.7 Docker

```sql
-- References only. Live container state is read from the daemon, never cached to disk.
CREATE TABLE docker_refs (
  id             INTEGER PRIMARY KEY,
  project_id     INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  compose_file   TEXT,                        -- path relative to project root
  compose_project TEXT,                       -- com.docker.compose.project label
  created_at     INTEGER NOT NULL,
  UNIQUE (project_id, compose_project)
);
```

### 3.8 Shelf

```sql
CREATE TABLE shelf_items (
  id         INTEGER PRIMARY KEY,
  project_id INTEGER REFERENCES projects(id) ON DELETE CASCADE,  -- NULL = global shelf
  path       TEXT    NOT NULL,                -- canonical absolute path
  kind       TEXT    NOT NULL DEFAULT 'file' CHECK (kind IN ('file','directory')),
  note       TEXT,
  sort_order INTEGER NOT NULL DEFAULT 0,
  added_at   INTEGER NOT NULL,
  UNIQUE (project_id, path)
);
```

`UNIQUE (project_id, path)` treats NULL project ids as distinct in SQLite, so the global
shelf permits duplicates of a path already shelved in a project — which is the intended
behaviour (different scope, different purpose).

### 3.9 Themes and preferences

```sql
CREATE TABLE themes (
  id          INTEGER PRIMARY KEY,
  key         TEXT    NOT NULL UNIQUE,  -- 'minimal' | 'cosmic' | 'sakura' | 'cyberpunk' | 'rain' | custom
  name        TEXT    NOT NULL,
  is_builtin  INTEGER NOT NULL DEFAULT 1 CHECK (is_builtin IN (0,1)),
  tokens      TEXT    NOT NULL DEFAULT '{}', -- JSON design-token overrides
  created_at  INTEGER NOT NULL
);

-- Single-row key/value store. Typed and validated in Rust, not by the schema.
CREATE TABLE preferences (
  key        TEXT PRIMARY KEY,
  value      TEXT NOT NULL,          -- JSON
  updated_at INTEGER NOT NULL
);
```

Known preference keys (validated in `mira-core`, defaults in parentheses):
`appearance.theme` (`system`), `appearance.accent` (`indigo`), `appearance.atmosphere`
(`minimal`), `appearance.density` (`comfortable`), `shortcut.toggle` (per-OS default),
`startup.autostart` (`false`), `privacy.record_sessions` (`true`),
`privacy.telemetry` (`false`, and there is no code that reads it as `true`),
`updates.check` (unset until the user answers at first run), `media.enabled` (`false`),
`ssh.parse_config` (`false`), `docker.socket_path` (unset = autodetect),
`polling.interval_seconds` (`5`).

### 3.10 Automation *(0.6+ — not created before then)*

Sketched only, so a later migration does not have to fight the existing shape. **No
`automations` table is created before 0.6+**; building empty tables for undesigned
features is how schemas rot. This is the schema-level statement of
[product-scope.md](../product/product-scope.md) §1 rule 2.

```sql
-- 0.6+, NOT CREATED BEFORE THEN:
-- automations(id, project_id, workspace_id, name, trigger_kind, trigger_config JSON,
--             is_enabled, created_at)
-- automation_actions(id, automation_id, sort_order, action_kind, action_config JSON)
```

Automation will require a trust model before a schema — an action list that can launch
processes is exactly the thing an attacker would want to write to.

---

## 4. Migrations

- Forward-only, numbered `NNNN_description.sql`, embedded in the binary.
- `PRAGMA user_version` holds the applied version; the runner applies each pending file
  in a transaction and bumps it.
- Every migration is idempotent (`IF NOT EXISTS`) and is tested by: build the previous
  version's database, apply, assert the schema and that data survived.
- Applied migrations are never edited. A mistake is corrected by a new migration.
- Baseline `0001_init.sql` creates everything in §3 except §3.10.

Backward compatibility: a **newer** database opened by an **older** Mira is detected via
`user_version` and refused with a clear message rather than being corrupted.

---

## 5. What is deliberately not stored

| Not stored | Why |
|---|---|
| Users, accounts, orgs, subscriptions | Single-user local app; [ADR-0006](../adr/0006-no-account-no-cloud.md) |
| Any credential, token, key, or passphrase | Mira never authenticates to anything |
| Environment variable values | Frequently secret; displayed live at most, never persisted |
| File contents | The Shelf and Peek reference paths |
| Live ports, processes, containers, system metrics | Observed, ephemeral, cheap to recompute |
| Git objects, diffs, or history | The repository is the source of truth |
| Command output or logs of launched apps | Mira spawns and forgets |
| Telemetry, usage counters, analytics | [ADR-0006](../adr/0006-no-account-no-cloud.md) |
| Anything about what the user did while locked | Session pauses store boundaries only |

## 6. Size and performance

Expected steady state: tens of projects, hundreds of shelf items, thousands of sessions —
a database measured in **hundreds of kilobytes**. Every query in the hot path
(project list, workspace resolution, shelf) is indexed and returns in under a millisecond.
No query in the product needs a join across more than three tables. If one ever does, the
model has drifted from "local notes about the user's own machine" and should be
questioned.
