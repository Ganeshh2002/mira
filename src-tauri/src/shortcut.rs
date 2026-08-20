//! Global shortcut plumbing.
//!
//! Slice 0 proves three things and nothing more: registration works, the event
//! reaches the application, and the application can show and focus its window. The
//! contextual command interface the chord will eventually summon is Slice 1.
//!
//! Whether to register at all is decided by `mira-platform`, not by an OS check
//! here — on Wayland the capability resolves to `Unavailable` with the documented
//! `mira --toggle` fallback, and Mira does not attempt a registration that would
//! fail (or, historically, crash inside libX11).

use mira_core::Capability;
use mira_platform::{Os, PlatformCapabilities};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutState};

/// The default chord, per `information-architecture.md` §7.
///
/// macOS uses `⌥Space`; every other platform uses `Ctrl+Alt+Space`. It is
/// rebindable from 0.1 onward; Slice 0 ships only the default.
#[must_use]
pub fn default_chord(os: Os) -> (Shortcut, &'static str) {
    match os {
        Os::MacOs => (
            Shortcut::new(Some(Modifiers::ALT), Code::Space),
            "Option+Space",
        ),
        _ => (
            Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::Space),
            "Ctrl+Alt+Space",
        ),
    }
}

/// Register the default chord if this machine can have one.
///
/// Returns whether a shortcut is now live. A `false` here is not an error: it is
/// the honest state a Wayland session is in, and Settings shows the fallback.
pub fn register_default<R: Runtime>(
    app: &AppHandle<R>,
    platform: &impl PlatformCapabilities,
    os: Os,
) -> bool {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    if !platform.status(Capability::GlobalShortcut).is_usable() {
        return false;
    }

    let (chord, _label) = default_chord(os);
    match app.global_shortcut().register(chord) {
        Ok(()) => true,
        Err(error) => {
            // Another application owns the chord. Mira keeps running and says so
            // rather than failing to start.
            eprintln!("Mira could not register its global shortcut: {error}");
            false
        }
    }
}

/// What happens when the chord fires: show the main window and focus it.
pub fn on_shortcut<R: Runtime>(app: &AppHandle<R>, event_state: ShortcutState) {
    if event_state != ShortcutState::Pressed {
        return;
    }
    toggle_main_window(app);
}

/// Show and focus the main window, or hide it if it is already in front.
///
/// This is also what `mira --toggle` reaches, which is the documented Wayland
/// fallback (`platform-abstraction.md` §4.1).
pub fn toggle_main_window<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    let visible = window.is_visible().unwrap_or(false);
    let focused = window.is_focused().unwrap_or(false);

    if visible && focused {
        let _ = window.hide();
    } else {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Reveal an existing window by label. `false` means there is no such window.
pub fn show_window<R: Runtime>(app: &AppHandle<R>, label: &str) -> bool {
    match app.get_webview_window(label) {
        Some(window) => {
            let _ = window.show();
            let _ = window.set_focus();
            true
        }
        None => false,
    }
}
