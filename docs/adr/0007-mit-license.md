# ADR-0007 — MIT licence

**Status:** Accepted · 2026-08-19

## Context

Mira is free and open source, wants contributors and packagers, and links a dependency
tree of permissively licensed Rust and JavaScript crates. Aviora also intends to build
Astra as a separate product, so the licence must not create ambiguity about a
commercially licensed neighbour that shares no code.

## Decision

**MIT.** Copyright "The Aviora Mira contributors". No CLA; contributions are under the
same licence by the standard inbound-equals-outbound convention.

## Alternatives considered

**Apache-2.0.** Better: an explicit patent grant and a contribution clause. Rejected only
on ecosystem convention — MIT is the shortest, most-recognised licence in this space, and
Mira holds no patentable invention. Reasonable people would choose Apache-2.0 here; the
difference is small enough that familiarity decided it. (Dual MIT/Apache-2.0, the Rust
norm, was considered and rejected as unnecessary ceremony for an application rather than a
library.)

**GPL-3.0.** Would ensure derivatives stay open — appealing for a privacy-first tool. But
it complicates distribution (app stores, some corporate environments), discourages the
casual contributor, and would raise questions about a proprietary sibling product it
shares no code with. Rejected: Mira's protection against enclosure is that it is small,
free, and easy to fork, not that a licence forbids it.

**AGPL-3.0.** Aimed at network services. Mira is a desktop app that talks to nothing.
Irrelevant.

**Open-core / BSL.** Would contradict "Free · Open source" in the product definition.

## Consequences

**Good.** Maximum compatibility with the dependency tree; no friction for packagers,
distros, or corporate users; the lowest possible barrier to a first contribution; no CLA
process to administer.

**Bad.** Someone may fork Mira, close their fork, and sell it. Accepted — a fork of a free
tool competes with a free tool, and the open version keeps the contributors.

**Housekeeping.** `cargo deny` enforces licence compatibility on every dependency; the
`LICENSE` file is the single authority, restated in the README.
