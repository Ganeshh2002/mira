//! The graph, against real repositories.
//!
//! `lanes.rs` proves the layout algorithm from tables of ids. This proves the
//! other half: that what libgit2 hands back — parents, refs, HEAD, a shallow
//! boundary, a history it cannot follow — arrives in the shape the layout expects
//! and comes out as a picture.
//!
//! Every fixture is an actual repository on disk, for the reason `overview.rs`
//! gives: the thing being tested is agreement with Git, and a fake would only
//! ever agree with itself.

use std::fs;
use std::path::Path;

use git2::{Repository, RepositoryInitOptions, Signature};
use mira_git::{
    CommitGraph, CommitId, EdgeKind, GitProvider, GraphRow, Head, Libgit2, RefKind, RowKind, PAGE,
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

/// Commit the worktree onto HEAD, at a fixed time.
fn commit_at(repo: &Repository, dir: &Path, subject: &str, when: i64) -> git2::Oid {
    fs::write(dir.join(format!("{subject}.txt")), subject).expect("write");
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

/// A repository with `count` commits on `main`, oldest first.
fn linear(dir: &Path, count: usize) -> Repository {
    let repo = empty_repo(dir);
    for n in 0..count {
        commit_at(&repo, dir, &format!("c{n}"), FIRST + (n as i64) * 60);
    }
    repo
}

/// The `Ready` variant, or a panic naming what came back instead.
struct Page {
    head: Head,
    rows: Vec<GraphRow>,
    next: Option<CommitId>,
    shallow: bool,
    lanes: u32,
    collapsed: bool,
    refs_truncated: bool,
}

fn ready(graph: CommitGraph) -> Page {
    match graph {
        CommitGraph::Ready {
            head,
            rows,
            next,
            shallow,
            lanes,
            collapsed,
            refs_truncated,
        } => Page {
            head,
            rows,
            next,
            shallow,
            lanes,
            collapsed,
            refs_truncated,
        },
        other => panic!("expected a readable graph, got {other:?}"),
    }
}

fn read(dir: &Path) -> Page {
    ready(Libgit2.graph(dir, None))
}

fn subjects(page: &Page) -> Vec<&str> {
    page.rows
        .iter()
        .map(|row| row.commit.subject.as_str())
        .collect()
}

fn row<'a>(page: &'a Page, subject: &str) -> &'a GraphRow {
    page.rows
        .iter()
        .find(|row| row.commit.subject == subject)
        .unwrap_or_else(|| panic!("no row for {subject}"))
}

/// Every reference and what it points at, as a comparable snapshot.
fn references(repo: &Repository) -> Vec<String> {
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
}

// ── Linear history ───────────────────────────────────────────────────────────

#[test]
fn a_linear_history_is_one_lane_of_ordinary_commits() {
    let dir = TempDir::new().expect("tempdir");
    linear(dir.path(), 4);

    let page = read(dir.path());

    assert_eq!(subjects(&page), ["c3", "c2", "c1", "c0"]);
    assert!(page.rows.iter().all(|row| row.lane == 0));
    assert_eq!(page.lanes, 1);
    assert!(!page.collapsed);
    assert_eq!(row(&page, "c2").kind, RowKind::Normal);
}

#[test]
fn the_graph_is_the_same_page_the_history_list_shows() {
    // The graph sits *beside* the history, so the rows have to be the same rows
    // in the same order — otherwise the gutter would label the wrong commits.
    let dir = TempDir::new().expect("tempdir");
    linear(dir.path(), 6);

    let page = read(dir.path());
    let list = match Libgit2.history(dir.path(), None) {
        mira_git::CommitPage::Ready { commits, .. } => commits,
        other => panic!("expected a page, got {other:?}"),
    };

    assert_eq!(page.rows.len(), list.len());
    for (graph, history) in page.rows.iter().zip(list.iter()) {
        assert_eq!(&graph.commit, history);
    }
}

