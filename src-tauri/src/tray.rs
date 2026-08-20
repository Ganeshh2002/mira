//! The menu bar / system tray.
//!
//! **Menu-first on every platform.** Linux tray icons never report clicks (a
//! libappindicator limitation, `platform-abstraction.md` §4.5), so no action in
//! Mira is ever reachable only by clicking the icon. Building the macOS and Windows
//! trays around clicks and bolting a menu on for Linux would produce exactly the
//! parity lie the product rules forbid, so there is one interaction model.
//!
//! If tray creation fails outright, Mira reports it once and keeps running.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Runtime};

use crate::{shortcut, windows};

const SHOW: &str = "show";
const SETTINGS: &str = "settings";
const QUIT: &str = "quit";

/// Build the tray icon and its menu.
pub fn build<R: Runtime>(app: &AppHandle<R>, shortcut_label: Option<&str>) -> tauri::Result<()> {
    let show_label = match shortcut_label {
        Some(chord) => format!("Show Mira\t{chord}"),
        None => "Show Mira".to_owned(),
    };

    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, SHOW, show_label, true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, SETTINGS, "Settings", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, QUIT, "Quit Mira", true, None::<&str>)?,
        ],
    )?;

    TrayIconBuilder::with_id("mira")
        .icon(
            app.default_window_icon()
                .cloned()
                .ok_or_else(|| tauri::Error::AssetNotFound("the application icon".to_owned()))?,
        )
        .icon_as_template(true)
        .tooltip("Mira")
        .menu(&menu)
        // Clicks are not part of the interaction model — see the module comment.
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            SHOW => shortcut::toggle_main_window(app),
            SETTINGS => {
                if let Err(error) = windows::open_settings(app) {
                    eprintln!("Mira could not open Settings: {error}");
                }
            }
            QUIT => app.exit(0),
            _ => {}
        })
        .build(app)?;

    Ok(())
}
