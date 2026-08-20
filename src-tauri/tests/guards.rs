//! Guard tests — the promises that must not rot.
//!
//! `docs/architecture/architecture.md` §10 and `security-and-privacy.md` §12 list
//! the guarantees that keep this codebase honest after the people who wrote them
//! move on. Each one below is a mechanism, not a comment: breaking the rule fails
//! the build.
//!
//! The guards that need a feature to exist (SSH keys never opened, Docker GET-only,
//! no read outside a project root, no private frameworks on macOS) arrive with the
//! slice that creates the risk, which is the rule these tests are written under.
//! Slice 0 ships the ones whose risk exists today.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri has a parent")
        .to_path_buf()
}

/// Every tracked source file with one of `extensions`, excluding build output,
/// dependencies, generated bindings, and this file (which necessarily names the
/// patterns it forbids).
fn sources(extensions: &[&str]) -> Vec<(PathBuf, String)> {
    fn walk(dir: &Path, extensions: &[&str], out: &mut Vec<(PathBuf, String)>) {
        const SKIP: [&str; 7] = [
            "target",
            "node_modules",
            ".git",
            "dist",
            "gen",
            "bindings",
            "icons",
        ];

        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();

            if path.is_dir() {
                if !SKIP.contains(&name.as_ref()) {
                    walk(&path, extensions, out);
                }
            } else if extensions
                .iter()
                .any(|ext| name.ends_with(&format!(".{ext}")))
                && name != "guards.rs"
            {
                if let Ok(text) = fs::read_to_string(&path) {
                    out.push((path, text));
                }
            }
        }
    }

    let mut out = Vec::new();
    walk(&repo_root(), extensions, &mut out);
    assert!(
        !out.is_empty(),
        "the source scan found nothing — it is broken"
    );
    out
}

