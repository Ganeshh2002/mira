# Changelog

All notable changes to Aviora Mira are documented here.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning: [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Product definition, product scope, PRD, information architecture, and design system
- Architecture, platform abstraction, data model, and security/privacy documents
- ADRs 0001–0014 covering the foundational technical decisions
- Locked phase plan (0.1 through 0.6+) and the fourteen-slice implementation roadmap
- Open-source project files (licence, contributing, code of conduct, security policy)
- **Foundation (slice 0).** Cargo workspace with `mira-core`, `mira-platform`, `mira-db`,
  `mira-projects`, and `mira-workspaces` behind the Tauri application boundary; runtime
  capability resolution reported as full, degraded, or unavailable with a reason; SQLite
  storage with a forward-only migration runner; typed commands with structured errors and
  TypeScript types generated from Rust; window, tray, settings surface, and global
  shortcut with a documented fallback; guard tests for the architectural rules; CI on
  macOS, Windows, and Linux.
- **Projects and Git context (slice 1).** Add a project by choosing a folder; Mira infers
  the name, detects Git by walking up to the worktree root, and records the project types
  it finds at the root. Projects persist, list most-recently-opened first, and keep
  independent state. Selecting one reads its Git context on demand: branch or detached
  HEAD, last commit with author and age, clean or changed, and ahead/behind against the
  tracked upstream — labelled as of the last fetch, because Mira does not fetch. Open a
  project's folder in the file manager, or remove a project without touching the folder.
  Read-only throughout: there is no commit, stage, checkout, push, or pull.
- **Monorepo awareness.** Mira tells a standalone repository, a monorepo root, and a
  package inside a monorepo apart. It reads workspace manifests — npm, pnpm and Yarn
  workspaces, Cargo workspaces, Turborepo, Nx — and never runs a package manager,
  installs anything, or writes to a repository. A package shows the repository it belongs
  to alongside its own path, so the two scopes are never confused. Detected packages are
  boundaries, not projects: nothing is adopted, and nothing about a workspace is stored.
- **Live project context (slice 2).** Mira now keeps up on its own. One scheduler owns
  every recurring task, gated on a window being visible and a project existing, so a
  hidden Mira does nothing at all. Git is re-read every five seconds — the project list
  gains a dirty indicator — and listening TCP ports are enumerated through native
  platform interfaces, with the process that owns each one. A service is placed in a
  project, and in the right package of a monorepo, by the **working directory** of its
  process and by nothing else; where that cannot be read, Mira says the service is there
  and that it cannot place it. Every reading shows its age, and a failed refresh appears
  beside what it could not replace rather than instead of it. Actions are read-only:
  open the address, copy the URL, the port, or the PID. Nothing here can stop a process.
- **Workspace context (slice 3).** A workspace is a user-defined working context on one
  project: a name, an optional description, and the kinds of application it works with.
  It has no directory of its own. Create as many as you like per project, switch between
  them, and each shows the project's Git state, its packages and its services — observed
  once for the project, so two workspaces can never disagree and switching costs no
  observation. Opening one records when; it starts nothing. Application context is a
  *kind* — editor, terminal, browser — with Mira discovering what this machine actually
  has, so a workspace that wants an editor says "Not installed" rather than forgetting it
  wanted one. Removing a project removes its workspaces, and the confirmation says so; a
  project whose folder went missing keeps everything.
- **Git history (slice 5a).** What happened lately, as a linear list: subject, author,
  how long ago, and the short id, one row per commit, twenty-five at a time with
  **Load more**. Opening a commit shows its body, both spellings of its id, the author
  and their email, the parent count and how many paths it changed — read-only, with no
  diff and nothing that could check out, revert or reset. Copying an id goes through a
  typed command that resolves the commit in the repository first, so the clipboard only
  ever receives something Mira read. History belongs to the **repository**: a package
  inside a monorepo shows the repository's history rather than a copy of its own.
  Every state has an answer — an empty repository, a branch with no commits, a detached
  HEAD, a shallow clone whose oldest row is where the copy stops, a history Git cannot
  follow, and a directory that is not a repository at all. History is read when you open
  it, refresh it, or ask for more; nothing polls it, and there is no way to ask Mira to
  walk a whole repository — a first page costs the same on a hundred commits as on ten
  thousand.
- **Keep Awake.** A small utility in the tray: Off, 30 minutes, 1 hour, or until turned
  off. It holds your operating system's own power request so the machine and the screen
  do not fall asleep on their own. It **never** simulates typing or pointer movement,
  never manufactures activity, and hides nothing from anything that reports it — an idle
  screen stays idle. It runs no program to do it. It is off by default, stops when you
  turn it off or when the time is up, is released when Mira quits, and never comes back
  by itself after a restart. Full on macOS; on Windows and Linux the control says why it
  is not available yet and points at the platform's own power settings
  ([ADR-0014](docs/adr/0014-keep-awake.md)).
- macOS and Windows ask the platform for its standard window material — Liquid Glass on
  macOS 26, Mica on Windows 11 — rather than drawing an imitation. Linux stays opaque.

The first release will be `0.1.0` — **the MVP** — covering roadmap slices 1–3 and 5a: projects, Git status and history, ports and
processes, application launching, tray, and the global shortcut. Ports, processes and
application launching are not built yet. Everything from 0.2 on is post-MVP and does not
block it. See [product scope](docs/product/product-scope.md).
