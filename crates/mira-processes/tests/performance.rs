//! What reading process facts costs, and what CPU% actually requires.
//!
//! Slice 2b's design turns on one question that cannot be answered from the
//! documentation alone: **`sysinfo` computes CPU share from the delta between
//! two refreshes of the same `System`**, so a provider that builds a fresh one
//! per call can only ever report zero. This file measures that, finds the
//! interval at which the number becomes meaningful, and prices the alternatives.
//!
//! Ignored by default — wall-clock thresholds in CI are flake generators. Run:
//!
//! ```text
//! cargo test -p mira-processes --release --test performance -- --ignored --nocapture
//! ```

use std::time::{Duration, Instant};

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

/// The pids of a realistic sample: whatever is listening on this machine, plus
/// this test process, so the benchmark is never empty.
fn sample_pids(count: usize) -> Vec<Pid> {
    let mut system = System::new();
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    let mut pids: Vec<Pid> = system.processes().keys().copied().collect();
    pids.sort_unstable();
    pids.truncate(count);
    if pids.is_empty() {
        pids.push(Pid::from_u32(std::process::id()));
    }
    pids
}

/// The refresh Mira does today: executable and working directory only.
fn today() -> ProcessRefreshKind {
    ProcessRefreshKind::nothing()
        .with_exe(UpdateKind::Always)
        .with_cwd(UpdateKind::Always)
}

/// The refresh slice 2b needs: the same, plus cpu and memory.
fn with_detail() -> ProcessRefreshKind {
    today().with_cpu().with_memory()
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
fn what_a_process_read_costs_today_and_with_detail() {
    println!();
    println!("  pids   exe+cwd (today)   + cpu & memory      delta");
    println!("  ----   ---------------   --------------   --------");

    for &count in &[1_usize, 4, 16, 64] {
        let pids = sample_pids(count);

        let before = time(40, || {
            let mut system = System::new();
            system.refresh_processes_specifics(ProcessesToUpdate::Some(&pids), true, today());
            std::hint::black_box(system.processes().len());
        });

        let after = time(40, || {
            let mut system = System::new();
            system.refresh_processes_specifics(ProcessesToUpdate::Some(&pids), true, with_detail());
            std::hint::black_box(system.processes().len());
        });

        println!(
            "  {:>4}   {:>15}   {:>14}   {:>8}",
            pids.len(),
            format!("{:.3} ms", before.as_secs_f64() * 1e3),
            format!("{:.3} ms", after.as_secs_f64() * 1e3),
            format!(
                "{:+.3} ms",
                (after.as_secs_f64() - before.as_secs_f64()) * 1e3
            ),
        );
    }
    println!();
}

#[test]
#[ignore = "measurement, not an assertion"]
fn a_fresh_system_can_never_report_cpu_share() {
    // The finding that decides the design, measured against a process that is
    // provably burning a core. One refresh has no previous reading to subtract,
    // so the answer is 0.00% however busy the process actually is.
    let me = Pid::from_u32(std::process::id());
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let spinning = {
        let stop = std::sync::Arc::clone(&stop);
        std::thread::spawn(move || {
            let mut x: u64 = 0;
            while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                x = x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
                std::hint::black_box(x);
            }
        })
    };
    std::thread::sleep(Duration::from_millis(500));

    let mut fresh = System::new();
    fresh.refresh_processes_specifics(ProcessesToUpdate::Some(&[me]), true, with_detail());
    let once = fresh.process(me).map_or(0.0, sysinfo::Process::cpu_usage);

    let mut kept = System::new();
    kept.refresh_processes_specifics(ProcessesToUpdate::Some(&[me]), true, with_detail());
    std::thread::sleep(Duration::from_millis(500));
    kept.refresh_processes_specifics(ProcessesToUpdate::Some(&[me]), true, with_detail());
    let twice = kept.process(me).map_or(0.0, sysinfo::Process::cpu_usage);

    println!();
    println!("  a process burning one core reads:");
    println!("    one refresh of a fresh System    {once:.2}%   <- unusable");
    println!("    second refresh of a kept System  {twice:.2}%   <- the real figure");
    println!();

    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    spinning.join().expect("join");
}

#[test]
#[ignore = "measurement, not an assertion"]
fn how_long_a_second_sample_must_wait_to_mean_anything() {
    // Measured against a process that is definitely busy — this one, with a
    // thread spinning. The first attempt at this benchmark sampled the lowest
    // two dozen pids on the machine, which on macOS are idle system daemons, and
    // reported 0% everywhere. That looked like the mechanism failing and was
    // actually the sample being wrong; a benchmark that cannot tell those apart
    // is worse than none.
    let me = Pid::from_u32(std::process::id());
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let spinning = {
        let stop = std::sync::Arc::clone(&stop);
        std::thread::spawn(move || {
            let mut x: u64 = 0;
            while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                x = x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
                std::hint::black_box(x);
            }
        })
    };

    println!();
    println!(
        "  sysinfo MINIMUM_CPU_UPDATE_INTERVAL = {:?}",
        sysinfo::MINIMUM_CPU_UPDATE_INTERVAL
    );
    println!();
    println!("    gap    cpu of a busy process   second refresh");
    println!("  -----   ---------------------   --------------");

    let mut system = System::new();
    system.refresh_processes_specifics(ProcessesToUpdate::Some(&[me]), true, with_detail());

    for gap in [
        Duration::from_millis(200),
        Duration::from_millis(500),
        Duration::from_secs(1),
        Duration::from_secs(5),
    ] {
        std::thread::sleep(gap);
        let started = Instant::now();
        system.refresh_processes_specifics(ProcessesToUpdate::Some(&[me]), true, with_detail());
        let cost = started.elapsed();
        let share = system.process(me).map_or(0.0, sysinfo::Process::cpu_usage);

        println!(
            "  {:>5}   {:>21}   {:>14}",
            format!("{gap:?}"),
            format!("{share:.2}%"),
            format!("{:.3} ms", cost.as_secs_f64() * 1e3),
        );
    }

    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    spinning.join().expect("join");
    println!();
}

#[test]
#[ignore = "measurement, not an assertion"]
fn keeping_one_system_across_ticks_versus_building_one_each_time() {
    // The two candidate implementations, priced. Reusing one `System` is the
    // only one that can report CPU at all; this says what it costs.
    let pids = sample_pids(24);

    let fresh_each = time(30, || {
        let mut system = System::new();
        system.refresh_processes_specifics(ProcessesToUpdate::Some(&pids), true, with_detail());
        std::hint::black_box(system.processes().len());
    });

    let mut kept = System::new();
    kept.refresh_processes_specifics(ProcessesToUpdate::Some(&pids), true, with_detail());
    let reused = time(30, || {
        kept.refresh_processes_specifics(ProcessesToUpdate::Some(&pids), true, with_detail());
        std::hint::black_box(kept.processes().len());
    });

    println!();
    println!(
        "  building a System each tick   {:.3} ms   (cpu always 0)",
        fresh_each.as_secs_f64() * 1e3
    );
    println!(
        "  refreshing one kept System    {:.3} ms   (cpu meaningful from the second tick)",
        reused.as_secs_f64() * 1e3
    );
    println!();
}
