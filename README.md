# Aviora Mira

**A lightweight desktop companion that understands what you're working on.**

Mira sits in your menu bar or system tray, keeps track of the projects you have open, and
answers the small questions that break flow — *what branch am I on, what's running on
:3000, is the stack up, where did I put that file* — then launches the tool that fixes it.

It is not an IDE, a terminal, a file manager, or a Git client. It is the thin layer that
points the tools you already use at the right context.

> ### Status: early, and honest about it
>
> **One feature is built.** Mira can hold a set of projects and read each one's Git
> state: branch, last commit, clean or changed, ahead and behind — and it knows the
> difference between a repository, a monorepo, and one package inside one. It keeps that
> up to date on its own, and shows which of your projects each listening port belongs to. Everything else below —
> ports, processes, launching your editor, the Shelf — is a plan, and is labelled as one
> on purpose. Nothing is installable yet. See
> [what's actually built](#project-status).

---

## Why it exists

Your working context is scattered across tools that each own one slice of it: the
repository state in your terminal, the runtime state in `lsof` and Docker Desktop, and the
session state — which windows, which tunnels, which port — only in your head. Every tool
that could hold it wants to own your whole screen instead.

Mira holds the context and delegates the work. It is free, open source, local-first,
account-free, and makes no network connection unless you ask it to.

## Principles

Free · Open source · Local-first · No account · No mandatory cloud · Privacy-first ·
Lightweight · Fast startup · Keyboard-friendly · Cross-platform · Developer-focused but
not an IDE.

Concretely, that means budgets rather than adjectives: **≤ 250 ms** from shortcut to an
interactive window, **≤ 150 MB** idle, **≤ 30 MB** installer, and **zero** outbound
connections on a fresh install.

## Supported platforms

macOS · Windows · Linux — as peers, not as a port and two afterthoughts.

Where an operating system genuinely cannot do something, Mira says so in the interface
instead of pretending. The full
[capability matrix](docs/architecture/platform-abstraction.md#5-capability-matrix) is
maintained honestly; the gaps that matter most:

| | macOS | Windows | Linux |
|---|---|---|---|
| Global shortcut | ✅ | ✅ | ✅ X11 · ⚠️ Wayland has no protocol — `mira --toggle` fallback |
| Tray icon | ✅ | ✅ | ⚠️ menu-only (click events don't fire on Linux) |
| Now-playing media | ❌ Apple restricts this to entitled apps | ✅ | ✅ |
| Lock detection | ✅ | ✅ | ⚠️ systemd-logind desktops only |
| Keep awake | ✅ | ❌ not built yet — use your platform's power settings | ❌ not built yet — use your desktop's power settings |
| Copy to clipboard | ✅ | ✅ | ⚠️ a copied value lasts while Mira is running |
| Auto-update | ✅ | ✅ | AppImage only |

## Screenshots

*Sketch of the 0.1 target, not a screenshot. Today's window is the left rail and the Git
section only; everything else below is planned. Nothing here is shown as if it existed.*

```
┌──────────────────────────────────────────────────────────────┐
│  ⌘K search            Mira                        ⚙  ●       │
├───────────────┬──────────────────────────────────────────────┤
│  PROJECTS     │  aviora-web            [Default ▾]           │
│  ● aviora-web │  main ↑2 · 3 changed · 2 ports               │
│  ○ mira       │  Git      main · "fix: …" · 2 h ago          │
│  ○ astra-api  │  Ports    5173 vite Ⓟ · 5432 postgres        │
│               │  Docker   web ✓  db ✓                        │
│  + Add        │  Apps     Editor · Terminal · Browser        │
└───────────────┴──────────────────────────────────────────────┘
```

## Architecture

```text
React + TypeScript          UI, keyboard, theming — no system privileges
        │                   typed commands ↓   events ↑
      Tauri 2               window · tray · shortcut · updater
        │
       Rust                 every system call in the product
 ┌──────┼───────────────┐
 │      │               │
Core  Services      Platform
 │    git · ports      macOS
 │    processes        Windows
 │    ssh · docker     Linux
 │    media · system
 │
SQLite                one local file, yours, deletable
```

The frontend has **no** filesystem, shell, or network capability — its entire privilege
surface is a list of named commands with validated arguments. Details in
[docs/architecture/architecture.md](docs/architecture/architecture.md).

## Documentation

| | |
|---|---|
| **Product** | [Definition](docs/product/product-definition.md) · [Scope & phases](docs/product/product-scope.md) · [PRD](docs/product/prd.md) |
| **UX** | [Information architecture](docs/ux/information-architecture.md) · [Design system](docs/ux/design-system.md) |
| **Architecture** | [Overview](docs/architecture/architecture.md) · [Platform abstraction](docs/architecture/platform-abstraction.md) · [Data model](docs/architecture/data-model.md) · [Security & privacy](docs/architecture/security-and-privacy.md) · [Review](docs/architecture/architecture-review.md) |
| **Decisions** | [ADRs 0001–0017](docs/adr/) |
| **Building it** | [Roadmap](docs/implementation/roadmap.md) · [Git strategy](docs/development/git-strategy.md) |

## Project status

Phases are locked. **0.1 is the MVP**; everything from 0.2 on is post-MVP and does not
block the first release.

| Phase | Scope | Slices | State |
|---|---|---|---|
| Documentation | Product, scope, architecture, roadmap | — | ✅ Complete |
| Foundation | Crates, capability model, database, IPC boundary, app shell, CI | 0 | ✅ Complete |
| Projects & Git | Add/list/switch/remove projects, read-only Git context, open folder | 1 | ✅ Complete |
| Monorepo awareness | Repository vs monorepo vs package, six workspace tools, read-only | 1.1 | ✅ Complete |
| Live context | Scheduler, live Git, listening ports, process attribution | 2 (part) | 🟡 In progress |
| Workspace context | Workspaces per project, application context, discovery | 4 (part) | 🟡 In progress |
| Application choice | Pick the editor, terminal and browser a workspace uses | 4b (part) | 🟡 In progress |
| Workspace services | The project services a workspace watches, and what each is doing | 4c (part) | 🟡 In progress |
| Workspace actions | A compiled catalogue of things a workspace does — never a command | 4d (part) | 🟡 In progress |
| Process detail & Ports | CPU, memory and uptime per listener; the machine-wide Ports view | 2b (part) | 🟡 In progress |
| Git history | Paged commit walk, commit detail, copy SHA; plus Keep Awake | 5a | ✅ Complete |
| Git graph | Lanes, edges and ref labels over the visible page | 5b (part) | 🟡 In progress |
| Git diff | Changed files and bounded patches, commit and working tree | 5c (part) | 🟡 In progress |
| File history | The commits that touched one file, across renames | 5d (part) | 🟡 In progress |
| History filters | Narrow by branch, author, file or subject text, within a budget | 5e (part) | 🟡 In progress |
| **0.1 — Core Companion** | Projects, Git status and history, ports, processes, editor/terminal/browser, tray, global shortcut | 1–3, 5a | 🟡 In progress |
| 0.2 — Workspace | Workspace model, app groups, restoration, Git graph and diff | 4, 5b, 11 | 🟡 In progress |
| 0.3 — Shelf | Drag-drop, temporary storage, Quick Peek | 6–7 | ⬜ Not started |
| 0.4 — System | CPU/RAM/battery, network, lock/session, media, displays | 9 | ⬜ Not started |
| 0.5 — Personality | Atmospheres, ambient effects, workspace identities | 10 | ⬜ Not started |
| 0.6+ — Automation | Contextual rules, SSH, Docker, plugins, Astra | 8, 12 | ⬜ Gated |

Slice numbers are stable identifiers and do not renumber when a phase changes. See
[product scope](docs/product/product-scope.md) for the lock, and the
[roadmap](docs/implementation/roadmap.md) for how the work is cut.

## Development setup

Requires [Rust](https://rustup.rs) (stable), Node.js 20+, and your platform's
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

```bash
git clone https://github.com/aviora/mira.git
cd mira
npm ci
npm run dev     # Vite + the Tauri shell
npm run check   # format, lint, types, both test suites
```

Full setup, checks, and standards: [CONTRIBUTING.md](CONTRIBUTING.md).

## Contributing

Early contributions are welcome, and right now the most useful ones are **challenges to
the design**: if something in the docs is wrong, contradictory, or over-scoped, open an
issue. If a platform claim in the capability matrix is wrong on your machine, that is a
bug worth reporting.

Read [CONTRIBUTING.md](CONTRIBUTING.md), then the
[Code of Conduct](CODE_OF_CONDUCT.md). Security issues go to
[SECURITY.md](SECURITY.md), never the issue tracker.

## Relationship to Aviora Astra

**Aviora Mira** is a local desktop companion. **Aviora Astra** is a separate, larger
remote-development workspace product.

**Mira works completely on its own and always will.** No Mira feature depends on Astra,
no Astra code exists in this repository, and if Astra were cancelled tomorrow nothing here
would change. Any future integration will be an optional module, off by default, that can
be deleted without affecting anything else — the same boundary a third-party plugin would
use.

## Licence

[MIT](LICENSE) © The Aviora Mira contributors
