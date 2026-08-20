# Aviora Mira — Design System

Status: **in progress.** Structure and navigation live in
[information-architecture.md](information-architecture.md); this document is the visual
and interaction language.

---

## 1. Direction

**Mira is a variable star.** Omicron Ceti — a red giant whose brightness rises and falls
on a slow, reliable cycle. That is also the product: a quiet surface whose *brightness*
tells you what is alive right now.

So the design system's organising decision is:

> **Status is carried by luminance first, hue second, shape always.**

A project that is running glows; a project that is idle recedes; a project that needs you
is warm. Nothing shouts. This buys three things at once — a calm surface that can sit on
screen all day, an accessible status language that survives every form of colour
blindness and every monitor, and a visual identity that is not the acid-green-on-black
that every developer tool reaches for.

Ambient light on a dark, cool ground; warm ember for the things that are live. An
observatory, not a cockpit.

### What it must never look like

A cockpit dashboard (gauges, wall of numbers), an IDE (chrome nested in chrome), a
consumer app (gradients, mascots, oversized rounded cards), a terminal cosplay
(everything monospaced, green on black), or a toy (bounce, confetti, personality copy).

---

## 2. Colour

Two grounds — **Night** (default) and **Day** — with one shared accent family. All values
are OKLCH-derived and shipped as hex tokens.

### Ground: Night

| Token | Hex | Use |
|---|---|---|
| `--ground-0` | `#0C1116` | Window background — cool ink, not black |
| `--ground-1` | `#121A21` | Panels, sidebar |
| `--ground-2` | `#18232C` | Cards, rows, inputs |
| `--ground-3` | `#213039` | Hover, raised rows |
| `--line` | `#2A3B46` | Hairlines, dividers |
| `--line-strong` | `#3A4E5A` | Focused borders |
| `--ink-0` | `#E8EFF3` | Primary text |
| `--ink-1` | `#A8BAC4` | Secondary text |
| `--ink-2` | `#6F8492` | Tertiary, metadata |
| `--ink-3` | `#4A5D69` | Disabled, placeholder |

### Ground: Day

| Token | Hex | Use |
|---|---|---|
| `--ground-0` | `#F7F9FA` | Window background |
| `--ground-1` | `#FFFFFF` | Panels |
| `--ground-2` | `#F0F4F6` | Cards, rows |
| `--ground-3` | `#E4EBEF` | Hover |
| `--line` | `#D5DFE4` | Hairlines |
| `--line-strong` | `#B4C3CB` | Focused borders |
| `--ink-0` | `#0E171D` | Primary text |
| `--ink-1` | `#3D5563` | Secondary |
| `--ink-2` | `#5F7784` | Tertiary |
| `--ink-3` | `#93A6B0` | Disabled |

Day is not an inversion. The grounds get *cooler and lighter*, the ink stays blue-black,
and the ember accent darkens so it stays legible on white.

### Accent: Ember

The one warm family in the product. Used for what is **live, selected, or yours**.

| Token | Night | Day | Use |
|---|---|---|---|
| `--ember-bright` | `#FFB454` | `#B25E00` | Active, running, primary action |
| `--ember` | `#E8963C` | `#9A5000` | Default accent |
| `--ember-dim` | `#8A5E2C` | `#D9A05C` | Idle-but-present, borders |
| `--ember-wash` | `#2A1F14` | `#FDF2E3` | Accent backgrounds, selection |

### Signals

Sparse by design — three signal hues total, each with a mandatory non-colour partner.

| Token | Night | Day | Meaning | Non-colour partner |
|---|---|---|---|---|
| `--signal-ok` | `#5FC9A0` | `#0F7A57` | Clean, healthy, up | filled dot `●` |
| `--signal-warn` | `#E8B84B` | `#8A6400` | Stale, degraded, unavailable | half dot `◐` |
| `--signal-danger` | `#F2735F` | `#B02B15` | Conflict, error, destructive | ring `◉` + label |

Rules:
1. **Never colour-only.** Every signal carries a shape, an icon, or text.
2. **No green for "running".** Running is *ember* — it is yours and it is live. Green
   means *healthy*, which is a different question, and conflating them is why most dev
   dashboards read as noise.
3. Danger appears only on genuinely destructive controls (terminate). Nothing else in the
   product is allowed to be red.

### Accent choice by the user

