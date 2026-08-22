# Aviora Mira — Implementation Roadmap

Status: **pre-implementation.** Nothing below is built.

Phase membership is locked in [product-scope.md](../product/product-scope.md) and is
not renegotiated here. This document says how the work is cut and sequenced; that one
says which release it lands in. **0.1 is the MVP; everything from 0.2 on is post-MVP.**

Mira is built in **vertical slices**. Every slice crosses the whole stack — schema, Rust
service, platform layer, IPC, UI — and ends with something a person can actually use.
There is no "backend phase". A slice that leaves the app less usable than it was is not
done.

---

## Rules

1. **Every slice ships a usable increment.** If you cannot demo it to someone who does not
   know the codebase, it is not finished.
2. **Every slice ends green on all three platforms.** CI on macOS, Windows, and Linux is
   the definition of done, not a later cleanup.
3. **Tests first for anything with logic.** Guard tests
   ([architecture.md](../architecture/architecture.md) §10) are written in the slice that
   creates the risk, not afterwards.
4. **Capabilities are declared honestly in the slice that introduces them** — matrix row
   filled, reason strings written, Degraded/Unavailable states rendered.
5. **Budgets are checked every slice** (startup, idle CPU, idle RSS). A regression is a
   bug in that slice.
6. **Docs move with the code.** A slice that changes documented behaviour updates the doc
   in the same PR.
7. **No speculative structure.** Build the crate when the slice needs it.
8. **No post-MVP feature is partially built during 0.1.** No table, no trait, no
   interface added because it will make a later phase easier
   ([product-scope.md](../product/product-scope.md) §1).

Sizes are relative effort for one developer, not calendar promises: **S** ≈ a few days,
**M** ≈ a week or two, **L** ≈ several weeks.

---

## Slice 0 — Skeleton *(prerequisite, S)*

**Gate: Slice 0 does not start until [product-scope.md](../product/product-scope.md) is
committed.** The scope document is the first artifact, before the skeleton, because
every slice after it is an argument about scope that the document has already settled.

Not in the original list, but Slice 1 cannot ship without it, so it is named rather than
smuggled in.

**Delivers:** a Tauri 2 app that opens an empty window, a Cargo workspace with
`mira-core` + `mira-db`, the migration runner with `0001_init.sql`, `ts-rs` type
generation wired into the build, the tokens file and base components from the design
system, and CI building and testing on all three platforms.

**Done when:** `npm run tauri dev` opens a window on all three OSes; CI is green; a
generated TypeScript type appears in the frontend; the database file is created on first
run and its location is correct per platform.

**Delivered.** The slice shipped wider than the line above: `mira-platform` with runtime
capability resolution, the `mira-projects` and `mira-workspaces` boundary crates, the
typed command boundary with structured errors, the tray and the global shortcut with its
Wayland fallback, a settings surface, and the guard tests that hold the architectural
rules. The tray and the shortcut therefore already exist as a shell when slice 1 starts;
slice 1 gives them project content.

---

## Slice 1 — A project, and its Git status *(M)*

> Launch Mira → create project → detect project directory → detect Git → display Git status.

**Delivers.** Add a project by picking a directory. Detection: is it a repo, what type
markers exist. The project list with a status dot. A detail view showing branch, last
commit, ahead/behind, and changed-file counts with a grouped path list. Project entries in
the tray menu, on top of the shell slice 0 built. The compact window in its simplest form
(project status + dismiss), opened by the global shortcut slice 0 registered.

**Crates.** `mira-projects`, `mira-git`, `mira-fs` (canonicalisation + containment),
`mira-platform` (shortcut, tray).

**Why first.** It proves the entire spine end to end — schema → service → IPC → UI →
tray → shortcut — on the smallest feature that is genuinely useful. Everything after it is
another service through a path that already works.

**Done when:** a developer can add three repos, see at a glance which are dirty, and
summon that view with a keystroke; the window opens within the 250 ms warm budget; idle
CPU is ~0% with the window closed; Wayland users have a working fallback.

**Risks.** Git status on very large repos (timeout + counts-unknown state); the
Wayland/X11 split appearing on day one (deliberate — it is better to meet it now than to
design around a false assumption).

**Delivered.** Add a project through a native picker, list projects most-recently-opened
first, switch between them, read branch / last commit / clean-dirty / ahead-behind on
demand, open the project folder, and remove a project. `mira-fs`, `mira-git` (libgit2,
[ADR-0009](../adr/0009-git-via-libgit2.md)), the project repository and the project
service. macOS and Windows now ask the platform for its standard window material.

