# Aviora Mira — Product Definition

> **Aviora Mira — a lightweight desktop companion that understands what you're working on.**

Status: **pre-implementation.** Nothing in this document is built yet.

---

## 1. The one-sentence version

Mira sits in your menu bar / system tray, knows which project you are working on, and
answers the small questions that break flow: *what branch am I on, what is running on
:3000, is the container up, where did I put that file, is my SSH tunnel alive* — and
launches the tool that fixes it.

## 2. Why it exists

A working developer keeps a mental model that no single tool holds:

- the **repository** state lives in the terminal or the editor's Git panel,
- the **runtime** state lives in `lsof`, Docker Desktop, and browser tabs,
- the **session** state (which windows, which tunnels, which env) lives only in the
  developer's head and dies when the machine sleeps.

Every existing tool owns one slice and wants to own the whole screen. IDEs absorb the
terminal; launchers absorb everything but keep no project context; dashboards need a
daemon and an account. The gap is a small, always-available layer that holds the
*context* and delegates the *work*.

Mira is that layer. It is deliberately not where you do the work.

## 3. Core principles

These are binding constraints on every future feature, not aspirations.

| Principle | What it forbids |
|---|---|
| **Free** | No paid tier, no feature gated behind payment. |
| **Open source** | MIT. No open-core split, no proprietary "pro" plugin. |
| **Local-first** | All state lives in a local SQLite file. The app is fully functional offline, forever. |
| **No account required** | There is no sign-up, no login screen, no user table. |
| **No mandatory cloud service** | No feature may hard-depend on a server Aviora runs. |
| **Privacy-first** | No telemetry by default; see [security-and-privacy.md](../architecture/security-and-privacy.md). |
| **Lightweight** | Idle RSS budget ≤ 150 MB; idle CPU ≈ 0% when no window is open. |
| **Fast startup** | Global shortcut → visible, interactive window in ≤ 250 ms warm. |
| **Keyboard-friendly** | Every MVP action reachable without a mouse. |
| **Cross-platform** | macOS, Windows, Linux are peers. None is a port of another. |
| **Developer-focused but not an IDE** | Mira never edits code. |

### Performance budgets (measurable, enforced from Slice 1)

| Budget | Target | Ceiling |
|---|---|---|
| Cold start → window visible | 800 ms | 1.5 s |
| Warm toggle (shortcut → interactive) | 150 ms | 250 ms |
| Idle memory (tray only, no window) | 80 MB | 150 MB |
| Idle CPU (no window, polling active) | < 0.5% | 1% |
| Installer size | 15 MB | 30 MB |

These numbers appear again as acceptance criteria in the [PRD](prd.md) and as a
release gate. A slice that breaks a ceiling is not done.

## 4. Target platforms

macOS, Windows, Linux — supported as first-class peers.

Where an OS genuinely cannot do something (Wayland global shortcuts, macOS
now-playing metadata), Mira **degrades visibly rather than pretending**. The feature
reports itself unavailable with a reason. See
[platform-abstraction.md](../architecture/platform-abstraction.md).

## 5. What Mira **is**

1. **A context holder.** It knows your projects, their directories, their repos, their
   ports, their containers, their tunnels — and which of them you touched last.
2. **A status surface.** Read-mostly, glanceable answers about the state of your work.
3. **A launcher with context.** "Open this project in my editor / terminal / browser"
   where *this project* is already known, so you do not retype paths.
4. **A small set of safe actions.** Kill the process on a port, copy a branch name,
   stash a file on the Shelf, open `localhost:5173`.
5. **A session restorer** (V1.x). Reopen what you had open for a project.
6. **A companion.** It expects you to have an editor, a terminal, a browser, and Git,
   and it makes them easier to point at the right thing.

## 6. What Mira is **not**

Non-negotiable. A proposal that moves Mira toward any of these is rejected by default;
overturning one requires an ADR.

| Not | Why | What Mira does instead |
|---|---|---|
| **An IDE** | Editing is solved; competing costs everything else. | Opens your editor at the right path. |
| **A full terminal replacement** | A good terminal is a deep product. | Opens your terminal in the project directory, optionally running one command. |
| **A file manager** | Browsing trees is not the problem. | The Shelf: a few files you deliberately parked. |
| **A music player** | Playback is not developer context. | Reads *what is playing* where the OS permits, to label a session. |
| **A browser** | No. | Opens URLs in your browser. |
| **A full Git client** | Merge/rebase UIs are their own product. | Status, branch, last commit, a readable graph — read-mostly. |
| **A Docker Desktop replacement** | Image/volume management is a product. | Awareness: which containers relate to this project, are they up. |
| **A Raycast clone** | General-purpose launching is taken and done well. | Project-scoped commands only. No app-wide fuzzy launcher. |
| **A Keyboard Maestro clone** | General automation is unbounded. | Narrow, project-triggered automation, and only in Future scope. |

### The boundary test

Before accepting a feature, ask: *does this help the developer point an existing tool at
the right context, or does it try to replace that tool?* The first is Mira. The second
is out of scope, however small the first version looks.

### The Git boundary, stated precisely

Mira reads Git and performs exactly these writes, and no others in MVP: **none**.
Fetch, pull, commit, push, merge, rebase, and conflict resolution stay in your existing
tools. V1.x may add `fetch` (safe, read-oriented) behind an explicit setting. Anything
that rewrites history is permanently out of scope.

## 7. Who it is for

- **Primary:** developers running several projects at once — a couple of repos, a few
  dev servers, some containers, an SSH box — who lose minutes per hour to
  "which one was that again?"
- **Secondary:** developers on shared or remote machines who want a local, private,
  accountless tool.
- **Explicitly not targeted:** non-technical users; teams wanting shared dashboards;
  anyone needing a hosted service. Those are Astra's questions, not Mira's.

## 8. Relationship to Aviora Astra

| | **Aviora Mira** | **Aviora Astra** |
|---|---|---|
| Shape | Local desktop companion | Larger remote-development workspace/product |
| Runs | On your machine | Remote / hosted |
| State | Local SQLite, yours | Whatever Astra defines |
| Account | Never required | Astra's concern |
| Ships | Now | Later, separately |

**The rule: Mira must remain completely useful if Astra never exists.**

Concretely, this is enforced by architecture, not goodwill:

1. Mira's core, services, and data model contain **no Astra types, no Astra tables, no
   Astra API calls, and no Astra crate dependency**. Nothing named `astra` appears
   outside an optional integration module.
2. Any future Astra integration arrives as an **optional module behind a capability
   flag**, disabled by default, that consumes only public Mira interfaces —
   the same surface a third-party plugin would use.
3. Astra integration may never become the only way to reach an MVP or V1.x feature.
4. If Astra is cancelled, deleting the integration module must leave a working Mira.
   That deletability is the test.

This means Mira's plugin/extension boundary (Future scope) and the Astra boundary are
**the same boundary**. Designing one designs the other; Astra is simply the first
consumer. See [architecture.md](../architecture/architecture.md) §Extension boundary.

## 9. Success criteria for the product (not the code)

Mira is working if:

1. A developer with three active projects opens it more often than they open Docker
   Desktop or `lsof`.
2. Answering "what is on port 3000 and can I kill it" takes one shortcut and one click.
3. It stays under the performance budgets on a five-year-old laptop.
4. A new contributor can build it from `git clone` in under 15 minutes.
5. Uninstalling leaves nothing but one SQLite file the user can delete.

## 10. Non-goals for the first year

Stated so they stop being re-litigated: mobile clients, team/multi-user features,
a hosted sync service, an app store, a marketplace, AI code generation, and any
form of usage analytics.
