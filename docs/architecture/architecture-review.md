# Aviora Mira — Architecture Review

Date: 2026-08-19 · Reviewing: the complete pre-implementation document set · Reviewer:
design pass, adversarial reading of our own work.

This is a critical review, not a summary. Its job is to find the places where the plan is
wrong, over-scoped, or quietly dishonest, before any code makes them expensive.

> **Superseded release plan — this document is a dated record and is left as written.**
> The release mapping and the meaning of "MVP" discussed below were changed after this
> review, by [product-scope.md](../product/product-scope.md): MVP now means the **0.1**
> release alone, and the phases are locked at 0.1 through 0.6+. The findings here still
> stand as findings — §4's scope-realism criticism is in fact what the phase lock
> answers — but do not read the version numbers or the "V1.x" tier in this document as
> current. Nothing below has been rewritten, deliberately.

---

## 1. What was checked

| Check | Result |
|---|---|
| Contradictions between documents | 1 found, fixed (see §2) |
| MVP scope realism | **Fails as written** — see §4, the most important finding |
| Platform assumptions accurate | Verified against current sources; 4 gaps confirmed real |
| Architecture supports multiple projects | Yes — verified structurally |
| Mira independent of Astra | Yes — verified by inspection |
| Future Astra integration possible without coupling | Yes — same boundary as plugins |
| Cross-document links | All resolve |
| Schema contains no cloud entities | Confirmed: 15 tables, no users/orgs/subscriptions |

---

## 2. Contradictions found

**Fixed:** the release table called 0.4.0 "MVP scope complete" and 0.5.0 "MVP
feature-complete". Now 0.4.0 is "Awareness features" and 0.5.0 alone is feature-complete.

**No other contradictions found**, but three tensions are worth naming because they are
where drift will start:

1. **"Lightweight" versus 22 MVP features.** Both are stated honestly, and they pull hard
   against each other. §4 resolves it.
2. **"Keyboard-friendly" versus "no keyboard-only kill path"** (PRD 8, IA §7). A
   deliberate exception to a principle. It is right, and it should stay documented as an
   exception rather than quietly generalised into "some actions need the mouse".
3. **"Local-first, no config in the repo"** (security §5.7) versus developer expectation.
   Most tools of this kind support a committed per-repo config file. Mira forbids it,
   because a committed file that names a program to run turns cloning a hostile repo into
   code execution. This will be requested by users, and the answer must stay no; the
   reasoning belongs in the FAQ before the first release.

---

## 3. Strengths

**The capability model is the best decision in the set.** Making `Full / Degraded /
Unavailable` a runtime value with user-facing copy, and banning `cfg(target_os)` outside
one crate, converts "do not fake parity" from a slogan into something the compiler and CI
enforce. Most cross-platform projects discover these gaps in bug reports; this one starts
from them.

**The research changed the design rather than decorating it.** Four findings —
Wayland has no global-shortcut protocol, Linux tray click events never fire, macOS
now-playing is entitlement-gated since 15.4, lock detection needs logind — each produced a
concrete design response (the `mira --toggle` fallback, a menu-first tray on every
platform, an honest Unavailable state plus an `otool -L` guard test, a Degraded fallback).
None was smoothed over.

**Rejecting `tauri-plugin-sql` is the highest-value technical call.** It is the path of
least resistance in the Tauri ecosystem, and taking it would have handed the webview an
arbitrary-SQL channel — dissolving the exact privilege boundary the rest of the security
model is built on. Choosing more ceremony here is correct.

**Security is structural, not procedural.** The no-shell rule is enforced by the *schema*
(`commands.args` is a JSON array, so there is nowhere to put a command line), by the API
(argv only), and by a guard test. That is three layers, and the cheapest one is the schema.

**Vertical slices are genuinely vertical.** Slice 1 crosses schema → service → IPC → UI →
tray → shortcut. Slice 0 is named rather than smuggled into Slice 1, which is where these
plans usually start lying.

**The Astra boundary is testable.** "Deleting the integration crate must leave a compiling,
fully functional Mira" is a check someone can run, unlike a promise of independence.

---

## 4. Risks

Ordered by expected damage.

### R1 — The MVP is not an MVP *(high, structural)*

Twenty-two features across Git, ports, processes, launching, SSH, Docker, files, previews,
system, media, sessions, shortcuts, tray, and theming — on three operating systems, with a
guard-test suite, from an empty repository. Realistically **six to nine months of solo
work**, and the first user feedback arrives at the end of it. That is the failure mode
every "MVP" list of this shape has.

The roadmap already contains the fix without naming it: **slices 0–3 produce a genuinely
useful tool** — projects, Git status, ports, kill, launch. That is the minimum viable
product. The remaining eighteen features are V1.

**Recommendation:** rename the scopes. `0.1.0` (slices 0–3) is the **MVP**; the current
22-feature list becomes **V1**; the current V1.x becomes **V2**. Nothing about the plan
changes except honesty about what ships first — and that honesty is what gets feedback
into the loop before slice 9 instead of after it.

