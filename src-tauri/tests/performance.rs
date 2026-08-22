//! What the machine-wide Ports view costs to arrange.
//!
//! The reading itself is the scheduler's and is already paid for — slice 4c
//! measured the socket scan at 2.5 ms and attribution at 1.0 ms on this machine.
//! What slice 2b adds is a *grouping* over that reading, and the question is
//! whether it is negligible beside the read or worth caching.
//!
//! Run:
//!
//! ```text
//! cargo test -p mira --release --test performance -- --ignored --nocapture
//! ```

use std::time::{Duration, Instant};

use mira_core::ProjectId;
use mira_ports::{Attribution, Listener, Unattributed};
use mira_processes::ProcessFacts;

use mira_lib::commands::ports::group;
use mira_lib::live::{Service, ServiceObservation};

fn facts(pid: u32) -> ProcessFacts {
    ProcessFacts {
        pid,
        name: "node".to_owned(),
        executable: Some("/usr/local/bin/node".to_owned()),
        parent: Some(1),
        working_directory: Some("/home/dev/aviora".to_owned()),
        cpu_share: Some(2.5),
        memory_bytes: Some(180 * 1024 * 1024),
        uptime_seconds: Some(3_600),
    }
}

/// An observation of `count` listeners spread over `projects` projects, with one
/// in five unattributable — roughly what a developer machine looks like.
fn observation(count: usize, projects: usize) -> ServiceObservation {
    let services = (0..count)
        .map(|n| Service {
            listener: Listener {
                port: 1_024 + u16::try_from(n % 60_000).unwrap_or(0),
                local_address: "127.0.0.1".to_owned(),
                pid: Some(1_000 + u32::try_from(n).unwrap_or(0)),
            },
            process: Some(facts(1_000 + u32::try_from(n).unwrap_or(0))),
            attribution: if n % 5 == 0 {
                Attribution::Unattributed {
                    reason: Unattributed::OutsideEveryProject,
                }
            } else {
                Attribution::Project {
                    project_id: ProjectId::new((n % projects.max(1)) as i64 + 1),
                    package: None,
                }
            },
        })
        .collect();

    ServiceObservation {
        services,
        error: None,
        observed_at: Some(1_800_000_000),
    }
}

fn named(projects: usize) -> Vec<(ProjectId, String)> {
    (1..=projects)
        .map(|n| (ProjectId::new(n as i64), format!("Project {n}")))
        .collect()
}

fn time(rounds: u32, mut body: impl FnMut()) -> Duration {
    let started = Instant::now();
    for _ in 0..rounds {
        body();
    }
    started.elapsed() / rounds
}

#[test]
#[ignore = "measurement, not an assertion"]
fn arranging_the_machine_wide_ports_view() {
    println!();
    println!("  listeners   projects      group   vs the 3.5 ms read it arranges");
    println!("  ---------   --------   --------   -----------------------------");

    for &(count, projects) in &[
        (13_usize, 3_usize), // this machine, measured in slice 4c
        (64, 5),
        (256, 10),
        (1_024, 20),
        (4_096, 40),
    ] {
        let observed = observation(count, projects);
        let names = named(projects);

        let cost = time(200, || {
            std::hint::black_box(group(&observed, &names));
        });

        println!(
            "  {count:>9}   {projects:>8}   {:>8}   {:>29}",
            format!("{:.3} ms", cost.as_secs_f64() * 1e3),
            format!("{:.2}% of the read", cost.as_secs_f64() * 1e3 / 3.5 * 100.0),
        );
    }
    println!();
}

#[test]
#[ignore = "measurement, not an assertion"]
fn the_grouping_is_linear_in_listeners_not_quadratic() {
    // The shape that matters. Finding a project group is a linear scan of the
    // groups found so far, which is O(listeners x projects) in the worst case —
    // this says whether that worst case is anywhere a machine goes.
    println!();
    println!("  a machine with many projects and many listeners:");
    for &(count, projects) in &[(1_024_usize, 1_usize), (1_024, 50), (1_024, 200)] {
        let observed = observation(count, projects);
        let names = named(projects);
        let cost = time(100, || {
            std::hint::black_box(group(&observed, &names));
        });
        println!(
            "    {count} listeners over {projects:>3} projects   {:.3} ms",
            cost.as_secs_f64() * 1e3
        );
    }
    println!();
}
