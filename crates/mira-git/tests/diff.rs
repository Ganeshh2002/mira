//! Diffs, against real repositories.
//!
//! Every fixture is an actual repository on disk, for the reason `overview.rs`
//! gives: the thing being tested is agreement with Git, and a fake would only ever
//! agree with itself. That matters more here than anywhere — rename detection,
//! binary sniffing and the shape of a merge diff are all libgit2's judgements, and
//! a double would encode this crate's guesses about them instead.
//!
//! Two things are covered exhaustively, because both are the point of the slice:
//! **every kind of file change**, and **every limit**, each of which must produce a
//! structured value rather than a shorter answer.

use std::fs;
use std::path::Path;

use git2::{Repository, RepositoryInitOptions, Signature};
use mira_git::{
    ChangeKind, ChangedFiles, Comparison, DiffScope, FileChange, FileDiff, FilesTruncated,
    GitProvider, Hunk, Libgit2, LineKind, PatchTruncated, MAX_BYTES, MAX_FILES, MAX_FILE_BYTES,
    MAX_LINES, MAX_LINE_BYTES,
};
use tempfile::TempDir;

// ── Fixtures ─────────────────────────────────────────────────────────────────

const FIRST: i64 = 1_700_000_000;

fn empty_repo(dir: &Path) -> Repository {
    let mut options = RepositoryInitOptions::new();
    options.initial_head("main");
    Repository::init_opts(dir, &options).expect("init")
}

fn who(when: i64) -> Signature<'static> {
    Signature::new(
        "Blacknit",
        "blacknit@example.com",
        &git2::Time::new(when, 0),
    )
    .expect("signature")
}

/// Commit whatever is in the worktree onto HEAD.
fn commit(repo: &Repository, subject: &str, when: i64) -> git2::Oid {
    let mut index = repo.index().expect("index");
    index
        .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
        .expect("add");
    index.write().expect("write index");
    let tree = repo
        .find_tree(index.write_tree().expect("write tree"))
        .expect("tree");

    let parents = match repo.head().ok().and_then(|head| head.target()) {
        Some(oid) => vec![repo.find_commit(oid).expect("parent")],
        None => Vec::new(),
    };
    let borrowed: Vec<&git2::Commit<'_>> = parents.iter().collect();
    let author = who(when);

    repo.commit(Some("HEAD"), &author, &author, subject, &tree, &borrowed)
        .expect("commit")
}

fn write(dir: &Path, path: &str, body: &str) {
    let full = dir.join(path);
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent).expect("create dirs");
    }
    fs::write(full, body).expect("write");
}

fn remove(dir: &Path, path: &str) {
    fs::remove_file(dir.join(path)).expect("remove");
}

fn at(id: git2::Oid) -> DiffScope {
    DiffScope::Commit {
        commit: id.to_string().parse().expect("a commit id"),
    }
}

/// The `Ready` variant, or a panic naming what came back instead.
fn files(found: ChangedFiles) -> (Vec<FileChange>, Comparison, FilesTruncated) {
    match found {
        ChangedFiles::Ready {
            files,
            against,
            truncated,
        } => (files, against, truncated),
        other => panic!("expected a change list, got {other:?}"),
    }
}

fn listed(root: &Path, scope: &DiffScope) -> Vec<FileChange> {
    files(Libgit2.changed_files(root, scope)).0
}

fn one<'a>(changes: &'a [FileChange], path: &str) -> &'a FileChange {
    changes
        .iter()
        .find(|change| change.path == path)
        .unwrap_or_else(|| panic!("no change for {path} in {changes:#?}"))
}

fn patch(found: FileDiff) -> (FileChange, Vec<Hunk>, PatchTruncated) {
    match found {
        FileDiff::Ready {
            change,
            hunks,
            truncated,
        } => (change, hunks, truncated),
        other => panic!("expected a patch, got {other:?}"),
    }
}