**Slice 1.1 — monorepo awareness.** Added before slice 2, because the distinction it
draws is one slice 2 depends on: attributing a port to a project means knowing whether
that project is a repository, a monorepo, or one package inside one. Detection reads
workspace manifests for npm, pnpm, Yarn, Cargo, Turborepo and Nx; it runs nothing, stores
nothing, and is bounded by named constants
([ADR-0010](../adr/0010-monorepo-detection.md)). New crate: `mira-monorepo`.

**Deferred out of slice 1, and why.** The dirty dot in the *list* needs Git for every
project on every render; the changed-path list, the 5 s refresh and the filesystem
watcher all need somewhere for recurring work to live. Both wait for the scheduler
(`architecture.md` §6), which arrives with slice 2's polling — starting a timer before it
exists is the thing the guard tests forbid. Project entries in the tray menu and the
compact window are likewise not built: they are the same status data in two more
surfaces, and cost more than they teach until that data refreshes on its own.

---

## Slice 2 — Ports and processes *(M)*

> Detect ports → display running processes → open localhost → terminate process.

**Delivers.** Listening-socket enumeration with PID and process name; attribution to
projects by working directory (executable path on Windows); the project Ports section and
the machine-wide Ports view; open-in-browser; process detail (CPU, memory, uptime, command
line); terminate with confirmation, graceful-then-explicit-force; expected ports per
workspace showing as "expected, not running".

**Crates.** `mira-ports`, `mira-processes`, platform implementations for all three OSes.

**Done when:** starting a dev server surfaces it in the right project within 5 s; killing
it frees the port; unattributable ports say so; the guard tests for termination safety
pass; polling stops when no window is visible.

**Delivered (part).** The scheduler ([ADR-0011](../adr/0011-one-scheduler.md)) and the
live context it exists for: Git re-read every five seconds with a dirty indicator in the
project list, listening ports enumerated natively, process facts for the pids that own
them, and attribution by working directory — including to the right package inside a
monorepo. Read-only actions only. New crates: `mira-scheduler`, `mira-ports`,
`mira-processes`.

**Still to come in this slice.** The machine-wide Ports view, process detail, and
**termination** — which is the destructive half and keeps its own design work, below.

**Expected ports landed in slice 4c**, as part of the workspace rather than as a
list of its own ([ADR-0020](../adr/0020-workspace-services.md)): a workspace
expects a service because it selected one Mira observed, and the difference
between *expected* and *running* is a state resolved on every read.

**Risks.** This slice contains the product's most destructive action. The confirmation
flow, the refusal rules (PID 0/1, self, other users), and the absence of any keyboard-only
kill path are part of the slice, not a follow-up. Windows attribution is Degraded from the
start and labelled as such.

---

## Slice 3 — Launching applications *(M)*

> Application launching → editor → terminal → browser.

**Delivers.** App detection per platform; the applications registry; per-project and
per-workspace preferences; launch with argv templates; editor / terminal / browser buttons
and their keyboard bindings; descriptor tables for the supported terminals and editors;
open-file-at-line; clear errors for a missing binary.

**Crates.** `mira-platform` (launcher), applications registry in `mira-db`.

**Done when:** on a stock machine with VS Code and the default terminal installed, a fresh
project offers both without configuration, and both open at the right path; a template
containing shell metacharacters demonstrably executes nothing extra.

**Risks.** The command-injection surface. The no-shell rule and its guard test are written
in this slice. Terminal and editor CLI variation is contained in data, with a documented
"add yours" contribution path.

**Delivered (part), after the workspace.** Built as the slice that makes a workspace
actionable ([ADR-0013](../adr/0013-launching-applications.md)). What is in: platform
launching for editor and terminal, per-platform candidate tables carrying their own
working-directory flags, the browser action on an observed service, `Open with` on the
workspace surface, and the boundary that makes it safe — a launch is asked for by
**kind**, and four guard tests keep it that way. macOS launches through `NSWorkspace`
rather than `open(1)`, so Mira never becomes the parent of what it starts.

**Still to come in this slice.** The applications registry in `mira-db` and per-project
or per-workspace *preferences* — which specific application, rather than which kind —
along with argv templates, open-file-at-line, and the keyboard bindings. The `commands`
table stays empty until then, deliberately: a stored command string is the thing worth
not having yet. There is also no "add yours" path, so an unrecognised editor is a table
row in the next release rather than a setting.

