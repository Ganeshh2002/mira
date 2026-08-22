# ADR-0019 — Application preferences: a choice is an identity, never a program

**Status:** Accepted · 2026-08-21

## Context

ADR-0013 gave Mira the ability to open a workspace in a real application, and
drew the boundary that made it safe: the interface asks for a **kind**, and
everything else — which directory, which application, which argv — is resolved
underneath from Mira's database and a table compiled into the binary.

It also wrote down the cost of that boundary, in the Consequences section:

> The candidate table is now a maintenance surface with opinions in it — which
> terminals take which flag, which editors are windowed. **Wrong rows are
> user-visible, and there is no way for a user to correct one; that is the deal
> struck above.**

That deal has a real victim. Mira's macOS editor list starts with Visual Studio
Code; somebody with both VS Code and Zed installed gets VS Code, forever, with no
way to say otherwise. The list's order is a guess about what people want, and it
is being applied as though it were an answer.

This slice pays that debt back, and the whole difficulty is doing so without
reopening the door ADR-0013 closed. The obvious implementation of "let me choose
my editor" is a settings field holding a program, which is
`launch_application(command_string)` with a nicer label — the alternative
ADR-0013 explicitly rejected. Four of Mira's docs say some version of *no
executable path from the frontend*; a feature whose entire purpose is to let a
person pick an application needs an answer, not an exemption.

## The measurement

Discovery has always short-circuited: "is there an editor here" stops at the
first row that is present. A chooser cannot — it has to ask about **every** row
to say which ones are installed. That is a different amount of work, and how much
more decides whether it can be probed on demand or needs something kept between
requests (which would need a clock to invalidate it, which ADR-0011 does not
allow here).

Measured on a machine with 63 entries on `PATH`:

| what is asked | before | after |
|---|---|---|
| the panel's question — first present, three kinds | 0.006 ms | 0.007 ms |
| the whole macOS catalogue, 20 rows | 0.566 ms | **0.208 ms** |
| the whole Linux catalogue, 19 rows | 3.396 ms | **1.015 ms** |
| the whole Windows catalogue, 12 rows | 2.465 ms | 3.193 ms |

Two findings, and the second was a surprise.

**A whole catalogue is cheap enough to ask for every time.** Even the worst list
is a millisecond or three. Nothing needs to be remembered, so nothing needs to be
invalidated, so no clock is involved.

**The dominant cost is a `PATH` miss, and most of it was being paid for nothing.**
`on_path` tried four spellings of every program — `.exe`, `.cmd`, `.bat` and the
bare name — because Windows spells a program four ways. Off Windows there is only
one spelling, so three quarters of the stats were asking about files that cannot
exist. Making the probe take the `Os` it is answering for cut macOS by 2.7× and
Linux by 3.3×, in the code path this slice makes twenty times hotter.

## Decision

### 1. A choice is a catalogue id

Every row in `applications.rs` gains a stable `id` — `vscode`, `iterm`,
`ghostty` — and that is what a preference stores.

```rust
pub struct AppPreference {
    pub kind: AppKind,
    pub application: AppId,
}
```

`AppId` validates as it deserialises, like `CommitId` and `Term` before it: one
to thirty-two characters of lower-case ASCII, digits and hyphens. So
`/usr/bin/code`, `code --wait`, `Visual Studio Code` and `../../etc/passwd` are
not values the type can hold.

But **validation is not the wall.** `code` and `sh` are perfectly good slugs. The
wall is that the only thing an id can be turned into is a row of the table
compiled into the binary:

```rust
pub fn find(os: Os, kind: AppKind, id: &AppId) -> Option<&'static Candidate>
```

An id that names no row resolves to nothing — and `workspaces.prefer` looks it up
*before storing it*, so an unknown id is refused at the moment of choosing rather
than accepted and then failing at every launch. The database can only ever hold
identities Mira itself offered.

This is the same move made four times now, and it is worth naming as a pattern:
**anything picked from a list Mira produced goes back as the identity Mira gave
it, never as the label that was read.** A commit id, a change-set ordinal, a file
subject, a ref tip — and now an application id.

### 2. The id is the same on every platform

`vscode` is `/Applications/Visual Studio Code.app` on macOS, `code` on Windows
and Linux. A workspace carried to another machine keeps its choice and resolves
it against that machine's list; a choice with no row on this platform is its own
state rather than a silent reset (`ChosenApp::Unknown`).

### 3. A choice is obeyed or refused — never substituted

Five states, because there are five different sentences:

| state | what it means | what the interface says |
|---|---|---|
| `Automatic` | nothing chosen | "Automatic · Ghostty" — and what that is today |
| `Ready` | chosen, here, openable | the name, on the button |
| `Missing` | chosen, not on this machine | "Zed is not on this machine. Nothing else will be opened — choose another." |
| `NotOpenable` | here, and not something a folder opens in | "Neovim is here, and is not something Mira can open a folder in." |
| `Unknown` | not in this platform's list at all | "This workspace uses nova, which Mira does not look for on this platform." |