#[test]
fn every_row_carries_the_parents_git_recorded() {
    let dir = TempDir::new().expect("tempdir");
    linear(dir.path(), 3);

    let page = read(dir.path());

    assert_eq!(row(&page, "c2").parents.len(), 1);
    assert_eq!(
        row(&page, "c2").parents[0].as_str(),
        row(&page, "c1").commit.sha,
        "the parent named is the row below"
    );
}

// ── Root commits ─────────────────────────────────────────────────────────────

#[test]
fn a_root_commit_says_it_is_one_and_ends_its_lane() {
    let dir = TempDir::new().expect("tempdir");
    linear(dir.path(), 3);

    let page = read(dir.path());
    let root = row(&page, "c0");

    assert_eq!(root.kind, RowKind::Root);
    assert!(root.parents.is_empty());
    assert!(root.edges.is_empty(), "nothing leaves the first commit");
    assert!(root.continuing.is_empty(), "and no line runs past it");
}

#[test]
fn a_repository_with_no_commits_draws_nothing_and_does_not_fail() {
    let dir = TempDir::new().expect("tempdir");
    empty_repo(dir.path());

    let page = read(dir.path());

    assert_eq!(page.head, Head::Unborn);
    assert!(page.rows.is_empty());
    assert_eq!(page.lanes, 0);
    assert_eq!(page.next, None);
}

// ── Branches and merges ──────────────────────────────────────────────────────

/// A repository shaped like a feature branch merged back into `main`.
///
/// ```text
/// merge      ← two parents
/// ├─ side
/// └─ main2
///    base
/// ```
fn merged(dir: &Path) -> Repository {
    let repo = empty_repo(dir);
    let base = commit_at(&repo, dir, "base", FIRST);

    repo.branch("side", &repo.find_commit(base).expect("base"), false)
        .expect("branch");

    let main2 = commit_at(&repo, dir, "main2", FIRST + 60);

    repo.set_head("refs/heads/side").expect("head");
    repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
        .expect("checkout");
    let side = commit_at(&repo, dir, "side-work", FIRST + 120);

    repo.set_head("refs/heads/main").expect("head");
    repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
        .expect("checkout");

    // Scoped, so every borrow of `repo` has ended before it is handed back.
    {
        let tree = repo
            .find_tree(repo.index().expect("index").write_tree().expect("tree"))
            .expect("tree");
        let author = who(FIRST + 180);
        repo.commit(
            Some("HEAD"),
            &author,
            &author,
            "merge",
            &tree,
            &[
                &repo.find_commit(main2).expect("main2"),
                &repo.find_commit(side).expect("side"),
            ],
        )
        .expect("merge commit");
    }

    repo
}

#[test]
fn a_merge_says_it_is_a_merge_and_names_both_parents() {
    let dir = TempDir::new().expect("tempdir");
    merged(dir.path());

    let page = read(dir.path());
    let merge = row(&page, "merge");

    assert_eq!(merge.kind, RowKind::Merge);
    assert_eq!(merge.parents.len(), 2);
    assert_eq!(merge.edges.len(), 2, "one line per parent");
    assert_eq!(merge.edges[0].kind, EdgeKind::Straight);
    assert_eq!(merge.edges[1].kind, EdgeKind::Merge);
    assert_ne!(
        merge.edges[0].to, merge.edges[1].to,
        "the two sides are drawn apart"
    );
}

