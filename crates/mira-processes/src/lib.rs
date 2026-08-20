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

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use serde::{Deserialize, Serialize};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use ts_rs::TS;

/// What Mira knows about one running process.
///
/// Everything except `pid` and `name` is optional, because every one of them is a
/// fact some platform declines to give. An absent field is reported as absent
/// rather than filled with a plausible value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
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
#[derive(Debug, Clone, Copy, Default)]
pub struct Processes;

impl Processes {
    /// A provider reading this machine.
    #[must_use]
    pub const fn new() -> Self {
        Self
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

        let mut system = System::new();
        // Refresh exactly the pids asked about, and only the fields Mira shows.
        // Anything wider would read the whole table to answer a question about
        // four processes.
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&wanted),
            true,
            ProcessRefreshKind::nothing()
                .with_exe(UpdateKind::Always)
                .with_cwd(UpdateKind::Always),
        );

        wanted
            .into_iter()
            .filter_map(|pid| {
                let process = system.process(pid)?;
                Some(ProcessFacts {
                    pid: pid.as_u32(),
                    name: process.name().to_string_lossy().into_owned(),
                    executable: process.exe().map(|path| path.display().to_string()),
                    parent: process.parent().map(Pid::as_u32),
                    working_directory: process.cwd().map(|path| path.display().to_string()),
                })
            })
            .collect()
    }
}
