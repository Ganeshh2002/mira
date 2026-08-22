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

/// The files that must quote what everything else may not contain.
///
/// A guard names the pattern it forbids, and `wire.rs` names the strings an
/// attacker would send — `/bin/sh`, a `file://` URL, a launch target built from
/// nothing. Scanning them would make every guard fail on its own evidence.
///
/// The exclusion is narrow on purpose, and each name has to earn it by the same
/// argument: a test binary with no path into the product, which exists to
/// *assert* the absence of what it mentions.
///
/// `keep_awake.rs` was the third. It asserts that no reason string Mira shows
/// names `caffeinate`, `powercfg`, `systemd-inhibit` or `xdotool` — Keep Awake is
/// a native power request, and the product must neither run those programs nor
/// recommend running them (ADR-0014). Making that assertion requires writing the
/// names down once.
const ADVERSARIAL: [&str; 3] = ["guards.rs", "wire.rs", "keep_awake.rs"];

/// Every tracked source file with one of `extensions`, excluding build output,
/// dependencies, generated bindings, and [`ADVERSARIAL`].
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
                && !ADVERSARIAL.contains(&name.as_ref())
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

/// A repo-relative path, written with `/` on every platform.
///
/// `display()` uses the platform's own separator, so on Windows this returned
/// `src-tauri\tests\guards.rs` while every guard below compares against `/`.
/// That cost two ways at once: `only_the_scheduler_owns_a_clock` stopped skipping
/// test files and failed on a `tokio::time::sleep` that belongs in one, and
/// `reading_a_repository_never_writes_to_it` stopped recognising its own reader
/// crates and skipped **every** file — passing while checking nothing, which is
/// the worse of the two failures because it is silent.
///
/// Normalising here rather than at each comparison keeps the next guard from
/// having to remember, and makes a violation message read the same on all three
/// platforms.
fn relative(path: &Path) -> String {
    match path.strip_prefix(repo_root()) {
        Ok(inside) => inside
            .components()
            .map(|part| part.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/"),
        // Outside the repository, so there is no relative form to write. Shown
        // as the platform writes it, which is what a person would search for.
        Err(_) => path.display().to_string(),
    }
}

#[test]
fn a_repo_relative_path_is_written_the_same_way_on_every_platform() {
    // The guards below are string comparisons against `/`-separated literals, and
    // several of them *skip* files rather than flag them — `/tests/` is how a
    // fixture is excused from a rule about product code. A separator that differed
    // by platform therefore did not merely fail on Windows; it made those guards
    // skip everything and pass while checking nothing.
    //
    // This asserts the shape the rest of the file depends on, so the day somebody
    // simplifies `relative` back to `display()`, one clearly-named test fails
    // instead of six guards quietly going hollow.
    let written = relative(
        &repo_root()
            .join("src-tauri")
            .join("tests")
            .join("guards.rs"),
    );

    assert_eq!(written, "src-tauri/tests/guards.rs");
    assert!(
        !written.contains('\\'),
        "a repo-relative path must never carry a platform separator: {written}"
    );

    // And the two comparisons that depend on it, spelled out.
    assert!(
        written.contains("/tests/"),
        "test files must be recognisable"
    );
    assert!(
        relative(
            &repo_root()
                .join("crates")
                .join("mira-git")
                .join("src")
                .join("walk.rs")
        )
        .starts_with("crates/mira-git"),
        "a crate must be recognisable by its path prefix"
    );
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
    // - `service_id` — a row id Mira issued when a workspace started watching a
    //   service. It is how a watched service is opened or forgotten, and it is
    //   resolved keyed by **both** the row and the workspace, so a sibling
    //   workspace's id matches nothing rather than reaching across (ADR-0020).
    // - `name`, `description` — text the user typed, stored and shown back. Never
    //   resolved against the filesystem.
    // - `kind`, `kinds` — a fixed enum; anything else fails to deserialise.
    //   `kind` is what a launch is asked for by: editor, terminal or browser, and
    //   never a program, a path or an argument (see the guards below).
    // - `commit`, `cursor` — a `CommitId`: four to forty hexadecimal characters,
    //   checked as it deserialises. It cannot spell `HEAD`, a refspec, a path or
    //   a flag, so it names an object in a repository or it does not arrive.
    //   `cursor` says *continue the history from here*; `commit` says *read this
    //   one*. Naming a commit that is not there is a state, not access to
    //   anything (`mira_git::CommitId`, and `wire.rs`).
    // - `form` — a fixed two-variant enum: the short spelling of a commit id or
    //   the full one. It selects between two values Mira already read from the
    //   repository; it does not supply either.
    // - `span` — a fixed four-variant enum: off, thirty minutes, an hour, until
    //   turned off. The interface cannot name a duration, so there is no number
    //   here to bound (ADR-0014).
    // - `scope` — a two-variant enum: one commit, or the working tree. The
    //   commit arm carries a `CommitId`, validated on the same terms as anywhere
    //   else, so the whole space this parameter admits is "a commit that exists"
    //   or "what is on disk right now".
    // - `at` — an ordinal in a list Mira produced. **Not a path, and not a
    //   port.** It names a file in a change set (ADR-0016) and, since slice 4c,
    //   a service in the list of observations Mira offered for one project
    //   (ADR-0020). Both times the interface can only ask for something Mira
    //   already decided to offer, and an ordinal past the list is a stale
    //   selection rather than an attempt at anything.
    // - `subject` — a `FileSubject`: a scope, an ordinal in it, and which side of
    //   that change to take the name from. It is how a file is named for a
    //   history trace, and it has no field that holds a path — the interface
    //   receives one and hands it back, and cannot describe a file Mira has not
    //   already offered it (ADR-0017).
    // - `wanted` — a `HistoryFilter`: a branch as a **commit id**, a file as a
    //   change-set position, and author and subject as validated terms that are
    //   only ever compared in Rust. There is no ref name, no path and no pattern
    //   in it; nothing it holds reaches libgit2 as an argument (ADR-0018).
    // - `app`, `state` — injected by Tauri, not sent by the page.
    let reviewed = [
        "app",
        "at",
        "commit",
        "cursor",
        "description",
        "form",
        "kind",
        "kinds",
        "name",
        "project_id",
        "scope",
        "service_id",
        "span",
        "state",
        "subject",
        "wanted",
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

// ── Launching applications ───────────────────────────────────────────────────

#[test]
fn no_command_names_something_to_run() {
    // The companion to the path rule, for the slice that starts applications.
    // A workspace is opened by *kind* — editor, terminal, browser — and the
    // program, the bundle and every argument are resolved beneath the IPC
    // boundary from a table compiled into the binary. A command that accepted a
    // program name, or an argument to append, would be a way to make Mira run
    // something of the caller's choosing, which is the one thing this slice must
    // not become (`security-and-privacy.md` §5 rule 1).
    let banned = [
        "command",
        "program",
        "executable",
        "exe",
        "binary",
        "argv",
        "args",
        "arguments",
        "application",
        "bundle",
        "launch",
        "shell",
    ];

    let mut violations = Vec::new();
    for (path, signature) in command_parameters() {
        for parameter in parameters_of(&signature) {
            let name = parameter_name(&parameter).to_lowercase();
            if banned.contains(&name.as_str()) {
                violations.push(format!("{path}: {parameter}"));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "a command lets the caller name what to run; the kind is the only thing it \
         may choose: {violations:#?}"
    );
}

#[test]
fn the_only_directory_a_launch_can_reach_is_a_project_root() {
    // `LaunchTarget::Directory` is the one type that carries a path into the
    // launcher, and there is exactly one place in the application shell that
    // builds one — from `working_directory`, which reads a project row and
    // canonicalises it. A second construction site is where a path from
    // somewhere else would enter, so the count is the guard.
    let built: Vec<String> = sources(&["rs"])
        .into_iter()
        .filter(|(path, _)| path.starts_with(repo_root().join("src-tauri")))
        .flat_map(|(path, text)| {
            code_only(&text)
                .lines()
                .filter(|line| line.contains("LaunchTarget::Directory("))
                .map(|line| format!("{}: {}", relative(&path), line.trim()))
                .collect::<Vec<_>>()
        })
        .collect();

    assert_eq!(
        built.len(),
        1,
        "a launch directory is built somewhere new; it must come from a project \
         root and nowhere else: {built:#?}"
    );
    assert!(
        built[0].contains("root"),
        "the one directory handed to an application is the resolved project root: {built:#?}"
    );
}

#[test]
fn every_address_a_browser_receives_is_one_mira_built() {
    // The same rule as `no_command_opens_an_arbitrary_url`, one layer down:
    // that one says no command *accepts* a URL, this says every URL that reaches
    // the launcher was constructed here from an observed port
    // (`security-and-privacy.md` §5 rule 5).
    let mut violations = Vec::new();
    for (path, text) in sources(&["rs"]) {
        if !path.starts_with(repo_root().join("src-tauri")) {
            continue;
        }
        for line in code_only(&text).lines() {
            if line.contains("LaunchTarget::WebAddress(") && !line.contains("localhost(") {
                violations.push(format!("{}: {}", relative(&path), line.trim()));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "an address reaches the browser without being built from a port: {violations:#?}"
    );
}

#[test]
fn starting_an_application_on_macos_starts_no_process() {
    // ADR-0013. `open -a "Some App" <path>` would be a command line, with
    // quoting to get wrong and a child process Mira would own; `NSWorkspace`
    // hands two typed URLs to the window server and owns nothing. The native
    // module must contain no trace of the other approach.
    let native = fs::read_to_string(repo_root().join("crates/mira-platform/src/macos.rs"))
        .expect("macos.rs");
    let code = code_only(&native);

    for absent in ["Command", "process::", "\"open\"", "spawn", "osascript"] {
        assert!(
            !code.contains(absent),
            "{absent} appeared in the native launch path, which exists precisely to \
             avoid it"
        );
    }
}

#[test]
fn the_interface_never_names_an_application() {
    // §6: never silently substitute an unrelated application. The interface
    // cannot substitute anything, because it does not know any application's
    // name — every name on screen came from discovery over the IPC boundary. A
    // hardcoded "Visual Studio Code" in a button would be a claim the machine
    // had not been asked to confirm.
    let table = fs::read_to_string(repo_root().join("crates/mira-platform/src/applications.rs"))
        .expect("applications.rs");

    // Only the rows of the table: every candidate is declared by one of four
    // constructors, so the names are exactly what follows the first quote on
    // those lines.
    let names: BTreeSet<String> = table
        .lines()
        .map(str::trim)
        .filter(|line| {
            [
                "bundle(",
                "program(",
                "opens_at(",
                "found_only(",
                "desktop(",
            ]
            .iter()
            .any(|constructor| line.starts_with(constructor))
        })
        .filter_map(|line| {
            let quoted = line.split_once('"')?.1;
            Some(quoted.split_once('"')?.0.to_owned())
        })
        // "Terminal" is both an application on macOS and the name of a *kind*.
        // The interface says the kind, and no scanner can tell the two apart, so
        // the kind vocabulary keeps its own words.
        .filter(|name| !["Editor", "Terminal", "Browser"].contains(&name.as_str()))
        .collect();

    assert!(
        names.len() > 20,
        "the application table was not read: {names:#?}"
    );

    let mut violations = Vec::new();
    for (path, text) in sources(&["ts", "tsx"]) {
        // A test names applications on purpose: its fixture stands in for
        // discovery, which is the only thing that may produce a name.
        if relative(&path).contains(".test.") {
            continue;
        }
        for name in &names {
            if text.contains(name.as_str()) {
                violations.push(format!("{}: {name}", relative(&path)));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "the interface names an application instead of showing what was found: \
         {violations:#?}"
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

// ── Reading history ──────────────────────────────────────────────────────────

#[test]
fn no_command_lets_the_frontend_choose_how_much_git_to_read() {
    // The companion to the path rule, for the slice that walks a repository.
    // History is paged, and the page size belongs to `mira-git` — a parameter
    // saying *how many* would be a way to ask Mira to walk an entire repository
    // on demand, which is the one thing this slice must not become. The interface
    // says *continue from here*, and nothing else.
    let banned = [
        "limit", "count", "max", "size", "depth", "page", "all", "since", "until", "n",
    ];

    let mut violations = Vec::new();
    for (path, signature) in command_parameters() {
        for parameter in parameters_of(&signature) {
            let name = parameter_name(&parameter).to_lowercase();
            if banned.contains(&name.as_str()) {
                violations.push(format!("{path}: {parameter}"));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "a command lets the caller choose how much history to read; the page size \
         is Mira's: {violations:#?}"
    );
}

#[test]
fn history_is_read_on_demand_and_never_on_a_clock() {
    // `information-architecture.md` §3 puts history in the **on-view** tier: it
    // is read when a person opens the surface, refreshes, or asks for more. An
    // observer that walked every project's history every five seconds would be
    // the most expensive thing in the product, and the gate would not save it —
    // the gate is open whenever a window is.
    //
    // The scheduler's observers are one file. This asserts history is not in it.
    let observers =
        fs::read_to_string(repo_root().join("src-tauri/src/observers.rs")).expect("observers");
    let code = code_only(&observers);

    for reading in ["history(", "commit(", "CommitPage", "CommitId"] {
        assert!(
            !code.contains(reading),
            "the scheduler must not read history; `{reading}` appeared in observers.rs"
        );
    }
}

#[test]
fn the_page_size_is_a_constant_and_not_a_number_in_a_call() {
    // A page that is 25 in one place and 100 in another is a page size nobody
    // owns. `mira-git` declares it once, and the walk is the only thing that
    // reads it.
    let history =
        fs::read_to_string(repo_root().join("crates/mira-git/src/history.rs")).expect("history");
    assert!(
        code_only(&history).contains("pub const PAGE: usize"),
        "the page size must be a named constant in mira-git"
    );

    let walk = fs::read_to_string(repo_root().join("crates/mira-git/src/walk.rs")).expect("walk");
    assert!(
        code_only(&walk).contains("PAGE + 1"),
        "the walk reads one more than a page to find the next cursor, and no more"
    );
}

// ── The clipboard ────────────────────────────────────────────────────────────

#[test]
fn only_the_platform_layer_touches_the_clipboard() {
    // Same rule as libgit2 and `Command::new`: the OS-specific call lives in
    // `mira-platform` and everything above it asks for a capability (ADR-0005).
    let allowed = repo_root().join("crates/mira-platform");

    let mut violations: Vec<String> = sources(&["rs"])
        .into_iter()
        .filter(|(path, text)| !path.starts_with(&allowed) && code_only(text).contains("arboard"))
        .map(|(path, _)| relative(&path))
        .collect();

    for entry in fs::read_dir(repo_root().join("crates"))
        .expect("crates/")
        .flatten()
    {
        if entry.file_name().to_string_lossy() == "mira-platform" {
            continue;
        }
        let manifest = entry.path().join("Cargo.toml");
        if fs::read_to_string(&manifest).is_ok_and(|text| text.contains("arboard")) {
            violations.push(relative(&manifest));
        }
    }

    let shell = fs::read_to_string(repo_root().join("src-tauri/Cargo.toml")).expect("manifest");
    if shell.contains("arboard") {
        violations.push("src-tauri/Cargo.toml".to_owned());
    }

    assert!(
        violations.is_empty(),
        "the clipboard is a platform capability, not a helper: {violations:#?}"
    );
}

#[test]
fn nothing_puts_text_the_frontend_chose_on_the_clipboard() {
    // The interface asks to copy **a commit**, and Mira resolves that commit in
    // the repository before writing anything. There is therefore no command
    // through which a page could place a string of its own choosing on somebody's
    // clipboard. One construction site is the guard, as with `LaunchTarget`.
    let built: Vec<String> = sources(&["rs"])
        .into_iter()
        .filter(|(path, _)| path.starts_with(repo_root().join("src-tauri")))
        .flat_map(|(path, text)| {
            code_only(&text)
                .lines()
                .filter(|line| line.contains("Clipboard::new("))
                .map(|line| format!("{}: {}", relative(&path), line.trim()))
                .collect::<Vec<_>>()
        })
        .collect();

    assert_eq!(
        built.len(),
        1,
        "the clipboard is written in one reviewed place, from a commit Mira read: \
         {built:#?}"
    );

    // And no command takes the value itself. A `value`, `text` or `content`
    // parameter would be exactly the gadget this guard exists to prevent.
    let banned = ["value", "text", "content", "clipboard", "sha", "id"];
    let mut violations = Vec::new();
    for (path, signature) in command_parameters() {
        for parameter in parameters_of(&signature) {
            let name = parameter_name(&parameter).to_lowercase();
            if banned.contains(&name.as_str()) {
                violations.push(format!("{path}: {parameter}"));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "a command accepts the text to copy; it must name a commit instead: \
         {violations:#?}"
    );
}

// ── Keep Awake ───────────────────────────────────────────────────────────────

#[test]
fn nothing_simulates_a_keystroke_or_a_pointer_movement() {
    // ADR-0014, and the line Keep Awake must never cross. Preventing sleep is
    // asking the operating system a question it has a public answer for.
    // Manufacturing input is impersonating the person at the keyboard: it defeats
    // idle detection everywhere, including in tooling somebody else is relying on,
    // and it is indistinguishable from what a malicious program does.
    //
    // Mira does neither, and the absence is a property of the build: there is no
    // code path to any of these, and no crate in the tree that offers one.
    let forbidden = [
        "CGEventPost",
        "CGEventCreateKeyboardEvent",
        "CGEventCreateMouseEvent",
        "CGWarpMouseCursorPosition",
        "IOHIDPostEvent",
        "SendInput",
        "keybd_event",
        "mouse_event",
        "SetCursorPos",
        "XTestFakeKeyEvent",
        "XTestFakeMotionEvent",
        "XTestFakeButtonEvent",
        "uinput",
        "UI_SET_KEYBIT",
    ];

    let mut violations = Vec::new();
    for (path, text) in sources(&["rs", "ts", "tsx"]) {
        let code = code_only(&text);
        for needle in forbidden {
            if code.contains(needle) {
                violations.push(format!("{}: {needle}", relative(&path)));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Keep Awake asks the operating system; it never pretends to be the person \
         at the keyboard: {violations:#?}"
    );
}

#[test]
fn no_crate_that_synthesises_input_is_in_the_tree() {
    // The source scan above says no code path reaches such an API. This says the
    // API is not even linked, so one could not be reached by accident.
    let lock = fs::read_to_string(repo_root().join("Cargo.lock")).expect("Cargo.lock");
    let packages = fs::read_to_string(repo_root().join("package-lock.json")).expect("npm lock");

    for crate_name in [
        "\"enigo\"",
        "\"autopilot\"",
        "\"inputbot\"",
        "\"rdev\"",
        "\"mouse-rs\"",
        "\"uinput\"",
    ] {
        assert!(
            !lock.contains(&format!("name = {crate_name}")),
            "{crate_name} synthesises input; nothing in Mira may depend on it"
        );
    }

    for package in ["robotjs", "@nut-tree/nut-js", "node-key-sender"] {
        assert!(
            !packages.contains(&format!("node_modules/{package}")),
            "{package} synthesises input; nothing in Mira may depend on it"
        );
    }
}

#[test]
fn no_power_setting_is_changed_by_running_a_program() {
    // §5 rule 1 applied to this slice. Every documented way to keep a machine
    // awake from a shell is named here, because each is what somebody reaches for
    // when the native API is inconvenient. Keep Awake holds an operating-system
    // request through a typed binding instead (`mira-platform/src/inhibit.rs`).
    let forbidden = [
        "caffeinate",
        "powercfg",
        "systemd-inhibit",
        "gnome-session-inhibit",
        "pmset",
        "xset",
        "SetThreadExecutionState",
        "xdg-screensaver",
    ];

    let mut violations = Vec::new();
    for (path, text) in sources(&["rs", "ts", "tsx", "json"]) {
        let code = code_only(&text);
        for needle in forbidden {
            if code.contains(needle) {
                violations.push(format!("{}: {needle}", relative(&path)));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "keeping a machine awake is a native request, never a program: {violations:#?}"
    );
}

#[test]
fn nothing_records_that_the_machine_was_kept_awake() {
    // ADR-0014's lifecycle rule, made structural: a lock cannot survive a restart
    // because there is nowhere to write it down. `data-model.md` §1 rule 2 draws
    // the same line — stated things are stored, observed and held things are not.
    let migrations = repo_root().join("crates/mira-db/migrations");
    let Ok(entries) = fs::read_dir(&migrations) else {
        panic!("migrations must exist");
    };

    for entry in entries.flatten() {
        let text = fs::read_to_string(entry.path()).unwrap_or_default();
        let lowered = text.to_lowercase();
        // Not "sleep": `sessions.cause` records that a session paused because
        // the machine slept, which is an observation about the past and not a
        // lock held over the future.
        for column in ["awake", "inhibit", "caffeine"] {
            assert!(
                !lowered.contains(column),
                "{} mentions {column}; Keep Awake has no table and must not get one",
                relative(&entry.path())
            );
        }
    }
}

#[test]
fn keep_awake_is_released_on_the_way_out() {
    // The one lifecycle rule a test cannot observe without changing a real power
    // setting, so it is asserted structurally instead: shutdown releases, and it
    // does so before anything else that could fail.
    let shell = fs::read_to_string(repo_root().join("src-tauri/src/lib.rs")).expect("lib.rs");
    let code = code_only(&shell);

    let shutdown = code
        .split_once("fn shutdown")
        .map(|(_, rest)| rest.to_owned())
        .expect("a shutdown function");

    assert!(
        shutdown.contains("awake.shutdown()"),
        "quitting must give the machine back"
    );

    let release = shutdown.find("awake.shutdown()").expect("release");
    let checkpoint = shutdown.find("checkpoint()").expect("checkpoint");
    assert!(
        release < checkpoint,
        "the power request is released before the database is checked in, so a \
         failure there cannot leave a machine pinned awake"
    );
}

#[test]
fn only_one_clock_exists_even_for_a_deadline() {
    // `only_the_scheduler_owns_a_clock` above bans a timer outside
    // `mira-scheduler`. Keep Awake needs one — a span that ends has to end — so
    // the one-shot lives beside the scheduler rather than in the feature that
    // wanted it. This asserts it is there, and that it is genuinely one-shot.
    let scheduler = fs::read_to_string(repo_root().join("crates/mira-scheduler/src/lib.rs"))
        .expect("scheduler");
    let code = code_only(&scheduler);

    assert!(
        code.contains("pub struct Deadline"),
        "a one-shot timer belongs to the crate that owns every clock"
    );

    let awake = fs::read_to_string(repo_root().join("src-tauri/src/awake.rs")).expect("awake.rs");
    let awake = code_only(&awake);
    assert!(
        awake.contains("Deadline::in_time"),
        "Keep Awake's timeout is that one-shot"
    );
    for loop_shaped in ["loop {", "interval(", "while "] {
        assert!(
            !awake.contains(loop_shaped),
            "Keep Awake must not poll: `{loop_shaped}` appeared in awake.rs"
        );
    }
}

// ── The graph is a picture, not a client ─────────────────────────────────────

#[test]
fn the_git_layer_contains_no_write_operation_at_all() {
    // Slice 5b draws the shape of a history, which is the moment somebody could
    // reasonably wonder whether a row might also be *actionable*. It is not, and
    // the absence is structural rather than a matter of restraint: every libgit2
    // call that would change a repository is named here, and none of them appears
    // in this crate's sources.
    //
    // `prd.md` FR-3.4 and `security-and-privacy.md` §5. Fixtures are excluded —
    // `tests/` builds repositories in order to read them back, which is the whole
    // method of this crate's suite.
    // Named precisely enough to mean libgit2 and not Rust: `push(` would match
    // `Vec::push`, `tag(` would match `is_tag()`, and a guard that cries wolf is
    // a guard somebody deletes. These are receiver-qualified or unambiguous.
    let forbidden = [
        "repo.checkout_head",
        "repo.checkout_tree",
        "repo.checkout_index",
        "repo.set_head",
        "repo.reset",
        "repo.commit(",
        "repo.branch(",
        "repo.branch_remote_name",
        "repo.tag(",
        "repo.tag_lightweight",
        "repo.tag_delete",
        "repo.remote(",
        "repo.remote_add",
        "repo.find_remote",
        "repo.reference(",
        "repo.reference_symbolic",
        "repo.apply(",
        "repo.cleanup_state",
        "Repository::init",
        "Repository::clone",
        "cherrypick",
        "rebase",
        "stash_",
        "merge_commits",
        "merge_trees",
        "RemoteCallbacks",
        "PushOptions",
        "FetchOptions",
        "CheckoutBuilder",
        "IndexAddOption",
        ".add_all(",
        ".write_tree(",
        ".set_target(",
        ".remove_all(",
    ];

    let git = repo_root().join("crates/mira-git/src");

    let mut violations = Vec::new();
    for (path, text) in sources(&["rs"]) {
        if !path.starts_with(&git) {
            continue;
        }
        let code = code_only(&text);
        for needle in forbidden {
            if code.contains(needle) {
                violations.push(format!("{}: {needle}", relative(&path)));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "the graph draws a repository; it never changes one: {violations:#?}"
    );
}

#[test]
fn no_command_can_act_on_a_commit() {
    // The other half of the same promise, at the boundary. A graph row is a
    // thing you look at. There is no command through which the interface could
    // ask Mira to do anything *to* one, and a parameter named for such an action
    // would be the first sign of one appearing.
    let banned = [
        "checkout",
        "merge",
        "rebase",
        "reset",
        "revert",
        "cherrypick",
        "stage",
        "unstage",
        "branch",
        "tag",
        "remote",
        "push",
        "pull",
        "fetch",
        "ref",
        "refspec",
        "message",
    ];

    let mut violations = Vec::new();
    for (path, signature) in command_parameters() {
        for parameter in parameters_of(&signature) {
            let name = parameter_name(&parameter).to_lowercase();
            if banned.contains(&name.as_str()) {
                violations.push(format!("{path}: {parameter}"));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "a command acts on a commit; the graph only draws one: {violations:#?}"
    );
}

#[test]
fn drawing_the_graph_never_asks_for_a_sorted_walk() {
    // The measured constraint, made structural (ADR-0015). libgit2's sorted
    // revwalks preprocess the entire reachable history before yielding a single
    // commit — measured at 3.4 ms, 34 ms and 426 ms for the same twenty-five rows
    // on repositories of 100, 1,000 and 10,000 commits, against a flat ~0.9 ms
    // unsorted. A bounded interface over an unbounded traversal is the one thing
    // this slice must not become.
    //
    // `tests/performance.rs` names both sorts, because measuring the thing Mira
    // refuses is how the refusal stays justified.
    let git = repo_root().join("crates/mira-git/src");

    let mut violations = Vec::new();
    for (path, text) in sources(&["rs"]) {
        if !path.starts_with(&git) {
            continue;
        }
        let code = code_only(&text);
        for sorting in ["Sort::TOPOLOGICAL", "Sort::TIME", "Sort::REVERSE"] {
            if code.contains(sorting) {
                violations.push(format!("{}: {sorting}", relative(&path)));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "a sorted revwalk reads the whole history to draw one page: {violations:#?}"
    );
}

#[test]
fn the_graph_is_laid_out_over_a_window_and_never_a_repository() {
    // Lane assignment takes a slice of nodes and returns a slice of placements.
    // It has no repository, no cursor and no way to ask for another commit, which
    // is what makes "the graph is computed over what is on screen" a fact about
    // the signature rather than a claim about the caller.
    let lanes =
        fs::read_to_string(repo_root().join("crates/mira-git/src/lanes.rs")).expect("lanes.rs");
    let code = code_only(&lanes);

    assert!(
        code.contains("pub fn layout(nodes: &[Node]) -> Layout"),
        "the layout takes a window and nothing else"
    );
    for reaching in ["Repository", "git2", "PAGE", "fs::"] {
        assert!(
            !code.contains(reaching),
            "lane assignment must stay pure; `{reaching}` appeared in lanes.rs"
        );
    }
}

#[test]
fn the_lane_cap_is_a_constant_and_the_interface_is_told_when_it_bites() {
    // `design-system.md` §8 caps the picture at eight lanes. Folding past that is
    // honest only if it is *said*: a graph silently drawing two branches on one
    // line would be a picture of something that is not there.
    let graph =
        fs::read_to_string(repo_root().join("crates/mira-git/src/graph.rs")).expect("graph.rs");
    let code = code_only(&graph);

    assert!(code.contains("pub const MAX_LANES: u32 = 8"));
    assert!(code.contains("pub const MAX_REFS: usize = 500"));
    assert!(
        code.contains("collapsed: bool"),
        "the interface has to be able to say the picture was folded"
    );
    assert!(
        code.contains("refs_truncated: bool"),
        "and that a label may be missing"
    );
}

#[test]
fn the_frontend_directory_contains_only_frontend_source() {
    // `src/` is the React application. Rust lives in `crates/` and `src-tauri/`,
    // and a `.rs` file here compiles into nothing, ships in nothing, and is read
    // by nobody — which is exactly why one can sit there unnoticed.
    //
    // This guard exists because one did. A scratch program written while
    // measuring the graph landed in `src/main.rs` and was committed: every gate
    // stayed green, because no gate was looking. Vite ignores it, `tsc` ignores
    // it, and Cargo never sees it.
    let frontend = repo_root().join("src");
    let allowed = ["ts", "tsx", "css"];

    let mut strays = Vec::new();
    for (path, _) in sources(&["rs", "js", "jsx", "mjs", "cjs", "toml", "lock"]) {
        if !path.starts_with(&frontend) {
            continue;
        }
        let extension = path
            .extension()
            .map(|found| found.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !allowed.contains(&extension.as_str()) {
            strays.push(relative(&path));
        }
    }

    assert!(
        strays.is_empty(),
        "src/ is the React application; these belong in crates/ or src-tauri/, or \
         nowhere: {strays:#?}"
    );
}

// ── Diffs are a view, never an edit ──────────────────────────────────────────

#[test]
fn no_command_lets_the_frontend_name_a_file_to_read() {
    // The rule that makes bounded diffs safe as well as fast. A diff needs to
    // know *which* file — and the answer is an **ordinal in a list Mira
    // produced**, never a path. `no_command_lets_the_frontend_name_a_path_on_disk`
    // already bans path-shaped parameters; this bans the shapes somebody would
    // reach for when they wanted a path and knew that rule existed.
    let banned = [
        "blob",
        "oid",
        "object",
        "entry",
        "filename",
        "basename",
        "relative",
        "pathspec",
        "glob",
        "pattern",
        "prefix",
        "location",
        "source",
        "destination",
    ];

    let mut violations = Vec::new();
    for (path, signature) in command_parameters() {
        for parameter in parameters_of(&signature) {
            let name = parameter_name(&parameter).to_lowercase();
            if banned.contains(&name.as_str()) {
                violations.push(format!("{path}: {parameter}"));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "a command names a file to read; the ordinal in a bounded change list is \
         the only way to choose one: {violations:#?}"
    );
}

#[test]
fn every_commit_a_command_accepts_is_a_validated_commit_id() {
    // Three commands take a commit now, directly or inside a scope. None of them
    // takes a `String`: `CommitId` validates as it deserialises, so `HEAD`, a
    // refspec, a path and a flag all fail on the wire (ADR-0009, ADR-0015).
    let commands = repo_root().join("src-tauri/src/commands");

    let mut taking = 0;
    let mut violations = Vec::new();
    for (path, text) in sources(&["rs"]) {
        if !path.starts_with(&commands) {
            continue;
        }
        for signature in command_signatures(&code_only(&text)) {
            for parameter in parameters_of(&signature) {
                let name = parameter_name(&parameter).to_lowercase();
                if name != "commit" && name != "cursor" && name != "scope" {
                    continue;
                }
                taking += 1;
                let admits_a_validated_id =
                    parameter.contains("CommitId") || parameter.contains("DiffScope");
                if !admits_a_validated_id {
                    violations.push(format!("{}: {parameter}", relative(&path)));
                }
            }
        }
    }

    assert!(
        taking > 0,
        "the scan found no commit parameters — it is broken"
    );
    assert!(
        violations.is_empty(),
        "a commit arrives as something other than a validated id: {violations:#?}"
    );
}

#[test]
fn the_diff_limits_are_constants_and_every_one_reports_itself() {
    // `security-and-privacy.md`: nothing is truncated silently. Each ceiling is a
    // named constant in `mira-git`, and each has a state that says it bit —
    // a shorter answer with no explanation would look like a complete one.
    let diff =
        fs::read_to_string(repo_root().join("crates/mira-git/src/diff.rs")).expect("diff.rs");
    let code = code_only(&diff);

    for limit in [
        "pub const MAX_FILES: usize",
        "pub const MAX_LINES: usize",
        "pub const MAX_BYTES: usize",
        "pub const MAX_LINE_BYTES: usize",
        "pub const MAX_FILE_BYTES: u64",
    ] {
        assert!(code.contains(limit), "missing a declared limit: {limit}");
    }

    for reported in [
        "pub enum FilesTruncated",
        "pub enum PatchTruncated",
        "TooLarge",
        "Binary",
    ] {
        assert!(
            code.contains(reported),
            "a limit bites with nothing to say about it: {reported}"
        );
    }
}

#[test]
fn a_binary_file_is_never_decoded_as_text() {
    // Identified, not decoded. `FileDiff::Binary` carries two sizes and no
    // content, and the renderer is only reached once libgit2 has said the file is
    // not binary — so there is no path on which bytes of a binary file become a
    // `String`.
    let patch =
        fs::read_to_string(repo_root().join("crates/mira-git/src/patch.rs")).expect("patch.rs");
    let code = code_only(&patch);

    assert!(
        code.contains("is_binary()"),
        "binariness has to be asked about before a patch is rendered"
    );
    assert!(
        code.contains("FileDiff::Binary"),
        "and answered with a state that carries no text"
    );

    let diff =
        fs::read_to_string(repo_root().join("crates/mira-git/src/diff.rs")).expect("diff.rs");
    let binary_variant = code_only(&diff)
        .split("Binary {")
        .nth(1)
        .and_then(|rest| rest.split('}').next())
        .unwrap_or_default()
        .to_owned();

    for carrying_content in ["text", "content", "lines", "hunks", "body"] {
        assert!(
            !binary_variant.contains(carrying_content),
            "FileDiff::Binary carries `{carrying_content}`; it must carry sizes and nothing else"
        );
    }
}

#[test]
fn reading_a_diff_reads_a_header_before_it_reads_a_file() {
    // The difference between a limit and a cleanup. libgit2 fills a delta's size
    // only once it has loaded the blob, so asking it directly would mean reading
    // the very file the ceiling exists to refuse. The size comes from the object
    // header instead — measured at 0.17 ms to refuse a 30 MB file, against 4 ms
    // to diff a 1 MB one (`tests/performance.rs`).
    let patch =
        fs::read_to_string(repo_root().join("crates/mira-git/src/patch.rs")).expect("patch.rs");
    let code = code_only(&patch);

    assert!(
        code.contains("read_header"),
        "a size must come from the object header, not from loading the object"
    );
    assert!(
        code.contains("max_size("),
        "and libgit2 must be given the same ceiling, so nothing large is materialised at all"
    );
}

// ── Tracing a file ───────────────────────────────────────────────────────────

#[test]
fn no_command_accepts_a_pathspec_or_a_glob() {
    // File history is the feature most likely to want one: `git log -- <path>`
    // is how everybody else spells it. Mira spells it with an ordinal in a change
    // set it produced, so a caller has nowhere to put a pattern even if it wanted
    // to (ADR-0017).
    let banned = [
        "pathspec",
        "spec",
        "glob",
        "pattern",
        "match",
        "filter",
        "include",
        "exclude",
        "since",
        "before_path",
        "wildcard",
        "regex",
        "query",
        "search",
    ];

    let mut violations = Vec::new();
    for (path, signature) in command_parameters() {
        for parameter in parameters_of(&signature) {
            let name = parameter_name(&parameter).to_lowercase();
            if banned.contains(&name.as_str()) {
                violations.push(format!("{path}: {parameter}"));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "a command accepts a pattern to match files with: {violations:#?}"
    );
}

#[test]
fn a_file_subject_carries_no_path() {
    // The type is the guarantee. `FileSubject` is a scope, an ordinal and a
    // side — there is no field on it a path could live in, so "no path crosses
    // the boundary" is a fact about the struct rather than about its callers.
    let trace =
        fs::read_to_string(repo_root().join("crates/mira-git/src/trace.rs")).expect("trace.rs");
    let code = code_only(&trace);

    let subject = code
        .split("pub struct FileSubject {")
        .nth(1)
        .and_then(|rest| rest.split('}').next())
        .expect("a FileSubject definition")
        .to_owned();

    for holding_a_path in ["path", "String", "PathBuf", "&str", "name", "spec"] {
        assert!(
            !subject.contains(holding_a_path),
            "FileSubject carries `{holding_a_path}`; it must carry a scope and an ordinal"
        );
    }
    assert!(subject.contains("scope: DiffScope"));
    assert!(subject.contains("at: u32"));
}

#[test]
fn a_file_trace_is_bounded_by_commits_examined() {
    // The measured constraint, made structural. File history is inherently
    // O(repository history) — 740 ms to walk a 20,000-commit repository for one
    // file, growing linearly — so one request stops after a declared number of
    // commits and says how far it got. Without the ceiling this is the one read
    // in the product that a large repository could make arbitrarily slow.
    let trace =
        fs::read_to_string(repo_root().join("crates/mira-git/src/trace.rs")).expect("trace.rs");
    let code = code_only(&trace);

    assert!(
        code.contains("pub const MAX_SCAN: usize"),
        "the budget must be a named constant"
    );
    assert!(
        code.contains("scanned >= MAX_SCAN"),
        "and the walk must actually stop at it"
    );
    assert!(
        code.contains("ScanStopped::Budget"),
        "and say so, because a trace that stopped early looks exactly like a file \
         with no history"
    );

    // The same rule as the graph: a sorted revwalk reads the whole history
    // before yielding anything, which is the opposite of a bounded scan.
    for sorting in ["Sort::TOPOLOGICAL", "Sort::TIME", "Sort::REVERSE"] {
        assert!(
            !code.contains(sorting),
            "a sorted revwalk defeats the budget: {sorting}"
        );
    }
}

#[test]
fn tracing_a_file_reads_trees_rather_than_diffs() {
    // What makes the budget affordable. Deciding whether a commit touched a path
    // is two tree lookups and an id comparison — 30 µs — not a diff. A diff runs
    // only where a rename can hide, which is the commit where the path appears,
    // and costs 0.03 ms when it does.
    let trace =
        fs::read_to_string(repo_root().join("crates/mira-git/src/trace.rs")).expect("trace.rs");
    let code = code_only(&trace);

    assert!(
        code.contains("get_path"),
        "the per-commit test must be a tree lookup"
    );

    let touched = code
        .split("fn touched(")
        .nth(1)
        .and_then(|rest| rest.split("\n}").next())
        .expect("a touched function")
        .to_owned();

    for expensive in [
        "diff_tree_to_tree",
        "Patch::",
        "changed_files",
        "find_similar",
    ] {
        assert!(
            !touched.contains(expensive),
            "the per-commit test uses `{expensive}`; at two thousand commits a \
             request that would be seconds rather than milliseconds"
        );
    }
}

// ── Searching history ────────────────────────────────────────────────────────

#[test]
fn a_history_filter_carries_no_ref_name_no_path_and_no_pattern() {
    // The type is the guarantee. Everything a filter holds is a validated commit
    // id, a change-set position, or a value that is only ever compared — so there
    // is nothing in it a caller could smuggle a revision expression through.
    let filter =
        fs::read_to_string(repo_root().join("crates/mira-git/src/filter.rs")).expect("filter.rs");
    let code = code_only(&filter);

    let fields = code
        .split("pub struct HistoryFilter {")
        .nth(1)
        .and_then(|rest| rest.split("\n}").next())
        .expect("a HistoryFilter definition")
        .to_owned();

    // Positive first, so the parse cannot succeed vacuously.
    assert!(fields.contains("branch: Option<CommitId>"), "got: {fields}");
    assert!(
        fields.contains("file: Option<FileSubject>"),
        "got: {fields}"
    );
    assert!(fields.contains("author: Option<Term>"), "got: {fields}");
    assert!(fields.contains("subject: Option<Term>"), "got: {fields}");

    for smuggled in [
        "String", "PathBuf", "&str", "pathspec", "glob", "refspec", "rev",
    ] {
        assert!(
            !fields.contains(smuggled),
            "HistoryFilter carries `{smuggled}`; every field must be a validated type"
        );
    }
}

#[test]
fn no_filter_value_ever_reaches_libgit2() {
    // Author and subject are compared in Rust against fields of a commit already
    // in memory. `revparse` is the one call that would interpret a string as a
    // revision, and it is not in this file at all — commit ids are resolved
    // through the same helper every other slice uses.
    let filter =
        fs::read_to_string(repo_root().join("crates/mira-git/src/filter.rs")).expect("filter.rs");
    let code = code_only(&filter);

    for interpreting in [
        "revparse",
        "reference_to_annotated_commit",
        "Pathspec",
        "pathspec",
        "DiffOptions",
        "glob",
    ] {
        assert!(
            !code.contains(interpreting),
            "a filter value could reach libgit2 through `{interpreting}`"
        );
    }

    // And the comparison is what it says it is: lower-cased on both sides.
    assert!(
        code.contains("to_lowercase"),
        "matching must fold case on both sides, as the type documents"
    );

    // Nowhere in the workspace does Mira spell a filter the way a command line
    // would. These are the flags a reviewer would look for, so they are the
    // flags the build looks for.
    for (path, source) in sources(&["rs"]) {
        let code = code_only(&source);

        for flag in [
            "--author",
            "--committer",
            "--grep",
            "--all-match",
            "--fixed-strings",
            "--regexp-ignore-case",
            "--follow",
            "push_glob",
            "push_ref",
            "push_range",
        ] {
            assert!(
                !code.contains(flag),
                "{}: `{flag}` — a filter is a typed value, never an argument",
                relative(&path)
            );
        }
    }
}

#[test]
fn a_search_is_bounded_by_commits_examined() {
    // The same ceiling as a file trace, for the same reason: a search over
    // history is proportional to the history. Measured at ~110 ms in the worst
    // case — a filter matching nothing in a ten-thousand-commit repository —
    // which plateaus at the budget instead of climbing with the repository.
    let filter =
        fs::read_to_string(repo_root().join("crates/mira-git/src/filter.rs")).expect("filter.rs");
    let code = code_only(&filter);

    assert!(code.contains("pub const MAX_FILTER_SCAN: usize"));
    assert!(
        code.contains("scanned >= MAX_FILTER_SCAN"),
        "the walk must actually stop at it"
    );
    assert!(
        code.contains("ScanStopped::Budget"),
        "and say so, because a search that stopped early looks exactly like a \
         search that found nothing"
    );

    for sorting in ["Sort::TOPOLOGICAL", "Sort::TIME", "Sort::REVERSE"] {
        assert!(
            !code.contains(sorting),
            "a sorted revwalk reads the whole history before yielding: {sorting}"
        );
    }
}

#[test]
fn a_branch_is_chosen_by_its_tip_rather_than_by_its_name() {
    // `git.refs` hands out a name to read and a **commit id** to ask with. That
    // is what keeps a ref name off the wire entirely: a name that never arrives
    // is a name that cannot be interpreted.
    let filter =
        fs::read_to_string(repo_root().join("crates/mira-git/src/filter.rs")).expect("filter.rs");
    let code = code_only(&filter);

    let tip = code
        .split("pub struct RefTip {")
        .nth(1)
        .and_then(|rest| rest.split("\n}").next())
        .expect("a RefTip definition")
        .to_owned();

    assert!(tip.contains("tip: CommitId"), "got: {tip}");
    assert!(
        tip.contains("name: String"),
        "the name is for reading: {tip}"
    );

    // And the filter itself takes the id, never the name.
    assert!(
        !code.contains("branch: Option<String>"),
        "a branch filter must carry a commit id"
    );
}

// ── Workspace services ───────────────────────────────────────────────────────

#[test]
fn no_command_takes_a_port_or_an_address_from_the_interface() {
    // Slice 4c's rule, and the one that changed an existing command to keep it.
    // `live.open_service` used to take `port: u16`, checked against the observed
    // list; the check was real, but the parameter was still a number of the
    // caller's choosing. It now takes a **position** in the list Mira produced.
    //
    // The direction matters and is asserted deliberately. A port travels
    // *outward* on every reading — the Services panel says `:5173`, and hiding
    // it would make the panel useless. What must never travel *inward* is a
    // port, an address, a host or a URL, because inbound values are
    // instructions and outbound values are information
    // (`security-and-privacy.md` §5 rule 5).
    let banned = [
        "port", "ports", "address", "addr", "host", "hostname", "url", "uri", "endpoint", "scheme",
        "origin", "socket", "pid", "process",
    ];

    let mut violations = Vec::new();
    for (path, signature) in command_parameters() {
        for parameter in parameters_of(&signature) {
            let name = parameter_name(&parameter).to_lowercase();
            if banned.contains(&name.as_str()) {
                violations.push(format!("{path}: {parameter}"));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "a command lets the caller name a port or an address; Mira names its own \
         observations and hands back positions and row ids: {violations:#?}"
    );

    // And positively: the one thing that *is* accepted for a service is an
    // ordinal or a row id, so this cannot pass by the commands having vanished.
    let services = fs::read_to_string(repo_root().join("src-tauri/src/commands/services.rs"))
        .expect("services.rs");
    let code = code_only(&services);
    assert!(
        code.contains("at: u32"),
        "a service is added by its position in the list Mira offered"
    );
    assert!(
        code.contains("service_id: WorkspaceServiceId"),
        "and opened or forgotten by the row id Mira issued"
    );
}

#[test]
fn a_watched_service_is_stored_as_a_port_and_nothing_else() {
    // The stored row is the place a process name, a command line or a URL would
    // end up if this feature were built the obvious way — "remember what was
    // running here". It holds four columns, and the assertion is positive as
    // well as negative so it cannot pass by the table having been renamed.
    let sql = fs::read_to_string(
        repo_root().join("crates/mira-db/migrations/0003_workspace_services.sql"),
    )
    .expect("0003_workspace_services.sql");
    let table = code_only(&sql)
        .split("CREATE TABLE IF NOT EXISTS workspace_services (")
        .nth(1)
        .and_then(|rest| rest.split(");").next())
        .expect("a workspace_services definition")
        .to_owned();

    for expected in ["id", "workspace_id", "port", "added_at"] {
        assert!(
            table.contains(expected),
            "the stored row is missing {expected}: {table}"
        );
    }

    for forbidden in [
        "pid",
        "process",
        "executable",
        "program",
        "command",
        "args",
        "argv",
        "url",
        "uri",
        "scheme",
        "path",
        "address",
        "host",
        "label",
        "name",
    ] {
        assert!(
            !table.contains(forbidden),
            "a watched service stores something that is observation or an \
             instruction: {forbidden} in {table}"
        );
    }
}

#[test]
fn nothing_reads_or_writes_the_table_that_could_hold_a_url() {
    // `expected_ports` came from `0001_init.sql`, before the security rules were
    // written, and it carries `scheme` and `path` — two columns whose only
    // purpose is to be concatenated into a URL. A stored path of
    // `@example.invalid/` turns `http://localhost:3000` into
    // `http://localhost:3000@example.invalid/`, which is a request to a remote
    // host wearing a loopback address.
    //
    // Slice 4c stores what a workspace watches somewhere else and leaves this
    // table empty. Empty is a claim, so it is enforced rather than intended.
    let mut violations = Vec::new();
    for (path, text) in sources(&["rs", "ts", "tsx"]) {
        // The migration set may *name* it in a comment saying why it stays
        // empty; `code_only` has already removed those lines.
        if relative(&path).ends_with("migrations.rs") {
            continue;
        }
        if code_only(&text).contains("expected_ports") {
            violations.push(relative(&path));
        }
    }

    assert!(
        violations.is_empty(),
        "a table with a scheme and a path column is being used; a URL Mira did \
         not build is a URL Mira cannot vouch for: {violations:#?}"
    );
}

#[test]
fn a_workspace_reaches_its_own_configuration_and_no_others() {
    // Two workspaces on one project intentionally watch different services, so
    // the isolation is not incidental — it is the feature. Every SQL statement
    // that touches `workspace_services` names `workspace_id`, which makes a
    // sibling's row id resolve to nothing rather than to a row.
    let repo = fs::read_to_string(repo_root().join("crates/mira-db/src/workspaces.rs"))
        .expect("workspaces.rs");
    let code = code_only(&repo);

    // Only the SQL, found by the keyword in front of the table name — the same
    // word appears as a Rust method name, and a method is not a statement.
    let mut statements = Vec::new();
    for keyword in ["FROM ", "INTO ", "UPDATE ", "JOIN "] {
        for reached in code.split(&format!("{keyword}workspace_services")).skip(1) {
            // A rusqlite literal is one string with `\` line continuations, so
            // the next quote is its end.
            let rest: String = reached.chars().take_while(|c| *c != '"').collect();
            statements.push(format!("{keyword}workspace_services{rest}"));
        }
    }

    assert!(
        statements.len() >= 5,
        "the SQL scan found {} statements — it is broken: {statements:#?}",
        statements.len()
    );

    let unscoped: Vec<&String> = statements
        .iter()
        .filter(|statement| !statement.contains("workspace_id"))
        .collect();

    assert!(
        unscoped.is_empty(),
        "a statement reaches workspace_services without saying whose: {unscoped:#?}"
    );
}

#[test]
fn an_observed_service_is_never_stored_as_something_to_run() {
    // The observers read a process's name, its executable path and its working
    // directory, because attribution needs them. None of that may become a
    // stored row: an executable path in Mira's database is a program somebody
    // could later be tempted to start, which is the shape §5 rule 4 exists to
    // forbid.
    let migrations = repo_root().join("crates/mira-db/migrations");
    let mut violations = Vec::new();

    for (path, text) in sources(&["sql"]) {
        if !path.starts_with(&migrations) {
            continue;
        }
        let schema = code_only(&text);
        for statement in schema.split("CREATE TABLE").skip(1) {
            let Some(name) = statement.split_whitespace().nth(3) else {
                continue;
            };
            if !name.starts_with("workspace") {
                continue;
            }
            let Some(body) = statement.split(");").next() else {
                continue;
            };
            for forbidden in ["executable", "working_directory", "cmdline", "pid"] {
                if body.contains(forbidden) {
                    violations.push(format!("{}: {name} has {forbidden}", relative(&path)));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "an observation was written down as something with a program in it: {violations:#?}"
    );

    // And the type that crosses the boundary carries a process *name* for
    // display and nothing that could be run: no path, no argv, no command line.
    let service = fs::read_to_string(repo_root().join("crates/mira-core/src/service.rs"))
        .expect("service.rs");
    let code = code_only(&service);
    for forbidden in ["executable", "working_directory", "argv", "command"] {
        assert!(
            !code.contains(forbidden),
            "a service state carries {forbidden}, which is a thing to run rather \
             than a thing to look at"
        );
    }
}

#[test]
fn no_command_stops_a_service() {
    // The companion to `nothing_can_stop_a_process`, at the IPC boundary rather
    // than in the code. Slice 4c is the first feature that gives a workspace a
    // *relationship* to a running process, which is exactly the moment a stop
    // button would feel natural. Termination arrives with its own confirmation
    // and refusal design or it does not arrive (`security-and-privacy.md` §5).
    let banned = [
        "stop",
        "kill",
        "terminate",
        "signal",
        "restart",
        "start",
        "run",
        "spawn",
        "force",
    ];

    let mut violations = Vec::new();
    for (path, signature) in command_parameters() {
        for parameter in parameters_of(&signature) {
            let name = parameter_name(&parameter).to_lowercase();
            if banned.contains(&name.as_str()) {
                violations.push(format!("{path}: {parameter}"));
            }
        }
    }

    // And no command is *named* for it either, which is the shape somebody
    // would reach for before they reached for a parameter.
    for (path, text) in sources(&["rs"]) {
        if !path.starts_with(repo_root().join("src-tauri/src/commands")) {
            continue;
        }
        for line in code_only(&text).lines() {
            let line = line.trim();
            if !line.starts_with("pub fn ") && !line.starts_with("pub async fn ") {
                continue;
            }
            for verb in [
                "stop_",
                "kill_",
                "terminate_",
                "_stop",
                "_kill",
                "_terminate",
            ] {
                if line.contains(verb) {
                    violations.push(format!("{}: {line}", relative(&path)));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "a command can stop something; termination is a slice with its own \
         design, not a parameter: {violations:#?}"
    );
}

#[test]
fn watching_a_service_starts_no_observer_and_no_clock() {
    // ADR-0011 says one scheduler owns every clock, and slice 4c is where a
    // per-workspace poller would be easiest to justify: each workspace has its
    // own list, and refreshing "just those" sounds thrifty. It would be a second
    // clock, and it would make N workspaces cost N reads of the same socket
    // table.
    //
    // Resolution is instead a pure function of two lists, which the measurement
    // says costs microseconds, so there is nothing to cache and no clock to own.
    let service = fs::read_to_string(repo_root().join("crates/mira-core/src/service.rs"))
        .expect("service.rs");
    let commands = fs::read_to_string(repo_root().join("src-tauri/src/commands/services.rs"))
        .expect("services.rs");

    for (name, text) in [("mira-core", &service), ("commands", &commands)] {
        let code = code_only(text);
        for forbidden in [
            "Instant",
            "SystemTime",
            "thread::spawn",
            "interval",
            "sleep",
            "Duration",
            "OnceLock",
            "LazyLock",
            "static mut",
        ] {
            assert!(
                !code.contains(forbidden),
                "{name} reaches for {forbidden}; watching a service is stored \
                 rows and a pure function, not a timer or a cache"
            );
        }
    }

    // Positively: the resolver takes what it is given and returns a value.
    assert!(
        code_only(&service).contains("pub fn resolve("),
        "the resolution is a function of its inputs"
    );
}
