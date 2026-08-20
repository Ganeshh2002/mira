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
| **On-view** | Git file list, graph, processes, containers, system | While that view is on screen |
| **On-demand** | Peek content, fuzzy search, SSH reachability | Explicit user action |

Nothing is computed while no window is visible, except tray-menu status refreshed on
menu open.

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

### Git view
Header: branch, upstream, ahead/behind (labelled "as of last fetch"), last commit.
Tabs: **Changes** (grouped path list) and **History** (commit list + commit detail in
0.1; the lane graph and diff view arrive in 0.2). Read-only throughout the 0.x line; the
absence of write actions is deliberate and visible — there are no disabled commit
buttons hinting at a future.

### Ports view (machine-scoped)
All listening ports, grouped: *this project*, *other projects*, *unattributed*. Same row
actions as the project section. This is the one place Mira shows machine-wide data
prominently, because "what has :3000" is asked without a project in mind.

### Shelf
Two scopes in one panel: **Project** and **Global**, switchable. Items are references;
Missing items surface at the top so the list self-heals.

### Peek
A transient overlay above whatever is showing. `Space` opens, `Esc` closes, `⏎` opens in
the editor, `⌘C`/`Ctrl+C` copies the path. It never becomes a window and never stacks.

### System & Media
System is a strip, not a page: compact window footer plus a Settings detail panel.
Media is a single line, only when enabled and available.

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
