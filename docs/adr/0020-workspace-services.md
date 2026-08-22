# ADR-0020 — Workspace services: a view over shared observation, named by identity

**Status:** Accepted · 2026-08-22

## Context

[ADR-0012](0012-workspace-semantics.md) closed with a cost it named honestly:

> A workspace cannot yet narrow anything: it shows the whole project's Git and
> all of its services, so three workspaces on one project currently differ only
> by name, description and application context.

That is the gap this slice closes. A monorepo running a web server, an API, a
worker queue, a database and a mailhog stub puts five rows on a workspace whose
work is two of them, and the surface stops being about the work.

The feature is easy to describe and easy to build badly. "Let a workspace say
which ports it cares about" is one text field away from a settings screen where
somebody types `3000`, and one column away from a table with `scheme`, `path` and
eventually `command` in it. `0001_init.sql` already contains that table —
`expected_ports`, with `scheme TEXT` and `path TEXT NOT NULL DEFAULT '/'` — written
before the security rules were.

Three constraints shape everything below.

1. **No raw port, address, URL, pid or process name may cross the IPC boundary
   inbound** (`security-and-privacy.md` §5 rule 5). Today `live.open_service`
   takes `port: u16`, checked against the observed list. The check is real; the
   parameter is still a number of the caller's choosing.
2. **A workspace has no runtime state** (ADR-0012). Whatever is stored must be
   *intent*, and everything observed must stay observed.
3. **One scheduler owns every clock** (ADR-0011). Whatever this costs, it must
   not be a per-workspace poller.

## Decision

### A workspace stores one port per watched service, and nothing else

```sql
CREATE TABLE workspace_services (
  id           INTEGER PRIMARY KEY,
  workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  port         INTEGER NOT NULL CHECK (port BETWEEN 1 AND 65535),
  added_at     INTEGER NOT NULL,
  UNIQUE (workspace_id, port)
);
```

No label, no process name, no pid, no address, no scheme, no path, no command.
Everything except the port is observation, it belongs to the project, and it is
read live.

The consequence is visible and intended: **a watched service that has stopped is
shown as its port and nothing else.** There is no "was: vite" and no remembered
name, because both would be a memory of something that is no longer true. What a
person gets back when the server comes up is the live reading, which is the only
kind Mira has.

### Expecting a port is what selecting an observed service *means*

There is no field to type a port into, and there is no separate "expected ports"
concept beside the watched ones. **A workspace expects a service because it saw
it once.** The row that says *5173 matters here* is the same row whether or not
anything is listening; the difference between "expected" and "running" is a
state resolved on every read, not two kinds of record.

That is what makes goal 2 and the no-raw-port rule compatible rather than
opposed. The stored thing is a typed port (`mira_core::Port`, validated 1–65535
where it deserialises and where it is read from the database); the *chosen* thing
is a position in a list Mira produced.

### Nothing inbound carries a port — a service is named by position, then by id

Two shapes, both already used elsewhere in the codebase:

| To do this | The interface sends | Resolved by |
|---|---|---|
| add a service | `at` — a position in the offer list | the same function that built the list |
| open or forget one | `service_id` — the row id Mira issued | a query keyed by row **and** workspace |

`at` is rule 31's ordinal, applied to a port instead of a file: the interface can
ask for something Mira already decided to offer, and an ordinal past the list is
a stale selection rather than an attempt at anything. `service_id` is a row id,
like `workspace_id` and `project_id` before it.

`live.open_service` changed to match: it took `port: u16`, it now takes `at:
u32`. After this slice **no command anywhere accepts a port, an address, a host,
a URL, a pid or a process name**, and a guard test enumerates all of them.

The direction is asserted deliberately. A port travels *outward* on every
reading — the Services panel says `:5173`, and hiding it would make the panel
unreadable. Outbound values are information; inbound values are instructions.

### Five states, because there are five different things to say

```rust
Running { address, process, pid }   // listening now, in this project
NotRunning                          // Mira looked; nothing is here
Taken { process }                   // something is here, and it is not yours
NeverObserved                       // Mira has not read the socket table yet
Unreadable { reason }               // Mira tried and could not
```

`NotRunning` is a **claim**, and Mira may only make it after looking. Before the
first scan and after a failed one the row says so instead, because telling
somebody their server is down when the truth is that Mira could not look is the
same failure as [ADR-0018](0018-history-filters.md)'s "no results" for a search
that ran out of budget.

`Taken` is the anti-substitution state and the one worth the extra variant. A
watched service resolves to `Running` only when the listener on that port is
attributed to **this workspace's project**. If the dev server stopped and
Postgres took 5173, the row says so and offers no Open button. Reporting a
stranger's process as your service — and inviting somebody to open it — is
exactly what [ADR-0013](0013-launching-applications.md) forbids for
applications, one layer over.

A listener Mira could not attribute at all is also `Taken`, not `Running`.
Windows cannot read another process's working directory, so attribution there is
Degraded; "something is here and Mira cannot tell whose" is not a service this
workspace can claim.

