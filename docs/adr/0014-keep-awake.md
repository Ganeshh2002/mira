# ADR-0014 — Keep Awake: a power request, never simulated activity

**Status:** Accepted · 2026-08-21

## Context

People ask a machine to stay awake for ordinary reasons: a presentation is running,
a build is compiling, a download is finishing, a dashboard is being watched, a long
document is being read. Every operating system Mira supports has a public interface
for exactly this, and every one of them exists because applications legitimately
need it.

There is also a second, popular way to keep a machine awake: **pretend somebody is
using it.** Move the pointer a pixel every thirty seconds; press and release a key
that does nothing. Tools that do this are widely available and are marketed with
the same words as the honest ones.

Mira is a companion that sits on a developer's machine all day, reads their
repositories, and reports what is running. It is exactly the sort of program that
would be trusted with such a thing, and exactly the sort of program that must not
take it. So the decision has to be recorded rather than assumed — the difference
between the two mechanisms is invisible in a feature list and total in what it
means.

The immediate need is Slice 5a's small companion capability: a Keep Awake control
in the tray, off by default, with a fixed set of durations.

## Decision

**Keep Awake holds an operating-system power request, through a public API, for
as long as the person chose. It never synthesises input, and never runs a
program.**

Four commitments, each with a mechanism behind it rather than a promise.

### 1. No simulated activity, ever

Mira does not post keyboard events, move the pointer, warp the cursor, or
manufacture activity of any kind — not for this feature, and not for any other.
A guard test scans every source file for the APIs that would do it (`CGEventPost`,
`SendInput`, `XTestFakeKeyEvent`, `uinput`, and the rest) and fails the build if
one appears. A second guard asserts that no crate or npm package capable of it —
`enigo`, `rdev`, `robotjs`, `nut-js` — is anywhere in the dependency tree.

This is not squeamishness. Synthetic input **defeats idle detection everywhere at
once**: the screen lock, the session timer, the away status somebody's colleague is
reading, and any tooling an employer or a team relies on. It is indistinguishable,
at the operating-system level, from what a malicious program does. A power request
is the opposite: it is a specific, declared, revocable statement that *this
application is doing something for the person in front of it*, visible to the
machine's own tooling.

The practical consequence, stated plainly: **an idle screen stays idle.** Keep
Awake stops the machine falling asleep; it does not make anything believe the
person is at the keyboard, and it is not a way around a policy.

### 2. Native interfaces, no subprocess

No `caffeinate`, no `powercfg`, no `systemd-inhibit`, no `pmset`, no `xset`.
`security-and-privacy.md` §5 rule 1 says Mira starts no child process outside one
reviewed function in `mira-platform`, and this feature does not need an exception —
every platform exposes the capability as an API.

On macOS that API is `NSProcessInfo`'s activity interface, asked for
`NSActivityIdleSystemSleepDisabled | NSActivityIdleDisplaySleepDisabled`: idle
sleep of the machine and of the screen. Closing the lid still sleeps the machine,
because closing the lid is an instruction rather than idleness, and an application
does not get to overrule it.

The **block** form of that API is used —
`performActivityWithOptions:reason:usingBlock:` — rather than the
`beginActivity` / `endActivity` pair. `endActivity:` is an `unsafe` binding in
`objc2` (it takes an activity token whose type it cannot check), and the block
form is safe, so `mira-platform` keeps `#![forbid(unsafe_code)]`. The block is a
thread parked on a channel for as long as the lock is held. It is not a timer and
it does not poll; it blocks, and it ends when the channel closes.

A guard test forbids every one of the named programs above from appearing in any
source file, including in a reason string: Mira neither runs them nor tells you to.

### 3. Windows and Linux are honestly Unavailable in this slice

`SetThreadExecutionState` on Windows and `org.freedesktop.login1`'s `Inhibit` on
Linux are the right mechanisms, and neither is built yet. The first needs an
`unsafe` FFI call in a crate that currently forbids one; the second needs a D-Bus
client that is not in the tree.