Users pick an accent (PRD 22). The picker computes contrast against both grounds and
**refuses** any accent failing 4.5:1 for body text or 3:1 for UI borders. A per-project
accent overrides the global one inside that project only — that is how you tell projects
apart at a glance, which is the whole premise of the app.

---

## 3. Typography

Two families, three roles, one opinion.

| Role | Family | Why |
|---|---|---|
| **Interface** | IBM Plex Sans | Humanist warmth with drafting-table engineering in the letterforms — designed for technical products without the neutrality of the usual UI grotesques. Open (OFL), bundled. |
| **Data** | IBM Plex Mono | Ports, PIDs, SHAs, paths, branch names, timings. Same skeleton as the interface face, so mixing them inside one line does not fracture. |
| **Emphasis** | IBM Plex Sans Medium/SemiBold | Mira has no headlines. Emphasis is a weight, never a second display face. |

**The signature typographic move: the data *is* the display type.** A status tool has no
hero copy. What the eye should land on is `:5173`, `main ↑2`, `a3f91c4`, `47 min` — so
those are set in Plex Mono with tabular figures, one step larger and one step brighter
than the label beside them. Labels shrink and recede; values carry the page. This inverts
the usual hierarchy, in which a bold label announces a small grey value.

```
PORT                     ← label: 11px, uppercase, tracked, --ink-2
:5173  vite  Ⓟ           ← value: 15px mono, --ink-0, tabular
```

### Scale

Sized for a window read at arm's length in the corner of a screen, not a document.

| Token | Size / line-height | Weight | Use |
|---|---|---|---|
| `--text-value-lg` | 17 / 22 | 500 mono | Primary values in detail views |
| `--text-value` | 15 / 20 | 400 mono | Ports, SHAs, paths, branches |
| `--text-body` | 14 / 20 | 400 | Interface text |
| `--text-ui` | 13 / 18 | 400 | Rows, controls, menus |
| `--text-label` | 11 / 14 | 500, +0.08em, uppercase | Section and field labels |
| `--text-micro` | 10 / 13 | 500 mono | Chips, counts, badges |

No size above 17px exists in the product. There is nothing in Mira that deserves to be
large; anything that seems to need it is a layout problem.

Rules: tabular figures everywhere numbers can change (`font-variant-numeric: tabular-nums`)
so rows do not jitter on refresh; paths truncate in the **middle** (`…/src/pages/Step7.jsx`),
never the end; commit subjects truncate at the end with a tooltip; no italics; no
letter-spacing on body text.

---

## 4. Space, shape, depth

**Grid:** 4px base. Spacing tokens `1`–`12` = 4–48px. Two densities:
**Comfortable** (row 32px, section gap 20px) and **Compact** (row 26px, section gap 14px).
Compact is a real mode, tested, not a squeeze.

**Radius:** `--r-sm 4px` (chips, inputs) · `--r-md 6px` (rows, cards) · `--r-lg 10px`
(windows, overlays). Nothing is fully rounded. No pill buttons.

**Depth is light, not shadow.** Layers separate by ground value and a hairline. Shadows
appear on exactly two things — the compact window and the Peek overlay — because those
genuinely float above the OS. Everything else is flat. A tool that sits on screen all day
cannot afford drop shadows on every card.

**Borders:** 1px hairlines at `--line`. Focus is a 2px `--ember` ring at 2px offset,
always visible, never removed.

---

## 5. The status language

One vocabulary, used identically in the tray, the list, the compact window, and detail
views. Learn it once.

| State | Mark | Luminance | Meaning |
|---|---|---|---|
| Idle | `○` hollow, `--ink-3` | recessed | Known, nothing running |
| Live | `●` filled, `--ember-bright` | bright, slow pulse | Something of yours is running |
| Dirty | `●` + `▪` notch, `--ember` | mid | Uncommitted changes |
| Attention | `◐` `--signal-warn` | mid-bright | Stale, degraded, unavailable |
| Conflict | `◉` `--signal-danger` | bright | Merge conflict or error |
| Missing | `⌀` `--ink-3`, 60% opacity | dim | Path gone |

### The pulse — the one signature effect

A project with something live breathes: a **6-second** luminance cycle between 82% and
100% of `--ember-bright`. Slow enough to read as ambient light rather than a blink, and
it is the only animated element in the default experience.

It obeys four rules without exception: it stops when the window loses focus, it never runs
in the tray icon (tray icons that animate are a cost people notice), it respects
`prefers-reduced-motion` by rendering at steady 100%, and it is pure CSS opacity on a
composited layer — measurably 0% CPU when off-screen.