### R2 — Windows weakens the flagship feature *(high, unavoidable)*

Port attribution is Mira's best answer to its best question ("what's on :3000, and is it
mine?"), and it depends on a process's working directory — which Windows routinely will
not give. The fallback, matching by executable path, is materially worse: every project
running `node.exe` looks alike.

**Mitigation:** be explicit in the UI ("matched by executable path"), and add a
Windows-specific signal early — reading the command line for a path argument, or matching
against the project's expected ports — as part of slice 2 rather than as a later fix. If
attribution proves unusable on Windows in practice, expected-ports-per-workspace becomes
the primary mechanism there rather than a convenience.

### R3 — Polling architecture is the whole performance story *(high, controllable)*

Idle CPU ≈ 0% and ≤ 150 MB RSS are the product's differentiators, and the single scheduler
with visibility gating is the only thing delivering them. One module starting its own
timer — easy to do, easy to miss in review — silently destroys the property.

**Mitigation:** the guard test asserting *no work while hidden* must exist in **slice 1**,
not slice 9, so every later slice inherits it. Budgets are measured every slice.

### R4 — WebKitGTK is the weakest surface *(medium, certain)*

The Linux webview differs per distro and is the least consistent of the three. Every
Tauri project pays this tax.

**Mitigation:** already partly designed for — the UI is deliberately plain (no WebGL, no
exotic CSS). Add: Linux CI from slice 0, and test on at least one GNOME/Wayland and one
KDE/X11 system before 0.1.0. The atmospheres' canvas layers are the most likely casualty;
they are also the most droppable feature, which is a reason to keep them last (slice 10).

### R5 — The guard-test suite is a large upfront investment *(medium)*

Ten guard tests, several needing real infrastructure (socket-level network assertions,
`otool -L` in CI, dependency-tree scans, a `capabilities/*.json` snapshot). Written all at
once they would delay slice 1 badly; written "later" they never appear.

**Mitigation:** the roadmap's rule — each guard test is written in the slice that creates
its risk — is correct. Slice 1 needs the path-containment and no-work-while-hidden tests
only. Make that explicit in the slice-1 issue list so it is not read as "write all ten now".

### R6 — Git graph lane layout *(medium, contained)*

Historically the most bug-prone UI in any Git tool.

**Mitigation:** already correct in the plan — a pure Rust function, unit-tested against
fixture repositories, never React logic. Add fixtures for octopus merges and long-lived
parallel branches before writing the renderer.

### R7 — Process termination is the highest-severity user harm *(medium, well mitigated)*

Killing the wrong process loses someone's work. Mitigations are strong (named
confirmation, no bulk kill, no keyboard-only path, refusal rules, graceful-first).

**Residual risk:** attribution errors (R2) feeding a destructive action. The rule "never
auto-act on attribution, always name the process in the confirmation" is what holds; it
must not be softened for convenience later.

### R8 — Two nouns where users expect one *(low–medium)*

Project → Workspace is real conceptual overhead.

**Mitigation:** hiding workspaces until a second exists is a genuine solution, and it is
specified as a hard requirement with an acceptance criterion rather than a nicety. Watch
for it eroding once workspace-specific features accumulate.

### R9 — Unsigned releases *(low now, medium at adoption)*

macOS Gatekeeper and Windows SmartScreen will scare off non-technical-adjacent users. For
0.1.0 the honest install instructions are acceptable; by the time Mira is recommended
publicly, certificates are needed.

### R10 — Solo-project bus factor *(low, inherent)*

Mitigated better than most: the decisions are written down, the ADRs carry reasoning, and
the crate boundaries let someone work on one service without understanding the rest.

---

## 5. Unresolved decisions

These are deliberately open. Each has a decision point in the roadmap and none blocks
slice 1 except the first.

| # | Decision | Due | Notes |
|---|---|---|---|
| U1 | **`git2` vs `gitoxide`** | Slice 1 | Plan says `git2` behind a trait. `gitoxide` is pure Rust, faster, no C build, and production-ready for the reads Mira needs — but `git2` is more battle-tested for status edge cases (submodules, worktrees, sparse checkouts). Start with `git2`; re-evaluate at slice 5 when the graph exercises the walk API. |
| U2 | **Port enumeration crate** | Slice 2 | `netstat2` is effectively unmaintained and does not give process names; `sysinfo` does not expose sockets; the `listeners` crate covers pid+name cross-platform but is young. Prototype `listeners` first, keep the `PortHost` trait tight enough to hand-roll per-OS if it disappoints. This is the least-settled dependency in the plan. |
| U3 | **Docker client** | Slice 8 | `bollard` (full-featured, heavier) vs a minimal hand-rolled read-only HTTP-over-socket client. Given the `GET`-only guarantee, a ~200-line client may be both smaller and easier to audit. Decide when the slice starts. |
| U4 | **Bundled fonts vs system stack** | Slice 0 | IBM Plex bundled costs ~400 KB and guarantees identical rendering; a system stack costs nothing and looks different on every OS. Leaning bundled — the design system depends on Plex's character — but measure against the 30 MB budget. |
| U5 | **Code-signing certificates** | Before 0.1.0 announcement | Apple Developer + Windows OV/EV. A cost and an identity decision, not technical. |
| U6 | **Application identifier / domain** | Slice 0 | `dev.aviora.mira` is used throughout the data model. Confirm the domain is actually controlled before it is baked into app-data paths, where changing it later strands every user's database. |
| U7 | **Session recording default** | Slice 9 | Currently on by default (`privacy.record_sessions = true`) with deletion available. Defensible, but a privacy-first product defaulting to recording deserves a second look — consider off-by-default with a first-run prompt. |
| U8 | **Windows attribution fallback strategy** | Slice 2 | See R2. Depends on what real testing shows. |

