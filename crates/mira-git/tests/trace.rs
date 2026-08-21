//! File history, against real repositories.
//!
//! The thing being tested is agreement with Git about what "this file's history"
//! means — which commits count, what happens across a rename, where a trace stops
//! — and every one of those is libgit2's judgement or a deliberate choice made
//! against it. A double would only encode the guesses.
//!
//! Two properties get the most attention, because they are what the slice is for:
//! **no path is ever sent** (a file is named by its position in a change set Mira
//! produced), and **the walk is bounded by commits examined** rather than by
//! anything about the file.

use std::fs;
use std::path::Path;

use git2::{Repository, RepositoryInitOptions, Signature};
use mira_git::{
    ChangeKind, ChangedFiles, CommitId, DiffScope, FileCommit, FileCursor, FileHistory,
    FileSubject, GitProvider, Libgit2, ScanStopped, MAX_SCAN, PAGE,
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

/// The subject naming `path` in a commit's change set — which is the only way a
/// file can be named, here or across the IPC boundary.
fn subject_for(root: &Path, scope: DiffScope, path: &str) -> FileSubject {
    let ChangedFiles::Ready { files, .. } = Libgit2.changed_files(root, &scope) else {
        panic!("expected a change list for {path}");
    };
    let found = files
        .iter()
        .find(|change| change.path == path)
        .unwrap_or_else(|| panic!("no change for {path} in {files:#?}"));

    FileSubject {
        scope,
        at: found.at,
        before: false,
    }
}

struct Traced {
    path: String,
    commits: Vec<FileCommit>,
    next: Option<FileCursor>,
    scanned: u32,
    stopped: ScanStopped,
    shallow: bool,
}

fn ready(found: FileHistory) -> Traced {
    match found {
        FileHistory::Ready {
            path,
            commits,
            next,
            scanned,
            stopped,
            shallow,
        } => Traced {
            path,
            commits,
            next,
            scanned,
            stopped,
            shallow,
        },
        other => panic!("expected a trace, got {other:?}"),
    }
}

fn trace(root: &Path, subject: &FileSubject, from: Option<&CommitId>) -> Traced {
    ready(Libgit2.file_history(root, subject, from))
}

fn subjects(traced: &Traced) -> Vec<&str> {
    traced
        .commits
        .iter()
        .map(|found| found.commit.subject.as_str())
        .collect()
}

// ── Normal history ───────────────────────────────────────────────────────────

#[test]
fn a_files_history_is_the_commits_that_touched_it_and_no_others() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());

    write(dir.path(), "app.ts", "one\n");
    let first = commit(&repo, "add app", FIRST);
    write(dir.path(), "other.ts", "other\n");
    commit(&repo, "add other — not about app", FIRST + 60);
    write(dir.path(), "app.ts", "one\ntwo\n");
    let third = commit(&repo, "edit app", FIRST + 120);
    write(dir.path(), "other.ts", "other again\n");
    commit(&repo, "edit other — still not about app", FIRST + 180);

    let subject = subject_for(dir.path(), at(third), "app.ts");
    let traced = trace(dir.path(), &subject, None);

    assert_eq!(traced.path, "app.ts");
    assert_eq!(subjects(&traced), ["edit app", "add app"]);
    assert_eq!(traced.commits[0].kind, ChangeKind::Modified);
    assert_eq!(traced.commits[1].kind, ChangeKind::Added);
    assert_eq!(traced.stopped, ScanStopped::No);
    assert_eq!(traced.next, None);
    assert_eq!(traced.commits[1].commit.sha, first.to_string());
}

#[test]
fn every_commit_carries_what_a_row_shows() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "app.ts", "one\n");
    let first = commit(&repo, "add app", FIRST);

    let traced = trace(
        dir.path(),
        &subject_for(dir.path(), at(first), "app.ts"),
        None,
    );
    let only = &traced.commits[0];

    assert_eq!(only.commit.subject, "add app");
    assert_eq!(only.commit.author, "Blacknit");
    assert_eq!(only.commit.committed_at, FIRST);
    assert_eq!(only.commit.short_sha.len(), 7);
    assert!(only.commit.sha.starts_with(&only.commit.short_sha));
    assert_eq!(only.path, "app.ts");
}

