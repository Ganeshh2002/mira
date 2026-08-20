//! What the application shell holds for the life of the process.

use std::path::PathBuf;
use std::sync::Arc;

use mira_db::Db;
use mira_platform::Platform;

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
}
