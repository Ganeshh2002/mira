//! Monorepo detection, against real fixture trees on disk.
//!
//! Every case below builds actual files, because the thing being tested is
//! agreement with what package managers write. A fake would only agree with
//! itself, and the failure mode that matters — claiming a package that is not
//! there — is invisible without real directories.
//!
//! Detection is read-only throughout. Nothing here runs a package manager,
//! installs anything, or writes to the fixture after it is built.

use std::fs;
use std::path::Path;

use mira_monorepo::{detect, MonorepoTool, RepositoryLayout};
use tempfile::TempDir;

fn write(root: &Path, relative: &str, body: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("a parent")).expect("mkdir");
    fs::write(path, body).expect("write");
}

/// The packages of a monorepo root, as `name → path` pairs.
fn packages(layout: &RepositoryLayout) -> Vec<(String, String)> {
    match layout {
        RepositoryLayout::MonorepoRoot { packages, .. } => packages
            .iter()
            .map(|package| (package.name.clone(), package.path.clone()))
            .collect(),
        other => panic!("expected a monorepo root, got {other:?}"),
    }
}

fn tools(layout: &RepositoryLayout) -> Vec<MonorepoTool> {
    match layout {
        RepositoryLayout::MonorepoRoot { tools, .. } | RepositoryLayout::Package { tools, .. } => {
            tools.clone()
        }
        RepositoryLayout::Standalone => Vec::new(),
    }
}

// ── The ordinary case ────────────────────────────────────────────────────────

