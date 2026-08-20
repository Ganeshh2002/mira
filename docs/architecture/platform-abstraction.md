# Aviora Mira — Cross-Platform Architecture

Status: **in progress.** Slices 0 and 1 are built; the rest is the design target.

This document exists because of one failure mode: OS-specific code leaking into the UI,
and features quietly pretending to work where they do not. Both are prevented
structurally here.

---

## 1. The rule

**No `#[cfg(target_os)]` and no `process.platform` check outside `mira-platform`.**

The UI knows *capabilities*, never operating systems. React may ask "can I register a
global shortcut?" It may never ask "am I on Linux?"

Exactly two exceptions, both cosmetic and both in one file each: keyboard glyphs
(`⌘` vs `Ctrl`) and native-chrome offsets for the custom title bar.

---

## 2. Capability model

Every OS-touching feature is a **capability** with a runtime status, not a compile-time
assumption.

```rust
pub enum CapabilityStatus {
    /// Works as specified.
    Full,
    /// Works with a user-visible reduction.
    Degraded { reason: String, detail: String },
    /// Cannot work here. `fallback` names what to do instead, if anything.
    Unavailable { reason: String, fallback: Option<String> },
}

pub enum Capability {
    GlobalShortcut, TrayIcon, TrayClickEvents, LaunchApplication,
    ProcessEnumeration, ProcessTermination, PortEnumeration, PortAttribution,
    LockDetection, SleepDetection, MediaNowPlaying, MediaControl,
    Notifications, FileWatching, RevealInFileManager, DragOutFiles,
    AutoStart, BatteryInfo, AutoUpdate,
}

pub trait PlatformCapabilities {
    fn status(&self, cap: Capability) -> CapabilityStatus;
}
```

Rules:

1. Status is resolved **at runtime**, on the actual machine — Wayland vs X11, logind vs
   not, Docker present or not, macOS version. Compile-time targets are too coarse to be
   honest.
2. Statuses are queried once at startup, cached, and re-evaluated on relevant system
   events; changes emit `mira://capability`.
3. Every `Degraded`/`Unavailable` string is **user-facing copy**, written to be read by a
   person, not a log parser.
4. A feature that is `Unavailable` renders as off *with its reason*. It is never hidden
   silently and never shown as broken.

### The parity rule

> **Do not fake parity.** If an OS cannot do a thing, Mira says so in the UI, in this
> document, and in the README. It does not emulate, it does not degrade silently, and it
> does not use private or SIP-defeating APIs to close a gap.

---

## 3. Trait surface

`mira-platform` exposes small traits, one per concern, each with three implementations
plus a fake for tests:

```rust
pub trait ShortcutHost   { fn register(&self, chord: &Chord) -> Result<()>; fn unregister_all(&self); }
// Implemented (Slice 4). `LaunchTarget` is a directory or an address Mira built —
// there is no variant carrying a program or an argument. See ADR-0013.
pub trait LaunchHost     { fn openable(&self) -> Vec<AppReport>;
                           fn launch(&self, kind: AppKind, target: LaunchTarget) -> Result<Launched>; }
pub trait ProcessHost    { fn info(&self, pid: Pid) -> Result<ProcessInfo>;
                           fn list_for_roots(&self, roots: &[&Path]) -> Vec<ProcessInfo>;
                           fn terminate(&self, pid: Pid, mode: TerminateMode) -> Result<()>; }
pub trait PortHost       { fn listening(&self) -> Result<Vec<Listener>>; }
pub trait SessionHost    { fn subscribe(&self, sink: EventSink<SessionEvent>) -> Subscription; }
pub trait MediaHost      { fn now_playing(&self) -> Result<Option<NowPlaying>>; }
pub trait SystemHost     { fn sample(&self) -> SystemSample; }
pub trait ShellHost      { fn reveal(&self, path: &Path) -> Result<()>;
                           fn open_url(&self, url: &Url) -> Result<()>; }
pub trait AutostartHost  { fn get(&self) -> bool; fn set(&self, on: bool) -> Result<()>; }
```

