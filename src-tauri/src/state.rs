//! What the application shell holds for the life of the process.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use mira_db::Db;
use mira_fs::PathMatching;
use mira_git::Libgit2;
use mira_platform::{Os, Platform, SurfaceTreatment};
use mira_processes::Processes;
use mira_projects::Projects;

use crate::awake::Awake;
use crate::live::Live;

/// Everything a command handler may reach.
///
/// Constructed once during setup and shared immutably. There is no other global
/// state and no second channel into the database.
pub struct AppState {
    /// The SQLite store, already migrated.
    pub db: Arc<Db>,
    /// The machine, observed once at startup.
    pub platform: Platform,
    /// Where the database file lives, shown in Settings so the user can find it.
    pub database_path: PathBuf,
    /// The chord Mira tried to register.
    pub shortcut_chord: String,
    /// Whether that registration actually succeeded on this machine.
    pub shortcut_registered: bool,
    /// This operating system, held only so the platform layer can be handed it
    /// back. Nothing in `src-tauri` branches on it.
    pub os: Os,
    /// Whether this filesystem tells `Aviora` and `aviora` apart.
    pub matching: PathMatching,
    /// The window material actually achieved, which is not always the one asked
    /// for. The interface renders what this says, never what the OS implies.
    pub surface: SurfaceTreatment,
    /// What the observers last saw. In memory only — observation is never
    /// written to disk (`data-model.md` §1 rule 2).
    pub live: Live,
    /// The process reader, kept for the life of the process.
    ///
    /// Kept rather than built per tick because CPU share is a *rate*: `sysinfo`
    /// computes it from the delta between two refreshes of the same reading, so
    /// a fresh one each time could only ever report zero. It holds no timer and
    /// schedules nothing — it is refreshed by the scheduler's own ticks, which
    /// are five seconds apart and therefore also the steadiest window available
    /// ([ADR-0022](../../docs/adr/0022-process-detail.md), ADR-0011).
    pub processes: Processes,
    /// Whether the machine is being kept awake, and until when.
    ///
    /// In memory and in one operating-system request owned by this process.
    /// There is no table for it, so it cannot survive a restart — which is the
    /// property that matters most for something that changes how a machine
    /// behaves (ADR-0014).
    pub awake: Awake,
    /// How many projects exist, for the scheduler's gate to read cheaply.
    ///
    /// Kept beside the database rather than queried from it because the gate is
    /// consulted on every tick of every observer, and "is there anything to do"
    /// should not cost a query.
    pub project_count: AtomicUsize,
}

impl AppState {
    /// The project service, composed for this machine.
    ///
    /// Built per call rather than stored: it is three borrowed handles and a
    /// copy of one enum, so constructing it is free, and keeping it out of the
    /// struct means the database stays the only shared, long-lived thing here.
    #[must_use]
    pub fn projects(&self) -> Projects<&Db, Libgit2> {
        Projects::new(self.db.as_ref(), Libgit2, self.matching)
    }

    /// Record how many projects there are, so the gate can answer without a query.
    pub fn set_project_count(&self, count: usize) {
        self.project_count.store(count, Ordering::Relaxed);
    }

    /// Whether there is anything at all worth observing.
    #[must_use]
    pub fn has_projects(&self) -> bool {
        self.project_count.load(Ordering::Relaxed) > 0
    }
}
