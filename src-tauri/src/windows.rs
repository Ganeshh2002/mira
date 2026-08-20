//! Windows Mira creates after start-up.
//!
//! Only the main window is declared in `tauri.conf.json`. A declared window costs
//! a webview process at launch whether or not it is visible — on macOS a hidden
//! second window measured about 46 MB — and the idle budget in
//! `product-definition.md` is 150 MB for the whole application. Settings is a
//! surface most people open rarely and close again, so it is built when it is
//! asked for and its cost is paid by the person who asked.

use tauri::{AppHandle, Runtime, WebviewUrl, WebviewWindowBuilder};

use crate::shortcut;

/// The label the frontend reads to decide which surface to render.
pub const SETTINGS: &str = "settings";

/// Show Settings, creating the window the first time it is asked for.
///
/// Reuses the window on every later call, so the tray item and the in-app button
/// raise the same window rather than opening a second one.
pub fn open_settings<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    if shortcut::show_window(app, SETTINGS) {
        return Ok(());
    }

    WebviewWindowBuilder::new(app, SETTINGS, WebviewUrl::App("index.html".into()))
        .title("Mira Settings")
        .inner_size(620.0, 720.0)
        .min_inner_size(480.0, 400.0)
        .resizable(true)
        .center()
        .build()?;

    Ok(())
}
