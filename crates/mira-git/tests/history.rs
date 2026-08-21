//! Reading a repository's past, against real repositories.
//!
//! Every test builds an actual repository on disk and reads it back through the
//! provider, for the reason `overview.rs` gives: the thing being tested is
//! agreement with Git, and a fake would only ever agree with itself.
//!
//! Covers `roadmap.md` slice 5a — commit walking with pagination, commit detail,
//! and the states that are not the happy path: an empty repository, an unborn
//! branch, a detached HEAD, a shallow clone, and a history libgit2 cannot follow.

use std::fs;
use std::path::Path;

use git2::{Repository, RepositoryInitOptions, Signature};
use mira_git::{Commit, CommitId, CommitLookup, CommitPage, GitProvider, Head, Libgit2, PAGE};
use tempfile::TempDir;

// ── Fixtures ─────────────────────────────────────────────────────────────────

/// A repository with `main` checked out and no commits yet.
fn empty_repo(dir: &Path) -> Repository {
    let mut options = RepositoryInitOptions::new();
    options.initial_head("main");
    Repository::init_opts(dir, &options).expect("init")
}

/// Commit every path currently in the worktree, at a fixed time.
///
/// The time advances by a minute per commit so that ordering is deterministic:
/// a walk over commits sharing a second would be free to return either.
fn commit_at(repo: &Repository, subject: &str, when: i64) -> git2::Oid {
    let mut index = repo.index().expect("index");
    index
        .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
        .expect("add");
    index.write().expect("write index");
    let tree = repo
        .find_tree(index.write_tree().expect("write tree"))
        .expect("tree");

    let who = Signature::new(
        "Blacknit",
        "blacknit@example.com",
        &git2::Time::new(when, 0),
    )
    .expect("signature");

    let parents = match repo.head().ok().and_then(|h| h.target()) {
        Some(oid) => vec![repo.find_commit(oid).expect("parent")],
        None => Vec::new(),
    };
    let borrowed: Vec<&git2::Commit<'_>> = parents.iter().collect();

    repo.commit(Some("HEAD"), &who, &who, subject, &tree, &borrowed)
        .expect("commit")
}

/// The first commit's time. Any fixed epoch would do; this one is readable.
const FIRST: i64 = 1_700_000_000;

/// A repository with `count` commits, oldest first, one file each.
fn repo_with(dir: &Path, count: usize) -> Repository {
    let repo = empty_repo(dir);
    for n in 0..count {
        fs::write(dir.join(format!("file-{n}.txt")), format!("body {n}")).expect("write");
        commit_at(&repo, &format!("commit {n}"), FIRST + (n as i64) * 60);
    }
    repo
}

/// The `Ready` variant of a page, or a panic naming what came back instead.
fn ready(page: CommitPage) -> (Head, Vec<Commit>, Option<CommitId>, bool) {
    match page {
        CommitPage::Ready {
            head,
            commits,
            next,
            shallow,
        } => (head, commits, next, shallow),
        other => panic!("expected a readable page, got {other:?}"),
    }
}

fn id(raw: &str) -> CommitId {
    raw.parse().expect("a commit id")
}

// ── Ordering ─────────────────────────────────────────────────────────────────

#[test]
fn history_is_newest_first() {
    let dir = TempDir::new().expect("tempdir");
    repo_with(dir.path(), 5);

    let (_, commits, _, _) = ready(Libgit2.history(dir.path(), None));

    let subjects: Vec<&str> = commits.iter().map(|c| c.subject.as_str()).collect();
    assert_eq!(
        subjects,
        ["commit 4", "commit 3", "commit 2", "commit 1", "commit 0"],
        "the most recent commit is the one a person is looking for"
    );
}

