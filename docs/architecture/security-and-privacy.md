# Aviora Mira — Security & Privacy

Status: **in progress.** Slices 0 and 1 are built; the rest is the design target. These rules are binding on every future change. A pull
request that weakens one needs an ADR, not a review comment.

---

## 1. Position

Mira is a **local, single-user, accountless desktop application**. It stores what you
tell it about your own machine, in one SQLite file you own, and it talks to no server.

That premise removes most of the usual attack surface — there is no auth to break, no
session to steal, no server to breach, no multi-tenant data to leak. What remains is
concentrated and specific, and this document is about that:

1. Mira **launches processes** the user configured.
2. Mira **terminates processes**.
3. Mira **reads files** and renders their content in a webview.
4. Mira **reads sensitive local configuration** (SSH config, Docker socket).
5. Mira **ships updates**.

Threat model: a **local, non-privileged attacker or malicious content** — a hostile
repository, a crafted filename, a poisoned project config, a malicious `.desktop` file.
Mira is not designed to defend against an attacker who already has your user account or
root; at that point everything is lost regardless.

---

## 2. Default policy

> **No telemetry. No account. No network connection unless the user explicitly asks for
> one.**

A freshly installed Mira, left alone, makes **zero outbound network connections**. Not
for updates (until answered), not for analytics, not for a "check what's new" banner, not
for crash reports, not for fonts or CDNs.

The only things that can ever cause network traffic, each off or unanswered by default:

| Traffic | Default | Control |
|---|---|---|
| Update check | **Unanswered at first run** — user picks | Settings → Advanced |
| SSH reachability probe | **Off**, per host | Per-host toggle |
| Opening a URL in the browser | User action | — |
| Docker daemon | Local socket only, never TCP | — |

Everything else — fonts, icons, styles, scripts — is bundled. The webview loads no
remote origin, and CSP enforces it.

---

## 3. Local-first behaviour

- All state is one SQLite file in the OS app-data directory ([data-model.md](data-model.md) §2).
- Mira is fully functional with networking disabled, permanently.
- Uninstalling and deleting that file removes everything Mira knows. There is no second
  hidden store, no registry sprawl beyond the optional autostart entry, no cloud copy.
- Settings → Privacy offers: delete all sessions, disable session recording, open the
  database's folder, and **erase all Mira data**.

---

## 4. Sensitive information

### 4.1 What Mira never stores

Credentials, tokens, API keys, passwords, SSH private keys or passphrases, environment
variable *values*, file contents, and command output. This is enforced by the schema
having nowhere to put them, and by review.

### 4.2 SSH

The strictest area in the product, because `~/.ssh` is the most valuable directory on a
developer's machine.

- Parsing `~/.ssh/config` is **opt-in** (`ssh.parse_config`, default off) with a first-run
  explanation of exactly what is read.
- Mira reads **names only**: `Host`, `HostName`, `User`, `Port`, `IdentityFile`.
- `IdentityFile` is stored **as a path string**. The file is never opened, read, hashed,
  or transmitted. A guard test fails the build if any code path opens a path that came
  from that column.
- `known_hosts`, `authorized_keys`, agent sockets, and key files are never touched.
- Reachability probing is a bare TCP connect, opt-in per host, 3 s timeout, no
  handshake, no authentication, no banner storage.
- Mira never runs `ssh` itself. "Open a terminal running ssh" hands the command to the
  user's terminal, where their agent and their prompts apply.

### 4.3 Environment variables

Mira does not read, display, or store the environment of other processes, pre-1.0. Command
lines are shown (they are already visible to any process the user owns) and may contain
secrets — so command lines are **truncated in the compact window** and shown fully only
on explicit expansion, and are never persisted.

### 4.4 Filesystem access

- Reads are confined: `peek.read` and shelf operations resolve the canonical path and
  verify it is inside a registered project root or an existing shelf entry. Path
  traversal (`..`), symlink escape, and Windows 8.3-name tricks are defeated by
  canonicalising **and then** checking containment.
- Mira **never writes** to a project directory. It creates no dotfiles, no caches, no
  lockfiles inside your repos.
- Mira **never deletes files**. There is no delete-file action in the product; removing a
  shelf item removes a reference.
