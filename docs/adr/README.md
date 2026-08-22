# Architecture Decision Records

One file per decision that was expensive to make and would be expensive to reverse.
Format: Context · Decision · Alternatives considered · Consequences.

ADRs are **immutable**. A decision that changes gets a new ADR that supersedes the old
one; the old file stays, marked superseded, because the reasoning is the point.

| # | Decision | Status |
|---|---|---|
| [0001](0001-tauri-2.md) | Tauri 2 as the desktop shell | Accepted |
| [0002](0002-react-typescript.md) | React 19 + TypeScript for the UI | Accepted |
| [0003](0003-rust-system-layer.md) | Rust for the system layer | Accepted |
| [0004](0004-sqlite-local-first.md) | SQLite via rusqlite, Rust-owned (not tauri-plugin-sql) | Accepted |
| [0005](0005-platform-abstraction.md) | Runtime capability model for cross-platform behaviour | Accepted |
| [0006](0006-no-account-no-cloud.md) | No account, no mandatory cloud, no telemetry | Accepted |
| [0007](0007-mit-license.md) | MIT licence | Accepted |
| [0008](0008-modular-architecture.md) | Modular crates with a one-way dependency rule | Accepted |
| [0009](0009-git-via-libgit2.md) | Git through libgit2, behind a provider trait | Accepted |
| [0010](0010-monorepo-detection.md) | Monorepo detection: separate crate, read-only, computed not stored | Accepted |
| [0011](0011-one-scheduler.md) | One scheduler, gated, with blocking observers | Accepted |
| [0012](0012-workspace-semantics.md) | Workspace semantics: stated not observed, a view not an action | Accepted |
| [0013](0013-launching-applications.md) | Launching applications: a kind, not a command; `NSWorkspace` on macOS | Accepted |
| [0014](0014-keep-awake.md) | Keep Awake: a power request, never simulated activity | Accepted |
| [0015](0015-graph-lanes.md) | Graph lanes over a bounded window, without topological ordering | Accepted |
| [0016](0016-bounded-diffs.md) | Bounded diffs, and a file chosen by ordinal | Accepted |
| [0017](0017-file-history.md) | File history: bounded by commits examined, named without a path | Accepted |
| [0018](0018-history-filters.md) | History filters: one budget, four questions, nothing that is an argument | Accepted |
| [0019](0019-application-preferences.md) | Application preferences: a choice is an identity, never a program | Accepted |

Decisions **not** recorded here because they are reversible in an afternoon: Tailwind,
TanStack Query, Zustand, Vite, the icon set. Those live in
[architecture.md](../architecture/architecture.md).
