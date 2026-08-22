//! Read-only process facts.
//!
//! Mira is not an activity monitor. There is deliberately **no method that
//! returns the process table**: the provider answers about the pids it is given,
//! which in practice are the pids that own listening sockets. A feature that
//! wanted the whole list would have to add the method and argue for it.
//!
//! Nothing here can stop, signal, or otherwise touch a process. Termination is a
//! later slice with its own security and confirmation design, and its absence
//! from this interface is what makes that ordering real rather than intended.
//!
//! Facts come from `sysinfo`, which reads them through each platform's native
//! interfaces — `libproc` on macOS, `/proc` on Linux, the Windows process APIs.
//! No command is run and no human-readable output is parsed
//! (`security-and-privacy.md` §5).
//!
//! # The command line is deliberately absent
//!
//! `roadmap.md` slice 2 lists argv under process detail, and slice 2b declines to
//! build it. A process's command line routinely carries credentials —
//! `--password=`, `PGPASSWORD=`, a token inside a `DATABASE_URL`, an API key a
//! task runner passed down. Mira showing it would put those on screen, into any
//! screenshot, and in front of anyone glancing at the window.
//!
//! Redaction was considered and rejected: it is a blocklist, and blocklists leak.
//! So there is no field for it on [`ProcessFacts`], nothing here calls
//! `Process::cmd()`, and a guard test fails the build if anything starts to
//! ([ADR-0022](../../../docs/adr/0022-process-detail.md)).

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::collections::HashSet;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use ts_rs::TS;

/// What Mira knows about one running process.
///
/// Everything except `pid` and `name` is optional, because every one of them is a
/// fact some platform declines to give. An absent field is reported as absent
/// rather than filled with a plausible value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProcessFacts {
    /// The process identifier.
    #[ts(type = "number")]
    pub pid: u32,
    /// The process name, as the operating system reports it.
    pub name: String,
    /// The executable's path, where the platform allows reading it.
    pub executable: Option<String>,
    /// The parent process identifier, where one is reported.
    #[ts(type = "number | null")]
    pub parent: Option<u32>,
    /// The process's working directory.
    ///
    /// The single fact project attribution rests on. Windows does not expose it
    /// for another process, which is why attribution there is Degraded and says
    /// so instead of guessing (`platform-abstraction.md` §5).
    pub working_directory: Option<String>,
    /// Share of one CPU, as a percentage, averaged over the interval between the
    /// last two readings.
    ///
    /// `None` until there have been two. A share is a *rate*, and a rate needs
    /// two samples — the first reading of a process has nothing to subtract, so
    /// reporting `0.0` there would say "idle" about a process that might be
    /// burning a core. Measured: a process spinning a full core reads `0.00%` on
    /// a first sample and its true figure on the second
    /// ([ADR-0022](../../../docs/adr/0022-process-detail.md)).
    ///
    /// Can exceed 100 on a multi-core machine: two busy threads read ~200.
    #[ts(type = "number | null")]
    pub cpu_share: Option<f32>,
    /// Resident memory in bytes, where the platform reports it.
    #[ts(type = "number | null")]
    pub memory_bytes: Option<u64>,
    /// How long the process has been running, in seconds.
    #[ts(type = "number | null")]
    pub uptime_seconds: Option<u64>,
}

/// What Mira may ask about processes.
pub trait ProcessProvider {
    /// Facts for each of `pids` that is still running.
    ///
    /// A pid that has exited is absent from the result rather than an error:
    /// sockets outlive their processes by moments, and that is ordinary.
    fn facts_for(&self, pids: &[u32]) -> Vec<ProcessFacts>;
}

/// The real process table, read through `sysinfo`.
///
/// **Stateful, and it has to be.** `sysinfo` computes CPU share from the delta
/// between two refreshes of the *same* `System`, so a provider that built a
/// fresh one per call could only ever report zero — measured, against a process
/// provably burning a core, which read `0.00%` from a fresh `System` and its
/// true figure from a kept one.
///
/// So one `System` is kept for the life of the process and refreshed on each
/// observer tick. That is not a second clock: it holds no timer and schedules
/// nothing, and the ticks it rides are the scheduler's own (ADR-0011). The gap
/// between them — five seconds — is well above `sysinfo`'s 200 ms minimum, and
/// measurement showed the longer window is also the *steadier* one: the same
/// busy process read 238% over 200 ms, 200% over 500 ms and 100.3% over five
/// seconds. Needing no new timer and wanting the widest window turned out to be
/// the same answer ([ADR-0022](../../../docs/adr/0022-process-detail.md)).
#[derive(Debug)]
pub struct Processes {
    /// The kept reading. Behind a mutex because the observer thread and a
    /// command handler may both reach it, and because `refresh` mutates.
    system: Mutex<System>,
    /// Which pids have been sampled at least once.
    ///
    /// The difference between "idle" and "not yet measured". `sysinfo` reports
    /// `0.0` for both, and only this set can tell them apart.
    sampled: Mutex<HashSet<u32>>,
}

impl Default for Processes {
    fn default() -> Self {
        Self::new()
    }
}

impl Processes {
    /// A provider reading this machine, with nothing sampled yet.
    #[must_use]
    pub fn new() -> Self {
        Self {
            system: Mutex::new(System::new()),
            sampled: Mutex::new(HashSet::new()),
        }
    }

    /// What Mira asks `sysinfo` for.
    ///
    /// Named rather than inlined so the list is one thing to read: the
    /// executable, the working directory, cpu and memory. **Not `cmd`** — the
    /// command line is not read, not stored and not shown, because argv
    /// routinely carries credentials (ADR-0022).
    fn wanted() -> ProcessRefreshKind {
        ProcessRefreshKind::nothing()
            .with_exe(UpdateKind::Always)
            .with_cwd(UpdateKind::Always)
            .with_cpu()
            .with_memory()
    }
}

impl ProcessProvider for Processes {
    fn facts_for(&self, pids: &[u32]) -> Vec<ProcessFacts> {
        if pids.is_empty() {
            return Vec::new();
        }

        let mut wanted: Vec<Pid> = pids.iter().map(|pid| Pid::from_u32(*pid)).collect();
        wanted.sort_unstable();
        wanted.dedup();

        let Ok(mut system) = self.system.lock() else {
            return Vec::new();
        };
        // Refresh exactly the pids asked about, and only the fields Mira shows.
        // Anything wider would read the whole table to answer a question about
        // four processes.
        system.refresh_processes_specifics(ProcessesToUpdate::Some(&wanted), true, Self::wanted());

        let mut sampled = self.sampled.lock().ok();

        wanted
            .into_iter()
            .filter_map(|pid| {
                let raw = pid.as_u32();
                let process = system.process(pid)?;

                // A share is only reported once there is a previous reading to
                // subtract. Before that the honest answer is "not measured yet",
                // which is a different thing from "idle".
                let seen = sampled.as_mut().is_some_and(|seen| !seen.insert(raw));

                Some(ProcessFacts {
                    pid: raw,
                    name: process.name().to_string_lossy().into_owned(),
                    executable: process.exe().map(|path| path.display().to_string()),
                    parent: process.parent().map(Pid::as_u32),
                    working_directory: process.cwd().map(|path| path.display().to_string()),
                    cpu_share: seen.then(|| process.cpu_usage()),
                    memory_bytes: Some(process.memory()),
                    uptime_seconds: Some(process.run_time()),
                })
            })
            .collect()
    }
}
