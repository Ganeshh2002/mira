# ADR-0006 — No account, no mandatory cloud, no telemetry

**Status:** Accepted · 2026-08-19

## Context

Mira reads the shape of a developer's working life: which projects they have, where the
code lives, what is running, which hosts they SSH to, when they were at the keyboard.
That is unusually sensitive for a utility. It is also a free open-source tool with no
business model to fund a service — and with Aviora Astra planned as a separate hosted
product, there will be pressure to make Mira an on-ramp to it.

## Decision

Mira has **no account system, no mandatory cloud service, and no telemetry**.

1. No `users`, `organizations`, or `subscriptions` tables; no login surface at all.
2. A fresh install makes **zero** outbound connections. Update checking is opt-in and
   unanswered until the user answers it.
3. No analytics of any kind — not anonymised, not aggregated, not crash reports. CI fails
   if a known analytics dependency enters the tree.
4. No feature may require a network connection to work.
5. Astra integration, if built, is an optional module, off by default, deletable without
   affecting anything else ([ADR-0008](0008-modular-architecture.md)).

Any future telemetry would have to be opt-**in**, off by default, fully documented field
by field, inspectable before sending, and permanently disableable. There is no plan to
add it.

## Alternatives considered

**Optional account for sync.** Genuinely useful — settings across machines. Rejected for
now: it requires a server, an auth system, a privacy policy, and a funding model, and it
would create pressure to make sync the default. Config export/import solves 80% of it with
none of the cost.

**Anonymous opt-out telemetry.** The industry norm, and it would answer real questions
(which platforms, which features get used). Rejected because "anonymous" telemetry from a
tool that knows your project names and SSH hosts is a claim users would have to take on
trust — and the trust is worth more than the data. Issues and discussions are a slower,
honest substitute.

**Crash reporting only.** Narrower, still an automatic upload from a privileged local tool.
Rejected: Mira writes a local log the user can attach to an issue themselves.

**Mandatory Astra sign-in.** Would make Mira an ad for another product and violate the
product definition's core promise.

## Consequences

**Good.** The privacy promise is simple enough to verify by reading the code, and the
threat model shrinks to local concerns. No server to run, secure, or fund. No auth
vulnerabilities, because there is no auth. Mira works on air-gapped machines.

**Bad.** We are blind: no usage data, no adoption numbers, no automatic crash reports.
Prioritisation runs on issues and conversation, which is slower and biased toward vocal
users. No cross-machine sync, which some people will want. If Astra ever needs an install
base, Mira will not supply one automatically — by design.

**Accepted.** These costs are the price of the product's central promise. Reversing this
requires a superseding ADR and a major version.
