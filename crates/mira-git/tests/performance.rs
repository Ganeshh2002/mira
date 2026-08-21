//! What history costs, measured rather than asserted.
//!
//! `roadmap.md` slice 5a asks for a page to render fast on a large repository,
//! and `roadmap.md` rule 5 asks for budgets to be checked every slice. The claim
//! worth proving is not a millisecond figure — that varies by machine and by
//! filesystem — but the **shape**: a page costs the same on a repository of ten
//! thousand commits as on one of a hundred, because the walk is bounded.
//!
//! Ignored by default. Building a ten-thousand-commit repository takes long enough
//! that it does not belong in every `cargo test`, and a wall-clock threshold in CI
//! would be a flake generator. Run it deliberately:
//!
//! ```text
//! cargo test -p mira-git --test performance -- --ignored --nocapture
//! ```

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use git2::{Repository, RepositoryInitOptions, Signature};
use mira_git::{CommitId, CommitPage, GitProvider, Libgit2, PAGE};
use tempfile::TempDir;

/// A repository of `count` commits, built as cheaply as the object format allows.
fn repo_with(dir: &Path, count: usize) -> Repository {
    let mut options = RepositoryInitOptions::new();
    options.initial_head("main");
    let repo = Repository::init_opts(dir, &options).expect("init");

    let who = Signature::new(
        "Blacknit",
        "blacknit@example.com",
        &git2::Time::new(1_700_000_000, 0),
    )
    .expect("signature");

    // One file, rewritten, so the tree stays one entry and the cost measured is
    // the walk rather than the fixture.
    let file = dir.join("body.txt");
    let mut parent: Option<git2::Oid> = None;

    for n in 0..count {
        fs::write(&file, format!("body {n}")).expect("write");
        let mut index = repo.index().expect("index");
        index.add_path(Path::new("body.txt")).expect("add");
        index.write().expect("write index");
        let tree = repo
            .find_tree(index.write_tree().expect("write tree"))
            .expect("tree");

        let parents: Vec<git2::Commit<'_>> = parent
            .iter()
            .map(|oid| repo.find_commit(*oid).expect("parent"))
            .collect();
        let borrowed: Vec<&git2::Commit<'_>> = parents.iter().collect();

        parent = Some(
            repo.commit(
                Some("HEAD"),
                &who,
                &who,
                &format!("commit {n}"),
                &tree,
                &borrowed,
            )
            .expect("commit"),
        );
    }

    repo
}

/// The first page, and the second, timed.
fn measure(root: &Path) -> (Duration, Duration, usize) {
    let started = Instant::now();
    let first = Libgit2.history(root, None);
    let first_took = started.elapsed();

    let cursor: Option<CommitId> = match &first {
        CommitPage::Ready { next, .. } => next.clone(),
        other => panic!("expected a page, got {other:?}"),
    };

    let started = Instant::now();
    let _ = Libgit2.history(root, cursor.as_ref());
    let second_took = started.elapsed();

    let read = match first {
        CommitPage::Ready { commits, .. } => commits.len(),
        _ => 0,
    };

    (first_took, second_took, read)
}

#[test]
#[ignore = "builds repositories of up to 10,000 commits; run it deliberately"]
fn a_page_costs_the_same_however_long_the_history_is() {
    println!();
    println!("  commits   first page   next page   read");
    println!("  -------   ----------   ---------   ----");

    for count in [100_usize, 1_000, 10_000] {
        let dir = TempDir::new().expect("tempdir");
        repo_with(dir.path(), count);
        let (first, second, read) = measure(dir.path());

        println!(
            "  {count:>7}   {:>8.2} ms   {:>6.2} ms   {read:>4}",
            first.as_secs_f64() * 1000.0,
            second.as_secs_f64() * 1000.0,
        );

        assert_eq!(read, PAGE, "a page is a page, whatever is behind it");
    }

    println!();
}
