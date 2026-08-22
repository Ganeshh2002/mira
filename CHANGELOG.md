# Changelog

All notable changes to Aviora Mira are documented here.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning: [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Product definition, product scope, PRD, information architecture, and design system
- Architecture, platform abstraction, data model, and security/privacy documents
- ADRs 0001–0019 covering the foundational technical decisions
- ADRs 0001–0022 covering the foundational technical decisions
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
- **Git graph (slice 5b, part).** The History surface now draws the shape of a history
  beside it: lanes and edges connecting each commit to its parents, with branch and tag
  labels on the rows they point at. A merge says "Merge of 2 parents" in words as well as
  in the picture, the first commit says so too, and the gutter itself is hidden from
  screen readers because everything it draws is already written on the row. Turn the graph
  off and the list stays, asking a cheaper question of the backend; narrow the window and
  the gutter hides itself. More than eight lanes fold onto the eighth and Mira says it has
  done so, rather than drawing a thicket.

  It is a **picture, not a Git client** — there is no checkout, merge, rebase, reset,
  cherry-pick, staging, branch or remote operation anywhere in it, and two guard tests
  keep it that way.

  The graph reads the same page as the list, through the same walk, so switching between
  them can never show different commits. It also refuses the ordering every published lane
  algorithm assumes: asking libgit2 for a topologically sorted walk makes it read the
  entire history before yielding one commit — 3.4 ms, 34 ms and 426 ms for the same
  twenty-five rows on repositories of 100, 1,000 and 10,000 commits, against a flat
  ~0.9 ms. A page costs a page ([ADR-0015](docs/adr/0015-graph-lanes.md)).
- **Git diff (slice 5c, part).** See what changed. A commit's changed files sit inside
  its detail in History, and what you have not committed yet is its own **Working tree**
  view — separate on purpose, because what is recorded and what is on disk are different
  questions. A row reads `M src/app.ts Modified +24 −8`: the letter *and* the word,
  because `C` and `M` mean nothing to somebody who has not memorised them. Renames say
  where they came from, binary files say they are binary, and opening a file shows its
  patch as a table with line numbers on both sides — `+` and `−` in the text as well as
  in colour, so it reads correctly in a screen reader and on a monochrome display. Long
  lines scroll inside their own box.

  **Nothing is shortened quietly.** A change set that stops at two hundred files says how
  many there were; a patch that stops says which limit it hit and what the limit is; a
  file too large to compare says so rather than appearing empty. A diff that looked
  complete when it was not would be worse than no diff at all.

  It stays fast by refusing rather than by hurrying: a 30 MB file is declined in 0.17 ms
  because it is never read, while a 1 MB one takes 4 ms to compare. And it is **a view,
  never an edit** — no staging, discarding, checkout, revert or apply, and nothing greyed
  out implying otherwise ([ADR-0016](docs/adr/0016-bounded-diffs.md)).
- **File history (slice 5d, part).** Every changed file now offers **History**: the
  commits that touched it, following it back across renames and copies. Each row shows
  the subject, the author, how long ago and the short id — and a rename says "Renamed
  from src/old.ts" in words. Clicking a commit opens that commit, in the detail view that
  already exists.

  This is the one read in Mira whose cost is set by how long your history is, not by how
  much you asked for — to know whether a commit touched a file you have to look at that
  commit. So a request looks at a bounded number of them and **tells you when it stopped**:
  "Nothing in the last 2,000 commits. There may be more further back" is a different
  sentence from "No commit has touched this file", and **Look further back** continues.
  Measured: an unbounded trace grows 24× across a twenty-fold repository; Mira's page
  grows 1.4× and stays around a tenth of a second.

  Following renames turned out to cost nothing — 0.03 ms per rename, and nothing per
  commit — which is why the limit is on commits examined rather than on the feature
  everybody expects to be expensive ([ADR-0017](docs/adr/0017-file-history.md)).
- **History filters (slice 5e, part).** History now has a compact filter bar: pick a
  **branch**, an **author** or a **file**, and **search** for words in a subject line.
  They compose, and clearing them puts the ordinary paged history back exactly as it was.

  Nothing you pick becomes a Git argument. A branch travels as the commit id Mira handed
  out, a file as its place in a change list, and the two text fields are compared inside
  Rust against commits already in memory — so `main`, `refs/heads/main`, `--all` and
  `src/**/*.ts` are not values these fields can hold at all. Search is a plain
  case-insensitive substring of the subject line, and the field says so: `^feat` finds
  commits containing `^feat`, not commits starting with `feat`.

  Measuring first collapsed four bounds into one: testing author, subject and path against
  a commit costs 19.1 µs, and *not* testing them costs 21.9 µs — loading the commit is the
  whole expense. So there is one budget, on commits examined, and a search that spends it
  says **"No match yet — nothing matched in the 2,000 commits examined"** with **Keep
  looking**, which is a different sentence from "No matching commits". A broad filter is
  flat across a hundred-fold repository; a narrow one plateaus at the budget instead of
  climbing ([ADR-0018](docs/adr/0018-history-filters.md)).
- **Application preferences (slice 4b, part).** A workspace can now choose which editor,
  terminal and browser it opens with, from the applications Mira knows how to look for.
  Each is marked *installed*, *not installed* or *cannot open a folder*, and **Automatic**
  — the behaviour every workspace had before — stays the default and says what it does
  today. Choosing is per workspace: two workspaces on one project can use different
  editors.

  You pick from a list; you never type a path. A choice is stored as Mira's own name for
  an application — `vscode`, `iterm` — which resolves to a row in a table compiled into
  the binary and to nothing else. There is no setting that points Mira at a program, and
  no column in the database where one could be written.

  If a chosen application is uninstalled, Mira says which one and opens **nothing**:
  *"Zed is not on this machine. Nothing else will be opened — choose another."* Quietly
  starting a different editor would mean never finding out. The same choice travels
  between machines — `vscode` is `vscode` on all three platforms — and one that a
  platform has never heard of says so rather than silently resetting
  ([ADR-0019](docs/adr/0019-application-preferences.md)).

  Looking for applications also got faster on the way past: the `PATH` probe now asks
  only about spellings the platform actually uses, which is 2.7× less work on macOS and
  3.3× less on Linux.
- **Workspace services (slice 4c, part).** A workspace now says which of its project's
  services are the work. The workspace surface reads **Project → Packages → Services →
  Git → Context**, and the Services section shows what this workspace watches rather than
  everything the project happens to be running — so a monorepo with five servers puts two
  rows on a workspace about two of them. Add one from a menu of what Mira observed, and
  remove it from a button that names the port it removes. Two workspaces on one project
  keep separate lists, and neither can see or change the other's.

  You cannot type a port anywhere, and after this slice **no command accepts a port, an
  address, a URL, a process name or a pid at all** — `live.open_service` used to take the
  port and now takes a position in the list Mira produced. A service is added by naming
  where it sat in that list and opened or forgotten by the row id Mira issued; the port
  lives on Mira's side of the boundary in both directions.

  A row says what Mira actually knows. **Running** and **Not running** are different
  states, and so are **Port taken** — something else is on the number, and Mira will not
  open it in place of yours — **Never observed** and **Cannot tell**, because an
  interface that renders "Mira could not look" as "your server is down" is claiming
  something Mira has not established. Nothing here starts or stops a process.

  Measuring reversed the design: indexing the observed listeners in a map is the obvious
  answer and loses everywhere a real machine lives, because the map is built over every
  socket whether a workspace watches one service or twenty. A linear scan is three to
  five times cheaper below twenty services and five hundred sockets, and fifty workspaces
  resolving against four thousand sockets cost 1.9 ms in total — so there is no cache and
  no new clock ([ADR-0020](docs/adr/0020-workspace-services.md)).
- **Workspace actions (slice 4d, part).** A workspace now has an **Actions** section: the
  few things it is actually asked to do. Pick from a catalogue of six — open in the
  editor, open a terminal here, open the running service, show the project folder, mark
  as opened, read everything again — and each row says **what it will do** before you
  press it.

  It is deliberately not a command list. There is no field to type into, no program, no
  argument list, no shell string and no template: the catalogue is a `const` array
  compiled into Mira, and a workspace stores only the identity of the rows it picked.
  `npm run dev`, `pnpm -w build`, `cargo run` and `/bin/sh` are not values any part of
  this feature can hold — the wire tests assert each one fails at the boundary. Every
  effect was already reachable from a button somewhere, so the set of things Mira can do
  is exactly what it was; only the ways to ask for them grew.

  An action that cannot be done is a sentence rather than a greyed-out button, and that
  includes the ambiguous one: with two of this workspace's services running, "open the
  running service" says so and points at the Services list instead of opening the first.
  An identity Mira no longer has says exactly that and offers removal — it is never
  quietly matched to the nearest action. Nothing here starts a server or stops a process.

  Measuring picked the shape: deciding all six actions costs 0.65 µs, while asking the
  machine what it can open costs 7.0 µs — so the machine is asked once per request rather
  than once per action, which is 8.0 µs instead of 47.9. At that price nothing is cached
  and no clock is added ([ADR-0021](docs/adr/0021-workspace-actions.md)).

  The `commands` table from the original schema — the one with `program TEXT NOT NULL` —
  is now permanently empty, with a guard enforcing it.
- **Process detail and the Ports view (slice 2b, part).** Every listener now shows what is
  behind it: **CPU share, memory and how long it has been running**, inline on the row
  rather than behind a modal. And a machine-wide **Ports** surface answers "what has
  :3000" without a project in mind, grouping every listening socket as this project's,
  another project's, or not in any project. It sits below the projects in the nav,
  labelled _this machine_, and says in its first sentence that it is not what a workspace
  watches.

  **Mira does not read your command lines.** The roadmap listed argv under process
  detail; this slice declines it. A process's arguments routinely carry credentials —
  `--password=`, `PGPASSWORD=`, a token inside a `DATABASE_URL` — and Mira sits open all
  day beside the work, which is the worst place for one to be permanently legible.
  Redaction was considered and rejected: it means a blocklist, and
  `--db=postgres://user:hunter2@host` contains none of the obvious words. There is no
  field for it, nothing reads it, and a guard fails the build if anything starts to.

  A CPU share says **"not measured yet"** rather than "0%" until it has been measured
  twice. `sysinfo` computes a share from the delta between two readings and returns zero
  for both an idle process and an unmeasured one; calling a process idle when it might be
  saturating a core is a claim Mira has not established.

  Measuring settled the rest. A share needs two samples, and the obvious fix — two
  readings 200 ms apart inside one request — is both a hidden timer and a noisier answer:
  the same busy process read 238% over 200 ms and 100.3% over five seconds. So it rides
  the scheduler's existing tick, which turned out to be the steadiest window available as
  well as the free one. Grouping the Ports view costs 0.003 ms against the 3.5 ms read it
  arranges. Nothing is stored, so there is no migration
  ([ADR-0022](docs/adr/0022-process-detail.md)).

  Still no way to stop anything: termination keeps its own slice and its own confirmation
  design, and there is no greyed-out control hinting at one.
- macOS and Windows ask the platform for its standard window material — Liquid Glass on
  macOS 26, Mica on Windows 11 — rather than drawing an imitation. Linux stays opaque.

The first release will be `0.1.0` — **the MVP** — covering roadmap slices 1–3 and 5a: projects, Git status and history, ports and
processes, application launching, tray, and the global shortcut. Ports, processes and
application launching are not built yet. Everything from 0.2 on is post-MVP and does not
block it. See [product scope](docs/product/product-scope.md).
