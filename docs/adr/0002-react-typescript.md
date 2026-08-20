# ADR-0002 — React 19 + TypeScript for the UI

**Status:** Accepted · 2026-08-19

## Context

The UI is a small number of dense, state-driven views: lists that update from push
events, an overlay, a command palette. It is not animation-heavy or graphics-heavy. The
project is open source and wants contributors, and it must not be rewritten in two years
because a framework moved on.

## Decision

**React 19 with TypeScript in strict mode**, built by Vite. Types crossing the IPC
boundary are generated from Rust (`ts-rs`), never hand-written.

## Alternatives considered

**Svelte 5.** Smaller output, less ceremony, genuinely pleasant for this kind of UI. Lost
on contributor reach: React is the default skill of the people most likely to send a PR
to a developer tool. For a project whose bottleneck is contributors rather than
kilobytes, that matters more than bundle size — and in a Tauri app the bundle is local
anyway, so the size argument nearly vanishes.

**Solid.** Excellent performance and a React-shaped API. Rejected on ecosystem depth for
headless component primitives and testing, and a smaller contributor pool.

**Vue.** Strong and viable; React's larger pool and our own familiarity decided it.

**No framework** (vanilla + web components). Attractive for a small UI, and rejected
because the command palette, keyboard model, and list virtualisation are exactly the
places where hand-rolled state management goes wrong.

**JavaScript without TypeScript.** Rejected outright: the IPC surface is the seam where
mistakes are silent and expensive, and generated types make it impossible to drift.

## Consequences

**Good.** Deep ecosystem for the pieces we need (TanStack Query, headless primitives,
Testing Library). Contributors can start immediately. Generated types make a Rust command
signature change a compile error in the UI rather than a runtime bug.

**Bad.** React ships more runtime than Svelte or Solid, and re-render discipline is on us
— for lists refreshing every 5 seconds we must memoise deliberately and keep events as
notifications rather than payloads. Strict TypeScript slows the first hour of a feature
and saves the first week of maintenance.

**Constraint accepted.** No `any` on the IPC boundary — CI rejects it. Generated types are
committed so a reviewer sees the contract change in the diff.
