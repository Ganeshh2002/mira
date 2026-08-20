//! Windows Mira creates after start-up.
//!
//! Only the main window is declared in `tauri.conf.json`. A declared window costs
//! a webview process at launch whether or not it is visible — on macOS a hidden
//! second window measured about 46 MB — and the idle budget in
//! `product-definition.md` is 150 MB for the whole application. Settings is a
//! surface most people open rarely and close again, so it is built when it is
//! asked for and its cost is paid by the person who asked.

use mira_platform::SurfaceTreatment;
use tauri::utils::config::WindowEffectsConfig;
use tauri::window::Effect;
use tauri::{AppHandle, Manager, Runtime, WebviewUrl, WebviewWindowBuilder};

use crate::shortcut;

/// The label the frontend reads to decide which surface to render.
pub const SETTINGS: &str = "settings";

/// The main window's label.
pub const MAIN: &str = "main";

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

/// Ask the platform for its window material, and report what was achieved.
///
/// Mira does not draw a glass panel of its own. It asks the operating system for
/// its standard window material and lets the ground show through — which on
/// macOS 26 is Liquid Glass, on earlier macOS is vibrancy, and on Windows is Mica.
/// Nothing here checks a version, because the material is whatever the platform
/// currently means by it.
///
/// The return value is what actually happened, not what was requested. A machine
/// that refuses the effect — Windows 10, a compositor without blur — gets an
/// opaque ground, and the interface is told so rather than left translucent over
/// nothing (`platform-abstraction.md` §2, the parity rule).
pub fn apply_surface<R: Runtime>(app: &AppHandle<R>, wanted: SurfaceTreatment) -> SurfaceTreatment {
    if wanted == SurfaceTreatment::Opaque {
        return SurfaceTreatment::Opaque;
    }

    let Some(window) = app.get_webview_window(MAIN) else {
        return SurfaceTreatment::Opaque;
    };

    // `Mica` is ignored by macOS and the macOS materials are ignored by Windows,
    // so one list serves both without either shell branching on the other.
    let effects = WindowEffectsConfig {
        effects: vec![Effect::Mica, Effect::Sidebar],
        state: None,
        radius: None,
        color: None,
    };

    match window.set_effects(Some(effects)) {
        Ok(()) => SurfaceTreatment::SystemMaterial,
        Err(error) => {
            eprintln!("Mira could not apply the system window material: {error}");
            SurfaceTreatment::Opaque
        }
    }
}