```text
crates/mira-platform/src/
  lib.rs          capability enum, status resolution, trait definitions
  common/         shared helpers (argv building, path canonicalisation)
  macos/          objc2 / core-foundation
  windows/        windows-rs
  linux/          zbus, /proc, xdg
  fake/           deterministic test doubles
```

Selection happens once, at startup, behind `Platform::detect()`. Nothing downstream
knows which arm was taken.

---

## 4. Per-concern platform behaviour

Each section states what happens per OS and, where relevant, what Mira refuses to do.

### 4.1 Global shortcuts

| OS | Status | Notes |
|---|---|---|
| macOS | **Full** | `RegisterEventHotKey` via Tauri's plugin. Some chords need Accessibility/Input-Monitoring permission; Mira prompts once with an explanation. |
| Windows | **Full** | `RegisterHotKey`. Fails if another app owns the chord — the error names the chord. |
| Linux / X11 | **Full** | X11 grab. |
| Linux / Wayland | **Unavailable** | No cross-compositor protocol exists; Tauri's implementation is X11-specific and is disabled on Wayland (it would otherwise crash in libX11). |

**Wayland fallback (implemented, not hand-waved):** Mira is single-instance and accepts
`mira --toggle`, which raises or hides the running instance. Settings detects Wayland,
shows the exact command, and gives per-desktop instructions (GNOME: Settings → Keyboard →
Custom Shortcuts; KDE: System Settings → Shortcuts → Custom). The capability reports
`Unavailable { fallback: Some("mira --toggle") }`.

Detection: `XDG_SESSION_TYPE=wayland` or `WAYLAND_DISPLAY` set.

### 4.2 Application launching

**Implemented in Slice 4.** ADR-0013 has the reasoning; this is the mechanism.

| OS | Mechanism |
|---|---|
| macOS | `NSWorkspace.openURLs:withApplicationAtURL:configuration:` — two typed `NSURL`s handed to the window server. **Not** `open(1)`: no command line, and no child process for Mira to own. Candidates are `.app` bundles at absolute paths. |
| Windows | The program started with an argv array, quoting owned by the platform layer. Candidates come from `PATH`, including the `.exe`/`.cmd`/`.bat` spellings. |
| Linux | The program started with an argv array. Candidates come from `PATH`; `.desktop` entries prove an application is *installed* (a Flatpak leaves nothing on `PATH`) but are never launched, because doing so needs a portal that is not present everywhere. |

Web addresses do not go to a named application on any platform. They go to the
desktop's own handler — `NSWorkspace.openURL:` on macOS, `explorer` on Windows,
`xdg-open` on Linux — so a service opens in the browser the person chose rather
than the one Mira found first.

**Universal, and enforced by guard tests rather than asserted here:**

- **Argv arrays only, no shell.** The domain layer never builds a command string,
  and `Command::new` exists in exactly one function in this crate.
- **One value in an argv.** Every other element is a `&'static str` from the
  candidate table. The target is the last argument and there is only one.
- **The caller names a kind.** `editor`, `terminal`, `browser` — never a program,
  a path, or an argument. There is no setting that lets someone point Mira at a
  binary; a machine with nothing openable says so.

**Detection and openability are separate answers.** `Candidate::launch` records
whether a found application is one Mira can open a folder *in*. Terminal editors
(Neovim, Vim, Emacs) and terminals with no working-directory option (`xterm`,
Windows PowerShell) are detected and never offered, because launching them would
produce an invisible process or the wrong directory. `Applications::survey`
answers "is one here"; `Applications::openable` answers "can Mira open one".

Status is `Full` on all three platforms. Where a *kind* has no openable
application the answer is `AppPresence::NotInstalled` — a fact shown in the
interface, not an error.

### 4.3 Process management

| OS | Enumeration | cwd | Termination |
|---|---|---|---|
| macOS | `libproc` / `sysinfo` | own-user only (`proc_pidinfo`) | `SIGTERM`, then explicit `SIGKILL` |
| Windows | Toolhelp / `sysinfo` | often unavailable; PEB reads are unreliable | `WM_CLOSE` best-effort, then `TerminateProcess` |
| Linux | `/proc` | `/proc/<pid>/cwd` (permission-dependent) | `SIGTERM`, then explicit `SIGKILL` |

