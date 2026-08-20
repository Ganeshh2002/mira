-- Mira baseline schema.
--
-- Creates everything in docs/architecture/data-model.md §3 except §3.10 (automation),
-- which is deliberately absent until 0.6+: building empty tables for undesigned
-- features is how schemas rot (product-scope.md §1 rule 2).
--
-- Conventions, per data-model.md §2:
--   * INTEGER PRIMARY KEY surrogate ids
--   * timestamps are INTEGER Unix epoch seconds UTC, suffixed _at
--   * booleans are INTEGER 0|1 with a CHECK
--   * enums are TEXT with a CHECK, because readable database dumps matter more
--     than two bytes
--
-- Applied migrations are never edited. A mistake is corrected by a new migration.

-- §3.1 Projects ---------------------------------------------------------------

CREATE TABLE IF NOT EXISTS projects (
  id             INTEGER PRIMARY KEY,
  name           TEXT    NOT NULL,
  root_path      TEXT    NOT NULL UNIQUE,
  color          TEXT    NOT NULL DEFAULT 'slate',
  icon           TEXT,
  is_git         INTEGER NOT NULL DEFAULT 0 CHECK (is_git IN (0,1)),
  git_root       TEXT,
  sort_order     INTEGER NOT NULL DEFAULT 0,
  last_opened_at INTEGER,
  created_at     INTEGER NOT NULL,
  updated_at     INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_projects_sort ON projects (sort_order, last_opened_at DESC);

CREATE TABLE IF NOT EXISTS project_markers (
  project_id  INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  marker      TEXT    NOT NULL,
  detected_at INTEGER NOT NULL,
  PRIMARY KEY (project_id, marker)
);

-- §3.2 Workspaces -------------------------------------------------------------

CREATE TABLE IF NOT EXISTS workspaces (
  id          INTEGER PRIMARY KEY,
  project_id  INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  name        TEXT    NOT NULL,
  subpath     TEXT,
  is_default  INTEGER NOT NULL DEFAULT 0 CHECK (is_default IN (0,1)),
  sort_order  INTEGER NOT NULL DEFAULT 0,
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL,
  UNIQUE (project_id, name)
);

-- Makes "exactly one default workspace per project" a database guarantee rather
-- than a code convention.
CREATE UNIQUE INDEX IF NOT EXISTS idx_workspace_one_default
  ON workspaces (project_id) WHERE is_default = 1;

CREATE TABLE IF NOT EXISTS project_active_workspace (
  project_id   INTEGER PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
  workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  changed_at   INTEGER NOT NULL
);

-- §3.3 Sessions ---------------------------------------------------------------

CREATE TABLE IF NOT EXISTS sessions (
  id             INTEGER PRIMARY KEY,
  project_id     INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  workspace_id   INTEGER          REFERENCES workspaces(id) ON DELETE SET NULL,
  started_at     INTEGER NOT NULL,
  ended_at       INTEGER,
  active_seconds INTEGER NOT NULL DEFAULT 0,
  media_label    TEXT,
  note           TEXT
);
CREATE INDEX IF NOT EXISTS idx_sessions_project ON sessions (project_id, started_at DESC);

-- Interval boundaries only — never what the user did while away.
CREATE TABLE IF NOT EXISTS session_pauses (
  id         INTEGER PRIMARY KEY,
  session_id INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
  paused_at  INTEGER NOT NULL,
  resumed_at INTEGER,
  cause      TEXT    NOT NULL CHECK (cause IN ('lock','sleep','idle','manual'))
);

-- §3.4 Applications and commands ----------------------------------------------

CREATE TABLE IF NOT EXISTS applications (
  id            INTEGER PRIMARY KEY,
  kind          TEXT    NOT NULL CHECK (kind IN ('editor','terminal','browser','other')),
  name          TEXT    NOT NULL,
  program       TEXT    NOT NULL,
  launch_form   TEXT    NOT NULL CHECK (launch_form IN ('exec','bundle','desktop','flatpak','snap')),
  args_template TEXT,
  supports_line   INTEGER NOT NULL DEFAULT 0 CHECK (supports_line IN (0,1)),
  supports_reuse  INTEGER NOT NULL DEFAULT 0 CHECK (supports_reuse IN (0,1)),
  is_detected   INTEGER NOT NULL DEFAULT 1 CHECK (is_detected IN (0,1)),
  created_at    INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS app_preferences (
  id             INTEGER PRIMARY KEY,
  project_id     INTEGER          REFERENCES projects(id)   ON DELETE CASCADE,
  workspace_id   INTEGER          REFERENCES workspaces(id) ON DELETE CASCADE,
  kind           TEXT    NOT NULL CHECK (kind IN ('editor','terminal','browser','other')),
  application_id INTEGER NOT NULL REFERENCES applications(id) ON DELETE CASCADE,
  CHECK (project_id IS NOT NULL OR workspace_id IS NOT NULL)
);

-- `args` is a JSON ARRAY, never a shell string. The schema itself forbids
-- "just put the whole command line in here", which is how shell injection gets in.
CREATE TABLE IF NOT EXISTS commands (
  id           INTEGER PRIMARY KEY,
  workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  label        TEXT    NOT NULL,
  program      TEXT    NOT NULL,
  args         TEXT    NOT NULL DEFAULT '[]',
  cwd          TEXT,
  run_in       TEXT    NOT NULL DEFAULT 'terminal'
               CHECK (run_in IN ('terminal','detached')),
  sort_order   INTEGER NOT NULL DEFAULT 0,
  created_at   INTEGER NOT NULL
);

-- §3.5 Ports ------------------------------------------------------------------
-- Only *expected* ports are stored — a statement of intent. Live ports are never
-- persisted.

CREATE TABLE IF NOT EXISTS expected_ports (
  id           INTEGER PRIMARY KEY,
  workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  port         INTEGER NOT NULL CHECK (port BETWEEN 1 AND 65535),
  label        TEXT,
  scheme       TEXT    NOT NULL DEFAULT 'http' CHECK (scheme IN ('http','https')),
  path         TEXT    NOT NULL DEFAULT '/',
  sort_order   INTEGER NOT NULL DEFAULT 0,
  UNIQUE (workspace_id, port)
);

-- §3.6 SSH --------------------------------------------------------------------
-- identity_path is a PATH ONLY. The file it names is never opened, read, hashed,
-- or transmitted. No password, passphrase, key material, or known_hosts content
-- has anywhere to live here.

CREATE TABLE IF NOT EXISTS ssh_hosts (
  id            INTEGER PRIMARY KEY,
  project_id    INTEGER          REFERENCES projects(id) ON DELETE CASCADE,
  alias         TEXT    NOT NULL,
  hostname      TEXT,
  username      TEXT,
  port          INTEGER CHECK (port IS NULL OR port BETWEEN 1 AND 65535),
  identity_path TEXT,
  source        TEXT    NOT NULL CHECK (source IN ('ssh_config','manual')),
  probe_enabled INTEGER NOT NULL DEFAULT 0 CHECK (probe_enabled IN (0,1)),
  created_at    INTEGER NOT NULL
);

-- §3.7 Docker -----------------------------------------------------------------
-- References only. Live container state is read from the daemon, never cached.

CREATE TABLE IF NOT EXISTS docker_refs (
  id              INTEGER PRIMARY KEY,
  project_id      INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  compose_file    TEXT,
  compose_project TEXT,
  created_at      INTEGER NOT NULL,
  UNIQUE (project_id, compose_project)
);

-- §3.8 Shelf ------------------------------------------------------------------
-- References, never copies. Mira never copies file content into its database.

CREATE TABLE IF NOT EXISTS shelf_items (
  id         INTEGER PRIMARY KEY,
  project_id INTEGER REFERENCES projects(id) ON DELETE CASCADE,
  path       TEXT    NOT NULL,
  kind       TEXT    NOT NULL DEFAULT 'file' CHECK (kind IN ('file','directory')),
  note       TEXT,
  sort_order INTEGER NOT NULL DEFAULT 0,
  added_at   INTEGER NOT NULL,
  UNIQUE (project_id, path)
);

-- §3.9 Themes and preferences -------------------------------------------------

CREATE TABLE IF NOT EXISTS themes (
  id         INTEGER PRIMARY KEY,
  key        TEXT    NOT NULL UNIQUE,
  name       TEXT    NOT NULL,
  is_builtin INTEGER NOT NULL DEFAULT 1 CHECK (is_builtin IN (0,1)),
  tokens     TEXT    NOT NULL DEFAULT '{}',
  created_at INTEGER NOT NULL
);

-- Single-row-per-key store. Values are JSON, typed and validated in Rust.
CREATE TABLE IF NOT EXISTS preferences (
  key        TEXT PRIMARY KEY,
  value      TEXT NOT NULL,
  updated_at INTEGER NOT NULL
);