#[test]
fn a_merged_branch_gets_a_lane_and_gives_it_back() {
    let dir = TempDir::new().expect("tempdir");
    merged(dir.path());

    let page = read(dir.path());
    let base = row(&page, "base");

    assert_eq!(page.lanes, 2, "a mainline and one side, and no more");
    assert_eq!(row(&page, "merge").lane, 0, "the merge is on the mainline");
    assert!(
        base.continuing.is_empty(),
        "and the picture closes at the base"
    );

    // Both sides converge on the base, and the assertion is about *that* rather
    // than about which lane the base lands in.
    //
    // Which lane it lands in is decided by whichever side of the merge the window
    // reached first, and in date order that is the side branch — it was committed
    // later than the mainline commit beside it. Topological order would put the
    // mainline first and keep the base at lane 0. That is the cost of the bound,
    // it is visible right here, and it is worth one line of picture rather than a
    // traversal of the whole repository (ADR-0015).
    let converging: Vec<u32> = ["main2", "side-work"]
        .iter()
        .map(|subject| row(&page, subject).edges[0].to)
        .collect();

    assert_eq!(
        converging,
        [base.lane, base.lane],
        "both lines lead to the row the base is actually on"
    );
    assert_eq!(row(&page, "main2").edges[0].kind, EdgeKind::Join);
    assert_eq!(row(&page, "side-work").edges[0].kind, EdgeKind::Straight);
}

#[test]
fn the_two_sides_of_a_merge_sit_in_different_lanes() {
    let dir = TempDir::new().expect("tempdir");
    merged(dir.path());

    let page = read(dir.path());

    assert_ne!(row(&page, "main2").lane, row(&page, "side-work").lane);
}

// ── References ───────────────────────────────────────────────────────────────

#[test]
fn a_branch_labels_the_commit_it_points_at() {
    let dir = TempDir::new().expect("tempdir");
    merged(dir.path());

    let page = read(dir.path());
    let names: Vec<&str> = row(&page, "side-work")
        .refs
        .iter()
        .map(|found| found.name.as_str())
        .collect();

    assert!(names.contains(&"side"), "got {names:?}");
    assert_eq!(
        row(&page, "side-work").refs[0].kind,
        RefKind::Branch,
        "and says what kind of reference it is"
    );
}

#[test]
fn a_tag_labels_its_commit_even_when_it_is_annotated() {
    // An annotated tag points at a tag object, not at a commit. Labelling the tag
    // object would put the label on a row that is in no history.
    let dir = TempDir::new().expect("tempdir");
    let repo = linear(dir.path(), 3);

    let target = repo
        .find_commit(repo.head().expect("head").target().expect("target"))
        .expect("commit");
    repo.tag(
        "v1.0",
        target.as_object(),
        &who(FIRST + 600),
        "the first release",
        false,
    )
    .expect("annotated tag");
    repo.tag_lightweight("nightly", target.as_object(), false)
        .expect("lightweight tag");

    let page = read(dir.path());
    let tags: Vec<&str> = row(&page, "c2")
        .refs
        .iter()
        .filter(|found| found.kind == RefKind::Tag)
        .map(|found| found.name.as_str())
        .collect();

    assert!(tags.contains(&"v1.0"), "got {tags:?}");
    assert!(tags.contains(&"nightly"), "got {tags:?}");
}

#[test]
fn head_labels_the_row_it_is_on() {
    let dir = TempDir::new().expect("tempdir");
    linear(dir.path(), 3);

    let page = read(dir.path());

    assert_eq!(
        row(&page, "c2").refs.first().map(|found| found.kind),
        Some(RefKind::Head),
        "where you are is read first"
    );
}

#[test]
fn labels_come_back_in_the_same_order_every_time() {
    let dir = TempDir::new().expect("tempdir");
    linear(dir.path(), 2);

    let first = read(dir.path());
    let second = read(dir.path());

    assert_eq!(row(&first, "c1").refs, row(&second, "c1").refs);
}

#[test]
fn a_row_with_no_reference_carries_no_label() {
    let dir = TempDir::new().expect("tempdir");
    linear(dir.path(), 3);

    let page = read(dir.path());

    assert!(row(&page, "c0").refs.is_empty());
}

#[test]
fn a_repository_with_few_references_does_not_report_truncation() {
    let dir = TempDir::new().expect("tempdir");
    linear(dir.path(), 2);

    assert!(!read(dir.path()).refs_truncated);
}

// ── Detached HEAD ────────────────────────────────────────────────────────────