Rather than ship something that half-works, `Capability::KeepAwake` resolves
`Unavailable` on both, with a true reason and a fallback naming the platform's own
power settings. The control is not hidden and is not shown as broken: the reason is
what the tray and Settings display (`platform-abstraction.md` §2 rule 4, and
roadmap rule 4 — a capability is declared honestly in the slice that introduces
it). The matrix row says so, and so does the README.

This is the parity rule applied to a feature that could easily have been faked, and
it is the whole reason the capability model exists.

### 4. Nothing survives the process

A lock is an operating-system request owned by this process, plus a timestamp in
memory. There is no table, no column, and no file — a guard test fails the build if
a migration ever mentions one. So:

- Quitting Mira releases the request. `shutdown` does it first, before the database
  checkpoint, so a failure there cannot leave a machine pinned awake.
- Restarting Mira starts **off**, whatever was held before. A power setting that
  came back by itself after a reboot would be a surprise, and surprises about how a
  machine behaves are the worst kind.
- A crash releases it too, because the request belongs to a process that no longer
  exists.

The span is one of four words — off, thirty minutes, an hour, until turned off —
and not a number of minutes. The interface therefore cannot ask for a week: there
is no parameter through which it could say so. A span with an end is armed with a
single one-shot [`Deadline`](../../crates/mira-scheduler/src/lib.rs) owned by
`mira-scheduler`, because that crate owns every clock in Mira
([ADR-0011](0011-one-scheduler.md)); it is one sleep and one release, not a poll,
and it is deliberately *not* behind the observers' gate — a lock has to end at the
time the person chose even if every window is hidden.

## Alternatives considered

**Simulated input.** Rejected on the reasoning in decision 1. It is the easy
implementation, it works identically on all three platforms without a single
platform-specific line, and it is the one thing this ADR exists to refuse.

**Shelling out to `caffeinate` / `powercfg` / `systemd-inhibit`.** Rejected. It
would ship all three platforms in an afternoon and it would put the first child
process in Mira's history behind a feature that does not need one — reopening the
command-injection surface that `security-and-privacy.md` §5 is built on not having.
It also inherits the lifetime problem: a subprocess outliving Mira is a machine
kept awake by a program nobody can see.

**A third-party cross-platform crate.** Attractive: one dependency, three
platforms, a safe API. Rejected for now on supply chain. This is a small,
well-understood capability in a security-conscious product, and it is a poor trade
to take an unaudited dependency that reaches the power subsystem in order to save a
few dozen lines of `objc2`. It stays on the table for the Windows and Linux
implementations, where the alternative is unsafe FFI and a D-Bus client, and that
comparison is worth making explicitly when the work is done.

**Waiting until all three platforms could ship at once.** Rejected because the
capability model exists precisely so a feature can be honest about where it works.
Holding macOS back would not make Windows or Linux better; it would just mean
nobody has it.

## Consequences

**Good.** A real capability with no ethical footnote, and one that can be described
to a security reviewer in a sentence. No subprocess, no unsafe code, no new
dependency. The lifecycle is structural rather than disciplined: nothing survives a
restart because there is nowhere to write it down, and one request exists at a time
because there is one place that takes one out.

**Costs.** Windows and Linux users see a reason instead of a control until the
native implementations land, and the reason is Mira's honesty rather than their
platform's limitation — which is a cost worth naming rather than softening. macOS
carries a thread while a lock is held, which is one thread doing nothing.

**Bounded.** The mechanism lives in one file (`mira-platform/src/inhibit.rs`), the
policy in another (`mira-platform/src/awake.rs`), and the lifecycle in a third
(`src-tauri/src/awake.rs`). Adding Windows or Linux is a new arm in `inhibit.rs`, a
changed row in the capability matrix, and nothing else.

**Not revisited without an ADR.** Decision 1 in particular. If a future release
wants to keep a machine awake by any means other than asking the operating system,
that is a new decision, recorded here, with the reasoning written down — not a
pull request.