#[test]
fn a_file_with_one_commit_has_a_history_of_one() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "only.ts", "only\n");
    let first = commit(&repo, "the only commit", FIRST);

    let traced = trace(
        dir.path(),
        &subject_for(dir.path(), at(first), "only.ts"),
        None,
    );

    assert_eq!(traced.commits.len(), 1);
    assert_eq!(traced.commits[0].kind, ChangeKind::Added);
    assert_eq!(traced.stopped, ScanStopped::No);
}

#[test]
fn a_root_commit_ends_the_trace() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "app.ts", "one\n");
    commit(&repo, "root", FIRST);
    write(dir.path(), "app.ts", "two\n");
    let second = commit(&repo, "edit", FIRST + 60);

    let traced = trace(
        dir.path(),
        &subject_for(dir.path(), at(second), "app.ts"),
        None,
    );

    assert_eq!(subjects(&traced), ["edit", "root"]);
    assert_eq!(traced.next, None, "there is nothing below a root commit");
    assert_eq!(traced.commits[1].kind, ChangeKind::Added);
}

// ── Renames and copies ───────────────────────────────────────────────────────

#[test]
fn a_trace_follows_a_file_across_a_rename() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());

    let body = (0..40).map(|n| format!("line {n}\n")).collect::<String>();
    write(dir.path(), "old.ts", &body);
    commit(&repo, "add under the old name", FIRST);
    write(dir.path(), "old.ts", &format!("{body}extra\n"));
    commit(&repo, "edit under the old name", FIRST + 60);

    remove(dir.path(), "old.ts");
    write(dir.path(), "new.ts", &format!("{body}extra\n"));
    let renamed = commit(&repo, "rename", FIRST + 120);

    write(dir.path(), "new.ts", &format!("{body}extra\nmore\n"));
    let latest = commit(&repo, "edit under the new name", FIRST + 180);

    let traced = trace(
        dir.path(),
        &subject_for(dir.path(), at(latest), "new.ts"),
        None,
    );

    assert_eq!(
        subjects(&traced),
        [
            "edit under the new name",
            "rename",
            "edit under the old name",
            "add under the old name"
        ],
        "the history continues under the name the file used to have"
    );

    let crossing = &traced.commits[1];
    assert_eq!(crossing.kind, ChangeKind::Renamed);
    assert_eq!(crossing.renamed_from.as_deref(), Some("old.ts"));
    assert_eq!(crossing.commit.sha, renamed.to_string());

    assert_eq!(
        traced.commits[2].path, "old.ts",
        "and rows below it are labelled with the name they had"
    );
}

#[test]
fn a_trace_of_a_copy_says_where_it_was_copied_from() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());

    let body = (0..60).map(|n| format!("line {n}\n")).collect::<String>();
    write(dir.path(), "source.ts", &body);
    commit(&repo, "add the source", FIRST);
    write(dir.path(), "copy.ts", &body);
    let copied = commit(&repo, "copy it", FIRST + 60);

    let traced = trace(
        dir.path(),
        &subject_for(dir.path(), at(copied), "copy.ts"),
        None,
    );

    // libgit2 reports a copy where it can and an addition where it cannot. Both
    // are honest; the test accepts either rather than asserting a heuristic.
    let first = &traced.commits[0];
    assert!(
        matches!(first.kind, ChangeKind::Copied | ChangeKind::Added),
        "got {:?}",
        first.kind
    );
    if first.kind == ChangeKind::Copied {
        assert_eq!(first.renamed_from.as_deref(), Some("source.ts"));
        assert!(
            traced.commits.len() > 1,
            "a copy continues into the history it was copied from"
        );
    }
}

#[test]
fn a_deleted_file_still_has_a_history() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "gone.ts", "one\n");
    commit(&repo, "add", FIRST);
    write(dir.path(), "gone.ts", "one\ntwo\n");
    commit(&repo, "edit", FIRST + 60);
    write(dir.path(), "keep.ts", "keep\n");
    remove(dir.path(), "gone.ts");
    let deleted = commit(&repo, "delete it", FIRST + 120);

    let traced = trace(
        dir.path(),
        &subject_for(dir.path(), at(deleted), "gone.ts"),
        None,
    );

    assert_eq!(subjects(&traced), ["delete it", "edit", "add"]);
    assert_eq!(traced.commits[0].kind, ChangeKind::Deleted);
}

