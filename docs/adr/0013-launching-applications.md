# ADR-0013 — Launching applications: a kind, not a command

**Status:** Accepted · 2026-08-20

## Context

Slice 3 gave a workspace an application *context* — this workspace works with an
editor, a terminal, a browser — and deliberately stopped there, on the grounds
that "launching things on a user's machine needs a trust model and a confirmation
flow, and shipping it accidentally as part of *make this workspace active* would
be the worst way to get it" (ADR-0012).

This is that trust model.

The risk is worth stating plainly, because it is the largest one in the product.
A companion that starts applications is one argument away from being a remote
execution primitive: a page in the webview that could name a program, or append
an argument, or supply a path, would have turned Mira into a way to run anything
as the logged-in user. `security-and-privacy.md` §5 has called this "the primary
risk" since before there was code.

The existing design notes had also aged badly. `platform-abstraction.md` §4.2 was
written years ahead of the implementation and proposed `open -b <bundle-id>
--args …` on macOS, plus "the user can always point at a binary" as a fallback.
Both are rejected below.

## Decision

### The interface asks for a kind. It cannot ask for anything else.

```text
Frontend    workspaces.launch(workspace_id, kind)     ← the entire privilege
   ↓
Typed IPC   a row id, and one of three enum variants
   ↓
Service     workspace → project → canonical root
   ↓
Launcher    root + one row from a table compiled into the binary
   ↓
Application
```

Read that as a list of what the caller *cannot* say: not a program, not a path,
not an argument, not a bundle, not a URL, not a working directory. Two values
cross the boundary, and neither is a string that means anything on disk.

Everything else is resolved beneath it:

- **The directory** is the project's canonical root, read from Mira's own
  database — a row that could only have been created by a native folder picker
  on a user gesture (§5 rule 8) — and canonicalised by the same `mira-fs` call
  that gave the project its identity.
- **The application** is the first entry in a fixed, ordered table that this
  machine actually has.
- **The arguments** are a `&'static [&'static str]` from that table, plus exactly
  one value: the resolved directory.

The type says it: an argv is a slice of `&'static str` and one target. There is
no code path that appends a second, and a guard test asserts the count.

Four guards make this structural rather than reviewed. No command may name
something to run; `LaunchTarget::Directory` is constructed in exactly one place
in the application shell, from the resolved root; every `LaunchTarget::WebAddress`
is built from an observed port; and the native launch path contains no process
spawning at all. Each was proven able to fail by injecting the violation it
forbids.

### macOS launches through `NSWorkspace`, not through `open(1)`

The alternative the old notes proposed — `open -b <bundle-id> --args …` — is
rejected on three counts, in increasing order of importance:

1. **It is a command line.** Not a shell one, but a program plus arguments whose
   meaning depends on `open`'s own parsing. `--args` in particular changes how
   everything after it is read. `NSWorkspace` takes two typed `NSURL`s; there is
   no syntax to get wrong.
2. **It makes Mira a parent process.** A child process is a handle, and a handle
   is a capability: something that owns a process can signal it. Quitting Mira
   also becomes a question about what happens to the editor. `NSWorkspace` asks
   the window server to launch the application, which ends up owned by the
   system, so Mira never holds anything it could use to stop your editor —
   complementing the guard that says nothing in the codebase can terminate a
   process at all.
3. **It is what the platform actually does.** Activation, reuse of a running
   instance, and Launch Services' own checks all apply, because it is the same
   call the Finder makes.

The cost is an FFI dependency. It turns out to be no cost at all: `objc2`,
`objc2-app-kit` and `objc2-foundation` are **already in the tree** at those exact
versions, because Tauri uses them, and every binding this needs
(`sharedWorkspace`, `openURLs:withApplicationAtURL:configuration:`, `openURL:`,
`fileURLWithPath:`, `URLWithString:`) is a **safe** function. `mira-platform`
keeps `#![forbid(unsafe_code)]`, unchanged.

Windows and Linux keep argv arrays, which is what those platforms offer.