- Symlinks are resolved before containment checks; a symlink pointing outside a project
  root is refused with a visible reason.

### 4.5 Docker

Access to the Docker socket is effectively root-equivalent on most systems. Therefore:

- Mira is **read-only**: only `GET` requests are issued, asserted by test.
- The socket is opened lazily, only when a project has a Docker reference and a
  Docker-bearing view is on screen.
- Mira never runs `docker` CLI commands and never execs into a container.
- Absence of Docker is a normal state, not an error, and is never "fixed" by installing
  anything.

---

## 5. Command execution — the primary risk

Launching applications is Mira's most dangerous capability, so the rules are absolute.

1. **No shell. Ever.** Every child process is spawned with an argv array
   (`Command::new(program).args([...])`). There is no `sh -c`, no `cmd /c` (except the
   explicit, quoted `.cmd`/`.bat` handling in the platform layer), no string
   concatenation into a command line, and no `shell` plugin enabled for the frontend.
2. **The schema forbids the unsafe shape.** `commands.args` is a JSON array
   ([data-model.md](data-model.md) §3.4). There is no column where a full command line
   can live.
3. **Placeholders are values, not syntax.** `{path}`, `{file}`, `{line}`, `{url}`,
   `{port}` are substituted into individual argv elements after validation. A path
   containing `; rm -rf ~` is passed as one argument and does nothing.
4. **Programs are resolved, not searched loosely.** A program is an absolute path, a
   detected bundle/desktop id, or a `PATH` lookup performed by Mira — never a string
   handed to an interpreter. **As built (Slice 4), it is narrower than that:** a
   program is a row in a table compiled into the binary, and nothing outside that
   table can be started. There is no setting that points Mira at a binary.
5. **URLs are allowlisted** to `http` and `https`. `file:`, `javascript:`, `data:`, and
   custom schemes are refused. (`file` was in the original list and was dropped when
   the rule was implemented: it would open a local path, which is the thing rule 8
   exists to prevent.) No command accepts a URL at all. **Since slice 4c no command
   accepts a port either** — the interface names a *position* in a list Mira produced,
   or a row id Mira issued, and Mira builds `http://localhost:<port>` in Rust from its
   own observation ([ADR-0020](../adr/0020-workspace-services.md), rules 41–44).
6. **No auto-run.** Mira never executes anything at startup, on project add, on
   detection, or on any event. Every launch is a user action. This is why automation is
   Future work with a trust model attached rather than a quick win.
7. **Project directories are untrusted input.** Mira reads no executable configuration
   from a project — no `.mirarc` that can specify a program to run. Configuration lives in
   Mira's database, entered by the user, not in the repository. This deliberately forgoes
   a convenient feature (per-repo committed config) because it would make cloning a
   hostile repo dangerous.

### Starting an application

Slice 4 makes a workspace actionable, which is the moment §5's "primary risk"
stops being hypothetical. ADR-0013 is the full argument; these are the rules.

10. **A launch is asked for by *kind*.** `workspaces.launch(workspace_id, kind)`
    is the entire privilege the frontend has to start anything. The caller cannot
    name a program, a path, an argument, a bundle, a working directory or a URL —
    there is no parameter for any of them, and a guard test fails the build if one
    appears.
11. **The directory comes from Mira's own database.** It is the project's
    canonical root, resolved by the same `mira-fs` call that gave the project its
    identity, from a row that could only have been created by a native picker
    (rule 8). `LaunchTarget::Directory` is constructed in exactly one place in the
    application shell, and a guard counts the construction sites.
12. **An argv is literals plus one value.** Every element except the last is a
    `&'static str` from the candidate table; the last is the resolved target.
    Enforced by the type and asserted by a test.
13. **macOS launches through the window server, not through a process.**
    `NSWorkspace`, never `open(1)`. Mira does not become the parent of what it
    starts, so it never holds a handle it could use to stop your editor — which is
    what makes rule 6's companion guard ("nothing in this codebase can terminate a
    process") hold even for applications Mira itself started.
14. **Launching writes nothing.** No row, no process id, no "currently open in"
    state. Starting an editor is something that happened, not something a
    workspace becomes.

### Choosing which application

Slice 4b lets a person pick their editor, terminal and browser — the feature whose
obvious implementation is a settings field holding a program.

