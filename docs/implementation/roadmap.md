# Aviora Mira — Implementation Roadmap

Status: **pre-implementation.** Nothing below is built.

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

Sizes are relative effort for one developer, not calendar promises: **S** ≈ a few days,
**M** ≈ a week or two, **L** ≈ several weeks.

---

## Slice 0 — Skeleton *(prerequisite, S)*

Not in the original list, but Slice 1 cannot ship without it, so it is named rather than
smuggled in.

**Delivers:** a Tauri 2 app that opens an empty window, a Cargo workspace with
`mira-core` + `mira-db`, the migration runner with `0001_init.sql`, `ts-rs` type
generation wired into the build, the tokens file and base components from the design
system, and CI building and testing on all three platforms.

**Done when:** `npm run tauri dev` opens a window on all three OSes; CI is green; a
generated TypeScript type appears in the frontend; the database file is created on first
run and its location is correct per platform.

---

## Slice 1 — A project, and its Git status *(M)*

> Launch Mira → create project → detect project directory → detect Git → display Git status.

**Delivers.** Add a project by picking a directory. Detection: is it a repo, what type
markers exist. The project list with a status dot. A detail view showing branch, last
commit, ahead/behind, and changed-file counts with a grouped path list. The tray icon and
menu. The global shortcut and the compact window in its simplest form (project status +
dismiss). Wayland detection with the `mira --toggle` fallback wired and documented.

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

---

## Slice 4 — Multiple projects and workspaces *(M)*

**Delivers.** Workspaces as a first-class concept: create, rename, duplicate, delete; the
switcher that stays hidden until a second workspace exists; per-workspace subpath, expected
ports, commands, and app overrides; per-project state (selection, collapse, scroll) that
survives switching; project reordering; `⌘1…⌘9`; the command palette in the compact window
ranking actions across all projects.

**Done when:** three projects with different workspaces stay independently live, switching
is instant and starts nothing, and a single-workspace user never sees the concept.

**Risks.** Concept overload — mitigated structurally by hiding it. The compact window's
ranking is where the palette either feels instant or does not; it is worth the time here.

---

## Slice 5 — Git history and graph *(M)*

**Delivers.** Commit walking with pagination; the lane-assignment algorithm in Rust, pure
and unit-tested; the History view with badged refs; commit detail (subject, body, author,
SHA copy); cancellation on navigation.

**Done when:** 200 commits render in under 500 ms on a 50 000-commit repo, merges draw
correctly, and leaving the view stops the work.

**Risks.** Graph layout is the fiddliest UI in the MVP. Keeping lanes in Rust behind a
pure function — testable against fixture repos — is the mitigation, and it is non-negotiable.

---

## Slice 6 — Shelf *(S)*

**Delivers.** Project and global shelves; drag-and-drop from the OS; add from the file
picker; open in editor, reveal in file manager, copy path; reorder; Missing detection and
relocation; drag-out where the platform supports it.

**Done when:** a dropped file opens in the editor, removing an item provably leaves the
file on disk, and Linux's reveal-opens-the-folder difference is stated in the UI.

**Risks.** Small. The one real hazard is users assuming the Shelf stores content — solved
with wording and the explicit Missing state.

---

## Slice 7 — Quick Peek *(M)*

**Delivers.** The read-only overlay: text with syntax highlighting up to 2 MB, images,
sandboxed SVG, a metadata card for everything else; binary sniffing; fuzzy path search
within a project respecting ignore rules; `Space` / `⏎` / `Esc`.

**Done when:** a 5 000-line file renders highlighted in under 200 ms, a 500 MB binary does
not freeze anything, an SVG containing a script does not execute it, and a path outside
every project root is refused by the backend.

**Risks.** Untrusted content in a webview — the CSP, the no-`innerHTML` rule, and the
image-not-DOM rule for SVG are all enforced and tested in this slice.

---

## Slice 8 — SSH and Docker awareness *(M)*

**Delivers.** Opt-in `~/.ssh/config` parsing (names only, keys never opened); manual
hosts; per-host opt-in reachability; detection of local `ssh` processes and their
forwards; Docker socket detection (`DOCKER_HOST`, candidate paths, Podman); read-only
container listing with compose-label association; container ports linked into the Ports
view.

