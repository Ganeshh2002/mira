//! History filtering, against real repositories.
//!
//! Four filters that compose, one budget, and one rule that matters more than any
//! of them: **a search that ran out of budget is a different answer from a search
//! that found nothing**, and the tests below check that distinction as carefully
//! as they check the matching.
//!
//! Nothing here is a Git argument. A branch is a commit id, a file is a position
//! in a change set, and author and subject are values compared in Rust — so these
//! tests are about *matching*, and `src-tauri/tests/wire.rs` is about what can
//! arrive in the first place.

use std::fs;
use std::path::Path;

use git2::{Repository, RepositoryInitOptions, Signature};
use mira_git::{
    ChangedFiles, Commit, CommitId, DiffScope, FileSubject, FilterCursor, FilteredHistory,
    GitProvider, Head, HistoryFilter, KnownAuthors, KnownRefs, Libgit2, RefKind, ScanStopped, Term,
    MAX_FILTER_SCAN, PAGE,
};
use tempfile::TempDir;

// ── Fixtures ─────────────────────────────────────────────────────────────────

const FIRST: i64 = 1_700_000_000;

fn empty_repo(dir: &Path) -> Repository {
    let mut options = RepositoryInitOptions::new();
    options.initial_head("main");
    Repository::init_opts(dir, &options).expect("init")
}

fn commit_as(repo: &Repository, author: &str, subject: &str, when: i64) -> git2::Oid {
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
    let who = Signature::new(author, "a@b.c", &git2::Time::new(when, 0)).expect("signature");

    repo.commit(Some("HEAD"), &who, &who, subject, &tree, &borrowed)
        .expect("commit")
}

fn write(dir: &Path, path: &str, body: &str) {
    let full = dir.join(path);
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent).expect("dirs");
    }
    fs::write(full, body).expect("write");
}

fn term(text: &str) -> Term {
    text.parse().expect("a filter term")
}

fn id(oid: git2::Oid) -> CommitId {
    oid.to_string().parse().expect("a commit id")
}

/// The subject naming `path` in a commit's change set.
fn subject_for(root: &Path, commit: git2::Oid, path: &str) -> FileSubject {
    let scope = DiffScope::Commit { commit: id(commit) };
    let ChangedFiles::Ready { files, .. } = Libgit2.changed_files(root, &scope) else {
        panic!("expected a change list");
    };
    let found = files
        .iter()
        .find(|change| change.path == path)
        .unwrap_or_else(|| panic!("no change for {path}"));

    FileSubject {
        scope,
        at: found.at,
        before: false,
    }
}

struct Found {
    head: Head,
    commits: Vec<Commit>,
    next: Option<FilterCursor>,
    scanned: u32,
    stopped: ScanStopped,
    shallow: bool,
}

fn ready(result: FilteredHistory) -> Found {
    match result {
        FilteredHistory::Ready {
            head,
            commits,
            next,
            scanned,
            stopped,
            shallow,
        } => Found {
            head,
            commits,
            next,
            scanned,
            stopped,
            shallow,
        },
        other => panic!("expected a filtered page, got {other:?}"),
    }
}

fn search(root: &Path, filter: &HistoryFilter) -> Found {
    ready(Libgit2.filtered_history(root, filter, None))
}

fn subjects(found: &Found) -> Vec<&str> {
    found
        .commits
        .iter()
        .map(|commit| commit.subject.as_str())
        .collect()
}

/// A repository with two authors, varied subjects, and a rarely-touched file.
fn varied(dir: &Path) -> Repository {
    let repo = empty_repo(dir);

    write(dir, "rare.ts", "one\n");
    commit_as(&repo, "Ada Lovelace", "feat: add the rare file", FIRST);

    write(dir, "noise.txt", "1\n");
    commit_as(&repo, "Grace Hopper", "fix: a widget problem", FIRST + 60);

    write(dir, "noise.txt", "2\n");
    commit_as(
        &repo,
        "Ada Lovelace",
        "docs: explain the widget",
        FIRST + 120,
    );

    write(dir, "rare.ts", "one\ntwo\n");
    commit_as(
        &repo,
        "Grace Hopper",
        "feat: extend the rare file",
        FIRST + 180,
    );

    write(dir, "noise.txt", "3\n");
    commit_as(&repo, "Grace Hopper", "chore: tidy the gadget", FIRST + 240);

    repo
}