15. **A choice is a catalogue id, and the catalogue is the validation.** `AppId`
    is one to thirty-two lower-case letters, digits and hyphens, so a path or a
    command line is not a value it can hold. That is defence in depth; the wall is
    that the only thing an id becomes is a row of the table compiled into the
    binary, and `workspaces.prefer` **refuses an id that names no row before
    storing it**. `code` and `sh` are well-formed ids that resolve to nothing.

16. **There is still no column a program could live in.** `0001_init.sql` created
    `applications` (with `program TEXT` — "an executable path, bundle id, or
    .desktop id") and `app_preferences` from `data-model.md` §3.4. Both stay
    empty; `workspace_app_preferences` holds a slug instead, its column checked
    against `[a-z0-9-]` at the schema level, and a guard test asserts no code
    reads or writes either baseline table.

17. **A choice is obeyed or refused, never substituted.** A chosen application
    that is no longer installed produces an error **naming it** — not a different
    editor opening. §6's rule against silent substitution applies with more force
    to a stated choice, not less, because the person would otherwise never learn
    their editor was gone. A guard asserts every failing branch returns an error
    and that no fallback-to-automatic shape is present, and it is proven able to
    fail by injection.

18. **Per workspace, by primary key.** `(workspace_id, kind)` — there is no row
    two workspaces could both read.

19. **The interface never writes an id down.** A guard asserts no frontend source
    contains a catalogue id as a whole string literal. The interface handles ids
    constantly and originates none of them
    ([ADR-0019](../adr/0019-application-preferences.md)).

### Reading history, and copying a commit id

Slice 5a adds the first Git value the interface may **name**, and the first thing
Mira puts on somebody's clipboard. Both are narrowed at the boundary rather than
inside it.

15. **A commit id is a type, not a string.** `CommitId` is four to forty
    hexadecimal characters, validated as it deserialises. `HEAD`, `main@{2}`,
    `refs/heads/main`, `--upload-pack=…`, `../../etc/passwd` and anything carrying
    a metacharacter all fail on the wire, before any code sees them. Mira links
    libgit2 and builds no command line at all ([ADR-0009](../adr/0009-git-via-libgit2.md));
    this is the wall that would still hold if it did.

16. **No command says how much to read.** History is paged, and the page size is
    `mira-git`'s. There is no `limit`, `count`, `depth` or `all` parameter on any
    command — a guard test fails the build if one appears — so a page costs a page
    whatever the repository behind it, and there is no request that makes Mira walk
    a whole history.

17. **The clipboard receives only what Mira read.** `git.copy_commit` names a
    **commit** and a spelling, never the text. The id is resolved in the repository
    first and what is written is what came back, so there is no path by which a
    page could use Mira to place a string of its own choosing on the clipboard of
    the person running it. The platform layer additionally refuses anything longer
    than 128 characters or containing a control character — a clipboard payload
    that can carry a newline is one that can be pasted into a terminal as two
    commands. One construction site exists and a guard test counts it.

18. **The clipboard is written natively.** `NSPasteboard`, the Win32 clipboard, the
    X11 selection. No `pbcopy`, no `clip.exe`, no `xclip` — rule 1 applies here as
    everywhere.

### Drawing a repository

Slice 5b adds a picture of a history, which is the moment somebody could
reasonably wonder whether a row might also be *actionable*. It is not.

23. **The Git layer contains no write operation at all.** Every libgit2 call that
    would change a repository — checkout, reset, commit, branch, tag, remote,
    reference, cherry-pick, rebase, stash, `add_all`, `write_tree` — is named in a
    guard test and none appears in `mira-git/src`. The guard is proven able to
    fail by injection, not merely asserted.

24. **No command acts on a commit.** There is no parameter named `checkout`,
    `merge`, `rebase`, `reset`, `revert`, `cherrypick`, `branch`, `tag`, `push`,
    `pull` or `fetch` on any command. A graph row is something you look at.

25. **The graph reads the same bounded page as the history.** Same walk, same
    cursor, same page size. Parent ids come from the commit objects already
    loaded; reference labelling is capped at `MAX_REFS` and says when the cap was
    reached. There is still no parameter that says how much to read.

26. **No sorted revwalk, ever.** A guard test forbids `Sort::TOPOLOGICAL`,
    `Sort::TIME` and `Sort::REVERSE` in `mira-git/src`. This is a bound rather
    than a preference: a sorted walk preprocesses the whole reachable history
    before yielding its first commit, which is O(history) work to render O(page)
    and would make a large repository a way to make Mira stall
    ([ADR-0015](../adr/0015-graph-lanes.md)).

### Showing what changed

Slice 5c reads file *contents* for the first time outside Peek, which makes it the
first Git read that could be made expensive on purpose.

27. **Five declared limits, none of them nameable.** Files per change set, lines
    per file, bytes per file, bytes per line, and the blob size Mira will diff at
    all — all constants in `mira-git`, all asserted by test. No command carries a
    number that could raise one.

28. **The size gate reads a header, not a file.** libgit2 fills a delta's size
    only once it has loaded the blob, so asking it directly would mean reading the
    very file the ceiling exists to refuse. The size comes from the object header,
    and libgit2 is handed the same ceiling as its own `max_size` — measured at
    0.17 ms to refuse a 30 MB file.

29. **Nothing is truncated silently.** Every limit has a value that says it bit,
    carrying what was shown and what the ceiling was. A diff that quietly stopped
    short would be worse than no diff, because somebody would review a change they
    had not seen.

30. **Binary files are identified, never decoded.** `FileDiff::Binary` carries two
    sizes and no text, and a guard test asserts the variant carries no field named
    for content.

31. **A file is chosen by its ordinal, never by its path.** `git.file_diff` takes
    a position in the list `git.changes` returned. There is no path on the wire,
    so rule 8 needs no exception for this feature — the interface can only ask for
    a file Mira already decided to offer, and an ordinal past the list reads
    nothing. A guard bans the shapes somebody would reach for instead: `blob`,
    `oid`, `pathspec`, `glob`, `filename`, `prefix`.

32. **Every commit still arrives as a validated `CommitId`**, including inside
    `DiffScope`. Nesting a value does not launder it
    ([ADR-0016](../adr/0016-bounded-diffs.md)).

### Tracing a file

Slice 5d is the feature whose defining input is, everywhere else, a path.

33. **A file is named by a `FileSubject`** — a change set Mira produced, a
    position in it, and which side of that change to take the name from. There is
    no field on the struct a path could live in, and a guard test asserts that
    while also asserting positively what *is* there, so it cannot pass vacuously.
    The interface receives a subject and hands it back; it cannot construct one.

34. **No command accepts a pathspec, a glob or a pattern.** `git log -- <path>` is
    how every other tool spells this, which is exactly why the guard names
    `pathspec`, `glob`, `pattern`, `match`, `filter`, `regex` and the rest.

35. **A trace is bounded by commits examined.** File history is inherently
    O(repository history), measured at 740 ms for one file in a
    twenty-thousand-commit repository. `MAX_SCAN` is where one request stops, and
    `ScanStopped::Budget` says how far it got — because a trace that stopped early
    looks exactly like a file with no history unless it says otherwise.

36. **Deciding whether a commit touched a path is two tree lookups**, not a diff.
    A guard test asserts the per-commit test contains no `diff_tree_to_tree`,
    `Patch::`, `changed_files` or `find_similar`: at two thousand commits that is
    the difference between a request in milliseconds and one in seconds
    ([ADR-0017](../adr/0017-file-history.md)).

### Searching a history

Slice 5e is the feature made of the four strings other tools pass straight to
`git log`. None of them is a string here.

37. **A branch is the commit id Mira handed out, never a ref name.** `git.refs`
    returns `RefTip { kind, name, tip }`; the interface shows the name and sends
    back the tip. `main`, `refs/heads/main`, `HEAD`, `origin/main`, `main..dev`,
    `@{upstream}` and `--all` are not values `HistoryFilter::branch` can hold, and
    a wire test asserts each one fails to deserialise. A guard test asserts the
    field's type is `CommitId` rather than `String`.

38. **Author and subject are `Term`s that are compared in Rust.** A `Term`
    validates as it deserialises — trimmed, non-empty, at most 200 characters,
    single line, no control characters — and is then matched against
    `commit.author().name_bytes()` and `commit.summary_bytes()`, values already in
    memory. It never reaches libgit2 and there is no `--author=` or `--grep=`
    anywhere in the workspace; a guard test scans for both.

39. **The file filter is rule 33, unchanged.** Same `FileSubject`, same guard.
    Filtering did not get its own way of naming a file, because a second way to
    name a file is a second thing to keep safe.

40. **A search is bounded by commits examined, and says how far it looked.**
    `MAX_FILTER_SCAN` is one budget for all four filters, because measurement
    showed every predicate costs what the walk costs. A search that spent its
    budget reports `ScanStopped::Budget`, and the interface renders that as *"No
    match yet — nothing matched in the 2 000 commits examined"* rather than as
    "No results" ([ADR-0018](../adr/0018-history-filters.md)).

### Watching a service

Slice 4c lets a workspace say which of its project's services are the work. It is
the feature whose defining input is, everywhere else, a port number.

41. **No command accepts a port, an address, a host, a URL, a pid or a process
    name.** A service is added by `at` — a position in the list
    `workspaces.service_offers` returned — and is opened or forgotten by
    `service_id`, the row id Mira issued when it was added. `live.open_service`
    took `port: u16` before this slice and takes `at: u32` after it; the check
    against the observed list was real, but the parameter was still a number of
    the caller's choosing. A guard test enumerates every banned parameter name.

    The direction is deliberate. A port travels *outward* on every reading — the
    Services panel says `:5173`, and hiding it would make the panel unreadable.
    Outbound values are information; inbound values are instructions.

42. **A watched service is stored as a port and nothing else.**
    `workspace_services` has four columns and no label, process name, pid,
    address, scheme, path or command. Everything except the port is observation,
    belongs to the project, and is read live — so a service that has stopped is
    shown as its port rather than as a memory of what used to be there. A guard
    asserts the column list positively and negatively, and a second asserts that
    no `workspace*` table anywhere carries `executable`, `working_directory`,
    `cmdline` or `pid`.

43. **A workspace's configuration is unreachable from another workspace.** Every
    statement touching `workspace_services` names `workspace_id`, so a sibling's
    row id resolves to nothing rather than to a row — including on the path that
    turns a row id into a port to open. A guard scans the SQL and fails on any
    statement that does not say whose.

44. **A service that is not this project's is never reported as running.** A
    watched port resolves to `Running` only when the listener on it is attributed
    to this workspace's project; anything else — including a listener Mira could
    not attribute at all — is a distinct `Taken` state with no Open button. This
    is [ADR-0013](../adr/0013-launching-applications.md)'s no-substitution rule,
    one layer over. `NotRunning` is likewise a claim Mira may only make after
    looking: before the first scan and after a failed one the row says so instead.

45. **Nothing here can stop anything.** Open is the only action on a watched
    service, and it goes through the same launcher an editor does. Beyond the
    existing "nothing in this codebase can terminate a process" guard, a second
    now asserts that no command is *named* or *parameterised* for stopping,
    killing, terminating, restarting or starting — the shapes somebody would
    reach for first. Termination arrives with its own confirmation and refusal
    design or it does not arrive.

### Doing something in a workspace

Slice 4d gives a workspace a list of things it can be asked to do. It is the
feature that, built the usual way, would be a table of shell commands.

46. **An action is a row in a catalogue compiled into the binary.**
    `mira_core::action::CATALOGUE` is a `const` array of six `&'static str`-bearing
    rows. There is no way to add one except to write it and compile it — not from
    the interface, not from the database, not from a project directory, not from a
    settings file. Those paths do not exist rather than being validated
    ([ADR-0021](../adr/0021-workspace-actions.md)).

47. **No program, no argv, no shell string, anywhere in the feature.** No field,
    column, parameter or wire type holds an executable, a path to one, an
    argument list, a template or a placeholder. An `ActionId` is
    `[a-z0-9-]{1,32}`, so `;`, `&&`, `|`, backticks, `$(`, quotes, spaces,
    slashes and newlines are characters it cannot contain. Wire tests assert that
    `npm run dev`, `pnpm -w build`, `cargo run --release`, `/bin/sh`,
    `open-editor; rm -rf ~` and a dozen others fail to deserialise.

48. **The effect enum is the whole privilege surface, and it is pinned.** Five
    variants — open the project root in an application, reveal it, open this
    workspace's running service, mark the workspace opened, take the scheduler's
    reading now. Every one was already reachable from a button before this slice,
    so **actions add no new way for Mira to affect the machine.** A guard asserts
    the variant list *exactly*, so a sixth fails the build until somebody adds it
    on purpose, and a second guard asserts the dispatch reaches for no process,
    no filesystem write and no address of its own.

49. **The construction sites did not multiply.** Opening in an application goes
    through `workspaces.launch`'s own body and opening a service through
    `workspaces.open_service`'s, so there is still exactly one
    `LaunchTarget::Directory` site and one address site in the application shell.
    The guards that count them pass unchanged, which is the point: reuse is what
    keeps them true.

50. **An unknown identity is a state, never a substitution.** A stored id this
    build has no row for stays on the list, is shown by its id, offers removal
    and not performance, and is never matched to the nearest row. A guard asserts
    the unknown arm reaches for no fallback.

51. **An ambiguous action is impossible rather than disabled.** "Open the running
    service" is available only when exactly one of this workspace's services is
    running; two or more is unavailable *with the count* and a sentence pointing
    at the Services list. Opening the first of three would have been convenient,
    silent and wrong.

52. **`commands` stays empty.** The table from `0001_init.sql` has
    `program TEXT NOT NULL`; a guard fails the build if any code issues `FROM`,
    `INTO`, `UPDATE` or `JOIN` against it.

### Keeping a machine awake

19. **Keep Awake holds an operating-system power request and nothing else.** Mira
    does not post keyboard events, move the pointer, warp the cursor, or
    manufacture activity of any kind. A guard test scans every source file for the
    APIs that would (`CGEventPost`, `SendInput`, `XTestFakeKeyEvent`, `uinput`, and
    the rest); a second asserts that no crate or npm package capable of it —
    `enigo`, `rdev`, `robotjs`, `nut-js` — is anywhere in the dependency tree.

    The distinction is the whole feature. Synthetic input defeats idle detection
    everywhere at once — the screen lock, the session timer, an away status
    somebody else is reading, and any tooling a team relies on — and is
    indistinguishable at the operating-system level from what a malicious program
    does. **An idle screen stays idle.** Keep Awake stops the machine falling
    asleep; it is not a way around a policy and it hides nothing from anybody.

20. **No program is run to change a power setting.** No `caffeinate`, no
    `powercfg`, no `systemd-inhibit`, no `pmset`, no `xset`. A guard test forbids
    all of them from appearing in any source file — including in a reason string,
    because Mira neither runs them nor recommends running them.

21. **The interface cannot ask for an arbitrary duration.** A span is one of four
    words: off, thirty minutes, an hour, until turned off. There is no number
    crossing the boundary, so there is nothing to bound.

22. **Nothing survives the process.** A lock is an OS request owned by this process
    plus a timestamp in memory. There is no table and no column — a guard test
    fails the build if a migration ever mentions one — so quitting releases it, a
    crash releases it, and a restart starts off.
    See [ADR-0014](../adr/0014-keep-awake.md).

### Registering a project root

Adding a project is the moment Mira is granted read access to a directory tree, so it is
treated as a privileged act rather than a form submission.

8. **No command takes a filesystem path from the frontend.** The webview names a
   *project*; it never names a *directory*. `projects.add` has no arguments — it opens a
   native folder picker in Rust on a user gesture and registers what the person chose.
   A compromised page therefore cannot ask Mira to adopt `/` or a sibling's home
   directory, because it has no way to say so. A guard test scans every command
   signature and fails the build if a path-shaped argument appears.
9. **Paths are canonicalised before they mean anything.** `mira-fs` resolves the chosen
   directory to one absolute path with no `..` left in it, and that canonical form is
   the project's identity. Containment checks compare whole path components, never
   string prefixes, so `/home/dev/aviora-secrets` is not inside `/home/dev/aviora`.

### Process termination

- Always confirmed, naming process and PID; never bulk; no keyboard-only fast path.
- Graceful first, force only as a second explicit action.
- Refuses PID 0/1, Mira's own process, and anything the user cannot signal.
- Never escalates privileges. Mira does not ship a helper daemon, does not use
  `sudo`/UAC, and does not request Full Disk Access.

---

## 6. Webview and rendering

The webview renders content that may come from untrusted files, so:

- **Strict CSP:** `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline';
  img-src 'self' asset: data: blob:; connect-src 'self' ipc:; frame-src 'none';
  object-src 'none'; base-uri 'none'`. No remote origin is permitted.
- **No `innerHTML`/`dangerouslySetInnerHTML` for any file-derived content.** Peek
  highlights escaped text.
- **SVG is rendered as an image** (`<img>`/blob URL), never inlined as DOM, so embedded
  scripts cannot execute.
- Filenames, branch names, commit messages, container names, and process command lines
  are all treated as untrusted strings and rendered as text.
- **Minimal Tauri capabilities:** the frontend gets no `fs`, `shell`, `http`, or `process`
  plugin access. Its entire privilege surface is the explicit command list, each with
  validated arguments. `capabilities/*.json` is reviewed like security code, because it
  is.
- Devtools are disabled in release builds.

---

## 7. Plugins *(Future)*

No plugin host exists before 0.6+. When one is designed, it starts from these constraints:

- Plugins declare capabilities up front; there is no ambient authority.
- No plugin gets filesystem, process-spawn, or network access by default.
- Anything a plugin requests is shown to the user at install time in plain language.
- Plugins run out-of-process or in a restricted context — never as arbitrary code inside
  Mira's process with full privileges.
- Distribution and update integrity (signing) are part of the design, not an afterthought.

If those cannot be met, Mira ships no plugin system. A convenient extension mechanism is
not worth turning a local tool into an arbitrary-code-execution vector.

---

## 8. Updates

- Update checks are **opt-in**; the user answers once at first run, and can change it.
- Updates are signature-verified by `tauri-plugin-updater` before installation. An update
  that fails verification is discarded, with a visible error.
- The manifest is static and served over HTTPS. The update check sends the current
  version and platform, and **nothing else** — no identifier, no timestamp beyond the
  request itself, no usage data.
- Linux: only AppImage supports in-app update; `.deb`/`.rpm` users update through their
  package manager, stated in the UI rather than silently doing nothing.
- Signing keys are held by maintainers and never committed. Release signing happens in
  CI with secrets that are not exposed to PR builds from forks.

---

## 9. Telemetry policy

**There is no telemetry.** Not anonymised, not aggregated, not "just crash reports".
No analytics SDK is a dependency; CI fails if a known analytics crate or npm package
enters the dependency tree.

If telemetry is ever proposed, it must be: opt-**in** (never opt-out), off by default,
fully documented as to every field sent, viewable by the user before sending, disableable
permanently in one click, and never required for any feature. There is no current plan to
add it, and none is needed for the product to succeed.

Crash reports are handled the same way: Mira writes a local log the user can attach to an
issue themselves. Nothing is uploaded automatically.

---

## 10. Supply chain

- Lockfiles committed; dependency review on every addition.
- `cargo audit` and `npm audit` in CI; advisories block release.
- `cargo deny` enforces licence and duplicate policy.
- Dependency additions are justified in the PR: what it does, why not std, how maintained.
- The npm surface is kept small; every transitive dependency in a desktop app that reads
  your filesystem is a liability.

---

## 11. Reporting a vulnerability

See [SECURITY.md](../../SECURITY.md). Summary: report privately via GitHub's security
advisories or the address listed there, not in a public issue. Expect acknowledgement
within 72 hours. Fixes for confirmed high-severity issues are prioritised over features.

---

## 12. Guard tests

These run in CI on every commit. They are the mechanism that keeps this document true
after the people who wrote it move on.

| Guarantee | Test |
|---|---|
| No shell execution | Source scan + spawn-path assertion |
| Argv injection is inert | Launch with metacharacter-laden args; assert one literal argument |
| File reads confined to project roots | Attempt traversal, symlink escape, absolute outside path — all refused |
| SSH key files never opened | Fails if any code opens a path sourced from `identity_path` |
| No network when probes disabled | Socket-level assertion during a full app run |
| Docker read-only | Only `GET` requests issued |
| No private frameworks on macOS | `otool -L` check |
| CSP present and strict | Built-artifact inspection |
| No analytics dependencies | Dependency-tree scan |
| Frontend has no fs/shell capability | `capabilities/*.json` snapshot test |
| No arbitrary Git argument | `CommitId` deserialisation refuses revisions, refspecs, paths, flags |
| No full-history load | No `limit`/`count`/`depth`/`all` parameter on any command |
| History is never polled | The scheduler's observers never read history |
| Clipboard carries only a resolved commit | One `Clipboard::new` site; no command takes the text |
| No synthetic keyboard or pointer input | Source scan + dependency-tree scan |
| No power command executed | Source scan for `caffeinate`, `powercfg`, `systemd-inhibit`, … |
| Keep Awake never persists | No migration mentions it; shutdown releases before checkpoint |
| Git layer performs no write | Source scan for every libgit2 write API, proven by injection |
| No command acts on a commit | Command-signature scan for checkout/merge/rebase/reset/… |
| No unbounded topological walk | Source scan for `Sort::*` in `mira-git/src` |
| Lane layout cannot reach a repository | `lanes.rs` mentions no `Repository`, `git2`, `PAGE` or `fs::` |
| No command names a file to read | Command-signature scan for `blob`, `oid`, `pathspec`, `glob`, … |
| Every commit is a validated id | Command-signature scan: `CommitId` or `DiffScope`, never `String` |
| Every diff limit is declared and reported | `diff.rs` carries the constants and the truncation states |
| Binary files are never decoded | `FileDiff::Binary` carries sizes and no content field |
| A size limit reads a header, not a file | `patch.rs` uses `read_header` and libgit2's `max_size` |
| The frontend directory holds no Rust | `src/` contains only `.ts`, `.tsx`, `.css` |
| No command accepts a pathspec or glob | Command-signature scan for `pathspec`, `glob`, `pattern`, … |
| A file subject carries no path | `FileSubject` holds a scope and an ordinal, nothing else |
| A file trace is bounded by commits examined | `MAX_SCAN`, the stop, and the state that reports it |
| A trace reads trees, not diffs | `touched()` contains no diff or patch call |
| A filter carries no ref name, path or pattern | `HistoryFilter` holds a `CommitId`, two `Term`s and a subject |
| No filter value reaches libgit2 | No `--author=`, `--grep=`, `pathspec` or `Revwalk::push_ref` anywhere |
| A branch is chosen by its tip | `RefTip` carries the commit; `branch` is a `CommitId` |
| A search is bounded by commits examined | `MAX_FILTER_SCAN`, the stop, and the state that reports it |
| A preference is a catalogue id and nothing else | `AppPreference` holds a kind and an `AppId`; the column refuses anything else |
| Nothing uses the table that could hold a program | No code reads or writes `applications` / `app_preferences` |
| A choice is chosen from a list Mira produced | `workspaces.prefer` looks the id up before storing it |
| A missing choice never becomes a different application | Every failing branch of `plan` returns an error; no fallback shape |
| The interface never writes a catalogue id down | No frontend source holds one as a string literal |
| Discovery remembers nothing between requests | No cache, lock or `OnceLock` in `applications.rs` |
| A power request says who made it and why | `REASON` names Mira; only the two idle-sleep flags are asked for |
| No command takes a port, address, URL, pid or process | Command-signature scan; proven by injection |
| A watched service stores a port and nothing else | Column list asserted positively and negatively |
| No observation is written down as something to run | No `workspace*` table carries `executable`, `cmdline`, `pid` |
| The URL-shaped table is unreachable | No code reads or writes `expected_ports` |
| A workspace cannot reach another's configuration | Every `workspace_services` statement names `workspace_id` |
| No command stops a service | Command name **and** parameter scan for stop/kill/terminate/… |
| Watching a service starts no observer or clock | No `Instant`, `interval`, `sleep`, `OnceLock` in the resolver |
| An action is a catalogue identity | Command-signature scan: the parameter's type is `ActionId` |
| No action names something to run | The catalogue row has no `program`, `args`, `cwd`, `exec`, … |
| The effects an action can have are pinned | The `Effect` variant list is asserted exactly |
| The table with a program column is unreachable | No code reads or writes `commands` |
| An action Mira no longer has never becomes another | The unknown arm reaches for no fallback |
| An action belongs to the workspace given it | Every `workspace_actions` statement names `workspace_id` |
| Performing an action reaches no new seam | The dispatch shares the one launch and one address site |
