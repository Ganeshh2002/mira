# ADR-0012 — Workspace semantics: stated, not observed; a view, not an action

**Status:** Accepted · 2026-08-20

## Context

`information-architecture.md` §1 has said since before any code existed that
Mira's spine is Project → Workspace → Session. Slice 3 builds the middle one, and
the word "workspace" arrives carrying three other meanings that would each be
wrong here:

- **npm and Cargo** call a set of packages a workspace. Mira calls those
  *packages*, and finds them by reading manifests (ADR-0010).
- **Editors** call the folder you have open a workspace. That is Mira's
  *project*.
- **Window managers** call a set of arranged windows a workspace. That is closer
  to what Mira may *restore* later, and is emphatically not what one is.

Getting this wrong is expensive in a specific way: every later feature — expected
ports, commands, restoration, sessions — hangs off the workspace, and a workspace
that means four things means those features cannot be reasoned about.

## Decision

### A workspace is a user-defined working context on exactly one project

**Project ≠ Workspace ≠ Package ≠ Session**, and the distinction is the decision:

| | What it is | Who makes it | Has a directory |
|---|---|---|---|
| **Project** | where the code lives | the user, by picking a folder | yes, and it is its identity |
| **Workspace** | what they are working on there | the user, by naming it | **no** |
| **Package** | a boundary the repository declares | the repository, in a manifest | yes, but Mira only reads it |
| **Session** | one stretch of actually working | Mira, by observing | no |

A workspace has **no directory of its own** and never requires one to exist. It
is a name, a description, a project, and a set of application kinds. That is the
whole model, and the emptiness is the point: everything else it displays belongs
to the project underneath it.

### Nothing observed is stored on a workspace

No branch column, no port column, no process column — and a test asserts the
absence against the table itself rather than trusting the schema file.

The consequence is the one that matters for §7 and §12 of the brief: **two
workspaces on one project share one set of observations.** They cannot disagree
about the branch, because neither holds a copy; the scheduler observes the
*project* once and every workspace reads that. Switching workspaces costs no
observation, and no scheduler is created per workspace, because there is nothing
per-workspace to observe.

### Opening a workspace is a change of view

Open records *when*, and nothing else. It launches no editor, starts no server,
restores no window. The brief lists that separately as a future restoration
feature, and the ordering is deliberate: launching things on a user's machine
needs a trust model and a confirmation flow, and shipping it accidentally as part
of "make this workspace active" would be the worst way to get it.

The interface says so in words rather than leaving it to be discovered, and there
is no disabled Launch button hinting at what is coming
(`information-architecture.md` §5).

### Application context is a *kind*, and availability is discovered

A workspace stores that it works with an **editor**, a **terminal**, a
**browser** — not that it works with VS Code.

This is the same intent/observation split as everywhere else, and it earns its
keep the moment a workspace moves between machines. The association is the
user's and travels; whether an editor exists here is read fresh by
`mira-platform`, which is what lets a row say "Editor · Not installed" instead of
quietly dropping the association or, worse, claiming an application that is not
there.

It also means `applications` and `app_preferences` (data-model §3.4) stay empty
until the slice that launches things. Those record *which specific application*,
which is a choice this slice deliberately does not ask anyone to make.

### A project's workspaces go with the project

`ON DELETE CASCADE`, chosen rather than inherited.

A workspace describes a project. Without the project there is nothing left for it
to describe, and an orphan is a row the user can neither see nor reach. The
project's removal confirmation names what goes with it, so this is never a
surprise.

**A missing folder is not a missing project.** The folder is observed; the
project and its workspaces are stated. A project whose directory was moved or
deleted keeps everything and shows the missing-folder state (`prd.md` FR-1.5).
Nothing is ever deleted because something went missing on disk.

## Alternatives considered

**A workspace with a subpath.** `0001_init.sql` gave `workspaces` a `subpath`
column for exactly this, and it is still there, unused. Rejected for now because
it makes a workspace *partly* a place, which is the confusion this ADR exists to
prevent — and because a monorepo's packages already answer "which part of the
project", better, from the repository itself. If the column stays unused through
0.2 it should be dropped by a migration rather than left as a suggestion.

**A default workspace per project.** The IA sketches an implicit `Default` that
stays invisible until a second one exists. Rejected for this slice: it would mean
every project silently gains a row, and the empty state ("No workspaces yet")
says more than a workspace nobody asked for.

**Persisting the resolved application.** Store "VS Code" rather than "editor".
Rejected above — it makes the workspace wrong on any machine without that
application, and quietly so.

**Runtime state cached per workspace.** Would let a workspace render without
consulting the project. Rejected: it is a copy that goes stale, and two
workspaces disagreeing about one repository's branch is a bug a user would never
be able to explain.

## Consequences

**Good.** The vocabulary holds, and the table shows it: a workspace is four
stated fields and a relationship. Observations stay single-sourced, so workspaces
are free to add. Restoration has somewhere to attach — the context is already
enumerated (project, packages, applications, services) without any of it having
been acted on.

**Costs.** A workspace cannot yet narrow anything: it shows the whole project's
Git and all of its services, so three workspaces on one project currently differ
only by name, description and application context. That is honest for a
foundation and thin as a feature, and the narrowing (expected ports, per-package
scoping) is what the following slices add. `subpath` sits unused in the schema, a
suggestion nobody has taken.

**Bounded.** `mira-workspaces` depends on `mira-core` and `mira-db` and nothing
else. Deleting it would remove the workspace surfaces and leave projects, Git,
monorepo detection and the scheduler working exactly as they do now.
