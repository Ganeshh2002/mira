//! Working out how the selected directory sits in its repository.

use std::path::{Path, PathBuf};

use crate::expand;
use crate::model::{MonorepoTool, Package, RepositoryLayout};
use crate::providers::{self, Declaration};

/// How far the search climbs when there is no Git root to stop it.
///
/// A Git root is the natural boundary — a workspace cannot span repositories —
/// and it is present for every repository Mira reads. This cap only governs the
/// unusual case of a project with no Git at all, where climbing to `/` would mean
/// reading manifests in directories that have nothing to do with the project.
const MAX_CLIMB: usize = 8;

/// How many packages one monorepo may report.
///
/// Past this the list has stopped being something a person reads. Truncating is
/// better than a view that takes a second to render, and the number is high
/// enough that no real repository meets it by accident.
const MAX_PACKAGES: usize = 500;

/// How `selected` relates to the repository around it.
///
/// `git_root` bounds the search upwards. Pass the worktree root when the project
/// is in a repository, and `None` when it is not; nothing here reads Git itself,
/// which is what keeps this crate independent of `mira-git`.
///
/// Never fails. Every unreadable manifest, missing directory, and exhausted
/// budget resolves to *less detected*, and the honest floor is
/// [`RepositoryLayout::Standalone`].
#[must_use]
pub fn detect(selected: &Path, git_root: Option<&Path>) -> RepositoryLayout {
    if let Some(layout) = at(selected) {
        return layout;
    }

    for ancestor in ancestors(selected, git_root) {
        let found = declarations(&ancestor);
        if found.is_empty() {
            continue;
        }

        let packages = packages_of(&ancestor, &found);
        let Some(package) = packages
            .iter()
            .find(|package| ancestor.join(&package.path) == selected)
        else {
            // The workspace above does not list this directory. It is inside the
            // monorepo but is not one of its packages — a `src` directory, say —
            // and there is no boundary here to report.
            continue;
        };

        return RepositoryLayout::Package {
            tools: found.iter().map(|(tool, _)| *tool).collect(),
            monorepo_root: ancestor.display().to_string(),
            package_path: package.path.clone(),
            package_name: package.name.clone(),
        };
    }

    RepositoryLayout::Standalone
}

/// The layout when `root` is itself a monorepo root.
fn at(root: &Path) -> Option<RepositoryLayout> {
    let found = declarations(root);
    if found.is_empty() {
        return None;
    }

    // A tool's manifest being present is not enough. A `pnpm-workspace.yaml` Mira
    // cannot read, a `turbo.json` with no package manager behind it, an `nx.json`
    // in a repository with no projects — each of those is a monorepo claim with
    // nothing behind it, and "only show boundaries that can be detected reliably"
    // means one confirmed package is the price of making the claim.
    let packages = packages_of(root, &found);
    if packages.is_empty() {
        return None;
    }

    Some(RepositoryLayout::MonorepoRoot {
        tools: found.iter().map(|(tool, _)| *tool).collect(),
        packages,
    })
}

/// Every tool whose manifest is at `root`.
fn declarations(root: &Path) -> Vec<(MonorepoTool, Declaration)> {
    providers::ALL
        .iter()
        .map(|provider| provider(root))
        .filter(|(_, declaration)| declaration.present)
        .collect()
}

/// The directories the declarations point at, confirmed and named.
fn packages_of(root: &Path, found: &[(MonorepoTool, Declaration)]) -> Vec<Package> {
    let mut packages: Vec<Package> = Vec::new();

    for (_, declaration) in found {
        for directory in expand::directories(root, &declaration.patterns) {
            if packages.len() >= MAX_PACKAGES {
                return finish(packages);
            }
            let Some(relative) = relative(root, &directory) else {
                continue;
            };
            if declaration
                .exclude
                .iter()
                .any(|path| path.trim_matches('/') == relative)
            {
                continue;
            }
            if packages.iter().any(|package| package.path == relative) {
                continue;
            }
            // A directory is a package only if it holds the manifest that would
            // make it one. Everything else is a directory in the right place,
            // which is not the same thing and must not be named as if it were.
            let Some(name) = name_of(&directory, declaration.manifests) else {
                continue;
            };
            packages.push(Package {
                name,
                path: relative,
            });
        }
    }

    finish(packages)
}

fn finish(mut packages: Vec<Package>) -> Vec<Package> {
    // By path, not by name: two packages may legitimately share a name, and the
    // path is the thing that is actually unique.
    packages.sort_by(|a, b| a.path.cmp(&b.path));
    packages
}

/// A package's own name, or its directory's, or nothing at all.
fn name_of(directory: &Path, manifests: &[&str]) -> Option<String> {
    let mut declared = None;

    for manifest in manifests {
        let path = directory.join(manifest);
        let Ok(source) = std::fs::read_to_string(&path) else {
            continue;
        };

        declared = declared.or_else(|| match *manifest {
            "Cargo.toml" => toml::from_str::<toml::Value>(&source)
                .ok()?
                .get("package")?
                .get("name")?
                .as_str()
                .map(str::to_owned),
            _ => serde_json::from_str::<serde_json::Value>(&source)
                .ok()?
                .get("name")?
                .as_str()
                .map(str::to_owned),
        });

        // The manifest exists, so this is a package. A missing or unreadable
        // `name` falls back to the directory, which is still a fact about disk.
        return Some(declared.unwrap_or_else(|| {
            directory
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default()
        }));
    }

    None
}

/// `directory` relative to `root`, with `/` separators for display and comparison.
fn relative(root: &Path, directory: &Path) -> Option<String> {
    let relative = directory.strip_prefix(root).ok()?;
    let joined = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");

    (!joined.is_empty()).then_some(joined)
}

/// The directories above `selected`, stopping at `git_root` or the climb cap.
fn ancestors(selected: &Path, git_root: Option<&Path>) -> Vec<PathBuf> {
    let mut ancestors = Vec::new();

    for ancestor in selected.ancestors().skip(1).take(MAX_CLIMB) {
        ancestors.push(ancestor.to_path_buf());
        if git_root.is_some_and(|root| root == ancestor) {
            break;
        }
    }

    // Without a Git root the walk is bounded only by the cap; with one it stops
    // there, because a workspace declared in another repository says nothing
    // about this one.
    if let Some(root) = git_root {
        ancestors.retain(|ancestor| ancestor.starts_with(root));
    }

    ancestors
}
