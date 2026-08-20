-- Workspace context (slice 3).
--
-- `0001_init.sql` created `workspaces` with everything a *configuration* needs:
-- a name, a subpath, a default flag, an order. Making a workspace something a
-- person opens and returns to adds two facts about it, and one relationship.
--
-- Additive only, and every statement is idempotent, per `data-model.md` §4.

-- A sentence the user wrote about what this workspace is for. Optional, and
-- absent is the normal case: a good name usually says enough.
ALTER TABLE workspaces ADD COLUMN description TEXT;

-- When it was last opened. Nullable because "never opened" is a real state and
-- is not the same as "opened at the moment it was created" — the list orders by
-- this, and a workspace you have never opened should not sort as if you had.
ALTER TABLE workspaces ADD COLUMN last_opened_at INTEGER;

CREATE INDEX IF NOT EXISTS idx_workspaces_recent
  ON workspaces (project_id, last_opened_at DESC);

-- Which kinds of application belong to a workspace.
--
-- **Intent, not observation.** The row says "this workspace works with an
-- editor"; *which* editor is on this machine is discovered at runtime by
-- `mira-platform` and never written here. That split is what lets Mira say
-- "Editor — not installed" honestly: the association survives, the availability
-- is read fresh, and moving to a machine without VS Code does not silently edit
-- the user's workspace.
--
-- `applications` and `app_preferences` (§3.4) stay empty until the slice that
-- launches things. They record *which specific application*, which is a choice
-- this slice deliberately does not ask anyone to make.
CREATE TABLE IF NOT EXISTS workspace_applications (
  workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  kind         TEXT    NOT NULL CHECK (kind IN ('editor','terminal','browser')),
  added_at     INTEGER NOT NULL,
  PRIMARY KEY (workspace_id, kind)
);
