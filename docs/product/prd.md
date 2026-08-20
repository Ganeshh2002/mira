# Aviora Mira — Product Requirements

Status: **pre-implementation.** No feature below is built.

Read [product-definition.md](product-definition.md) first — it defines the boundary
that every requirement here respects — then [product-scope.md](product-scope.md), which
locks the phases and defines what "MVP" means. Phase membership is decided there and
binding here; this document specifies the features themselves, and is in turn binding on
[roadmap.md](../implementation/roadmap.md).

---

## 0. How to read this

Scope decisions live in [product-scope.md](product-scope.md) and are binding on this
document. This section restates only the vocabulary needed to read the requirements.

- **MVP** = the **0.1** release. Nothing else. If a feature is not in the 0.1 section
  below, its absence does not block the first public release.
- **Post-MVP** = 0.2 through 0.6+. Planned and scheduled, specified here at the depth
  that phase has earned.
- **The 0.x line** = 0.1 through 0.6+, every milestone before 1.0.
- **Future** = directionally accepted, deliberately undesigned, not assigned to a phase.

**Feature numbers are stable identifiers, not reading order.** They do not renumber when
a feature moves between phases, so within a phase section they run out of sequence. Use
the index below to find one.

**Specification depth follows the phase.** Every 0.1 feature is specified in full:
Purpose · User experience · Functional requirements · Platform considerations ·
Acceptance criteria · Dependencies · Risks. Post-MVP features carry the depth they had
when written — several are specified in full because they were drafted before the phases
were locked, and that detail is kept rather than thrown away. Features marked *shape
only* are a paragraph describing intent; they are specified properly in their own phase's
design pass, not now.

Capability language used throughout: a feature is **Full** (works as specified),
**Degraded** (works with a documented, user-visible reduction), or **Unavailable**
(reports itself off with a reason). Mira never silently no-ops. See
[platform-abstraction.md](../architecture/platform-abstraction.md).

### Feature index

**Status** records what is *built*, not what is planned. Partial means the feature has a
working core with named gaps, listed in its own section.

| # | Feature | Phase | Slice | Status |
|---|---|---|---|---|
| 1 | Multiple projects | **0.1** | 1 | Partial |
| 3 | Git status | **0.1** | 1 | Partial |
| 4 | Last commit | **0.1** | 1 | Built |
| 5 | Branch information | **0.1** | 1 | Built |
| 6 | Basic Git graph | **0.1** | 5a | — |
| 7 | Running ports | **0.1** | 2 | Partial |
| 8 | Process information | **0.1** | 2 | Partial |
| 9 | Application launching | **0.1** | 3 | — |
| 10 | Terminal integration | **0.1** | 3 | — |
| 11 | Editor integration | **0.1** | 3 | — |
| 12 | Browser integration | **0.1** | 3 | — |
| 20 | Global shortcut | **0.1** | 1 | Partial |
| 21 | Menu bar / tray | **0.1** | 1 | Partial |
| 2 | Project/workspace management | 0.2 | 4 | — |
| 23 | App groups *(shape only)* | 0.2 | 4 | — |
| 24 | Workspace restoration *(shape only)* | 0.2 | 11 | — |
| 15 | File Shelf | 0.3 | 6 | — |
| 16 | Quick Peek | 0.3 | 7 | — |
| 17 | System status | 0.4 | 9 | — |
| 18 | Media detection | 0.4 | 9 | — |
| 19 | Lock/session awareness | 0.4 | 9 | — |
| 25 | Displays *(shape only)* | 0.4 | 9 | — |
| 22 | Themes and personalization | 0.5 | 10 | — |
| 26 | Workspace identities *(shape only)* | 0.5 | 10 | — |
| 27 | Music-reactive ambience *(shape only)* | 0.5 | 10 | — |
| 13 | SSH awareness | 0.6+ | 8 | — |
| 14 | Docker awareness | 0.6+ | 8 | — |

---

# 0.1 — Core Companion

**This phase is the MVP.** It is the first public release and it stands on its own: a
developer installs it, keeps it, and gets an answer to "what branch, what's dirty,
what's on :3000, open it" across several projects from a single keystroke.

Nothing in a later phase blocks this one, and nothing in a later phase is partially
built during it — no table, no trait, no interface added because it will make 0.2
easier. See [product-scope.md](product-scope.md) §1.

---

## 1. Multiple projects

**Purpose.** Mira's premise: a developer has several projects live at once and needs
one place that keeps them apart. Everything else in the product hangs off a project.

**User experience.** First run shows an empty state with one action: *Add project*.
Picking a directory creates a project — name inferred from the folder, icon/colour
auto-assigned, Git detected silently. The main window lists projects with a compact
status line (branch · dirty marker · running ports count). Selecting one opens its
detail view. `⌘1…⌘9` / `Ctrl+1…9` jump to the first nine.

**Functional requirements.**
- FR-1.1 Add a project by choosing a directory; reject a directory already registered.
- FR-1.2 Store: name, root path, created/last-opened timestamps, colour, optional icon.
- FR-1.3 Detect on add: is it a Git repo (walk up to the worktree root), and what
  project type markers exist (`package.json`, `Cargo.toml`, `go.mod`, `pyproject.toml`,
  `pom.xml`, `Gemfile`, `composer.json`, `*.sln`, `Makefile`, `docker-compose.y*ml`).
- FR-1.4 Rename, recolour, and remove a project. Removal never touches the filesystem
  and asks for confirmation naming the project.
- FR-1.5 A project whose directory no longer exists renders as *Missing* with
  *Relocate* and *Remove* actions. It is never auto-deleted.
- FR-1.6 Ordering: manual drag order, persisted; default is most-recently-opened first.
- FR-1.7 Filter list by typed substring over name and path.
- FR-1.8 Distinguish a standalone repository, the root of a monorepo, and a package
  inside one. Where the selected directory is a package, show the Git repository it
  belongs to **and** the package path, so the two scopes are never confused.
- FR-1.9 Detect monorepo tooling by **reading manifests only**: npm, pnpm and Yarn
  workspaces, Cargo workspaces, Turborepo, and Nx. Never run a package manager, never
  install anything, never write to the repository.
- FR-1.10 Report a package only where a manifest confirms it, and a monorepo only where
  at least one package is confirmed. Package names come from the package's own manifest
  or its directory; they are never invented.
- FR-1.11 Detected packages are **not** projects. They are shown as boundaries; adopting
  one as a project stays an explicit user action.