### A browser opens a service, and the desktop picks the browser

The two other kinds open a *directory*; a browser opens an *address*, and the
only addresses that exist are the ones Mira built from a port it was already
watching (`http://localhost:<port>`, from Slice 2, unchanged and reused).

The browser is therefore resolved differently on purpose. Discovery decides
whether the action exists at all; the **desktop** decides which browser runs.
Opening a service in Chrome because Chrome is installed, when the person's
default is Safari, is exactly the "silently substitute an unrelated application"
failure the slice forbids — Mira's candidate order is a detection heuristic, not
a statement about what someone prefers.

This is also why "Open with" offers two buttons and not three. There is nothing
to hand a browser until something is listening, so the browser action lives on
the service row, and disappears with the service.

### Discovery and openability are two questions

A machine can *have* an editor that Mira has no way to open a folder in. Neovim
is the clean example: it is unarguably an editor, and starting it from a windowed
application produces a headless process nobody can see. So the candidate table
carries `Launch::With(args)` or `Launch::NotFromHere` per row, and:

- the **Context** panel answers *is one here* — "Editor · Neovim";
- **Open with** answers *can Mira open a folder in one* — and on that machine,
  says so instead of offering a button.

Conflating them would have to lie about one of the two. The same field retires
terminals Mira cannot point at a directory: `xterm` has no working-directory
option, and a terminal that opens at `$HOME` is the wrong project, quietly. A
test asserts that every offered terminal, on every platform, carries a way to say
where.

### Launching changes nothing

No row is written. `last_opened_at` is not touched, no process id is recorded, no
"currently open in" state exists. A workspace is what someone stated
(ADR-0012), and starting an editor is not a statement about the workspace — it is
a thing that happened once. Runtime facts stay in the observation layer, which is
`data-model.md` §1 rule 2 applied to a verb instead of a noun.

No scheduler, no observer, no timer. Launching is a function call that returns.

## Alternatives considered

**Let the user point at a binary.** The old platform note's fallback for failed
detection. Rejected: it is precisely `launch_application(command_string)` with a
settings screen in front of it. A machine Mira cannot open an editor on gets a
sentence saying so, and the table gets a row in the next release — which is the
cost of the guarantee, paid where it can be seen.

**A `commands` table with `args_template` and `{path}` placeholders.**
`data-model.md` §3.4 sketches this, and §5 rules 2–3 already constrain it.
Rejected for this slice: substituting into a stored template is a safe mechanism
guarding a *stored string*, and the stored string is the thing worth not having
yet. The table stays empty until a slice needs per-application arguments, and it
will arrive with its own ADR.

**Launching from the workspace service.** Rejected to avoid a `mira-workspaces →
mira-platform` edge for one call. The command orchestrates — resolve, then launch
— exactly as `projects.reveal` has since Slice 1, and the resolution half lives
in `working_directory`, where it is testable without a window.

**Opening the package a service runs from, rather than the project root.** Slice
1.1 knows a monorepo's packages, so `apps/web` is available. Rejected as
premature: nothing has asked which package a workspace is *for* (ADR-0012
rejected `subpath` for the same reason), and an editor opened at a subdirectory
loses the repository. `LaunchTarget` is an enum, so adding it later is additive.

## Consequences

**Good.** The dangerous capability arrived with its boundary already drawn, and
the boundary is four guards and a type rather than a paragraph. A workspace is
finally worth opening: it shows where things stand *and* gets you into them.
macOS launching is native, which removes the last place a command line could have
crept in.

**Costs.** The candidate table is now a maintenance surface with opinions in it —
which terminals take which flag, which editors are windowed. Wrong rows are
user-visible, and there is no way for a user to correct one; that is the deal
struck above. `.desktop`-only installations (Flatpak, Snap) are detected but
cannot be launched, so a Flatpak-only VS Code shows as present and unopenable.

**Bounded.** Deleting `launch.rs`, `macos.rs` and `OpenWith.tsx` removes the
ability to start anything and leaves Slice 3 exactly as it was.