#[test]
fn every_commit_carries_what_a_row_shows() {
    let dir = TempDir::new().expect("tempdir");
    repo_with(dir.path(), 1);

    let (_, commits, _, _) = ready(Libgit2.history(dir.path(), None));
    let only = commits.first().expect("one commit");

    assert_eq!(only.subject, "commit 0");
    assert_eq!(only.author, "Blacknit");
    assert_eq!(only.committed_at, FIRST);
    assert_eq!(only.sha.len(), 40);
    assert_eq!(only.short_sha.len(), 7);
    assert!(
        only.sha.starts_with(&only.short_sha),
        "the abbreviation is a prefix of the id it abbreviates"
    );
}

#[test]
fn the_branch_the_history_belongs_to_is_reported() {
    let dir = TempDir::new().expect("tempdir");
    repo_with(dir.path(), 1);

    let (head, _, _, _) = ready(Libgit2.history(dir.path(), None));

    assert_eq!(
        head,
        Head::Branch {
            name: "main".to_owned()
        }
    );
}

// ── Pagination ───────────────────────────────────────────────────────────────

#[test]
fn a_page_is_never_larger_than_the_page_size() {
    let dir = TempDir::new().expect("tempdir");
    repo_with(dir.path(), PAGE * 3);

    let (_, commits, next, _) = ready(Libgit2.history(dir.path(), None));

    assert_eq!(commits.len(), PAGE);
    assert!(next.is_some(), "there is more, so there is a cursor");
}

#[test]
fn the_next_page_continues_where_the_last_one_stopped() {
    let dir = TempDir::new().expect("tempdir");
    repo_with(dir.path(), PAGE * 2);

    let (_, first, next, _) = ready(Libgit2.history(dir.path(), None));
    let cursor = next.expect("a second page exists");
    let (_, second, after, _) = ready(Libgit2.history(dir.path(), Some(&cursor)));

    assert_eq!(second.len(), PAGE);
    assert_eq!(
        second.first().map(|c| c.sha.as_str()),
        Some(cursor.as_str()),
        "the cursor names the first commit of the page it opens"
    );
    assert_eq!(after, None, "fifty commits are exactly two pages");

    let overlap = first
        .iter()
        .filter(|a| second.iter().any(|b| b.sha == a.sha))
        .count();
    assert_eq!(overlap, 0, "no commit appears on two pages");
}

#[test]
fn the_pages_together_are_the_whole_history_in_order() {
    let dir = TempDir::new().expect("tempdir");
    let total = PAGE * 2 + 7;
    repo_with(dir.path(), total);

    let mut seen: Vec<String> = Vec::new();
    let mut cursor: Option<CommitId> = None;
    let mut pages = 0;

    loop {
        let (_, commits, next, _) = ready(Libgit2.history(dir.path(), cursor.as_ref()));
        seen.extend(commits.into_iter().map(|c| c.subject));
        pages += 1;
        assert!(pages <= 10, "paging must terminate");
        match next {
            Some(id) => cursor = Some(id),
            None => break,
        }
    }

    assert_eq!(pages, 3);
    assert_eq!(seen.len(), total);
    let expected: Vec<String> = (0..total).rev().map(|n| format!("commit {n}")).collect();
    assert_eq!(seen, expected);
}

#[test]
fn a_history_shorter_than_a_page_has_no_next_cursor() {
    let dir = TempDir::new().expect("tempdir");
    repo_with(dir.path(), 3);

    let (_, commits, next, _) = ready(Libgit2.history(dir.path(), None));

    assert_eq!(commits.len(), 3);
    assert_eq!(next, None, "there is no second page to offer");
}

#[test]
fn a_history_of_exactly_one_page_has_no_next_cursor() {
    // The off-by-one that matters: the walk reads PAGE + 1 to learn whether there
    // is more, and a repository of exactly PAGE commits must not claim there is.
    let dir = TempDir::new().expect("tempdir");
    repo_with(dir.path(), PAGE);

    let (_, commits, next, _) = ready(Libgit2.history(dir.path(), None));

    assert_eq!(commits.len(), PAGE);
    assert_eq!(next, None);
}