Honest differences, surfaced in the UI: Windows has no true graceful stop for console
apps — "Terminate" is closer to `taskkill` than to Ctrl-C, and the confirmation says so.
Working directory is frequently unknown on Windows, which weakens port attribution there
(§4.4). Elevated or other-user processes cannot be signalled by a non-elevated Mira; the
action is **disabled with a reason**, never attempted-and-failed. Mira never requests
elevation.

### 4.4 Port enumeration and attribution

| OS | Source | Attribution quality |
|---|---|---|
| macOS | `libproc` socket info for own-user processes | Good (cwd available) |
| Windows | IP Helper (`GetExtendedTcpTable`) | **Degraded** — PID yes, cwd usually no, so attribution falls back to executable path |
| Linux | `/proc/net/tcp{,6}` + `/proc/<pid>/fd` inode matching | Good |

Everywhere: sockets owned by other users or by the system appear as **"not
attributable"** rather than being hidden. Docker-published ports are owned by the daemon
and are attributed to Docker, then linked to a project through the container's compose
labels (PRD 14) — not through cwd.

Windows attribution is formally `Degraded`, and the UI says "matched by executable path"
on those rows.

### 4.5 Tray / menu bar

| OS | Status | Notes |
|---|---|---|
| macOS | **Full** | `NSStatusItem`, template image, accessory activation policy (no dock icon by default). |
| Windows | **Full** | Notification-area icon. The OS may hide it; first run explains where. |
| Linux | **Degraded** | StatusNotifierItem via libayatana-appindicator. **Click events do not fire** — a libappindicator limitation — so the menu is the only interaction. Stock GNOME needs an extension for tray icons at all. |

Consequence, baked into the IA: **Mira's tray is menu-first on every platform.** No
feature is reachable only by clicking the icon. If tray creation fails outright, Mira
reports it once and keeps running.

### 4.6 Lock / session detection

| OS | Mechanism | Status |
|---|---|---|
| macOS | `com.apple.screenIsLocked` / `com.apple.screenIsUnlocked` distributed notifications; `NSWorkspace` sleep/wake | **Full** |
| Windows | `WTSRegisterSessionNotification` (lock/unlock), `WM_POWERBROADCAST` (suspend/resume) | **Full** |
| Linux | `org.freedesktop.login1`: `LockedHint` property + `PrepareForSleep` signal, over D-Bus | **Degraded** |

Linux detail: logind covers most modern desktops, but non-logind systems and some lock
screens never set `LockedHint`. There Mira reports `Degraded` and falls back to
window-focus plus an idle timer for session pausing. Sleep/wake detection remains
reliable, which preserves the battery-saving behaviour even when lock is invisible.

### 4.7 Media detection

| OS | API | Status |
|---|---|---|
| Linux | MPRIS2 over D-Bus (`org.mpris.MediaPlayer2.*`) — public, stable | **Full** |
| Windows | `GlobalSystemMediaTransportControlsSessionManager` (WinRT) — public | **Full** |
| macOS | `MediaRemote` — **private framework** | **Unavailable** |

macOS, stated plainly: there is no public API for now-playing metadata. Since
macOS 15.4, `mediaremoted` verifies entitlements and denies unentitled clients, breaking
the private-framework approach that community apps used. The known workarounds require
disabling SIP or piggy-backing on entitled system binaries such as `/usr/bin/perl`.

**Mira ships neither.** On macOS the capability is `Unavailable { reason: "Apple
restricts now-playing information to entitled applications" }`, the setting is disabled
with that text, and CI asserts (`otool -L`) that the binary links no private framework.
This is revisited only if Apple ships a public interface.

### 4.8 Notifications

Native on all three (`UNUserNotificationCenter`, Windows toast, `org.freedesktop.Notifications`).
Practical differences: macOS requires user permission and silently drops notifications
when denied — Mira checks the permission state and shows it in Settings rather than
firing into the void. Linux requires a running notification daemon; absent one, the
capability is `Unavailable`. Notifications are opt-in per condition and are accepted
but unscheduled ([product-scope.md](../product/product-scope.md) §5); the capability
exists from 0.1 only to report status.