// ── Merges ───────────────────────────────────────────────────────────────────

#[test]
fn a_merge_that_brought_a_change_to_the_file_is_in_its_history() {
    // Compared against the first parent, like every other diff in Mira: the merge
    // brought the side branch's edit onto the mainline, so it changed the file.
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());

    write(dir.path(), "app.ts", "base\n");
    let base = commit(&repo, "base", FIRST);

    repo.branch("side", &repo.find_commit(base).expect("base"), false)
        .expect("branch");
    write(dir.path(), "other.ts", "main work\n");
    let main = commit(&repo, "main work", FIRST + 60);

    repo.set_head("refs/heads/side").expect("head");
    repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
        .expect("checkout");
    write(dir.path(), "app.ts", "base\nfrom the side\n");
    let side = commit(&repo, "side edits app", FIRST + 120);

    repo.set_head("refs/heads/main").expect("head");
    repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
        .expect("checkout");
    write(dir.path(), "app.ts", "base\nfrom the side\n");

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

    let traced = trace(
        dir.path(),
        &subject_for(dir.path(), at(merge), "app.ts"),
        None,
    );
    let found = subjects(&traced);

    assert!(found.contains(&"merge side"), "got {found:?}");
    assert!(found.contains(&"side edits app"), "got {found:?}");
    assert!(found.contains(&"base"), "got {found:?}");
    assert!(
        !found.contains(&"main work"),
        "a commit that did not touch the file is not in its history: {found:?}"
    );
}

// ── Nothing to show ──────────────────────────────────────────────────────────

#[test]
fn a_file_that_only_exists_in_the_working_tree_has_no_committed_history() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "tracked.ts", "tracked\n");
    commit(&repo, "first", FIRST);
    write(dir.path(), "brand-new.ts", "not committed\n");

    let subject = subject_for(dir.path(), DiffScope::WorkingTree, "brand-new.ts");
    let traced = trace(dir.path(), &subject, None);

    assert!(traced.commits.is_empty(), "it has never been committed");
    assert_eq!(
        traced.stopped,
        ScanStopped::No,
        "and that is the whole answer, not a bound that bit"
    );
    assert_eq!(traced.next, None);
}

#[test]
fn a_repository_with_no_commits_traces_nothing_and_does_not_fail() {
    let dir = TempDir::new().expect("tempdir");
    empty_repo(dir.path());
    write(dir.path(), "first.ts", "before any commit\n");

    let subject = subject_for(dir.path(), DiffScope::WorkingTree, "first.ts");
    let traced = trace(dir.path(), &subject, None);

    assert!(traced.commits.is_empty());
    assert_eq!(traced.scanned, 0);
    assert_eq!(traced.stopped, ScanStopped::No);
}

// ── Pagination and the bound ─────────────────────────────────────────────────

/// A repository where `app.ts` changes in every commit.
fn busy(dir: &Path, commits: usize) -> git2::Oid {
    let repo = empty_repo(dir);
    let mut last = None;
    for n in 0..commits {
        write(dir, "app.ts", &format!("version {n}\n"));
        last = Some(commit(&repo, &format!("edit {n}"), FIRST + (n as i64) * 60));
    }
    last.expect("at least one commit")
}

#[test]
fn a_page_is_never_larger_than_the_page_size() {
    let dir = TempDir::new().expect("tempdir");
    let head = busy(dir.path(), PAGE * 3);

    let traced = trace(
        dir.path(),
        &subject_for(dir.path(), at(head), "app.ts"),
        None,
    );

    assert_eq!(traced.commits.len(), PAGE);
    assert!(traced.next.is_some(), "there is more, so there is a cursor");
}

#[test]
fn the_next_page_continues_where_the_last_one_stopped() {
    let dir = TempDir::new().expect("tempdir");
    let head = busy(dir.path(), PAGE * 2);

    let first = trace(
        dir.path(),
        &subject_for(dir.path(), at(head), "app.ts"),
        None,
    );
    let cursor = first.next.clone().expect("a second page");
    let second = trace(dir.path(), &cursor.subject, Some(&cursor.from));

    assert_eq!(second.commits.len(), PAGE);
    assert_eq!(second.next, None, "fifty commits are exactly two pages");

    let overlap = first
        .commits
        .iter()
        .filter(|a| second.commits.iter().any(|b| b.commit.sha == a.commit.sha))
        .count();
    assert_eq!(overlap, 0, "no commit appears on two pages");
}

