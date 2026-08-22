-- Workspace actions (slice 4d).
--
-- Which of Mira's actions belong to this working context. A workspace already
-- says where the code is, which applications it works with (0003) and which
-- services matter (0004); this is the handful of things it can be asked to do.
--
-- Additive only, and every statement is idempotent, per `data-model.md` §4.

-- One action this workspace has.
--
-- Three columns, and what is absent is the whole point. There is no `program`,
-- no `args`, no `cwd`, no `run_in`, no `label` and no text of any kind that gets
-- executed. `action` is an **identity in a catalogue compiled into the binary**:
-- what it means is decided by Rust, and an id naming no row means nothing at all.
--
-- The CHECK is defence in depth rather than the wall. The wall is that
-- `mira_core::action::find` is the only way an id becomes anything, and the
-- command layer refuses an id that finds nothing *before* storing it. This makes
-- the column additionally unable to hold a slash, a space, a quote, a semicolon
-- or a newline — so whatever else goes wrong, a row here cannot be a path, a
-- program or anything with syntax in it (ADR-0021).
CREATE TABLE IF NOT EXISTS workspace_actions (
  workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  action       TEXT    NOT NULL
               CHECK (length(action) BETWEEN 1 AND 32
                      AND action GLOB '[a-z0-9-]*'
                      AND action NOT GLOB '*[^a-z0-9-]*'),
  added_at     INTEGER NOT NULL,
  PRIMARY KEY (workspace_id, action)
);

-- `commands` from `0001_init.sql` stays empty, and this is the migration that
-- decides it will.
--
-- It was designed for user-defined commands and carries `program TEXT NOT NULL`,
-- `args`, `cwd` and `run_in`. Its own comment already says `args` is a JSON array
-- "never a shell string", which was the right instinct and the wrong altitude:
-- the safe version of this feature has no column a program can live in at all,
-- because the set of programs is compiled in rather than stored.
--
-- So slice 4d does not fill that table. It stores catalogue identities in the
-- table above, and a guard test fails the build if any code reads or writes
-- `commands`. Left in place rather than dropped: dropping is not additive, and an
-- empty table nothing can reach is not a risk. Same treatment as `applications`,
-- `app_preferences` and `expected_ports`.