/// Every line of a patch, flattened.
fn lines(hunks: &[Hunk]) -> Vec<(LineKind, String)> {
    hunks
        .iter()
        .flat_map(|hunk| hunk.lines.iter().map(|line| (line.kind, line.text.clone())))
        .collect()
}

// ── Every kind of file change ────────────────────────────────────────────────

#[test]
fn an_added_file_reads_as_added() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "one.txt", "one\n");
    commit(&repo, "first", FIRST);
    write(dir.path(), "two.txt", "two\n");
    let second = commit(&repo, "add two", FIRST + 60);

    let changes = listed(dir.path(), &at(second));
    let added = one(&changes, "two.txt");

    assert_eq!(added.kind, ChangeKind::Added);
    assert_eq!(added.kind.letter(), "A");
    assert_eq!(added.additions, Some(1));
    assert_eq!(added.deletions, Some(0));
    assert!(!added.binary);
    assert_eq!(added.from_path, None);
}

#[test]
fn a_deleted_file_reads_as_deleted_and_keeps_its_path() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "gone.txt", "here\nfor now\n");
    write(dir.path(), "stays.txt", "stays\n");
    commit(&repo, "first", FIRST);
    remove(dir.path(), "gone.txt");
    let second = commit(&repo, "remove", FIRST + 60);

    let changes = listed(dir.path(), &at(second));
    let deleted = one(&changes, "gone.txt");

    assert_eq!(deleted.kind, ChangeKind::Deleted);
    assert_eq!(deleted.additions, Some(0));
    assert_eq!(deleted.deletions, Some(2));
}

#[test]
fn a_modified_file_counts_both_sides() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "app.ts", "one\ntwo\nthree\n");
    commit(&repo, "first", FIRST);
    write(dir.path(), "app.ts", "one\nTWO\nthree\nfour\n");
    let second = commit(&repo, "edit", FIRST + 60);

    let changes = listed(dir.path(), &at(second));
    let modified = one(&changes, "app.ts");

    assert_eq!(modified.kind, ChangeKind::Modified);
    assert_eq!(modified.additions, Some(2));
    assert_eq!(modified.deletions, Some(1));
}

#[test]
fn a_rename_is_recognised_and_says_where_it_came_from() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    let body = (0..40).map(|n| format!("line {n}\n")).collect::<String>();
    write(dir.path(), "old.ts", &body);
    commit(&repo, "first", FIRST);
    remove(dir.path(), "old.ts");
    write(dir.path(), "new.ts", &body);
    let second = commit(&repo, "move", FIRST + 60);

    let changes = listed(dir.path(), &at(second));

    assert_eq!(changes.len(), 1, "a rename is one change, not two");
    let renamed = one(&changes, "new.ts");
    assert_eq!(renamed.kind, ChangeKind::Renamed);
    assert_eq!(renamed.from_path.as_deref(), Some("old.ts"));
}

#[test]
fn a_copy_is_recognised_and_says_where_it_came_from() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    let body = (0..60).map(|n| format!("line {n}\n")).collect::<String>();
    write(dir.path(), "source.ts", &body);
    commit(&repo, "first", FIRST);
    write(dir.path(), "copy.ts", &body);
    let second = commit(&repo, "copy", FIRST + 60);

    let changes = listed(dir.path(), &at(second));
    let copied = one(&changes, "copy.ts");

    // libgit2 reports a copy where it can and an addition where it cannot; both
    // are honest, and the test accepts either rather than asserting a heuristic.
    assert!(
        matches!(copied.kind, ChangeKind::Copied | ChangeKind::Added),
        "got {:?}",
        copied.kind
    );
    if copied.kind == ChangeKind::Copied {
        assert_eq!(copied.from_path.as_deref(), Some("source.ts"));
    }
}