**Platform considerations.** Path handling must be case-insensitive-aware on
macOS/Windows and case-sensitive on Linux; store the canonicalised absolute path and
compare canonical forms. Windows paths may exceed `MAX_PATH` — use verbatim
(`\\?\`) forms internally. UNC and network paths are accepted but flagged as
potentially slow (detection runs with a timeout).

**Acceptance criteria.**
- AC-1.1 Adding a directory containing a `.git` shows branch and dirty state within
  1 s of the add completing.
- AC-1.2 Ten projects render in the list with no perceptible scroll jank (60 fps).
- AC-1.3 Removing a project leaves the directory byte-identical.
- AC-1.4 Restarting Mira restores the full list, order, and last selection.
- AC-1.5 Adding a directory on an unmounted network share fails with a clear error
  within 5 s instead of hanging.
- AC-1.6 A pnpm/Turborepo monorepo shows its tools and its packages; selecting one of
  those packages shows the repository root and the package path together.
- AC-1.7 A repository nested inside a monorepo is treated as its own repository, and the
  outer workspace's configuration is not applied to it.
- AC-1.8 Detection over a repository with a populated `node_modules` stays bounded: no
  dependency is reported as a package, and the walk has a fixed visit budget.

**Dependencies.** SQLite persistence; `projects` module; filesystem probe; `monorepo`
module ([ADR-0010](../adr/0010-monorepo-detection.md)).

**Risks.** Directory-based identity breaks when users move folders (mitigated by
*Relocate*). Scanning very large directories on add — mitigated by probing only for
known marker files at depth 1, never a recursive walk.

**As built (slice 1).** FR-1.1, 1.2, 1.3, 1.5 and 1.6 are done, and removal is done
except for the confirmation copy naming the project, which it does. The folder is chosen
by a native picker opened in Rust: the interface never sends a path, so a directory
already registered is reported by name rather than as a constraint violation. **Not yet
built:** rename and recolour (FR-1.4), *Relocate* for a missing project (FR-1.5 — a
missing project is shown as missing with *Remove* offered, and is never auto-deleted),
manual drag ordering (FR-1.6), the filter field (FR-1.7), and the `⌘1…⌘9` jumps. UNC and
network-path flagging (AC-1.5) is not implemented; a slow share currently blocks the add
for as long as the operating system takes to answer.

**Monorepo awareness (slice 1.1).** FR-1.8 through FR-1.11 are done, for all six tools.
Detection is read-only and computed on demand — nothing about a workspace is stored, and
no package becomes a project on its own. **Not yet built:** adding a detected package as
a project in one click, and per-package Git scoping (a package shows its repository's
state, which is what Git itself reports for a path inside a worktree).

---

## 3. Git status

**Purpose.** Answer "is this project clean?" without opening a terminal.

**User experience.** Project row: a dot — clean (muted), dirty (accent), conflicted
(warning). Detail view: counts for staged / unstaged / untracked / conflicted, and a
scrollable path list grouped by state. Rows are read-only; clicking copies the path,
`⏎` reveals it in the OS file manager.

**Functional requirements.**
- FR-3.1 Compute status for the project's Git worktree, honouring `.gitignore` and
  `core.excludesFile`.
- FR-3.2 Report counts and paths for: staged, modified-unstaged, untracked,
  conflicted, deleted, renamed.
- FR-3.3 Refresh on: project selection, window focus, an explicit refresh action, and
  a filesystem-watch debounce (500 ms) on the worktree.
- FR-3.4 Status is **read-only** pre-1.0. No stage, unstage, discard, commit.
- FR-3.5 Repos with > 5 000 changed paths report counts and truncate the path list to
  the first 500 with an explicit "…and N more" marker.
- FR-3.6 Non-repo projects show a neutral "not a Git repository" state, not an error.
- FR-3.7 Detached HEAD, bare repos, submodules, and worktrees do not crash; submodule
  contents are not recursed in 0.1.

**Platform considerations.** Filesystem watching differs per OS (FSEvents / ReadDirectoryChangesW /
inotify); inotify watch limits on Linux mean the watcher may fail on huge trees —
Mira falls back to focus-triggered polling and says so. Windows antivirus can make
status slow on large repos; all Git work is off the UI thread with a visible pending
state.

**Acceptance criteria.**
- AC-3.1 A repo with 100 changed files reports correct counts within 300 ms warm.
- AC-3.2 Editing a file in the editor updates Mira's dirty dot within 1 s while the
  window is open.
- AC-3.3 A 50 000-file repo does not block the UI; the pending state appears and
  resolves.
- AC-3.4 A conflicted merge state renders as conflicted, not merely dirty.

**Dependencies.** `git` module (libgit2); filesystem watcher.

**Risks.** Status on very large monorepos is inherently slow. Mitigation: per-repo
timeout (2 s) after which Mira shows counts-unknown rather than hanging.

**As built (slice 1).** FR-3.1, 3.4, 3.6 and 3.7 are done, and the 2 s timeout is in
place: a repository that takes longer reports the timeout with the project named, rather
than hanging the view. Status is a **count and a clean/dirty state**, not yet the
per-path list. **Not yet built:** the grouped path list and its per-state counts
(FR-3.2), so FR-3.5's truncation does not apply yet either. Conflicted state is not yet
distinguished from dirty (AC-3.4).

**Refreshed (slice 2).** FR-3.3 is now live: Git is re-read every five seconds by the
one scheduler ([ADR-0011](../adr/0011-one-scheduler.md)), and on project selection and
explicit refresh. The filesystem-watch trigger is deliberately still absent — bounded
polling gives a five-second worst case, and a watcher is an optimisation on top of the
scheduler rather than a prerequisite for it. The project list now carries a dirty
indicator, and every reading shows its age.

---

## 4. Last commit

**Purpose.** Orientation: "where did I leave this?"

**User experience.** Detail header shows subject (one line, truncated), author,
relative time ("2 h ago", absolute on hover), and short SHA. Click copies the SHA.

**Functional requirements.**
- FR-4.1 Read HEAD's commit: short + full SHA, subject, author name, author time.
- FR-4.2 Relative time updates while the window is open.
- FR-4.3 Empty repository (no commits) shows "no commits yet".
- FR-4.4 Detached HEAD shows the SHA in place of a branch name and marks it detached.

**Platform considerations.** None.

**Acceptance criteria.**
- AC-4.1 Correct subject/author/time for HEAD, verified against `git log -1`.
- AC-4.2 A newly initialised empty repo renders without error.

**Dependencies.** Feature 3.

**Risks.** Minimal. Non-UTF-8 commit messages must not panic — lossy-decode.

---

## 5. Branch information

**Purpose.** The single most-asked question about a repo.

**User experience.** Branch name in the project row and detail header, with
ahead/behind chips (`↑2 ↓1`) against the tracked upstream. A dropdown lists local
branches for **copying names** — not for checkout, pre-1.0.

**Functional requirements.**
- FR-5.1 Current branch, or detached-HEAD SHA.
- FR-5.2 Upstream tracking branch and ahead/behind counts, computed **locally** — Mira
  does not fetch pre-1.0, so counts reflect the last fetch the user performed.
- FR-5.3 Explicitly label staleness: hovering ahead/behind shows "as of last fetch".
- FR-5.4 List local branches with last-commit time; copy name on click.
- FR-5.5 No checkout, create, delete, or push pre-1.0.

**Platform considerations.** None.

**Acceptance criteria.**
- AC-5.1 Ahead/behind matches `git rev-list --left-right --count @{u}...HEAD`.
- AC-5.2 A branch with no upstream shows no chips and no error.
- AC-5.3 No network request is made by this feature (verified by inspection).

**Dependencies.** Feature 3.

**Risks.** Users may read stale ahead/behind as live. Mitigated by FR-5.3's explicit
labelling — a real risk we choose to solve with honesty rather than background fetch.

---

## 6. Basic Git graph

**Purpose.** Recent history, readable at a glance — "what happened here lately".

**Basic means basic.** In 0.1 this is a linear commit list, not a drawn graph. Lane
assignment, branch lines, and merge rendering are the 0.2 extension below. The split is
deliberate: lane layout is the fiddliest UI in the product, and the first release is not
worth holding for it.

**User experience.** A vertical list of the last N commits, newest first, with refs
badged on their commits. Click a commit for subject/body, author, SHA-copy. Not a diff
viewer.

**Functional requirements (0.1).**
- FR-6.1 Walk commits from HEAD (and, optionally, all local refs) newest-first,
  default limit 200, "load more" in pages of 200.
- FR-6.2 Badge local branches, remote-tracking branches, and tags on their commits.
- FR-6.3 Show commit subject, author, relative time; full body in the detail pane.
- FR-6.4 **No** blame, checkout, revert, cherry-pick, or rebase — those are anti-scope,
  not deferrals.
- FR-6.5 No lane rendering. A commit with two parents is listed as one row and labelled
  a merge; it does not draw branch lines.
- FR-6.6 Commit walking happens off the UI thread and is cancellable on navigation.

**Platform considerations.** None.

**Acceptance criteria (0.1).**
- AC-6.1 200 commits render in < 500 ms on a repo with 50 000 commits.
- AC-6.2 A merge commit is visibly labelled as one.
- AC-6.3 Navigating away mid-walk cancels it (no CPU after leaving the view).

**Dependencies.** Features 3–5.

**Risks.** Low, by construction — the risk was moved to 0.2 along with the lanes.

### 0.2 extension — better Git visualization

**Phase 0.2 · Slice 5b.** Adds to this feature rather than replacing it.

- FR-6.7 Compute lane assignment for parents/merges; render ≤ 8 lanes, collapsing
  beyond that with an indicator.
- FR-6.8 Diff view for a single commit, read-only.
- FR-6.9 File history for a path.
- FR-6.10 Graph filtering by branch, author, or path.
- AC-6.4 A merge commit shows two parent lanes correctly.
- AC-6.5 Lane computation stays within the 0.1 render budget on the same fixture repo.

**Risks.** Graph layout is the fiddliest UI in the product. Mitigation: the lane
algorithm lives in Rust, is pure, and is unit-tested against fixture repos — it never
becomes React logic. Still read-only: no rebase or merge UI, in this phase or any
other.

---

## 7. Running ports

**Purpose.** The flagship answer: *what is on :3000, and is it mine?*

**User experience.** Project detail shows a Ports section: port, owning process name,
PID, and a relevance marker (Ⓟ = process's working directory is inside this project).
Actions per row: open `http://localhost:<port>`, copy URL, kill process (confirm).
A global Ports view lists every listening port on the machine, grouped by project where
attributable.

**Functional requirements.**
- FR-7.1 Enumerate **listening** TCP sockets (IPv4 + IPv6) with owning PID and process
  name. UDP is out of scope pre-1.0.
- FR-7.2 Attribute a port to a project when the owning process's working directory (or
  executable path) is inside the project root; otherwise leave unattributed.
