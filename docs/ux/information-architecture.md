# Aviora Mira — Information Architecture & UX

Status: **in progress.** This document defines structure and navigation, not
visual style. Visuals live in [design-system.md](design-system.md).

---

## 1. The organising idea

Mira has exactly one spine:

```
Project ──▶ Workspace ──▶ Session
```

Everything else attaches to one of these three. If a new concept does not attach
cleanly, it is probably out of scope.

### Project — *where the code lives*

A directory on disk plus what Mira learned about it (repo, type markers, colour, name).

**A project is not the same thing as a repository, and not the same thing as a package.**
One repository may hold many packages, and a person may add the monorepo root, one
package, or several packages as separate projects — all three are legitimate. Mira shows
which case it is looking at and never collapses them: a package selected inside a
monorepo displays the repository it belongs to alongside its own path, because its Git
state belongs to the repository and its identity belongs to the package. Detected
packages are **boundaries, not projects**; adopting one is an explicit act
([ADR-0010](../adr/0010-monorepo-detection.md)).
Long-lived. Created by the user, one per repository or per logical codebase. **Nouns
that belong to a project:** repo, Git state, shelf items, SSH hosts, containers,
default apps.

### Workspace — *a way of working on that project*

A named configuration inside a project: which subdirectory, which ports matter, which
commands, which apps. Long-lived, user-authored, cheap to add. Every project has an
implicit `Default` workspace that stays invisible until a second one exists.

**Workspaces are configuration, not runtime.** Switching one changes what Mira *shows
and offers*; it never starts or stops anything. This is the rule that keeps the concept
safe to explore.

**As built (slices 3–4c).** A workspace is a name, an optional description, a project,
the kinds of application it works with, and **which of the project's services are the
work**. It has **no directory of its own** and never needs one. Everything it displays —
Git, packages, services — belongs to the project underneath it and is observed once for
that project, so two workspaces can never disagree and switching between them costs no
observation. What differs between them is the *view*: which services are shown. Opening
one records *when*; it launches nothing ([ADR-0012](../adr/0012-workspace-semantics.md),
[ADR-0020](../adr/0020-workspace-services.md)).

### Session — *one stretch of actually working*

A time-bounded record created by Mira, not by the user: opened at `T`, project P,
workspace W, paused while the screen was locked, closed at `T+n`. Sessions are what
make "welcome back" and (0.2) workspace restoration possible.

**Sessions are observed, not configured.** The user never creates one. They may delete
them, and may turn session recording off entirely.

### Why three and not two

