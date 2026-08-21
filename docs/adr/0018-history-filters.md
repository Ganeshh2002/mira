# ADR-0018 — History filters: one budget, four questions, and nothing that is an argument

**Status:** Accepted · 2026-08-21

## Context

Slice 5e lets a reader narrow the history: to a branch, to an author, to a file,
to some words in a subject line — and to any combination of those at once.

It is the first read in Mira whose *input* is a search rather than a position.
Every read before it named a thing (a commit, a page, a change set, a file) and
Mira went and fetched it. This one describes a thing and Mira goes looking. That
changes two properties of the surface at once, and this ADR is about both.

**Every other tool spells this as arguments.** `git log --author=… --grep=… main
-- path`. Four of those five are strings from the user, and one of them is a
pathspec. `security-and-privacy.md` §5 says no command takes a Git argument or a
filesystem path from the frontend, and ADR-0017 already had to answer that once
for a single path. A feature made of *four* such strings needs the answer to
generalise rather than be re-invented.

**Searching is unbounded by nature.** "The commits by Grace" has no answer until
the whole history has been read, and the honest size of that answer is unknown
before you start. Paging by twenty-five works when every commit is a result; it
does not work when the first result might be four thousand commits back.

## The measurement

Taken before the design, as ADR-0015 and ADR-0017 were. The first question was
whether the four filters differ enough to need different treatment — because a
cheap filter and an expensive one would justify different budgets, and possibly
different UI.

**Cost per commit examined, by predicate** (release build, macOS, Apple silicon):

| predicate | 1 000 commits | 10 000 commits |
|---|---|---|
| none — the walk alone | 21.9 µs | 22.9 µs |
| author contains | 19.2 µs | 22.1 µs |
| subject contains (Unicode lowercase) | 19.1 µs | 25.0 µs |
| subject, allocation-free ASCII | 18.4 µs | 23.8 µs |
| file touched | 19.2 µs | 23.3 µs |
| all three together | 19.1 µs | 22.6 µs |

> **Every filter costs what the walk costs.** Loading the commit object dominates
> so completely that testing three predicates on it is free — indistinguishable
> from testing none, and within noise of the walk by itself.

That single finding decided most of what follows. There is no cheap filter to
fast-path and no expensive one to bound separately: there is one cost, the walk,
and the only lever is **how many commits are examined**.

The allocation-free ASCII matcher was measured because it looked like the obvious
optimisation. It is ~5% faster and wrong for every non-English name in the
repository, so it was not taken.

**A page, end to end** (committed as `every_filter_costs_what_the_walk_costs`):

| commits | branch | author (broad) | subject (broad) | file | all four | no match |
|---|---|---|---|---|---|---|
| 100 | 0.8 ms | 3.7 ms | 3.7 ms | 8.8 ms | 5.1 ms | 3.8 ms |
| 1 000 | 1.1 ms | 3.4 ms | 2.4 ms | 67.6 ms | 51.4 ms | 43.2 ms |
| 10 000 | 1.1 ms | 3.2 ms | 2.6 ms | 109.8 ms | 114.3 ms † | 114.7 ms † |
| **100 → 10 000** | **×1.4** | **×0.9** | **×0.7** | **×12.5** | **×22.4** † | **×30.2** † |

† stopped at the budget.

Two shapes in that table, and both are the point:

- A **broad** filter is flat. It fills a page of twenty-five out of the first few
  dozen commits and stops (99 examined at every size), so a repository a hundred
  times longer costs the same.
- A **narrow or absent** match grows until it hits the budget and then stops
  growing. The ×30 is the walk to two thousand; it does not become ×300 at a
  hundred thousand commits, because the budget is where it ends.

## Decision

### 1. One budget, on commits examined

`MAX_FILTER_SCAN = 2 000` commits per request, `PAGE = 25` results. A request
returns when it has a full page **or** when it has examined two thousand commits,
whichever happens first, and both outcomes carry a cursor.

One budget rather than four, because the measurement says there is one cost. The
same constant as ADR-0017's `MAX_SCAN`, for the same reason and at the same
measured price (~100 ms), so a reader who has learned one number has learned both.

### 2. "Nothing yet" and "nothing" are different sentences

This is the requirement the slice was built around, and it is where a search
surface most easily lies.

`ScanStopped::Budget { scanned, limit }` travels with every page, and the
interface renders it distinctly:

| what happened | what it says |
|---|---|
| examined 42, reached the end, found none | **No matching commits** — "Nothing in this history matches author Grace Hopper." |
| examined 2 000, budget spent, found none | **No match yet** — "Nothing matched in the 2 000 commits examined. There may be more further back." + **Keep looking** |
| examined 2 000, budget spent, found some | the matches, plus "Stopped after examining 2 000 commits, so this is what matched so far rather than everything that matches." |

Never "No results" for the second row. A rarely-committing author in a long
history is *exactly* the case the budget bites on, and it is exactly the case
where a confident "no" would be false.

### 3. A branch is a commit id, not a ref name

