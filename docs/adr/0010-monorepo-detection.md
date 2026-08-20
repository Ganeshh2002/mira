# ADR-0010 — Monorepo detection: a separate crate, read-only, computed not stored

**Status:** Accepted · 2026-08-20

## Context

A directory a person points Mira at is one of three things: a project on its own,
the root of a monorepo, or one package inside one. Slice 1 treated all three
identically, which is wrong in a specific and visible way — a package selected
inside a monorepo showed the whole repository's Git state with no indication that
the two were different scopes.

Fixing it raises four questions that are expensive to answer twice: where the
detection lives, how a workspace manifest is read, whether the result is stored,
and what stops a walk over someone's repository from becoming a walk over their
disk.

## Decision

### A separate crate, named `mira-monorepo`

Detection lives in its own crate, depending only on `mira-core`, exposing one
function and three types.

It is **not** called `mira-workspace`. "Workspace" is already taken:
[information-architecture.md](../ux/information-architecture.md) §1 defines
Project → Workspace → Session as the product's spine, and Workspace there means
*a named way of working on a project*, arriving in 0.2. npm and Cargo also call
their package sets workspaces. Using one word for both would corrupt the
vocabulary in the one document whose job is to keep it straight, so the crate is
named after what it detects and the detected units are **packages**.

It is also not part of `mira-git`. The two answer different questions from
different files: Git reads `.git`, this reads `package.json` and `Cargo.toml`.
The only thing they share is that a worktree root bounds the search, and that
arrives as a parameter — so this crate needs no Git, and its tests need no
repository.

### Manifests are read, never executed

Every provider reads a file and parses it. Nothing runs `npm`, `pnpm`, `yarn`,
`cargo metadata`, `nx graph`, or `turbo`. Two guard tests hold the line: one
restricts `Command::new` to `mira-platform`, and one forbids every filesystem
write in the crates that read a user's repository.

This is not caution for its own sake. `cargo metadata` would be the accurate way
to enumerate a Cargo workspace, and `pnpm list` the accurate way to enumerate a
pnpm one — and both execute build scripts and plugins from a repository the user
may have only just cloned. Mira reads a project; it does not run one.

### pnpm's YAML is parsed as the subset it is

`pnpm-workspace.yaml` needs one key: `packages`, a list of strings. The obvious
dependency, `serde_yaml`, is archived and carries a RUSTSEC unmaintained
advisory. Taking it would put an unmaintained parser for a large, ambiguous
format on the path that reads files out of repositories the user did not write.
The available forks are young.

So Mira parses the documented subset it needs — block sequences, flow sequences,
quotes, comments — in about eighty lines with its own tests, and treats anything
it cannot read as *no packages declared*. The cost of that trade falls on a user
with an exotic workspace file: they see no monorepo badge. The cost of the
alternative falls on everyone.

`Cargo.toml` uses `toml`, and the JSON manifests use `serde_json`. Both are
maintained, both are already in the tree.

### Nothing detected is persisted

No migration, no new table, no new column.

[data-model.md](../architecture/data-model.md) §1 rule 2 says store intent, not
observation: projects are what a person stated, and a package list is what a
repository currently happens to contain. It changes when someone edits
`pnpm-workspace.yaml` or adds a directory, which is to say it changes without
Mira watching — and a cached list of packages is a list that goes wrong silently.
Detection is cheap enough to run with the Git read it already accompanies.

The stronger version of the same argument: turning every detected package into a
persistent Project would hand the user a list they did not ask for and now have
to curate. Adopting a package as a project is their decision. Mira shows the
boundary; it does not act on it.

### Every walk is bounded, and the bounds are constants

A repository is exactly where an unbounded directory walk goes wrong. `**` at the
root of a large monorepo is a request to read the whole tree, and a single
`node_modules` holds tens of thousands of manifests.

Four bounds, all named constants rather than heuristics: `**` spans at most four
segments; one detection visits at most 4 000 directory entries; at most 500
packages are reported; and the climb upwards stops at the Git root, or after
eight levels when there is no repository. A skip list keeps the walk out of
`node_modules`, `target`, `dist` and their kind, and symlinks are not followed,
so a link pointing back up the tree cannot make the walk cycle.

Exceeding a bound yields **fewer packages, never an error and never a hang**. A
monorepo whose shape Mira cannot fully see is still more useful than a spinner.

### A claim needs a confirmation

A directory is reported as a package only when the manifest that would make it
one is actually present — `package.json`, `Cargo.toml`, or Nx's `project.json`.
A directory that merely sits in the right place is not evidence of a package, and
naming it would be inventing one.

The same rule applies upwards: a monorepo root is reported only when at least one
package is confirmed. A `turbo.json` with no package manager behind it, an
`nx.json` in a repository with no projects, a `pnpm-workspace.yaml` Mira could
not read — each is a monorepo claim with nothing behind it, and the requirement
was to show only what can be detected reliably.

## Alternatives considered

**Ask the tools.** `cargo metadata --no-deps`, `pnpm -r list --json`,
`nx show projects`. Accurate, and the reason they are accurate is that they
execute the repository. Rejected on the security model, not on taste
([security-and-privacy.md](../architecture/security-and-privacy.md) §5 rule 6:
Mira never executes anything that was not a user action).

**Detect by directory shape.** Treat `apps/*` and `packages/*` as packages
wherever they appear. Rejected: it invents boundaries in repositories that merely
happen to use those names, and the product rule is to show what is there.

**Persist packages as rows.** Would let the project list show package counts
without a scan. Rejected above; revisit only if a scheduler makes the staleness
manageable, which is a 0.2 conversation at the earliest.

**One provider trait with dynamic dispatch.** The providers share a shape but not
enough behaviour to be worth a trait object; a table of functions says the same
thing and adding a tool stays a two-line change.

## Consequences

**Good.** Six tools supported by reading six manifest formats, with no process
execution and no network. Detection is a pure function of the filesystem, so it
is tested against real fixture trees on any machine. Adding a tool is a provider
function and a row. Nothing new is stored, so nothing new can go stale.

**Costs.** The pnpm parser is Mira's to maintain, and a workspace file outside its
subset silently detects nothing — the failure is invisible rather than loud,
which is the right shape here but is still a failure. `MAX_PACKAGES` truncates
without saying so; at 500 packages no real repository is affected, but the cap is
a silent one and is named here so it is not a surprise later. And detection runs
on every context read rather than once, which is a directory walk per project
view — bounded, but not free.

**Bounded.** `mira-monorepo` is reached only from the context command. Deleting
the crate would remove the Monorepo section and leave everything else compiling.