#[test]
fn a_type_change_reads_as_a_type_change() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "thing", "a plain file\n");
    commit(&repo, "first", FIRST);

    remove(dir.path(), "thing");
    #[cfg(unix)]
    std::os::unix::fs::symlink("elsewhere", dir.path().join("thing")).expect("symlink");
    #[cfg(not(unix))]
    write(dir.path(), "thing", "still a plain file, changed\n");
    let second = commit(&repo, "become a link", FIRST + 60);

    let changes = listed(dir.path(), &at(second));
    let changed = one(&changes, "thing");

    #[cfg(unix)]
    assert_eq!(changed.kind, ChangeKind::TypeChanged);
    #[cfg(not(unix))]
    assert_eq!(changed.kind, ChangeKind::Modified);
}

#[test]
fn an_empty_commit_lists_nothing_and_does_not_fail() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "one.txt", "one\n");
    let first = commit(&repo, "first", FIRST);

    // A commit with the same tree as its parent. Legal, and made by `--allow-empty`.
    let tree = repo
        .find_commit(first)
        .expect("first")
        .tree()
        .expect("tree");
    let author = who(FIRST + 60);
    let empty = repo
        .commit(
            Some("HEAD"),
            &author,
            &author,
            "nothing at all",
            &tree,
            &[&repo.find_commit(first).expect("first")],
        )
        .expect("empty commit");

    let (changes, against, truncated) = files(Libgit2.changed_files(dir.path(), &at(empty)));

    assert!(changes.is_empty());
    assert_eq!(against, Comparison::Parent);
    assert_eq!(truncated, FilesTruncated::No);
}

#[test]
fn a_root_commit_is_compared_against_nothing() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "one.txt", "one\n");
    write(dir.path(), "two.txt", "two\n");
    let first = commit(&repo, "first", FIRST);

    let (changes, against, _) = files(Libgit2.changed_files(dir.path(), &at(first)));

    assert_eq!(against, Comparison::EmptyTree);
    assert_eq!(changes.len(), 2);
    assert!(changes
        .iter()
        .all(|change| change.kind == ChangeKind::Added));
}

// ── Merges ───────────────────────────────────────────────────────────────────

/// A merge of a side branch into `main`, with each side touching its own file.
fn merged(dir: &Path) -> (Repository, git2::Oid) {
    let repo = empty_repo(dir);
    write(dir, "base.txt", "base\n");
    let base = commit(&repo, "base", FIRST);

    repo.branch("side", &repo.find_commit(base).expect("base"), false)
        .expect("branch");
    write(dir, "on-main.txt", "main\n");
    let main = commit(&repo, "main work", FIRST + 60);

    repo.set_head("refs/heads/side").expect("head");
    repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
        .expect("checkout");
    write(dir, "on-side.txt", "side\n");
    let side = commit(&repo, "side work", FIRST + 120);

    repo.set_head("refs/heads/main").expect("head");
    repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
        .expect("checkout");
    write(dir, "on-side.txt", "side\n");

    let merge = {
        let mut index = repo.index().expect("index");
        index
            .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
            .expect("add");
        index.write().expect("write index");
        let tree = repo
            .find_tree(index.write_tree().expect("write tree"))
            .expect("tree");
        let author = who(FIRST + 180);
        repo.commit(
            Some("HEAD"),
            &author,
            &author,
            "merge side",
            &tree,
            &[
                &repo.find_commit(main).expect("main"),
                &repo.find_commit(side).expect("side"),
            ],
        )
        .expect("merge")
    };

    (repo, merge)
}

#[test]
fn a_merge_is_compared_against_its_first_parent_and_says_so() {
    // The answer would be different against the second parent, so a diff that did
    // not say which side it picked would be quietly choosing one.
    let dir = TempDir::new().expect("tempdir");
    let (_repo, merge) = merged(dir.path());

    let (changes, against, _) = files(Libgit2.changed_files(dir.path(), &at(merge)));

    assert_eq!(against, Comparison::FirstParent { parents: 2 });
    assert_eq!(
        changes.len(),
        1,
        "against the first parent, only the side branch's file is new"
    );
    assert_eq!(one(&changes, "on-side.txt").kind, ChangeKind::Added);
}

