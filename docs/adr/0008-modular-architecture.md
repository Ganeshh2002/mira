# ADR-0008 — Modular crates with a one-way dependency rule

**Status:** Accepted · 2026-08-19

## Context

Mira spans a dozen loosely related domains — Git, ports, processes, files, SSH, Docker,
media, system — across three operating systems, with an open-source contributor base and
a planned optional Astra integration that must never become load-bearing. The obvious
failure mode is a single `src-tauri` crate where every command handler grows business
logic, platform quirks leak everywhere, and nothing can be tested without launching the
app.

## Decision

A **Cargo workspace of small crates**, each owning one domain behind an interface, with a
strictly one-way dependency rule:

```
src-tauri → domain crates → services → mira-platform → mira-core
                  ↓                                        ↑
               mira-db ─────────────────────────────────────┘
```

1. `mira-core` depends on nothing in the workspace.
2. No crate except `src-tauri` depends on `tauri`.
3. Domain and service crates never depend on each other sideways; cross-domain work is
   composed in `src-tauri/commands`.
4. Services are traits with a concrete implementation and a fake.
5. CI fails the build when these are violated.

The same boundary serves plugins and Astra: an integration consumes public interfaces
only, and deleting it must leave a compiling, fully functional Mira.

## Alternatives considered

**Single crate, modules only.** Faster to start and much faster to compile. Rejected
because module boundaries are advisory — nothing stops a `use crate::db` appearing inside
the Git code — and because rule 2 becomes unenforceable, which is the rule that keeps the
domain testable.

**Two crates** (`mira-core`, `mira-app`). A middle ground that keeps most of the coupling
problems while adding little enforcement.

**Full hexagonal architecture** with ports and adapters throughout. More ceremony than a
desktop utility earns; we take the parts that pay for themselves (traits at the I/O edge,
a pure core) and skip the rest.

**Microservice-style separate processes.** Absurd at this scale.

## Consequences

**Good.** Each crate is small enough to hold in mind — which improves both human review
and machine-assisted editing. Domain logic tests with `cargo test`, no webview. Swapping
an implementation (git2 → gitoxide, one port crate for another) is contained. A
contributor can work on `mira-ports` without understanding the UI. Astra deletability is
structural rather than a promise.

**Bad.** More `Cargo.toml` files, more boilerplate to add a feature (crate, trait, fake,
commands, generated types). Incremental compilation across a workspace is slower to
configure well. Over-splitting is a real hazard — a crate per *file* would be worse than
one crate.

**Rule of thumb.** A crate exists when it has a distinct interface and could plausibly be
faked or replaced. `mira-automation` is therefore **not** created before 0.6+:
building an empty crate for an undesigned feature is speculative architecture, and the
dependency rules already make adding it cheap.