#[test]
fn a_cursor_naming_a_commit_that_is_not_here_ends_the_history() {
    let dir = TempDir::new().expect("tempdir");
    repo_with(dir.path(), 3);

    // A commit id that is well-formed and simply absent — what a cursor becomes
    // after the branch it came from is rebased away.
    let (_, commits, next, _) = ready(Libgit2.history(
        dir.path(),
        Some(&id("0123456789abcdef0123456789abcdef01234567")),
    ));

    assert!(commits.is_empty());
    assert_eq!(next, None);
}

#[test]
fn an_abbreviated_cursor_resolves_like_a_full_one() {
    let dir = TempDir::new().expect("tempdir");
    repo_with(dir.path(), 4);

    let (_, commits, _, _) = ready(Libgit2.history(dir.path(), None));
    let third = commits.get(2).expect("a third commit");

    let (_, from_short, _, _) = ready(Libgit2.history(dir.path(), Some(&id(&third.short_sha))));
    let (_, from_full, _, _) = ready(Libgit2.history(dir.path(), Some(&id(&third.sha))));

    assert_eq!(from_short, from_full);
    assert_eq!(from_short.len(), 2, "the third commit and the one below it");
}

// ── The states that are not the happy path ───────────────────────────────────

#[test]
fn a_plain_directory_has_no_history() {
    let dir = TempDir::new().expect("tempdir");

    assert_eq!(
        Libgit2.history(dir.path(), None),
        CommitPage::NotARepository,
        "a directory without Git is a neutral state, never an error"
    );
}

#[test]
fn an_empty_repository_has_an_empty_history_rather_than_a_failure() {
    let dir = TempDir::new().expect("tempdir");
    empty_repo(dir.path());

    let (head, commits, next, _) = ready(Libgit2.history(dir.path(), None));

    assert_eq!(head, Head::Unborn, "the branch exists but has no commits");
    assert!(commits.is_empty());
    assert_eq!(next, None);
}

#[test]
fn an_unborn_head_is_a_state_and_not_an_error() {
    // A repository initialised and then given a file, but never committed.
    let dir = TempDir::new().expect("tempdir");
    empty_repo(dir.path());
    fs::write(dir.path().join("README.md"), "hello").expect("write");

    let (head, commits, _, _) = ready(Libgit2.history(dir.path(), None));

    assert_eq!(head, Head::Unborn);
    assert!(commits.is_empty());
}

#[test]
fn a_detached_head_still_has_a_history() {
    let dir = TempDir::new().expect("tempdir");
    let repo = repo_with(dir.path(), 4);

    let second_newest = {
        let (_, commits, _, _) = ready(Libgit2.history(dir.path(), None));
        commits.get(1).expect("a second commit").sha.clone()
    };
    repo.set_head_detached(second_newest.parse().expect("oid"))
        .expect("detach");

    let (head, commits, _, _) = ready(Libgit2.history(dir.path(), None));

    match head {
        Head::Detached { sha } => assert!(second_newest.starts_with(&sha)),
        other => panic!("expected a detached head, got {other:?}"),
    }
    assert_eq!(
        commits.len(),
        3,
        "history is what is reachable from where HEAD actually is"
    );
    assert_eq!(commits.first().map(|c| c.sha.clone()), Some(second_newest));
}

#[test]
fn a_shallow_repository_says_so() {
    let dir = TempDir::new().expect("tempdir");
    let repo = repo_with(dir.path(), 4);

    // A shallow clone is a `.git/shallow` file naming the grafted boundary. It is
    // written here rather than fetched, because Mira never opens a network
    // transport and neither may its tests.
    let (_, commits, _, _) = ready(Libgit2.history(dir.path(), None));
    let boundary = commits.get(1).expect("a boundary commit").sha.clone();
    fs::write(repo.path().join("shallow"), format!("{boundary}\n")).expect("write shallow");

    let (_, commits, _, shallow) = ready(Libgit2.history(dir.path(), None));

    assert!(
        shallow,
        "the interface has to be able to say the copy is partial"
    );
    assert!(
        commits.len() <= 2,
        "a shallow boundary ends the walk: {} commits came back",
        commits.len()
    );
}

