# ADR-0016 — Bounded diffs, and a file chosen by ordinal

**Status:** Accepted · 2026-08-21

## Context

Slice 5c shows what changed: a commit's files, a working tree's files, and one
file's patch. It is the last read in the 0.x Git surface, and it is the first one
whose size is set by **the repository's files** rather than by a page of history.

That difference is the whole decision. Slices 5a and 5b bounded their reads by
counting *commits* — twenty-five of them, whatever the repository. A diff cannot
be bounded that way:

- one commit can touch ten thousand paths (a dependency bump, a formatting pass, a
  vendored directory);
- one path can be a forty-megabyte minified bundle **on a single line**;
- one path can be a four-megabyte PNG, which has no lines at all;
- rename detection is quadratic in the candidates it considers.

Any of those, read naively, turns a click into a stall — and none of them is
exotic. The two hazards are equally real: **reading too much**, and **appearing to
have read everything when you did not**.

There is also a second question that only arises here. A diff needs to know *which
file*, and `security-and-privacy.md` §5 rule 8 says no command takes a filesystem
path from the frontend. Registering a project root is a privileged act reserved to
a native picker; a diff must not become the back door around it.

## The measurement

Taken before choosing the limits, on the fixtures in
[`crates/mira-git/tests/performance.rs`](../../crates/mira-git/tests/performance.rs)
(release build, macOS, Apple silicon).

| subject | on disk | change list | patch | lines returned | bytes returned | state |
|---|---|---|---|---|---|---|
| tiny text | < 1 KB | 0.37 ms | **0.31 ms** | 3 | < 1 KB | whole |
| 1 MB text | 937 KB | 4.82 ms | **4.03 ms** | 2 000 | 21 KB | line limit |
| large text | 7 031 KB | 0.26 ms | **0.19 ms** | 0 | 0 | too large |
| wide lines | 29 316 KB | 0.21 ms | **0.17 ms** | 0 | 0 | too large |
| binary, under ceiling | 1 464 KB | 2.17 ms | **2.11 ms** | 0 | 0 | binary |
| binary, over ceiling | 3 906 KB | 0.22 ms | **0.18 ms** | 0 | 0 | too large |

| files changed | change list | returned |
|---|---|---|
| 10 | 1.83 ms | 10 |
| 500 | 18.69 ms | 200 |
| 4 000 | 20.12 ms | 200 |

The shape worth reading twice: **a 30 MB file costs 0.17 ms and a 1 MB file costs
4 ms.** The large one is faster because it is never read. And a commit of 4 000
files costs barely more than one of 500, because both stop at the same ceiling.

## Decision

**Five declared limits, none of them nameable by a caller; every one of them
reports itself; and a file is chosen by its ordinal in a list Mira produced.**

### 1. The limits

| Limit | Constant | Value |
|---|---|---|
| Files listed per change set | `MAX_FILES` | 200 |
| Lines rendered per file | `MAX_LINES` | 2 000 |
| Bytes of patch text per file | `MAX_BYTES` | 256 KiB |
| Bytes kept on one line | `MAX_LINE_BYTES` | 2 000 |
| Blob size Mira will diff at all | `MAX_FILE_BYTES` | 2 MiB |
| Blob budget for counting a list's lines | `MAX_STATS_BYTES` | 8 MiB |

All are constants in `mira-git`. **No command carries a number that could raise
one** — a guard test fails the build if a `limit`, `count`, `max`, `size`, `depth`
or `all` parameter appears on any command, and `wire.rs` asserts the values.

`MAX_LINES` and `MAX_BYTES` both exist because neither implies the other: two
thousand lines of a minified bundle is still megabytes, and 256 KiB of ordinary
source is far more than two thousand lines. Whichever bites first is the one
reported.

### 2. The size gate reads a header, not a file

This is what makes the ceiling a *limit* rather than a *cleanup*. libgit2 fills a
delta's `size` only once it has loaded the blob, so asking it directly would mean
reading the very file the ceiling exists to refuse. Mira reads the **object
header** instead (`Odb::read_header`), which carries the size and not the content,
and hands libgit2 the same ceiling as its own `max_size` so nothing large is
materialised even to be rejected.

That is why the 30 MB row above costs 0.17 ms.