- FR-7.3 A workspace may declare expected ports; those render even when *not* listening,
  as "expected, not running".
- FR-7.4 Poll every 5 s while a relevant view is open; never poll when no window is
  visible (tray-only) except on an explicit tray action.
- FR-7.5 "Open in browser" uses `http://localhost:<port>` by default; the scheme and
  path are configurable per workspace (e.g. `https`, `/admin`).
- FR-7.6 Kill = feature 8's terminate flow, with the same confirmation.
- FR-7.7 Ports owned by other users or by the system show without a PID/name where the
  OS withholds it, marked "not attributable".

**Platform considerations.**
- **macOS:** enumerating other users' sockets requires elevation; Mira enumerates what
  the current user can see and labels the rest "not attributable". Mira never asks for
  root.
- **Windows:** the IP Helper API returns PIDs for the current user's sockets; some
  system-owned ports report PID 4/0 and are non-actionable — rendered read-only.
- **Linux:** reads `/proc/net/tcp{,6}` + `/proc/<pid>/fd`; sockets of other users are
  visible but not attributable to a process without permission. Containers publish
  ports owned by the Docker daemon — attributed to Docker, not the project (see
  feature 14 for the container link).

**Acceptance criteria.**
- AC-7.1 Starting a dev server on :5173 in a project surfaces it in that project within
  one poll cycle (≤ 5 s), correctly attributed.
- AC-7.2 Ports the user cannot introspect appear as "not attributable" rather than
  being hidden or faked.
- AC-7.3 Closing all windows stops port polling (verified: no syscalls while idle).
- AC-7.4 Enumeration of a machine with 200 listening sockets completes in < 200 ms.

**Dependencies.** `ports` module; `processes` module; browser integration (feature 12).

**Risks.** Attribution is heuristic and will sometimes be wrong. Mitigation: never
*hide* a port because attribution failed, and never auto-act on attribution — the
destructive action (kill) always names the process and asks.

---

**As built (slice 2).** Listening TCP sockets are enumerated every five seconds through
native platform interfaces — never by running `lsof` and parsing it. Each service shows
its port, bound address, owning pid and process name, and is placed in a project by the
**working directory of the owning process** and by nothing else. Where it cannot be
placed it is shown as unattributed with the reason. **Not yet built:** the machine-wide
Ports view, expected ports, and the port-conflict warning. Actions are read-only —
open, copy URL, copy port, copy PID — and terminating a process is a later slice with
its own confirmation design.

## 8. Process information

**Purpose.** Enough process context to decide whether to kill something.

**User experience.** A process row shows name, PID, CPU %, memory, uptime, and command
line (truncated, full on hover/expand). One action: *Terminate*, behind a confirmation
naming the process and PID.

**Functional requirements.**
- FR-8.1 For a PID: name, executable path, command line, working directory (where the
  OS permits), parent PID, start time, CPU %, RSS.
- FR-8.2 List processes relevant to a project — those whose cwd/exe is inside the
  project root, plus those owning its ports. Mira is **not** a general task manager and
  does not show an unfiltered system process list.
- FR-8.3 Terminate sends a graceful signal first (`SIGTERM` / `WM_CLOSE`+`TerminateProcess`
  fallback on Windows), waits 5 s, then offers force-kill as a separate explicit action.
- FR-8.4 Never terminate PID 0/1, the Mira process itself, or a process the user does
  not own — those actions are disabled with a reason.
- FR-8.5 Confirmation dialog states process name, PID, and project attribution.
- FR-8.6 Sampling CPU % requires two samples; the first render shows "—", not 0%.

**Platform considerations.**
- **macOS:** `libproc` gives cwd for own-user processes only; command lines of other
  users' processes may be withheld. No root escalation, ever.