#[test]
fn a_detached_head_still_draws_and_marks_where_it_is() {
    let dir = TempDir::new().expect("tempdir");
    let repo = linear(dir.path(), 4);

    let second_newest = read(dir.path()).rows[1].commit.sha.clone();
    repo.set_head_detached(second_newest.parse().expect("oid"))
        .expect("detach");

    let page = read(dir.path());

    match &page.head {
        Head::Detached { sha } => assert!(second_newest.starts_with(sha)),
        other => panic!("expected a detached head, got {other:?}"),
    }
    assert_eq!(page.rows.len(), 3, "history is what HEAD can reach");
    assert_eq!(
        page.rows[0].refs.first().map(|found| found.kind),
        Some(RefKind::Head),
        "the marker is what makes a detached HEAD visible"
    );
}

// ── Shallow repositories ─────────────────────────────────────────────────────

#[test]
fn a_shallow_repository_says_so_and_its_lanes_run_off_the_bottom() {
    let dir = TempDir::new().expect("tempdir");
    let repo = linear(dir.path(), 4);

    let boundary = read(dir.path()).rows[1].commit.sha.clone();
    fs::write(repo.path().join("shallow"), format!("{boundary}\n")).expect("write shallow");

    let page = read(dir.path());

    assert!(page.shallow, "the interface has to be able to say so");
    assert!(page.rows.len() <= 2);
    let last = page.rows.last().expect("a row");
    assert!(
        !last.continuing.is_empty() || last.edges.is_empty(),
        "either the line leaves the window or the history genuinely ended"
    );
}

// ── The bounded window ───────────────────────────────────────────────────────

#[test]
fn the_graph_never_reads_more_than_a_page() {
    let dir = TempDir::new().expect("tempdir");
    linear(dir.path(), PAGE * 3);

    let page = read(dir.path());

    assert_eq!(page.rows.len(), PAGE);
    assert!(page.next.is_some(), "there is more, so there is a cursor");
}

#[test]
fn the_next_page_of_the_graph_continues_where_the_last_one_stopped() {
    let dir = TempDir::new().expect("tempdir");
    linear(dir.path(), PAGE * 2);

    let first = read(dir.path());
    let cursor = first.next.clone().expect("a second page");
    let second = ready(Libgit2.graph(dir.path(), Some(&cursor)));

    assert_eq!(second.rows.len(), PAGE);
    assert_eq!(
        second.rows[0].commit.sha,
        cursor.as_str(),
        "the cursor names the first row of the page it opens"
    );
    assert_eq!(second.next, None);

    let overlap = first
        .rows
        .iter()
        .filter(|a| second.rows.iter().any(|b| b.commit.sha == a.commit.sha))
        .count();
    assert_eq!(overlap, 0, "no commit is drawn on two pages");
}

#[test]
fn the_graph_and_the_history_agree_about_where_the_next_page_starts() {
    // Two commands, one walk. A cursor from one must be a cursor for the other,
    // or switching between the list and the graph would jump.
    let dir = TempDir::new().expect("tempdir");
    linear(dir.path(), PAGE * 2);

    let graph_cursor = read(dir.path()).next;
    let history_cursor = match Libgit2.history(dir.path(), None) {
        mira_git::CommitPage::Ready { next, .. } => next,
        other => panic!("expected a page, got {other:?}"),
    };

    assert_eq!(graph_cursor, history_cursor);
}

#[test]
fn a_lane_open_at_the_bottom_of_a_page_is_reported_as_continuing() {
    // The bound made visible: the window ends, the history does not, and the
    // lines say so rather than the picture pretending the history stopped.
    let dir = TempDir::new().expect("tempdir");
    linear(dir.path(), PAGE * 2);

    let page = read(dir.path());
    let last = page.rows.last().expect("a row");

    assert_eq!(last.edges.len(), 1);
    assert_eq!(last.edges[0].kind, EdgeKind::Straight);
    assert_eq!(
        last.continuing,
        [0],
        "the mainline leaves the window rather than ending"
    );
}