### 3. Nothing is truncated silently

Every limit has a value that says it bit: `FilesTruncated::Yes { shown, total,
limit }`, `PatchTruncated::Lines { .. }`, `PatchTruncated::Bytes { .. }`,
`FileDiff::TooLarge { bytes, limit }`, `DiffLine::cut`. Each renders as a sentence
naming the number.

A diff that quietly stopped short would be **worse than no diff**, because it
would look complete — and somebody would review a change they had not seen.

### 4. Binary files are identified, never decoded

`FileDiff::Binary` carries the two sizes and no text. A guard test asserts that the
variant carries no field named for content, and that binariness is asked about
before a patch is rendered. Binariness is libgit2's judgement, made once it has
looked — so it is read from the generated patch's delta rather than guessed.

### 5. A file is chosen by its ordinal, never by a path

`git.file_diff` takes `at: u32` — a **position in the list `git.changes`
returned**. The interface can only ask for a file Mira already decided to offer,
and an ordinal past the list reads nothing. So:

- there is no path-shaped argument, which §5 rule 8 forbids;
- there is no way to reach a file outside the change set, whatever the repository
  holds;
- an ordinal past `MAX_FILES` is refused before any comparison is built.

A guard test bans the shapes somebody would reach for when they wanted a path and
knew the path rule existed: `blob`, `oid`, `pathspec`, `glob`, `filename`,
`prefix`, and the rest.

### 6. A merge is compared against its first parent, and says so

Against the second parent the same commit changed different things. `Comparison`
records which side was taken and the interface renders it — a diff that did not say
would be quietly choosing one. (Slice 5a's commit detail declines to give a
changed-file *count* for a merge for the same reason; the list, which can explain
itself, gives one.)

### 7. The working tree is a separate question

`DiffScope` has two variants: one commit, or the working tree. They are separate
answers in separate places — one is what is recorded, the other is what is on
disk, and a single list of both would make it impossible to tell them apart.

### 8. It is a view, never an edit

No staging, checkout, apply, revert, commit, cherry-pick, merge, rebase, reset or
remote. The guard added in 5b — every libgit2 write API absent from `mira-git`,
proven able to fail by injection — covers this slice unchanged, and a second guard
bans a command parameter named for any of those verbs.

## Alternatives considered

**A path parameter, validated against the change set.** The obvious design, and
rejected: it puts a path on the wire and then relies on a check to make it safe.
An ordinal has nothing to validate, because there is nothing a path could have
been. Fewer moving parts, and the guard is a type rather than a comparison.

**Caching the change set between the list and the patch request.** Would save
rebuilding the comparison per file. Rejected for now: the rebuild is bounded and
measured at 0.2–5 ms, and a cache would introduce a coherence question (what if the
working tree moved between the two calls?) in exchange for milliseconds. The
current answer to that question is the honest one — the ordinal names whatever is
there now, and a stale selection reads as `Unknown`.

**Streaming a large diff instead of truncating it.** The right answer for a
terminal, the wrong one for a webview that has to hold what it renders. Truncation
with a stated number is what a reader can act on; an endless scroll is not.

**A higher `MAX_FILE_BYTES`.** 2 MiB refuses some legitimately large generated
files. The trade is deliberate: the refusal is instant and explains itself, while
the alternative is a 30 MB string crossing the IPC boundary into a webview. Raising
it is a one-line change with a measurement attached, not a debate.

## Consequences

**Good.** A diff costs what the limits say, not what the repository holds — 0.17 ms
to refuse 30 MB. Every stopping point is visible, so nothing looks complete when it
is not. No path crosses the IPC boundary, so §5 rule 8 survives a feature that
looked like it would need an exception. Binary files never become strings.

**Costs.** Six constants somebody has to keep honest, and one — `MAX_STATS_BYTES` —
whose effect is subtle: past it, a change list shows no `+`/`−` counts for the
remaining files. That is a visible gap rather than a wrong number, and opening the
file is authoritative. The 2 MiB file ceiling will occasionally refuse something a
person wanted to see.

**Bounded.** The limits are in one file, the reads in another, and the benchmark
that justifies them is a committed test that runs on any machine. Changing a limit
means changing a constant and re-running one command.
