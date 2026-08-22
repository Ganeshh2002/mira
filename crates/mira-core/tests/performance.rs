//! What resolving a workspace's services costs, measured rather than assumed.
//!
//! `roadmap.md` rule 5 asks for budgets every slice, and slice 4c's brief asks
//! for the data model to be **measured before it is chosen**. There were two
//! candidate shapes, and the difference between them is only visible on a
//! machine that is actually busy:
//!
//! - **scan** — for each watched service, walk the observed listeners looking for
//!   its port. O(watched × listening), no allocation.
//! - **index** — build one map of the observed listeners, then look each watched
//!   service up. O(watched + listening), one allocation.
//!
//! The scan wins while a machine has a handful of sockets, which is every
//! developer laptop at rest and no container host under load. The question this
//! benchmark answers is *where it stops winning*, and whether that crossover is
//! somewhere a real machine goes.
//!
//! Ignored by default — a wall-clock threshold in CI is a flake generator. Run it
//! deliberately:
//!
//! ```text
//! cargo test -p mira-core --release --test performance -- --ignored --nocapture
//! ```

use std::collections::HashMap;
use std::time::{Duration, Instant};

use mira_core::service::{Listening, Observed, Port, WatchedService};
use mira_core::{resolve, ProjectId, WorkspaceId, WorkspaceServiceId};

/// How many times each measurement is repeated before it is believed.
const ROUNDS: u32 = 2_000;

fn listeners(count: usize, project: ProjectId) -> Vec<Listening> {
    (0..count)
        .map(|n| Listening {
            // Spread across the ephemeral range so the ports are not contiguous
            // with the watched set: a lookup that misses is the common case.
            port: Port::try_from(1_024 + (n as u16).wrapping_mul(7) % 60_000 + 1).expect("port"),
            project_id: if n % 3 == 0 { Some(project) } else { None },
            address: "127.0.0.1".to_owned(),
            process: Some("node".to_owned()),
            pid: Some(1_000 + n as u32),
        })
        .collect()
}

fn watched(count: usize, workspace: WorkspaceId) -> Vec<WatchedService> {
    (0..count)
        .map(|n| WatchedService {
            id: WorkspaceServiceId::new(n as i64 + 1),
            workspace_id: workspace,
            port: Port::try_from(3_000 + n as u16).expect("port"),
            added_at: 1_800_000_000,
        })
        .collect()
}

/// The shape that was not chosen, kept so the comparison can be re-run.
fn by_scan(watched: &[WatchedService], listening: &[Listening]) -> usize {
    watched
        .iter()
        .filter(|service| {
            listening
                .iter()
                .any(|listener| listener.port == service.port)
        })
        .count()
}

/// The shape that was chosen, in isolation from the rest of `resolve`.
fn by_index(watched: &[WatchedService], listening: &[Listening]) -> usize {
    let index: HashMap<Port, &Listening> = listening
        .iter()
        .map(|listener| (listener.port, listener))
        .collect();
    watched
        .iter()
        .filter(|service| index.contains_key(&service.port))
        .count()
}

fn time(mut body: impl FnMut()) -> Duration {
    let started = Instant::now();
    for _ in 0..ROUNDS {
        body();
    }
    started.elapsed() / ROUNDS
}

#[test]
#[ignore = "measurement, not an assertion"]
fn scanning_and_indexing_the_observed_listeners() {
    let project = ProjectId::new(1);
    let workspace = WorkspaceId::new(1);

    println!();
    println!("  watched   listening        scan       index");
    println!("  -------   ---------   ---------   ---------");

    for &watch_count in &[1_usize, 4, 20] {
        let services = watched(watch_count, workspace);

        for &listen_count in &[8_usize, 64, 512, 4_096] {
            let observed = listeners(listen_count, project);

            let scan = time(|| {
                std::hint::black_box(by_scan(&services, &observed));
            });
            let index = time(|| {
                std::hint::black_box(by_index(&services, &observed));
            });

            println!(
                "  {watch_count:>7}   {listen_count:>9}   {:>9}   {:>9}",
                format!("{:.3} us", scan.as_secs_f64() * 1e6),
                format!("{:.3} us", index.as_secs_f64() * 1e6),
            );
        }
    }
    println!();
}