```rust
pub struct HistoryFilter {
    pub branch: Option<CommitId>,   // a tip Mira handed out
    pub author: Option<Term>,       // compared in Rust
    pub subject: Option<Term>,      // compared in Rust
    pub file: Option<FileSubject>,  // ADR-0017's identity, unchanged
}
```

`git.refs` returns `RefTip { kind, name, tip }`. The interface shows the **name**
and sends back the **tip**. So `main`, `refs/heads/main`, `HEAD`, `main..dev`,
`@{upstream}` and `--all` are all simply not values this field can hold — a wire
test asserts each one fails to deserialise.

This is the generalisation ADR-0017 was reaching for. *Anything the reader picks
from a list Mira produced can be sent back as the identity Mira gave it, rather
than as the label the reader read.* Commit id, change-set ordinal, file subject,
ref tip — four instances of one rule now.

### 4. Author and subject are `Term`s, and a `Term` never reaches Git

A `Term` validates as it deserialises, like `CommitId`: trimmed, non-empty, at
most 200 characters, single line, no control characters.

It is then compared **in Rust against a commit already in memory** —
`commit.author().name_bytes()`, `commit.summary_bytes()` — and never given to
libgit2, let alone to a shell. There is no `--author=` in this codebase, and a
guard test proves it by scanning for one.

The validation is therefore defence in depth rather than the wall itself. It
exists because a value that can hold a newline is a value somebody will eventually
put somewhere a newline matters.

### 5. Subject search is a case-insensitive substring, and says so

Defined, because "search" means five different things in five different tools:

- **Substring.** Not a glob, not a regular expression, not word-boundaried. `*`,
  `wid*get`, `^feat` and `feat.*rare` are searched for **literally** — tests
  assert each matches nothing in a repository that would match if it were a
  pattern.
- **Case-insensitive**, by Unicode `to_lowercase` on both sides. Not
  `eq_ignore_ascii_case`, which was measured and rejected in §the measurement.
- **The subject line only.** The body is not searched. A test asserts a phrase
  present only in a body does not match.
- **Trimmed**, by `Term` itself.

The field's own description says all of this in the interface, because a search
box whose rules live only in an ADR is a search box whose rules nobody knows.

### 6. A filtered page is a list, not a graph

The graph toggle is not offered while a filter is on. A filtered history is a set
of matches with holes in it, and lanes drawn between commits that are not adjacent
would be a picture of something that does not exist. Clearing the filters brings
the graph back.

### 7. The default view does not change, and does not pay

`history` and `graph` are untouched. `search` is a separate command, asked only
when something is actually narrowed — so the ordinary case still costs
twenty-five commits, and the costlier question is the one somebody asked for.

### 8. The file filter is ADR-0017's mechanism, entire

Same `FileSubject`, same `touched()`, same rename-following at the one commit
where a rename can hide, same `RenameLost` when a rename cannot be followed. The
cursor carries the file forward for the same reason a file trace's does: a rename
crossed mid-search changes which file the next page is about, and the new name is
expressed as a position rather than written down.

## Alternatives considered

**`--author` / `--grep` / pathspec, as Git spells them.** Rejected: it is four
strings from the frontend becoming four Git arguments, which is the thing
§5 exists to prevent. The measurement also shows it would buy nothing —
libgit2 would test the same predicates against the same loaded commits.

**A different budget per filter.** Rejected on the measurement: there is no cheap
filter to give a bigger budget to. One number is also one number to keep honest.

**Search as you type.** Rejected: a bounded walk per keystroke is a bounded walk
per keystroke. The field is submitted, and the cost is attached to a deliberate
act.

**A commit-message index.** Would make repeat searches instant. Rejected for
ADR-0017's reason: it is a cache over a mutable repository, so it needs an
invalidation story, and what it speeds up is a thing people do occasionally and
on purpose.

**Reporting "no results" and letting the reader page on.** Rejected outright.
It is the specific lie this slice exists to not tell.

**Free-text author entry.** Rejected: it puts a string on the wire whose only
purpose is to be matched against names Mira could have offered instead. The menu
is bounded by the same budget and says so — "authors of the last 2 000 commits,
somebody further back may be missing" — which is a smaller and more honest gap
than an unvalidated field.

## Consequences

**Good.** A broad filter is flat across a hundred-fold repository (×0.7 to ×1.4).
A narrow one plateaus at the budget rather than growing. No ref name, path,
pathspec, glob or pattern crosses the IPC boundary, so §5 rules 7 and 8 survive a
feature made of four search strings. Every partial answer says it is partial, in
words, with a way to continue.

**Costs.** A rare match in a long history takes several presses of **Keep
looking**, each one a bounded walk — visible, repeated work instead of an
invisible wait. The author menu is only as deep as one budget, so a filter cannot
offer somebody it has not seen. And there are now two constants (`MAX_SCAN`,
`MAX_FILTER_SCAN`) that are the same number for the same reason, which somebody
will one day have to be told is not a coincidence.

**Bounded.** One walk, one budget, one page size — and the benchmark that
justifies the number is a committed test, including the per-predicate table that
says why there is only one of it.