#[test]
fn a_history_libgit2_cannot_follow_returns_what_it_could_read() {
    let dir = TempDir::new().expect("tempdir");
    let repo = repo_with(dir.path(), 6);

    // Remove the object of the third-newest commit. Everything below it becomes
    // unreachable — the shape of a repository truncated by a failed transfer or a
    // half-finished `gc`.
    let (_, commits, _, _) = ready(Libgit2.history(dir.path(), None));
    let broken = commits.get(2).expect("a third commit").sha.clone();
    let (prefix, rest) = broken.split_at(2);
    let object = repo.path().join("objects").join(prefix).join(rest);
    if object.exists() {
        fs::remove_file(&object).expect("remove object");
    } else {
        // A packed repository keeps no loose object to remove; the test would be
        // asserting nothing, so it says so rather than passing quietly.
        panic!("expected a loose object at {}", object.display());
    }

    let page = Libgit2.history(dir.path(), None);

    match page {
        CommitPage::Ready { commits, .. } => assert!(
            commits.len() < 6,
            "a broken history is truncated, never invented"
        ),
        // libgit2 may refuse the walk outright, which is equally acceptable: the
        // requirement is a graceful state, not a particular one.
        CommitPage::Unreadable { detail } => assert!(!detail.is_empty()),
        other => panic!("expected a page or a readable failure, got {other:?}"),
    }
}

#[test]
fn an_unreadable_repository_says_why_rather_than_denying_it_is_one() {
    let dir = TempDir::new().expect("tempdir");
    fs::create_dir(dir.path().join(".git")).expect("create .git");
    fs::write(dir.path().join(".git").join("HEAD"), "not a ref").expect("write");

    match Libgit2.history(dir.path(), None) {
        CommitPage::Unreadable { detail } => assert!(!detail.is_empty()),
        other => panic!("expected an unreadable repository, got {other:?}"),
    }
}

// ── Commit detail ────────────────────────────────────────────────────────────

#[test]
fn a_commit_reports_what_the_detail_surface_shows() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    fs::write(dir.path().join("one.txt"), "one").expect("write");
    commit_at(&repo, "first", FIRST);
    fs::write(dir.path().join("two.txt"), "two").expect("write");
    fs::write(dir.path().join("three.txt"), "three").expect("write");
    let newest = commit_at(&repo, "second\n\nA body, on its own lines.\n", FIRST + 60);

    let lookup = Libgit2.commit(dir.path(), &id(&newest.to_string()));

    match lookup {
        CommitLookup::Ready { commit } => {
            assert_eq!(commit.commit.subject, "second");
            assert_eq!(commit.body.as_deref(), Some("A body, on its own lines."));
            assert_eq!(commit.commit.author, "Blacknit");
            assert_eq!(commit.author_email, "blacknit@example.com");
            assert_eq!(commit.commit.sha, newest.to_string());
            assert_eq!(commit.commit.committed_at, FIRST + 60);
            assert_eq!(commit.parents, 1);
            assert_eq!(commit.changed_files, Some(2));
        }
        other => panic!("expected a commit, got {other:?}"),
    }
}

#[test]
fn a_commit_with_only_a_subject_has_no_body() {
    let dir = TempDir::new().expect("tempdir");
    let repo = repo_with(dir.path(), 1);
    let only = repo.head().expect("head").target().expect("target");

    match Libgit2.commit(dir.path(), &id(&only.to_string())) {
        CommitLookup::Ready { commit } => {
            assert_eq!(commit.body, None);
            assert_eq!(commit.parents, 0, "a root commit has no parent");
            assert_eq!(commit.changed_files, Some(1), "everything in it is new");
        }
        other => panic!("expected a commit, got {other:?}"),
    }
}