// ── Binary files ─────────────────────────────────────────────────────────────

#[test]
fn a_binary_file_is_identified_rather_than_decoded() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "readme.txt", "text\n");
    commit(&repo, "first", FIRST);

    // NUL bytes are what Git sniffs for.
    fs::write(dir.path().join("logo.png"), [0u8, 1, 2, 0, 255, 0, 42]).expect("write binary");
    let second = commit(&repo, "add an image", FIRST + 60);

    let changes = listed(dir.path(), &at(second));
    let binary = one(&changes, "logo.png");

    assert!(binary.binary, "Git considers it binary");
    assert_eq!(binary.additions, None, "there are no lines to count");
    assert_eq!(binary.deletions, None);

    match Libgit2.file_diff(dir.path(), &at(second), binary.at) {
        FileDiff::Binary {
            change, new_bytes, ..
        } => {
            assert_eq!(change.path, "logo.png");
            assert_eq!(new_bytes, Some(7), "the size, not the contents");
        }
        other => panic!("expected a binary file, got {other:?}"),
    }
}

#[test]
fn a_binary_file_never_reaches_the_patch_renderer() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    fs::write(dir.path().join("blob.bin"), [0u8; 64]).expect("write");
    commit(&repo, "first", FIRST);
    fs::write(dir.path().join("blob.bin"), [0u8, 9, 9, 0, 9]).expect("write");
    let second = commit(&repo, "change it", FIRST + 60);

    assert!(matches!(
        Libgit2.file_diff(dir.path(), &at(second), 0),
        FileDiff::Binary { .. }
    ));
}

// ── The limits, each producing a structured state ────────────────────────────

#[test]
fn more_files_than_the_limit_says_how_many_there_were() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "seed.txt", "seed\n");
    commit(&repo, "first", FIRST);

    let many = MAX_FILES + 37;
    for n in 0..many {
        write(dir.path(), &format!("file-{n:04}.txt", n = n), "body\n");
    }
    let second = commit(&repo, "many files", FIRST + 60);

    let (changes, _, truncated) = files(Libgit2.changed_files(dir.path(), &at(second)));

    assert_eq!(changes.len(), MAX_FILES);
    match truncated {
        FilesTruncated::Yes {
            shown,
            total,
            limit,
        } => {
            assert_eq!(shown as usize, MAX_FILES);
            assert_eq!(total as usize, many);
            assert_eq!(limit as usize, MAX_FILES);
        }
        FilesTruncated::No => panic!("the list stopped short and did not say so"),
    }
}

#[test]
fn a_change_set_inside_the_limit_reports_no_truncation() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "one.txt", "one\n");
    let first = commit(&repo, "first", FIRST);

    assert_eq!(
        files(Libgit2.changed_files(dir.path(), &at(first))).2,
        FilesTruncated::No
    );
}

#[test]
fn more_lines_than_the_limit_says_how_many_were_shown() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "seed.txt", "seed\n");
    commit(&repo, "first", FIRST);

    let long: String = (0..MAX_LINES + 500)
        .map(|n| format!("line {n}\n"))
        .collect();
    write(dir.path(), "long.txt", &long);
    let second = commit(&repo, "a long file", FIRST + 60);

    let (_, hunks, truncated) = patch(Libgit2.file_diff(dir.path(), &at(second), 0));

    match truncated {
        PatchTruncated::Lines { shown, limit } => {
            assert_eq!(shown as usize, MAX_LINES);
            assert_eq!(limit as usize, MAX_LINES);
        }
        other => panic!("expected a line limit, got {other:?}"),
    }
    assert_eq!(
        lines(&hunks).len(),
        MAX_LINES,
        "exactly the limit is returned, never one more"
    );
}

