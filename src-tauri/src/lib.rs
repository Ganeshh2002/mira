//! Mira's Tauri shell.
//!
//! This is the only crate in the workspace that depends on `tauri`
//! (ADR-0008 rule 2, enforced by a guard test). Everything below it is a plain
//! Rust library that tests with `cargo test`, with no webview and no app harness.
//!
//! Slice 0 wires the shell and nothing else: a window, a tray, a global shortcut,
//! a migrated database, and two typed commands that prove the spine works.

pub mod commands;
pub mod shortcut;
pub mod state;
pub mod tray;
pub mod windows;

use std::sync::Arc;

use mira_db::Db;
use mira_platform::Platform;
use tauri::Manager;

use state::AppState;

/// Build and run the application.
///
/// # Panics
///
/// Only if the database cannot be opened or migrated. That is a genuinely
/// unrecoverable start-up condition — Mira without its store is not a degraded
/// Mira, it is a different program — and it aborts with a message a person can act
/// on rather than starting into an inconsistent state.
pub fn run() {
    let platform = Platform::detect();
    let os = platform.facts().os;
    let (_, chord_label) = shortcut::default_chord(os);

    // Mira is a desktop application (ADR-0001); there is no mobile target to
    // compile these out for, so there is no cfg(target_os) here — and per ADR-0005
    // there is none anywhere outside `mira-platform`.
    tauri::Builder::default()
        // Single instance keeps two processes from fighting over one SQLite file,
        // and is what makes `mira --toggle` work as the Wayland fallback.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if argv.iter().any(|arg| arg == "--toggle") {
                shortcut::toggle_main_window(app);
            } else {
                shortcut::show_window(app, "main");
            }
        }))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    shortcut::on_shortcut(app, event.state());
                })
                .build(),
        )
        .setup(move |app| {
            let handle = app.handle().clone();

            let database_path = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("Mira could not find its data directory: {e}"))?
                .join("mira.db");

            let db = Db::open(&database_path).map_err(|e| e.to_string())?;

            let shortcut_registered = shortcut::register_default(&handle, &platform, os);

            if let Err(error) = tray::build(&handle, Some(chord_label)) {
                // A missing tray is a reduced Mira, not a broken one.
                eprintln!("Mira could not create its tray icon: {error}");
            }

            app.manage(AppState {
                db: Arc::new(db),
                platform: platform.clone(),
                database_path,
                shortcut_chord: chord_label.to_owned(),
                shortcut_registered,
            });

            // `mira --toggle` on the very first launch, before another instance exists.
            if std::env::args().any(|arg| arg == "--toggle") {
                shortcut::toggle_main_window(&handle);
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_foundation_status,
            commands::app::app_open_settings,
        ])
        .build(tauri::generate_context!())
        .expect("Mira failed to start")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                shutdown(app);
            }
        });
}

/// Leave the database as it would be found on a fresh machine.
///
/// Quitting goes through `app.exit`, which ends the process without running
/// destructors, so SQLite never gets the checkpoint it would normally perform when
/// the last connection closes. Doing it here is what makes the sentence Settings
/// shows — everything Mira knows is in this one file — true at rest.
fn shutdown<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    if let Err(error) = state.db.checkpoint() {
        eprintln!("Mira could not check its database in on the way out: {error}");
    }
}