One effect, one meaning, everywhere. That is the whole personality budget.

### The ground, and the platform's material

The window's ground is the one place Mira defers to the operating system rather than to
this document.

Mira asks the platform for its **standard window material** and lets `--ground-0` step
back to let it through: Liquid Glass on macOS 26, vibrancy on earlier macOS, Mica on
Windows 11. Mira does not draw a glass panel of its own, and there is no cross-platform
"glass" layer — an imitation built once and applied everywhere would quietly become the
design, and it would look wrong on all three platforms instead of right on any.

Where no material exists — Linux, Windows 10, a refused effect — the ground is solid.
That is a first-class outcome, not a degraded one, and Linux is emphatically not given a
hand-drawn blur to match (`platform-abstraction.md` §4.11).

What the material may **never** do:

1. **Carry information.** It is a backdrop. Status is still luminance, hue and shape.
2. **Lower contrast.** Panels, rows, cards and text stay fully opaque. Every contrast
   floor in §2 holds with the material on, because none of them is measured against it.
3. **Override the person.** `prefers-reduced-transparency: reduce` returns the ground to
   solid, immediately and without a preference to find.
4. **Be assumed.** The interface applies `data-surface` from what the shell reports it
   achieved, and never from a platform check — a guard test fails the build if a
   platform name appears in the frontend or the stylesheet.

The atmospheres in §7 layer *inside* this, and none of them may reach the ground
treatment: an atmosphere changes light, never the window's relationship to the OS.

---

## 6. Motion

Motion exists to explain change, never to entertain.

| Token | Duration | Curve | Use |
|---|---|---|---|
| `--motion-instant` | 80ms | `ease-out` | Hover, focus, press |
| `--motion-quick` | 140ms | `cubic-bezier(.2,0,0,1)` | Section expand, row insert |
| `--motion-surface` | 180ms | `cubic-bezier(.2,0,0,1)` | Compact window, Peek |
| `--motion-pulse` | 6000ms | `ease-in-out` alternate | The pulse |

Prohibited: bounce, spring overshoot, staggered list entrances, skeleton shimmer (show
last-known data with a pending marker instead), page transitions, parallax, and anything
that delays a user-initiated action. `prefers-reduced-motion` disables everything except
opacity fades under 100ms.

The compact window's appearance is the one place motion is felt: 180ms opacity plus a 4px
rise. Any slower and the 250ms budget is gone.

---

## 7. Atmospheres

Personalization without becoming a toy. **Minimal is the default and always will be.**

An atmosphere may change: background treatment, accent family, and one ambient layer.
It may **never** change: type scale, spacing, contrast floors, the status language, or
layout. That constraint is what keeps Mira professional with any atmosphere on — the
information design is invariant, only the light changes.

| Atmosphere | Ground | Accent | Ambient layer |
|---|---|---|---|
| **Minimal** *(default)* | Night / Day as specified | Ember | None |
| **Cosmic** | Deeper indigo-black `#080B14` | Ember, cooler `#FFC069` | Static star field, one very slow drifting layer |
| **Sakura** | Warm grey-mauve `#171316` / `#FAF6F7` | Soft rose `#E8879B` | Slow falling petals, ≤ 12 at a time, edges only |
| **Cyberpunk** | Ink `#0A0E14` with a magenta cast | Cyan `#4FD6E8` + magenta signal | Faint scanline, static gradient — **no** flicker, **no** glow on text |
| **Rain** | Slate `#101720` | Ember, dimmed | Rain on the window edge, ≤ 20 drops, no sound |
| **Custom** | User image or colour | User accent | None — user art is the atmosphere |

### Hard constraints

1. **≤ 2% CPU** while idle-visible on a 2019 laptop. An atmosphere that misses this is not
   shipped, not "optimised later".
2. Ambient layers **pause entirely** when the window is unfocused, hidden, or the battery
   is below 20%.
3. `prefers-reduced-motion` freezes every ambient layer to a static frame.
4. AA contrast for all text and UI in every atmosphere, verified in CI on a rendered
   snapshot per atmosphere.
5. Ambient layers sit **behind** content at ≤ 12% opacity and never overlap text.
6. Custom backgrounds get an automatic scrim so ink contrast holds regardless of the
   image.