// ── Author ───────────────────────────────────────────────────────────────────

#[test]
fn an_author_filter_keeps_only_that_authors_commits() {
    let dir = TempDir::new().expect("tempdir");
    varied(dir.path());

    let found = search(
        dir.path(),
        &HistoryFilter {
            author: Some(term("Grace Hopper")),
            ..HistoryFilter::default()
        },
    );

    assert_eq!(
        subjects(&found),
        [
            "chore: tidy the gadget",
            "feat: extend the rare file",
            "fix: a widget problem"
        ]
    );
    assert_eq!(found.stopped, ScanStopped::No);
}

#[test]
fn an_author_filter_ignores_case_and_matches_part_of_a_name() {
    let dir = TempDir::new().expect("tempdir");
    varied(dir.path());

    for wanted in ["grace", "HOPPER", "ce Hop"] {
        let found = search(
            dir.path(),
            &HistoryFilter {
                author: Some(term(wanted)),
                ..HistoryFilter::default()
            },
        );
        assert_eq!(found.commits.len(), 3, "{wanted} should match Grace Hopper");
    }
}

// ── Subject ──────────────────────────────────────────────────────────────────

#[test]
fn a_subject_filter_is_a_case_insensitive_substring_of_the_subject_line() {
    let dir = TempDir::new().expect("tempdir");
    varied(dir.path());

    let found = search(
        dir.path(),
        &HistoryFilter {
            subject: Some(term("WIDGET")),
            ..HistoryFilter::default()
        },
    );

    assert_eq!(
        subjects(&found),
        ["docs: explain the widget", "fix: a widget problem"]
    );
}

#[test]
fn a_subject_filter_is_not_a_pattern() {
    // Documented semantics, asserted: substring, never a glob or a regex. A `*`
    // matches a literal asterisk, which is in no subject here.
    let dir = TempDir::new().expect("tempdir");
    varied(dir.path());

    for pattern in ["*", "wid*get", "^feat", "feat.*rare", "wid?et"] {
        let found = search(
            dir.path(),
            &HistoryFilter {
                subject: Some(term(pattern)),
                ..HistoryFilter::default()
            },
        );
        assert!(
            found.commits.is_empty(),
            "{pattern} matched something; it must be treated as literal text"
        );
    }
}

#[test]
fn a_subject_filter_does_not_search_the_body() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "app.ts", "one\n");
    commit_as(
        &repo,
        "Ada Lovelace",
        "feat: a short subject\n\nThe body mentions marzipan.",
        FIRST,
    );

    let by_subject = search(
        dir.path(),
        &HistoryFilter {
            subject: Some(term("short subject")),
            ..HistoryFilter::default()
        },
    );
    let by_body = search(
        dir.path(),
        &HistoryFilter {
            subject: Some(term("marzipan")),
            ..HistoryFilter::default()
        },
    );

    assert_eq!(by_subject.commits.len(), 1);
    assert!(
        by_body.commits.is_empty(),
        "the subject line is the subject line"
    );
}

// ── Branch ───────────────────────────────────────────────────────────────────

