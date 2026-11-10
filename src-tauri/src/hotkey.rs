//! The popup shortcut, stored as an accelerator string so the user can change it.

use crate::state::AppState;
use rusqlite::Connection;
use std::str::FromStr;
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

/// Platform default.
///
/// Ctrl+Shift+V is the natural choice on Windows, matching Win+V. On Linux it is
/// already taken: terminals use it for paste and browsers and office suites use
/// it for "paste as plain text", so a global grab would shadow all of them.
#[cfg(windows)]
pub const DEFAULT: &str = "Ctrl+Shift+V";
#[cfg(not(windows))]
pub const DEFAULT: &str = "Ctrl+Alt+V";

pub fn stored(conn: &Connection) -> String {
    crate::db::get_setting(conn, "hotkey").unwrap_or_else(|| DEFAULT.to_string())
}

pub fn parse(accel: &str) -> Result<Shortcut, String> {
    Shortcut::from_str(accel).map_err(|e| format!("'{accel}' is not a valid shortcut: {e}"))
}

/// Registers `accel`, releasing whatever was registered before.
///
/// On failure the previous shortcut is left in place, so a shortcut another app
/// already owns cannot leave the popup with no way to open it.
pub fn apply(app: &AppHandle, accel: &str) -> Result<(), String> {
    let shortcut = parse(accel)?;
    let state = app.state::<Arc<AppState>>();
    let manager = app.global_shortcut();

    let previous = *state.hotkey.lock().map_err(|e| e.to_string())?;
    if previous == Some(shortcut) {
        return Ok(());
    }

    manager
        .register(shortcut)
        .map_err(|e| format!("could not register {accel}: {e}"))?;

    if let Some(old) = previous {
        let _ = manager.unregister(old);
    }

    *state.hotkey.lock().map_err(|e| e.to_string())? = Some(shortcut);
    Ok(())
}

/// True when `shortcut` is the one currently bound to the popup.
pub fn is_current(state: &AppState, shortcut: &Shortcut) -> bool {
    state
        .hotkey
        .lock()
        .map(|h| h.as_ref() == Some(shortcut))
        .unwrap_or(false)
}
