# ADR-0017 — File history: bounded by commits examined, and named without a path

**Status:** Accepted · 2026-08-21

## Context

Slice 5d answers "what has happened to this file". It is the last read in the 0.x
Git surface, and it is the one that does not fit the pattern the previous three
established.

5a and 5b bounded their reads by counting **commits** — twenty-five per page,
whatever the repository. 5c bounded its reads by declaring limits on **files,
lines and bytes**, and refusing anything past them in constant time by reading an
object header rather than an object.

File history admits neither trick, for a reason that is not an implementation
detail:

> To know whether a commit touched a path, you have to look at that commit. To
> find the commits that touched it, you have to look at all of them.

`git log -- <path>` does exactly this. There is no index of "commits that touched
path P", and building one would be a cache with an invalidation problem attached.

There is also a second question, and it is the sharper one.
`security-and-privacy.md` §5 rule 8 says **no command takes a filesystem path from
the frontend**. Every other tool spells file history as `git log -- <pathspec>`. A
feature whose defining input is a path, in a product whose defining rule is that
paths do not cross the boundary, needs an answer rather than an exception.

## The measurement

Taken before the design was chosen, on a repository where one file changes every
fiftieth commit (release build, macOS, Apple silicon).

**An unbounded trace — what every other client does:**

| commits | full scan | per commit |
|---|---|---|
| 1 000 | 30.3 ms | 30.3 µs |
| 5 000 | 157.1 ms | 31.4 µs |
| 20 000 | **740.0 ms** | 37.0 µs |
| **1 000 → 20 000** | **×24.4** | linear |

**Mira's bounded page, same repositories:**

| commits | found | examined | page |
|---|---|---|---|
| 1 000 | 20 (all there were) | 1 000 | 70.1 ms |
| 5 000 | 25 | 1 250 | 104.3 ms |
| 20 000 | 25 | 1 250 | **100.0 ms** |
| **1 000 → 20 000** | | | **×1.4** |

And the question the slice was asked to settle specifically — **does following
renames cause repository-wide traversal?**

> **No.** A rename-detecting diff of one commit costs **0.03 ms**, and a rename is
> a rare event rather than a per-commit cost. Following renames adds nothing
> measurable. The traversal is the cost, and it is the cost whether or not renames
> are followed.

That is the whole finding. It moved the bound off the feature people assume is
expensive and onto the one that actually is.

## Decision

### 1. The bound is on commits examined, not on anything about the file

`MAX_SCAN = 2 000` commits per request. Measured at ~100 ms; ten thousand would be
~500 ms, which is past the point where a click feels like one.

A request stops at whichever comes first: a full page ([`PAGE`] = 25 commits that
touched the file), or the scan budget. **Both outcomes carry a cursor**, so what a
page did not reach the next one does.

### 2. Stopping early is a different sentence from finding nothing

`ScanStopped::Budget { scanned, limit }` says how many commits were looked at. The
interface renders *"Nothing in the last 2 000 commits. There may be more further
back"* — never *"No commit has touched this file"*.

Confusing those two would be the worst thing this surface could do, and it would
happen precisely for the files the bound exists for: the rarely-touched ones deep
in a long history.

### 3. Deciding whether a commit touched a path is two tree lookups

Not a diff. `tree.get_path(p)` on the commit and on its first parent, then compare
the blob ids — 30 µs. A diff per commit would be ~0.03 ms each, which is 60 ms per
two thousand commits *on top of* the walk, and it would buy nothing.

A rename-detecting diff runs at exactly one place: **the commit where the path
appears**, which is the only place a rename can be hiding. A guard test asserts
that `touched()` contains no `diff_tree_to_tree`, `Patch::`, `changed_files` or
`find_similar`.

A merge is compared against its **first parent**, the same choice ADR-0016 makes
and for the same reason.

### 4. A file is named by a `FileSubject`, never by a path

```rust
pub struct FileSubject {
    pub scope: DiffScope,  // a change set Mira produced
    pub at: u32,           // a position in it
    pub before: bool,      // which side of that change to take the name from
}
```

There is no field a path could live in — a guard test asserts that, and asserts
positively that `scope` and `at` are there so it cannot pass vacuously. The
interface **receives** a subject (in a change list, or in a previous page's
cursor) and hands it back. It has no way to build a different one.

`before` is what makes rename-following survive pagination. When a page ends on
the commit that renamed the file, the next page must continue under the *old*
name — and the old name is recoverable as "the `from_path` of that same change",
which is a position in a list Mira produced. So the name changes across pages
without a name ever being sent.

That is the answer to the second question in the context: file history does not
need an exception to rule 8. It needs a different way of pointing.

### 5. It is a view, never an edit

No checkout, restore, revert, blame-annotate-and-edit, or anything else. ADR-0015's
guard — every libgit2 write API absent from `mira-git`, proven by injection —
covers this slice unchanged.

### 6. Clicking a commit goes to the existing commit surface

A file's history is a list of commits, and Mira already has a place to look at
one. Selecting a row opens that commit's detail rather than growing a fourth kind
of commit view.

## Alternatives considered

**A pathspec parameter.** How every other tool spells it, and the reason this ADR
exists. Rejected: it puts a path on the wire and then relies on validation to make
it safe. A subject has nothing to validate, because there is nothing a path could
have been.

**Caching a path→commits index.** Would make repeat traces instant. Rejected for
now: it is a cache over a mutable repository, so it needs an invalidation story,
and the thing it would speed up is a read a person does deliberately and
occasionally. Worth revisiting if traces become a hot path; not worth a coherence
bug today.

**`--follow`-style heuristics beyond one rename.** Git's own `--follow` is
documented as a heuristic and is famously imperfect. Mira does the tractable part —
follow the rename recorded at the commit where the path appears — and says so when
it cannot go further (`ScanStopped::RenameLost`), rather than guessing.

**A bigger budget.** 10 000 commits costs ~500 ms. Rejected: a slow answer that
arrives is worse than a fast partial one that says it is partial and offers more.

**No budget, with a spinner.** Rejected on the 740 ms row above, which is a
20 000-commit repository — small by the standards of the projects Mira is for.

## Consequences

**Good.** A trace costs what the budget says: ×1.4 across a twenty-fold
repository, against ×24.4 unbounded. Renames are followed and cost nothing.
No path crosses the IPC boundary, so §5 rule 8 survives the one feature that
looked certain to need an exception. Every stopping point is a sentence.

**Costs.** A rarely-touched file in a very long history takes several requests to
trace fully, each one a button press. That is a real cost and it is visible —
which is the trade this ADR chose over an invisible one. `MAX_SCAN` is one
constant somebody has to keep honest, and the `before` flag on `FileSubject` is a
subtlety that will need explaining to the next reader (hence this section).

**Bounded.** The walk is one file, the budget one constant, and the benchmark that
justifies it a committed test — including the unbounded comparison, kept so the
refusal stays justified rather than merely inherited.
