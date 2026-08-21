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
//! Slice 5e adds the filter benchmarks, and their finding is the opposite of the
//! usual one: **every filter costs the same**, because loading the commit object
//! dominates and the walk pays that anyway
//! ([ADR-0018](../../../docs/adr/0018-history-filters.md)).
//!
//! Slice 5d adds the file-history benchmark, and it is the one that changed a
//! design. File history is *inherently* O(repository history) — to know whether a
//! commit touched a path you have to look at that commit — so the question was
//! never "how fast" but "**where does it stop**"
//! ([ADR-0017](../../../docs/adr/0017-file-history.md)).
//!
//! Slice 5c adds the diff benchmarks. A diff is the first read in the product
//! whose size is set by *the repository's files* rather than by a page of
//! history, so the question is not "how fast" but "**bounded by what**": a
//! forty-megabyte file and a four-thousand-file commit must cost what the limits
//! say, not what the repository holds ([ADR-0016](../../../docs/adr/0016-bounded-diffs.md)).
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
use mira_git::{
    ChangedFiles, CommitGraph, CommitId, CommitPage, DiffScope, FileDiff, FileHistory, FileSubject,
    FilteredHistory, GitProvider, HistoryFilter, Libgit2, ScanStopped, Term, MAX_BYTES, MAX_FILES,
    MAX_FILE_BYTES, MAX_FILTER_SCAN, MAX_LINES, MAX_SCAN, PAGE,
};
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

// ── Diffs ────────────────────────────────────────────────────────────────────

/// A repository whose HEAD commit adds one file of `body`.
fn repo_with_file(dir: &Path, name: &str, body: &[u8]) -> git2::Oid {
    let mut options = RepositoryInitOptions::new();
    options.initial_head("main");
    let repo = Repository::init_opts(dir, &options).expect("init");
    let who = Signature::new(
        "Blacknit",
        "blacknit@example.com",
        &git2::Time::new(1_700_000_000, 0),
    )
    .expect("signature");

    fs::write(dir.join("seed.txt"), "seed\n").expect("write");
    let first = commit_all(&repo, &who, "seed", &[]);

    fs::write(dir.join(name), body).expect("write");
    commit_all(&repo, &who, "add the subject", &[first])
}

/// A repository whose HEAD commit adds `count` small files.
fn repo_with_many(dir: &Path, count: usize) -> git2::Oid {
    let mut options = RepositoryInitOptions::new();
    options.initial_head("main");
    let repo = Repository::init_opts(dir, &options).expect("init");
    let who = Signature::new(
        "Blacknit",
        "blacknit@example.com",
        &git2::Time::new(1_700_000_000, 0),
    )
    .expect("signature");

    fs::write(dir.join("seed.txt"), "seed\n").expect("write");
    let first = commit_all(&repo, &who, "seed", &[]);

    for n in 0..count {
        fs::write(dir.join(format!("file-{n:05}.txt")), format!("body {n}\n")).expect("write");
    }
    commit_all(&repo, &who, "many files", &[first])
}

fn commit_all(
    repo: &Repository,
    who: &Signature<'_>,
    subject: &str,
    parents: &[git2::Oid],
) -> git2::Oid {
    let mut index = repo.index().expect("index");
    index
        .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
        .expect("add");
    index.write().expect("write index");
    let tree = repo
        .find_tree(index.write_tree().expect("write tree"))
        .expect("tree");
    let found: Vec<git2::Commit<'_>> = parents
        .iter()
        .map(|oid| repo.find_commit(*oid).expect("parent"))
        .collect();
    let borrowed: Vec<&git2::Commit<'_>> = found.iter().collect();

    repo.commit(Some("HEAD"), who, who, subject, &tree, &borrowed)
        .expect("commit")
}

fn scope(id: git2::Oid) -> DiffScope {
    DiffScope::Commit {
        commit: id.to_string().parse().expect("a commit id"),
    }
}

/// How long a change list takes, and how much of it came back.
fn time_list(root: &Path, at: &DiffScope) -> (Duration, usize, String) {
    let started = Instant::now();
    let listed = Libgit2.changed_files(root, at);
    let took = started.elapsed();

    match listed {
        ChangedFiles::Ready {
            files, truncated, ..
        } => (took, files.len(), format!("{truncated:?}")),
        other => panic!("expected a change list, got {other:?}"),
    }
}

