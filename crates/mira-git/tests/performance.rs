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
//!
//! Slice 5b adds the comparison that decided the graph's design: a page of
//! history, the same page with lanes and labels, and what a **topologically
//! ordered** walk of the same repository costs. The last one is measured here and
//! used nowhere, which is the point — it is the number that justifies the bound
//! ([ADR-0015](../../../docs/adr/0015-graph-lanes.md)).

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use git2::{Repository, RepositoryInitOptions, Signature, Sort};
use mira_git::{CommitGraph, CommitId, CommitPage, GitProvider, Libgit2, PAGE};
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

/// One page of graph: the same walk, plus parents, lanes and labels.
fn graph_page(root: &Path) -> (Duration, usize, u32) {
    let started = Instant::now();
    let page = Libgit2.graph(root, None);
    let took = started.elapsed();

    match page {
        CommitGraph::Ready { rows, lanes, .. } => (took, rows.len(), lanes),
        other => panic!("expected a graph, got {other:?}"),
    }
}

/// What a topologically ordered walk of the *whole* repository costs.
///
/// Measured with `git2` directly, because nothing in `mira-git` does this — that
/// is the finding. libgit2's sorted revwalks preprocess the entire reachable
/// history before yielding a single commit, so asking for topological order to
/// draw twenty-five rows means walking every commit behind them.
fn topological_first_page(root: &Path) -> Duration {
    let repo = Repository::open(root).expect("open");

    let started = Instant::now();
    let mut walk = repo.revwalk().expect("revwalk");
    walk.set_sorting(Sort::TOPOLOGICAL).expect("sorting");
    walk.push_head().expect("head");
    let read = walk.take(PAGE).filter_map(Result::ok).count();
    let took = started.elapsed();

    assert_eq!(
        read, PAGE,
        "the same twenty-five rows, however they were sorted"
    );
    took
}

#[test]
#[ignore = "builds repositories of up to 10,000 commits; run it deliberately"]
fn topological_order_would_cost_the_whole_repository_to_draw_one_page() {
    // The measurement that decided the design. All three columns produce the same
    // twenty-five rows; only the last one reads the whole history to do it.
    println!();
    println!("  commits   history page   graph page   lanes   topological page");
    println!("  -------   ------------   ----------   -----   ----------------");

    let mut history_growth = Vec::new();
    let mut graph_growth = Vec::new();
    let mut topological_growth = Vec::new();

    for count in [100_usize, 1_000, 10_000] {
        let dir = TempDir::new().expect("tempdir");
        repo_with(dir.path(), count);

        let (history, _, read) = measure(dir.path());
        let (graph, rows, lanes) = graph_page(dir.path());
        let topological = topological_first_page(dir.path());

        println!(
            "  {count:>7}   {:>9.2} ms   {:>7.2} ms   {lanes:>5}   {:>13.2} ms",
            history.as_secs_f64() * 1000.0,
            graph.as_secs_f64() * 1000.0,
            topological.as_secs_f64() * 1000.0,
        );

        assert_eq!(read, PAGE);
        assert_eq!(rows, PAGE, "the graph reads the same page as the list");
        assert_eq!(lanes, 1, "a linear history is one lane at any size");

        history_growth.push(history.as_secs_f64());
        graph_growth.push(graph.as_secs_f64());
        topological_growth.push(topological.as_secs_f64());
    }

    println!();
    println!(
        "  100 -> 10,000 commits:  history x{:.1}   graph x{:.1}   topological x{:.1}",
        ratio(&history_growth),
        ratio(&graph_growth),
        ratio(&topological_growth),
    );
    println!();

    // The assertion is about *shape*, not milliseconds: a bounded read must not
    // grow with the history behind it, and a hundredfold repository is the test.
    // Generous, because this runs on whatever machine happens to be to hand — it
    // is there to catch a return to a sorted revwalk, which is a tenfold change,
    // not to police a few per cent of noise.
    assert!(
        ratio(&graph_growth) < 10.0,
        "a graph page grew {:.1}x for a 100x repository; the walk is no longer bounded",
        ratio(&graph_growth)
    );
}

/// How much slower the largest repository was than the smallest.
fn ratio(measurements: &[f64]) -> f64 {
    match (measurements.first(), measurements.last()) {
        (Some(first), Some(last)) if *first > 0.0 => last / first,
        _ => 1.0,
    }
}