#[test]
fn a_plain_repository_is_standalone() {
    let dir = TempDir::new().expect("tempdir");
    write(dir.path(), "package.json", r#"{"name":"aviora"}"#);
    write(dir.path(), "src/index.ts", "export {};");

    assert_eq!(
        detect(dir.path(), Some(dir.path())),
        RepositoryLayout::Standalone,
        "a package.json without workspaces is one project, not a monorepo"
    );
}

#[test]
fn a_directory_with_nothing_in_it_is_standalone() {
    let dir = TempDir::new().expect("tempdir");

    assert_eq!(detect(dir.path(), None), RepositoryLayout::Standalone);
}

// ── npm and Yarn ─────────────────────────────────────────────────────────────

#[test]
fn npm_workspaces_are_read_from_the_root_package_json() {
    let dir = TempDir::new().expect("tempdir");
    write(
        dir.path(),
        "package.json",
        r#"{"name":"aviora","workspaces":["apps/*","packages/*"]}"#,
    );
    write(
        dir.path(),
        "apps/web/package.json",
        r#"{"name":"@aviora/web"}"#,
    );
    write(
        dir.path(),
        "apps/mobile/package.json",
        r#"{"name":"@aviora/mobile"}"#,
    );
    write(
        dir.path(),
        "packages/ui/package.json",
        r#"{"name":"@aviora/ui"}"#,
    );

    let layout = detect(dir.path(), Some(dir.path()));

    assert_eq!(tools(&layout), [MonorepoTool::NpmWorkspaces]);
    assert_eq!(
        packages(&layout),
        [
            ("@aviora/mobile".to_owned(), "apps/mobile".to_owned()),
            ("@aviora/web".to_owned(), "apps/web".to_owned()),
            ("@aviora/ui".to_owned(), "packages/ui".to_owned()),
        ],
        "sorted by path, so the list does not reshuffle between reads"
    );
}

#[test]
fn yarn_object_form_workspaces_are_understood() {
    // Yarn's classic form nests the globs under `packages`.
    let dir = TempDir::new().expect("tempdir");
    write(
        dir.path(),
        "package.json",
        r#"{"name":"aviora","workspaces":{"packages":["apps/*"],"nohoist":["**/react-native"]}}"#,
    );
    write(dir.path(), "apps/web/package.json", r#"{"name":"web"}"#);

    let layout = detect(dir.path(), Some(dir.path()));

    assert_eq!(tools(&layout), [MonorepoTool::YarnWorkspaces]);
    assert_eq!(
        packages(&layout),
        [("web".to_owned(), "apps/web".to_owned())]
    );
}

#[test]
fn a_package_without_a_name_falls_back_to_its_directory() {
    let dir = TempDir::new().expect("tempdir");
    write(dir.path(), "package.json", r#"{"workspaces":["apps/*"]}"#);
    write(dir.path(), "apps/web/package.json", "{}");

    assert_eq!(
        packages(&detect(dir.path(), Some(dir.path()))),
        [("web".to_owned(), "apps/web".to_owned())],
        "the directory name is a fact; a made-up name would not be"
    );
}

// ── pnpm ─────────────────────────────────────────────────────────────────────

#[test]
fn pnpm_workspaces_are_read_from_pnpm_workspace_yaml() {
    let dir = TempDir::new().expect("tempdir");
    write(dir.path(), "package.json", r#"{"name":"aviora"}"#);
    write(
        dir.path(),
        "pnpm-workspace.yaml",
        "# the apps and their shared code\npackages:\n  - 'apps/*'\n  - \"packages/*\"\n",
    );
    write(dir.path(), "apps/web/package.json", r#"{"name":"web"}"#);
    write(dir.path(), "packages/api/package.json", r#"{"name":"api"}"#);

    let layout = detect(dir.path(), Some(dir.path()));

    assert_eq!(tools(&layout), [MonorepoTool::PnpmWorkspaces]);
    assert_eq!(
        packages(&layout),
        [
            ("web".to_owned(), "apps/web".to_owned()),
            ("api".to_owned(), "packages/api".to_owned()),
        ]
    );
}

#[test]
fn a_pnpm_flow_sequence_is_understood() {
    let dir = TempDir::new().expect("tempdir");
    write(dir.path(), "pnpm-workspace.yaml", "packages: ['apps/*']\n");
    write(dir.path(), "apps/web/package.json", r#"{"name":"web"}"#);

    assert_eq!(
        packages(&detect(dir.path(), Some(dir.path()))),
        [("web".to_owned(), "apps/web".to_owned())]
    );
}

#[test]
fn a_pnpm_file_with_no_packages_key_detects_nothing() {
    let dir = TempDir::new().expect("tempdir");
    write(
        dir.path(),
        "pnpm-workspace.yaml",
        "onlyBuiltDependencies:\n  - esbuild\n",
    );
    write(dir.path(), "apps/web/package.json", r#"{"name":"web"}"#);

    assert_eq!(
        detect(dir.path(), Some(dir.path())),
        RepositoryLayout::Standalone,
        "a workspace file that declares no packages declares no monorepo"
    );
}

// ── Cargo ────────────────────────────────────────────────────────────────────

#[test]
fn cargo_workspace_members_are_read_from_the_root_manifest() {
    let dir = TempDir::new().expect("tempdir");
    write(
        dir.path(),
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/*\", \"src-tauri\"]\n",
    );
    write(
        dir.path(),
        "crates/mira-core/Cargo.toml",
        "[package]\nname = \"mira-core\"\n",
    );
    write(
        dir.path(),
        "crates/mira-db/Cargo.toml",
        "[package]\nname = \"mira-db\"\n",
    );
    write(
        dir.path(),
        "src-tauri/Cargo.toml",
        "[package]\nname = \"mira\"\n",
    );

    let layout = detect(dir.path(), Some(dir.path()));

    assert_eq!(tools(&layout), [MonorepoTool::CargoWorkspace]);
    assert_eq!(
        packages(&layout),
        [
            ("mira-core".to_owned(), "crates/mira-core".to_owned()),
            ("mira-db".to_owned(), "crates/mira-db".to_owned()),
            ("mira".to_owned(), "src-tauri".to_owned()),
        ]
    );
}

#[test]
fn a_cargo_manifest_without_a_workspace_table_is_standalone() {
    let dir = TempDir::new().expect("tempdir");
    write(dir.path(), "Cargo.toml", "[package]\nname = \"mira\"\n");

    assert_eq!(
        detect(dir.path(), Some(dir.path())),
        RepositoryLayout::Standalone
    );
}

#[test]
fn cargo_members_that_are_excluded_are_not_packages() {
    let dir = TempDir::new().expect("tempdir");
    write(
        dir.path(),
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/*\"]\nexclude = [\"crates/scratch\"]\n",
    );
    write(
        dir.path(),
        "crates/real/Cargo.toml",
        "[package]\nname = \"real\"\n",
    );
    write(
        dir.path(),
        "crates/scratch/Cargo.toml",
        "[package]\nname = \"scratch\"\n",
    );

    assert_eq!(
        packages(&detect(dir.path(), Some(dir.path()))),
        [("real".to_owned(), "crates/real".to_owned())]
    );
}

// ── Turborepo and Nx ─────────────────────────────────────────────────────────

#[test]
fn turborepo_is_reported_alongside_the_package_manager_that_declares_the_packages() {
    // turbo.json says "this is a monorepo"; it does not say which packages exist.
    // The package manager is still the authority on that.
    let dir = TempDir::new().expect("tempdir");
    write(
        dir.path(),
        "package.json",
        r#"{"name":"aviora","workspaces":["apps/*"]}"#,
    );
    write(
        dir.path(),
        "turbo.json",
        r#"{"$schema":"https://turbo.build/schema.json"}"#,
    );
    write(dir.path(), "apps/web/package.json", r#"{"name":"web"}"#);

    let layout = detect(dir.path(), Some(dir.path()));

    assert_eq!(
        tools(&layout),
        [MonorepoTool::NpmWorkspaces, MonorepoTool::Turborepo],
        "both are true, and saying only one of them would be less useful"
    );
    assert_eq!(
        packages(&layout),
        [("web".to_owned(), "apps/web".to_owned())]
    );
}

#[test]
fn nx_finds_projects_under_its_declared_layout() {
    let dir = TempDir::new().expect("tempdir");
    write(dir.path(), "package.json", r#"{"name":"aviora"}"#);
    write(
        dir.path(),
        "nx.json",
        r#"{"workspaceLayout":{"appsDir":"apps","libsDir":"libs"}}"#,
    );
    write(dir.path(), "apps/web/project.json", r#"{"name":"web"}"#);
    write(dir.path(), "libs/ui/project.json", r#"{"name":"ui"}"#);

    let layout = detect(dir.path(), Some(dir.path()));

    assert_eq!(tools(&layout), [MonorepoTool::Nx]);
    assert_eq!(
        packages(&layout),
        [
            ("web".to_owned(), "apps/web".to_owned()),
            ("ui".to_owned(), "libs/ui".to_owned()),
        ]
    );
}

#[test]
fn nx_without_a_declared_layout_uses_the_directories_nx_itself_defaults_to() {
    let dir = TempDir::new().expect("tempdir");
    write(dir.path(), "nx.json", "{}");
    write(dir.path(), "apps/web/project.json", "{}");
    write(dir.path(), "libs/ui/project.json", "{}");

    assert_eq!(
        packages(&detect(dir.path(), Some(dir.path()))),
        [
            ("web".to_owned(), "apps/web".to_owned()),
            ("ui".to_owned(), "libs/ui".to_owned()),
        ]
    );
}

#[test]
fn a_directory_under_the_nx_layout_without_a_manifest_is_not_a_package() {
    let dir = TempDir::new().expect("tempdir");
    write(dir.path(), "nx.json", "{}");
    write(dir.path(), "apps/web/project.json", "{}");
    fs::create_dir_all(dir.path().join("apps/notes")).expect("mkdir");

    assert_eq!(
        packages(&detect(dir.path(), Some(dir.path()))),
        [("web".to_owned(), "apps/web".to_owned())],
        "a directory in the right place is not evidence of a package"
    );
}

// ── Selecting a package inside a monorepo ────────────────────────────────────

#[test]
fn selecting_a_package_reports_the_monorepo_it_belongs_to() {
    let dir = TempDir::new().expect("tempdir");
    write(
        dir.path(),
        "package.json",
        r#"{"name":"aviora","workspaces":["apps/*"]}"#,
    );
    write(
        dir.path(),
        "apps/web/package.json",
        r#"{"name":"@aviora/web"}"#,
    );

    let layout = detect(&dir.path().join("apps/web"), Some(dir.path()));

    match layout {
        RepositoryLayout::Package {
            tools,
            monorepo_root,
            package_path,
            package_name,
        } => {
            assert_eq!(tools, [MonorepoTool::NpmWorkspaces]);
            assert_eq!(Path::new(&monorepo_root), dir.path());
            assert_eq!(package_path, "apps/web");
            assert_eq!(package_name, "@aviora/web");
        }
        other => panic!("expected a package, got {other:?}"),
    }
}

#[test]
fn a_directory_inside_a_monorepo_that_is_not_a_package_is_standalone() {
    // `apps/web/src` is inside the monorepo but is not one of its packages, so
    // there is no boundary to report.
    let dir = TempDir::new().expect("tempdir");
    write(dir.path(), "package.json", r#"{"workspaces":["apps/*"]}"#);
    write(dir.path(), "apps/web/package.json", r#"{"name":"web"}"#);
    fs::create_dir_all(dir.path().join("apps/web/src")).expect("mkdir");

    assert_eq!(
        detect(&dir.path().join("apps/web/src"), Some(dir.path())),
        RepositoryLayout::Standalone
    );
}

#[test]
fn a_cargo_member_selected_directly_reports_its_workspace() {
    let dir = TempDir::new().expect("tempdir");
    write(
        dir.path(),
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/*\"]\n",
    );
    write(
        dir.path(),
        "crates/mira-core/Cargo.toml",
        "[package]\nname = \"mira-core\"\n",
    );

    match detect(&dir.path().join("crates/mira-core"), Some(dir.path())) {
        RepositoryLayout::Package {
            package_path,
            package_name,
            ..
        } => {
            assert_eq!(package_path, "crates/mira-core");
            assert_eq!(package_name, "mira-core");
        }
        other => panic!("expected a package, got {other:?}"),
    }
}

// ── Boundaries and bad input ─────────────────────────────────────────────────

#[test]
fn the_walk_up_stops_at_the_git_root() {
    // The monorepo above is a different repository. Its workspace configuration
    // says nothing about this one, and crossing that boundary would attach a
    // project to a repository it is not in.
    let outer = TempDir::new().expect("tempdir");
    write(outer.path(), "package.json", r#"{"workspaces":["apps/*"]}"#);
    write(outer.path(), "apps/web/package.json", r#"{"name":"web"}"#);

    let nested = outer.path().join("apps/web");

    assert_eq!(
        detect(&nested, Some(&nested)),
        RepositoryLayout::Standalone,
        "with its own Git root, the package is its own repository"
    );
}

#[test]
fn detection_without_a_git_root_still_stops_climbing_eventually() {
    let dir = TempDir::new().expect("tempdir");
    write(dir.path(), "package.json", r#"{"workspaces":["a/*"]}"#);
    write(dir.path(), "a/deep/package.json", r#"{"name":"deep"}"#);

    match detect(&dir.path().join("a/deep"), None) {
        RepositoryLayout::Package { package_path, .. } => assert_eq!(package_path, "a/deep"),
        other => panic!("expected a package, got {other:?}"),
    }
}

#[test]
fn malformed_configuration_detects_nothing_instead_of_failing() {
    for (name, body) in [
        ("package.json", "{ this is not json"),
        ("Cargo.toml", "[workspace\nmembers = "),
        ("pnpm-workspace.yaml", "packages:\n\t- broken\x00"),
        ("nx.json", "]["),
        ("turbo.json", "{"),
    ] {
        let dir = TempDir::new().expect("tempdir");
        write(dir.path(), name, body);
        write(dir.path(), "apps/web/package.json", r#"{"name":"web"}"#);

        assert_eq!(
            detect(dir.path(), Some(dir.path())),
            RepositoryLayout::Standalone,
            "{name} is unreadable, so nothing is claimed about it"
        );
    }
}

#[test]
fn a_pattern_pointing_at_nothing_contributes_no_packages() {
    let dir = TempDir::new().expect("tempdir");
    write(
        dir.path(),
        "package.json",
        r#"{"workspaces":["apps/*","services/*","tools/build"]}"#,
    );
    write(dir.path(), "apps/web/package.json", r#"{"name":"web"}"#);

    assert_eq!(
        packages(&detect(dir.path(), Some(dir.path()))),
        [("web".to_owned(), "apps/web".to_owned())],
        "missing directories are missing, not empty packages"
    );
}

#[test]
fn two_packages_may_share_a_name_because_their_paths_differ() {
    let dir = TempDir::new().expect("tempdir");
    write(
        dir.path(),
        "package.json",
        r#"{"workspaces":["apps/*","legacy/*"]}"#,
    );
    write(dir.path(), "apps/api/package.json", r#"{"name":"api"}"#);
    write(dir.path(), "legacy/api/package.json", r#"{"name":"api"}"#);

    assert_eq!(
        packages(&detect(dir.path(), Some(dir.path()))),
        [
            ("api".to_owned(), "apps/api".to_owned()),
            ("api".to_owned(), "legacy/api".to_owned()),
        ],
        "hiding one of them would hide a real directory the user has"
    );
}

#[test]
fn detection_never_descends_into_dependency_directories() {
    // `node_modules` holds thousands of package.json files. Walking into it would
    // be both slow and wrong: those are dependencies, not this repository's
    // packages.
    let dir = TempDir::new().expect("tempdir");
    write(dir.path(), "package.json", r#"{"workspaces":["**"]}"#);
    write(dir.path(), "apps/web/package.json", r#"{"name":"web"}"#);
    write(
        dir.path(),
        "node_modules/react/package.json",
        r#"{"name":"react"}"#,
    );
    write(
        dir.path(),
        "apps/web/node_modules/lodash/package.json",
        r#"{"name":"lodash"}"#,
    );

    let found = packages(&detect(dir.path(), Some(dir.path())));

    assert!(
        found
            .iter()
            .all(|(name, _)| name != "react" && name != "lodash"),
        "a dependency is not a package of this repository: {found:?}"
    );
}

#[test]
fn a_double_star_pattern_is_bounded_rather_than_unlimited() {
    let dir = TempDir::new().expect("tempdir");
    write(dir.path(), "package.json", r#"{"workspaces":["**"]}"#);
    write(dir.path(), "a/package.json", r#"{"name":"a"}"#);
    write(
        dir.path(),
        "a/b/c/d/e/f/g/package.json",
        r#"{"name":"too-deep"}"#,
    );

    let found = packages(&detect(dir.path(), Some(dir.path())));

    assert!(found.iter().any(|(name, _)| name == "a"));
    assert!(
        found.iter().all(|(name, _)| name != "too-deep"),
        "the walk has a depth budget, so a pathological tree cannot stall the view"
    );
}

#[test]
fn the_root_itself_is_never_listed_as_one_of_its_own_packages() {
    let dir = TempDir::new().expect("tempdir");
    write(
        dir.path(),
        "package.json",
        r#"{"name":"aviora","workspaces":["**"]}"#,
    );
    write(dir.path(), "apps/web/package.json", r#"{"name":"web"}"#);

    assert_eq!(
        packages(&detect(dir.path(), Some(dir.path()))),
        [("web".to_owned(), "apps/web".to_owned())]
    );
}

#[test]
fn every_tool_has_a_label_a_person_can_read() {
    for tool in MonorepoTool::ALL {
        let label = tool.label();
        let glued = label
            .chars()
            .zip(label.chars().skip(1))
            .any(|(before, after)| before.is_lowercase() && after.is_uppercase());

        assert!(!label.is_empty());
        assert!(
            !label.contains('_') && !glued,
            "{tool:?} is shown to a person, so it is written the way the tool \
             writes its own name — `npm workspaces`, not `NpmWorkspaces`. \
             `Turborepo` is fine: that is genuinely how it is spelled."
        );
    }
}