7. **No anime art, no mascots, no characters.** Atmospheres are abstract light and
   weather. Cyberpunk gets neither glow-on-text nor flicker, because both hurt to read.

Atmospheres are one CSS custom-property file plus at most one canvas layer. They cannot
add DOM to product views, which makes them safe to accept as contributions.

---

## 8. Components

A small kit. Anything not on this list needs justification.

| Component | Notes |
|---|---|
| **Row** | The workhorse: mark · label · value · actions-on-hover. Ports, files, commits, shelf items, branches are all Rows. |
| **Section** | Label + collapse + content. Absent when it has nothing to say — never rendered empty. |
| **Chip** | Micro mono text on `--ground-3`. Counts, ahead/behind, container state. |
| **Button** | Three kinds only: quiet (default), accent (one per view maximum), danger (terminate only). No icon-only buttons without a tooltip and an accessible name. |
| **Field** | Single-line input, hairline border, ember focus ring. |
| **Menu** | Tray menu and context menus; native where the OS provides one. |
| **Overlay** | Peek and confirmations. Only two overlays exist; they never stack. |
| **Sparkline** | System strip only. 60 samples, 1px stroke, no axes, no grid, no fill. |
| **Graph lanes** | Git history: 1px strokes, ≤ 8 lanes, lane colour by index from a muted 8-step ramp — never signal hues, which would imply meaning that is not there. |
| **Empty state** | One sentence plus the single action that resolves it. |

### Icons

One set, 16px, 1.5px stroke, monochrome, inheriting `currentColor`. No filled icons, no
brand logos inside the UI (an editor's identity is its name, not its mark), no emoji in
product chrome — users may pick an emoji as a *project* icon, which is their content.

---

## 9. Voice

Interface copy is design material and follows one register: plain, active, specific.

- **Actions say what happens.** "Open terminal", not "Launch". "Terminate process", not
  "Stop" — the stronger word is the honest one, and the confirmation names the target.
- **Consistency over variety.** A control named "Open editor" produces the state "Opened
  in Zed". The same action never has two names.
- **Errors state the fact and the fix.** "VS Code not found. Choose your editor in
  Settings." Never "Oops!", never an apology, never a stack trace as the first thing.
- **Unavailable states name the reason**, and the reason is true:
  "Not available on macOS — Apple restricts this to entitled apps."
- **Empty states invite one action.** "No projects yet. Add the folder you're working in."
- **No personality copy.** Mira does not greet you, congratulate you, or make jokes. The
  single soft moment in the product is "Welcome back — 47 minutes away", and it exists
  because the number is genuinely useful.
- Sentence case everywhere except `--text-label`, which is uppercase by treatment, not by
  writing.

---

## 10. Accessibility floor

Not a phase — a build gate.

- AA contrast minimum for all text and UI, in every ground and every atmosphere, verified
  in CI.
- Colour is never the only channel; every status has a shape or text partner (§5).
- Visible focus on every interactive element; tab order matches visual order.
- Full keyboard operation of every action (IA §7). No hover-only affordance: row actions
  appear on hover *and* on focus.
- Screen-reader names on all controls; status dots expose text ("running", "conflict").
- `prefers-reduced-motion` and `prefers-contrast` are honoured.
- Text scales to 200% without loss of function; no fixed-height text containers.
- Minimum hit target 28×28px in Comfortable, 24×24px in Compact.

---

## 11. Tokens

One source of truth: `src/styles/tokens.css` defines every value here as a CSS custom
property. Themes and atmospheres override **only** token values. Components reference
tokens exclusively — a raw hex or a magic pixel value in a component is a review failure.

```css
:root {
  --ground-0:#0C1116; --ground-1:#121A21; --ground-2:#18232C; --ground-3:#213039;
  --line:#2A3B46; --line-strong:#3A4E5A;
  --ink-0:#E8EFF3; --ink-1:#A8BAC4; --ink-2:#6F8492; --ink-3:#4A5D69;
  --ember-bright:#FFB454; --ember:#E8963C; --ember-dim:#8A5E2C; --ember-wash:#2A1F14;
  --signal-ok:#5FC9A0; --signal-warn:#E8B84B; --signal-danger:#F2735F;
  --r-sm:4px; --r-md:6px; --r-lg:10px;
  --motion-instant:80ms; --motion-quick:140ms; --motion-surface:180ms;
}
```

Tailwind consumes these tokens; it does not define its own palette. The design system is
the tokens file, and this document explains it.