/// How long one file's patch takes, and what came back.
fn time_patch(root: &Path, at: &DiffScope) -> (Duration, usize, usize, String) {
    let started = Instant::now();
    let found = Libgit2.file_diff(root, at, 0);
    let took = started.elapsed();

    match found {
        FileDiff::Ready {
            hunks, truncated, ..
        } => {
            let lines: usize = hunks.iter().map(|hunk| hunk.lines.len()).sum();
            let bytes: usize = hunks
                .iter()
                .flat_map(|hunk| hunk.lines.iter())
                .map(|line| line.text.len())
                .sum();
            (took, lines, bytes, format!("{truncated:?}"))
        }
        FileDiff::Binary { .. } => (took, 0, 0, "Binary".to_owned()),
        FileDiff::TooLarge { .. } => (took, 0, 0, "TooLarge".to_owned()),
        other => panic!("expected a patch, got {other:?}"),
    }
}

#[test]
#[ignore = "writes files of up to 32 MB; run it deliberately"]
fn a_diff_costs_what_the_limits_say_and_not_what_the_file_holds() {
    println!();
    println!("  subject                on disk     list      patch     lines    returned   state");
    println!("  ---------------------  ---------  --------  --------  -------  ---------  -----");

    let tiny = "one\ntwo\nthree\n".to_owned();
    let one_mb: String = (0..80_000).map(|n| format!("line {n:06}\n")).collect();
    let large: String = (0..600_000).map(|n| format!("line {n:06}\n")).collect();
    let wide: String = (0..20_000)
        .map(|_| format!("{}\n", "w".repeat(1_500)))
        .collect();
    // Under the file ceiling, so the *binary* path is what gets measured rather
    // than the size gate — which fires first for the larger one below.
    let binary_small: Vec<u8> = (0..1_500_000u32).map(|n| (n % 251) as u8).collect();
    let binary_large: Vec<u8> = (0..4_000_000u32).map(|n| (n % 251) as u8).collect();

    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("tiny text", tiny.into_bytes()),
        ("1 MB text", one_mb.into_bytes()),
        ("large text (~8 MB)", large.into_bytes()),
        ("wide lines (~30 MB)", wide.into_bytes()),
        ("binary, under ceiling", binary_small),
        ("binary, over ceiling", binary_large),
    ];

    for (name, body) in cases {
        let dir = TempDir::new().expect("tempdir");
        let head = repo_with_file(dir.path(), "subject.bin", &body);
        let at = scope(head);

        let (list, listed, list_state) = time_list(dir.path(), &at);
        let (patch, lines, bytes, state) = time_patch(dir.path(), &at);

        println!(
            "  {name:<21}  {:>7} KB  {:>6.2} ms  {:>6.2} ms  {lines:>7}  {:>7} KB  {state} / {list_state}",
            body.len() / 1024,
            list.as_secs_f64() * 1000.0,
            patch.as_secs_f64() * 1000.0,
            bytes / 1024,
        );

        assert_eq!(listed, 1, "one file changed, whatever its size");
        assert!(lines <= MAX_LINES, "{lines} lines exceeded the ceiling");
        assert!(
            bytes <= MAX_BYTES + 4_096,
            "{bytes} bytes exceeded the ceiling"
        );
    }

    println!();
    println!(
        "  limits: {MAX_FILES} files · {MAX_LINES} lines · {} KB patch · {} MB file",
        MAX_BYTES / 1024,
        MAX_FILE_BYTES / 1024 / 1024
    );
    println!();
}

#[test]
#[ignore = "builds commits of up to 4,000 files; run it deliberately"]
fn a_change_list_costs_what_the_limit_says_and_not_what_the_commit_holds() {
    println!();
    println!("  files changed   list      returned   state");
    println!("  -------------  --------  ---------  -----");

    let mut growth = Vec::new();

    for count in [10_usize, 500, 4_000] {
        let dir = TempDir::new().expect("tempdir");
        let head = repo_with_many(dir.path(), count);
        let (took, listed, state) = time_list(dir.path(), &scope(head));

        println!(
            "  {count:>13}  {:>6.2} ms  {listed:>9}  {state}",
            took.as_secs_f64() * 1000.0
        );

        assert!(listed <= MAX_FILES, "{listed} files exceeded the ceiling");
        growth.push(took.as_secs_f64());
    }

    println!();
    println!("  10 -> 4,000 changed files: x{:.1}", ratio(&growth));
    println!();

    // The list is capped, but building the diff still walks the trees, so this is
    // not flat — it is *sub-linear in the change set* and bounded in what it
    // returns and reads. The threshold is generous because this runs on whatever
    // machine is to hand; it exists to catch a return to unbounded work, which
    // would be an order of magnitude, not a few per cent.
    assert!(
        ratio(&growth) < 50.0,
        "a change list grew {:.1}x for a 400x commit; the read is no longer bounded",
        ratio(&growth)
    );
}

