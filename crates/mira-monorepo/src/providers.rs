//! One reader per tool.
//!
//! Each provider answers two questions about a candidate root: *is your manifest
//! here*, and *which patterns does it declare*. Neither question runs anything —
//! a manifest is read as a file, never executed, and no package manager, script,
//! or network call is involved at any point.
//!
//! A provider that cannot parse its manifest reports [`Declaration::none`]: the
//! tool is not claimed and no packages are invented. Adding a tool is adding a
//! provider and a row in [`ALL`].

use std::path::Path;

use crate::model::MonorepoTool;
use crate::pnpm;

/// What one provider found at a candidate root.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Declaration {
    /// Whether this tool's manifest is present and readable.
    pub present: bool,
    /// The package patterns it declares. Empty is meaningful: Turborepo declares a
    /// monorepo without saying which packages are in it.
    pub patterns: Vec<String>,
    /// The manifest a candidate directory must contain to count as a package.
    pub manifests: &'static [&'static str],
    /// Paths the tool explicitly detaches from the workspace.
    ///
    /// Cargo's `exclude` names directories, not patterns, so it can only be
    /// applied once the patterns have been expanded into directories.
    pub exclude: Vec<String>,
}

impl Declaration {
    /// Nothing here.
    const fn none() -> Self {
        Self {
            present: false,
            patterns: Vec::new(),
            manifests: &[],
            exclude: Vec::new(),
        }
    }

    const fn present(patterns: Vec<String>, manifests: &'static [&'static str]) -> Self {
        Self {
            present: true,
            patterns,
            manifests,
            exclude: Vec::new(),
        }
    }

    fn excluding(mut self, exclude: Vec<String>) -> Self {
        self.exclude = exclude;
        self
    }
}

const NODE: &[&str] = &["package.json"];
const RUST: &[&str] = &["Cargo.toml"];
const NX: &[&str] = &["project.json", "package.json"];

/// One provider: given a candidate root, what this tool declares there.
pub type Provider = fn(&Path) -> (MonorepoTool, Declaration);

/// Every provider, in reporting order. Adding a tool is adding a row.
pub const ALL: [Provider; 6] = [npm, pnpm_workspaces, yarn, cargo, turborepo, nx];

fn read(root: &Path, name: &str) -> Option<String> {
    std::fs::read_to_string(root.join(name)).ok()
}

fn json(root: &Path, name: &str) -> Option<serde_json::Value> {
    serde_json::from_str(&read(root, name)?).ok()
}

/// The `workspaces` field of a root `package.json`, in either accepted form.
///
/// npm and Yarn read the same field. They are told apart by its shape: the array
/// is npm's documented form and Yarn's modern one, the object with `packages` is
/// Yarn classic. Getting this exactly right is not important — the packages are
/// identical either way — but naming the wrong tool in the interface is a small
/// lie, so the distinction is made where the file makes it.
fn node_workspaces(root: &Path) -> Option<(bool, Vec<String>)> {
    let manifest = json(root, "package.json")?;
    let workspaces = manifest.get("workspaces")?;

    if let Some(array) = workspaces.as_array() {
        return Some((false, strings(array)));
    }

    let packages = workspaces.get("packages")?.as_array()?;
    Some((true, strings(packages)))
}

fn strings(values: &[serde_json::Value]) -> Vec<String> {
    values
        .iter()
        .filter_map(|value| value.as_str().map(str::to_owned))
        .collect()
}

fn npm(root: &Path) -> (MonorepoTool, Declaration) {
    let declaration = match node_workspaces(root) {
        Some((false, patterns)) if !patterns.is_empty() => Declaration::present(patterns, NODE),
        _ => Declaration::none(),
    };
    (MonorepoTool::NpmWorkspaces, declaration)
}

fn yarn(root: &Path) -> (MonorepoTool, Declaration) {
    let declaration = match node_workspaces(root) {
        Some((true, patterns)) if !patterns.is_empty() => Declaration::present(patterns, NODE),
        _ => Declaration::none(),
    };
    (MonorepoTool::YarnWorkspaces, declaration)
}

fn pnpm_workspaces(root: &Path) -> (MonorepoTool, Declaration) {
    let declaration = read(root, "pnpm-workspace.yaml")
        .or_else(|| read(root, "pnpm-workspace.yml"))
        .and_then(|source| pnpm::packages(&source))
        .map_or_else(Declaration::none, |patterns| {
            Declaration::present(patterns, NODE)
        });
    (MonorepoTool::PnpmWorkspaces, declaration)
}

fn cargo(root: &Path) -> (MonorepoTool, Declaration) {
    let declaration = cargo_members(root).map_or_else(Declaration::none, |(patterns, exclude)| {
        Declaration::present(patterns, RUST).excluding(exclude)
    });
    (MonorepoTool::CargoWorkspace, declaration)
}

/// `[workspace] members`, and the paths `exclude` detaches from them.
///
/// Cargo's `exclude` is a list of directories rather than patterns, so it cannot
/// be applied to `members` — `crates/*` and `crates/scratch` never compare equal.
/// It is carried through and applied to the directories the globs produce, which
/// is what keeps a deliberately-detached crate out of the package list.
fn cargo_members(root: &Path) -> Option<(Vec<String>, Vec<String>)> {
    let manifest: toml::Value = toml::from_str(&read(root, "Cargo.toml")?).ok()?;
    let workspace = manifest.get("workspace")?;

    let members: Vec<String> = workspace
        .get("members")?
        .as_array()?
        .iter()
        .filter_map(|value| value.as_str().map(str::to_owned))
        .collect();

    let excluded: Vec<String> = workspace
        .get("exclude")
        .and_then(toml::Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();

    (!members.is_empty()).then_some((members, excluded))
}

/// `turbo.json` marks a monorepo and stops there.
///
/// Turborepo reads the package manager's workspaces; it does not declare packages
/// of its own. Reporting it with no patterns is the honest shape: the tool is
/// present, and something else says what is in it.
fn turborepo(root: &Path) -> (MonorepoTool, Declaration) {
    let declaration = match json(root, "turbo.json") {
        Some(_) => Declaration::present(Vec::new(), NODE),
        None => Declaration::none(),
    };
    (MonorepoTool::Turborepo, declaration)
}

/// `nx.json`, with projects under the layout it declares.
///
/// Nx puts applications and libraries in two directories named by
/// `workspaceLayout`, defaulting to `apps` and `libs` — Nx's own defaults, not a
/// guess. Every candidate still has to hold a `project.json` or a `package.json`
/// to be reported, so a directory that merely sits in the right place is not
/// mistaken for a project.
fn nx(root: &Path) -> (MonorepoTool, Declaration) {
    let Some(manifest) = json(root, "nx.json") else {
        return (MonorepoTool::Nx, Declaration::none());
    };

    let layout = manifest.get("workspaceLayout");
    let directory = |key: &str, fallback: &str| {
        layout
            .and_then(|layout| layout.get(key))
            .and_then(serde_json::Value::as_str)
            .unwrap_or(fallback)
            .trim_matches('/')
            .to_owned()
    };

    let patterns = vec![
        format!("{}/*", directory("appsDir", "apps")),
        format!("{}/*", directory("libsDir", "libs")),
    ];

    (MonorepoTool::Nx, Declaration::present(patterns, NX))
}