#[test]
fn more_bytes_than_the_limit_says_so_before_the_line_limit_would() {
    // The line cap alone is not a byte cap: wide lines exhaust the byte budget
    // first, and the answer has to say which limit actually bit.
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "seed.txt", "seed\n");
    commit(&repo, "first", FIRST);

    let wide = "w".repeat(1_500);
    let body: String = (0..400).map(|_| format!("{wide}\n")).collect();
    write(dir.path(), "wide.txt", &body);
    let second = commit(&repo, "wide lines", FIRST + 60);

    let (_, hunks, truncated) = patch(Libgit2.file_diff(dir.path(), &at(second), 0));

    match truncated {
        PatchTruncated::Bytes { shown, limit } => {
            assert!(shown >= limit, "{shown} should have reached {limit}");
            assert_eq!(limit as usize, MAX_BYTES);
        }
        other => panic!("expected a byte limit, got {other:?}"),
    }
    assert!(lines(&hunks).len() < MAX_LINES, "the byte limit bit first");
}

#[test]
fn a_line_longer_than_the_limit_is_cut_and_marked() {
    // A minified bundle is one line of megabytes. Cutting it keeps the diff
    // readable and keeps one path from spending the whole byte budget.
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "seed.txt", "seed\n");
    commit(&repo, "first", FIRST);

    write(
        dir.path(),
        "bundle.js",
        &format!("{}\n", "x".repeat(MAX_LINE_BYTES * 3)),
    );
    let second = commit(&repo, "a bundle", FIRST + 60);

    let (_, hunks, _) = patch(Libgit2.file_diff(dir.path(), &at(second), 0));
    let line = hunks
        .first()
        .and_then(|hunk| hunk.lines.first())
        .expect("a line");

    assert!(line.cut, "the interface has to be able to say it was cut");
    assert!(line.text.len() <= MAX_LINE_BYTES + 4);
}

#[test]
fn a_file_larger_than_the_limit_is_not_read_at_all() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "seed.txt", "seed\n");
    commit(&repo, "first", FIRST);

    // Text, not binary — so the size limit is what stops it rather than sniffing.
    let huge: String = (0..(MAX_FILE_BYTES / 8 + 4_000))
        .map(|n| format!("{n:07}\n"))
        .collect();
    assert!(huge.len() as u64 > MAX_FILE_BYTES);
    write(dir.path(), "huge.txt", &huge);
    let second = commit(&repo, "a huge file", FIRST + 60);

    match Libgit2.file_diff(dir.path(), &at(second), 0) {
        FileDiff::TooLarge {
            change,
            bytes,
            limit,
        } => {
            assert_eq!(change.path, "huge.txt");
            assert!(bytes > limit);
            assert_eq!(limit, MAX_FILE_BYTES);
        }
        // libgit2's own `max_size` marks an oversized blob binary before Mira
        // sees a size, which is the same refusal reached one step earlier.
        FileDiff::Binary { change, .. } => assert_eq!(change.path, "huge.txt"),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

// ── Reading one file's patch ─────────────────────────────────────────────────

#[test]
fn a_patch_carries_line_numbers_on_both_sides() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "app.ts", "one\ntwo\nthree\n");
    commit(&repo, "first", FIRST);
    write(dir.path(), "app.ts", "one\nTWO\nthree\n");
    let second = commit(&repo, "edit", FIRST + 60);

    let (change, hunks, truncated) = patch(Libgit2.file_diff(dir.path(), &at(second), 0));

    assert_eq!(change.path, "app.ts");
    assert_eq!(truncated, PatchTruncated::No);
    assert_eq!(hunks.len(), 1);
    assert!(
        hunks[0].header.starts_with("@@"),
        "a boundary a reader knows"
    );

    let removed = hunks[0]
        .lines
        .iter()
        .find(|line| line.kind == LineKind::Deletion)
        .expect("a deletion");
    let added = hunks[0]
        .lines
        .iter()
        .find(|line| line.kind == LineKind::Addition)
        .expect("an addition");

    assert_eq!(removed.text, "two");
    assert_eq!(removed.old_line, Some(2));
    assert_eq!(removed.new_line, None);
    assert_eq!(added.text, "TWO");
    assert_eq!(added.new_line, Some(2));
    assert_eq!(added.old_line, None);
}