`Missing` is emphatically **not** a reason to open something else.
`security-and-privacy.md` §6 forbids silently substituting an unrelated
application, and a stated choice makes a substitution worse rather than more
excusable: the person would never find out their editor was uninstalled. A guard
test asserts every failing branch of `plan` ends in a `return Err`, and that none
of the shapes somebody would reach for to make it "just work"
(`unwrap_or_else(|| automatic …)`) is present — proven able to fail by injection.

The interface asks `workspaces.chosen` **before** offering a button, so a missing
choice is a sentence on the row rather than an error after a click.

### 4. The empty tables in `0001_init.sql` stay empty

`data-model.md` §3.4 sketched `applications` + `app_preferences`, and the baseline
migration created both. `applications.program` is documented as *"an executable
path, bundle id, or .desktop id"*, and `app_preferences` points at a row in it.

Those two tables are **not** used, and `workspace_app_preferences` exists instead:

```sql
CREATE TABLE workspace_app_preferences (
  workspace_id   INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  kind           TEXT    NOT NULL CHECK (kind IN ('editor','terminal','browser')),
  application_id TEXT    NOT NULL CHECK (… GLOB '[a-z0-9-]*' …),
  chosen_at      INTEGER NOT NULL,
  PRIMARY KEY (workspace_id, kind)
);
```

A stored `program` column is a place for an executable path to live, and a
preference pointing at a row in a mutable registry would make "which application"
a piece of data rather than an identity. A guard test asserts no code reads or
writes either baseline table, so their emptiness is enforced rather than an
oversight waiting for somebody to fill it in.

The primary key is what makes "per workspace" a property of the schema: there is
no row two workspaces could both read.

### 5. A chosen browser is opened; an unchosen one still goes to the desktop

ADR-0013 sends web addresses to `LaunchMethod::DefaultHandler`, because *which
browser you use* is a choice already made in the operating system, and overruling
it with the first row of a list is exactly the substitution §6 forbids.

That reasoning is unchanged when nothing is chosen. When a workspace **does**
choose a browser, that is the same choice said to Mira directly, and it is obeyed.
`live.open_service` passes `None` deliberately: it is the machine's service list
rather than a workspace's, so there is no workspace whose choice could apply.

### 6. Availability is read fresh, every time

No cache, no observer, no clock. The measurement is why this is affordable, and a
guard asserts `applications.rs` contains no `OnceLock`, `Mutex`, or anything
called `cache` — a remembered answer would need something to invalidate it, and
the only honest thing to invalidate it with is the one clock ADR-0011 reserves
for the scheduler.

### 7. The interface never writes an id down

A guard asserts no frontend source file contains a catalogue id **as a whole
string literal** (matched exactly rather than as a substring: `cursor` is an
editor and a CSS property, and `arc` is inside `search`). The interface handles
ids constantly and originates none of them.

## Alternatives considered

**A path or command field in Settings.** The obvious feature, and the reason this
ADR exists. Rejected for ADR-0013's reason, which has not weakened: it is
`launch_application(command_string)` with a settings screen in front of it.

**Reordering the candidate list per user.** "Move Zed above VS Code." Rejected:
it makes the answer depend on a stored ordering that has to stay in step with a
compiled list, and it says nothing at all about what happens when the top entry
is uninstalled. A choice is a smaller idea and a more honest one.

**Falling back to automatic when a choice goes missing.** Tempting, and it is the
thing most tools do. Rejected outright — see §3. A guard exists specifically to
stop somebody adding it later in a helpful mood.

**Populating the `applications` table by detection.** What §3.4 anticipated.
Rejected: a detected row is a stored `program` string, which is the shape the
whole boundary exists to avoid, and it buys nothing the compiled table does not
already give.

**Per-project as well as per-workspace preferences.** `app_preferences` sketched
both, with `workspace_id NULL` meaning a project-level default. Rejected as
premature: nothing has asked for a default that spans workspaces, and adding a
second scope means deciding what happens when they disagree. The table is keyed
by workspace, so a project scope is additive later.

## Consequences

**Good.** The debt ADR-0013 wrote down is paid: a wrong row in the table is no
longer permanent for the person it is wrong for. The boundary is unchanged — no
path, no program, no argv, no command crosses it, and there is still no column
anywhere a program could be written into. A choice that stops being true says so,
by name, and opens nothing else. Discovery got 2.7–3.3× cheaper on the way past.

**Costs.** The catalogue is now a public vocabulary: an id, once shipped, is
stored in people's databases, so renaming one silently drops their choice.
(`ChosenApp::Unknown` means it drops *visibly*, which is the mitigation, not a
fix.) The `id` column is one more thing to get right when adding a row, and a
duplicate would make a choice ambiguous — asserted by a test. And a workspace that
chose an application Mira later removes from the table lands in `Unknown`, which
is honest but not fixable from the interface.

**Bounded.** Deleting `AppChooser.tsx`, the three commands, and
`workspace_app_preferences` returns the product to ADR-0013 exactly: automatic for
everyone, and the list's order deciding again.
