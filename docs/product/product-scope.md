# Aviora Mira — Product Scope

Status: **pre-implementation. Binding.**

Read [product-definition.md](product-definition.md) first — it defines what Mira is.
This document defines *when* each part of it gets built, and it is binding on
[prd.md](prd.md) and [roadmap.md](../implementation/roadmap.md). Where any of them
disagree about phase membership, this document wins.

---

## 0. Why this document exists

Without it, "MVP" drifts. A roadmap that maps releases to slices answers *what ships
together*; it does not answer *what we are allowed to not build yet*. This document
answers that, once, so the answer stops being renegotiated.

---

## 1. The MVP line

> **0.1 — Core Companion is the first public MVP. Everything from 0.2 onward is
> post-MVP.**

Four rules follow from that, and they are the whole point of the document:

1. **No post-MVP feature blocks 0.1.** If a thing is not in the 0.1 list in §3, its
   absence is not a release blocker. It does not get "one more week".
2. **No post-MVP feature is partially built during 0.1.** No table, no trait, no
   interface, no abstraction added because it will "make 0.2 easier". This restates
   roadmap rule 7 and is the failure mode this document exists to prevent.
3. **0.1 must be usable on its own.** It is a product someone installs and keeps, not a
   preview of a later product. If the 0.1 list stops meeting that bar, the list changes
   — not the bar.
4. **Cutting a 0.1 feature changes this document first.** In the same pull request, not
   afterwards.

---

## 2. Vocabulary

These terms are used with exactly these meanings across every document in the repository.

| Term | Meaning |
|---|---|
| **MVP** | The 0.1 release. Nothing else. Never a synonym for "the 0.x line". |
| **The 0.x line** | 0.1 through 0.6+ — every milestone before 1.0. |
| **Post-MVP** | 0.2 through 0.6+. Planned and scheduled, but not required for the first public release. |
| **0.6+** | The 0.6 milestone and any further 0.x milestones it turns out to need. Deliberately open-ended: automation is gated on a trust model, not a date. |
| **1.0.0** | Tagged after the 0.x line has shipped, been used in earnest, and the command surface and schema have settled. Not a feature milestone. |
| **Future** | Directionally accepted, deliberately undesigned, not assigned to a phase. |
| **Anti-scope** | Will not be built. Stated permanently so it stops being proposed. |

**Retired: "V1.x".** The tier no longer exists. Everything that carried the label has
been assigned to a numbered phase in §3 or moved to §5. If you find "V1.x" in a
document, it is stale — except in `architecture-review.md`, which is a dated record and
is left as written.

---

## 3. The locked phases

Phase membership is locked. Feature numbers refer to [prd.md](prd.md) and are **stable
identifiers, not reading order**; slice numbers refer to
[roadmap.md](../implementation/roadmap.md) and are likewise stable.

### 0.1 — Core Companion · **the MVP**

*Slices 0, 1, 2, 3, 5a.* Answers "what branch, what's dirty, what's on :3000, open it"
for several projects at once, from a keystroke.

| Capability | PRD | Slice |
|---|---|---|
| Create/manage projects | 1 | 1 |
| Multiple projects | 1 | 1 |
| Git status | 3 | 1 |
| Last commit | 4 | 1 |
| Branch information | 5 | 1 |
| Basic Git graph | 6 | 5a |
| Port detection | 7 | 2 |
| Process information | 8 | 2 |
| Open localhost | 12 | 3 |
| Open project in editor | 11 | 3 |
| Open terminal | 10 | 3 |
| Tray / menu bar | 21 | 1 |
| Global shortcut | 20 | 1 |

**"Basic Git graph" means basic.** A paginated commit list with badged refs and commit
detail. No lane assignment, no branch lines, no merge rendering. The lane graph is 0.2.
This split is deliberate: lane layout is the fiddliest UI in the product and it is not
worth holding the first release for.

### 0.2 — Workspace

*Slices 4, 5b, 11.* Several projects stop being a list and become a place you work.

| Capability | PRD | Slice |
|---|---|---|
| Workspace / session model | 2 | 4 |
| App groups | 23 *(shape only)* | 4 |
| Workspace restoration | 24 *(shape only)* | 11 |
| Project-specific context | 2 | 4 |
| Better Git visualization | 6 *(extended)* | 5b |

Better Git visualization covers the lane graph, diff view for a commit, file history,
and graph filtering. Still read-only — no rebase or merge UI, ever.

### 0.3 — Shelf

*Slices 6, 7.* A place to put a file that is not yet a decision.