---

## Slice 4 — Multiple projects and workspaces *(M, 0.2)*

**Delivers.** Workspaces as a first-class concept: create, rename, duplicate, delete;
app groups (a named set of apps, terminals, and URLs opened as one action — the unit
Slice 11 later saves); the
switcher that stays hidden until a second workspace exists; per-workspace subpath, expected
ports, commands, and app overrides; per-project state (selection, collapse, scroll) that
survives switching; project reordering; `⌘1…⌘9`; the command palette in the compact window
ranking actions across all projects.

**Done when:** three projects with different workspaces stay independently live, switching
is instant and starts nothing, and a single-workspace user never sees the concept.

**Risks.** Concept overload — mitigated structurally by hiding it. The compact window's
ranking is where the palette either feels instant or does not; it is worth the time here.

---

**Delivered (part), out of order.** Workspaces were built before slice 3's application
launching, because the workspace is what launching would hang off — a context to launch
*into* is worth having before the launching. What is in: the workspace model and
service, create/list/rename/open/remove, application **context** as a set of kinds with
platform discovery behind it, and a workspace surface that composes the project's
existing observations ([ADR-0012](../adr/0012-workspace-semantics.md)).

**Delivered (part), 4c — workspace services.** The narrowing ADR-0012 said was
missing. A workspace now says which of its project's observed services are the
work, and the workspace surface shows those and not the project's others:
Project → Packages → Services → Git → Context. Only a port is stored, and only
against a workspace; a service is added by naming a position in the list Mira
offered and opened or forgotten by the row id Mira issued, so no port, address or
URL crosses the IPC boundary inbound at all — `live.open_service` was narrowed
from a port to an ordinal in the same slice. Expected-but-not-running, running,
port-taken, never-observed and unreadable are five distinct states, because
"Mira could not look" must never render as "your server is down"
([ADR-0020](../adr/0020-workspace-services.md)). New table:
`workspace_services`. No new observer and no new clock — resolution is a pure
function measured at microseconds.

**Still to come in this slice.** Per-workspace commands, app groups, the implicit
default workspace, and the workspace switcher in the project header.

## Slice 5a — Git history *(S, 0.1)*

**Delivers.** Commit walking with pagination; the History view as a linear list with
badged refs; commit detail (subject, body, author, SHA copy); cancellation on
navigation. No lanes, no branch lines — a merge is a labelled row.

**Done when:** 200 commits render in under 500 ms on a 50 000-commit repo and leaving
the view stops the work.

**Risks.** Low, by construction. The risk lives in 5b.

**Delivered.** History through the existing `GitProvider` — a second method on the
trait, not a second Git implementation. A page is twenty-five commits and the page
size belongs to `mira-git`, so there is no argument through which the interface could
ask for a whole repository; the cursor is a `CommitId`, a validated hexadecimal
newtype that cannot spell `HEAD`, a refspec, a path or a flag. Every state the
question has is a state the surface renders: empty repository, unborn HEAD, detached
HEAD, shallow clone, a history libgit2 cannot follow, and a directory that is not a
repository. Commit detail is subject, body, both spellings of the id, author and
email, parent count and changed-file count — no diff, which is 5b. Copying goes
through a typed command that resolves the commit first, so the clipboard only ever
receives something Mira read. History belongs to the **repository**: a package inside
a monorepo shows the repository's history and nothing is duplicated per package.

Measured, on this machine, in release: a first page costs **0.9 ms** on repositories
of 100, 1 000 and 10 000 commits, and a later page 0.7–0.8 ms
(`crates/mira-git/tests/performance.rs`, run with `--ignored`). Getting there needed
one finding worth keeping: asking libgit2 for an explicitly time-sorted revwalk makes
it preprocess the whole reachable history before yielding anything — 272 ms on ten
thousand commits — while its default order is the same reverse-chronological sequence
produced lazily.

**Not built, and why.** Cancellation on navigation: a page is twenty-five commits and
one bounded read, so leaving the view leaves at most one short walk in flight and
starts nothing further. A cancellation mechanism would be machinery in front of work
that is already over. Ref badges wait for 5b, which is where refs are read.

**Keep Awake**, shipped alongside. Not a Git feature and not on the original list: a
small system capability — off, 30 minutes, an hour, or until turned off — that holds
the operating system's own power request. Native and public API only, no subprocess,
and **no simulated keyboard or pointer input, ever**
([ADR-0014](../adr/0014-keep-awake.md)). macOS is Full; Windows and Linux report
`Unavailable` with a true reason until their native mechanisms are built, which is
rule 4 rather than an omission. It brings the first one-shot timer in the product, and
that timer lives in `mira-scheduler` beside the only other clock.

