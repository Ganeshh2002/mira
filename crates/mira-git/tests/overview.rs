//! Read-only Git context, against real repositories.
//!
//! Every test builds an actual repository on disk and reads it back through the
//! provider. There are no mocks here: the thing being tested is agreement with
//! Git, and a fake would only ever agree with itself.
//!
//! Covers `prd.md` features 3 (status), 4 (last commit) and 5 (branch).

use std::fs;
use std::path::Path;

use git2::{Repository, RepositoryInitOptions, Signature};
use mira_git::{GitOverview, GitProvider, Head, Libgit2};
use tempfile::TempDir;

/// A repository with `main` checked out and no commits yet.
fn empty_repo(dir: &Path) -> Repository {
    let mut options = RepositoryInitOptions::new();
    options.initial_head("main");
    Repository::init_opts(dir, &options).expect("init")
}

/// Commit every path currently in the worktree, returning the new commit id.
fn commit(repo: &Repository, subject: &str) -> git2::Oid {
    let mut index = repo.index().expect("index");
    index
        .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
        .expect("add");
    index.write().expect("write index");
    let tree = repo
        .find_tree(index.write_tree().expect("write tree"))
        .expect("tree");

    let who = Signature::now("Blacknit", "blacknit@example.com").expect("signature");
    let parents = match repo.head().ok().and_then(|h| h.target()) {
        Some(oid) => vec![repo.find_commit(oid).expect("parent")],
        None => Vec::new(),
    };
    let borrowed: Vec<&git2::Commit<'_>> = parents.iter().collect();

    repo.commit(Some("HEAD"), &who, &who, subject, &tree, &borrowed)
        .expect("commit")
}

fn write(dir: &Path, name: &str, body: &str) {
    fs::write(dir.join(name), body).expect("write");
}

/// The `Ready` variant, or a panic naming what came back instead.
fn ready(
    overview: GitOverview,
) -> (
    Head,
    bool,
    u32,
    Option<mira_git::Commit>,
    Option<mira_git::Upstream>,
) {
    match overview {
        GitOverview::Ready {
            head,
            clean,
            changed,
            last_commit,
            upstream,
        } => (head, clean, changed, last_commit, upstream),
        other => panic!("expected a readable repository, got {other:?}"),
    }
}

// ── Detection ────────────────────────────────────────────────────────────────

#[test]
fn a_plain_directory_is_not_a_repository() {
    let dir = TempDir::new().expect("tempdir");

    assert_eq!(
        Libgit2.overview(dir.path()),
        GitOverview::NotARepository,
        "a directory without Git is a neutral state, never an error (FR-3.6)"
    );
}

#[test]
fn a_subdirectory_of_a_repository_discovers_the_worktree_root() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "README.md", "hello");
    commit(&repo, "init");
    fs::create_dir_all(dir.path().join("src/deep")).expect("mkdir");

    let found = Libgit2
        .discover(&dir.path().join("src/deep"))
        .expect("the worktree root is found by walking up");

    assert_eq!(
        found.canonicalize().expect("canonical"),
        dir.path().canonicalize().expect("canonical"),
        "FR-1.3 walks up to the worktree root"
    );
}

#[test]
fn a_plain_directory_discovers_nothing() {
    let dir = TempDir::new().expect("tempdir");
    assert_eq!(Libgit2.discover(dir.path()), None);
}

#[test]
fn a_broken_git_directory_reads_as_unreadable_rather_than_crashing() {
    // A .git that exists but is not a repository is what a half-finished copy or a
    // truncated clone leaves behind. Mira reports it and stays up.
    let dir = TempDir::new().expect("tempdir");
    fs::create_dir(dir.path().join(".git")).expect("mkdir");
    write(&dir.path().join(".git"), "HEAD", "this is not a git head");

    match Libgit2.overview(dir.path()) {
        GitOverview::Unreadable { detail } => {
            assert!(!detail.is_empty(), "the reason is shown to a person");
        }
        other => panic!("expected Unreadable, got {other:?}"),
    }
}

// ── Head and last commit ─────────────────────────────────────────────────────

#[test]
fn a_repository_with_no_commits_has_an_unborn_head() {
    let dir = TempDir::new().expect("tempdir");
    empty_repo(dir.path());

    let (head, clean, _, last, _) = ready(Libgit2.overview(dir.path()));

    assert_eq!(head, Head::Unborn, "FR-4.3: no commits yet");
    assert_eq!(last, None);
    assert!(clean, "an empty repository is clean");
}

#[test]
fn the_current_branch_is_reported_by_name() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "README.md", "hello");
    commit(&repo, "init");

    let (head, _, _, _, _) = ready(Libgit2.overview(dir.path()));

    assert_eq!(
        head,
        Head::Branch {
            name: "main".to_owned()
        }
    );
}

