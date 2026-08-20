# Contributing to Aviora Mira

Thanks for looking. Mira is early. The repository holds the product definition,
architecture, and roadmap, the **technical foundation** — crate layout, capability model,
database and migrations, typed command boundary, application shell — and the **first
feature**: projects and their read-only Git context. That is a good moment to arrive: the
decisions are written down and open to challenge, and most of the product is still ahead.

---

## Before you start

Read, in this order:

1. [Product definition](docs/product/product-definition.md) — what Mira is, and the list
   of things it will never become.
2. [Roadmap](docs/implementation/roadmap.md) — what is being built, in what order.
3. [Architecture](docs/architecture/architecture.md) — how the pieces fit.
4. [ADRs](docs/adr/) — why the big choices were made.

If a change contradicts an ADR, that is not automatically wrong — but it needs a new ADR
that supersedes the old one, not a quiet exception.

## Ways to help right now

- **Challenge the design.** Open an issue if something in the docs is wrong,
  contradictory, or missing. This is the most valuable contribution today.
- **Verify platform claims.** The
  [capability matrix](docs/architecture/platform-abstraction.md#5-capability-matrix) makes
  specific claims per OS. If one is wrong on your machine, tell us — accuracy there is a
  core promise.
- **Take a slice task.** Slices 0, 1 and 1.1 are done; slice 2's scheduler and live
  context are in, and workspaces have landed early. Issues are labelled `slice:N`.

## Development setup

**Prerequisites**

| Tool | Version | Notes |
|---|---|---|
| Rust | stable, via [rustup](https://rustup.rs) | pinned in `rust-toolchain.toml` |
| Node.js | 20+ | 22 is what CI runs |
| Tauri prerequisites | per platform | see below |

Tauri's own [prerequisites guide](https://v2.tauri.app/start/prerequisites/) is the
authority; in short:

- **macOS** — Xcode Command Line Tools (`xcode-select --install`)
- **Windows** — MSVC build tools + WebView2 (present on Windows 11)
- **Linux** — `webkit2gtk-4.1`, `libayatana-appindicator3`, `librsvg2`, `patchelf`,
  `build-essential`, `curl`, `wget`, `file`, `libssl-dev`

**Run it**

```bash
git clone https://github.com/aviora/mira.git
cd mira
npm ci
npm run dev
```

`npm run dev` starts Vite and the Tauri shell together. `npm run web:dev` runs only the
frontend in a browser, which is useful for layout work but cannot call any command — the
IPC bridge exists only inside the Tauri window.

**Checks — run these before pushing**

```bash
npm run check
```

That is formatting, lint, types, and both test suites in one command. Individually:

| Command | What it covers |
|---|---|
| `npm run format` | `prettier --write` and `cargo fmt --all` |
| `npm run format:check` | the same, as a check |
| `npm run lint` / `npm run lint:rust` | eslint / clippy at `-D warnings` |
| `npm run typecheck` | `tsc --noEmit` |
| `npm test` | vitest |
| `npm run test:rust` | the Rust suite, including the guard tests |
| `npm run db:check` | the migration suite alone |
| `npm run generate-types` | regenerates `src/bindings` from Rust |

Two things CI checks that the local commands do not imply:

- **Generated types must be committed.** `src/bindings` is produced by ts-rs from the Rust
  types. Run `npm run generate-types` after changing anything crossing the IPC boundary and
  commit the result; CI fails on a drift rather than letting it reach runtime.
- **Supply chain.** `cargo deny check` and `npm audit --omit=dev --audit-level=high` run on
  every push. Policy lives in `deny.toml`.

CI runs all of the above on macOS, Windows, and Linux. All three must be green.

## How we work

- **Branch** from `main`: `feat/…`, `fix/…`, `docs/…`, `chore/…`
- **Commit** with [Conventional Commits](https://www.conventionalcommits.org/):
  `feat(ports): attribute listeners by process cwd`
- **Test first** for anything with logic. A bug fix starts with a test that reproduces it.
- **Small PRs.** One change per PR. Draft PRs are welcome and encouraged.
- **Squash merge** into `main`.

Full detail in [docs/development/git-strategy.md](docs/development/git-strategy.md).

## Code standards

**Rust**
- `rustfmt` defaults; clippy clean at `-D warnings`.
- No `unwrap`/`expect` on any command path — return `MiraError` with something a person
  can read.
- Platform-specific code lives **only** in `mira-platform`. A `#[cfg(target_os)]` anywhere
  else fails CI.
- No crate except `src-tauri` may depend on `tauri`.
- Files past ~400 lines are a signal to split.

**TypeScript / React**
- Strict mode. No `any` on the IPC boundary — types are generated from Rust.
- Components read design tokens; a raw hex or magic pixel value is a review failure.
- Components past ~200 lines are a signal to split.

**Both**
- Every user-visible string is written for a person: plain, active, specific
  ([design system §9](docs/ux/design-system.md#9-voice)).
- Every OS-touching feature declares its capability status honestly. A feature that
  cannot work on a platform says so with a reason — it never silently no-ops.

## Things a PR will be asked to change

These are the recurring ones, listed so you do not discover them in review:

- Invoking a shell, or building a command string instead of an argv array
- Accepting a filesystem path from the frontend instead of a native picker
- Running a package manager, or any process outside `mira-platform`
- Writing to a user's repository from a crate whose job is to read it
- Storing anything observed on a workspace — Git, ports and processes belong to the
  project and are read live
- Reading a file outside a registered project root
- Opening anything under `~/.ssh` other than `config`
- Sending a write request to the Docker API
- Adding a network call that is not explicitly opt-in
- Starting a timer or poller outside `mira-scheduler`, or a `setInterval` in the UI
- Adding telemetry or an analytics dependency
- Claiming parity for a platform where the feature does not actually work
- Adding a `users`, `accounts`, or `telemetry` table
- Making a feature depend on Astra

Several of these are enforced by guard tests, so CI will usually tell you before a human
does.

## Adding your terminal or editor

The most contributable part of Mira. Terminals and editors are **data**, not code: add a
descriptor with the program, argv template, and capability flags
(`supports_line`, `supports_reuse`), plus a test. No platform code required.

## Reporting bugs

Include: OS and version (for Linux, your desktop environment and X11/Wayland), Mira
version, what you expected, what happened, and steps to reproduce. Platform bugs should
say which platform in the title.

**Security issues do not go in the issue tracker** — see [SECURITY.md](SECURITY.md).

## Code of conduct

By participating you agree to the [Code of Conduct](CODE_OF_CONDUCT.md).

## Licence

Contributions are accepted under the [MIT Licence](LICENSE). There is no CLA.