#[test]
fn a_branch_filter_searches_only_what_that_branch_reaches() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());

    write(dir.path(), "base.txt", "base\n");
    let base = commit_as(&repo, "Ada Lovelace", "base", FIRST);

    repo.branch("side", &repo.find_commit(base).expect("base"), false)
        .expect("branch");

    write(dir.path(), "main.txt", "main\n");
    commit_as(&repo, "Ada Lovelace", "only on main", FIRST + 60);

    repo.set_head("refs/heads/side").expect("head");
    repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
        .expect("checkout");
    write(dir.path(), "side.txt", "side\n");
    let side = commit_as(&repo, "Ada Lovelace", "only on side", FIRST + 120);

    repo.set_head("refs/heads/main").expect("head");
    repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
        .expect("checkout");

    let on_side = search(
        dir.path(),
        &HistoryFilter {
            branch: Some(id(side)),
            ..HistoryFilter::default()
        },
    );

    assert_eq!(subjects(&on_side), ["only on side", "base"]);
    assert!(
        !subjects(&on_side).contains(&"only on main"),
        "a branch filter does not reach the other branch"
    );
}

#[test]
fn the_branch_filter_carries_a_commit_id_and_the_refs_say_which() {
    // The interface picks a ref from this list and sends the tip it was given.
    // There is never a ref *name* on the wire.
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "app.ts", "one\n");
    let base = commit_as(&repo, "Ada Lovelace", "base", FIRST);
    repo.branch("release", &repo.find_commit(base).expect("base"), false)
        .expect("branch");
    repo.tag_lightweight(
        "v1.0",
        repo.find_commit(base).expect("base").as_object(),
        false,
    )
    .expect("tag");

    let KnownRefs::Ready { refs, truncated } = Libgit2.known_refs(dir.path()) else {
        panic!("expected a ref list");
    };

    assert!(!truncated);
    let names: Vec<&str> = refs.iter().map(|found| found.name.as_str()).collect();
    assert!(names.contains(&"main"), "got {names:?}");
    assert!(names.contains(&"release"), "got {names:?}");
    assert!(names.contains(&"v1.0"), "got {names:?}");

    let release = refs
        .iter()
        .find(|found| found.name == "release")
        .expect("release");
    assert_eq!(release.kind, RefKind::Branch);
    assert_eq!(release.tip, id(base), "the tip is a commit id, not a name");

    let tag = refs.iter().find(|found| found.name == "v1.0").expect("tag");
    assert_eq!(tag.kind, RefKind::Tag);
}

#[test]
fn a_branch_tip_that_is_not_here_is_a_stale_selection() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "app.ts", "one\n");
    commit_as(&repo, "Ada Lovelace", "one", FIRST);

    let gone: CommitId = "0123456789abcdef0123456789abcdef01234567"
        .parse()
        .expect("id");

    assert_eq!(
        Libgit2.filtered_history(
            dir.path(),
            &HistoryFilter {
                branch: Some(gone),
                ..HistoryFilter::default()
            },
            None
        ),
        FilteredHistory::Unknown
    );
}

// ── File ─────────────────────────────────────────────────────────────────────

#[test]
fn a_file_filter_keeps_only_commits_that_touched_it() {
    let dir = TempDir::new().expect("tempdir");
    let repo = varied(dir.path());
    let head = repo.head().expect("head").target().expect("target");
    let _ = head;

    let extended = repo
        .revparse_single("HEAD~1")
        .ok()
        .and_then(|object| object.peel_to_commit().ok())
        .expect("the commit that extended the file")
        .id();

    let found = search(
        dir.path(),
        &HistoryFilter {
            file: Some(subject_for(dir.path(), extended, "rare.ts")),
            ..HistoryFilter::default()
        },
    );

    assert_eq!(
        subjects(&found),
        ["feat: extend the rare file", "feat: add the rare file"]
    );
}

#[test]
fn a_file_filter_follows_a_rename() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());

    let body = (0..40).map(|n| format!("line {n}\n")).collect::<String>();
    write(dir.path(), "old.ts", &body);
    commit_as(&repo, "Ada Lovelace", "add under the old name", FIRST);

    fs::remove_file(dir.path().join("old.ts")).expect("remove");
    write(dir.path(), "new.ts", &body);
    let renamed = commit_as(&repo, "Ada Lovelace", "rename", FIRST + 60);

    write(dir.path(), "noise.txt", "noise\n");
    commit_as(&repo, "Ada Lovelace", "unrelated", FIRST + 120);

    let found = search(
        dir.path(),
        &HistoryFilter {
            file: Some(subject_for(dir.path(), renamed, "new.ts")),
            ..HistoryFilter::default()
        },
    );

    assert_eq!(subjects(&found), ["rename", "add under the old name"]);
}

