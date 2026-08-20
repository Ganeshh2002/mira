//! Project type detection.
//!
//! Markers are read from the files sitting directly inside the project root and
//! nowhere else. A recursive walk is the one thing this must never do: adding a
//! directory with a large `node_modules` would then take seconds and read
//! thousands of files Mira has no business reading (`prd.md` feature 1, risks).

use std::path::Path;

/// The marker files Mira recognises, and the type each one means.
///
/// Data, not code — extending it is adding a row (`architecture.md` §12).
pub const MARKERS: &[(&str, &str)] = &[
    ("package.json", "node"),
    ("Cargo.toml", "rust"),
    ("go.mod", "go"),
    ("pyproject.toml", "python"),
    ("requirements.txt", "python"),
    ("pom.xml", "java"),
    ("build.gradle", "java"),
    ("build.gradle.kts", "java"),
    ("Gemfile", "ruby"),
    ("composer.json", "php"),
    ("Makefile", "make"),
    ("docker-compose.yml", "docker"),
    ("docker-compose.yaml", "docker"),
    ("compose.yml", "docker"),
    ("compose.yaml", "docker"),
];

/// The suffix that marks a .NET solution, which has no fixed file name.
const SOLUTION: &str = ".sln";

/// Every type marker found at the root of `root`, sorted and deduplicated.
pub fn detect(root: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(root) else {
        // An unreadable directory is not a detection failure worth reporting: the
        // project is still perfectly usable, it simply has no markers.
        return Vec::new();
    };

    let mut found: Vec<String> = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| classify(&entry.file_name().to_string_lossy()))
        .collect();

    found.sort_unstable();
    found.dedup();
    found
}

fn classify(file_name: &str) -> Option<String> {
    if file_name.ends_with(SOLUTION) {
        return Some("dotnet".to_owned());
    }

    MARKERS
        .iter()
        .find(|(name, _)| *name == file_name)
        .map(|(_, marker)| (*marker).to_owned())
}