- **Windows:** graceful termination of a console/GUI app is best-effort; Mira documents
  that "terminate" is closer to `taskkill` than to Ctrl-C. Elevated processes cannot be
  terminated by a non-elevated Mira — the action is disabled, not attempted.
- **Linux:** cwd via `/proc/<pid>/cwd`; permission-denied is normal and rendered as
  unknown.

**Acceptance criteria.**
- AC-8.1 Terminating a `npm run dev` process frees its port within one poll cycle.
- AC-8.2 The terminate action is disabled with an explanatory tooltip for processes the
  user cannot signal.
- AC-8.3 No process is ever terminated without an explicit confirmation click.
- AC-8.4 Force-kill is never automatic; it is a second, separate user action.

**Dependencies.** `processes` module; platform layer.

**Risks.** Killing the wrong thing is the highest-severity user harm in the product. Mitigations
are structural: confirmation naming the target, no bulk kill, no "kill all", no
keyboard-only path that can fire without reading the dialog.

---

**As built (slice 2).** Read-only facts for the processes that own listening sockets:
pid, name, executable, parent, and working directory where the platform exposes one.
Deliberately **not** the whole process table — there is no method that returns it, so
Mira cannot become an activity monitor by accident. **Not yet built:** CPU and memory
per process, uptime, the command line, and the process detail view. Termination is
absent from the code rather than merely unused, and a guard test keeps it that way.

## 9. Application launching

**Purpose.** Turn "open my project in X" into one keystroke, with the path already right.

**User experience.** A project row of app buttons: Editor, Terminal, Browser, plus any
custom app the user configured. Click launches; the button flashes on success and shows
an inline error on failure ("VS Code not found — configure in Settings").

**Functional requirements.**
- FR-9.1 Configure per-project (and per-workspace override) target applications by
  choosing from detected candidates or entering an explicit path/command.
- FR-9.2 Detect installed candidates per platform (bundle IDs on macOS, registry/known
  paths on Windows, `.desktop` entries / `$PATH` on Linux).
- FR-9.3 Launch with an argument template supporting `{path}`, `{file}`, `{line}`,
  `{url}`, `{port}` placeholders.
- FR-9.4 Launching is **spawn-and-forget**: Mira does not keep the child attached, does
  not capture its output, and does not die with it.
- FR-9.5 Arguments are passed as an argv array — **never** through a shell. No string
  interpolation into a command line.
- FR-9.6 A failed launch (binary missing, non-zero immediate exit) surfaces a specific
  error, never a silent no-op.
- FR-9.7 Custom commands per workspace: name, program, argv template, working directory.

**Platform considerations.**
- **macOS:** prefer `open -b <bundle-id> --args`, falling back to a direct executable
  path. Sandboxing is not used (Mira ships outside the App Store), but Gatekeeper may
  prompt on first launch of a user-specified binary.
- **Windows:** launch via `CreateProcess` with argv quoting handled by the platform
  layer; `.cmd`/`.bat` targets require special quoting and are supported explicitly.
- **Linux:** prefer `.desktop` `Exec` resolution via `gio launch`/`xdg-open` where
  appropriate; fall back to `$PATH` lookup. Flatpak/Snap editors need their own
  launch form and are detected as distinct candidates.

**Acceptance criteria.**
- AC-9.1 On a machine with VS Code installed, a fresh project offers it as a detected
  editor candidate with no manual configuration.
- AC-9.2 Launching opens the editor at the project root.
- AC-9.3 Quitting Mira does not close applications it launched.
- AC-9.4 A template containing shell metacharacters (`;`, `&&`, backticks) is passed
  literally as an argument and executes nothing extra — asserted by test.

**Dependencies.** `platform` module; `applications` registry.

**Risks.** Command injection is the primary security risk in the product. Mitigation is
absolute: argv arrays only, no shell, no user-supplied string reaching a shell parser.
See [security-and-privacy.md](../architecture/security-and-privacy.md).

---

## 10. Terminal integration

**Purpose.** "Open a terminal *here*" — the single most repeated developer action.

**User experience.** One click opens the configured terminal in the project (or
workspace) directory. Workspace commands appear as buttons that open a terminal
*running* that command.

**Functional requirements.**
- FR-10.1 Detect and support common terminals: macOS (Terminal.app, iTerm2, WezTerm,
  Alacritty, Kitty, Ghostty), Windows (Windows Terminal, PowerShell, cmd, WezTerm),
  Linux (the `x-terminal-emulator` alternative, GNOME Terminal, Konsole, Alacritty,
  Kitty, WezTerm, Foot).
- FR-10.2 Open at the workspace root (falling back to project root).
- FR-10.3 Optionally run a configured command on open; the command is passed per that
  terminal's documented argv contract, per terminal, not by string concatenation.
- FR-10.4 Mira does **not** embed a terminal emulator, capture output, or manage the
  session's lifetime.
- FR-10.5 If no terminal is detected, Settings shows a clear "choose your terminal"
  state rather than a broken button.

**Platform considerations.** Each terminal has its own working-directory and
run-command flags (`-d`, `--working-directory`, `--cwd`, `start-directory`); these live
in a per-terminal descriptor table in the platform layer, not in UI code. Windows
Terminal profiles complicate "run this command" and are handled by an explicit
`wt.exe -d <dir> -- <argv>` form.

**Acceptance criteria.**
- AC-10.1 On each supported OS, opening the default terminal lands in the correct
  directory (`pwd` matches the workspace root).
- AC-10.2 A configured command with spaces and quotes in its arguments runs correctly
  and unmodified.
- AC-10.3 Closing Mira leaves the terminal running.

**Dependencies.** Feature 9.

**Risks.** Terminal-specific flag drift over releases. Mitigation: descriptors are data,
contributable in one file, with a documented "add your terminal" contribution path.

---

## 11. Editor integration

**Purpose.** Open the project — or a specific file and line — in the right editor.

**User experience.** Editor button on the project; from the Shelf and Quick Peek, an
"open in editor" action that jumps to the exact file (and line where known).

**Functional requirements.**
- FR-11.1 Detect common editors: VS Code (+ Insiders/VSCodium), Cursor, Zed, JetBrains
  IDEs (via their CLI launchers), Sublime Text, Neovim/Vim (through the configured
  terminal), Emacs, Helix.
- FR-11.2 Support opening: project root, a file, and a file at `{line}` where the
  editor's CLI supports it; where it does not, open the file and say so once.
- FR-11.3 Per-project override of the global default editor; per-workspace override of
  the project default.
- FR-11.4 Reuse an existing window where the editor's CLI offers it (e.g. `code -r`),
  configurable.

**Platform considerations.** JetBrains CLI launchers are opt-in installs and are absent
by default — detect and, if missing, link the user to the IDE's "create command-line
launcher" action rather than silently failing. On macOS, editors installed as `.app`
bundles are launched by bundle ID; on Windows, per-user installs live under
`%LOCALAPPDATA%` and are detected from the registry.