// ── Unusual and malformed history ────────────────────────────────────────────

#[test]
fn a_plain_directory_has_no_graph() {
    let dir = TempDir::new().expect("tempdir");

    assert_eq!(Libgit2.graph(dir.path(), None), CommitGraph::NotARepository);
}

#[test]
fn an_unreadable_repository_says_why() {
    let dir = TempDir::new().expect("tempdir");
    fs::create_dir(dir.path().join(".git")).expect("create .git");
    fs::write(dir.path().join(".git").join("HEAD"), "not a ref").expect("write");

    match Libgit2.graph(dir.path(), None) {
        CommitGraph::Unreadable { detail } => assert!(!detail.is_empty()),
        other => panic!("expected an unreadable repository, got {other:?}"),
    }
}

#[test]
fn a_history_that_cannot_be_followed_still_draws_what_it_could_read() {
    let dir = TempDir::new().expect("tempdir");
    let repo = linear(dir.path(), 6);

    let broken = read(dir.path()).rows[2].commit.sha.clone();
    let (prefix, rest) = broken.split_at(2);
    let object = repo.path().join("objects").join(prefix).join(rest);
    assert!(object.exists(), "expected a loose object to remove");
    fs::remove_file(&object).expect("remove object");

    match Libgit2.graph(dir.path(), None) {
        CommitGraph::Ready { rows, .. } => {
            assert!(
                rows.len() < 6,
                "a broken history is truncated, never invented"
            );
            assert!(
                rows.iter().all(|row| row.lane < 8),
                "and every row it did read is still placed"
            );
        }
        CommitGraph::Unreadable { detail } => assert!(!detail.is_empty()),
        other => panic!("expected a graph or a readable failure, got {other:?}"),
    }
}

#[test]
fn a_cursor_naming_a_commit_that_is_not_here_draws_an_empty_page() {
    let dir = TempDir::new().expect("tempdir");
    linear(dir.path(), 3);

    let page = ready(
        Libgit2.graph(
            dir.path(),
            Some(
                &"0123456789abcdef0123456789abcdef01234567"
                    .parse()
                    .expect("id"),
            ),
        ),
    );

    assert!(page.rows.is_empty());
    assert_eq!(page.next, None);
    assert_eq!(page.lanes, 0);
}

// ── The provider boundary ────────────────────────────────────────────────────

#[test]
fn the_graph_is_read_through_the_trait_like_everything_else() {
    fn draw(provider: &dyn GitProvider, root: &Path) -> CommitGraph {
        provider.graph(root, None)
    }

    let dir = TempDir::new().expect("tempdir");
    linear(dir.path(), 2);

    assert_eq!(ready(draw(&Libgit2, dir.path())).rows.len(), 2);
}

#[test]
fn a_graph_belongs_to_the_repository_and_not_to_the_directory_asked_about() {
    let dir = TempDir::new().expect("tempdir");
    let package = dir.path().join("apps").join("web");
    fs::create_dir_all(&package).expect("create package");
    linear(dir.path(), 3);

    let from_root = read(dir.path());
    let from_package = read(&package);

    assert_eq!(subjects(&from_root), subjects(&from_package));
    assert_eq!(from_root.lanes, from_package.lanes);
}

#[test]
fn reading_a_graph_changes_nothing_about_the_repository() {
    // The graph is a picture, not a client. Reading it twice must leave the
    // repository exactly as it was — same HEAD, same refs, same commits.
    let dir = TempDir::new().expect("tempdir");
    let repo = merged(dir.path());

    let before = references(&repo);
    let head_before = repo.head().expect("head").target();

    let _ = read(dir.path());
    let _ = read(dir.path());

    assert_eq!(before, references(&repo), "no reference moved");
    assert_eq!(
        head_before,
        repo.head().expect("head").target(),
        "HEAD did not move"
    );
}