#[test]
fn a_file_subject_that_is_not_there_is_a_stale_selection() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "app.ts", "one\n");
    let first = commit_as(&repo, "Ada Lovelace", "one", FIRST);

    assert_eq!(
        Libgit2.filtered_history(
            dir.path(),
            &HistoryFilter {
                file: Some(FileSubject {
                    scope: DiffScope::Commit { commit: id(first) },
                    at: 99,
                    before: false,
                }),
                ..HistoryFilter::default()
            },
            None
        ),
        FilteredHistory::Unknown
    );
}

// ── Composition ──────────────────────────────────────────────────────────────

#[test]
fn filters_compose_and_every_one_has_to_match() {
    let dir = TempDir::new().expect("tempdir");
    varied(dir.path());

    let both = search(
        dir.path(),
        &HistoryFilter {
            author: Some(term("Grace")),
            subject: Some(term("widget")),
            ..HistoryFilter::default()
        },
    );

    assert_eq!(
        subjects(&both),
        ["fix: a widget problem"],
        "Ada also wrote about the widget, and Grace also wrote about the gadget"
    );
}

#[test]
fn a_branch_an_author_and_a_subject_narrow_together() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());

    write(dir.path(), "base.txt", "base\n");
    let base = commit_as(&repo, "Ada Lovelace", "feat: base widget", FIRST);
    repo.branch("side", &repo.find_commit(base).expect("base"), false)
        .expect("branch");

    write(dir.path(), "main.txt", "m\n");
    commit_as(&repo, "Ada Lovelace", "feat: main widget", FIRST + 60);

    repo.set_head("refs/heads/side").expect("head");
    repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
        .expect("checkout");
    write(dir.path(), "side.txt", "s\n");
    commit_as(&repo, "Grace Hopper", "feat: side widget", FIRST + 120);
    write(dir.path(), "side2.txt", "s\n");
    let side = commit_as(&repo, "Ada Lovelace", "feat: side gadget", FIRST + 180);

    let found = search(
        dir.path(),
        &HistoryFilter {
            branch: Some(id(side)),
            author: Some(term("Ada")),
            subject: Some(term("widget")),
            file: None,
        },
    );

    assert_eq!(
        subjects(&found),
        ["feat: base widget"],
        "on this branch, by Ada, about the widget"
    );
}

#[test]
fn every_filter_at_once_still_matches_what_it_should() {
    let dir = TempDir::new().expect("tempdir");
    let repo = varied(dir.path());
    let extended = repo
        .revparse_single("HEAD~1")
        .ok()
        .and_then(|object| object.peel_to_commit().ok())
        .expect("commit")
        .id();
    let head = repo.head().expect("head").target().expect("target");

    let found = search(
        dir.path(),
        &HistoryFilter {
            branch: Some(id(head)),
            author: Some(term("Grace")),
            subject: Some(term("extend")),
            file: Some(subject_for(dir.path(), extended, "rare.ts")),
        },
    );

    assert_eq!(subjects(&found), ["feat: extend the rare file"]);
}

// ── Nothing found, and nothing found *yet* ───────────────────────────────────

#[test]
fn a_filter_that_matches_nothing_says_so_without_a_budget_excuse() {
    let dir = TempDir::new().expect("tempdir");
    varied(dir.path());

    let found = search(
        dir.path(),
        &HistoryFilter {
            subject: Some(term("marzipan")),
            ..HistoryFilter::default()
        },
    );

    assert!(found.commits.is_empty());
    assert_eq!(
        found.stopped,
        ScanStopped::No,
        "the search reached the end; nothing matches, and that is the whole answer"
    );
    assert_eq!(found.next, None);
}