#[test]
fn the_last_commit_carries_subject_author_and_time() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "README.md", "hello");
    let oid = commit(&repo, "feat: improve workspace restoration");

    let (_, _, _, last, _) = ready(Libgit2.overview(dir.path()));
    let last = last.expect("HEAD has a commit");

    assert_eq!(last.sha, oid.to_string());
    assert_eq!(last.short_sha, oid.to_string()[..7].to_owned());
    assert_eq!(last.subject, "feat: improve workspace restoration");
    assert_eq!(last.author, "Blacknit");
    assert!(last.committed_at > 0, "a real epoch second");
}

#[test]
fn only_the_first_line_of_a_commit_message_is_the_subject() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "README.md", "hello");
    commit(
        &repo,
        "fix: the thing\n\nA longer body that the header must not show.",
    );

    let (_, _, _, last, _) = ready(Libgit2.overview(dir.path()));

    assert_eq!(last.expect("commit").subject, "fix: the thing");
}

#[test]
fn a_commit_message_that_is_not_utf8_is_decoded_lossily_rather_than_panicking() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "README.md", "hello");

    let mut index = repo.index().expect("index");
    index
        .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
        .expect("add");
    index.write().expect("write index");
    let tree = repo
        .find_tree(index.write_tree().expect("tree"))
        .expect("tree");
    let who = Signature::now("Blacknit", "blacknit@example.com").expect("signature");
    repo.commit_create_buffer(&who, &who, "latin1: caf\u{e9}", &tree, &[])
        .expect("buffer");
    // Write the raw, invalid-UTF-8 message through the odb so libgit2 stores bytes.
    let raw = format!(
        "tree {}\nauthor {} <{}> 0 +0000\ncommitter {} <{}> 0 +0000\n\nlatin1: caf\u{fffd}\n",
        tree.id(),
        "Blacknit",
        "blacknit@example.com",
        "Blacknit",
        "blacknit@example.com",
    );
    let mut bytes = raw.into_bytes();
    let marker = bytes.len() - 2;
    bytes[marker] = 0xE9; // a lone Latin-1 é: not valid UTF-8
    let oid = repo
        .odb()
        .expect("odb")
        .write(git2::ObjectType::Commit, &bytes)
        .expect("write commit");
    repo.reference("refs/heads/main", oid, true, "test")
        .expect("point main at it");

    let (_, _, _, last, _) = ready(Libgit2.overview(dir.path()));

    assert!(
        last.expect("commit").subject.contains("latin1"),
        "a non-UTF-8 message is shown lossily, never a panic"
    );
}

#[test]
fn a_detached_head_reports_a_sha_instead_of_a_branch() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "README.md", "hello");
    let oid = commit(&repo, "init");
    repo.set_head_detached(oid).expect("detach");

    let (head, _, _, _, _) = ready(Libgit2.overview(dir.path()));

    assert_eq!(
        head,
        Head::Detached {
            sha: oid.to_string()[..7].to_owned()
        },
        "FR-4.4 shows the SHA and marks it detached"
    );
}

// ── Working tree ─────────────────────────────────────────────────────────────

#[test]
fn a_committed_worktree_is_clean() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "README.md", "hello");
    commit(&repo, "init");

    let (_, clean, changed, _, _) = ready(Libgit2.overview(dir.path()));

    assert!(clean);
    assert_eq!(changed, 0);
}

#[test]
fn a_modified_tracked_file_makes_the_worktree_dirty() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "README.md", "hello");
    commit(&repo, "init");
    write(dir.path(), "README.md", "hello, changed");

    let (_, clean, changed, _, _) = ready(Libgit2.overview(dir.path()));

    assert!(!clean);
    assert_eq!(changed, 1);
}

#[test]
fn an_untracked_file_makes_the_worktree_dirty() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "README.md", "hello");
    commit(&repo, "init");
    write(dir.path(), "scratch.txt", "notes");

    let (_, clean, changed, _, _) = ready(Libgit2.overview(dir.path()));

    assert!(!clean, "an untracked file is a change the user cares about");
    assert_eq!(changed, 1);
}

#[test]
fn an_ignored_file_leaves_the_worktree_clean() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), ".gitignore", "target/\n");
    commit(&repo, "init");
    fs::create_dir(dir.path().join("target")).expect("mkdir");
    write(&dir.path().join("target"), "huge.bin", "…");

    let (_, clean, changed, _, _) = ready(Libgit2.overview(dir.path()));

    assert!(clean, "FR-3.1 honours .gitignore");
    assert_eq!(changed, 0);
}

// ── Upstream ─────────────────────────────────────────────────────────────────

#[test]
fn a_branch_without_an_upstream_reports_no_distance() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "README.md", "hello");
    commit(&repo, "init");

    let (_, _, _, _, upstream) = ready(Libgit2.overview(dir.path()));

    assert_eq!(upstream, None, "AC-5.2: no chips and no error");
}

