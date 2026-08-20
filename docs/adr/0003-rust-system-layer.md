# ADR-0003 — Rust for the system layer

**Status:** Accepted · 2026-08-19

## Context

Most of Mira's value is system inspection: enumerating listening sockets, walking
processes, reading Git repositories, talking to D-Bus and WinRT, watching filesystems,
and terminating processes safely. This code runs continuously, handles untrusted input
(filenames, repository content), and holds the app's most dangerous capabilities.

## Decision

**Rust**, stable toolchain, 2021 edition, for everything below the webview. Pinned via
`rust-toolchain.toml`.

## Alternatives considered

**Rust was largely settled by [ADR-0001](0001-tauri-2.md)** — Tauri's host process is
Rust — so the real question was how much logic belongs there versus in the frontend.

**Thin Rust, thick JavaScript** (Rust as a syscall shim, logic in the UI). Rejected: it
would put path-containment checks, argv construction, and termination guards in the least
trustworthy layer, and it would make the domain untestable without a webview.

**Rust plus a Node sidecar** for ecosystem convenience. Rejected: a second runtime to
ship, update, and secure, for libraries we do not need.

**C++ / Go for the system layer.** Both plausible in isolation, neither compatible with a
Tauri host without FFI overhead that buys nothing.

## Consequences

**Good.** Memory safety in the code that parses untrusted input. The type system carries
domain invariants — `ProjectId` cannot be confused with `Pid`, a `CapabilityStatus` must
be matched exhaustively. Excellent crates exist for every service (sysinfo, git2, zbus,
windows, objc2, notify). Domain crates test with `cargo test`, with no app harness.

**Bad.** Compile times slow the inner loop (mitigated by workspace splitting and
`cargo check`). Contributors who know React may not know Rust — mitigated by keeping
platform quirks inside `mira-platform` and labelling `good first issue` on both sides of
the boundary. Async Rust adds ceremony; we confine `async` to genuine I/O waits and use
`spawn_blocking` for syscall-heavy work rather than making everything async.

**Rule that follows.** No crate in the workspace depends on `tauri` except `src-tauri`.
That is what keeps the system layer a library rather than an app fragment.