#[test]
fn a_search_that_runs_out_of_budget_says_how_far_it_looked() {
    // The distinction the whole feature turns on: this looks identical to "no
    // results" unless it says otherwise, and it is a completely different fact.
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());

    write(dir.path(), "rare.ts", "one\n");
    commit_as(&repo, "Marzipan Author", "the only match", FIRST);
    for n in 1..(MAX_FILTER_SCAN + 40) {
        write(dir.path(), "noise.txt", &format!("{n}\n"));
        commit_as(
            &repo,
            "Ada Lovelace",
            &format!("noise {n}"),
            FIRST + n as i64 * 60,
        );
    }

    let found = search(
        dir.path(),
        &HistoryFilter {
            author: Some(term("Marzipan")),
            ..HistoryFilter::default()
        },
    );

    assert!(found.commits.is_empty(), "the match is below the budget");
    match found.stopped {
        ScanStopped::Budget { scanned, limit } => {
            assert_eq!(scanned as usize, MAX_FILTER_SCAN);
            assert_eq!(limit as usize, MAX_FILTER_SCAN);
        }
        other => panic!("expected a budget stop, got {other:?}"),
    }
    assert!(
        found.next.is_some(),
        "and the rest is reachable by continuing"
    );
}

#[test]
fn continuing_past_the_budget_eventually_finds_the_match() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());

    write(dir.path(), "rare.ts", "one\n");
    commit_as(&repo, "Marzipan Author", "the only match", FIRST);
    for n in 1..(MAX_FILTER_SCAN + 40) {
        write(dir.path(), "noise.txt", &format!("{n}\n"));
        commit_as(
            &repo,
            "Ada Lovelace",
            &format!("noise {n}"),
            FIRST + n as i64 * 60,
        );
    }

    let filter = HistoryFilter {
        author: Some(term("Marzipan")),
        ..HistoryFilter::default()
    };
    let mut from: Option<CommitId> = None;
    let mut seen: Vec<String> = Vec::new();
    let mut requests = 0;

    loop {
        let found = ready(Libgit2.filtered_history(dir.path(), &filter, from.as_ref()));
        seen.extend(found.commits.iter().map(|commit| commit.subject.clone()));
        requests += 1;
        assert!(requests <= 5, "two budgets should be enough");

        match found.next {
            Some(cursor) => from = Some(cursor.from),
            None => break,
        }
    }

    assert_eq!(seen, ["the only match"]);
    assert_eq!(requests, 2, "one budget short, one to finish");
}

#[test]
fn a_search_never_examines_more_than_its_budget() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    write(dir.path(), "app.ts", "one\n");
    commit_as(&repo, "Ada Lovelace", "first", FIRST);
    for n in 1..(MAX_FILTER_SCAN + 100) {
        write(dir.path(), "noise.txt", &format!("{n}\n"));
        commit_as(
            &repo,
            "Ada Lovelace",
            &format!("noise {n}"),
            FIRST + n as i64 * 60,
        );
    }

    let found = search(
        dir.path(),
        &HistoryFilter {
            subject: Some(term("nothing matches this")),
            ..HistoryFilter::default()
        },
    );

    assert!((found.scanned as usize) <= MAX_FILTER_SCAN);
}

// ── Pagination ───────────────────────────────────────────────────────────────

#[test]
fn a_filtered_page_is_never_larger_than_the_page_size() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    for n in 0..(PAGE * 3) {
        write(dir.path(), "app.ts", &format!("v{n}\n"));
        commit_as(
            &repo,
            "Ada Lovelace",
            &format!("feat: change {n}"),
            FIRST + n as i64 * 60,
        );
    }

    let found = search(
        dir.path(),
        &HistoryFilter {
            author: Some(term("Ada")),
            ..HistoryFilter::default()
        },
    );

    assert_eq!(found.commits.len(), PAGE);
    assert!(found.next.is_some());
}