// ── File history ─────────────────────────────────────────────────────────────

/// A repository of `commits` where `subject.txt` changes every `every`-th one.
fn repo_with_sparse_file(dir: &Path, commits: usize, every: usize) -> Repository {
    let mut options = RepositoryInitOptions::new();
    options.initial_head("main");
    let repo = Repository::init_opts(dir, &options).expect("init");
    let who = Signature::new(
        "Blacknit",
        "blacknit@example.com",
        &git2::Time::new(1_700_000_000, 0),
    )
    .expect("signature");

    fs::create_dir_all(dir.join("src/deep/nested")).expect("dirs");
    let mut parent: Option<git2::Oid> = None;

    for n in 0..commits {
        if n % every == 0 {
            fs::write(dir.join("src/deep/nested/subject.txt"), format!("v{n}\n")).expect("write");
        }
        fs::write(dir.join("noise.txt"), format!("n{n}\n")).expect("write");
        parent = Some(commit_all(
            &repo,
            &who,
            &format!("c{n}"),
            &parent.into_iter().collect::<Vec<_>>(),
        ));
    }

    repo
}

/// The subject naming a path in the root commit's change set.
fn subject_in(root: &Path, commit: git2::Oid, path: &str) -> FileSubject {
    let scope = DiffScope::Commit {
        commit: commit.to_string().parse().expect("a commit id"),
    };
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

#[test]
#[ignore = "builds repositories of up to 20,000 commits; run it deliberately"]
fn a_file_trace_costs_what_its_budget_says_and_not_what_the_history_holds() {
    // The finding, in one table. An *unbounded* trace of one file is linear in
    // the repository — which is why there is a budget, and why the budget is
    // about commits examined rather than about anything to do with the file.
    println!();
    println!("  commits   found   scanned   page ms   stopped");
    println!("  -------   -----   -------   -------   -------");

    let mut growth = Vec::new();

    for commits in [1_000_usize, 5_000, 20_000] {
        let dir = TempDir::new().expect("tempdir");
        let repo = repo_with_sparse_file(dir.path(), commits, 50);
        let root = repo
            .revwalk()
            .and_then(|mut walk| {
                walk.set_sorting(Sort::NONE)?;
                walk.push_head()?;
                Ok(walk.last())
            })
            .expect("walk")
            .and_then(Result::ok)
            .expect("a root commit");
        let subject = subject_in(dir.path(), root, "src/deep/nested/subject.txt");

        let started = Instant::now();
        let traced = Libgit2.file_history(dir.path(), &subject, None);
        let took = started.elapsed();

        match traced {
            FileHistory::Ready {
                commits: found,
                scanned,
                stopped,
                ..
            } => {
                println!(
                    "  {commits:>7}   {:>5}   {scanned:>7}   {:>5.1} ms   {}",
                    found.len(),
                    took.as_secs_f64() * 1000.0,
                    match stopped {
                        ScanStopped::No => "reached the end".to_owned(),
                        ScanStopped::Budget { .. } => "budget".to_owned(),
                        ScanStopped::RenameLost { .. } => "rename lost".to_owned(),
                    },
                );
                assert!(found.len() <= PAGE, "a page is a page");
                assert!(
                    (scanned as usize) <= MAX_SCAN,
                    "examined {scanned}, past the ceiling of {MAX_SCAN}"
                );
            }
            other => panic!("expected a trace, got {other:?}"),
        }

        growth.push(took.as_secs_f64());
    }

    println!();
    println!(
        "  1,000 -> 20,000 commits: x{:.1}   (budget {MAX_SCAN} commits, page {PAGE})",
        ratio(&growth)
    );
    println!();

    // Twenty times the history must not cost twenty times the page. The
    // threshold is generous because this runs on whatever machine is to hand; it
    // catches a return to an unbounded walk, which is an order of magnitude.
    assert!(
        ratio(&growth) < 5.0,
        "a trace grew {:.1}x for a 20x repository; the walk is no longer bounded",
        ratio(&growth)
    );
}

#[test]
#[ignore = "builds a 20,000-commit repository; run it deliberately"]
fn an_unbounded_trace_would_be_linear_in_the_repository() {
    // The number the budget exists because of. This walks the *whole* history the
    // way `git log -- <path>` does, which is what Mira refuses to do in one
    // request — measured here so the refusal stays justified.
    println!();
    println!("  commits   full scan   per commit");
    println!("  -------   ---------   ----------");

    let mut growth = Vec::new();

    for commits in [1_000_usize, 5_000, 20_000] {
        let dir = TempDir::new().expect("tempdir");
        let repo = repo_with_sparse_file(dir.path(), commits, 50);

        let started = Instant::now();
        let mut walk = repo.revwalk().expect("revwalk");
        walk.set_sorting(Sort::NONE).expect("sorting");
        walk.push_head().expect("head");
        let subject = Path::new("src/deep/nested/subject.txt");
        let mut scanned = 0usize;
        for oid in walk {
            let Ok(oid) = oid else { break };
            scanned += 1;
            let found = repo.find_commit(oid).expect("commit");
            let new = found
                .tree()
                .ok()
                .and_then(|tree| tree.get_path(subject).ok())
                .map(|entry| entry.id());
            let old = found
                .parent(0)
                .ok()
                .and_then(|parent| parent.tree().ok())
                .and_then(|tree| tree.get_path(subject).ok())
                .map(|entry| entry.id());
            let _ = new != old;
        }
        let took = started.elapsed();

        println!(
            "  {commits:>7}   {:>6.1} ms   {:>7.1} us",
            took.as_secs_f64() * 1000.0,
            took.as_secs_f64() * 1_000_000.0 / scanned as f64,
        );
        growth.push(took.as_secs_f64());
    }

    println!();
    println!(
        "  1,000 -> 20,000 commits: x{:.1}  <- linear, which is why Mira bounds it",
        ratio(&growth)
    );
    println!();
}

// ── Filtering ────────────────────────────────────────────────────────────────

/// A repository with several authors, varied subjects, and a rare file.
fn repo_for_filtering(dir: &Path, commits: usize) -> Repository {
    let mut options = RepositoryInitOptions::new();
    options.initial_head("main");
    let repo = Repository::init_opts(dir, &options).expect("init");
    fs::create_dir_all(dir.join("src/deep/nested")).expect("dirs");

    let names = [
        "Ada Lovelace",
        "Grace Hopper",
        "Alan Turing",
        "Barbara Liskov",
    ];
    let mut parent: Option<git2::Oid> = None;

    for n in 0..commits {
        if n % 50 == 0 {
            fs::write(dir.join("src/deep/nested/subject.txt"), format!("v{n}\n")).expect("write");
        }
        fs::write(dir.join("noise.txt"), format!("n{n}\n")).expect("write");

        let who = Signature::new(
            names[n % names.len()],
            "a@b.c",
            &git2::Time::new(1_700_000_000 + n as i64, 0),
        )
        .expect("signature");
        let subject = format!(
            "change {n} to the {} module",
            if n % 3 == 0 { "widget" } else { "gadget" }
        );
        parent = Some(commit_all(
            &repo,
            &who,
            &subject,
            &parent.into_iter().collect::<Vec<_>>(),
        ));
    }

    repo
}

fn term(text: &str) -> Term {
    text.parse().expect("a filter term")
}

/// One filtered request: (ms, matched, examined, stopped early).
fn time_filter(root: &Path, filter: &HistoryFilter) -> (f64, usize, u32, bool) {
    let started = Instant::now();
    let found = Libgit2.filtered_history(root, filter, None);
    let took = started.elapsed().as_secs_f64() * 1000.0;

    match found {
        FilteredHistory::Ready {
            commits,
            scanned,
            stopped,
            ..
        } => (
            took,
            commits.len(),
            scanned,
            !matches!(stopped, ScanStopped::No),
        ),
        other => panic!("expected a filtered page, got {other:?}"),
    }
}

#[test]
#[ignore = "builds repositories of up to 10,000 commits; run it deliberately"]
fn every_filter_costs_what_the_walk_costs() {
    // The finding. Author, subject and file all land within noise of an
    // unfiltered walk, because loading the commit object is the expense and the
    // walk has already paid it — so one budget serves every filter, and ordering
    // the predicates cheapest-first would buy nothing.
    println!();
    println!("  commits   filter               matched  examined     ms   partial");
    println!("  -------   -------------------  -------  --------  -----   -------");

    let mut broad = Vec::new();
    let mut selective = Vec::new();

    for commits in [100_usize, 1_000, 10_000] {
        let dir = TempDir::new().expect("tempdir");
        let repo = repo_for_filtering(dir.path(), commits);
        let head = repo.head().expect("head").target().expect("target");
        let root = repo
            .revwalk()
            .and_then(|mut walk| {
                walk.set_sorting(Sort::NONE)?;
                walk.push_head()?;
                Ok(walk.last())
            })
            .expect("walk")
            .and_then(Result::ok)
            .expect("root");

        let file = subject_in(dir.path(), root, "src/deep/nested/subject.txt");
        let tip: CommitId = head.to_string().parse().expect("a commit id");

        let cases: Vec<(&str, HistoryFilter)> = vec![
            (
                "branch only",
                HistoryFilter {
                    branch: Some(tip.clone()),
                    ..HistoryFilter::default()
                },
            ),
            (
                "author (broad)",
                HistoryFilter {
                    author: Some(term("Grace")),
                    ..HistoryFilter::default()
                },
            ),
            (
                "subject (broad)",
                HistoryFilter {
                    subject: Some(term("widget")),
                    ..HistoryFilter::default()
                },
            ),
            (
                "file (selective)",
                HistoryFilter {
                    file: Some(file.clone()),
                    ..HistoryFilter::default()
                },
            ),
            (
                "all four",
                HistoryFilter {
                    branch: Some(tip),
                    author: Some(term("Grace")),
                    subject: Some(term("widget")),
                    file: Some(file),
                },
            ),
            (
                "no match at all",
                HistoryFilter {
                    subject: Some(term("marzipan")),
                    ..HistoryFilter::default()
                },
            ),
        ];

        for (name, filter) in cases {
            let (ms, matched, examined, partial) = time_filter(dir.path(), &filter);
            println!(
                "  {commits:>7}   {name:<19}  {matched:>7}  {examined:>8}  {ms:>5.1}   {}",
                if partial { "yes" } else { "no" }
            );

            assert!(matched <= PAGE, "a page is a page");
            assert!(
                (examined as usize) <= MAX_FILTER_SCAN,
                "examined {examined}, past the ceiling of {MAX_FILTER_SCAN}"
            );

            if name == "author (broad)" {
                broad.push(ms);
            }
            if name == "no match at all" {
                selective.push(ms);
            }
        }
        println!();
    }

    println!(
        "  100 -> 10,000 commits:  broad filter x{:.1}   no-match filter x{:.1}   (budget {MAX_FILTER_SCAN})",
        ratio(&broad),
        ratio(&selective),
    );
    println!();

    // A broad filter fills a page early, so it barely grows. A no-match filter
    // spends the whole budget every time — which is the point of having one, and
    // is why it plateaus instead of climbing with the repository.
    assert!(
        ratio(&broad) < 5.0,
        "a broad filter grew {:.1}x for a 100x repository",
        ratio(&broad)
    );
    assert!(
        ratio(&selective) < 40.0,
        "a no-match filter grew {:.1}x; it should plateau at the budget",
        ratio(&selective)
    );
}

#[test]
#[ignore = "builds a 10,000-commit repository; run it deliberately"]
fn offering_authors_to_choose_from_is_bounded_too() {
    println!();
    println!("  commits   authors  examined     ms   partial");
    println!("  -------   -------  --------  -----   -------");

    for commits in [100_usize, 1_000, 10_000] {
        let dir = TempDir::new().expect("tempdir");
        repo_for_filtering(dir.path(), commits);

        let started = Instant::now();
        let listed = Libgit2.known_authors(dir.path(), None);
        let ms = started.elapsed().as_secs_f64() * 1000.0;

        match listed {
            mira_git::KnownAuthors::Ready {
                authors,
                scanned,
                stopped,
            } => {
                println!(
                    "  {commits:>7}   {:>7}  {scanned:>8}  {ms:>5.1}   {}",
                    authors.len(),
                    if matches!(stopped, ScanStopped::No) {
                        "no"
                    } else {
                        "yes"
                    }
                );
                assert!((scanned as usize) <= MAX_FILTER_SCAN);
            }
            other => panic!("expected an author list, got {other:?}"),
        }
    }
    println!();
}
