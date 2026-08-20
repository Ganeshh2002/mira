//! Turning workspace globs into directories, within a budget.
//!
//! Every package manager declares its packages as patterns — `apps/*`,
//! `crates/*`, sometimes `**`. Expanding them means walking directories, and a
//! repository is exactly the place where an unbounded walk goes wrong: a
//! `node_modules` holds tens of thousands of manifests, and `**` at the root of a
//! large monorepo is a request to read the whole tree.
//!
//! So the walk is bounded three ways — a depth cap, a visit budget, and a skip
//! list — and the bounds are constants a person can read rather than heuristics.
//! Hitting one yields fewer packages, never a hang and never an error: a monorepo
//! whose shape Mira cannot fully see is still more useful than a spinner.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// How many segments a `**` may stand in for.
///
/// Four covers every real layout — `packages/group/name` is already unusual — and
/// stops a pathological tree from turning detection into a full-disk scan.
const MAX_DEPTH: usize = 4;

/// How many directory entries one detection may look at, in total.
const VISIT_BUDGET: usize = 4_000;

/// Directories that never contain this repository's own packages.
///
/// `node_modules` is the one that matters: it is full of `package.json` files
/// belonging to other people's code, and treating them as packages of this
/// repository would be both slow and wrong.
const SKIP: [&str; 12] = [
    "node_modules",
    ".git",
    "target",
    "dist",
    "build",
    "out",
    ".next",
    ".turbo",
    ".nx",
    "vendor",
    ".venv",
    "__pycache__",
];

/// Every directory under `root` matching any of `patterns`, sorted and deduplicated.
///
/// Patterns are matched segment by segment: `*` stands for exactly one directory,
/// `**` for up to [`MAX_DEPTH`] of them. A pattern with no wildcard is a direct
/// path, checked rather than searched.
#[must_use]
pub fn directories(root: &Path, patterns: &[String]) -> Vec<PathBuf> {
    let mut found = BTreeSet::new();
    let mut budget = VISIT_BUDGET;

    for pattern in patterns {
        let segments: Vec<&str> = pattern
            .split('/')
            .filter(|segment| !segment.is_empty() && *segment != ".")
            .collect();

        if segments.is_empty() {
            continue;
        }
        walk(root, &segments, root, &mut found, &mut budget);
    }

    found.into_iter().collect()
}

fn walk(
    current: &Path,
    remaining: &[&str],
    root: &Path,
    found: &mut BTreeSet<PathBuf>,
    budget: &mut usize,
) {
    let Some((segment, rest)) = remaining.split_first() else {
        // The pattern is exhausted: this directory is a match, unless it is the
        // root itself — a repository is not one of its own packages.
        if current != root && current.is_dir() {
            found.insert(current.to_path_buf());
        }
        return;
    };

    if *segment == "**" {
        // `**` matches nothing at all, then one level, then two, up to the cap.
        walk(current, rest, root, found, budget);
        descend(current, remaining, root, found, budget, MAX_DEPTH);
        return;
    }

    if segment.contains('*') {
        for child in children(current, budget) {
            if matches(segment, &name_of(&child)) {
                walk(&child, rest, root, found, budget);
            }
        }
        return;
    }

    let child = current.join(segment);
    if child.is_dir() {
        walk(&child, rest, root, found, budget);
    }
}

/// Keep applying a `**` one directory deeper, until the depth cap.
fn descend(
    current: &Path,
    remaining: &[&str],
    root: &Path,
    found: &mut BTreeSet<PathBuf>,
    budget: &mut usize,
    depth: usize,
) {
    if depth == 0 {
        return;
    }
    for child in children(current, budget) {
        walk(&child, &remaining[1..], root, found, budget);
        descend(&child, remaining, root, found, budget, depth - 1);
    }
}

/// The sub-directories of `dir` worth looking at, spending from the budget.
fn children(dir: &Path, budget: &mut usize) -> Vec<PathBuf> {
    if *budget == 0 {
        return Vec::new();
    }

    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut children = Vec::new();
    for entry in entries.flatten() {
        if *budget == 0 {
            break;
        }
        *budget -= 1;

        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') || SKIP.contains(&name.as_ref()) {
            continue;
        }
        // `file_type` does not follow symlinks, which is what keeps a link back up
        // the tree from turning the walk into a cycle.
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            children.push(entry.path());
        }
    }

    children.sort();
    children
}

fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Whether one path segment matches a pattern segment containing `*`.
fn matches(pattern: &str, name: &str) -> bool {
    let mut parts = pattern.split('*');
    let Some(first) = parts.next() else {
        return false;
    };
    if !name.starts_with(first) {
        return false;
    }

    let mut rest = &name[first.len()..];
    let parts: Vec<&str> = parts.collect();

    if parts.is_empty() {
        // No `*` in the pattern at all: `web` names `web`, and nothing else.
        return rest.is_empty();
    }

    for (index, part) in parts.iter().enumerate() {
        if index == parts.len() - 1 {
            // The final literal has to sit at the very end, or `*` would be
            // allowed to swallow it.
            return rest.len() >= part.len() && rest.ends_with(part);
        }
        match rest.find(part) {
            Some(at) => rest = &rest[at + part.len()..],
            None => return false,
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::matches;

    #[test]
    fn a_bare_star_matches_any_segment() {
        assert!(matches("*", "web"));
        assert!(matches("*", ""));
    }

    #[test]
    fn a_prefix_and_suffix_both_have_to_hold() {
        assert!(matches("mira-*", "mira-core"));
        assert!(!matches("mira-*", "aviora-core"));
        assert!(matches("*-core", "mira-core"));
        assert!(!matches("*-core", "mira-db"));
    }

    #[test]
    fn a_literal_segment_matches_only_itself() {
        assert!(matches("web", "web"));
        assert!(!matches("web", "website"));
    }

    #[test]
    fn a_star_in_the_middle_spans_whatever_is_between() {
        assert!(matches("app-*-web", "app-legacy-web"));
        assert!(!matches("app-*-web", "app-legacy-api"));
    }
}