#[test]
fn the_pages_of_a_filtered_search_are_the_whole_result_in_order() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    let total = PAGE * 2 + 6;
    for n in 0..total {
        write(dir.path(), "app.ts", &format!("v{n}\n"));
        // Only every other commit is Grace's.
        let author = if n % 2 == 0 {
            "Grace Hopper"
        } else {
            "Ada Lovelace"
        };
        commit_as(&repo, author, &format!("change {n}"), FIRST + n as i64 * 60);
    }

    let filter = HistoryFilter {
        author: Some(term("Grace")),
        ..HistoryFilter::default()
    };
    let mut from: Option<CommitId> = None;
    let mut seen: Vec<String> = Vec::new();
    let mut pages = 0;

    loop {
        let found = ready(Libgit2.filtered_history(dir.path(), &filter, from.as_ref()));
        seen.extend(found.commits.iter().map(|commit| commit.subject.clone()));
        pages += 1;
        assert!(pages <= 10, "paging must terminate");
        match found.next {
            Some(cursor) => from = Some(cursor.from),
            None => break,
        }
    }

    let expected: Vec<String> = (0..total)
        .rev()
        .filter(|n| n % 2 == 0)
        .map(|n| format!("change {n}"))
        .collect();
    assert_eq!(seen, expected);
    assert!(pages >= 2, "the result needed more than one page");
}

#[test]
fn a_filtered_page_never_repeats_a_commit() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());
    for n in 0..(PAGE * 2) {
        write(dir.path(), "app.ts", &format!("v{n}\n"));
        commit_as(
            &repo,
            "Ada Lovelace",
            &format!("change {n}"),
            FIRST + n as i64 * 60,
        );
    }

    let filter = HistoryFilter {
        author: Some(term("Ada")),
        ..HistoryFilter::default()
    };
    let first = search(dir.path(), &filter);
    let cursor = first.next.clone().expect("a second page");
    let second = ready(Libgit2.filtered_history(dir.path(), &filter, Some(&cursor.from)));

    let overlap = first
        .commits
        .iter()
        .filter(|a| second.commits.iter().any(|b| b.sha == a.sha))
        .count();
    assert_eq!(overlap, 0);
}

// ── Authors offered for choosing ─────────────────────────────────────────────

#[test]
fn the_authors_offered_are_the_ones_in_what_was_examined() {
    let dir = TempDir::new().expect("tempdir");
    varied(dir.path());

    let KnownAuthors::Ready {
        authors,
        scanned,
        stopped,
    } = Libgit2.known_authors(dir.path(), None)
    else {
        panic!("expected an author list");
    };

    assert_eq!(scanned, 5);
    assert_eq!(stopped, ScanStopped::No);
    // Most frequent first: Grace wrote three, Ada two.
    assert_eq!(authors[0].name.as_str(), "Grace Hopper");
    assert_eq!(authors[0].commits, 3);
    assert_eq!(authors[1].name.as_str(), "Ada Lovelace");
    assert_eq!(authors[1].commits, 2);
}

#[test]
fn an_author_list_from_an_empty_repository_is_empty_rather_than_a_failure() {
    let dir = TempDir::new().expect("tempdir");
    empty_repo(dir.path());

    let KnownAuthors::Ready { authors, .. } = Libgit2.known_authors(dir.path(), None) else {
        panic!("expected an author list");
    };
    assert!(authors.is_empty());
}

// ── Unusual repositories ─────────────────────────────────────────────────────