#[test]
fn a_pure_rename_has_a_change_and_no_patch() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    let body = (0..40).map(|n| format!("line {n}\n")).collect::<String>();
    write(dir.path(), "old.ts", &body);
    commit(&repo, "first", FIRST);
    remove(dir.path(), "old.ts");
    write(dir.path(), "new.ts", &body);
    let second = commit(&repo, "move", FIRST + 60);

    let (change, hunks, truncated) = patch(Libgit2.file_diff(dir.path(), &at(second), 0));

    assert_eq!(change.kind, ChangeKind::Renamed);
    assert!(hunks.is_empty(), "nothing changed inside the file");
    assert_eq!(truncated, PatchTruncated::No);
}

#[test]
fn a_change_that_is_not_there_is_a_stale_selection_rather_than_a_failure() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "one.txt", "one\n");
    let first = commit(&repo, "first", FIRST);

    assert_eq!(
        Libgit2.file_diff(dir.path(), &at(first), 99),
        FileDiff::Unknown
    );
    assert_eq!(
        Libgit2.file_diff(dir.path(), &at(first), u32::MAX),
        FileDiff::Unknown,
        "and an ordinal past every limit is refused before anything is read"
    );
}

#[test]
fn the_ordinal_in_the_list_is_the_ordinal_the_patch_answers_to() {
    // The whole file-selection contract: a caller can only ask for a file by its
    // place in a list Mira produced, so that place has to name the same file back.
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "seed.txt", "seed\n");
    commit(&repo, "first", FIRST);
    for n in 0..6 {
        write(dir.path(), &format!("path-{n}.txt"), "body\n");
    }
    let second = commit(&repo, "six files", FIRST + 60);

    let changes = listed(dir.path(), &at(second));
    assert_eq!(changes.len(), 6);

    for change in &changes {
        let (found, _, _) = patch(Libgit2.file_diff(dir.path(), &at(second), change.at));
        assert_eq!(
            found.path, change.path,
            "ordinal {} named a different file",
            change.at
        );
    }
}

// ── The working tree, kept separate from commits ─────────────────────────────

#[test]
fn the_working_tree_is_a_separate_question_from_any_commit() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "tracked.txt", "one\n");
    let first = commit(&repo, "first", FIRST);

    write(dir.path(), "tracked.txt", "one\ntwo\n");
    write(dir.path(), "untracked.txt", "new\n");

    let (working, against, _) = files(Libgit2.changed_files(dir.path(), &DiffScope::WorkingTree));
    let committed = listed(dir.path(), &at(first));

    assert_eq!(against, Comparison::Head);
    assert_eq!(one(&working, "tracked.txt").kind, ChangeKind::Modified);
    assert_eq!(one(&working, "untracked.txt").kind, ChangeKind::Added);
    assert_eq!(
        committed.len(),
        1,
        "the commit's own change set is untouched by what is on disk"
    );
    assert_eq!(committed[0].path, "tracked.txt");
}

#[test]
fn a_clean_working_tree_lists_nothing() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "one.txt", "one\n");
    commit(&repo, "first", FIRST);

    assert!(
        files(Libgit2.changed_files(dir.path(), &DiffScope::WorkingTree))
            .0
            .is_empty()
    );
}

#[test]
fn a_working_tree_file_can_be_read_by_its_ordinal_too() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "app.ts", "one\ntwo\n");
    commit(&repo, "first", FIRST);
    write(dir.path(), "app.ts", "one\ntwo\nthree\n");

    let changes = listed(dir.path(), &DiffScope::WorkingTree);
    let (change, hunks, _) =
        patch(Libgit2.file_diff(dir.path(), &DiffScope::WorkingTree, changes[0].at));

    assert_eq!(change.path, "app.ts");
    assert!(lines(&hunks)
        .iter()
        .any(|(kind, text)| *kind == LineKind::Addition && text == "three"));
}

