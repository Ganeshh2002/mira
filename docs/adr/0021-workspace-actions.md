# ADR-0021 — Workspace actions: a compiled catalogue, an identity, and no command

**Status:** Accepted · 2026-08-22

## Context

A workspace now says where the code is, which application kinds it works with
([ADR-0012](0012-workspace-semantics.md)), which specific applications
([ADR-0019](0019-application-preferences.md)) and which services matter
([ADR-0020](0020-workspace-services.md)). The remaining thing a working context
has is a handful of **things you do** — and `roadmap.md` slice 4 has listed
"per-workspace commands" and "app groups" since before any code existed.

That feature is the most dangerous one in the product, and it is dangerous in a
quiet way. "Let a workspace keep the commands you run in it" is a sentence
everybody nods at, and the obvious implementation is the one `0001_init.sql`
already anticipated:

```sql
CREATE TABLE commands (
  ..., program TEXT NOT NULL, args TEXT NOT NULL DEFAULT '[]', cwd TEXT,
  run_in TEXT NOT NULL CHECK (run_in IN ('terminal','detached')), ...
);
```

Its own comment says `args` is a JSON array "never a shell string", which was the
right instinct at the wrong altitude. An argv array is safer than a command
line, but a table with `program TEXT NOT NULL` in it is still a table where the
answer to *what may Mira start?* is "whatever is written here" — and everything
that writes there becomes security-critical: the IPC boundary, the database file,
any future import, any future sync.

`security-and-privacy.md` §5 rule 4 already narrowed this once, for launching:

> **As built (Slice 4), it is narrower than that:** a program is a row in a table
> compiled into the binary, and nothing outside that table can be started.

This ADR applies the same narrowing to actions, and states the boundary in the
form that survives being read quickly.

## Decision

### A workspace action is a row in a catalogue compiled into the binary

```rust
pub struct Action {
    pub id: &'static str,        // "open-editor"
    pub label: &'static str,     // "Open in the editor"
    pub describes: &'static str, // what it will actually do, in a sentence
    pub icon: &'static str,
    pub effect: Effect,
}

pub const CATALOGUE: [Action; 6] = [ /* six rows */ ];
```

`&'static str` throughout, in a `const` array. There is no way to add a row
except to write one and compile it. Not from the interface, not from the
database, not from a project directory, not from a settings file — those paths do
not exist rather than being validated.

### What a workspace action is **not**

Stated as absences, because that is how it will be read in two years:

- **not a program.** There is no field, column, parameter or wire type anywhere in
  this feature that holds an executable, a path to one, or a bundle id.
- **not an argument list.** No `args`, no `argv`, no template, no `{placeholder}`
  substitution, no user-supplied fragment of any kind.