Two would collapse either *configuration* into *place* (one project can only be worked
on one way — false for anything with a frontend and a backend) or *history* into
*configuration* (restoration would mutate the user's setup — surprising and unsafe).
Three nouns, each with one job, is the smallest honest model.

---

## 2. Where each domain lives

| Domain | Owned by | Surfaces in | Persisted |
|---|---|---|---|
| **Projects** | — (top level) | Project list, tray menu, switcher | Yes |
| **Workspaces** | Project | Project header switcher, Workspace view | Yes |
| **Sessions** | Project + Workspace | Session strip, project history | Yes (deletable, disableable) |
| **Git** | Project | Project detail, Git view | No (read live) |
| **Ports** | Project (attributed) + machine | Project detail, global Ports view | Expected ports only |
| **Processes** | Derived from ports/paths | Inline under Ports, process detail | No |
| **Shelf** | Project *or* global | Shelf panel, Peek | Yes (references only) |
| **Peek** | Transient overlay | Anywhere a file is named | No |
| **System** | Machine | Compact window strip, Settings | No |
| **Media** | Machine | Compact window line, session label | Only if attached to a session |
| **Automations** *(0.6+)* | Project or Workspace | Not before 0.6+ | Would be |
| **Settings** | App | Settings window | Yes |

Two rules fall out of this table and are binding:

1. **Machine-scoped data is never stored per project.** Ports, processes, system, and
   media are read live and thrown away. Only *expected* ports (a user statement of
   intent) persist.
2. **Nothing observed is persisted without the user asking for it.** Sessions are the
   one exception, and they are deletable and can be switched off.

---

## 3. Multiple projects, simultaneously

This is a structural requirement, not a preference.

- The project list is **always** the root of the main window. There is no "current
  project" mode that hides the others.
- Every project keeps its own independent state: selected workspace, scroll position,
  expanded sections, live status.
- Status for **all** projects stays fresh enough to be trustworthy in the list (branch,
  dirty, port count) — but expensive detail (graph, process detail, containers) is
  computed only for the project on screen.
- Switching projects is instant and lossless: no reload, no re-fetch of what was
  already known, no losing your place.
- The tray menu lists projects directly, so a second project is one click away without
  opening the window.

**Anti-requirement:** Mira must never present a single global "active workspace". If a
future feature needs one, it is the wrong feature.

### Freshness tiers

| Tier | What | When it updates |
|---|---|---|
| **Always-fresh** | Project name, branch, dirty dot, port count | Watcher + 5 s poll while any window is open |
| **On-view** | Git history, file list, graph, processes, containers, system | While that view is on screen |
| **On-demand** | Peek content, fuzzy search, SSH reachability | Explicit user action |

Nothing is computed while no window is visible, except tray-menu status refreshed on
menu open.

**As built (slice 2).** The always-fresh tier is a five-second poll owned by the one
scheduler ([ADR-0011](../adr/0011-one-scheduler.md)), gated on *a window being visible*
and *at least one project existing*. With either false, the clock ticks and nothing is
read. The watcher half of the tier is deferred: bounded polling gives a five-second
worst case for a fifth of the code, and a watcher becomes an optimisation on top of a
scheduler that already exists.

Everything observed carries **when it was read**, and the interface shows it — "Updated
3 s ago". Stale data is never presented as current. Where a refresh fails, a project's
last Git reading stays visible with the failure beside it, and the service list is
emptied rather than kept, because "what is running right now" has no useful stale
answer.

---

## 4. Surfaces

Mira has five surfaces. Adding a sixth requires justification.

### 4.1 Main window

The full experience. Resizable, remembers size and position per display.

```
┌──────────────────────────────────────────────────────────────┐
│  ⌘K search            Mira                        ⚙  ●       │  title bar
├───────────────┬──────────────────────────────────────────────┤
│  PROJECTS     │  aviora-web            [Default ▾]           │
│  ● aviora-web │  main ↑2 · 3 changed · 2 ports               │
│  ○ mira       │  ┌────────────────────────────────────────┐  │
│  ○ astra-api  │  │ Git      main · "fix: …" · 2 h ago      │  │
│  ⚠ old-thing  │  │ Ports    5173 vite Ⓟ · 5432 postgres    │  │
│               │  │ Docker   web ✓  db ✓                    │  │
│  + Add        │  │ Shelf    3 items                        │  │
├───────────────┤  │ Apps     Editor · Terminal · Browser    │  │
│  Ports        │  └────────────────────────────────────────┘  │
│  Shelf        │                                              │
│  Settings     │                                              │
└───────────────┴──────────────────────────────────────────────┘
```

- **Left rail:** project list (primary), then machine-scoped views (Ports, Shelf) and
  Settings. The rail is the only navigation; there is no nested menu tree.
- **Right pane:** the selected project's detail, or a machine-scoped view.
- **Sections** in the detail pane are collapsible and remember their state per project.

### 4.2 Compact command/context window

The keyboard surface, summoned by the global shortcut. Centred on the cursor's display,
no title bar, dismissed on `Esc` or focus loss.

```
┌────────────────────────────────────────────┐
│  ▸ aviora-web · main ↑2 · 3 changed        │
│  ────────────────────────────────────────  │
│  › open ter▌                               │
│  ────────────────────────────────────────  │
│  ⏎  Open Terminal          aviora-web      │
│     Open Terminal          mira            │
│     Open Editor            aviora-web      │
│     Kill :5173  vite       aviora-web      │
│  ────────────────────────────────────────  │
│  CPU 12%  MEM 9.1/16G   ♪ Weather — Tycho  │
└────────────────────────────────────────────┘
```

- Header: the current project's one-line status.
- One input; results are **project-scoped actions only** — never a general app launcher,
  never arbitrary filesystem search. That constraint is what stops Mira becoming Raycast.
- Ranked by: current project first, then recency, then alphabetical.
- Footer: the system strip (feature 17) and, if enabled and available, now-playing.
- Destructive results (kill) are visually distinct and still require confirmation.

### 4.3 Menu bar / system tray

The resting state.

```
Mira
─────────────────────
● aviora-web   main ↑2
○ mira         main
○ astra-api    dev ⚠
─────────────────────
Show Mira      ⌥Space
Settings
─────────────────────
Quit Mira
```

Menu-first by design, because Linux tray click events do not fire (see PRD 21).
Selecting a project opens the main window on that project.

### 4.4 Project detail view

The default right-pane content. Header (name, workspace switcher, branch, actions) plus
sections in fixed order: **Git · Ports · Docker · SSH · Shelf · Apps & Commands**.
Sections with nothing to say (no Docker, no SSH hosts) are absent rather than empty.

Sub-views reached from here — Workspace, Git, Shelf — replace the right pane and are
returned from with `Esc` or the breadcrumb. No modal stacking beyond Peek.

### 4.5 Settings

A separate window, not a rail page, because it is app-scoped and rarely needed:
**General · Appearance · Applications · Shortcuts · Privacy · Advanced · About**.

Per-project and per-workspace configuration lives with the project, never in Settings.
Settings holds only global defaults.

---

## 5. Domain views

### Workspace view
Name, root subdirectory, expected ports, commands (label + program + args + cwd),
preferred apps, and a session list. Editing here changes configuration only.

**As built (Slices 3–4c).** Name and description; the project underneath, with its
packages; the **services this workspace watches**; Git; **Open with**; and the
application context as a set of kinds. Commands and sessions are not built.

The order is **Project → Packages → Services → Git → Context**: what this is, what it is
made of, what is running, where the code stands, and what opens it.

**Services here are the workspace's, not the project's.** A monorepo running five
servers puts five rows on the project surface and, on a workspace whose work is two of
them, two. A service is added from a menu of what Mira observed for this project —
never a field to type a port into — and removed by a button that names the port it
removes. Two workspaces on one project keep separate lists and neither can see or change
the other's ([ADR-0020](../adr/0020-workspace-services.md)).

Every row says what Mira actually knows, which is why there are five states and not two:

| State | What it means |
|---|---|
| **Running** | Listening now, from inside this project. The only state with an Open button. |
| **Not running** | Mira looked, and nothing is here. Expected, and down. |
| **Port taken** | Something *else* is on the number. Never reported as running, and never opened in place of yours. |
| **Never observed** | Mira has not read the socket table yet. |
| **Cannot tell** | Mira tried and could not, with what the platform said. |

The last two exist because §5's rule cuts both ways: an interface that renders "Mira
could not look" as "your server is down" is telling somebody something Mira has not
established. And nothing on this surface starts or stops anything — Open is the only
action, and there is no disabled Stop button hinting at one.

Open with sits under Git, because that is the order the questions come in: where does
this stand, then get me into it. It offers **Editor** and **Terminal** — each
naming the application that will actually open, so the button never claims something
the machine was not asked to confirm. A browser is not there: a browser opens a
*service*, so its action lives on the service row and disappears with the service.

A kind with nothing behind it is a sentence, not a disabled button — "Editor · nothing
here Mira can open". §5's rule is that a greyed control promises a later release, and a
machine with no editor is not waiting for anything. The same rule still holds for what
has not been built: nothing restores a workspace, nothing starts a development server,
and there is no control hinting that either nearly happens.

When the project's folder is missing, the actions are **absent** rather than failing.
There is nowhere to open, and finding that out by pressing a button and reading an error
is worse than not being offered one.

### Git view
Header: branch, upstream, ahead/behind (labelled "as of last fetch"), last commit.
Tabs: **Changes** (grouped path list) and **History** (commit list + commit detail in
0.1; the lane graph and diff view arrive in 0.2). Read-only throughout the 0.x line; the
absence of write actions is deliberate and visible — there are no disabled commit
buttons hinting at a future.

**As built (slice 5a).** History is reached from the project's Git panel and is the
third level of the navigation model — rail → project → History. It is a linear list:
one row per commit, carrying the subject, the author, how long ago, and the
abbreviated id, each marked with an icon so the eye can find one among three short
strings. A row reads correctly with the icons stripped out; none of them is load-
bearing. Clicking a row opens that commit's detail *within* the surface rather than
descending a fourth level, and `Esc` closes it, then leaves.

Pagination is **Load more**, twenty-five commits at a time, and the button is absent
once there is nothing more. There is no page-size control, because there is no
page-size argument (`architecture.md` §5 rule 7).

Every state has a shape: an empty repository says "No commits yet"; a directory
without Git says so; an unreadable one shows its reason; a shallow clone says the
oldest row is where the copy stops rather than where the history does; a detached
HEAD names the commit it is on. A merge is a row like any other, and says on its
detail that it has two parents and that its changed-file count depends on which
parent you compare against.

Copy is the one icon-only action, with a name and a tooltip: the short id from a
row, the full id from the detail. The **Changes** tab is still to come — Slice 5a
built History only.

**As built (slice 5b) — the graph.** The lanes sit in a gutter to the *left of the
same rows*, so the picture and the list are one thing rather than two views to keep
in step. A **Graph** toggle in the header turns the gutter off; that is not
cosmetic, because List asks a cheaper question of the backend (commits alone, no
parents, lanes or reference labels).

Four rules keep it a picture rather than a puzzle:

1. **Every fact it draws is also written.** A merge says "Merge of 2 parents" in
   words; a root says "First commit"; branches and tags are labelled chips with
   their kind in the tooltip. The gutter is `aria-hidden` — announcing it would
   read the row out twice — and hiding it loses width, never content.
2. **Shape before colour.** A ring is an ordinary commit, a ring with a filled
   centre is a merge, a filled disc is the first commit. Lane colour comes from a
   muted eight-step ramp and carries no meaning beyond "a different line".
3. **The bounds are said out loud.** More than eight lanes folds onto the eighth
   and the surface says so; more references than Mira reads at once, and it says a
   label may be missing; a shallow clone still says where the copy stops.
4. **Nothing is actionable.** No checkout, merge, rebase, reset, cherry-pick or
   revert — and no disabled control implying one later
   ([ADR-0015](../adr/0015-graph-lanes.md)).

**Keyboard.** `↑` and `↓` move between commits, `⏎` opens one, `Esc` closes it and
then leaves the surface. Tab still reaches every row; the arrows are the faster
path rather than the only one (§7).

**Narrow windows.** The gutter is hidden below the small breakpoint rather than
reflowed, and the ref chips wrap. That is the degradation rule in one line: drop
the decoration, keep the content.

**As built (slice 5c) — changed files and diffs.** The **Changes** tab arrives, in
two places rather than one, because there are two questions:

- **A commit's changes** sit inside the commit's own detail panel in History, and a
  file's patch opens inside its own row. History → commit → files → patch is four
  things to read and *one place to be*: the depth cap in §6 is kept by disclosure
  rather than by navigation.
- **The working tree** is its own sub-view, reached from the project's Git panel by
  **Changed files** — offered only when there is something to show. It is separate
  on purpose: one is what is recorded and the other is what is on disk, and a
  single list of both would make it impossible to tell them apart.

A row reads `M  src/app.ts  Modified  +24 −8`. The letter *and* the word, because
`C` and `M` are indistinguishable to somebody who has not memorised them, and
colour is the third channel rather than the first. A rename says where it came
from; a binary file says it is binary.

The patch is a **table** — line before, line after, the line — with `+` and `−` in
the text as well as colour, so it reads correctly in a screen reader and on a
monochrome display. Long lines scroll inside the table's own box; the page never
scrolls sideways.

**Every stopping point is a sentence.** "Showing 200 of 4,312 changed files";
"Showing the first 2,000 lines"; "8.0 MB is larger than the 2.0 MB Mira will
compare, so this file was not read". Nothing is shortened quietly — a truncated
diff that looked complete would be the worst thing this surface could do
([ADR-0016](../adr/0016-bounded-diffs.md)).

**Still read-only.** No staging, discarding, checkout, revert or apply — and no
greyed-out control implying that one is coming.

**As built (slice 5d) — file history.** Every changed-file row carries a second
disclosure beside the patch one: **History**, which lists the commits that touched
that file. It opens *in the row*, so the path stays `Commit → Changed files → File
history` — three things to read and one place to be, like the patch beside it.

Each row shows the subject, the author, how long ago, and the short id — the same
four facts every commit row in Mira shows, so the surfaces read alike. A rename
says *"Renamed from src/old.ts"* in words, with the icon as a companion.

**Clicking a commit opens that commit**, in the detail surface that already
exists. From the working tree that means leaving for History with the commit
already open, rather than growing a second commit view.

**A trace that stopped early says so, and says it differently from finding
nothing.** *"Nothing in the last 2,000 commits. There may be more further back"* is
not *"No commit has touched this file"*, and **Look further back** continues. That
distinction is the whole reason the bound is visible rather than silent
([ADR-0017](../adr/0017-file-history.md)).

`↑` and `↓` move between commits here too.

**As built (slice 5e) — filtering.** A single compact bar sits under History's
header: `[ Branch ▾ ] [ Author ▾ ] [ File ▾ ]` and a **Search** field. Nothing
above it moves, and the default view is unchanged — with no filter on, History is
the same list it was, asking the same cheap question.

Every menu is a list Mira produced. Branches and tags come from `git.refs`,
authors from the commits within one scan budget, files from the working tree's
bounded change list. Each item is picked, never typed, which is what keeps a name
off the wire; and each menu says its own limit when it has one (*"Authors of the
last 2,000 commits. Somebody further back may be missing."*).

The **Search** field is submitted rather than searched-as-typed, because each
search is a bounded walk. What it matches is written in the field's own
description: *part of a commit's subject line, ignoring case; not a pattern.*

Filters **compose** — branch and author and file and text all narrow together —
and **Clear filters** returns the surface to ordinary pagination rather than to an
empty search. The graph toggle is absent while a filter is on, because a filtered
history is a set of matches rather than a shape, and lanes drawn between
non-adjacent commits would be a picture of something that does not exist.

**A search that ran out of budget says "No match yet", never "No results".** The
three endings are three different sentences: *"Nothing in this history matches
author Grace Hopper"* when the walk reached the end; *"Nothing matched in the 2,000
commits examined. There may be more further back"* with **Keep looking** when it
did not; and *"Stopped after examining 2,000 commits, so this is what matched so
far"* above a partial list. What is filtered is also announced in words for a
reader who cannot see which controls are lit
([ADR-0018](../adr/0018-history-filters.md)).

**As built (slice 4b) — choosing an application.** Each Context row carries a
compact `[ Editor application ▾ ]` menu beside what was found. It offers what Mira
looks for on this platform, each row marked *installed*, *not installed* or
*cannot open a folder* — in words, so the state is not carried by shade alone.

**Automatic is the first option and stays the default**, and it says what it does
today: *"Automatic · Ghostty"* rather than a word whose meaning has to be guessed.
Going back to it is choosing it, not clearing a field. There is no field: nothing
here can be typed into, because there is no command that would take a typed
application.

**A choice that stopped being true says so, on the row.** *"Zed is not on this
machine. Nothing else will be opened — choose another."* The **Open with** button
for that kind is replaced by the same sentence rather than offered and left to
fail — and Mira does not open a different editor, because then nobody would ever
find out.

**Open with names what will actually open.** If the workspace chose Zed the button
says *"Editor · Zed"*, whatever Mira would have picked on its own. A button naming
one application and starting another would be the worst small lie on this surface
([ADR-0019](../adr/0019-application-preferences.md)).

`Esc` closes a menu and gives the focus back; `↑` and `↓` move through it; the
chosen row says *"chosen"* in words.

### Ports view (machine-scoped)
All listening ports, grouped: *this project*, *other projects*, *unattributed*. Same row
actions as the project section. This is the one place Mira shows machine-wide data
prominently, because "what has :3000" is asked without a project in mind.

**Not built, and deliberately not.** The machine-wide view is its own surface with its
own questions; slice 4c narrowed downward instead, to what one workspace watches.

**A project's Services section is not that place.** It answers "what is running in *this*
project", so a listener Mira positively determined is outside every project stays out of
it — an editor's language server and another app's helper are noise under your project's
name. One exception, and it is the honest one: where the platform would not say which
process owns a socket, or would not expose its working directory, Mira did not decide the
service is elsewhere — it failed to tell. Those are shown with the reason, because on
Windows that is every service, and an empty list beside a running dev server would be a
worse lie than an uncertain row.

### Shelf
Two scopes in one panel: **Project** and **Global**, switchable. Items are references;
Missing items surface at the top so the list self-heals.

### Peek
A transient overlay above whatever is showing. `Space` opens, `Esc` closes, `⏎` opens in
the editor, `⌘C`/`Ctrl+C` copies the path. It never becomes a window and never stacks.

### System & Media
System is a strip, not a page: compact window footer plus a Settings detail panel.
Media is a single line, only when enabled and available.

### Keep Awake
A utility, sized like one. It lives in the **tray menu** — a machine-wide switch
belongs where machine-wide switches go, and the tray is menu-first on every platform
(§4.5) — as a submenu of four check items: Off, 30 minutes, 1 hour, Until turned off.
Settings carries the same four choices plus what the state means and, where the
platform cannot do it, the reason.

It never becomes a page and never grows a dashboard. On a platform where the
capability is `Unavailable` the submenu is one disabled line carrying the reason,
which is §8's Unavailable rule rather than a special case
([ADR-0014](../adr/0014-keep-awake.md)).

### Automations *(Future)*
Reserved location: a per-project section below Apps & Commands. Nothing is built, and
no schema is created for it before 0.6+.

---

## 6. Navigation model

Flat and shallow. Depth is capped at three levels: **rail → project → sub-view**.

```
Main window
├── Projects (rail)
│   └── Project detail
│       ├── Workspace view
│       ├── Git view (Changes | History)
│       └── Shelf (project scope)
├── Ports (rail, machine-scoped)
├── Shelf (rail, global scope)
└── Settings (separate window)

Compact window  ──▶ actions on any project (parallel entry point)
Tray menu       ──▶ projects, toggle, settings, quit
Peek            ──▶ overlay, any surface
```

Rules:
1. **No breadcrumb longer than two hops.** If a design needs three, the design is wrong.
2. **`Esc` always goes back one level** and, at the top level, hides the window.
3. **Every destination is reachable by keyboard**, and the compact window can reach any
   *action* without navigating at all.
4. **No modal dialogs except confirmations.** Destructive actions confirm; nothing else
   blocks.

---

## 7. Keyboard model

Keyboard-first is a principle, so bindings are specified here rather than left to
implementation. `⌘` on macOS = `Ctrl` elsewhere.

### Global (OS-wide)
| Binding | Action |
|---|---|
| `⌥Space` / `Ctrl+Alt+Space` | Toggle compact window (rebindable; Wayland uses `mira --toggle`) |

### Anywhere in Mira
| Binding | Action |
|---|---|
| `⌘K` | Command/context search |
| `⌘1`…`⌘9` | Jump to project 1–9 |
| `⌘,` | Settings |
| `Esc` | Back one level / dismiss / hide |
| `⌘W` | Close window (Mira keeps running) |
| `⌘R` | Refresh current view |
| `?` | Keyboard reference sheet |

### Project detail
| Binding | Action |
|---|---|
| `↑` `↓` | Move within a section |
| `←` `→` | Collapse / expand section |
| `Tab` / `Shift+Tab` | Next / previous section |
| `E` `T` `B` | Open Editor / Terminal / Browser |
| `G` | Git view |
| `P` | Ports view (project-scoped) |
| `S` | Shelf |
| `W` | Workspace switcher |
| `Space` | Peek the selected file |
| `⏎` | Primary action for the selected row |
| `⌘C` | Copy the selected row's most useful value |

### Lists (Git changes, ports, shelf, graph)
`↑`/`↓` move · `⏎` primary · `Space` peek · `⌘C` copy · `⌫` remove **where removal is
non-destructive to disk** (shelf only; never files, never processes).

Deliberate omission: **no single-key binding terminates a process.** Kill is reached
through the row's action and confirmed. Speed is not worth that class of accident.

### Focus and accessibility
Visible focus rings everywhere, logical tab order matching visual order, ARIA roles on
custom widgets, full screen-reader labels on status dots (which never carry meaning by
colour alone — every dot has a shape or text equivalent).

---

## 8. States every surface must define

Because a status tool is mostly *not* in its happy path:

| State | Rule |
|---|---|
| **Empty** | Explain and offer the one action that resolves it. Never a blank pane. |
| **Loading** | Show last-known data with a subtle pending marker. Never a spinner replacing content. |
| **Stale** | Say so ("as of last fetch", "not refreshed since 14:02"). |
| **Unavailable** | State the reason ("Docker not installed", "not available on macOS"). Never hide silently. |
| **Degraded** | Show what works and name what does not. |
| **Error** | One sentence, plain language, plus the action that fixes it. Details behind a disclosure. |
| **Missing** | For paths that vanished: mark, offer Relocate/Remove, never auto-delete. |

---

## 9. What the UX must never feel like

- **An IDE.** No panels-in-panels, no docking, no tool windows, no layout persistence
  beyond section collapse.
- **A dashboard.** No wall of gauges. System status is a strip.
- **A wizard.** Adding a project is one directory pick.
- **A chat.** No conversational surface anywhere.

The test: a user should be able to open Mira, answer their question, and dismiss it in
under five seconds without reading anything they did not come for.
