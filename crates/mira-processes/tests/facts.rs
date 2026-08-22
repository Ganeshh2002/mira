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

// ── Detail: cpu, memory, uptime (slice 2b) ───────────────────────────────────

#[test]
fn memory_and_uptime_are_reported_from_the_first_reading() {
    // Neither is a rate, so neither needs a previous sample. A process that
    // exists has occupied memory and has been running for some time.
    let facts = Processes::new().facts_for(&[me()]);
    let mine = facts.first().expect("this process exists");

    assert!(
        mine.memory_bytes.is_some_and(|bytes| bytes > 0),
        "a running process occupies memory: {:?}",
        mine.memory_bytes
    );
    assert!(
        mine.uptime_seconds.is_some(),
        "a running process has been running: {:?}",
        mine.uptime_seconds
    );
}

#[test]
fn cpu_share_is_absent_on_a_first_reading_rather_than_zero() {
    // The distinction the whole design turns on. A share is a *rate*, and a rate
    // needs two samples — there is nothing to subtract from the first, so
    // reporting `0.0` would call a process idle that might be burning a core.
    // Measured: a process spinning a full core reads 0.00% from a fresh reading
    // ([ADR-0022](../../../docs/adr/0022-process-detail.md)).
    let facts = Processes::new().facts_for(&[me()]);
    let mine = facts.first().expect("this process exists");

    assert_eq!(
        mine.cpu_share, None,
        "the first reading has no previous one to compare against"
    );
}

#[test]
fn cpu_share_arrives_once_there_are_two_readings_to_compare() {
    // The same provider, asked twice. This is exactly what the observer does on
    // consecutive ticks, and it is why the provider is kept rather than rebuilt.
    let processes = Processes::new();

    assert_eq!(processes.facts_for(&[me()])[0].cpu_share, None);

    let second = processes.facts_for(&[me()]);
    let mine = second.first().expect("this process exists");

    assert!(
        mine.cpu_share.is_some(),
        "a second reading has something to compare against"
    );
    assert!(
        mine.cpu_share.is_some_and(|share| share >= 0.0),
        "a share is never negative: {:?}",
        mine.cpu_share
    );
}

#[test]
fn a_second_provider_starts_over_rather_than_inheriting_a_reading() {
    // The property that makes the kept provider necessary: a fresh one has no
    // history, so it reports absence rather than borrowing somebody else's
    // sample.
    let first = Processes::new();
    let _ = first.facts_for(&[me()]);
    assert!(first.facts_for(&[me()])[0].cpu_share.is_some());

    assert_eq!(
        Processes::new().facts_for(&[me()])[0].cpu_share,
        None,
        "a new provider has measured nothing yet"
    );
}

#[test]
fn each_process_is_tracked_separately_for_its_first_reading() {
    // Asking about a new pid on a later tick must still report absence for that
    // pid, not inherit "we have sampled before" from the provider as a whole.
    let processes = Processes::new();
    let _ = processes.facts_for(&[me()]);
    let _ = processes.facts_for(&[me()]);

    // The parent is a pid this provider has never been asked about.
    let parent = processes.facts_for(&[me()])[0].parent;
    let Some(parent) = parent else {
        return; // No parent exposed on this platform; nothing to assert.
    };

    let first_sight = processes.facts_for(&[parent]);
    if let Some(theirs) = first_sight.first() {
        assert_eq!(
            theirs.cpu_share, None,
            "a pid seen for the first time has no share yet, whatever else has been sampled"
        );
    }
}

#[test]
fn a_process_that_is_not_there_is_absent_rather_than_invented() {
    // A socket can outlive its process by moments. The honest answer is a
    // shorter list, never a row of zeroes.
    let processes = Processes::new();
    // Pid 0 is not a process a user can read on any platform Mira supports, and
    // u32::MAX is not a pid at all.
    let facts = processes.facts_for(&[0, u32::MAX]);

    assert!(
        facts.iter().all(|fact| fact.pid != u32::MAX),
        "a pid that does not exist produced a row: {facts:?}"
    );
}

#[test]
fn no_fact_carries_a_command_line() {
    // Serialised, because that is the form that reaches the interface. A
    // credential in argv cannot leak through a field that does not exist
    // (ADR-0022).
    let facts = Processes::new().facts_for(&[me()]);
    let json = serde_json::to_value(&facts[0]).expect("serialise");
    let object = json.as_object().expect("an object");

    assert_eq!(object.len(), 8, "got {object:#?}");
    for absent in ["cmd", "argv", "args", "commandLine", "environ"] {
        assert!(
            !object.contains_key(absent),
            "{absent} is present: {object:#?}"
        );
    }
}
