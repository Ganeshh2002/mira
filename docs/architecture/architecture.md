# Aviora Mira — Architecture

Status: **in progress.** Slices 0 and 1 are built; the rest of this document is the
design target. Decisions are recorded as ADRs in [../adr/](../adr/) — this document is
the map, the ADRs are the reasoning.

---

## 1. Stack

| Layer | Choice | ADR |
|---|---|---|
| UI | React 19 + TypeScript (strict), Vite | [0002](../adr/0002-react-typescript.md) |
| Desktop shell | Tauri 2 | [0001](../adr/0001-tauri-2.md) |
| System layer | Rust (2021 edition, stable toolchain) | [0003](../adr/0003-rust-system-layer.md) |
| Persistence | SQLite via `rusqlite` (bundled), Rust-owned | [0004](../adr/0004-sqlite-local-first.md) |
| Platform abstraction | Capability traits per OS | [0005](../adr/0005-platform-abstraction.md) |
| Extension boundary | Modules over a stable command surface | [0008](../adr/0008-modular-architecture.md) |
| Git | libgit2 (`git2`) behind a provider trait | [0009](../adr/0009-git-via-libgit2.md) |
| Monorepo layout | Manifest reading in its own crate, never stored | [0010](../adr/0010-monorepo-detection.md) |
| Recurring work | One gated scheduler, blocking observers | [0011](../adr/0011-one-scheduler.md) |

Chosen because the constraints in
[product-definition.md](../product/product-definition.md) — ≤ 30 MB installer, ≤ 150 MB
idle RSS, three OSes, deep system access, no bundled runtime — eliminate Electron by
arithmetic and eliminate a pure-web approach entirely. Tauri 2 uses the OS webview and
gives a first-class Rust process for the system work that *is* the product.

---

## 2. Shape

```text
┌──────────────────────────────────────────────────────────────────────┐
│  React / TypeScript          (views, state, keyboard, theming)       │
│  ── typed IPC client ────────────────────────────────────────────────│
└───────────────────────────────┬──────────────────────────────────────┘
                                │  Tauri commands (request/response)
                                │  Tauri events   (push: status, ticks)
┌───────────────────────────────▼──────────────────────────────────────┐
│  Tauri 2 shell        window/tray/shortcut/updater/single-instance    │
├──────────────────────────────────────────────────────────────────────┤
│  mira-app             command handlers · event bus · scheduler        │
├──────────────────────────────────────────────────────────────────────┤
│  mira-core            domain model · IDs · errors · capabilities      │
├───────────────┬───────────────────────────────┬──────────────────────┤
│  Domain       │  Services (I/O)               │  Platform            │
│  projects     │  git · ports · processes      │  macos               │
│  workspaces   │  filesystem · shelf · peek    │  windows             │
│  sessions     │  ssh · docker · media · system│  linux               │
├───────────────┴───────────────────────────────┴──────────────────────┤
│  mira-db              SQLite (rusqlite) · migrations · repositories   │
└──────────────────────────────────────────────────────────────────────┘
```

Two directions of traffic, and only two: the UI **asks** (commands) and the backend
**tells** (events). There is no third channel, no shared mutable global, and no
filesystem or process access from the frontend.

---

## 3. Cargo workspace

```text
src-tauri/                  # Tauri binary — thin
  src/main.rs               # setup, tray, shortcut, plugin wiring
  src/commands/             # one module per domain; #[tauri::command] only
  src/events.rs             # typed event emitters
  src/scheduler.rs          # visibility-gated pollers
crates/
  mira-core/                # domain types, IDs, errors, Capability
  mira-db/                  # rusqlite, migrations, repositories
  mira-projects/            # project CRUD + detection
  mira-workspaces/
  mira-sessions/
  mira-git/                 # libgit2 behind a trait
  mira-monorepo/            # workspace manifests → package boundaries
  mira-scheduler/           # the only clock: observations, gate, shutdown
  mira-ports/
  mira-processes/
  mira-fs/                  # path safety, watching, shelf, peek reads
  mira-ssh/                 # config parse only
  mira-docker/              # read-only API client
  mira-media/
  mira-system/
  mira-platform/            # capability traits + per-OS impls
  mira-automation/          # 0.6+ — not created before then
src/                        # React app
  app/ features/ components/ lib/ styles/
```

**`mira-automation` is deliberately absent until 0.6+.** Creating an empty crate for an
undesigned feature is speculative architecture; the extension boundary (§8) is what
makes adding it later cheap.