### 4.8b Ports and processes

Both are read through native interfaces, never by running a command and parsing
its output. `netstat2` uses `sysctl` on macOS, `GetExtendedTcpTable` on Windows,
and `/proc/net` plus netlink on Linux; `sysinfo` uses `libproc`, the Windows
process APIs, and `/proc`. Nothing here shells out, which removes both the
injection surface and the fragility of screen-scraping `lsof`.

| Concern | macOS | Windows | Linux |
|---|---|---|---|
| Listening TCP sockets | Full | Full | Full |
| Owning pid for a socket | Full (own user) | Full | Full (own user) |
| Process name and parent | Full | Full | Full |
| Executable path | Full | Degraded (may be refused) | Full |
| **Process working directory** | Full (own user) | **Unavailable** | Full (own user) |

The last row is the one that matters, because **project attribution rests on it
alone**. A service is placed by the directory its process is running in — never by
process name, which would put every `node` on :3000 in whichever project was
listed first. Where the working directory is not available, Mira says the service
is there and that it cannot place it, with the reason. That is why Windows shows
services as unattributed rather than guessing, and why the capability matrix
already listed *Process cwd* as Degraded there.

Sockets owned by another user are reported without a pid, and are unattributed for
the same reason. Mira never elevates to see more.

### 4.9 Filesystem behaviour

