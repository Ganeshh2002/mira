//! Process facts.
//!
//! Every test here asks about **this** process. That is deliberate: the brief for
//! this slice forbids tests that depend on whatever happens to be running on the
//! developer's machine, and the test runner is the one process guaranteed to
//! exist, to have a working directory, and to have a parent.

use std::collections::HashSet;

use mira_processes::{ProcessProvider, Processes};

fn me() -> u32 {
    std::process::id()
}

#[test]
fn a_process_reports_its_own_identity() {
    let facts = Processes::new().facts_for(&[me()]);
    let mine = facts.first().expect("this process exists");

    assert_eq!(mine.pid, me());
    assert!(!mine.name.is_empty(), "a process always has a name");
}

#[test]
fn the_executable_path_is_reported_where_the_platform_allows_it() {
    let facts = Processes::new().facts_for(&[me()]);
    let mine = facts.first().expect("this process exists");

    // macOS and Linux both expose this for a process the user owns. It is
    // `Option` because Windows can refuse, and Mira says so rather than guessing.
    let executable = mine.executable.as_deref().expect("our own executable path");
    assert!(
        executable.contains("facts") || executable.contains("mira"),
        "the path points at the test binary: {executable}"
    );
}

#[test]
fn the_working_directory_is_reported_where_the_platform_allows_it() {
    // This is the fact project attribution is built on. Windows does not expose
    // another process's working directory at all, which is why the whole
    // attribution path treats it as optional rather than assumed.
    let facts = Processes::new().facts_for(&[me()]);
    let mine = facts.first().expect("this process exists");

    let cwd = mine
        .working_directory
        .as_deref()
        .expect("our own working directory");
    assert!(
        std::path::Path::new(cwd).is_absolute(),
        "attribution compares canonical absolute paths: {cwd}"
    );
}

#[test]
fn a_process_reports_its_parent() {
    let facts = Processes::new().facts_for(&[me()]);
    let mine = facts.first().expect("this process exists");

    let parent = mine
        .parent
        .expect("the test runner was started by something");
    assert_ne!(parent, me());
}

#[test]
fn a_pid_that_is_not_running_is_simply_absent() {
    // Sockets outlive processes by moments. A pid that has already exited is an
    // ordinary outcome, not a failure worth reporting.
    let facts = Processes::new().facts_for(&[u32::MAX]);

    assert!(facts.is_empty());
}

#[test]
fn only_the_processes_asked_about_come_back() {
    // Mira is not an activity monitor (slice brief §5). The provider takes the
    // pids that own listening sockets and answers about those; there is no method
    // that returns the whole process table, so there is nothing to leak.
    let facts = Processes::new().facts_for(&[me()]);

    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].pid, me());
}

#[test]
fn asking_about_nothing_reads_nothing() {
    assert!(Processes::new().facts_for(&[]).is_empty());
}

#[test]
fn a_pid_asked_for_twice_comes_back_once() {
    let facts = Processes::new().facts_for(&[me(), me(), me()]);

    let unique: HashSet<u32> = facts.iter().map(|fact| fact.pid).collect();
    assert_eq!(facts.len(), unique.len());
}