---

## Slice 5b — Git graph and diff *(M, 0.2)*

**Delivers.** The lane-assignment algorithm in Rust, pure and unit-tested; lane
rendering with collapse beyond 8; read-only diff view for a commit; file history;
filtering by branch, author, or path.

**Done when:** merges draw correctly and lane computation stays inside 5a's render
budget on the same fixture repo.

**As built.** Shipped in four slices rather than one, because each turned out to
need its own bound and its own way of naming things: **5b** the graph
([ADR-0015](../adr/0015-graph-lanes.md)), **5c** bounded diffs
([ADR-0016](../adr/0016-bounded-diffs.md)), **5d** file history
([ADR-0017](../adr/0017-file-history.md)), **5e** filtering by branch, author,
file and subject text ([ADR-0018](../adr/0018-history-filters.md)). All four are
read-only, all four measured before they were designed, and all four ended with a
declared limit that reports itself.

**Risks.** Graph layout is the fiddliest UI in the product. Keeping lanes in Rust behind
a pure function — testable against fixture repos — is the mitigation, and it is
non-negotiable. This is exactly why it is not in the first release.

---

## Slice 6 — Shelf *(S, 0.3)*

**Delivers.** Project and global shelves; drag-and-drop from the OS; add from the file
picker; open in editor, reveal in file manager, copy path; reorder; Missing detection and
relocation; drag-out where the platform supports it.

**Done when:** a dropped file opens in the editor, removing an item provably leaves the
file on disk, and Linux's reveal-opens-the-folder difference is stated in the UI.

**Risks.** Small. The one real hazard is users assuming the Shelf stores content — solved
with wording and the explicit Missing state.

---

## Slice 7 — Quick Peek *(M, 0.3)*

**Delivers.** The read-only overlay: text with syntax highlighting up to 2 MB, images,
sandboxed SVG, a metadata card for everything else; binary sniffing; fuzzy path search
within a project respecting ignore rules; `Space` / `⏎` / `Esc`.

**Done when:** a 5 000-line file renders highlighted in under 200 ms, a 500 MB binary does
not freeze anything, an SVG containing a script does not execute it, and a path outside
every project root is refused by the backend.

**Risks.** Untrusted content in a webview — the CSP, the no-`innerHTML` rule, and the
image-not-DOM rule for SVG are all enforced and tested in this slice.

---

## Slice 8 — SSH and Docker awareness *(M, 0.6+)*

**Delivers.** Opt-in `~/.ssh/config` parsing (names only, keys never opened); manual
hosts; per-host opt-in reachability; detection of local `ssh` processes and their
forwards; Docker socket detection (`DOCKER_HOST`, candidate paths, Podman); read-only
container listing with compose-label association; container ports linked into the Ports
view.

**Done when:** a compose stack shows correct state in its project; Docker being absent
renders nothing at all rather than an error; the guard tests prove no key file is opened,
no `GET`-only violation occurs, and no network connection happens with probing disabled.

**Risks.** The two most sensitive integrations in the product. Both are read-only, both
opt-in, both fenced by guard tests written in this slice. They sit in 0.6+ rather than
earlier for that reason — they are the read-only half of the surface an automation rule
would act on, and they inherit that phase's gate.

---

## Slice 9 — System, media, and session awareness *(M, 0.4)*

**Delivers.** The system strip (CPU, memory, disk, battery, network) with visibility-gated
sampling; display awareness (count and arrangement, read-only, for predictable compact-
window placement on multi-monitor setups); lock/sleep/wake detection per platform; polling suspension while locked; session
recording with pauses and "welcome back"; media detection on Windows and Linux, with
macOS reporting Unavailable and the `otool -L` guard test in CI.

**Done when:** locking the screen provably stops all polling within 2 s, a 30-minute lock
does not count as work, non-logind Linux runs correctly as Degraded, and macOS shows the
honest media message.

**Risks.** The strongest temptation in the project is to close the macOS media gap with a
private framework. The answer is settled: no. The test enforces it.

---

## Slice 10 — Themes and personalization *(S–M, 0.5)*