#[test]
fn a_repository_with_no_commits_still_answers_about_its_working_tree() {
    let dir = TempDir::new().expect("tempdir");
    empty_repo(dir.path());
    write(dir.path(), "first.txt", "before any commit\n");

    let (changes, against, _) = files(Libgit2.changed_files(dir.path(), &DiffScope::WorkingTree));

    assert_eq!(against, Comparison::Head);
    assert_eq!(one(&changes, "first.txt").kind, ChangeKind::Added);
}

// ── Unusual and unreadable repositories ──────────────────────────────────────

#[test]
fn a_plain_directory_has_no_changes_to_show() {
    let dir = TempDir::new().expect("tempdir");

    assert_eq!(
        Libgit2.changed_files(dir.path(), &DiffScope::WorkingTree),
        ChangedFiles::NotARepository
    );
    assert_eq!(
        Libgit2.file_diff(dir.path(), &DiffScope::WorkingTree, 0),
        FileDiff::NotARepository
    );
}

#[test]
fn a_commit_that_is_not_here_is_unknown_rather_than_a_failure() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "one.txt", "one\n");
    commit(&repo, "first", FIRST);

    let absent = DiffScope::Commit {
        commit: "0123456789abcdef0123456789abcdef01234567"
            .parse()
            .expect("id"),
    };

    assert_eq!(
        Libgit2.changed_files(dir.path(), &absent),
        ChangedFiles::Unknown
    );
    assert_eq!(Libgit2.file_diff(dir.path(), &absent, 0), FileDiff::Unknown);
}

#[test]
fn an_unreadable_repository_says_why() {
    let dir = TempDir::new().expect("tempdir");
    fs::create_dir(dir.path().join(".git")).expect("create .git");
    fs::write(dir.path().join(".git").join("HEAD"), "not a ref").expect("write");

    match Libgit2.changed_files(dir.path(), &DiffScope::WorkingTree) {
        ChangedFiles::Unreadable { detail } => assert!(!detail.is_empty()),
        other => panic!("expected an unreadable repository, got {other:?}"),
    }
}

#[test]
fn a_commit_whose_object_is_missing_is_unknown() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "one.txt", "one\n");
    let first = commit(&repo, "first", FIRST);
    write(dir.path(), "two.txt", "two\n");
    let second = commit(&repo, "second", FIRST + 60);

    let sha = second.to_string();
    let (prefix, rest) = sha.split_at(2);
    let object = repo.path().join("objects").join(prefix).join(rest);
    assert!(object.exists(), "expected a loose object to remove");
    fs::remove_file(&object).expect("remove object");

    match Libgit2.changed_files(dir.path(), &at(second)) {
        ChangedFiles::Unknown => {}
        ChangedFiles::Unreadable { detail } => assert!(!detail.is_empty()),
        other => panic!("expected unknown or unreadable, got {other:?}"),
    }
    // And the commit that is still there is still readable.
    assert!(matches!(
        Libgit2.changed_files(dir.path(), &at(first)),
        ChangedFiles::Ready { .. }
    ));
}

#[test]
fn a_shallow_repository_reads_the_commits_it_has() {
    // A shallow clone's boundary commit has a parent Git does not hold. Diffing
    // it must produce a picture rather than an error about the missing parent.
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "one.txt", "one\n");
    commit(&repo, "first", FIRST);
    write(dir.path(), "two.txt", "two\n");
    let second = commit(&repo, "second", FIRST + 60);

    fs::write(repo.path().join("shallow"), format!("{second}\n")).expect("write shallow");

    match Libgit2.changed_files(dir.path(), &at(second)) {
        ChangedFiles::Ready { against, .. } => {
            assert!(matches!(
                against,
                Comparison::Parent | Comparison::EmptyTree
            ));
        }
        ChangedFiles::Unreadable { detail } => assert!(!detail.is_empty()),
        other => panic!("expected a readable answer, got {other:?}"),
    }
}

