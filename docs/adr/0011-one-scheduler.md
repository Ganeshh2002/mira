# ADR-0011 — One scheduler, gated, with blocking observers

**Status:** Accepted · 2026-08-20

## Context

Mira's premise is that it sits on screen all day and stays current. That means
recurring work: re-reading a repository, re-enumerating listening sockets,
re-reading the process table. It also means an idle-CPU budget of approximately
zero, because a companion that costs 3% of a core forever is one people quit.

Those two are in tension, and the tension is resolved by *where the timers live*
rather than by how efficient each poller is. A codebase where any module may
start a `setInterval` or a `tokio::time::interval` accumulates them: each one is
locally reasonable, and together they are a laptop fan. There is no later audit
that fixes this, because by then the timers are load-bearing.

[architecture.md](../architecture/architecture.md) §6 anticipated this before any
existed: *one scheduler owns every recurring task; no module starts its own
timer*. This ADR is that rule made real, with the design decisions that follow
from it.

## Decision

### One scheduler, in its own crate

`mira-scheduler` holds the engine: a list of observations, each with an interval,
a gate, and a shutdown. It depends on `mira-core` and `tokio`, and knows nothing
about Git, ports, processes, or Tauri.

Its own crate rather than a module in `src-tauri`, which is where §3 sketched it,
because a scheduler is exactly the thing you want to test without a webview: the
lifecycle tests pause the clock and assert scheduling, and they run under plain
`cargo test`. That is the same argument as ADR-0008 rule 4, applied one level
further in.

The rule is enforced, not intended: a guard test fails the build if
`tokio::time::sleep`, `tokio::time::interval`, or `thread::sleep` appears outside
this crate, and a second one forbids `setInterval` and `refetchInterval` in the
interface. **Both earned their keep in the slice that introduced them** — the
first draft of the live feature grew a second loop in `lib.rs` to emit events on
a cadence, and the guard refused it. The observers notify after each round
instead, and there is still exactly one clock.

### A gate, not a cancellation

The scheduler asks a [`Gate`] before every observation. Two conditions in
practice: there is at least one project, and at least one window is visible.
Closing the gate stops all work within one interval without tearing anything
down, and opening it resumes without a restart.

**Missed ticks are not replayed.** Coming back to Mira after an hour hidden runs
one observation, not seven hundred. A backlog would turn "you looked at it again"
into a thundering herd at exactly the moment a person is waiting.

Visibility is *asked of the window on each tick* rather than tracked through
events. Events get missed — a window shown from the tray, a Space switch, a
compositor with its own ideas about focus — and a gate that has drifted either
burns CPU forever or never opens again. One call per tick is cheaper than either
failure.

### Observations are blocking, and awaited

An observation is a synchronous function run on `spawn_blocking`. Reading a
repository, walking the socket table and reading the process table are syscall
work, and §6 puts syscall work off the async workers.

Each observer is a loop that **awaits its own observation before sleeping
again**. Two consequences, both wanted: one observer can never overlap itself, so
a repository that takes longer than the interval simply gets read less often
rather than piling up; and shutdown can wait for work in flight, so quitting
never leaves a half-finished read racing the database's final write. Neither
needs a lock or a flag — they fall out of the shape.

### Failures are isolated and retried

An observer that returns an error is logged, kept, and tried again next interval.
Its neighbours never see it. Port enumeration can fail transiently — a permission
prompt, a machine waking — and giving up on the first error would mean one hiccup
hides every service until Mira restarts.

### Observations are notifications, not payloads

After each round an observer calls a notifier, which emits `mira://live` carrying
nothing. The interface hears that something moved and asks for what it needs
([architecture.md](../architecture/architecture.md) §5). The notifier is a
closure rather than an `AppHandle`, so the observers stay free of Tauri types and
can be tested with a counter.

### Observed state is memory, and only memory

No table, no column, no migration. Git state, listening ports and running
processes are observations; [data-model.md](../architecture/data-model.md) §1
rule 2 stores intent, not observation. Every reading carries **when it was
taken**, because the interface's job is to show freshness rather than to imply
it.

The two failure shapes differ deliberately. A project's Git reading is *kept*
when a refresh fails, with the failure beside it — something true and old beats
nothing. The service list is *emptied*, because the whole question it answers is
"what is running right now", and a stale list of servers is actively misleading.

## Alternatives considered

**Filesystem watching instead of polling.** The right long-term answer for Git,
and explicitly deferred. FSEvents, `ReadDirectoryChangesW` and inotify are three
different models with three different failure modes — inotify exhausts its watch
limit on large trees — and each needs a debounce and a fallback. Bounded polling
is a fifth of the code and gives a five-second worst case; the watcher becomes an
optimisation on top of a scheduler that already exists, rather than the thing the
whole feature rests on.

**One timer, all observers.** Simpler to write and wrong the first time two
observers want different intervals — and they will, since a port scan is cheap
and a repository read is not.

**A worker pool with a queue.** More machinery than two observers need, and it
reintroduces the overlap problem the awaited loop removes for free.

**Events for window visibility.** Rejected above: a gate that can silently drift
is worse than one call per tick.

**`sysinfo` for ports too.** It does not enumerate listening sockets. `netstat2`
does, through `sysctl`, `GetExtendedTcpTable` and `/proc` — native interfaces, no
`lsof` output to parse, and so no injection surface and no fragility when the
format changes.

## Consequences

**Good.** Every recurring task is in one file, under one gate, with one shutdown.
The idle budget is structural: with no projects or no visible window, there is
nowhere for work to come from. Adding an observer is implementing one trait.

**Costs.** Five seconds is a worst case a person can notice, and polling does
work that a watcher would not — bounded and gated, but not free. The gate makes
"why did it not update" a question with two answers instead of none. And
observation lives only as long as the process, so every restart begins with one
round of reading everything.

**Bounded.** `mira-scheduler` is reached from `src-tauri` and nowhere else.
Deleting it would remove live updating and leave every command working, because
each observer's work is also reachable through `live.refresh`.