**Acceptance criteria.**
- AC-11.1 "Open file at line" lands on the correct line in VS Code, Cursor, and Zed.
- AC-11.2 An editor configured but since uninstalled produces a specific error naming
  the missing binary.

**Dependencies.** Feature 9.

**Risks.** Editor CLI surface varies. Mitigation: same descriptor-table approach as
terminals; capability flags per editor (`supports_line`, `supports_reuse_window`).

---

## 12. Browser integration

**Purpose.** Get to the running app or a project URL in one action.

**User experience.** Browser button opens the workspace's primary URL (default
`http://localhost:<first expected or detected port>`). Port rows carry their own
"open" action. A workspace may hold a small set of named links (staging, docs, board).

**Functional requirements.**
- FR-12.1 Open a URL in the system default browser, or in a specific configured browser.
- FR-12.2 Detect installed browsers as launch candidates.
- FR-12.3 Named links per workspace: label + URL, ordered, opened by click or keyboard.
- FR-12.4 Only `http`, `https`, and `file` schemes may be opened. Everything else is
  refused with a message — no `javascript:`, no arbitrary custom schemes.
- FR-12.5 URLs are validated and percent-encoded before launch.

**Platform considerations.** Default-browser resolution: `open` (macOS),
`ShellExecute`/`start` (Windows), `xdg-open` (Linux). Linux desktops without
`xdg-utils` degrade to a configured browser binary.

**Acceptance criteria.**
- AC-12.1 Clicking a port's open action loads the app in the default browser.
- AC-12.2 A link with a non-http(s)/file scheme is refused with a visible reason.
- AC-12.3 A URL containing spaces or unicode opens correctly.

**Dependencies.** Feature 9; feature 7.

**Risks.** Scheme abuse (a malicious project config opening a dangerous URI).
Mitigated by the FR-12.4 allowlist.

---

## 20. Global shortcut

**Purpose.** Mira must be one keystroke away or it will not be used.

**User experience.** A default chord toggles the compact window: focused and typing-ready
if hidden, dismissed if visible. Rebindable in Settings with live conflict detection.

**Functional requirements.**
- FR-20.1 Default binding: `⌥Space` (macOS), `Ctrl+Alt+Space` (Windows/Linux) —
  chosen to avoid Spotlight (`⌘Space`) and common IME toggles.
- FR-20.2 Rebindable; the new binding is validated before being saved, and registration
  failure is reported immediately with the conflicting-owner reason where the OS gives one.
- FR-20.3 Toggle semantics: show+focus when hidden, hide when visible and focused.
- FR-20.4 The window appears on the display containing the cursor.
- FR-20.5 If the shortcut cannot be registered at all, Mira still runs; the tray icon
  and (feature 21) a documented fallback remain.

**Platform considerations.**
- **macOS: Full.** Registration may require Accessibility/Input Monitoring permission
  for some chords; Mira prompts and explains once, and never silently retries.
- **Windows: Full.** `RegisterHotKey` fails if another app owns the chord; the error is
  surfaced with the chord named.
- **Linux/X11: Full.**
- **Linux/Wayland: Unavailable.** Wayland has no cross-compositor global-shortcut
  protocol; Tauri's global-shortcut implementation is X11-specific and is disabled on
  Wayland to avoid a libX11 crash. **Mira's fallback:** the app is single-instance, and
  running `mira --toggle` raises/hides the window. Settings shows the exact command and
  a per-desktop recipe (GNOME/KDE custom shortcut) so the user binds it in their own
  compositor. This is documented as Unavailable-with-fallback, never presented as working.

**Acceptance criteria.**
- AC-20.1 Warm toggle to interactive window ≤ 250 ms on all platforms.
- AC-20.2 A chord already owned by another app produces a specific, visible error.
- AC-20.3 On Wayland, Settings shows the fallback recipe and `mira --toggle` works,
  raising the existing instance rather than starting a second one.
- AC-20.4 Mira starts and is usable with no shortcut registered at all.

**Dependencies.** `tauri-plugin-global-shortcut`, `tauri-plugin-single-instance`, CLI arg
handling.

**Risks.** Wayland is a growing share of Linux desktops and this gap will not close on
Mira's schedule. Accepted and documented.

---

## 21. Menu bar / system tray

**Purpose.** Mira's resting state. It lives here, not in the dock/taskbar.

**User experience.** A monochrome icon; click (or menu, on Linux) reveals a compact
menu: active projects with status dots, toggle window, recent projects, quit. Optional
badge/marker when something wants attention (e.g. a port died) — off by default.

**Functional requirements.**
- FR-21.1 Tray/menu-bar icon present whenever Mira runs.
- FR-21.2 Menu: toggle main window, up to 5 recent projects (each opening its detail
  view), Settings, Quit.
- FR-21.3 Closing the last window does not quit Mira; Quit is explicit.
- FR-21.4 Optional "start on login" setting, off by default.
- FR-21.5 The icon adapts to light/dark system appearance (template rendering).

**Platform considerations.**
- **macOS: Full.** Menu-bar item with template image; the app runs as an accessory
  (no dock icon) by default, configurable.
- **Windows: Full.** Notification-area icon; users may hide icons via OS settings —
  first run tells them where it went.
- **Linux: Degraded.** Tray relies on libayatana-appindicator/StatusNotifierItem.
  Click events do not fire on Linux (a libappindicator limitation), so **the menu is
  the only interaction** — Mira's Linux tray is designed menu-first rather than
  click-to-toggle. Some desktops (notably stock GNOME) need an extension for tray
  icons at all; if the icon cannot be created Mira says so and continues, with the
  window and `mira --toggle` as the entry points.

**Acceptance criteria.**
- AC-21.1 Every 0.1 entry point is reachable from the tray menu on all three platforms.
- AC-21.2 On Linux, no feature is reachable *only* by clicking the tray icon.
- AC-21.3 Closing the window leaves Mira running with idle CPU < 1%.
- AC-21.4 Tray creation failure is reported once and does not abort startup.

**Dependencies.** Tauri tray APIs; feature 20.

**Risks.** Linux tray support is genuinely uneven. Mitigation: the menu-first design
plus the CLI toggle means the tray is never the sole path to anything.

---

# 0.2 — Workspace

Post-MVP. Several projects stop being a list and become a place you work.

---

## 2. Project/workspace management

**Purpose.** A project is *where the code is*; a **workspace** is *a way of working on
it* (e.g. "backend + db", "frontend only", "debugging prod issue"). Workspaces let one
project carry different tool/port/command sets without duplication.

**User experience.** Each project has at least a `Default` workspace, created
implicitly and never shown as a choice until a second one exists — a single-workspace
user never learns the concept. Adding a second reveals a workspace switcher in the
project header. Switching changes which commands, ports of interest, and app targets
are surfaced. Switching never kills anything.

**Functional requirements.**
- FR-2.1 Every project has ≥ 1 workspace; the implicit `Default` cannot be deleted
  while it is the only one.
- FR-2.2 A workspace stores: name, optional subdirectory root (relative to project
  root), expected ports, configured commands, preferred apps overriding project
  defaults, and layout/session references.