#[test]
fn the_pages_together_are_the_whole_history_in_order() {
    let dir = TempDir::new().expect("tempdir");
    let total = PAGE * 2 + 9;
    let head = busy(dir.path(), total);

    let mut seen: Vec<String> = Vec::new();
    let mut subject = subject_for(dir.path(), at(head), "app.ts");
    let mut from: Option<CommitId> = None;
    let mut pages = 0;

    loop {
        let traced = trace(dir.path(), &subject, from.as_ref());
        seen.extend(traced.commits.iter().map(|c| c.commit.subject.clone()));
        pages += 1;
        assert!(pages <= 10, "paging must terminate");

        match traced.next {
            Some(cursor) => {
                subject = cursor.subject;
                from = Some(cursor.from);
            }
            None => break,
        }
    }

    assert_eq!(pages, 3);
    assert_eq!(seen.len(), total);
    let expected: Vec<String> = (0..total).rev().map(|n| format!("edit {n}")).collect();
    assert_eq!(seen, expected);
}

#[test]
fn paging_survives_a_rename_that_falls_on_a_page_boundary() {
    // The hardest case the cursor has to carry: a page ends on the commit that
    // renamed the file, so the next page has to continue under a name that was
    // never sent to anybody.
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());

    let body = (0..40).map(|n| format!("line {n}\n")).collect::<String>();
    // Below the rename: enough commits to need a second page.
    write(dir.path(), "old.ts", &body);
    commit(&repo, "old 0", FIRST);
    for n in 1..PAGE {
        write(dir.path(), "old.ts", &format!("{body}v{n}\n"));
        commit(&repo, &format!("old {n}"), FIRST + (n as i64) * 60);
    }

    remove(dir.path(), "old.ts");
    write(dir.path(), "new.ts", &format!("{body}v{}\n", PAGE - 1));
    commit(&repo, "rename", FIRST + 10_000);
    write(dir.path(), "new.ts", &format!("{body}after\n"));
    let head = commit(&repo, "new 1", FIRST + 10_060);

    let first = trace(
        dir.path(),
        &subject_for(dir.path(), at(head), "new.ts"),
        None,
    );
    assert_eq!(first.commits.len(), PAGE);
    let cursor = first.next.clone().expect("a second page");

    let second = trace(dir.path(), &cursor.subject, Some(&cursor.from));

    assert!(
        !second.commits.is_empty(),
        "the trace continued under the old name"
    );
    assert!(
        second.commits.iter().all(|found| found.path == "old.ts"),
        "and every row below the rename is labelled with the old name"
    );

    let all: Vec<String> = first
        .commits
        .iter()
        .chain(second.commits.iter())
        .map(|found| found.commit.subject.clone())
        .collect();
    assert!(all.contains(&"rename".to_owned()));
    assert!(all.contains(&"old 0".to_owned()), "back to the beginning");
}

#[test]
fn a_scan_that_runs_out_of_budget_says_how_far_it_got() {
    // The bound made visible. A file touched once at the very bottom of a long
    // history: the scan runs out before finding it, and says so rather than
    // reporting that the file has no history.
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());

    write(dir.path(), "rare.ts", "written once\n");
    write(dir.path(), "noise.txt", "0\n");
    let root = commit(&repo, "the one commit that touches rare", FIRST);

    for n in 1..(MAX_SCAN + 40) {
        write(dir.path(), "noise.txt", &format!("{n}\n"));
        commit(&repo, &format!("noise {n}"), FIRST + (n as i64) * 60);
    }

    let subject = subject_for(dir.path(), at(root), "rare.ts");
    let traced = trace(dir.path(), &subject, None);

    assert!(
        traced.commits.is_empty(),
        "the one match is below the budget"
    );
    assert_eq!(
        traced.scanned as usize, MAX_SCAN,
        "and the budget was spent looking"
    );
    match traced.stopped {
        ScanStopped::Budget { scanned, limit } => {
            assert_eq!(scanned as usize, MAX_SCAN);
            assert_eq!(limit as usize, MAX_SCAN);
        }
        other => panic!("expected a budget stop, got {other:?}"),
    }
    assert!(
        traced.next.is_some(),
        "and the rest is reachable by continuing"
    );
}