- **not a shell string,** and no shell operator can occur in anything stored: an
  `ActionId` is `[a-z0-9-]{1,32}`, so `;`, `&&`, `|`, `` ` ``, `$(`, quotes,
  spaces, slashes and newlines are not characters it can contain.
- **not a package-manager invocation.** `npm run dev`, `pnpm -w build` and
  `cargo run` are not values this feature can hold; the wire tests assert each of
  them fails to deserialise.
- **not a terminal that gets typed into.** "Open a terminal here" opens one at the
  project root. Nothing is sent to it — and the catalogue row says so, in the
  sentence a person reads before pressing it.
- **not a way to stop anything.** There is no `Stop`, `Kill`, `Restart` or
  `Terminate` effect, and there is no parameter through which one could be asked
  for.

### The effect enum is the whole privilege surface

```rust
pub enum Effect {
    OpenIn { kind: AppKind },  // the project root, in the workspace's editor/terminal
    RevealProject,             // the file manager
    OpenService,               // this workspace's running service, in the browser
    MarkOpened,                // record that this is the workspace being worked in
    Observe,                   // take the scheduler's reading now
}
```

Five variants, and **every one of them was already reachable from a button
somewhere in Mira before this slice.** That is the claim that makes the feature
safe to add at all: actions are not a new way to affect the machine, they are a
way for a workspace to say which existing ones are its own.

It is enforced rather than asserted. The dispatch calls
`commands::workspaces::launch_at_root` and `commands::services::open_watched` —
the shared bodies of commands that were already reviewed — so there is still
exactly **one** `LaunchTarget::Directory` construction site and exactly one
address construction site in the application shell, and the guards that count
them still pass unchanged. A guard additionally asserts the variant list
*exactly*, so a sixth effect fails the build until somebody adds it here on
purpose.

### A workspace stores the identity, and only Rust knows what it means

`workspace_actions(workspace_id, action, added_at)`. The `action` column holds a
catalogue id and `mira_core::action::find` is the only thing that turns one into
an `Effect`. `workspaces.set_action` refuses an id that finds nothing **before
storing it**, so the database can only hold identities Mira compiled in.

### Named by identity, not by ordinal — and why that differs from services

[ADR-0020](0020-workspace-services.md) has the interface point into a service
list by **position**, and this one points into the catalogue by **identity**.
The difference is what the list is made of. A service list is *observed*: it
moves under the caller, so a position is the only stable way to point into it and
a stale position must be a refusal. A catalogue is *compiled in*: it cannot move
between the render and the click, so an identity is both stable and far more
legible in a log, an error message and a database dump.

Both shapes have the same property, which is the one that matters: **the
interface can only ask for something Mira already offered.**

### Unknown is a state, never a substitution

A workspace can hold an id this build has no row for — it was given one by a
later version, or a row was removed. That resolves to `ActionState::Unknown`, and
three things follow: it stays on the list rather than vanishing, it is shown by
its id with a sentence saying Mira has no such action, and it offers removal but
not performance. It is **never** matched to the nearest row. A guard asserts the
unknown arm reaches for no fallback (`CATALOGUE[0]`, `first()`, `unwrap_or`).

### An ambiguous action is impossible, not disabled

"Open the running service" is available only when **exactly one** of this
workspace's watched services is running. Zero is unavailable with a reason. Two
or more is unavailable *with the count*, and the sentence points at the Services
list.

This is the brief's "make destructive/ambiguous actions impossible rather than
merely disabled", and it is the one place the rule had teeth: opening the first
of three running services would have been convenient, silent, and wrong.

Unavailability is rendered as a sentence rather than a greyed-out button
throughout (`information-architecture.md` §5): a greyed control promises a later
release, and a machine with no editor is not waiting for anything.

### Measured before the model was chosen

Two candidate shapes for availability: ask the machine once per **request**, or
once per **action**. Measured on a release build:

| | cost |
|---|---|
| resolve six actions, given the answers | **0.646 µs** |
| `Applications::openable()` — what can open a folder | 6.975 µs |
| `Path::is_dir()` — is the project folder there | 1.016 µs |
| gathered once per request | **7.991 µs** |
| asked once per action, six actions | 47.946 µs |

The machine survey is **twelve times** the cost of deciding all six actions, so
the resolution takes a `Support` value gathered once by the caller. Asking per
action would have been six times the work for the same answer.

The catalogue lookup is a linear walk of six rows: **7 ns** for a hit, 4 ns for a
full miss. It needs no index and will not need one at any size this catalogue
should ever reach.

At 8 µs a request there is nothing worth keeping between requests, so there is no
cache and therefore **no new clock and no new observer** (ADR-0011).

### `commands` stays empty

The table exists, has `program TEXT NOT NULL`, and is now permanently unused. A
guard fails the build if any code issues `FROM`, `INTO`, `UPDATE` or `JOIN`
against it. Left in place rather than dropped — dropping is not additive, and an
empty table nothing can reach is not a risk. Same treatment as `applications`,
`app_preferences` and `expected_ports`.

### The migration numbers

PR #7 and PR #8 were both cut from the same `dev` and both numbered their
migration `0003`. They are independent — one adds `workspace_app_preferences`,
the other `workspace_services` — so either order is correct. The order taken is
the order the PRs were opened in:

| version | migration |
|---|---|
| 0003 | `application_preferences` (PR #7) |
| 0004 | `workspace_services` (PR #8, renumbered) |
| 0005 | `workspace_actions` (this slice) |

Asserted by a test rather than described in prose, because "we renumbered it" is
the kind of claim that rots.

## Alternatives considered

**The `commands` table as designed.** Rejected above. An argv array is safer than
a command line and still leaves `program` as a stored answer to "what may Mira
start".

**A catalogue with parameters** — "run script X from package.json". Rejected for
this slice and probably for good: reading a script name out of a project
directory makes the repository an input to what Mira will start, which
`security-and-privacy.md` §5 rule 7 forbids by name. If it is ever wanted, it
needs its own ADR and its own trust model, not a field here.

**Command *groups* that fire several actions at once.** The roadmap's "app
groups". Not built: a button that starts four things is harder to reason about
than four buttons, and it is really the restoration feature slice 11 owns, which
needs its own confirmation design. Six single actions first.

**A default set of actions on every new workspace.** Rejected on the same grounds
ADR-0012 rejected an implicit default workspace: every workspace would silently
gain rows nobody asked for. The Open with row already gives a workspace its basic
launches, so an empty Actions section costs nothing.

**Opening the first of several running services.** The convenient reading of the
ambiguous case, and the one this ADR exists to refuse.

**An ordinal into the catalogue, matching ADR-0020.** Rejected: the reason
services use an ordinal is that their list moves. A compiled catalogue does not,
and an identity reads better everywhere a value is ever seen.

## Consequences

**Good.** A workspace finally has a *doing* dimension, and it arrived without
widening the boundary: the set of things Mira can do is byte-for-byte what it was
before, and the guards that count the launch and address construction sites still
pass unchanged. The IPC surface grew by four commands, all of which accept only
identities. Adding an action later is one `const` row.

**Costs.** The catalogue is six rows and cannot be extended by a user. Somebody
who wants a "start the dev server" button will not get one, this release or
probably any release — that is the boundary working, not a gap, and the ADR says
so in its own words so nobody re-litigates it from the roadmap line alone.

An action list nobody curates is empty, so the feature does nothing until someone
adds to it. That is deliberate.

**Bounded.** `mira_core::action` depends on nothing outside `mira_core`. Deleting
the table, the four commands and one component would leave every other surface
exactly as it is, including everything the actions dispatch to.