#[test]
fn ahead_and_behind_are_counted_against_the_tracked_upstream() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "README.md", "one");
    let base = commit(&repo, "one");
    write(dir.path(), "README.md", "two");
    commit(&repo, "two");
    write(dir.path(), "README.md", "three");
    commit(&repo, "three");

    // The upstream sits two commits back. No network is involved: this is exactly
    // the state a user is in between `git fetch` and `git push` (FR-5.2).
    repo.remote("origin", "https://example.invalid/repo.git")
        .expect("remote");
    repo.reference("refs/remotes/origin/main", base, true, "test")
        .expect("remote ref");
    repo.find_branch("main", git2::BranchType::Local)
        .expect("branch")
        .set_upstream(Some("origin/main"))
        .expect("track");

    let (_, _, _, _, upstream) = ready(Libgit2.overview(dir.path()));
    let upstream = upstream.expect("a tracked branch");

    assert_eq!(upstream.name, "origin/main");
    assert_eq!(upstream.ahead, 2);
    assert_eq!(upstream.behind, 0);
}

#[test]
fn behind_is_counted_when_the_upstream_has_moved_on() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "README.md", "one");
    commit(&repo, "one");
    write(dir.path(), "README.md", "two");
    let ahead = commit(&repo, "two");
    // Move the branch back, leaving the remote ref two ahead of it.
    let first = repo
        .find_commit(ahead)
        .expect("commit")
        .parent(0)
        .expect("parent")
        .id();
    repo.reference("refs/heads/main", first, true, "rewind")
        .expect("rewind");
    repo.remote("origin", "https://example.invalid/repo.git")
        .expect("remote");
    repo.reference("refs/remotes/origin/main", ahead, true, "test")
        .expect("remote ref");
    repo.find_branch("main", git2::BranchType::Local)
        .expect("branch")
        .set_upstream(Some("origin/main"))
        .expect("track");

    let (_, _, _, _, upstream) = ready(Libgit2.overview(dir.path()));
    let upstream = upstream.expect("a tracked branch");

    assert_eq!(upstream.ahead, 0);
    assert_eq!(upstream.behind, 1);
}

#[test]
fn reading_a_repository_makes_no_network_request() {
    // AC-5.3. The remote is configured with an unroutable URL; if any code path
    // fetched, this test would hang or fail rather than pass instantly.
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "README.md", "hello");
    let oid = commit(&repo, "init");
    repo.remote("origin", "https://127.0.0.1:1/repo.git")
        .expect("remote");
    repo.reference("refs/remotes/origin/main", oid, true, "test")
        .expect("remote ref");
    repo.find_branch("main", git2::BranchType::Local)
        .expect("branch")
        .set_upstream(Some("origin/main"))
        .expect("track");

    let (_, _, _, _, upstream) = ready(Libgit2.overview(dir.path()));

    assert_eq!(upstream.expect("tracked").ahead, 0);
}

#[test]
fn a_repository_nested_inside_another_discovers_its_own_root() {
    // A repository checked out inside a monorepo — a vendored dependency, a
    // half-migrated submodule — belongs to itself. Walking past it to the outer
    // repository would attach a project to a repository it is not in, and would
    // make the outer workspace's configuration appear to describe it.
    let outer = TempDir::new().expect("tempdir");
    let outer_repo = empty_repo(outer.path());
    write(outer.path(), "README.md", "outer");
    commit(&outer_repo, "outer");

    let inner = outer.path().join("vendor/inner");
    fs::create_dir_all(&inner).expect("mkdir");
    let inner_repo = empty_repo(&inner);
    write(&inner, "README.md", "inner");
    commit(&inner_repo, "inner");

    let found = Libgit2.discover(&inner).expect("a worktree root");

    assert_eq!(
        found.canonicalize().expect("canonical"),
        inner.canonicalize().expect("canonical"),
        "the nearest .git wins, which is what git itself does"
    );
    assert_ne!(
        found.canonicalize().expect("canonical"),
        outer.path().canonicalize().expect("canonical")
    );
}

#[test]
fn a_nested_repository_reports_its_own_head_not_the_outer_one() {
    let outer = TempDir::new().expect("tempdir");
    let outer_repo = empty_repo(outer.path());
    write(outer.path(), "README.md", "outer");
    commit(&outer_repo, "outer only");

    let inner = outer.path().join("vendor/inner");
    fs::create_dir_all(&inner).expect("mkdir");
    let inner_repo = empty_repo(&inner);
    write(&inner, "README.md", "inner");
    commit(&inner_repo, "inner only");

    let (_, _, _, last, _) = ready(Libgit2.overview(&inner));

    assert_eq!(last.expect("a commit").subject, "inner only");
}
