//! The menu bar / system tray.
//!
//! **Menu-first on every platform.** Linux tray icons never report clicks (a
//! libappindicator limitation, `platform-abstraction.md` §4.5), so no action in
//! Mira is ever reachable only by clicking the icon. Building the macOS and Windows
//! trays around clicks and bolting a menu on for Linux would produce exactly the
//! parity lie the product rules forbid, so there is one interaction model.
//!
//! If tray creation fails outright, Mira reports it once and keeps running.

use mira_platform::{KeepAwakeSpan, KeepAwakeState};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Runtime};

use crate::state::AppState;
use crate::{shortcut, windows};

const SHOW: &str = "show";
const SETTINGS: &str = "settings";
const QUIT: &str = "quit";

/// Keep Awake's menu items, so their check marks can be kept true.
///
/// The tray is where Keep Awake lives, because it is a machine-wide utility
/// rather than a fact about a project (`design-system.md` §8, and the menu-first
/// rule above). Menu items cannot be read back, so the handles are kept: a span
/// that ends on its own has to un-check itself, and there is no other way to say
/// so in a native menu.
pub struct AwakeMenu<R: Runtime> {
    items: SpanItems<R>,
}

/// One check item per span, paired with the span it stands for.
type SpanItems<R> = Vec<(KeepAwakeSpan, CheckMenuItem<R>)>;

/// The menu id for one span. `keep-awake:` prefixed so it cannot collide.
fn awake_id(span: KeepAwakeSpan) -> String {
    format!("keep-awake:{}", span.label())
}

/// Build the Keep Awake submenu, and hand back the items to keep.
fn awake_menu<R: Runtime>(
    app: &AppHandle<R>,
    state: &KeepAwakeState,
) -> tauri::Result<(Submenu<R>, SpanItems<R>)> {
    // An unavailable platform gets one disabled line carrying the reason, not a
    // set of controls that would do nothing. The reason is the product copy the
    // capability resolved (`platform-abstraction.md` §2 rule 4).
    if let KeepAwakeState::Unavailable { reason, .. } = state {
        let explanation = MenuItem::with_id(app, "keep-awake:off", reason, false, None::<&str>)?;
        let menu = Submenu::with_items(app, "Keep Awake", true, &[&explanation])?;
        return Ok((menu, Vec::new()));
    }

    let mut items = Vec::with_capacity(KeepAwakeSpan::ALL.len());
    for span in KeepAwakeSpan::ALL {
        items.push((
            span,
            CheckMenuItem::with_id(
                app,
                awake_id(span),
                span.label(),
                true,
                chosen(state) == span,
                None::<&str>,
            )?,
        ));
    }

    let entries: Vec<&dyn tauri::menu::IsMenuItem<R>> = items
        .iter()
        .map(|(_, item)| item as &dyn tauri::menu::IsMenuItem<R>)
        .collect();

    Ok((
        Submenu::with_items(app, "Keep Awake", true, &entries)?,
        items,
    ))
}

/// Which span a state corresponds to.
const fn chosen(state: &KeepAwakeState) -> KeepAwakeSpan {
    match state {
        KeepAwakeState::On { span, .. } => *span,
        KeepAwakeState::Off | KeepAwakeState::Unavailable { .. } => KeepAwakeSpan::Off,
    }
}

/// Make the menu's check marks say what is actually held.
///
/// Called after every change, including the one nobody made: a span reaching its
/// end un-checks itself here.
pub fn reflect<R: Runtime>(app: &AppHandle<R>, state: &KeepAwakeState) {
    let Some(menu) = app.try_state::<AwakeMenu<R>>() else {
        return;
    };
    let held = chosen(state);

    for (span, item) in &menu.items {
        let _ = item.set_checked(*span == held);
    }
}

/// Act on a Keep Awake menu selection.
fn choose<R: Runtime>(app: &AppHandle<R>, id: &str) {
    let Some(span) = KeepAwakeSpan::ALL
        .into_iter()
        .find(|span| awake_id(*span) == id)
    else {
        return;
    };
    let Some(state) = app.try_state::<std::sync::Arc<AppState>>() else {
        return;
    };

    match state.awake.set(span) {
        Ok(held) => reflect(app, &held),
        Err(error) => {
            // A menu has nowhere to show an error, so the check marks are put
            // back to what is actually true and Settings carries the reason.
            eprintln!("Mira could not change Keep Awake: {error}");
            reflect(app, &state.awake.state());
        }
    }
}

/// Build the tray icon and its menu.
pub fn build<R: Runtime>(
    app: &AppHandle<R>,
    shortcut_label: Option<&str>,
    awake: &KeepAwakeState,
) -> tauri::Result<()> {
    let show_label = match shortcut_label {
        Some(chord) => format!("Show Mira\t{chord}"),
        None => "Show Mira".to_owned(),
    };

    let (keep_awake, items) = awake_menu(app, awake)?;
    app.manage(AwakeMenu { items });

    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, SHOW, show_label, true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &keep_awake,
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
            other => choose(app, other),
        })
        .build(app)?;

    Ok(())
}