| Concern | macOS | Windows | Linux |
|---|---|---|---|
| Case sensitivity | Insensitive (usually), preserving | Insensitive | Sensitive |
| Path length | 1024 | 260 unless verbatim `\\?\` | 4096 |
| Watching | FSEvents (coalesced, directory-level) | `ReadDirectoryChangesW` | inotify (per-watch limits) |
| Reveal in manager | `open -R <file>` — selects the file | `explorer /select,<file>` — selects the file | `xdg-open <parent>` — **cannot select the file** |
| Drag files out | Full | Full | **Degraded** — depends on the desktop/webview |

Rules: always store canonical absolute paths; compare case-insensitively on
macOS/Windows; use verbatim paths internally on Windows; treat inotify exhaustion as a
`Degraded` watcher and fall back to focus-triggered refresh with a visible note.
Reveal-in-manager on Linux opens the containing directory and the UI says "opens the
folder" there — it does not claim to select the file.

### 4.10 Startup behaviour

| OS | Mechanism |
|---|---|
| macOS | `SMAppService` login item (Ventura+), via `tauri-plugin-autostart` |
| Windows | `HKCU\...\Run` registry entry |
| Linux | `~/.config/autostart/mira.desktop` (XDG) |

Off by default everywhere. Sandboxed/immutable environments (some Flatpak setups) may
refuse; the toggle then reports `Unavailable` with the reason instead of silently
failing. Started-at-login means **tray only** — no window is shown.

### 4.11 Window behaviour

Differences accepted rather than normalised: macOS uses traffic-light controls and
`⌘W` closes-but-keeps-running; Windows/Linux use standard controls and minimise-to-tray.
The compact window is borderless, always-centred-on-cursor-display, and non-resizable on
all three. Multi-monitor DPI changes are handled by the webview; Mira only persists
window geometry per display id.

Linux/Wayland cannot position windows programmatically — the compositor decides. The
compact window's "appear at the cursor's display" behaviour is therefore `Degraded` on
Wayland: it appears wherever the compositor places it.

**Window material.** Mira asks the operating system for its *standard* window material
rather than drawing an imitation of one. On macOS 26 that material is Liquid Glass; on
earlier macOS it is vibrancy; on Windows 11 it is Mica. Nothing checks a version,
because the platform decides what its own material looks like — which is the difference
between using a platform's design and copying its screenshots.

| OS | Material | Treatment reported |
|---|---|---|
| macOS | `NSVisualEffectView`, standard window material (Liquid Glass on 26) | `systemMaterial` |
| Windows 11 | Mica | `systemMaterial` |
| Windows 10 | none applied | `opaque` |
| Linux | none — blur depends on the compositor and the desktop | `opaque` |

Three rules make this honest rather than decorative:

1. **The treatment is reported, not assumed.** Applying the effect can fail. The shell
   reports what it *achieved*, and a machine that refused it is told `opaque` so the
   window is solid rather than translucent over nothing. This is §2's parity rule
   applied to visuals.
2. **The interface never names an operating system.** It applies
   `data-surface="<treatment>"` and styles that. A guard test fails the build if a
   platform name appears in the frontend or the stylesheet (ADR-0005).
3. **The material is a backdrop, never a contrast mechanism.** Panels, rows and text
   stay fully opaque, and `prefers-reduced-transparency: reduce` returns the ground to
   solid. A person who asked their system to reduce transparency gets that answer, and
   the contrast floors in `design-system.md` §2 hold in every case.

Linux is not given a fake glass layer to match. That would be exactly the parity lie
this document exists to prevent, and a cross-platform imitation would also become the
design — which is worse than three platforms each looking like themselves.

### 4.12 Power / battery

`sysinfo` plus per-OS specifics (IOKit, `GetSystemPowerStatus`, `/sys/class/power_supply`).
Desktops and many VMs have no battery: the capability reports `Unavailable` and the UI
element is absent, not zeroed. Battery state feeds one behaviour: ambient effects switch
off below 20%.

---

## 5. Capability matrix

The honest summary. This table is duplicated in the README so it is impossible to ship a
claim that contradicts it.

| Capability | macOS | Windows | Linux (X11) | Linux (Wayland) |
|---|---|---|---|---|
| Global shortcut | Full | Full | Full | **Unavailable** → `mira --toggle` |
| Tray icon | Full | Full | Degraded (menu-only) | Degraded (menu-only) |
| Tray click events | Full | Full | **Unavailable** | **Unavailable** |
| Launch applications | Full | Full | Full | Full |
| Process enumeration | Full | Full | Full | Full |
| Process cwd | Full (own user) | Degraded | Full (permitting) | Full (permitting) |
| Process termination | Full | Degraded (no true graceful stop) | Full | Full |
| Port enumeration | Full | Full | Full | Full |
| Port attribution | Full | Degraded (exe-path match) | Full | Full |
| Lock detection | Full | Full | Degraded (logind only) | Degraded (logind only) |
| Sleep/wake detection | Full | Full | Full | Full |
| Media now-playing | **Unavailable** | Full | Full | Full |
| Notifications | Full (permission) | Full | Degraded (needs daemon) | Degraded (needs daemon) |
| File watching | Full | Full | Degraded (inotify limits) | Degraded (inotify limits) |
| Reveal in file manager | Full (selects file) | Full (selects file) | Degraded (opens folder) | Degraded (opens folder) |
| Drag files out | Full | Full | Degraded | Degraded |
| Autostart | Full | Full | Full | Full |
| Battery info | Full (laptops) | Full (laptops) | Full (laptops) | Full (laptops) |
| Auto-update | Full (unsigned warns) | Full (unsigned warns) | AppImage only | AppImage only |

Features that **cannot** behave identically on all three, restated for emphasis:
**global shortcuts (Wayland), tray click events (Linux), now-playing (macOS), lock
detection (non-logind Linux), reveal-and-select (Linux), process cwd and graceful
termination (Windows), auto-update (deb/rpm installs).**

---

## 6. Testing the abstraction

- Every trait has a `fake/` implementation; domain tests never touch a real OS.
- Capability resolution is unit-tested against synthetic environments (Wayland vars set,
  logind absent, Docker socket missing, no battery).
- CI runs the full suite on all three OSes. Platform-specific tests are `#[cfg]`-gated
  and **must exist for every `Full` claim in the matrix above** — an unclaimed capability
  is a missing test, and the matrix is the checklist.
- A guard test asserts no `cfg(target_os)` appears outside `mira-platform` (a source
  scan). Adding one elsewhere fails the build.

---

## 7. Adding a platform

The cost of a fourth platform (say, BSD) is: implement the traits, add a capability
resolver, fill a matrix column honestly, and add fixtures. No domain, UI, or schema
change. If a port ever requires touching those layers, this abstraction has failed and
should be fixed rather than worked around.
