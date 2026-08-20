# ADR-0009 — Git through libgit2, behind a provider trait

**Status:** Accepted · 2026-08-20

## Context

Reading Git is the first thing Mira does that is genuinely useful, and one of the
few things it does that a user would notice being wrong. Slice 1 needs the current
branch, HEAD's commit, whether the working tree is clean, and the distance to the
tracked upstream. Later slices in the 0.x line add the changed-path list, a commit
walk, and a lane graph. Nothing in the 0.x line **writes** to a repository
([prd.md](../product/prd.md) FR-3.4), and nothing fetches (FR-5.2).

Three ways to read a repository from Rust: run `git`, link libgit2, or use
`gitoxide`. [architecture.md](../architecture/architecture.md) §9 already recorded
the choice in prose; this ADR is that decision written down properly, with the
licence question answered, because it is expensive to reverse once the graph and
status code exist.

## Decision

**libgit2, through the `git2` crate, behind a `GitProvider` trait in `mira-git`.**

- `git2` is built with `default-features = false`. Its defaults compile HTTPS and
  SSH transports, which pull in OpenSSL on Linux. Mira never opens a network
  connection to a remote, so the transports are not merely unused — they are not
  built at all, and a future call that tried to fetch would fail to compile.
- `libgit2-sys` vendors and builds libgit2 from source, matching the reasoning
  behind bundled SQLite in [ADR-0004](0004-sqlite-local-first.md): "which libgit2
  does this machine have" is not a question the support matrix should contain.
- Every consumer sees the trait. `GitProvider::overview` returns states, not
  errors: "not a repository" and "cannot be read" are things the interface renders.

## Alternatives considered

**Shelling out to `git`.** Rejected, and not narrowly. It depends on the user's
`PATH` and on a `git` being installed at all; it costs a process per query, which
matters when the answer is wanted per project; its output is a text format tuned
for humans and changed between versions; and it would make the
command-injection surface real, in a product whose security model
([security-and-privacy.md](../architecture/security-and-privacy.md) §5) is built
on there being no shell anywhere. A guard test fails the build if a shell
invocation appears, and that guard is worth more than the convenience.

**`gitoxide` (`gix`).** The more attractive long-term choice, and still the
expected destination: pure Rust, no C toolchain, faster on large repositories, and
`MIT OR Apache-2.0` without the licence footnote below. It is production-ready for
the reads Mira performs. It was not chosen now because its status and graph APIs
are still moving, and Slice 1 is the wrong moment to absorb churn in the layer that
answers the product's most-asked question. The trait exists so this stays a
crate-internal change: swapping the implementation touches `mira-git` and nothing
above it, which is the acceptance test for the boundary
([architecture.md](../architecture/architecture.md) §4).

## The licence question, stated plainly

`git2` and `libgit2-sys` declare `MIT OR Apache-2.0`, so `cargo deny` sees a
permissive tree and passes. That is not the whole answer, and accepting it without
looking would be passing a licence check on a technicality.

The C library those crates vendor is **GPLv2 with a linking exception**. The
exception is explicit:

> In addition to the permissions in the GNU General Public License, the authors
> give you unlimited permission to link the compiled version of this library into
> combinations with other programs, and to distribute those combinations without
> any restriction coming from the use of this file.

Linking libgit2 into Mira and shipping Mira under MIT is exactly the case the
exception was written for. The GPL's terms continue to apply to libgit2's own
source: if Mira ever modified it, those modifications would be GPLv2, and Mira
does not modify it — `libgit2-sys` builds it unpatched from the vendored source.

The consequence worth naming: this reasoning lives here rather than in
`deny.toml`, because a tool that reads crate metadata cannot see a C library's
COPYING file. Anyone auditing Mira's licensing should read this section rather
than trusting the green check.

## Consequences

**Good.** A mature, well-understood library covering everything the 0.x line
needs. No `PATH` dependency and no `git` requirement on the user's machine. No
process per query. Reads are a library call, so the per-repository timeout
([prd.md](../product/prd.md) feature 3) is a `spawn_blocking` plus a deadline
rather than process management.

**Costs.** A C toolchain is required to build Mira — already true for Tauri on all
three platforms, so the marginal cost is close to zero, but it is now true for two
reasons instead of one. Compile time grows by the libgit2 build, which the CI
cache absorbs. And the licence reasoning above has to be maintained by a person,
because no tool checks it.

**Bounded.** libgit2 is reached only through `mira-git`. A guard test keeps `git2`
out of every other crate's manifest, so the migration to `gitoxide`, when its APIs
settle, is one crate's internals and no caller's problem.
