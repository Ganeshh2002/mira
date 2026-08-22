-- Which application a workspace uses (slice 4b).
--
-- `0002_workspace_context.sql` recorded which *kinds* a workspace works with —
-- "this one uses an editor" — and left which editor to whatever the machine had
-- first. This adds the other half: the user's choice, per workspace.
--
-- Additive only, and every statement is idempotent, per `data-model.md` §4.

-- `0001_init.sql` already created `applications` + `app_preferences` from
-- `data-model.md` §3.4: a mutable registry of detected-or-user-added programs,
-- with `program TEXT` holding "an executable path, bundle id, or .desktop id"
-- and an `args_template` of argv parts. **Those two tables stay empty, and this
-- one is why.**
--
-- §5 rule 4 as built is narrower than the sketch: a program is a row in a table
-- compiled into the binary, and nothing outside it can be started. A stored
-- `program` column is a place for an executable path to live, and a preference
-- pointing at a row in it would make "which application" a piece of mutable data
-- rather than an identity. A guard test asserts that no code reads or writes
-- either table, so the empty tables are an enforced state rather than an
-- oversight waiting for someone to fill it in.
--
-- `application_id` is a **catalogue slug** — 'vscode', 'iterm', 'ghostty' — that
-- names a row in `mira-platform`'s compiled list. It is meaningless to anything
-- but Mira, it resolves to nothing if the catalogue does not know it, and it is
-- the same slug on every platform, so a workspace that prefers 'vscode' still
-- prefers it after moving from a Mac to a Linux machine.
--
-- The CHECK is defence in depth rather than the wall: the wall is that an id
-- only ever becomes an application by being found in the compiled catalogue.
-- But a column that cannot hold a slash, a space or a quote is a column nobody
-- has to wonder about.
CREATE TABLE IF NOT EXISTS workspace_app_preferences (
  workspace_id   INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  kind           TEXT    NOT NULL CHECK (kind IN ('editor','terminal','browser')),
  application_id TEXT    NOT NULL
                 CHECK (
                   length(application_id) BETWEEN 1 AND 32
                   AND application_id GLOB '[a-z0-9-]*'
                   AND application_id NOT GLOB '*[^a-z0-9-]*'
                 ),
  chosen_at      INTEGER NOT NULL,

  -- One choice per kind per workspace. Choosing again replaces, and the primary
  -- key is what makes "per workspace" a property of the schema: there is no way
  -- to write a row that two workspaces would both read.
  PRIMARY KEY (workspace_id, kind)
);
