# Aviora Mira — Git & Release Strategy

Deliberately small. This is a young open-source project, not an enterprise; every rule
below exists because it saves work, and anything that did not was left out.

---

## 1. Repository structure

**One repository.** Frontend, Rust backend, and docs ship together because they are
released together and change together. A split would buy nothing and cost every
cross-cutting change a second PR.

```
mira/
├─ src/                     React + TypeScript
├─ src-tauri/               Tauri binary
├─ crates/                  Rust library crates
├─ docs/                    product, ux, architecture, adr, implementation, development
├─ .github/workflows/       CI
└─ README · LICENSE · CONTRIBUTING · CODE_OF_CONDUCT · SECURITY · CHANGELOG
```

Lockfiles (`Cargo.lock`, `package-lock.json`) are **committed** — this is a shipped
application, not a library.

## 2. Branches

```
main ────────●────────●────────●────────●──▶   always releasable
              \      /          \      /
               feat/x            fix/y
```

- **`main`** is the only long-lived branch. It always builds and always passes CI.
- Work happens on short-lived branches: `feat/`, `fix/`, `docs/`, `chore/`, `refactor/`,
  named `feat/port-attribution`, not `feat/issue-42`.
- Branches live days, not weeks. A branch open longer than a week is a scope problem.
- No `develop`, no release branches, no GitFlow. A single-artifact desktop app with one
  release train does not need them. Release branches (`release/0.2.x`) appear only if a
  patch is ever needed for an old minor — which has not happened yet and may never.

## 3. Merging

**Squash-merge into `main`.** One feature, one commit, one revert. The PR title becomes
the commit subject, so PR titles follow the commit convention below.

Required to merge: CI green on macOS, Windows, and Linux; one maintainer approval; the
conversation resolved. Force-pushing to `main` is disabled.

Rebase your branch on `main` before merging; do not merge `main` into your branch
repeatedly — the history should read as a straight line.

## 4. Commit convention

[Conventional Commits](https://www.conventionalcommits.org/), because it makes the
changelog derivable rather than hand-written.

```
<type>(<scope>): <subject>

[body: why, not what]

[footer: Closes #12 / BREAKING CHANGE: …]
```

**Types:** `feat` · `fix` · `docs` · `refactor` · `perf` · `test` · `build` · `ci` ·
`chore`.
**Scopes** map to modules: `git`, `ports`, `processes`, `shelf`, `peek`, `ssh`, `docker`,
`media`, `system`, `platform`, `ui`, `db`, `tray`, `shortcut`, `docs`.

```
feat(ports): attribute listeners to projects by process cwd
fix(git): stop panicking on non-UTF-8 commit messages
docs(adr): record the choice of rusqlite over tauri-plugin-sql
```

Subject: imperative, lower case, no trailing period, under 72 characters. The body
explains **why** — the diff already shows what.

## 5. Development workflow

```
issue ──▶ branch ──▶ TDD ──▶ PR ──▶ CI + review ──▶ squash ──▶ main
```

1. **Issue first** for anything non-trivial, so the discussion outlives the PR.
2. **Branch** from current `main`.
3. **Test first.** Logic gets a failing test before the implementation
   ([architecture.md](../architecture/architecture.md) §10).
4. **PR early**, marked draft if it is not ready. Small PRs get reviewed; large ones rot.
5. **CI** runs fmt, clippy (`-D warnings`), tsc, eslint, unit and integration tests, the
   guard tests, and a build on all three platforms.
6. **Review** by a maintainer. Verify the claim, not just the diff.
7. **Squash-merge.** Delete the branch.

Docs are part of the change, not a follow-up: a PR that alters behaviour updates the
document that described the old behaviour, in the same PR. A PR that contradicts an ADR
must add a superseding ADR.

## 6. Versioning

[Semantic Versioning](https://semver.org/), with the pre-1.0 rules stated plainly:

- **`0.x.y` until the product is feature-complete for V1.** In `0.x`, a **minor** bump
  may change behaviour, and a **patch** is a fix. Nothing is promised as stable yet.
- **`1.0.0`** when the MVP has shipped, been used, and the command surface has settled.
- After 1.0: MAJOR = a breaking change to the database schema, the command surface, or a
  documented behaviour; MINOR = a feature; PATCH = a fix.

The version lives in `package.json`, `src-tauri/tauri.conf.json`, and the workspace
`Cargo.toml`. One script bumps all three; they may never drift.

**Schema versions are separate** and only move forward. A database from a newer Mira is
refused by an older Mira with a clear message
([data-model.md](../architecture/data-model.md) §4).

## 7. Releases

Tag-driven. `v0.2.0` on `main` triggers the release workflow: build and bundle for the
six targets, generate the changelog section, create a draft GitHub release, attach
artifacts and the updater manifest, and publish once a maintainer confirms.

- **Cadence:** when something is worth shipping. A slice completing is the natural
  trigger (see [roadmap.md](../implementation/roadmap.md)). No calendar releases.
- **Pre-releases:** `v0.3.0-rc.1` for anything touching migrations or the platform layer.
- **Artifacts:** `.dmg`, `.msi`, `.exe` (NSIS), `.AppImage`, `.deb`, `.rpm`, plus
  `latest.json` for the updater.
- **Signing:** macOS notarisation and Windows signing when certificates exist. Until then
  releases say plainly that builds are unsigned and how to open them — never a workaround
  disguised as a feature.
- **Release notes** state what changed, what broke, and what is still not implemented.
  Never claim a planned feature as shipped.

## 8. Changelog

`CHANGELOG.md`, [Keep a Changelog](https://keepachangelog.com/) format, generated from
Conventional Commits and then **edited by a human** before release — generated changelogs
are accurate and unreadable.

`## [Unreleased]` collects work as it merges; releasing renames it to the version and
date.

## 9. Issue and PR hygiene

Labels: `bug` · `feature` · `docs` · `good first issue` · `platform:macos|windows|linux` ·
`slice:N` · `blocked` · `wontfix`.

Every issue that touches a platform names which one. Every feature issue names the slice
it belongs to, or is explicitly out of scope — that is how the roadmap stays honest
instead of becoming a wish list.

## 10. What this project deliberately does not do

No signed commits requirement, no CLA, no merge queue, no monorepo tooling, no
auto-merge, no changesets, no release calendar, no `develop` branch, no minimum-review
count above one. Each would add process before there is enough traffic to need it. They
can be added the day the project outgrows this page.