### Dependency rule

```
src-tauri  →  domain crates  →  services  →  mira-platform  →  mira-core
                    ↓                                              ↑
                 mira-db  ────────────────────────────────────────┘
```

Enforced, not merely intended:

1. `mira-core` depends on nothing in the workspace.
2. `mira-platform` depends only on `mira-core`.
3. Domain and service crates never depend on each other **sideways**; cross-domain work
   is composed in `src-tauri/commands`.
4. No crate depends on `tauri` except `src-tauri` itself. This keeps the domain testable
   with plain `cargo test`, with no webview and no app harness.
5. CI fails the build if these are violated (`cargo-deny` + a dependency-graph check).

Rule 4 is the load-bearing one: it is what makes the system layer a library that happens
to have a Tauri front end, rather than a Tauri app with logic scattered through it.

---

## 4. Module boundaries

Each module answers three questions — *what does it do, how is it used, what does it
depend on* — and nothing outside it may reach past its interface.

| Module | Does | Interface | Depends on |
|---|---|---|---|
| **core** | Domain types (`ProjectId`, `Workspace`, `Capability`, `MiraError`), no I/O | Types + traits | — |
| **db** | Owns the SQLite connection, migrations, repositories | `ProjectRepo`, `WorkspaceRepo`, … | core |
| **projects** | Project lifecycle, directory probing, type markers | `ProjectService` | core, db, fs |
| **workspaces** | Workspace CRUD, active-workspace resolution | `WorkspaceService` | core, db |
| **sessions** | Session start/pause/resume/close from lock+focus events | `SessionService` | core, db, platform |
| **git** | Status, HEAD, branches, ahead/behind, commit walk, lane layout | `GitProvider` trait | core |
| **monorepo** | Workspace manifests → tools and package boundaries, read-only | `detect(selected, git_root)` | core |
| **scheduler** | The only clock: intervals, gate, cancellation, isolation | `Observation`, `Gate`, `Scheduler` | core |
| **ports** | Listening sockets → (port, pid, process) + attribution | `PortScanner` | core, processes |
| **processes** | Process facts, safe termination | `ProcessProvider` | core, platform |
| **filesystem** | Path canonicalisation, **root containment checks**, watching, safe reads | pure functions, then `FsService` | core |
| **shelf** | Shelf item references, scopes, missing detection | `ShelfService` | core, db, fs |
| **peek** | Bounded, read-only previews with type sniffing | `PeekService` | core, fs |
| **ssh** | Parse `~/.ssh/config` names; optional reachability probe | `SshService` | core, fs |
| **docker** | Read-only container listing over the local socket | `DockerClient` | core |
| **media** | Now-playing where the OS permits | `MediaProvider` | core, platform |
| **system** | CPU/memory/disk/battery/network sampling | `SystemProvider` | core, platform |
| **platform** | Every OS-specific call in the product | Capability traits | core |
| **automation** | *(Future)* | — | — |

### Boundary tests
- Can a consumer use it without reading its internals? Each is an interface plus a
  concrete impl behind it.
- Can internals change without breaking consumers? Providers are traits; `git` can move
  from libgit2 to gitoxide, `ports` from one crate to another, without touching callers.
- Is it independently testable? Yes — none needs a running app, and the OS-touching ones
  have fake implementations of their trait.

**Size discipline:** a Rust file past ~400 lines or a React component past ~200 is a
signal to split. Large files are where boundaries go to die, and they degrade both human
and machine reasoning about the code.

---

## 5. The IPC contract

### Commands (UI asks)

Namespaced `domain.verb`, snake_case params, typed results:

```ts
projects.list()                       → Project[]
projects.add()                        → Project | null
projects.open({ projectId })          → Project
projects.remove({ projectId })        → void
projects.reveal({ projectId })        → void
live.snapshot()                       → LiveSnapshot
live.refresh()                        → LiveSnapshot
live.open_service({ port })           → void
git.log({ projectId, limit, cursor }) → CommitPage
ports.scan({ projectId? })            → PortEntry[]
processes.terminate({ pid, force })   → TerminateOutcome
apps.launch({ target, projectId })    → LaunchOutcome
peek.read({ path, maxBytes })         → PeekPayload
shelf.add({ path, scope })            → ShelfItem
```

Rules:
1. Every command returns `Result<T, MiraError>`; there are no `unwrap`s on the command
   path and a panic in a handler is a bug that fails CI.