/// Source with whole-line comments removed.
///
/// A guard that scans raw text will flag the comment explaining the guard, which
/// is how this function came to exist. Trailing comments are left alone so that a
/// URL in a string literal survives.
fn code_only(text: &str) -> String {
    text.lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !(trimmed.starts_with("//") || trimmed.starts_with("--") || trimmed.starts_with('*'))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn relative(path: &Path) -> String {
    path.strip_prefix(repo_root())
        .unwrap_or(path)
        .display()
        .to_string()
}

// ── No shell, anywhere ───────────────────────────────────────────────────────

#[test]
fn no_code_path_reaches_a_shell() {
    // security-and-privacy.md §5 rule 1. Every child process is spawned with an
    // argv array; there is no `sh -c`, no `cmd /c`, and no string concatenated
    // into a command line. Slice 0 spawns nothing at all, which is the easiest
    // moment to make the rule permanent.
    let forbidden = [
        "Command::new(\"sh\")",
        "Command::new(\"bash\")",
        "Command::new(\"zsh\")",
        "Command::new(\"cmd\")",
        "Command::new(\"powershell\")",
        "sh -c",
        "cmd /c",
        "/bin/sh",
        "shell_execute",
        "ShellExecute",
    ];

    let mut violations = Vec::new();
    for (path, text) in sources(&["rs"]) {
        let text = code_only(&text);
        for needle in forbidden {
            if text.contains(needle) {
                violations.push(format!("{}: {needle}", relative(&path)));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "a shell invocation appeared: {violations:#?}"
    );
}

#[test]
fn the_frontend_has_no_shell_plugin() {
    let manifest = fs::read_to_string(repo_root().join("src-tauri/Cargo.toml")).expect("manifest");
    for plugin in [
        "tauri-plugin-shell",
        "tauri-plugin-fs",
        "tauri-plugin-http",
        "tauri-plugin-sql",
        "tauri-plugin-process",
    ] {
        assert!(
            !manifest.contains(plugin),
            "{plugin} would hand the webview ambient authority the security model denies it"
        );
    }
}

#[test]
fn only_the_platform_layer_starts_a_child_process() {
    // The shell rule above says *how* a process may be started; this says *who*
    // may start one. It is what makes "Mira never runs your package manager" a
    // property of the build rather than a promise: monorepo detection reads
    // manifests as files, and there is nowhere else a `npm install`, a
    // `cargo metadata`, or an `nx graph` could be hiding.
    let allowed = repo_root().join("crates/mira-platform");

    let violations: Vec<String> = sources(&["rs"])
        .into_iter()
        .filter(|(path, text)| {
            !path.starts_with(&allowed) && code_only(text).contains("Command::new")
        })
        .map(|(path, _)| relative(&path))
        .collect();

    assert!(
        violations.is_empty(),
        "spawning a process belongs to mira-platform, and to one reviewed function \
         in it: {violations:#?}"
    );
}

#[test]
fn reading_a_repository_never_writes_to_it() {
    // Mira is a companion, not a build tool. It reads what is in a project and
    // changes nothing — no lockfile touched, no cache directory created, no
    // manifest rewritten. The crates that read a user's repository therefore
    // contain no write at all.
    let readers = ["crates/mira-monorepo", "crates/mira-git"];
    let writes = [
        "fs::write",
        "fs::create_dir",
        "fs::remove_",
        "fs::rename",
        "fs::copy",
        "File::create",
        "OpenOptions",
    ];

    let mut violations = Vec::new();
    for (path, text) in sources(&["rs"]) {
        let relative_path = relative(&path);
        // Test fixtures build repositories to read; the crates themselves do not.
        if !readers
            .iter()
            .any(|reader| relative_path.starts_with(reader))
            || relative_path.contains("/tests/")
        {
            continue;
        }
        let code = code_only(&text);
        for write in writes {
            if code.contains(write) {
                violations.push(format!("{relative_path}: {write}"));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "detection is read-only; a write here would change a repository the user \
         did not ask Mira to change: {violations:#?}"
    );
}

// ── Platform code stays behind the platform boundary ─────────────────────────

#[test]
fn cfg_target_os_appears_only_in_mira_platform() {
    // ADR-0005. The UI knows capabilities, never operating systems, and the same
    // rule applied to the Rust side is what keeps the capability model the single
    // source of truth about what this machine can do.
    let allowed = repo_root().join("crates/mira-platform");

    let violations: Vec<String> = sources(&["rs"])
        .into_iter()
        .filter(|(path, text)| {
            !path.starts_with(&allowed) && code_only(text).contains("cfg(target_os")
        })
        .map(|(path, _)| relative(&path))
        .collect();

    assert!(
        violations.is_empty(),
        "cfg(target_os) belongs only in mira-platform, found in: {violations:#?}"
    );
}

#[test]
fn the_frontend_never_asks_which_operating_system_it_is_on() {
    let violations: Vec<String> = sources(&["ts", "tsx"])
        .into_iter()
        .filter(|(_, text)| {
            let text = code_only(text);
            text.contains("navigator.platform")
                || text.contains("navigator.userAgent")
                || text.contains("process.platform")
                || text.contains("os.platform")
        })
        .map(|(path, _)| relative(&path))
        .collect();

    assert!(
        violations.is_empty(),
        "the UI must ask about capabilities, not operating systems: {violations:#?}"
    );
}

// ── The dependency rule ──────────────────────────────────────────────────────

#[test]
fn no_crate_except_src_tauri_depends_on_tauri() {
    // ADR-0008 rule 2, the load-bearing one: it is what makes the system layer a
    // library that happens to have a Tauri front end, testable with plain
    // `cargo test` and no webview.
    let crates_dir = repo_root().join("crates");
    let mut violations = Vec::new();

    for entry in fs::read_dir(&crates_dir).expect("crates/").flatten() {
        let manifest = entry.path().join("Cargo.toml");
        let Ok(text) = fs::read_to_string(&manifest) else {
            continue;
        };
        let parsed: toml::Value = toml::from_str(&text).expect("valid manifest");

        for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
            let Some(table) = parsed.get(section).and_then(toml::Value::as_table) else {
                continue;
            };
            for name in table.keys() {
                if name == "tauri" || name.starts_with("tauri-") {
                    violations.push(format!("{}: {section}.{name}", relative(&manifest)));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "only src-tauri may depend on tauri: {violations:#?}"
    );
}

#[test]
fn mira_core_depends_on_nothing_in_the_workspace() {
    let text =
        fs::read_to_string(repo_root().join("crates/mira-core/Cargo.toml")).expect("manifest");
    let parsed: toml::Value = toml::from_str(&text).expect("valid manifest");
    let deps = parsed
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .expect("dependencies");

    let internal: Vec<&String> = deps.keys().filter(|k| k.starts_with("mira-")).collect();
    assert!(
        internal.is_empty(),
        "mira-core is the bottom of the graph: {internal:?}"
    );
}

#[test]
fn only_mira_git_depends_on_libgit2() {
    // ADR-0009: libgit2 is reached through the `GitProvider` trait and nowhere
    // else, which is what keeps the eventual move to gitoxide a change inside one
    // crate rather than a change to every caller.
    let mut violations = Vec::new();

    for entry in fs::read_dir(repo_root().join("crates"))
        .expect("crates/")
        .flatten()
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == "mira-git" {
            continue;
        }
        let manifest = entry.path().join("Cargo.toml");
        let Ok(text) = fs::read_to_string(&manifest) else {
            continue;
        };
        if text.contains("git2") {
            violations.push(relative(&manifest));
        }
    }

    let shell = fs::read_to_string(repo_root().join("src-tauri/Cargo.toml")).expect("manifest");
    if shell.contains("git2") {
        violations.push("src-tauri/Cargo.toml".to_owned());
    }

    assert!(
        violations.is_empty(),
        "git2 belongs to mira-git alone: {violations:#?}"
    );
}

// ── The frontend's privilege surface ─────────────────────────────────────────

#[test]
fn the_frontend_capability_list_matches_its_reviewed_snapshot() {
    // security-and-privacy.md §12: capabilities/*.json is reviewed like security
    // code. A change here must be a deliberate edit to this test, not a quiet
    // addition to a JSON file nobody reads.
    let text =
        fs::read_to_string(repo_root().join("src-tauri/capabilities/default.json")).expect("read");
    let parsed: serde_json::Value = serde_json::from_str(&text).expect("valid json");

    let granted: BTreeSet<String> = parsed["permissions"]
        .as_array()
        .expect("permissions array")
        .iter()
        .map(|value| value.as_str().expect("string").to_owned())
        .collect();

    let reviewed: BTreeSet<String> = [
        "core:app:allow-version",
        "core:app:allow-name",
        "core:event:allow-listen",
        "core:event:allow-unlisten",
        "core:window:allow-close",
        "core:window:allow-hide",
        "core:window:allow-set-focus",
    ]
    .into_iter()
    .map(ToOwned::to_owned)
    .collect();

    assert_eq!(
        granted, reviewed,
        "the frontend's privilege surface changed; review it, then update this snapshot"
    );
}

#[test]
fn the_frontend_is_granted_no_privileged_namespace() {
    let text =
        fs::read_to_string(repo_root().join("src-tauri/capabilities/default.json")).expect("read");

    for namespace in [
        "fs:",
        "shell:",
        "http:",
        "process:",
        "sql:",
        "websocket:",
        "upload:",
    ] {
        assert!(
            !text.contains(namespace),
            "the webview must never hold a {namespace} permission"
        );
    }
}

#[test]
fn the_content_security_policy_is_present_and_admits_no_remote_origin() {
    let text =
        fs::read_to_string(repo_root().join("src-tauri/tauri.conf.json")).expect("read config");
    let config: serde_json::Value = serde_json::from_str(&text).expect("valid json");
    let csp = config["app"]["security"]["csp"]
        .as_str()
        .expect("a csp must be configured");

    for directive in [
        "default-src 'self'",
        "script-src 'self'",
        "frame-src 'none'",
        "object-src 'none'",
        "base-uri 'none'",
    ] {
        assert!(
            csp.contains(directive),
            "the CSP is missing `{directive}`: {csp}"
        );
    }

    assert!(
        !csp.contains("https://") && !csp.contains("*"),
        "no remote origin may be permitted; everything is bundled: {csp}"
    );
}

#[test]
fn only_one_window_is_created_at_startup() {
    // product-definition.md sets a <= 150 MB idle RSS budget, and architecture.md
    // §1 treats it as a reason Mira is not an Electron app. Every declared window
    // costs a webview process at launch whether or not it is visible: on macOS a
    // second, hidden window measured ~46 MB of WebContent for a surface most
    // people never open. Windows past the first are created on demand instead.
    let text =
        fs::read_to_string(repo_root().join("src-tauri/tauri.conf.json")).expect("read config");
    let config: serde_json::Value = serde_json::from_str(&text).expect("valid json");
    let windows = config["app"]["windows"]
        .as_array()
        .expect("windows must be configured");

    let labels: Vec<&str> = windows.iter().filter_map(|w| w["label"].as_str()).collect();

    assert_eq!(
        labels,
        ["main"],
        "only the main window may be declared; anything else is built on demand \
         so it does not spend a webview process at launch"
    );
}

#[test]
fn no_command_lets_the_frontend_name_a_path_on_disk() {
    // The webview may say *which project*; it may never say *which directory*.
    // Registering a root is the moment Mira is granted read access to a tree, so
    // that moment belongs to a native file picker driven by the person at the
    // keyboard — not to a JSON argument a compromised page could forge
    // (`security-and-privacy.md` §5).
    //
    // Path *types* are banned outright, and so is any parameter whose name reads
    // like a location. Free text is handled by the reviewed list below, because
    // a `String` is only dangerous when something treats it as a path, and that
    // is a judgement a scanner cannot make.
    let banned_types = [": PathBuf", ": &Path", ": Option<PathBuf>", ": &str"];
    let banned_names = [
        "path",
        "dir",
        "directory",
        "folder",
        "file",
        "root",
        "cwd",
        "target",
        "url",
    ];

    let mut violations = Vec::new();
    for (path, signature) in command_parameters() {
        for banned in banned_types {
            if signature.contains(banned) {
                violations.push(format!("{path}: {signature}"));
            }
        }
        for parameter in parameters_of(&signature) {
            let name = parameter_name(&parameter).to_lowercase();
            if banned_names.iter().any(|banned| name == *banned) {
                violations.push(format!("{path}: {parameter}"));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "a command names a location the frontend chose; the picker must supply it \
         instead: {violations:#?}"
    );
}

#[test]
fn every_command_parameter_is_one_that_has_been_reviewed() {
    // The privilege surface, enumerated. `capabilities/default.json` says what the
    // webview may *call*; this says what it may *say*, and a new parameter fails
    // the build until someone adds it here on purpose.
    //
    // Reviewed, with why each is safe to accept from a page:
    //
    // - `project_id`, `workspace_id` — row ids. Naming a row you do not have is
    //   `NotFound`, not access to anything.
    // - `port`      — a `u16`, and only one Mira is already observing.
    // - `name`, `description` — text the user typed, stored and shown back. Never
    //   resolved against the filesystem.
    // - `kinds`     — a fixed enum; anything else fails to deserialise.
    // - `app`, `state` — injected by Tauri, not sent by the page.
    let reviewed = [
        "app",
        "description",
        "kinds",
        "name",
        "port",
        "project_id",
        "state",
        "workspace_id",
    ];

    let mut unreviewed: Vec<String> = Vec::new();
    for (path, signature) in command_parameters() {
        for parameter in parameters_of(&signature) {
            let name = parameter_name(&parameter);
            if name.is_empty() || reviewed.contains(&name.as_str()) {
                continue;
            }
            unreviewed.push(format!("{path}: {name}"));
        }
    }

    assert!(
        unreviewed.is_empty(),
        "a command accepts something nobody has reviewed; add it to the list above \
         with the reason it is safe: {unreviewed:#?}"
    );
}

/// Every command's parameter list, with the file it came from.
fn command_parameters() -> Vec<(String, String)> {
    let commands = repo_root().join("src-tauri/src/commands");
    let mut found = Vec::new();

    for (path, text) in sources(&["rs"]) {
        if !path.starts_with(&commands) {
            continue;
        }
        for signature in command_signatures(&code_only(&text)) {
            found.push((relative(&path), signature));
        }
    }

    assert!(
        !found.is_empty(),
        "the command scan found nothing — it is broken"
    );
    found
}

/// The parameter list of every `#[tauri::command]` in `code`, as one line each.
fn command_signatures(code: &str) -> Vec<String> {
    let mut signatures = Vec::new();
    let mut lines = code.lines().peekable();

    while let Some(line) = lines.next() {
        if !line.trim_start().starts_with("#[tauri::command]") {
            continue;
        }

        let mut signature = String::new();
        for line in lines.by_ref() {
            signature.push_str(line.trim());
            signature.push(' ');
            if line.contains('{') {
                break;
            }
        }
        signatures.push(between_parentheses(&signature));
    }

    signatures
}

/// What sits between a signature's first `(` and the `)` that closes it.
///
/// Depth-counted rather than `rfind`, because `-> Result<()>` puts a closing
/// parenthesis after the parameter list and the naive version swallowed the
/// return type into the parameters.
fn between_parentheses(signature: &str) -> String {
    let Some(open) = signature.find('(') else {
        return String::new();
    };

    let mut depth = 0_usize;
    for (at, character) in signature.char_indices().skip(open) {
        match character {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return signature[open + 1..at].to_owned();
                }
            }
            _ => {}
        }
    }

    String::new()
}

/// One parameter list, split into parameters.
///
/// Splits on commas at depth zero only: `State<'_, Arc<AppState>>` is one
/// parameter, and a naive split made it look like two.
fn parameters_of(signature: &str) -> Vec<String> {
    let mut parameters = Vec::new();
    let mut depth = 0_i32;
    let mut current = String::new();

    for character in signature.chars() {
        match character {
            '<' | '(' | '[' => depth += 1,
            '>' | ')' | ']' => depth -= 1,
            ',' if depth == 0 => {
                parameters.push(std::mem::take(&mut current));
                continue;
            }
            _ => {}
        }
        current.push(character);
    }
    parameters.push(current);

    parameters
        .into_iter()
        .map(|parameter| parameter.trim().to_owned())
        .filter(|parameter| !parameter.is_empty())
        .collect()
}

/// The name of one parameter, as written.
fn parameter_name(parameter: &str) -> String {
    parameter
        .split(':')
        .next()
        .unwrap_or_default()
        .trim()
        .trim_start_matches("mut ")
        .to_owned()
}

#[test]
fn the_frontend_is_granted_no_file_dialog_permission() {
    // Mira opens the folder picker from Rust, on a user gesture. Handing the
    // webview `dialog:allow-open` would let a page raise a picker of its own.
    let text = fs::read_to_string(repo_root().join("src-tauri/capabilities/default.json"))
        .expect("read capabilities");

    assert!(
        !text.contains("dialog:"),
        "the dialog plugin's frontend permissions must stay unclaimed: {text}"
    );
}

#[test]
fn the_frontend_never_names_an_operating_system_to_choose_a_look() {
    // The macOS material, Mica, and the opaque ground are selected by the
    // treatment `mira-platform` resolved, never by sniffing the platform. This is
    // the visual half of ADR-0005.
    let violations: Vec<String> = sources(&["ts", "tsx", "css"])
        .into_iter()
        .filter(|(_, text)| {
            let code = code_only(text);
            ["macos", "macOS", "windows", "isMac", "isWindows", "darwin"]
                .iter()
                .any(|needle| code.contains(needle))
        })
        .map(|(path, _)| relative(&path))
        .collect();

    assert!(
        violations.is_empty(),
        "the interface selects on a capability or a treatment, never an OS: {violations:#?}"
    );
}

#[test]
fn nothing_can_stop_a_process() {
    // Slice 2 reads processes; stopping one is a later slice with its own
    // confirmation and refusal design (`security-and-privacy.md` §5). Until then
    // the capability is absent from the code rather than merely unused, so
    // "Mira cannot kill your dev server" is a fact about the build.
    let forbidden = [
        "libc::kill",
        ".kill()",
        "signal::kill",
        "TerminateProcess",
        "SIGKILL",
        "SIGTERM",
    ];

    let mut violations = Vec::new();
    for (path, text) in sources(&["rs"]) {
        let code = code_only(&text);
        for needle in forbidden {
            if code.contains(needle) {
                violations.push(format!("{}: {needle}", relative(&path)));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "process termination arrives with its confirmation flow, not before it: {violations:#?}"
    );
}

#[test]
fn no_command_opens_an_arbitrary_url() {
    // Opening a service means opening `http://localhost:<port>` built in Rust
    // from an observed port number. A command taking a URL from the interface
    // would be a way to make Mira open anything, `file://` and custom schemes
    // included (`security-and-privacy.md` §5 rule 5).
    let commands = repo_root().join("src-tauri/src/commands");

    let mut violations = Vec::new();
    for (path, text) in sources(&["rs"]) {
        if !path.starts_with(&commands) {
            continue;
        }
        for signature in command_signatures(&code_only(&text)) {
            if signature.contains("url") || signature.contains("Url") {
                violations.push(format!("{}: {signature}", relative(&path)));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "the interface names a port; Mira builds the URL: {violations:#?}"
    );
}

// ── No telemetry, no phone-home, no background work ──────────────────────────

#[test]
fn no_analytics_dependency_is_in_the_tree() {
    // ADR-0006 and security-and-privacy.md §9. There is no telemetry — not
    // anonymised, not aggregated, not "just crash reports".
    let cargo_lock = fs::read_to_string(repo_root().join("Cargo.lock")).expect("Cargo.lock");
    let npm_lock =
        fs::read_to_string(repo_root().join("package-lock.json")).expect("package-lock.json");

    for name in [
        "sentry",
        "posthog",
        "mixpanel",
        "amplitude",
        "segment-analytics",
        "google-analytics",
        "datadog",
        "bugsnag",
        "rollbar",
        "@amplitude/",
        "@sentry/",
    ] {
        assert!(
            !cargo_lock.contains(name),
            "{name} is an analytics dependency"
        );
        assert!(
            !npm_lock.contains(name),
            "{name} is an analytics dependency"
        );
    }
}

#[test]
fn only_the_scheduler_owns_a_clock() {
    // `architecture.md` §6: one scheduler owns every recurring task, and no
    // module starts its own timer. That is what makes the idle-CPU budget a
    // property of the design — a poller that ignored the gate would have to have
    // its own clock, and there is nowhere to put one.
    //
    // Slice 2 is where this guard earned its keep: the first draft of the live
    // feature grew a second loop in `lib.rs` to emit events on a cadence, and
    // this test refused it. The observers notify after each round instead.
    let scheduler = repo_root().join("crates/mira-scheduler");
    let clocks = [
        "tokio::time::sleep",
        "tokio::time::interval",
        "thread::sleep",
    ];

    let mut violations = Vec::new();
    for (path, text) in sources(&["rs"]) {
        if path.starts_with(&scheduler) || relative(&path).contains("/tests/") {
            continue;
        }
        let code = code_only(&text);
        for clock in clocks {
            if code.contains(clock) {
                violations.push(format!("{}: {clock}", relative(&path)));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "recurring work belongs to mira-scheduler, behind its gate: {violations:#?}"
    );
}

#[test]
fn the_interface_starts_no_clock_of_its_own() {
    // The frontend's half of the same rule. A `setInterval` or a TanStack Query
    // `refetchInterval` would poll regardless of whether Mira's own gate is shut,
    // which is exactly the hidden-window CPU the budget forbids.
    let violations: Vec<String> = sources(&["ts", "tsx"])
        .into_iter()
        .filter(|(_, text)| {
            let code = code_only(text);
            code.contains("setInterval") || code.contains("refetchInterval")
        })
        .map(|(path, _)| relative(&path))
        .collect();

    assert!(
        violations.is_empty(),
        "the interface refreshes when the backend says something moved: {violations:#?}"
    );
}

#[test]
fn no_network_client_is_linked() {
    // A fresh Mira, left alone, makes zero outbound connections
    // (security-and-privacy.md §2). Slice 0 has nothing that could make one, and
    // no HTTP client is compiled in to make it possible by accident.
    let manifest = fs::read_to_string(repo_root().join("src-tauri/Cargo.toml")).expect("manifest");
    for client in ["reqwest", "ureq", "hyper", "isahc", "surf"] {
        assert!(
            !manifest.contains(client),
            "{client} would put an HTTP client in a build that must make no connections"
        );
    }
}

#[test]
fn the_frontend_never_writes_html_directly() {
    // security-and-privacy.md §6: no innerHTML or dangerouslySetInnerHTML for any
    // file-derived content. The webview renders untrusted strings — filenames,
    // branch names, commit messages — as text.
    let violations: Vec<String> = sources(&["ts", "tsx"])
        .into_iter()
        .filter(|(_, text)| {
            let text = code_only(text);
            text.contains("dangerouslySetInnerHTML") || text.contains(".innerHTML")
        })
        .map(|(path, _)| relative(&path))
        .collect();

    assert!(
        violations.is_empty(),
        "untrusted content is rendered as text: {violations:#?}"
    );
}

// ── Scope ────────────────────────────────────────────────────────────────────

#[test]
fn no_crate_exists_for_a_feature_this_slice_does_not_build() {
    // roadmap.md rule 7 and product-scope.md §1 rule 2. An empty crate for an
    // undesigned feature is speculative architecture, and `mira-automation` in
    // particular does not exist before 0.6+.
    let crates: BTreeSet<String> = fs::read_dir(repo_root().join("crates"))
        .expect("crates/")
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();

    for absent in [
        "mira-automation",
        "mira-ssh",
        "mira-docker",
        "mira-media",
        "mira-system",
        "mira-shelf",
        "mira-peek",
        "mira-astra",
    ] {
        assert!(
            !crates.contains(absent),
            "{absent} belongs to a later slice; build the crate when the slice needs it"
        );
    }
}

#[test]
fn nothing_in_the_workspace_is_named_after_astra() {
    // architecture.md §8 rule 1: nothing named `astra` exists in core, services,
    // domain, or the schema.
    let violations: Vec<String> = sources(&["rs", "ts", "tsx", "sql"])
        .into_iter()
        .filter(|(_, text)| code_only(text).to_lowercase().contains("astra"))
        .map(|(path, _)| relative(&path))
        .collect();

    assert!(
        violations.is_empty(),
        "Astra must not appear in code: {violations:#?}"
    );
}
