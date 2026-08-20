# ADR-0005 — Runtime capability model for cross-platform behaviour

**Status:** Accepted · 2026-08-19

## Context

Research during design produced hard facts, not preferences:

- Global shortcuts cannot work on Wayland — no cross-compositor protocol exists, and
  Tauri's X11 implementation is disabled there to avoid a libX11 crash.
- Tray **click** events never fire on Linux (a libappindicator limitation), while macOS
  and Windows deliver them.
- Now-playing metadata has public APIs on Windows (GSMTC) and Linux (MPRIS), and **none**
  on macOS; since macOS 15.4 the private MediaRemote route is entitlement-gated.
- Lock detection is reliable on macOS and Windows, and only on logind-based Linux.
- Process working directory is routinely unavailable on Windows, which weakens port
  attribution there.

A compile-time `#[cfg(target_os)]` model cannot express any of this: the same Linux binary
runs on Wayland and X11, with and without logind, with and without a tray host.

## Decision

Every OS-touching feature is a **capability with a runtime status** —
`Full` / `Degraded { reason }` / `Unavailable { reason, fallback }` — resolved on the
actual machine at startup and re-resolved on relevant events. Implementations live behind
traits in `mira-platform`. `#[cfg(target_os)]` is banned outside that crate, enforced by a
CI source scan. The UI asks about capabilities and never about operating systems.

The reason strings are user-facing copy. **Do not fake parity** is a product rule with a
mechanism behind it.

## Alternatives considered

**Compile-time cfg throughout.** Simple, and wrong: it cannot distinguish Wayland from
X11, or logind from not, and it scatters OS knowledge across the codebase.

**Lowest common denominator** — ship only what all three do identically. Would delete
global shortcuts, tray interaction, media, and lock detection: most of what makes Mira
ambient.

**Silent degradation** — features that quietly do nothing where unsupported. The worst
option: users conclude the app is broken, and file bugs that cannot be fixed.

**Platform-specific builds** with different feature sets. Fragments documentation, support,
and testing, and still cannot handle a runtime split like Wayland vs X11 inside one build.

## Consequences

**Good.** Honest UI: a disabled control always states why. Runtime detection handles splits
compile-time flags cannot see. Fake implementations of each trait make domain logic
testable without an OS. Adding a platform means implementing traits and filling a matrix
column — no changes elsewhere.

**Bad.** Every feature touching the OS carries a status and a piece of copy; that is real
work per feature. The capability matrix must be maintained or it becomes a lie — so it is
duplicated in the README and each `Full` claim requires a platform test.

**Consequence accepted deliberately.** macOS ships without media detection. We will not
link a private framework, defeat SIP, or piggy-back on entitled system binaries to close a
gap Apple has closed on purpose.