| Capability | PRD | Slice |
|---|---|---|
| Drag / drop files | 15 | 6 |
| Temporary file storage | 15 | 6 |
| Project-specific shelves | 15 | 6 |
| Quick Peek | 16 | 7 |

### 0.4 — System

*Slice 9.* Mira notices the state of the machine it lives on.

| Capability | PRD | Slice |
|---|---|---|
| CPU / RAM / battery | 17 | 9 |
| Network | 17 | 9 |
| Lock / session awareness | 19 | 9 |
| Media detection | 18 | 9 |
| Displays | 25 *(shape only)* | 9 |

### 0.5 — Personality

*Slice 10.* Mira stops looking like a dashboard.

| Capability | PRD | Slice |
|---|---|---|
| Cosmic / Sakura / etc. | 22 | 10 |
| Ambient effects | 22 *(extended)* | 10 |
| Workspace identities | 26 *(shape only)* | 10 |
| Music-reactive ambience | 27 *(shape only)* | 10 |

Every atmosphere obeys the same constraints as 0.5's predecessors: tokens plus one
canvas layer, never layout or spacing or the status language; AA contrast gate in CI;
under 2% idle CPU for the heaviest; `prefers-reduced-motion` stops everything. A
personality phase is not a licence to redecorate the information architecture.

### 0.6+ — Automation

*Slices 8, 12.* The phase that needs a trust model before it needs a schema.

| Capability | PRD | Slice |
|---|---|---|
| Contextual rules | — *(undesigned)* | 12 |
| SSH awareness | 13 | 8 |
| Docker awareness | 14 | 8 |
| Plugins | — *(undesigned)* | — |
| Astra integration | — *(undesigned)* | — |

SSH and Docker awareness were previously scheduled for 0.4 and keep their full
specifications; they moved here because they share the same property as the rest of
this phase — they are the read-only half of the surface an automation rule would act
on, and they are the two most sensitive integrations in the product.

**Nothing in 0.6+ starts before:** the MVP has shipped and been in real use; a written
trust model exists covering where rules come from, how they are reviewed, and what they
may do; and demand is evidenced by actual requests rather than assumption.

---

## 4. Known platform capability limitations

These are properties of the platforms, not defects in Mira. They are recorded here so
they are met as facts during planning rather than as surprises during a phase.

| Limitation | Affects | Status |
|---|---|---|
| **macOS exposes no public API for now-playing media.** The private framework that would provide it is not an option — see [ADR-0006](../adr/0006-no-account-no-cloud.md) and the `otool -L` guard test in CI. | Media detection (0.4), music-reactive ambience (0.5) | `Unavailable` on macOS with an honest reason string. Full on Windows (GSMTC) and Linux (MPRIS). |
| **Wayland does not permit global shortcut registration** the way X11 does. | Global shortcut (0.1) | `Degraded` — documented `mira --toggle` fallback. |
| **Windows cannot attribute a listening socket to a working directory** as reliably as Unix. | Process attribution (0.1) | `Degraded` — attribution by executable path, labelled as such. |

Music-reactive ambience is therefore designed to degrade to a non-reactive atmosphere
rather than to disappear. If Apple ships a public API for this, the capability is
revisited then — the constraint is Apple's, and it is allowed to change.

---

## 5. Accepted but unscheduled

Directionally agreed, no phase assigned. These were previously "V1.x" and are recorded
here so retiring that tier loses nothing.

- **Project health overview** — one card per project rolling up Git, ports, containers,
  disk, and last activity. Aggregation of existing signals only.
- **Notifications on watched conditions** — opt-in per condition. The capability exists
  from 0.1 only to report its own status.
- **Improved media integration** — playback controls where the OS permits. macOS
  remains `Unavailable`.
- **Platform-specific integrations** — macOS Shortcuts, Windows jump lists, Linux
  desktop actions. Additive only; no core flow may exist on one OS alone.
- **Git `fetch` behind an explicit setting**, **Docker start/stop**, **shelf-item notes**
  — plausible, not promised.

---

## 6. Changing this document

Moving a capability between phases is a decision, not a detail. It requires:

1. An explicit change here, stating what moved and why.
2. [prd.md](prd.md) and [roadmap.md](../implementation/roadmap.md) updated in the **same
   pull request**.
3. For anything leaving 0.1: a sentence on why the MVP is still usable without it.

---

## 7. Gate

**Slice 0 does not start until this document is committed.** It is the first artifact,
before the skeleton, because every slice after it is an argument about scope that this
document has already settled.
