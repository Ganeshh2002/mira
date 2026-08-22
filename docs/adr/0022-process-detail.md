# ADR-0022 — Process detail: a rate needs two samples, and argv is not shown

**Status:** Accepted · 2026-08-22

## Context

`product-scope.md` §3 lists **Process information** (PRD 8) in the 0.1 MVP, and
`roadmap.md` slice 2 spells it out: *"process detail (CPU, memory, uptime,
command line)"*. Slice 2 shipped in part — sockets, pids, names, attribution —
and left the detail, the machine-wide Ports view, and termination.

This slice takes the first two. Termination is the destructive half and keeps its
own design work; putting it in a pull request about a read-only view would be the
wrong place to argue about confirmation flows.

Two questions had to be answered before any of it could be built, and only one of
them was obvious from the outside.

## Decision

### The command line is not shown, not stored, and not read

The roadmap lists argv and this slice declines it.

A process's command line routinely carries credentials. `--password=` on a
database client. `PGPASSWORD=` in front of one. A token inside a `DATABASE_URL`.
An API key a task runner passed to a child. None of that is exotic; it is
Tuesday.

Mira showing argv would put those on screen, into every screenshot of that
window, and in front of anyone glancing at the machine. Mira is a companion that
sits open all day beside the work — the worst possible place for a credential to
be permanently legible.

**Redaction was considered and rejected.** Redacting means a blocklist —
`password`, `token`, `secret`, `key`, `pwd`, `auth`, `credential` — and
blocklists leak. `--db=postgres://user:hunter2@host` contains none of those
words. A redactor that is wrong once is worse than no feature, because it teaches
people the output is safe to screenshot.

So there is no field on `ProcessFacts`, nothing calls `sysinfo`'s `Process::cmd()`,
and a guard test fails the build if anything starts to. The absence is enforced,
not intended.

What is lost: you cannot tell two `node` processes apart by their arguments. The
port, the working directory and the project attribution already distinguish them
for the question Mira answers — *what is running in my project, and on what port*
— and a terminal is one keystroke away for the question it does not.

### CPU share is a rate, so it is absent until there are two samples

This is the finding that shaped the implementation, and it was not visible from
the API's signature. `sysinfo` computes CPU share from the delta between two
refreshes of the **same** `System`. A provider that builds a fresh one per call —
which is exactly what Mira did — can only ever report `0.00%`.

Measured, against a process provably burning a core:

| | reading |
|---|---|
| one refresh of a fresh `System` | **0.00%** — unusable |
| second refresh of a kept `System` | the true figure |

So `Processes` keeps one `System` for the life of the process. It is refreshed by
the observer ticks that already exist; it holds no timer and schedules nothing,
so ADR-0011's one-scheduler rule is intact.

**`cpu_share` is `Option<f32>`, and the first reading of any pid is `None`.**
`sysinfo` returns `0.0` both for a process that is idle and for one it has not
measured yet; only Mira's own record of which pids it has sampled can tell those
apart. Reporting `0%` on a first sighting would call a process idle that might be
saturating a core — a claim Mira has not established, which is the same failure
as [ADR-0018](0018-history-filters.md)'s "no results" for a search that ran out
of budget.

The interface renders the difference: *"not measured yet"*, never *"0%"*.

### The existing five-second tick is not a compromise — it is the best window

The tempting shortcut is two refreshes 200 ms apart inside one request. That
needs a sleep on the observer thread, and a sleep is a clock.

It is also worse. Measured against the same busy process:

| gap between samples | reported share |
|---|---|
| 200 ms (`sysinfo`'s minimum) | 238% |
| 500 ms | 200% |
| 1 s | 130% |
| 5 s | **100.3%** |

A short window over a bursty process is noise. The five-second gap between the
scheduler's ticks is the steadiest figure available. **Needing no new timer and
wanting the widest window turned out to be the same answer**, which does not
usually happen and is worth writing down when it does.

Adding cpu and memory to the refresh is free — within measurement noise, and
sometimes marginally faster:

| pids | exe + cwd (before) | + cpu & memory |
|---|---|---|
| 4 | 0.038 ms | 0.036 ms |
| 16 | 0.069 ms | 0.072 ms |
| 64 | 0.208 ms | 0.194 ms |

Keeping the `System` rather than rebuilding it costs 0.160 ms against 0.146 ms at
24 pids — beside the 2.5 ms socket scan it rides along with, that is noise.

### Memory and uptime need no such care

Neither is a rate. Both are reported from the first reading, and both are
`Option` for the ordinary reason: some platform, some day, declines to give one,
and an absent fact is shown as absent rather than as zero.

### The machine-wide Ports view is an arrangement, not a reading

`live.ports` groups the observation the scheduler already took —
*this project's* / *other projects'* / *not in any project* — exactly as
`information-architecture.md` §5 describes. It starts no observer, opens no
socket, and touches no file; a guard asserts all three.

Grouping is negligible: **0.003 ms** for this machine's 13 listeners, 0.1% of the
3.5 ms read it arranges. Even 4 096 listeners over 40 projects is 0.424 ms, and
the cost is flat in project count (0.163 ms at 1 project, 0.225 ms at 200, both
over 1 024 listeners), so the linear group scan never becomes quadratic in
practice. No cache, no index, no clock.

A listener attributed to a project Mira no longer knows falls to *not in any
project* rather than being shown under a name Mira cannot produce.

### The surface says what it is

A machine-wide list of running servers sitting beside a workspace's Services list
is a confusion waiting to happen. The Ports view is filed below the projects and
visibly apart from them, labelled *this machine*, and its first sentence is
*"Everything listening on this computer — not just your projects, and not what a
workspace watches."*

### Nothing here can stop anything

Open is the only action, and it sends a **position** in the list Mira produced —
the same shape a workspace's services use ([ADR-0020](0020-workspace-services.md))
— so no port, pid, address or process name travels inward. There is no Stop, no
Kill, and no greyed-out control hinting at one. A guard asserts the Ports surface
contains none of those words and that the module exposes exactly one command,
which is a read.

### Nothing is stored

CPU, memory, uptime and pids are observations (`data-model.md` §1 rule 2).
**Slice 2b adds no migration**, and a guard asserts both that no migration
mentions any of them and that the migration set is still five files.

## Alternatives considered

**Show argv, redacted.** Rejected above: a blocklist that is wrong once is worse
than no feature.

**Show argv behind a "reveal" control.** Rejected. It moves the decision to
someone who cannot see what is in the string before they ask for it, and the
credential is on screen either way.

**Sample CPU twice per request, 200 ms apart.** A sleep on the observer thread,
a second clock in all but name, and — measured — a noisier number than the free
one.

**A dedicated faster tick for CPU.** A second scheduler. ADR-0011 exists to
prevent exactly this, and the measurement removed the motive.

**Report `0.0` on a first sample.** One fewer `Option`, and a lie on the first
paint after every launch.

**Fold the machine-wide view into the project's Services section.** Rejected by
`information-architecture.md` §5: a project's section answers "what is running in
*this* project", and machine-wide noise under a project's name is the thing that
rule exists to prevent.

## Consequences

**Good.** The 0.1 MVP's Process information capability is complete. CPU, memory
and uptime arrive on every surface that already showed a process, inline, with no
new privilege and no new clock. The machine-wide view answers "what has :3000"
without a project in mind. Nothing was stored, so there is no migration to
review.

**Costs.** CPU is blank for the first five seconds after launch and for the first
tick after a service appears. That is honest and it is briefly ugly.

Two `node` processes in the same project on different ports are told apart by
port and working directory, not by their arguments. That is the price of the argv
decision and it is deliberate.

Termination is still not built, so a machine-wide list of servers is a list you
cannot act on beyond opening. That is the correct order.

**Bounded.** `mira-processes` gained three fields and a kept reading. Deleting
the Ports surface and the inline detail would leave every other surface exactly
as it is.