// ── The provider boundary ────────────────────────────────────────────────────

#[test]
fn diffs_are_read_through_the_trait_like_everything_else() {
    fn read(provider: &dyn GitProvider, root: &Path) -> ChangedFiles {
        provider.changed_files(root, &DiffScope::WorkingTree)
    }

    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "one.txt", "one\n");
    commit(&repo, "first", FIRST);
    write(dir.path(), "one.txt", "changed\n");

    assert!(matches!(
        read(&Libgit2, dir.path()),
        ChangedFiles::Ready { .. }
    ));
}

#[test]
fn reading_a_diff_changes_nothing_about_the_repository() {
    let dir = TempDir::new().expect("tempdir");
    let (repo, merge) = merged(dir.path());

    let before: Vec<String> = repo
        .references()
        .expect("refs")
        .flatten()
        .filter_map(|reference| {
            let name = reference.name().ok()?.to_owned();
            let target = reference.target()?;
            Some(format!("{name}={target}"))
        })
        .collect();
    let head_before = repo.head().expect("head").target();
    let status_before = repo.statuses(None).expect("statuses").len();

    let _ = Libgit2.changed_files(dir.path(), &at(merge));
    let _ = Libgit2.file_diff(dir.path(), &at(merge), 0);
    let _ = Libgit2.changed_files(dir.path(), &DiffScope::WorkingTree);

    let after: Vec<String> = repo
        .references()
        .expect("refs")
        .flatten()
        .filter_map(|reference| {
            let name = reference.name().ok()?.to_owned();
            let target = reference.target()?;
            Some(format!("{name}={target}"))
        })
        .collect();

    assert_eq!(before, after, "no reference moved");
    assert_eq!(
        head_before,
        repo.head().expect("head").target(),
        "HEAD did not move"
    );
    assert_eq!(
        status_before,
        repo.statuses(None).expect("statuses").len(),
        "nothing was staged, restored or written"
    );
}

#[test]
fn every_path_is_written_with_one_separator_on_every_platform() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "seed.txt", "seed\n");
    commit(&repo, "first", FIRST);
    write(dir.path(), "src/deep/nested/file.ts", "nested\n");
    let second = commit(&repo, "nested", FIRST + 60);

    let changes = listed(dir.path(), &at(second));

    assert_eq!(changes[0].path, "src/deep/nested/file.ts");
    assert!(!changes[0].path.contains('\\'));
}

#[test]
fn a_new_untracked_file_shows_what_is_in_it() {
    // libgit2 reports an untracked path without its contents unless asked, which
    // would make every new file read "+0 −0" — the least useful half of the
    // answer. It goes through the same size gate as anything else.
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "tracked.txt", "tracked\n");
    commit(&repo, "first", FIRST);

    write(
        dir.path(),
        "brand-new.ts",
        "export const x = 1;\nexport const y = 2;\n",
    );

    let changes = listed(dir.path(), &DiffScope::WorkingTree);
    let new = one(&changes, "brand-new.ts");

    assert_eq!(new.kind, ChangeKind::Added);
    assert_eq!(new.additions, Some(2), "a new file's lines are counted");
    assert_eq!(new.deletions, Some(0));

    let (_, hunks, _) = patch(Libgit2.file_diff(dir.path(), &DiffScope::WorkingTree, new.at));
    let shown = lines(&hunks);

    assert_eq!(shown.len(), 2, "and its contents are readable");
    assert!(shown.iter().all(|(kind, _)| *kind == LineKind::Addition));
    assert_eq!(shown[0].1, "export const x = 1;");
}