#[test]
fn a_merge_reports_its_parents_and_declines_to_invent_a_file_count() {
    let dir = TempDir::new().expect("tempdir");
    let repo = repo_with(dir.path(), 1);

    let base = repo.head().expect("head").target().expect("target");
    fs::write(dir.path().join("left.txt"), "left").expect("write");
    let left = commit_at(&repo, "left", FIRST + 60);

    repo.set_head_detached(base).expect("detach");
    repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
        .expect("checkout");
    fs::write(dir.path().join("right.txt"), "right").expect("write");
    let right = commit_at(&repo, "right", FIRST + 120);

    let who = Signature::new(
        "Blacknit",
        "blacknit@example.com",
        &git2::Time::new(FIRST + 180, 0),
    )
    .expect("signature");
    let tree = repo
        .find_tree(
            repo.index()
                .expect("index")
                .write_tree()
                .expect("write tree"),
        )
        .expect("tree");
    let merged = repo
        .commit(
            Some("HEAD"),
            &who,
            &who,
            "merge left into right",
            &tree,
            &[
                &repo.find_commit(right).expect("right"),
                &repo.find_commit(left).expect("left"),
            ],
        )
        .expect("merge commit");

    match Libgit2.commit(dir.path(), &id(&merged.to_string())) {
        CommitLookup::Ready { commit } => {
            assert_eq!(commit.parents, 2);
            assert_eq!(
                commit.changed_files, None,
                "a merge's file count depends on which parent you compare against, \
                 so there is no single true number to show"
            );
        }
        other => panic!("expected a commit, got {other:?}"),
    }

    // And the merge is a row in the list like any other — no lanes in 5a.
    let (_, commits, _, _) = ready(Libgit2.history(dir.path(), None));
    assert_eq!(
        commits.first().map(|c| c.subject.as_str()),
        Some("merge left into right")
    );
}

#[test]
fn a_commit_that_is_not_here_is_unknown_rather_than_a_failure() {
    let dir = TempDir::new().expect("tempdir");
    repo_with(dir.path(), 2);

    assert_eq!(
        Libgit2.commit(dir.path(), &id("0123456789abcdef0123456789abcdef01234567")),
        CommitLookup::Unknown
    );
}

#[test]
fn a_commit_asked_for_outside_a_repository_says_there_is_no_repository() {
    let dir = TempDir::new().expect("tempdir");

    assert_eq!(
        Libgit2.commit(dir.path(), &id("ad50fc7")),
        CommitLookup::NotARepository
    );
}

#[test]
fn an_id_that_names_something_other_than_a_commit_is_unknown() {
    // A tree's object id is a well-formed hexadecimal id that is not a commit.
    // Resolving it must not produce a commit belonging to somebody else.
    let dir = TempDir::new().expect("tempdir");
    let repo = repo_with(dir.path(), 1);
    let tree = repo
        .find_commit(repo.head().expect("head").target().expect("target"))
        .expect("commit")
        .tree()
        .expect("tree")
        .id();

    assert_eq!(
        Libgit2.commit(dir.path(), &id(&tree.to_string())),
        CommitLookup::Unknown
    );
}

// ── The provider boundary ────────────────────────────────────────────────────

#[test]
fn history_is_read_through_the_trait_like_everything_else() {
    // The point of the trait is that a caller never names an implementation.
    // This function takes any provider; that it compiles is the assertion.
    fn read(provider: &dyn GitProvider, root: &Path) -> CommitPage {
        provider.history(root, None)
    }

    let dir = TempDir::new().expect("tempdir");
    repo_with(dir.path(), 2);

    let (_, commits, _, _) = ready(read(&Libgit2, dir.path()));
    assert_eq!(commits.len(), 2);
}

#[test]
fn history_belongs_to_the_repository_and_not_to_the_directory_asked_about() {
    // A package inside a repository shows the repository's history. There is no
    // second, per-package walk, and nothing is filtered by path — that is what
    // makes a monorepo's packages share one history rather than each holding a
    // copy of it.
    let dir = TempDir::new().expect("tempdir");
    let package = dir.path().join("apps").join("web");
    fs::create_dir_all(&package).expect("create package");
    repo_with(dir.path(), 3);

    let (_, from_root, _, _) = ready(Libgit2.history(dir.path(), None));
    let (_, from_package, _, _) = ready(Libgit2.history(&package, None));

    assert_eq!(from_root, from_package);
}