**Done when:** a compose stack shows correct state in its project; Docker being absent
renders nothing at all rather than an error; the guard tests prove no key file is opened,
no `GET`-only violation occurs, and no network connection happens with probing disabled.

**Risks.** The two most sensitive integrations in the product. Both are read-only, both
opt-in, both fenced by guard tests written in this slice.

---

## Slice 9 — System, media, and session awareness *(M)*

**Delivers.** The system strip (CPU, memory, disk, battery, network) with visibility-gated
sampling; lock/sleep/wake detection per platform; polling suspension while locked; session
recording with pauses and "welcome back"; media detection on Windows and Linux, with
macOS reporting Unavailable and the `otool -L` guard test in CI.

**Done when:** locking the screen provably stops all polling within 2 s, a 30-minute lock
does not count as work, non-logind Linux runs correctly as Degraded, and macOS shows the
honest media message.

**Risks.** The strongest temptation in the project is to close the macOS media gap with a
private framework. The answer is settled: no. The test enforces it.

---

## Slice 10 — Themes and personalization *(S–M)*

**Delivers.** Light/Dark/System; accent selection with a contrast gate; density modes;
the five atmospheres plus Custom; the pulse; motion and battery rules; per-project accent.

**Done when:** every atmosphere passes AA contrast in CI, the heaviest stays under 2% idle
CPU, `prefers-reduced-motion` stops everything, and ambient layers pause when unfocused.

**Risks.** Scope creep into decoration. The constraint that an atmosphere may only change
tokens plus one canvas layer — never layout, spacing, or the status language — is what
keeps this a week rather than a month.

---

## Slice 11 — Workspace restoration *(M, V1.x)*

**Delivers.** Saving a workspace's open apps, terminals, URLs, and expected containers;
restoring them on demand; a preview of what will open before it opens.

**Done when:** restoring reopens the set without killing anything already running, and the
user sees exactly what is about to happen first.

**Risks.** Restoration that surprises people is worse than none. It is always explicit,
never automatic on launch, and never force-closes anything.

---

## Slice 12 — Automation *(L, Future)*

**Delivers.** Nothing yet. Automation needs a trust model before a schema — it is a rules
engine that launches processes, which is the exact shape an attacker wants to write to
([security-and-privacy.md](../architecture/security-and-privacy.md) §5).

**Prerequisites before any design work:** MVP shipped and in real use; a written trust
model covering where rules come from, how they are reviewed, and what they may do; and
demand evidenced by actual requests rather than assumption.

Listed here for completeness, deliberately unspecified.

---

## Sequence and dependencies

```
0 Skeleton
└─ 1 Project + Git ──┬─ 2 Ports/Processes ──┬─ 8 SSH/Docker
                     │                      └─ 9 System/Media/Session
                     ├─ 3 App launching ────── 6 Shelf ── 7 Peek
                     ├─ 4 Workspaces
                     └─ 5 Git graph
                                              10 Themes  (any time after 1)
                                              11 Restoration (needs 3, 4, 9)
                                              12 Automation (needs everything + a trust model)
```

Slices 2 and 3 can proceed in parallel after 1. Slice 10 can be pulled forward if the app
needs to look finished for a demo; it depends only on the token layer.

## Release mapping

| Release | Contains | Meaning |
|---|---|---|
| **0.1.0** | Slices 0–3 | First public release. Projects, Git status, ports, launching. Genuinely useful. |
| **0.2.0** | Slices 4–5 | Workspaces and history. |
| **0.3.0** | Slices 6–7 | Shelf and Peek. |
| **0.4.0** | Slices 8–9 | Awareness features. |
| **0.5.0** | Slice 10 | Personalization. MVP feature-complete. |
| **1.0.0** | after real use | Command surface and schema settled. |

**0.1.0 is a real release, not a preview.** Three slices in, Mira answers "what branch,
what's dirty, what's on :3000, open it" — which is already worth installing. That is the
test of whether the slicing is vertical.

## What is explicitly not built during MVP

A plugin host · any Astra code · cloud or sync · notifications beyond capability
reporting · Git write operations of any kind · Docker lifecycle actions · an
`automations` table · a general app launcher · file editing or deletion · telemetry.

Each is either Future scope or permanently out of scope. Building "just the schema" or
"just the interface" for any of them ahead of time is the failure this roadmap is shaped
to avoid.
