-- Workspace services (slice 4c).
--
-- Which of the project's observed services this workspace actually cares about.
-- A workspace on a monorepo currently shows every listening socket the project
-- owns; this is the table that lets it show two.
--
-- Additive only, and every statement is idempotent, per `data-model.md` §4.

-- A port this workspace watches.
--
-- Four columns, and the absences are the design. There is no label, no process
-- name, no pid, no address, no scheme, no path, no command and no argument:
-- everything except the port is *observation*, it belongs to the project, and it
-- is read live on every request (`data-model.md` §1 rule 2). A watched service
-- that has stopped is therefore shown as its port and nothing more, because
-- anything else Mira could have shown would be a memory of something no longer
-- true.
--
-- The port is not a number the interface may send. A service is added by naming
-- a position in the list of services Mira already observed for this workspace's
-- project, and is opened or forgotten by the row id below. Nothing crossing the
-- IPC boundary carries a port, an address or a URL (ADR-0020,
-- `security-and-privacy.md` §5 rule 5).
CREATE TABLE IF NOT EXISTS workspace_services (
  id           INTEGER PRIMARY KEY,
  workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  port         INTEGER NOT NULL CHECK (port BETWEEN 1 AND 65535),
  added_at     INTEGER NOT NULL,
  UNIQUE (workspace_id, port)
);

-- Every read is "what does this workspace watch", so the workspace is the index.
-- The UNIQUE above already covers (workspace_id, port) and would serve the same
-- lookup; it is written out because the uniqueness is a rule about duplicates
-- and this is a statement about the query, and a later migration that relaxed
-- one should not silently change the other.
CREATE INDEX IF NOT EXISTS idx_workspace_services_workspace
  ON workspace_services (workspace_id);

-- `expected_ports` from `0001_init.sql` stays empty, and this is the migration
-- that decides it will.
--
-- It was designed before the security rules were written, and it carries
-- `scheme TEXT` and `path TEXT NOT NULL DEFAULT '/'` — two columns that exist to
-- be concatenated into a URL. That is the arbitrary-URL shape §5 rule 5 forbids,
-- and it is not theoretical: a stored path of `@example.invalid/` turns
-- `http://localhost:3000` + path into `http://localhost:3000@example.invalid/`,
-- which is a request to a remote host wearing a loopback address. Mira builds
-- `http://localhost:<port>` in Rust from a port and nothing else, so there is
-- nowhere for either column to be used, and a guard test asserts that no code
-- reads or writes this table.
--
-- Left in place rather than dropped: dropping a table is not additive, and an
-- empty table nothing can reach is not a risk. It goes when a migration has a
-- reason of its own to touch it.