#[test]
#[ignore = "measurement, not an assertion"]
fn resolving_a_whole_workspace() {
    let project = ProjectId::new(1);
    let workspace = WorkspaceId::new(1);

    println!();
    println!("  watched   listening      resolve   fifty workspaces");
    println!("  -------   ---------   ----------   ----------------");

    for &(watch_count, listen_count) in &[
        (1_usize, 8_usize),
        (4, 64),
        (4, 512),
        (20, 512),
        (20, 4_096),
    ] {
        let services = watched(watch_count, workspace);
        let observed = listeners(listen_count, project);

        let once = time(|| {
            std::hint::black_box(resolve(&services, project, Observed::Seen(&observed)));
        });

        println!(
            "  {watch_count:>7}   {listen_count:>9}   {:>10}   {:>16}",
            format!("{:.3} us", once.as_secs_f64() * 1e6),
            format!("{:.3} ms", once.as_secs_f64() * 1e3 * 50.0),
        );
    }
    println!();
}

// ── Workspace actions (slice 4d) ─────────────────────────────────────────────

/// A workspace's chosen actions, in catalogue order.
fn chosen(count: usize) -> Vec<mira_core::ActionId> {
    mira_core::CATALOGUE
        .iter()
        .take(count)
        .map(|action| action.id.parse().expect("a catalogue id"))
        .collect()
}

fn support(running: usize) -> mira_core::Support {
    mira_core::Support {
        openable: vec![
            (mira_core::AppKind::Editor, "Visual Studio Code".to_owned()),
            (mira_core::AppKind::Terminal, "Terminal".to_owned()),
        ],
        folder_exists: true,
        reveal: None,
        running: (0..running).map(|n| format!(":{}", 3_000 + n)).collect(),
    }
}

#[test]
#[ignore = "measurement, not an assertion"]
fn resolving_a_workspaces_actions() {
    // The design question this answers: is `Support` worth gathering once per
    // request, or should each action ask the machine for itself?
    //
    // The answer only means something next to what asking the machine costs.
    // `Applications::openable()` was measured at 6.9 µs on this machine in slice
    // 4b, and a `Path::is_dir()` at roughly 2 µs; the numbers below are what
    // deciding costs once those are in hand.
    println!();
    println!("  actions   running       resolve   fifty workspaces");
    println!("  -------   -------   -----------   ----------------");

    for &(action_count, running) in &[(1_usize, 1_usize), (3, 1), (6, 0), (6, 1), (6, 3), (6, 40)] {
        let actions = chosen(action_count);
        let machine = support(running);

        let once = time(|| {
            std::hint::black_box(mira_core::action::resolve(&actions, &machine));
        });

        println!(
            "  {action_count:>7}   {running:>7}   {:>11}   {:>16}",
            format!("{:.3} us", once.as_secs_f64() * 1e6),
            format!("{:.3} ms", once.as_secs_f64() * 1e3 * 50.0),
        );
    }
    println!();
}

#[test]
#[ignore = "measurement, not an assertion"]
fn looking_one_action_up_in_the_catalogue() {
    // `find` is a linear walk of a six-row array. The question is whether the
    // catalogue needs an index — and, more usefully, what the cost per row is,
    // so the answer stays true as the catalogue grows.
    let known: mira_core::ActionId = "refresh".parse().expect("a catalogue id");
    let unknown: mira_core::ActionId = "not-an-action".parse().expect("a well-formed id");

    let hit = time(|| {
        std::hint::black_box(mira_core::action::find(&known));
    });
    let miss = time(|| {
        std::hint::black_box(mira_core::action::find(&unknown));
    });

    println!();
    println!("  catalogue rows            {}", mira_core::CATALOGUE.len());
    println!(
        "  find, last row            {:.4} us",
        hit.as_secs_f64() * 1e6
    );
    println!(
        "  find, no row (full walk)  {:.4} us",
        miss.as_secs_f64() * 1e6
    );
    println!();
}