#[test]
fn continuing_past_the_budget_eventually_finds_what_is_there() {
    // The other half of the bound: partial is not lost.
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());

    write(dir.path(), "rare.ts", "written once\n");
    write(dir.path(), "noise.txt", "0\n");
    let root = commit(&repo, "the one commit that touches rare", FIRST);

    for n in 1..(MAX_SCAN + 40) {
        write(dir.path(), "noise.txt", &format!("{n}\n"));
        commit(&repo, &format!("noise {n}"), FIRST + (n as i64) * 60);
    }

    let mut subject = subject_for(dir.path(), at(root), "rare.ts");
    let mut from: Option<CommitId> = None;
    let mut found: Vec<String> = Vec::new();
    let mut requests = 0;

    loop {
        let traced = trace(dir.path(), &subject, from.as_ref());
        found.extend(traced.commits.iter().map(|c| c.commit.subject.clone()));
        requests += 1;
        assert!(requests <= 5, "two budgets should be enough here");

        match traced.next {
            Some(cursor) => {
                subject = cursor.subject;
                from = Some(cursor.from);
            }
            None => break,
        }
    }

    assert_eq!(found, ["the one commit that touches rare"]);
    assert_eq!(requests, 2, "one budget short, one to finish");
}

// ── Unusual repositories ─────────────────────────────────────────────────────

#[test]
fn a_detached_head_traces_from_where_head_actually_is() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "app.ts", "one\n");
    commit(&repo, "one", FIRST);
    write(dir.path(), "app.ts", "two\n");
    let second = commit(&repo, "two", FIRST + 60);
    write(dir.path(), "app.ts", "three\n");
    commit(&repo, "three", FIRST + 120);

    repo.set_head_detached(second).expect("detach");

    let traced = trace(
        dir.path(),
        &subject_for(dir.path(), at(second), "app.ts"),
        None,
    );

    assert_eq!(
        subjects(&traced),
        ["two", "one"],
        "history is what HEAD can reach"
    );
}

#[test]
fn a_shallow_repository_says_so_and_traces_what_it_has() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "app.ts", "one\n");
    commit(&repo, "one", FIRST);
    write(dir.path(), "app.ts", "two\n");
    let second = commit(&repo, "two", FIRST + 60);
    write(dir.path(), "app.ts", "three\n");
    let third = commit(&repo, "three", FIRST + 120);

    fs::write(repo.path().join("shallow"), format!("{second}\n")).expect("write shallow");

    let traced = trace(
        dir.path(),
        &subject_for(dir.path(), at(third), "app.ts"),
        None,
    );

    assert!(traced.shallow, "the interface has to be able to say so");
    assert!(
        traced.commits.len() <= 2,
        "a shallow boundary ends the trace: {:?}",
        subjects(&traced)
    );
}

#[test]
fn a_plain_directory_has_no_file_history() {
    let dir = TempDir::new().expect("tempdir");
    let subject = FileSubject {
        scope: DiffScope::WorkingTree,
        at: 0,
        before: false,
    };

    assert_eq!(
        Libgit2.file_history(dir.path(), &subject, None),
        FileHistory::NotARepository
    );
}

#[test]
fn an_unreadable_repository_says_why() {
    let dir = TempDir::new().expect("tempdir");
    fs::create_dir(dir.path().join(".git")).expect("create .git");
    fs::write(dir.path().join(".git").join("HEAD"), "not a ref").expect("write");

    let subject = FileSubject {
        scope: DiffScope::WorkingTree,
        at: 0,
        before: false,
    };

    match Libgit2.file_history(dir.path(), &subject, None) {
        FileHistory::Unreadable { detail } => assert!(!detail.is_empty()),
        other => panic!("expected an unreadable repository, got {other:?}"),
    }
}