**Delivers.** Light/Dark/System; accent selection with a contrast gate; density modes;
the five atmospheres plus Custom; the pulse; motion and battery rules; per-project
accent; per-workspace visual identities; music-reactive ambience where the platform
exposes playback state (Full on Windows and Linux, degrading to a non-reactive
atmosphere on macOS — see Slice 9).

**Done when:** every atmosphere passes AA contrast in CI, the heaviest stays under 2% idle
CPU, `prefers-reduced-motion` stops everything, and ambient layers pause when unfocused.

**Risks.** Scope creep into decoration. The constraint that an atmosphere may only change
tokens plus one canvas layer — never layout, spacing, or the status language — is what
keeps this a week rather than a month.

---

## Slice 11 — Workspace restoration *(M, 0.2)*

**Delivers.** Saving a workspace's open apps, terminals, URLs, and expected containers;
restoring them on demand; a preview of what will open before it opens.

**Done when:** restoring reopens the set without killing anything already running, and the
user sees exactly what is about to happen first.

**Risks.** Restoration that surprises people is worse than none. It is always explicit,
never automatic on launch, and never force-closes anything.

---

## Slice 12 — Automation *(L, 0.6+)*

**Delivers.** Nothing yet. Automation needs a trust model before a schema — it is a rules
engine that launches processes, which is the exact shape an attacker wants to write to
([security-and-privacy.md](../architecture/security-and-privacy.md) §5).

**Prerequisites before any design work:** the MVP (0.1) shipped and in real use; a
written trust model covering where rules come from, how they are reviewed, and what they
may do; and demand evidenced by actual requests rather than assumption. Plugins and
Astra integration share this phase and this gate.

Listed here for completeness, deliberately unspecified.

---

## Sequence and dependencies

```
0 Skeleton  (gated on product-scope.md)
└─ 1 Project + Git ──┬─ 2 Ports/Processes ──┬─ 8 SSH/Docker      (0.6+)
                     │                      └─ 9 System/Media    (0.4)
                     ├─ 3 App launching ────── 6 Shelf ── 7 Peek (0.3)
                     ├─ 4 Workspaces          (0.2)
                     └─ 5a Git history  (0.1) ── 5b Git graph    (0.2)
                                              10 Themes  (0.5, any time after 1)
                                              11 Restoration (0.2, needs 3, 4)
                                              12 Automation (0.6+, needs everything
                                                 + a trust model)
```

Slices 2, 3, and 5a can proceed in parallel after 1. Slice 10 can be pulled forward if
the app needs to look finished for a demo; it depends only on the token layer, and
pulling it forward does not move it into 0.1.

Slice 11 needs 3 and 4. It no longer waits on 9 — restoration reopens apps, terminals,
and URLs, none of which need system awareness.

## Release mapping

Phases are locked in [product-scope.md](../product/product-scope.md). This table maps
them onto slices.

| Release | Contains | Meaning |
|---|---|---|
| **0.1.0** | Slices 0–3, 5a | **The MVP.** Projects, Git status and history, ports, processes, launching, tray, shortcut. |
| **0.2.0** | Slices 4, 5b, 11 | Workspace. App groups, restoration, the lane graph and diff. |
| **0.3.0** | Slices 6–7 | Shelf. Drag-drop, temporary storage, Quick Peek. |
| **0.4.0** | Slice 9 | System. CPU/RAM/battery, network, lock/session, media, displays. |
| **0.5.0** | Slice 10 | Personality. Atmospheres, ambient effects, workspace identities. |
| **0.6+** | Slices 8, 12 | Automation. SSH, Docker, contextual rules, plugins, Astra. Gated. |
| **1.0.0** | after the 0.x line ships and is used | Command surface and schema settled. Not a feature milestone. |

**0.1.0 is the MVP, and a real release rather than a preview.** Four slices and a small
fifth in, Mira answers "what branch, what's dirty, what happened lately, what's on
:3000, open it" — which is already worth installing and keeping. That is the test of
whether the slicing is vertical.

## What is explicitly not built during 0.1

Workspaces · app groups · restoration · the lane graph and diff view · the Shelf ·
Quick Peek · system, media, and session awareness · atmospheres beyond the default ·
SSH · Docker · a plugin host · any Astra code · cloud or sync · notifications beyond
capability reporting · Git write operations of any kind · an `automations` table · a
general app launcher · file editing or deletion · telemetry.

The first ten have a phase and will be built. The rest are Future scope or permanently
out of scope. Either way, building "just the schema" or "just the interface" for any of
them during 0.1 is the failure this roadmap is shaped to avoid — see rule 8.