---

## 6. What should explicitly NOT be built yet

Restating so it survives contact with enthusiasm:

- **No plugin host.** Needs a trust model first. The dependency rules keep it cheap later.
- **No `automations` table, no automation UI, no trigger schema.** Not even "just the
  table". An action list that launches processes is the highest-value target in the app.
- **No Astra anything** — no crate, no types, no schema column, no feature flag.
- **No cloud, sync, account, or telemetry surface**, including a disabled toggle.
- **No Git writes.** Not commit, not fetch, not even a disabled button hinting at one.
- **No Docker lifecycle actions.** Read-only until there is a reason and a review.
- **No notification conditions** beyond capability reporting.
- **No `mira-automation` crate**, even empty.
- **No general app launcher or global fuzzy search.** That is the Raycast boundary; the
  compact window stays project-scoped.
- **No abstraction built "for when we add X".** Every one of the above is cheap to add
  later precisely because nothing was pre-built for it.

---

## 7. Verification of the specific claims

**Does the architecture support multiple projects?** Yes, structurally: the project list is
the root of the main window with no global-current-project mode; per-project state is
independent and persisted; the schema scopes workspaces, sessions, shelf items, SSH hosts,
and Docker refs by `project_id`; the active workspace is per project
(`project_active_workspace`, one row per project); and freshness tiers keep all projects
current in the list while computing expensive detail only for the visible one. The IA
states "never a single global active workspace" as an anti-requirement.

**Is Mira independent of Astra?** Yes. Astra appears in the documents only as prose about
the relationship and as an example project name in mockups (`astra-api`). No crate, type,
table, column, dependency, or feature flag references it. Deleting every mention would
change no design.

**Is future Astra integration possible without coupling?** Yes, through the same boundary
plugins will use: an optional module consuming public interfaces, off by default, with
deletability as the acceptance test. The one-way dependency rule (nothing but `src-tauri`
depends on `tauri`; core depends on nothing) is what makes that boundary real rather than
aspirational.

---

## 8. Recommended next step

**Adopt R1's renaming**, then build **Slice 0 (Skeleton)** immediately, followed by
**Slice 1**.

Slice 0 first, without adding features, because it de-risks the three things that are
painful to retrofit: cross-platform CI, the migration runner, and Rust→TypeScript type
generation. It should take days, and it makes every subsequent slice a normal piece of
work.

Concretely, before writing product code:
1. Confirm U6 (the `dev.aviora.mira` identifier) — it is baked into user data paths.
2. Stand up CI on all three platforms with an empty test suite. Green on Linux is the
   canary.
3. Write `0001_init.sql` and the migration runner with its round-trip test.
4. Wire `ts-rs` and commit a generated type, so the contract is visible in diffs.
5. Add the design tokens file. Nothing else from slice 10.

Then Slice 1, with the path-containment and no-work-while-hidden guard tests included in
its definition of done.

---

## 9. GO / NO-GO

### **GO** — begin Slice 0, then Slice 1.

**Conditions attached to the GO:**

1. **Rename the scopes per R1** before publishing a roadmap anyone plans against. Shipping
   0.1.0 after slices 0–3 is the plan; calling 22 features an MVP is not.
2. **Confirm U6** (application identifier / domain) before the first migration runs on a
   real machine.
3. **Include the two guard tests in slice 1's definition of done** — path containment and
   no-work-while-hidden. They are cheap now and unaffordable to retrofit.
4. **Linux CI from slice 0**, not later. It is the platform most likely to invalidate an
   assumption, and it is where an assumption is cheapest to fix on day one.

**Why GO rather than more design.** The plan's remaining uncertainty is concentrated in
things that documents cannot resolve — how `listeners` behaves on a real Windows machine,
whether WebKitGTK renders the compact window acceptably, whether port attribution is
usable without cwd. Those are answered by slice 1 and 2, not by another review. The
architecture is sound, the security model is unusually well-specified for this stage, the
platform honesty is real rather than performed, and the slices are genuinely vertical.

**What would make this a NO-GO** — if any of these were true, more design would be needed
first: an MVP with no usable increment before month six (R1's rename resolves it); a
security model relying on review discipline rather than structure (it does not); a
cross-platform plan assuming parity that does not exist (it assumes the opposite); or a
data model carrying cloud entities on the promise of never using them (it carries none).

None hold. Build it.