#[test]
fn a_subject_naming_a_change_that_is_not_there_is_a_stale_selection() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "app.ts", "one\n");
    let first = commit(&repo, "one", FIRST);

    let absent = FileSubject {
        scope: at(first),
        at: 99,
        before: false,
    };
    assert_eq!(
        Libgit2.file_history(dir.path(), &absent, None),
        FileHistory::Unknown
    );

    let no_commit = FileSubject {
        scope: DiffScope::Commit {
            commit: "0123456789abcdef0123456789abcdef01234567"
                .parse()
                .expect("id"),
        },
        at: 0,
        before: false,
    };
    assert_eq!(
        Libgit2.file_history(dir.path(), &no_commit, None),
        FileHistory::Unknown
    );
}

#[test]
fn a_cursor_naming_a_commit_that_is_not_here_is_a_stale_selection() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "app.ts", "one\n");
    let first = commit(&repo, "one", FIRST);

    let subject = subject_for(dir.path(), at(first), "app.ts");
    let gone: CommitId = "0123456789abcdef0123456789abcdef01234567"
        .parse()
        .expect("id");

    assert_eq!(
        Libgit2.file_history(dir.path(), &subject, Some(&gone)),
        FileHistory::Unknown
    );
}

// ── The boundary and the provider ────────────────────────────────────────────

#[test]
fn a_file_is_named_by_a_change_set_and_never_by_a_path() {
    // The contract this whole slice rests on: a subject is a scope, an ordinal
    // and a side. There is no field on it that holds a path, so a caller that
    // wanted to name one has nowhere to put it.
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "app.ts", "one\n");
    let first = commit(&repo, "one", FIRST);

    let subject = subject_for(dir.path(), at(first), "app.ts");
    let written = serde_json::to_string(&subject).expect("serialise");

    assert!(
        !written.contains("app.ts"),
        "no path on the wire: {written}"
    );
    assert!(written.contains("\"at\""));
    assert!(written.contains("\"scope\""));
}

#[test]
fn a_trace_is_read_through_the_trait_like_everything_else() {
    fn read(provider: &dyn GitProvider, root: &Path, subject: &FileSubject) -> FileHistory {
        provider.file_history(root, subject, None)
    }

    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "app.ts", "one\n");
    let first = commit(&repo, "one", FIRST);

    let subject = subject_for(dir.path(), at(first), "app.ts");
    assert!(matches!(
        read(&Libgit2, dir.path(), &subject),
        FileHistory::Ready { .. }
    ));
}

#[test]
fn tracing_a_file_changes_nothing_about_the_repository() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "app.ts", "one\n");
    commit(&repo, "one", FIRST);
    write(dir.path(), "app.ts", "two\n");
    let second = commit(&repo, "two", FIRST + 60);

    let references = |repo: &Repository| -> Vec<String> {
        let mut found: Vec<String> = repo
            .references()
            .expect("refs")
            .flatten()
            .filter_map(|reference| {
                let name = reference.name().ok()?.to_owned();
                let target = reference.target()?;
                Some(format!("{name}={target}"))
            })
            .collect();
        found.sort();
        found
    };

    let before = references(&repo);
    let head_before = repo.head().expect("head").target();
    let status_before = repo.statuses(None).expect("statuses").len();

    let subject = subject_for(dir.path(), at(second), "app.ts");
    let _ = Libgit2.file_history(dir.path(), &subject, None);
    let _ = Libgit2.file_history(dir.path(), &subject, None);

    assert_eq!(before, references(&repo), "no reference moved");
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
fn a_trace_never_examines_more_than_its_budget() {
    // The bound, asserted directly rather than inferred from a clock.
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "rare.ts", "once\n");
    write(dir.path(), "noise.txt", "0\n");
    let root = commit(&repo, "root", FIRST);
    for n in 1..(MAX_SCAN + 100) {
        write(dir.path(), "noise.txt", &format!("{n}\n"));
        commit(&repo, &format!("noise {n}"), FIRST + (n as i64) * 60);
    }

    let subject = subject_for(dir.path(), at(root), "rare.ts");
    let traced = trace(dir.path(), &subject, None);

    assert!(
        (traced.scanned as usize) <= MAX_SCAN,
        "examined {} commits, past the ceiling of {MAX_SCAN}",
        traced.scanned
    );
}