### Resolution is a pure function, called on every read

`resolve(watched, project_id, observed) -> Vec<WorkspaceService>` takes the
stored rows and the reading the scheduler already took. No cache, no observer,
no timer, no per-workspace anything. Watching a service is rows in a table; the
state is computed when somebody looks.

### Measured before the data model was chosen

The one place this could have gone wrong is the lookup. Two shapes were written
and timed on a release build:

| watched | listening | scan | index |
|---|---|---|---|
| 1 | 8 | **0.003 µs** | 0.078 µs |
| 4 | 64 | **0.069 µs** | 0.336 µs |
| 4 | 512 | **0.565 µs** | 2.666 µs |
| 20 | 512 | 2.517 µs | **2.394 µs** |
| 20 | 4 096 | 37.143 µs | **19.264 µs** |

**The measurement reversed the decision.** Building one `HashMap` of the
listeners and looking each watched service up is O(watched + listening) and looks
like the obvious answer; it loses everywhere a real machine lives, because the
map has to be built over *every* listening socket whether the workspace watches
one service or twenty, and hashing a `u16` costs more than comparing one. The
crossover is around twenty watched services and five hundred sockets.

Whole-workspace resolution, for the record:

| watched | listening | resolve | fifty workspaces |
|---|---|---|---|
| 4 | 64 | 0.104 µs | 0.005 ms |
| 20 | 512 | 2.611 µs | 0.131 ms |
| 20 | 4 096 | 38.551 µs | 1.928 ms |

Fifty workspaces watching twenty services each, against a machine with four
thousand listening sockets, is under two milliseconds in total. There is nothing
here worth caching, which is why there is no clock to own.

On the machine this was written on — thirteen listening sockets, read through the
real socket table — the scan itself costs **2.5 ms**, attribution another
**1.0 ms**, and resolving four watched services against the result **667 ns**.
The reading dominates by three orders of magnitude, and the scheduler already
takes it once for everybody.

### `expected_ports` stays empty

The `0001` table carries `scheme` and `path` — two columns whose only purpose is
to be concatenated into a URL. That is not a theoretical objection: a stored path
of `@example.invalid/` turns `http://localhost:3000` + path into
`http://localhost:3000@example.invalid/`, which is a request to a remote host
wearing a loopback address. Mira builds `http://localhost:<port>` in Rust from a
port and nothing else, so neither column has anywhere to be used, and a guard
test asserts that no code reads or writes the table.

Left in place rather than dropped: dropping is not additive, and an empty table
nothing can reach is not a risk. This is the same treatment `commands`,
`applications` and `app_preferences` already have.

## Alternatives considered

**A port field somebody types into.** The obvious feature, and the one thing the
brief forbids. It is also worse than it looks: a typed port is one Mira has never
seen, so every state it could be in is `NotRunning` or `Taken`, and Open would be
offering to launch a browser at a number with nothing behind it.

**An opaque token minted per observed service.** Genuinely unforgeable, and
useless: the token would have to survive a restart to persist a selection, and
the map that gave it meaning lives in memory. Every selection would read as
"never observed" after a restart.

**Storing what was running, so a stopped service can still be named.** Rejected
by `data-model.md` §1 rule 2, and by what it would look like: a row saying
`:5173 vite` when vite has not run for a week is a claim Mira cannot support.

**Reusing `expected_ports`.** Rejected above. The columns are the problem, and
adding a `CHECK` that they stay empty would be a stranger artefact than a new
table.

**Matching a watched service by port alone, ignoring attribution.** Simpler, and
it is the substitution bug: the row would go green the moment anything bound the
number.

**A per-workspace refresh of just its own services.** Sounds thrifty, is a second
clock and N reads of one socket table (ADR-0011). The measurement removed the
motive.

**Keeping the project's full service list on the workspace surface as well.**
Rejected: "unselected project services should not clutter the workspace surface"
is the whole feature. The full list is one click away on the project surface,
unchanged.

## Consequences

**Good.** A workspace finally narrows something, which is what ADR-0012 said it
could not. Two workspaces on one repository can watch different services, and
neither can see or change the other's list — the delete and the read are keyed by
row *and* workspace, so a sibling's id resolves to nothing rather than to a row.
The IPC boundary got narrower rather than wider: a command that used to take a
port no longer does, and no command takes one anywhere.

**Costs.** You cannot expect a port you have never seen. Somebody who knows their
API will run on 8080 has to start it once before Mira will watch it. That is a
real limitation and it is the price of the no-raw-port rule; the safe way to lift
it later is a chooser of ports Mira has seen *at any time*, not a text field.

A stopped service is shown as a bare port. Three stopped services are three bare
ports, distinguished only by number. That is honest and it is thin; a
user-written note would fix it and is a separate decision about a text column.

**Bounded.** `mira_core::service` depends on nothing. Deleting the table, the
five commands and one component would leave every other surface exactly as it is
— including the project's own service list, which is untouched.
