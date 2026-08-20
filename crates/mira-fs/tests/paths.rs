//! Path safety.
//!
//! Two jobs, both load-bearing. Canonicalisation decides project identity — a
//! project *is* its canonical root, so two spellings of one directory must not
//! become two projects (`prd.md` FR-1.1). Containment decides what Mira is allowed
//! to read: every later slice reads files, and every one of those reads must be
//! provably inside a root the user registered (`security-and-privacy.md` §5).

use std::fs;

use mira_core::MiraError;
use mira_fs::{canonical_dir, contains, PathMatching};
use tempfile::TempDir;

#[test]
fn a_directory_canonicalises_to_an_absolute_path() {
    let dir = TempDir::new().expect("tempdir");
    let resolved = canonical_dir(dir.path()).expect("canonicalise");

    assert!(resolved.is_absolute(), "{resolved:?} must be absolute");
    assert!(resolved.exists());
}

#[test]
fn a_relative_path_resolves_against_the_filesystem() {
    let dir = TempDir::new().expect("tempdir");
    fs::create_dir(dir.path().join("inner")).expect("mkdir");

    let with_dots = dir.path().join("inner").join("..").join("inner");
    let resolved = canonical_dir(&with_dots).expect("canonicalise");

    assert_eq!(
        resolved,
        canonical_dir(&dir.path().join("inner")).expect("canonicalise"),
        "`..` segments are resolved away, so one directory has one spelling"
    );
}

#[test]
fn a_missing_directory_is_reported_as_not_found() {
    let dir = TempDir::new().expect("tempdir");
    let missing = dir.path().join("nope");

    match canonical_dir(&missing) {
        Err(MiraError::NotFound { what }) => {
            assert!(
                what.contains("nope"),
                "the message names the path the user chose: {what}"
            );
        }
        other => panic!("a missing directory must be NotFound, got {other:?}"),
    }
}

#[test]
fn a_file_is_rejected_because_a_project_is_a_directory() {
    let dir = TempDir::new().expect("tempdir");
    let file = dir.path().join("README.md");
    fs::write(&file, "hello").expect("write");

    match canonical_dir(&file) {
        Err(MiraError::Invalid { field, detail }) => {
            assert_eq!(field, "path");
            assert!(
                detail.contains("directory"),
                "the message says what was wrong with it: {detail}"
            );
        }
        other => panic!("a file must be Invalid, got {other:?}"),
    }
}

#[test]
fn an_empty_path_is_rejected_before_it_reaches_the_filesystem() {
    match canonical_dir(std::path::Path::new("")) {
        Err(MiraError::Invalid { field, .. }) => assert_eq!(field, "path"),
        other => panic!("an empty path must be Invalid, got {other:?}"),
    }
}

// ── Containment ──────────────────────────────────────────────────────────────

#[test]
fn a_path_inside_a_root_is_contained() {
    let root = std::path::Path::new("/home/dev/aviora");
    assert!(contains(
        root,
        std::path::Path::new("/home/dev/aviora/src/main.rs"),
        PathMatching::CaseSensitive
    ));
}

#[test]
fn a_root_contains_itself() {
    let root = std::path::Path::new("/home/dev/aviora");
    assert!(contains(root, root, PathMatching::CaseSensitive));
}

#[test]
fn a_sibling_with_a_shared_prefix_is_not_contained() {
    // The bug this test exists for: `starts_with` on strings says
    // /home/dev/aviora-secrets begins with /home/dev/aviora. Comparing whole path
    // components is what stops one project's root leaking into its neighbour.
    assert!(!contains(
        std::path::Path::new("/home/dev/aviora"),
        std::path::Path::new("/home/dev/aviora-secrets/key.txt"),
        PathMatching::CaseSensitive
    ));
}

#[test]
fn a_parent_is_not_contained_by_its_child() {
    assert!(!contains(
        std::path::Path::new("/home/dev/aviora/src"),
        std::path::Path::new("/home/dev/aviora"),
        PathMatching::CaseSensitive
    ));
}

#[test]
fn case_matters_only_where_the_filesystem_says_it_does() {
    let root = std::path::Path::new("/Users/dev/Aviora");
    let candidate = std::path::Path::new("/users/dev/aviora/src");

    assert!(
        contains(root, candidate, PathMatching::CaseInsensitive),
        "macOS and Windows treat these as one directory"
    );
    assert!(
        !contains(root, candidate, PathMatching::CaseSensitive),
        "Linux treats them as two, and Mira does not guess otherwise"
    );
}

#[test]
fn the_same_directory_spelled_differently_compares_equal_where_case_is_ignored() {
    assert!(PathMatching::CaseInsensitive.same_path(
        std::path::Path::new("/Users/dev/Aviora"),
        std::path::Path::new("/users/DEV/aviora")
    ));
    assert!(!PathMatching::CaseSensitive.same_path(
        std::path::Path::new("/Users/dev/Aviora"),
        std::path::Path::new("/users/DEV/aviora")
    ));
}

#[test]
fn the_platform_answer_selects_the_matching_rule() {
    // `mira-platform` reports whether this filesystem is case-sensitive; nothing in
    // `mira-fs` may ask which operating system produced that answer (ADR-0005).
    assert_eq!(
        PathMatching::from_case_sensitivity(true),
        PathMatching::CaseSensitive
    );
    assert_eq!(
        PathMatching::from_case_sensitivity(false),
        PathMatching::CaseInsensitive
    );
}
