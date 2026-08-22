//! Mira's Tauri shell.
//!
//! This is the only crate in the workspace that depends on `tauri`
//! (ADR-0008 rule 2, enforced by a guard test). Everything below it is a plain
//! Rust library that tests with `cargo test`, with no webview and no app harness.
//!
//! Slice 0 wires the shell and nothing else: a window, a tray, a global shortcut,
//! a migrated database, and two typed commands that prove the spine works.

pub mod awake;
pub mod clock;
pub mod commands;
pub mod events;
pub mod gate;
pub mod live;
pub mod observers;
pub mod shortcut;
pub mod state;
pub mod tray;
pub mod windows;

use std::sync::Arc;

use mira_db::Db;
use mira_fs::PathMatching;
use mira_platform::{surface_treatment, Platform};
use mira_scheduler::Scheduler;
use std::sync::Mutex;
use tauri::{Emitter, Manager};

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
        // The picker runs in Rust on a user gesture; the webview is granted none of
        // this plugin's permissions (guard test).
        .plugin(tauri_plugin_dialog::init())
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
            let surface = windows::apply_surface(&handle, surface_treatment(platform.facts()));

            // Keep Awake tells the interface only when it changes on its own —
            // every other change is the answer to a command the interface made.
            // It also puts the tray's check marks back, because a span that ran
            // out has to un-check itself (`awake.rs`).
            let awake_announce: Arc<dyn Fn() + Send + Sync> = {
                let handle = handle.clone();
                Arc::new(move || {
                    let _ = handle.emit(events::KEEP_AWAKE, ());
                    if let Some(state) = handle.try_state::<Arc<AppState>>() {
                        tray::reflect(&handle, &state.awake.state());
                    }
                })
            };
            let awake = awake::Awake::new(platform.clone(), awake_announce);

            // Built before the tray, so the menu opens showing the truth: off,
            // on this and every other launch, because nothing about Keep Awake
            // survives the process that held it (ADR-0014).
            if let Err(error) = tray::build(&handle, Some(chord_label), &awake.state()) {
                // A missing tray is a reduced Mira, not a broken one.
                eprintln!("Mira could not create its tray icon: {error}");
            }

            let state = Arc::new(AppState {
                db: Arc::new(db),
                platform: platform.clone(),
                database_path,
                shortcut_chord: chord_label.to_owned(),
                shortcut_registered,
                os,
                matching: PathMatching::from_case_sensitivity(
                    platform.facts().paths_are_case_sensitive(),
                ),
                surface,
                live: live::Live::new(),
                awake,
                project_count: std::sync::atomic::AtomicUsize::new(0),
            });

            // The gate needs to know whether there is anything to observe before
            // the interface has asked for anything.
            if let Ok(count) = mira_db::ProjectRepo::count(state.db.as_ref()) {
                state.set_project_count(count as usize);
            }

            // Each round of observation tells the interface to re-read. The
            // notification carries no payload: it says something moved, and the
            // interface asks for what it needs (architecture.md §5). Passed as a
            // closure so the observers stay free of Tauri types.
            let announce: Arc<dyn Fn() + Send + Sync> = {
                let handle = handle.clone();
                Arc::new(move || {
                    let _ = handle.emit(events::LIVE, ());
                })
            };

            // The scheduler is started here and stopped in `shutdown`. Nothing
            // else in Mira may start a recurring task (architecture.md §6, and a
            // guard test).
            //
            // Set-up runs on the main thread, outside the async runtime, and
            // spawning from there panics. Entering the runtime for the length of
            // this call is what the guard is for.
            let runtime = tauri::async_runtime::handle();
            let _entered = runtime.inner().enter();

            let scheduler = Scheduler::start(
                vec![
                    Arc::new(observers::GitObserver::new(
                        Arc::clone(&state),
                        Arc::clone(&announce),
                    )),
                    Arc::new(observers::ServiceObserver::new(
                        Arc::clone(&state),
                        Arc::clone(&announce),
                    )),
                ],
                Arc::new(gate::WhenVisible::new(handle.clone(), Arc::clone(&state))),
            );

            // A first reading straight away, off the setup thread, so the window
            // does not open onto five seconds of nothing.
            {
                let state = Arc::clone(&state);
                let announce = Arc::clone(&announce);
                tauri::async_runtime::spawn_blocking(move || {
                    let _ = observers::observe_once(&state);
                    announce();
                });
            }

            app.manage(Live(Mutex::new(Some(scheduler))));
            app.manage(state);

            // `mira --toggle` on the very first launch, before another instance exists.
            if std::env::args().any(|arg| arg == "--toggle") {
                shortcut::toggle_main_window(&handle);
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_foundation_status,
            commands::app::app_open_settings,
            commands::projects::projects_list,
            commands::projects::projects_add,
            commands::projects::projects_open,
            commands::projects::projects_remove,
            commands::projects::projects_reveal,
            commands::live::live_snapshot,
            commands::live::live_refresh,
            commands::live::live_open_service,
            commands::git::git_history,
            commands::git::git_graph,
            commands::git::git_changes,
            commands::git::git_file_diff,
            commands::git::git_file_history,
            commands::git::git_search,
            commands::git::git_refs,
            commands::git::git_authors,
            commands::git::git_commit,
            commands::git::git_copy_commit,
            commands::awake::keep_awake_state,
            commands::awake::keep_awake_set,
            commands::workspaces::workspaces_list,
            commands::workspaces::workspaces_create,
            commands::workspaces::workspaces_rename,
            commands::workspaces::workspaces_open,
            commands::workspaces::workspaces_remove,
            commands::workspaces::workspaces_set_applications,
            commands::workspaces::workspaces_applications,
            commands::workspaces::workspaces_launch,
            commands::workspaces::workspaces_openable,
            commands::services::workspaces_services,
            commands::services::workspaces_service_offers,
            commands::services::workspaces_watch_service,
            commands::services::workspaces_forget_service,
            commands::services::workspaces_open_service,
        ])
        .build(tauri::generate_context!())
        .expect("Mira failed to start")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                shutdown(app);
            }
        });
}

/// The running scheduler, so shutdown can stop it.
///
/// Managed separately from [`AppState`] because it is the one piece of state that
/// is consumed rather than shared: stopping it takes ownership.
pub struct Live(Mutex<Option<Scheduler>>);

/// Leave the machine as it would be found on a fresh start.
///
/// Quitting goes through `app.exit`, which ends the process without running
/// destructors, so SQLite never gets the checkpoint it would normally perform when
/// the last connection closes. Doing it here is what makes the sentence Settings
/// shows — everything Mira knows is in this one file — true at rest.
fn shutdown<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    // Stop observing before the last write. A scheduler still running while the
    // database is checked in would be a read racing a close.
    if let Some(live) = app.try_state::<Live>() {
        if let Some(scheduler) = live.0.lock().ok().and_then(|mut held| held.take()) {
            tauri::async_runtime::block_on(scheduler.shutdown());
        }
    }

    let Some(state) = app.try_state::<Arc<AppState>>() else {
        return;
    };

    // Give the machine back before anything else. Quitting must leave the power
    // settings exactly as Mira found them, whatever else goes wrong on the way
    // out — which is also why nothing about Keep Awake is written down: there is
    // no state that could be left behind for the next launch to inherit.
    state.awake.shutdown();
    if let Err(error) = state.db.checkpoint() {
        eprintln!("Mira could not check its database in on the way out: {error}");
    }
}
