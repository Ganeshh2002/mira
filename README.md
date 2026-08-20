# Aviora Mira

**A lightweight desktop companion that understands what you're working on.**

Mira sits in your menu bar or system tray, keeps track of the projects you have open, and
answers the small questions that break flow — *what branch am I on, what's running on
:3000, is the stack up, where did I put that file* — then launches the tool that fixes it.

It is not an IDE, a terminal, a file manager, or a Git client. It is the thin layer that
points the tools you already use at the right context.

> ### Status: pre-implementation
>
> **No code has been written yet.** This repository currently contains the product
> definition, architecture, and implementation roadmap. Nothing described below is
> installable today. Everything here is a plan, and it is labelled as one on purpose —
> see [what's actually built](#project-status).

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
| Auto-update | ✅ | ✅ | AppImage only |

## Screenshots

*Placeholder — screenshots will be added when the first slice is implemented. Nothing is
shown here that does not exist.*

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
| **Product** | [Definition](docs/product/product-definition.md) · [PRD](docs/product/prd.md) |
| **UX** | [Information architecture](docs/ux/information-architecture.md) · [Design system](docs/ux/design-system.md) |
| **Architecture** | [Overview](docs/architecture/architecture.md) · [Platform abstraction](docs/architecture/platform-abstraction.md) · [Data model](docs/architecture/data-model.md) · [Security & privacy](docs/architecture/security-and-privacy.md) · [Review](docs/architecture/architecture-review.md) |
| **Decisions** | [ADRs 0001–0008](docs/adr/) |
| **Building it** | [Roadmap](docs/implementation/roadmap.md) · [Git strategy](docs/development/git-strategy.md) |

## Project status

| Slice | Scope | State |
|---|---|---|
| Documentation | Product, architecture, roadmap | ✅ Complete |
| 0 — Skeleton | Tauri app, workspace, migrations, CI | ⬜ Not started |
| 1 — Project + Git | Add project, detect repo, show status | ⬜ Not started |
| 2 — Ports + processes | Detect, attribute, open, terminate | ⬜ Not started |
| 3 — Launching | Editor, terminal, browser | ⬜ Not started |
| 4–12 | Workspaces, graph, shelf, peek, awareness, themes | ⬜ Not started |

First release (`0.1.0`) will cover slices 0–3. See the
[roadmap](docs/implementation/roadmap.md).

## Development setup

Requires [Rust](https://rustup.rs) (stable), Node.js 20+, and your platform's
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

```bash
git clone https://github.com/aviora/mira.git
cd mira
npm install
npm run tauri dev
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