#[test]
fn a_merge_is_matched_like_any_other_commit() {
    let dir = TempDir::new().expect("tempdir");
    let repo = empty_repo(dir.path());

    write(dir.path(), "base.txt", "base\n");
    let base = commit_as(&repo, "Ada Lovelace", "base", FIRST);
    repo.branch("side", &repo.find_commit(base).expect("base"), false)
        .expect("branch");
    write(dir.path(), "main.txt", "m\n");
    let main = commit_as(&repo, "Ada Lovelace", "main work", FIRST + 60);

    repo.set_head("refs/heads/side").expect("head");
    repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
        .expect("checkout");
    write(dir.path(), "side.txt", "s\n");
    let side = commit_as(&repo, "Ada Lovelace", "side work", FIRST + 120);

    repo.set_head("refs/heads/main").expect("head");
    repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
        .expect("checkout");
    write(dir.path(), "side.txt", "s\n");
    {
        let mut index = repo.index().expect("index");
        index
            .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
            .expect("add");
        index.write().expect("write index");
        let tree = repo
            .find_tree(index.write_tree().expect("write tree"))
            .expect("tree");
        let who =
            Signature::new("Grace Hopper", "a@b.c", &git2::Time::new(FIRST + 180, 0)).expect("sig");
        repo.commit(
            Some("HEAD"),
            &who,
            &who,
            "merge: bring in the side",
            &tree,
            &[
                &repo.find_commit(main).expect("main"),
                &repo.find_commit(side).expect("side"),
            ],
        )
        .expect("merge");
    }

    let found = search(
        dir.path(),
        &HistoryFilter {
            subject: Some(term("merge")),
            ..HistoryFilter::default()
        },
    );

    assert_eq!(subjects(&found), ["merge: bring in the side"]);
    assert_eq!(
        search(
            dir.path(),
            &HistoryFilter {
                author: Some(term("Grace")),
                ..HistoryFilter::default()
            }
        )
        .commits
        .len(),
        1,
        "and a merge's author is its author"
    );
}

#[test]
fn a_detached_head_searches_from_where_head_actually_is() {
    let dir = TempDir::new().expect("tempdir");
    let repo = varied(dir.path());
    let second = repo
        .revparse_single("HEAD~2")
        .ok()
        .and_then(|object| object.peel_to_commit().ok())
        .expect("commit")
        .id();

    repo.set_head_detached(second).expect("detach");

    let found = search(
        dir.path(),
        &HistoryFilter {
            author: Some(term("Grace")),
            ..HistoryFilter::default()
        },
    );

    assert!(matches!(found.head, Head::Detached { .. }));
    assert_eq!(
        subjects(&found),
        ["fix: a widget problem"],
        "only what this HEAD reaches"
    );
}

#[test]
fn a_shallow_repository_says_so_and_searches_what_it_has() {
    let dir = TempDir::new().expect("tempdir");
    let repo = varied(dir.path());
    let boundary = repo
        .revparse_single("HEAD~1")
        .ok()
        .and_then(|object| object.peel_to_commit().ok())
        .expect("commit")
        .id();
    fs::write(repo.path().join("shallow"), format!("{boundary}\n")).expect("shallow");

    let found = search(
        dir.path(),
        &HistoryFilter {
            author: Some(term("Grace")),
            ..HistoryFilter::default()
        },
    );

    assert!(found.shallow);
    assert!(found.commits.len() <= 2);
}

#[test]
fn a_repository_with_no_commits_matches_nothing_and_does_not_fail() {
    let dir = TempDir::new().expect("tempdir");
    empty_repo(dir.path());

    let found = search(
        dir.path(),
        &HistoryFilter {
            subject: Some(term("anything")),
            ..HistoryFilter::default()
        },
    );

    assert_eq!(found.head, Head::Unborn);
    assert!(found.commits.is_empty());
    assert_eq!(found.scanned, 0);
    assert_eq!(found.stopped, ScanStopped::No);
}

#[test]
fn a_plain_directory_has_nothing_to_filter() {
    let dir = TempDir::new().expect("tempdir");

    assert_eq!(
        Libgit2.filtered_history(dir.path(), &HistoryFilter::default(), None),
        FilteredHistory::NotARepository
    );
    assert_eq!(Libgit2.known_refs(dir.path()), KnownRefs::NotARepository);
    assert_eq!(
        Libgit2.known_authors(dir.path(), None),
        KnownAuthors::NotARepository
    );
}

