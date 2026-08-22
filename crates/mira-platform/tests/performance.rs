//! What discovery costs, measured before it was made bigger.
//!
//! Run with `cargo test -p mira-platform --release --test performance -- --ignored
//! --nocapture`. Ignored by default: these are measurements, not assertions, and a
//! number that depends on how many entries are on this machine's `PATH` is not a
//! thing to fail a build on.
//!
//! The question this file exists to answer: **the Context panel asks "is there an
//! editor" and stops at the first one; a chooser has to ask about every row.** That
//! is a different amount of work, and how much more decides whether a chooser can
//! probe on demand or needs something kept between requests.

use std::time::Instant;

use mira_core::AppKind;
use mira_platform::{candidates, first_present, probe_for, Applications, Os};

const EVERY_OS: [Os; 3] = [Os::MacOs, Os::Windows, Os::Linux];

/// Probe every candidate rather than stopping at the first, the way a chooser must.
fn probe_all(os: Os, kind: AppKind) -> usize {
    candidates(os, kind)
        .iter()
        .filter(|candidate| probe_for(os, candidate))
        .count()
}

#[test]
#[ignore = "measurement, not an assertion"]
fn asking_about_every_application_costs_what_asking_about_one_costs() {
    let path = std::env::var("PATH").unwrap_or_default();
    println!(
        "\n  PATH has {} entries\n",
        std::env::split_paths(&path).count()
    );

    println!("  os       kind      rows  first-present   whole-list   installed");
    println!("  -------  --------  ----  -------------   ----------   ---------");

    for os in EVERY_OS {
        for kind in AppKind::ALL {
            let rows = candidates(os, kind).len();

            let started = Instant::now();
            for _ in 0..100 {
                let _ = first_present(candidates(os, kind), |candidate| probe_for(os, candidate));
            }
            let first = started.elapsed().as_secs_f64() * 1000.0 / 100.0;

            let started = Instant::now();
            let mut installed = 0;
            for _ in 0..100 {
                installed = probe_all(os, kind);
            }
            let whole = started.elapsed().as_secs_f64() * 1000.0 / 100.0;

            println!(
                "  {os:<7?}  {kind:<8?}  {rows:>4}  {first:>10.3} ms  {whole:>8.3} ms   {installed}"
            );
        }
    }
}

#[test]
#[ignore = "measurement, not an assertion"]
fn the_whole_catalogue_at_once_is_what_the_chooser_asks_for() {
    // Three kinds, every row, which is the most a single request can ask for.
    for os in EVERY_OS {
        let started = Instant::now();
        for _ in 0..50 {
            for kind in AppKind::ALL {
                let _ = probe_all(os, kind);
            }
        }
        let each = started.elapsed().as_secs_f64() * 1000.0 / 50.0;

        let rows: usize = AppKind::ALL
            .into_iter()
            .map(|kind| candidates(os, kind).len())
            .sum();

        println!("  {os:?}: {rows} rows, whole catalogue {each:.3} ms");
    }

    // And what the panel asks for today, for comparison.
    let started = Instant::now();
    for _ in 0..50 {
        let _ = Applications::detect().survey();
    }
    println!(
        "  this machine: survey (first-present, three kinds) {:.3} ms",
        started.elapsed().as_secs_f64() * 1000.0 / 50.0
    );
}

#[test]
#[ignore = "measurement, not an assertion"]
fn what_gathering_support_for_a_workspaces_actions_costs() {
    // Slice 4d's design question. Resolving six actions costs 0.65 µs once the
    // machine's answers are in hand (`mira-core`'s benchmark). What those
    // answers cost is the number that decides whether they are gathered once per
    // request or once per action.
    use std::time::Instant;

    let os = mira_platform::env::EnvFacts::detect().os;
    let apps = mira_platform::Applications::for_os(os);
    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));

    const ROUNDS: u32 = 500;

    let started = Instant::now();
    for _ in 0..ROUNDS {
        std::hint::black_box(apps.openable());
    }
    let openable = started.elapsed() / ROUNDS;

    let started = Instant::now();
    for _ in 0..ROUNDS {
        std::hint::black_box(here.is_dir());
    }
    let stat = started.elapsed() / ROUNDS;

    println!();
    println!("  openable() — which kinds can open a folder   {openable:?}");
    println!("  is_dir()   — is the project folder there      {stat:?}");
    println!(
        "  gathered once per request                     {:?}",
        openable + stat
    );
    println!(
        "  asked once per action, six actions            {:?}",
        (openable + stat) * 6
    );
    println!();
}
