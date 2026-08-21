# ADR-0015 — Graph lanes over a bounded window, without topological ordering

**Status:** Accepted · 2026-08-21

## Context

Slice 5b draws the shape of a history: which commits descend from which, where a
branch left the mainline, where it came back. The interface for it is settled —
`design-system.md` §8 gives lanes 1px strokes, a cap of eight, and a muted ramp;
`information-architecture.md` §5 puts the picture beside the History list.

The question this ADR answers is not how to draw it. It is **how much of a
repository Mira is allowed to read in order to draw twenty-five rows.**

A graph is the first read in the product whose obvious implementation is
unbounded. Every drawing algorithm in the literature assumes a *topologically
ordered* commit stream, because topological order guarantees that a parent is
never emitted before its child — which makes lane assignment a single forward
pass with no special cases. Git itself offers that ordering, and so does libgit2:
`Sort::TOPOLOGICAL`.

Slice 5a had already found the shape of the problem from the other side. Asking
libgit2 for `Sort::TIME` made a first page cost 2.5 ms on a hundred commits and
272 ms on ten thousand, because a sorted revwalk **preprocesses the entire
reachable history before yielding its first commit**. The default order — the
same reverse-chronological sequence `git log` prints — is produced lazily and
costs the same at any size. History shipped unsorted for that reason.

So the graph arrives at a fork. Take the ordering the algorithm wants and give up
the bound; or keep the bound and make the algorithm survive an ordering that is
*mostly* topological and occasionally not.

## The measurement

Taken before deciding, on the fixtures in
[`crates/mira-git/tests/performance.rs`](../../crates/mira-git/tests/performance.rs)
(release build, macOS, Apple silicon). All three columns produce the **same
twenty-five rows**.

| commits | history page | graph page | topological page |
|---|---|---|---|
| 100 | 0.91 ms | 1.15 ms | 3.40 ms |
| 1 000 | 0.93 ms | 0.88 ms | 34.03 ms |
| 10 000 | 1.02 ms | 0.88 ms | **425.71 ms** |
| **100 → 10 000** | **×1.1** | **×0.8** | **×125.2** |

The graph column includes parent ids, lane assignment and reference labelling. It
is flat. The topological column is the same page, ordered the way the textbook
algorithm wants, and it grows linearly with the repository behind it — half a
second on ten thousand commits, and a repository ten times that size is ordinary.

That is not a slow implementation to be optimised. It is a different asymptotic
class: **O(history) to render O(page).**

## Decision

**Lanes are computed by a pure function over the visible window, from the same
unsorted walk the history list already uses. No sorted revwalk, ever.**

### 1. The window is the input, and the only input

`lanes::layout(&[Node]) -> Layout` takes commit ids and parent ids and returns
placements. It has no `Repository`, no cursor, and no way to ask for another
commit — a guard test asserts that `Repository`, `git2`, `PAGE` and `fs::` do not
appear in the file. "The graph is computed over what is on screen" is therefore a
fact about the signature rather than a claim about the caller.

Cost is proportional to rows on screen and lanes among them. A repository with a
million commits costs what one with thirty costs.

### 2. There is no second history

The graph reads the *same page* through the *same walk* as `GitProvider::history`,
and adds two things: parent ids, which are already inside the commit objects the
walk loaded and cost nothing; and the references that point into the window.
`git.graph` and `git.history` return the same commits in the same order with the
same cursor — asserted by test, because the picture sits beside the list and a
divergence would label the wrong rows.

### 3. The ordering cost is paid honestly, not hidden

Reading in date order means a repository with skewed commit timestamps can place
a parent **above** its child. The layout detects this — it knows which ids it has
already emitted — and reports the relationship as `EdgeKind::Reordered`, for which
**no line is drawn**.

That is the one rule the picture cannot break: a line running upward into a row
that has already been scrolled past is a picture of something that is not there.
Reporting the relationship and drawing nothing is a small, local, honest loss. It
is also rare: it requires a repository whose commit dates disagree with its
topology, which is a broken clock or a rewritten history.

A second, more visible cost: when two sides of a merge share a parent, the lane
that parent lands in depends on which side the window reached first, and in date
order that is whichever was committed later. Topological order would keep the
mainline at lane 0. One line of the picture bends where it need not have, and the
test that covers it says so out loud rather than asserting a lane number that
happens to be true.

### 4. Bounds are declared and the interface is told when they bite

- **Lanes** cap at `MAX_LANES = 8` (`design-system.md` §8). Past that, lanes fold
  onto the eighth, every commit stays listed, and `collapsed` tells the interface
  to say so. Folding is honest; dropping rows would not be.
- **References** cap at `MAX_REFS = 500`. A reference scan is bounded by how many
  refs a repository has rather than by how long its history is — but that is not
  the same as "small", and a repository with tens of thousands of tags exists.
  `refs_truncated` says when the ceiling was reached.

Both flags render as a sentence rather than being swallowed.

### 5. It is a picture, not a client

Nothing in `mira-git` can check out, merge, rebase, reset, cherry-pick, create a
commit, stage a path, move a reference, or reach a remote. A guard test names
every libgit2 write API and fails the build if one appears in the crate's
sources — proven able to fail by injection. A second guard bans a command
parameter named for any of those verbs.

## Alternatives considered

**`Sort::TOPOLOGICAL`.** The textbook answer, and the one the measurement
rejected. It would make lane assignment simpler — no reordered case, and a
shared parent always landing on the mainline — at the cost of reading the whole
repository to draw one page. Half a second on ten thousand commits is already
past the point where a view feels responsive, and it gets worse for exactly the
users who have the most history to look at. A bounded interface over an unbounded
traversal is the failure this slice was most at risk of, and it would have been
invisible on the fixtures a developer tests with.

**A topological sort of the window alone.** Locally correct, bounded, and
rejected for a different reason: it would reorder the rows relative to the History
list, and the graph is drawn *beside* that list. Two orderings would mean two
lists, or a gutter labelling the wrong rows.

**Fetching a lookahead beyond the window** — say, PAGE × 4 — so that more parents
resolve inside it. Rejected as a bound in disguise: it makes the cost bigger
without making it fixed, and it does not remove any case, only makes it rarer.
Rarer bugs are worse than visible ones.

**Rendering the graph as one tall canvas.** Rejected in favour of a per-row SVG.
A single canvas has to stay in step with a list whose rows it does not own; a
per-row gutter with fixed geometry means a line drawn to the bottom of one row
meets the line drawn from the top of the next without either knowing the other
exists.

## Consequences

**Good.** A page costs a page. The layout is a pure function testable from a table
of ids, so a merge, a root, an octopus, a fork, a broken history and a clock skew
are all assertable without a repository on disk. The graph and the list are one
read, so switching between them cannot show different commits. The picture is
decoration in the strict sense — `aria-hidden`, with every fact it conveys also
written on the row — so a narrow window hides the gutter and loses width, not
content.

**Costs.** Two visible imperfections, both documented above and both tested: a
reordered parent draws no line, and a shared parent's lane depends on which side
the window reached first. Neither is a wrong picture; both are pictures that a
topological ordering would have drawn more tidily.

**Bounded.** The layout is one file, the bound is two constants, and the
measurement that justifies them is a committed test that can be re-run on any
machine. If a future release wants topological order, it needs a new measurement
and a new ADR — not a one-line change to a `set_sorting` call, which a guard test
now refuses.