#[test]
fn an_unreadable_repository_says_why() {
    let dir = TempDir::new().expect("tempdir");
    fs::create_dir(dir.path().join(".git")).expect("create .git");
    fs::write(dir.path().join(".git").join("HEAD"), "not a ref").expect("write");

    match Libgit2.filtered_history(dir.path(), &HistoryFilter::default(), None) {
        FilteredHistory::Unreadable { detail } => assert!(!detail.is_empty()),
        other => panic!("expected an unreadable repository, got {other:?}"),
    }
}

#[test]
fn a_history_with_an_unreachable_object_returns_what_it_could_read() {
    let dir = TempDir::new().expect("tempdir");
    let repo = varied(dir.path());

    let broken = repo
        .revparse_single("HEAD~2")
        .ok()
        .and_then(|object| object.peel_to_commit().ok())
        .expect("commit")
        .id()
        .to_string();
    let (prefix, rest) = broken.split_at(2);
    let object = repo.path().join("objects").join(prefix).join(rest);
    assert!(object.exists(), "expected a loose object");
    fs::remove_file(&object).expect("remove");

    match Libgit2.filtered_history(
        dir.path(),
        &HistoryFilter {
            author: Some(term("Grace")),
            ..HistoryFilter::default()
        },
        None,
    ) {
        FilteredHistory::Ready { commits, .. } => {
            assert!(
                commits.len() < 3,
                "a broken history is truncated, never invented"
            );
        }
        FilteredHistory::Unreadable { detail } => assert!(!detail.is_empty()),
        other => panic!("expected a page or a readable failure, got {other:?}"),
    }
}

// ── The boundary and the provider ────────────────────────────────────────────

#[test]
fn a_filter_is_read_through_the_trait_like_everything_else() {
    fn read(provider: &dyn GitProvider, root: &Path) -> FilteredHistory {
        provider.filtered_history(root, &HistoryFilter::default(), None)
    }

    let dir = TempDir::new().expect("tempdir");
    varied(dir.path());

    assert!(matches!(
        read(&Libgit2, dir.path()),
        FilteredHistory::Ready { .. }
    ));
}

#[test]
fn a_filter_carries_no_ref_name_and_no_path() {
    // The contract: everything a filter holds is either a validated commit id, a
    // change-set position, or a value that is only ever compared. None of them is
    // a string Git would interpret.
    let dir = TempDir::new().expect("tempdir");
    let repo = varied(dir.path());
    let head = repo.head().expect("head").target().expect("target");

    let filter = HistoryFilter {
        branch: Some(id(head)),
        author: Some(term("Grace Hopper")),
        subject: Some(term("widget")),
        file: Some(subject_for(dir.path(), head, "noise.txt")),
    };
    let written = serde_json::to_string(&filter).expect("serialise");

    assert!(!written.contains("main"), "no ref name: {written}");
    assert!(!written.contains("refs/"), "no ref path: {written}");
    assert!(!written.contains("noise.txt"), "no file path: {written}");
    assert!(
        written.contains(&head.to_string()),
        "the branch is a commit id"
    );
}

#[test]
fn filtering_changes_nothing_about_the_repository() {
    let dir = TempDir::new().expect("tempdir");
    let repo = varied(dir.path());

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

    let head = head_before.expect("head");
    let _ = Libgit2.filtered_history(
        dir.path(),
        &HistoryFilter {
            branch: Some(id(head)),
            author: Some(term("Grace")),
            subject: Some(term("widget")),
            file: None,
        },
        None,
    );
    let _ = Libgit2.known_refs(dir.path());
    let _ = Libgit2.known_authors(dir.path(), None);

    assert_eq!(before, references(&repo), "no reference moved");
    assert_eq!(
        head_before,
        repo.head().expect("head").target(),
        "HEAD did not move"
    );
    assert_eq!(
        status_before,
        repo.statuses(None).expect("statuses").len(),
        "nothing was staged, checked out or written"
    );
}