2. Commands are **thin** — argument validation, service call, mapping. No business logic
   lives in `src-tauri`.
3. Anything that can exceed ~50 ms runs on a blocking task; commands never block the
   webview's event loop.
4. Types are defined once in Rust and exported to TypeScript by `ts-rs` at build time.
   A drift between the two fails the build rather than reaching runtime.
5. **No command takes a filesystem path from the frontend.** The webview may name a
   *project*; it may never name a *directory*. Registering a root is the moment Mira is
   granted read access to a tree, so that moment belongs to a native picker opened in
   Rust on a user gesture — which is why `projects.add` has no arguments. Enforced by a
   guard test that scans every command signature.

### Events (backend tells)

```
mira://live                   an observation round finished
mira://status/{projectId}     coalesced project status changed
mira://ports/{projectId}      port set changed
mira://system                 system sample tick
mira://session                lock / unlock / sleep / wake
mira://capability             a capability's availability changed
```

Events carry **change notifications, not large payloads** — the UI re-queries what it
needs. This keeps the serialisation cost of a busy watcher near zero and stops the event
channel becoming a second, unversioned API.

### Error model

```rust
enum MiraError {
  NotFound { what: String },
  PermissionDenied { what: String, hint: String },
  Unsupported { capability: Capability, reason: String },
  Timeout { operation: String, after_ms: u64 },
  External { source: String, detail: String },
  Invalid { field: String, detail: String },
}
```

Every variant carries something the UI can *show a human*. `Unsupported` is what makes
the Degraded/Unavailable states of the IA truthful rather than decorative.

---

## 6. Concurrency and scheduling

The whole product is polling and watching, so scheduling is a first-class concern rather
than an implementation detail.

- One Tokio multi-thread runtime, owned by `src-tauri`.
- **One scheduler, in `mira-scheduler`** ([ADR-0011](../adr/0011-one-scheduler.md)).
  It holds `(observation, interval, gate)` and nothing else; the observers in
  `src-tauri/src/observers.rs` say what is watched. A guard test fails the build if
  any timer appears outside it.
- CPU- or syscall-heavy work (`git status`, socket enumeration, process walks) runs on
  `spawn_blocking`; async is used for I/O waits (Docker socket, D-Bus), not for CPU.
- **One scheduler owns every recurring task.** No module starts its own timer. It holds
  `(task, interval, gate)` and gates on: any window visible, screen unlocked, and the
  view that needs the data being on screen.
- Watchers (filesystem) are debounced at 500 ms and coalesced per project.
- Every scan is cancellable, and navigating away cancels in-flight work.
- SQLite access is serialised through a single connection guarded by a mutex; SQLite is
  fast enough here that a pool would add contention bugs and no measurable speed.

This is the mechanism behind the "idle CPU ≈ 0%" budget: it is impossible to poll while
hidden, because the only clock in the process refuses to fire.

---

## 7. Frontend architecture

- **React 19 + TypeScript strict.** No `any` on the IPC boundary — generated types only.
- **State:** TanStack Query for everything server-owned (commands are queries, events
  invalidate them); Zustand for genuinely local UI state (selection, collapse, overlay).
  No Redux, no context-as-state-store.
- **Routing:** in-memory state machine, not a URL router. Mira has five surfaces and no
  address bar; a router would be ceremony.
- **Components:** headless primitives + Tailwind (see design-system.md). No component
  framework whose defaults would fight the visual direction.
- **The frontend has no privileges.** No `fs`, `shell`, or `http` plugin is enabled for
  it. Every capability it needs arrives as a named command with a validated argument.
  This is enforced in `capabilities/*.json`, and it is the single most important
  security property of the app.

---

## 8. Extension boundary (Future plugins, and Astra)

One boundary serves both, by design:

```
        ┌─────────────────────────────────────┐
        │  Mira core + services + UI          │
        │                                     │
        │   ┌───────────────────────────┐     │
        │   │  Extension host (Future)  │     │
        │   │  · capability-scoped      │     │
        │   │  · declared permissions   │     │
        │   │  · no ambient access      │     │
        │   └───────────┬───────────────┘     │
        └───────────────┼─────────────────────┘
                        │
              ┌─────────┴─────────┐
              │                   │
       third-party plugin    Astra integration
```

Rules that hold from day one, before any host exists:

1. Nothing named `astra` exists in core, services, domain, or the schema.
2. An integration may only consume public interfaces — the same ones a plugin would.
3. No feature in the 0.x line may become reachable *only* through an integration.
4. Deleting the integration crate must leave a compiling, fully functional Mira. That
   deletability is the acceptance test for the boundary.

We build **no plugin host before 0.6+**. The boundary is maintained by the dependency
rules in §3, which cost nothing now and make the host tractable later.

---

## 9. Cross-cutting decisions worth naming

**Persistence is Rust-owned; the frontend cannot execute SQL.** The obvious path —
`tauri-plugin-sql` — hands the webview a `Database.execute(sql)` surface. That inverts
the security model in §7 (any XSS becomes full database access) and dissolves the
repository boundary, since queries would spread through React. Mira instead uses
`rusqlite` inside `mira-db` behind repository traits, exposed only as typed commands.
See [ADR-0004](../adr/0004-sqlite-local-first.md).

**Git through libgit2 (`git2`), behind a trait.** Recorded in full as
[ADR-0009](../adr/0009-git-via-libgit2.md), including the licence analysis: the crates
declare MIT/Apache-2.0, and the C library they vendor is GPLv2 *with a linking
exception* that permits exactly this use. `git2` is built with default features off, so
its HTTPS and SSH transports are not compiled in — Mira could not fetch if it tried.
`gitoxide` remains the expected destination once its status and graph APIs settle; the
trait keeps that a change inside one crate. Shelling out to `git` was rejected: it
depends on the user's PATH, costs a process per query, and would make the
command-injection surface real.

**No shell, anywhere.** All child processes are spawned with an argv array. There is no
code path where a user-supplied string reaches a shell parser. This is checked by test.

**Bundled SQLite.** `rusqlite`'s bundled build removes "which SQLite does this OS have"
from the support matrix, at a small binary cost.

**Single instance.** `tauri-plugin-single-instance` makes `mira --toggle` the Wayland
fallback for the global shortcut and prevents two processes fighting over the database.

---

## 10. Testing strategy

| Level | Scope | Tooling |
|---|---|---|
| Unit (Rust) | Pure logic: graph lanes, path containment, ssh-config parsing, attribution, argv building | `cargo test` |
| Integration (Rust) | Services against fixtures: temp Git repos, temp SQLite, fake platform | `cargo test`, `tempfile` |
| Contract | Generated TS types match Rust; command names exist | `ts-rs` + a codegen diff check in CI |
| Component (UI) | Views against a mocked IPC client | Vitest + Testing Library |
| E2E | The 0.1 happy paths per OS | Tauri driver / WebDriver, best-effort in CI |
| Guard tests | The promises that must not rot | see below |

**Guard tests** are the ones that encode this document's non-negotiables, and they run on
every commit:

- no shell invocation anywhere in the process-spawn path;
- no file read outside a registered project root or the shelf;
- SSH: no `IdentityFile` path is ever opened;
- Docker: only `GET` requests are issued;
- no network request when SSH reachability is disabled;
- macOS binary links no private framework (`otool -L`);
- schedulers produce no work while all windows are hidden.

TDD is the working default: for anything with logic, the failing test comes first.

---

## 11. Build, packaging, distribution

- **Bundles:** `.dmg` + `.app` (macOS, universal), `.msi` + NSIS `.exe` (Windows),
  `.AppImage` + `.deb` + `.rpm` (Linux).
- **Updater:** `tauri-plugin-updater`, opt-in at first run, signature-verified, static
  JSON manifest on a static host. No account, no telemetry, no phone-home unless the
  user enables update checks.
- **Signing:** macOS notarisation and Windows signing require certificates the project
  may not have at 0.1.0; unsigned builds ship with clear install instructions rather
  than pretending. Linux needs no signing.
- **CI:** build and test on all three OSes for every PR; release builds are tagged.
- **Reproducibility:** pinned toolchain (`rust-toolchain.toml`), committed lockfiles.

---

## 12. What this architecture makes easy — and what it does not

**Easy:** adding a service (new crate, new trait, new commands, no changes elsewhere);
swapping an implementation (git, ports, media); supporting a new terminal/editor (a
descriptor row); adding a platform (implement the capability traits); deleting a feature
cleanly.

**Deliberately hard:** giving the frontend direct system access; running SQL from the UI;
starting a background poller without the scheduler; adding an Astra dependency to core;
shipping a feature that pretends to work on a platform where it does not.

The second list is the point. Each item is something that would be convenient once and
corrosive thereafter.