- FR-2.3 Create, rename, duplicate, delete workspaces. Deleting warns if it holds
  commands or a saved session.
- FR-2.4 Switching workspace is a pure view/config change — no processes started or
  stopped, ever.
- FR-2.5 The active workspace per project is remembered across restarts.
- FR-2.6 Multiple projects are "active" simultaneously; there is **no** global
  single-workspace mode.

**Platform considerations.** None beyond path handling (FR-1 platform notes).

**Acceptance criteria.**
- AC-2.1 A new project shows no workspace UI at all until a second workspace exists.
- AC-2.2 Switching workspaces changes the displayed command/port set in < 100 ms and
  starts/stops nothing (verified: process list unchanged).
- AC-2.3 Deleting a workspace leaves the project and its other workspaces intact.

**Dependencies.** Feature 1; `workspaces` module.

**Risks.** Concept overload — two nouns where users expect one. Mitigated by hiding
workspaces until a second exists, which is a hard requirement, not a nicety.

---

## 23. App groups *(shape only)*

**Phase 0.2 · Slice 4 · specified in full during the 0.2 design pass.**

**Shape.** A named set of applications, terminals, and URLs that belong together and can
be opened as one action. Groups are defined per workspace and are the unit that
[workspace restoration](#24-workspace-restoration-shape-only) saves and reopens; without
them, restoration has nothing to name. Launching a group uses the same argv-template
machinery as [feature 9](#9-application-launching) and inherits its no-shell rule
unchanged.

**Boundaries already settled.** A group launches applications; it never closes them. It
has no conditions and no triggers — a group that fires on an event is an automation rule
and belongs to 0.6+, behind the trust model.

---

## 24. Workspace restoration *(shape only)*

**Phase 0.2 · Slice 11 · specified in full during the 0.2 design pass.**

**Shape.** Save the set of open applications, terminals, URLs, and expected containers
for a workspace, and reopen them on demand.

**Boundaries already settled.** Restoration is always an explicit user action, never
automatic on launch. It never force-closes anything that is already running, and it
never kills a process to make room for the one it is starting. The user sees a preview
of exactly what is about to open, before it opens. Restoration that surprises people is
worse than no restoration at all.

---

# 0.3 — Shelf

Post-MVP. A place to put a file that is not yet a decision.

---

## 15. File Shelf

**Purpose.** A place to park the four files you keep re-finding — cross-project, not a
file browser.

**User experience.** A Shelf panel per project plus a global shelf. Drag a file in from
the OS, or add from Quick Peek. Items show name, parent directory, and a type icon.
Actions: open in editor, reveal in file manager, copy path, drag out to another app,
remove from shelf.

**Functional requirements.**
- FR-15.1 Add by drag-and-drop from the OS, by file picker, or from Quick Peek.
- FR-15.2 Store a **reference** (absolute path + optional note + added-at), never a copy
  of the file's contents.
- FR-15.3 Items whose file no longer exists render as *Missing*; the user removes or
  relocates them. Mira never deletes shelf items automatically.
- FR-15.4 Reorder by drag; scope an item to a project or to the global shelf.
- FR-15.5 Removing from the Shelf **never** touches the file on disk. There is no
  delete-file action anywhere in Mira.
- FR-15.6 Directories may be shelved as well as files.
- FR-15.7 Soft cap of 200 items per shelf with a clear message at the limit.

**Platform considerations.** Drag-out to another application is well supported on
macOS and Windows; on Linux it depends on the desktop environment and is treated as
Degraded where the webview cannot originate a file drag. Reveal-in-file-manager uses
`open -R` / explorer `/select,` / `xdg-open` on the parent directory (Linux cannot
reliably select the file itself — documented, not faked).

**Acceptance criteria.**
- AC-15.1 A dropped file appears on the Shelf and opens in the configured editor.
- AC-15.2 Removing an item leaves the file on disk (verified by test).
- AC-15.3 A shelved file that is deleted externally shows as Missing without breaking
  the panel.

**Dependencies.** `shelf` module; feature 11.

**Risks.** Users may assume the Shelf stores content, then delete the original.
Mitigated by wording ("reference to") and by the Missing state being explicit.

---

## 16. Quick Peek

**Purpose.** Look at a file *without* leaving the keyboard or loading an IDE.

**User experience.** From the Shelf, Git status list, or a fuzzy path search inside a
project, `Space` opens a read-only preview overlay: text with syntax highlighting,
images, and a metadata card for anything else. `⏎` opens it in the editor, `Esc`
closes.

**Functional requirements.**
- FR-16.1 Preview text files with syntax highlighting, up to 2 MB; larger files show
  the first 2 MB with a clear truncation marker.
- FR-16.2 Preview common images (png, jpg, gif, webp, svg) and render SVG **as an
  image, sandboxed** — never as inline DOM.
- FR-16.3 Anything else: name, size, type, modified time, path, plus open/reveal.
- FR-16.4 Read-only. No editing, no saving, no execution.
- FR-16.5 Binary detection by null-byte sniffing of the first 8 KB before any attempt to
  render as text.
- FR-16.6 File reads are restricted to paths under a registered project root or the
  Shelf — Mira does not become an arbitrary file reader.
- FR-16.7 Fuzzy path search within a project respects `.gitignore` and skips
  `node_modules`, `target`, `dist`, `.git`, and equivalents.

**Platform considerations.** None beyond path handling; large-file reads are streamed
off-thread on all platforms.

**Acceptance criteria.**
- AC-16.1 A 5 000-line source file renders highlighted in < 200 ms.
- AC-16.2 A 500 MB binary does not freeze the UI and is not read into memory.
- AC-16.3 An SVG containing a script tag renders without executing it (asserted).
- AC-16.4 A path outside every project root is refused by the backend command.

**Dependencies.** Feature 15; `peek` module; `filesystem` module.

**Risks.** Untrusted file content rendered in a webview. Mitigations: images via
sandboxed `<img>`/blob URLs, no `innerHTML` for file content, highlighting on
escaped text, strict CSP.

---

# 0.4 — System

Post-MVP. Mira notices the state of the machine it lives on.

---

## 17. System status

**Purpose.** Ambient machine context — enough to explain "why is everything slow".

**User experience.** A compact strip: CPU load, memory pressure, disk free on the
project's volume, battery state (where present), network reachability. Sparklines, no
dashboard.

**Functional requirements.**
- FR-17.1 Sample CPU %, total/used memory, per-volume free space, battery
  percentage/charging state, and whether a default route exists.
- FR-17.2 Sample every 5 s while visible; **never** when no window is open.
- FR-17.3 Desktops without a battery hide the battery element entirely.
- FR-17.4 Network status is local-only (route/interface state). Mira does not ping a
  remote host to determine "online".
- FR-17.5 Keep at most 60 samples in memory for sparklines; persist nothing.

**Platform considerations.** Battery reporting is unavailable on most desktops and on
some VMs; memory "pressure" means different things per OS, so Mira reports plain
used/total rather than inventing a unified pressure score.

**Acceptance criteria.**
- AC-17.1 Values match the OS's own tools within a reasonable margin.
- AC-17.2 Sampling stops when the window closes (verified: no CPU wake-ups).
- AC-17.3 No system metric is written to disk.

**Dependencies.** `system` module (sysinfo).

**Risks.** Polling defeats the lightweight principle. Mitigation: visibility-gated
sampling is a hard requirement with a test.

---

## 18. Media detection

**Purpose.** Label a session with what was playing — a memory hook, not a player.

**User experience.** When available, a one-line "now playing" in the compact window and
optionally attached to a saved session. Playback controls are **not** part of any 0.x
milestone — see [product-scope.md](product-scope.md) §5.

**Functional requirements.**
- FR-18.1 Read current track title, artist, and source application where the OS exposes
  it through a public interface.
- FR-18.2 Off by default; enabling it is an explicit setting.
- FR-18.3 Where unsupported, the setting is visibly disabled with the reason.
- FR-18.4 Media metadata is never persisted unless the user attaches it to a session.

**Platform considerations — this feature has real, unfixable parity gaps.**
- **Linux: Full.** MPRIS2 over D-Bus is a public, stable interface.
- **Windows: Full.** `GlobalSystemMediaTransportControlsSessionManager` (WinRT) is
  public API.
- **macOS: Unavailable.** The only route to now-playing metadata is the private
  `MediaRemote` framework, and since macOS 15.4 `mediaremoted` refuses unentitled
  clients. The documented community workarounds require disabling SIP or piggy-backing
  on entitled system binaries. **Mira will not ship either.** On macOS the setting
  reads "Not available on macOS — Apple restricts this API to entitled apps", and no
  private framework is linked.

**Acceptance criteria.**
- AC-18.1 On Linux with a MPRIS-capable player, the current track appears within 2 s.
- AC-18.2 On Windows with Spotify/Groove playing, the current track appears.
- AC-18.3 On macOS the feature reports Unavailable with the stated reason, and the
  binary links no private framework (asserted by an `otool -L` check in CI).
- AC-18.4 Disabled by default on every platform.

**Dependencies.** `media` module; platform layer.

**Risks.** Temptation to use the private API for parity. This is settled: no. Revisit
only if Apple ships a public interface.

---

## 19. Lock/session awareness

**Purpose.** Know when you stepped away, so Mira can pause work and time sessions
honestly.

**User experience.** Mostly invisible. On lock/sleep, polling stops and the active
session is marked paused; on unlock, a small "welcome back — 47 min away" line with the
project you were on.

**Functional requirements.**
- FR-19.1 Detect: screen locked, screen unlocked, system sleep, system wake.
- FR-19.2 On lock/sleep: suspend all polling (ports, processes, Docker, system, media)
  and stop timers.
- FR-19.3 On unlock/wake: resume polling and refresh the visible view once.
- FR-19.4 Session time excludes locked/asleep intervals.
- FR-19.5 Never record *what* the user did while away; only the interval boundaries.

**Platform considerations.**
- **macOS: Full.** `com.apple.screenIsLocked` / `com.apple.screenIsUnlocked`
  distributed notifications, plus `NSWorkspace` sleep/wake notifications.
- **Windows: Full.** `WTSRegisterSessionNotification` for lock/unlock plus
  `WM_POWERBROADCAST` for suspend/resume.
- **Linux: Degraded.** `org.freedesktop.login1`'s `LockedHint` property and
  `PrepareForSleep` signal cover systemd-logind desktops (most). Non-logind systems and
  some lock screens do not report lock; there Mira falls back to window-focus and idle
  heuristics and marks the capability Degraded.

**Acceptance criteria.**
- AC-19.1 Locking the screen stops all polling within 2 s (verified by syscall trace).
- AC-19.2 Unlocking resumes and refreshes exactly once.
- AC-19.3 A 30-minute lock does not count toward session time.
- AC-19.4 On a non-logind Linux system, Mira runs correctly with the capability
  reported as Degraded.

**Dependencies.** `sessions` module; platform layer.

**Risks.** Linux fragmentation. Mitigation: honest Degraded state; sleep/wake alone
still covers the main battery-saving case.

---

## 25. Displays *(shape only)*

**Phase 0.4 · Slice 9 · specified in full during the 0.4 design pass.**

**Shape.** Awareness of the attached displays — how many, their arrangement, and which
one Mira's compact window is currently summoned onto. The immediate use is placing that
window predictably on a multi-monitor setup, which is a real irritation on all three
platforms today.

**Boundaries already settled.** Read-only. Mira reports the display configuration; it
never changes resolution, arrangement, or scaling. Display arrangement is per-platform
and will be `Degraded` where the OS does not expose it — Wayland in particular.

---

# 0.5 — Personality

Post-MVP. Mira stops looking like a dashboard.

An atmosphere may change design tokens and add one canvas layer. It may not change
layout, spacing, or the status language. That constraint is what keeps this phase a
short one.

---

## 22. Basic themes and personalization

**Purpose.** A tool you keep open should feel like yours — without becoming a toy.

**User experience.** Settings → Appearance: theme (System / Light / Dark), accent
colour, density (Comfortable / Compact), and an *Atmosphere* selector that is **Minimal
by default**. Non-minimal atmospheres add restrained ambient treatment; all of them stay
professional and are capped by a performance budget.

**Functional requirements.**
- FR-22.1 Light/Dark/System theme, following the OS immediately on change.
- FR-22.2 Accent colour from a curated set plus a custom picker; all combinations must
  keep WCAG AA contrast for text — the picker rejects failing choices.
- FR-22.3 Atmospheres in 0.5: **Minimal** (default), Cosmic, Sakura, Cyberpunk, Rain,
  Custom. Custom = user-supplied background image/colour + accent, no scripting.
- FR-22.4 Ambient motion is off unless the atmosphere defines it, respects
  `prefers-reduced-motion`, pauses entirely when the window is not focused, and is
  disabled automatically on battery below 20%.
- FR-22.5 Any atmosphere costing more than 2% CPU while idle-visible is not shipped.
- FR-22.6 Per-project accent colour overrides the global accent in that project's views.
- FR-22.7 No anime/mascot art is bundled or required; atmospheres are abstract.

**Platform considerations.** Vibrancy/acrylic/blur effects differ per OS and are treated
as optional enhancements, never as the basis of contrast or legibility.

**Acceptance criteria.**
- AC-22.1 Switching the OS theme updates Mira within 1 s with no restart.
- AC-22.2 Every atmosphere passes AA contrast for body text and controls.
- AC-22.3 With `prefers-reduced-motion`, no atmosphere animates.
- AC-22.4 Idle-visible CPU with the heaviest atmosphere stays under 2%.

**Dependencies.** [design-system.md](../ux/design-system.md).

**Risks.** Personalization eating performance and taste. Mitigations: Minimal default,
hard CPU budget, contrast gate, abstract-only art.

---

## 26. Workspace identities *(shape only)*

**Phase 0.5 · Slice 10 · specified in full during the 0.5 design pass.**

**Shape.** A workspace carries its own visual identity — accent, atmosphere, and
optionally an icon or short label — so that switching workspaces is recognisable
peripherally, before you have read anything. This is the personalization layer applied
to the concept 0.2 introduces.

**Boundaries already settled.** An identity is tokens and one canvas layer, like every
other atmosphere. It may not change layout, spacing, or the status language, and it is
subject to the same AA contrast gate and idle-CPU ceiling. A workspace with no identity
set looks exactly like the default — this never becomes required configuration.

---

## 27. Music-reactive ambience *(shape only)*

**Phase 0.5 · Slice 10 · specified in full during the 0.5 design pass.**

**Shape.** The ambient canvas layer responds to whatever is currently playing —
amplitude or tempo driving motion in the atmosphere, not a visualiser and not a
playback UI.

**Platform capability limitation, not a defect.** This depends on
[feature 18](#18-media-detection), which is `Unavailable` on macOS: there is no public
API for now-playing media, and the private framework that would provide one is not an
option — see [ADR-0006](../adr/0006-no-account-no-cloud.md) and the `otool -L` guard
test in CI. The feature is therefore designed to **degrade to a non-reactive
atmosphere** rather than to disappear or to render an error. It is Full on Windows
(GSMTC) and Linux (MPRIS). If Apple ships a public API, the capability is revisited
then; the constraint is Apple's, and it is allowed to change.

**Boundaries already settled.** Reactive motion obeys the same 2% idle-CPU ceiling as
every other atmosphere, stops entirely under `prefers-reduced-motion`, and pauses when
the window is unfocused. Mira reads playback state; it never controls playback.

---

# 0.6+ — Automation

Post-MVP, and gated. Nothing here starts before the MVP has shipped and been in real
use, a written trust model exists, and demand is evidenced by actual requests.

SSH and Docker awareness were previously scheduled earlier and keep their full
specifications below. They belong here because they are the read-only half of the
surface an automation rule would act on, and because they are the two most sensitive
integrations in the product. Contextual rules, plugins, and Astra integration are
deliberately unspecified — a rules engine that launches processes needs a trust model
before it needs a schema ([security-and-privacy.md](../architecture/security-and-privacy.md) §5).

---

## 13. SSH awareness

**Purpose.** Remote work is part of many projects; Mira should show whether the box is
reachable and whether a tunnel is up — **without becoming an SSH client**.

**User experience.** A project may list SSH hosts (imported from `~/.ssh/config` or
added manually). Each shows: alias, user@host:port, reachability, and whether an active
local `ssh` process references it. Actions: copy the `ssh` command, open a terminal
running it. Nothing else.

**Functional requirements.**
- FR-13.1 Parse `~/.ssh/config` read-only for `Host`/`HostName`/`User`/`Port`/
  `IdentityFile` **names**; associate hosts with projects manually.
- FR-13.2 **Never** read private key material, never read passphrases, never store
  credentials. `IdentityFile` is stored as a path string only, and the file is not
  opened.
- FR-13.3 Reachability = optional, opt-in, per-host TCP connect to the SSH port with a
  3 s timeout. Off by default; no probing happens until the user enables it.
- FR-13.4 Detect local `ssh` processes whose command line references a known host, to
  show "tunnel/session active"; parse the `-L`/`-R` forwards for display only.
- FR-13.5 Mira does not open, hold, or authenticate SSH connections itself.

**Platform considerations.** `~/.ssh/config` location differs on Windows
(`%USERPROFILE%\.ssh\config`); OpenSSH-for-Windows is standard but PuTTY users have no
such file — that case shows an empty state, not an error. Reading command lines of
`ssh` processes is subject to the same permission limits as feature 8.

**Acceptance criteria.**
- AC-13.1 Hosts parse from a realistic `~/.ssh/config` including `Include` directives
  and wildcard `Host *` blocks (wildcards are listed but not probed).
- AC-13.2 With reachability disabled (default), Mira makes zero network connections —
  verified by test.
- AC-13.3 No key file is ever read; asserted by a test that fails if the code opens a
  path from `IdentityFile`.

**Dependencies.** Features 8, 10.

**Risks.** Privacy: reading SSH config is sensitive. Mitigations: read-only, names only,
no key access, no probing by default, and an explicit first-run consent before the
config is parsed at all.

---

## 14. Docker awareness

**Purpose.** "Is my stack up?" without opening Docker Desktop.

**User experience.** If a project contains a `docker-compose.y*ml` or `Dockerfile`, a
Containers section lists related containers: name, image, state, uptime, published
ports (linked to feature 7). Actions: copy container name, open a published
port, open a terminal at the compose file. **No** start/stop/restart/exec/logs.

**Functional requirements.**
- FR-14.1 Detect Docker availability by probing the local daemon socket
  (`/var/run/docker.sock`, `npipe:////./pipe/docker_engine`, and the Docker Desktop
  per-user socket path); absence is a normal state, not an error.
- FR-14.2 List containers with name, image, status, created time, published ports,
  compose project label (`com.docker.compose.project`).
- FR-14.3 Associate containers with a Mira project by compose project name or by the
  compose file's working-directory label matching the project root.
- FR-14.4 Read-only. No lifecycle actions.
- FR-14.5 Poll only while a Docker-bearing view is open (5 s), never in the background.
- FR-14.6 Podman's Docker-compatible socket is used when present; other runtimes are
  out of scope pre-1.0.

**Platform considerations.** Socket paths differ per OS and per Docker Desktop version;
rootless Docker and Colima/OrbStack use non-default socket paths — Mira reads
`DOCKER_HOST` first, then a candidate list, and lets the user set the path in Settings.
On Windows, named-pipe access needs the user to be in `docker-users`; permission denied
renders as "Docker present but not accessible", with the reason.

**Acceptance criteria.**
- AC-14.1 With a compose stack up, the project shows its containers with correct state.
- AC-14.2 With Docker not installed, the section is absent — no error, no spinner.
- AC-14.3 Mira never sends a write request to the Docker API (asserted by test:
  only `GET` requests are issued).

**Dependencies.** Feature 7; `docker` module.

**Risks.** Access to the Docker socket is effectively root-equivalent; even read-only
use deserves care. Mitigation: read-only client, `GET`-only assertion test, explicit
documentation in the security doc.

---

# Future

Directionally accepted, deliberately undesigned. No commitments, no schema, no
abstractions built during the 0.x line to accommodate them.

- **Contextual automation** — rules like "when I open project X, start the compose
  stack and open the editor". Requires a trust model before design.
- **Plugin system** — third-party modules over a stable, capability-scoped interface.
  This is the same boundary Astra would use.
- **Custom integrations** — user-authored service adapters.
- **Deeper Astra integration** — optional, off by default, deletable.
- **Optional cloud functionality** — only with a strong, specific reason; would remain
  opt-in and never required. There is currently no such reason.

---

## Anti-scope (things that will not be built)

Stated permanently so they stop being proposed: code editing, an embedded terminal
emulator, general file browsing, git history rewriting, container image management, a
general app launcher, arbitrary macro recording, usage analytics, and any account
system.
