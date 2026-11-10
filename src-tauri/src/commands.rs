use crate::db::{self, ClipItem};
use crate::state::AppState;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::State;

type Res<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[tauri::command]
pub fn list_items(state: State<Arc<AppState>>, limit: Option<i64>) -> Res<Vec<ClipItem>> {
    let conn = state.db.lock().map_err(err)?;
    db::list(&conn, limit.unwrap_or(db::DEFAULT_LIMIT)).map_err(err)
}

#[tauri::command]
pub fn search_items(
    state: State<Arc<AppState>>,
    query: String,
    limit: Option<i64>,
) -> Res<Vec<ClipItem>> {
    let conn = state.db.lock().map_err(err)?;
    let limit = limit.unwrap_or(db::DEFAULT_LIMIT);
    if query.trim().is_empty() {
        db::list(&conn, limit).map_err(err)
    } else {
        db::search(&conn, &query, limit).map_err(err)
    }
}

#[tauri::command]
pub fn toggle_pin(state: State<Arc<AppState>>, id: i64) -> Res<bool> {
    let conn = state.db.lock().map_err(err)?;
    db::toggle_pin(&conn, id).map_err(err)
}

#[tauri::command]
pub fn delete_item(state: State<Arc<AppState>>, id: i64) -> Res<()> {
    let conn = state.db.lock().map_err(err)?;
    if let Some(path) = db::delete(&conn, id).map_err(err)? {
        let _ = std::fs::remove_file(path);
    }
    Ok(())
}

/// Clears history. Pinned items are always kept.
#[tauri::command]
pub fn clear_all(state: State<Arc<AppState>>) -> Res<()> {
    let conn = state.db.lock().map_err(err)?;
    for path in db::clear_unpinned(&conn).map_err(err)? {
        let _ = std::fs::remove_file(path);
    }
    Ok(())
}

/// Puts an item back on the system clipboard without pasting it.
#[tauri::command]
pub fn copy_item(state: State<Arc<AppState>>, id: i64) -> Res<()> {
    let item = {
        let conn = state.db.lock().map_err(err)?;
        db::get(&conn, id).map_err(err)?
    };

    let mut cb = arboard::Clipboard::new().map_err(err)?;

    match item.kind.as_str() {
        "image" => {
            let path = item.image_path.ok_or("image item has no file path")?;
            let (width, height, rgba) = crate::images::load_rgba(std::path::Path::new(&path))
                .ok_or_else(|| format!("could not read image file: {path}"))?;

            *state.last_written.lock().map_err(err)? = Some(crate::images::hash_bytes(&rgba));

            cb.set_image(arboard::ImageData {
                width,
                height,
                bytes: std::borrow::Cow::Owned(rgba),
            })
            .map_err(err)
        }
        _ => {
            let text = item.content.ok_or("item has no text content")?;
            *state.last_written.lock().map_err(err)? = Some(db::hash_text(&text));
            cb.set_text(text).map_err(err)
        }
    }
}

/// Returns a small PNG data URL for an image item, for the list preview.
#[tauri::command]
pub fn item_thumbnail(state: State<Arc<AppState>>, id: i64) -> Res<Option<String>> {
    let item = {
        let conn = state.db.lock().map_err(err)?;
        db::get(&conn, id).map_err(err)?
    };
    let Some(path) = item.image_path else {
        return Ok(None);
    };
    Ok(crate::images::thumbnail_data_url(std::path::Path::new(&path)))
}

#[tauri::command]
pub fn set_paused(state: State<Arc<AppState>>, paused: bool) -> Res<()> {
    state.paused.store(paused, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
pub fn is_paused(state: State<Arc<AppState>>) -> Res<bool> {
    Ok(state.paused.load(Ordering::Relaxed))
}

/// Hides the popup. Called when the user presses Esc on an empty search box.
#[tauri::command]
pub fn hide_popup(window: tauri::Window) -> Res<()> {
    window.hide().map_err(err)
}

/// Puts the item on the clipboard, hides the popup, then sends Ctrl+V to the
/// app that had focus. Returns whether the keystroke was attempted: when
/// auto-paste is off the item is only copied.
#[tauri::command]
pub fn paste_item(state: State<Arc<AppState>>, window: tauri::Window, id: i64) -> Res<bool> {
    copy_item(state.clone(), id)?;

    // Hide first: focus must return to the target app before Ctrl+V is sent.
    window.hide().map_err(err)?;

    let auto = {
        let conn = state.db.lock().map_err(err)?;
        db::get_setting(&conn, "auto_paste").map(|v| v != "false").unwrap_or(true)
    };

    if auto {
        crate::paste::send_paste_after_delay();
    }
    Ok(auto)
}

#[derive(serde::Serialize)]
pub struct Settings {
    pub auto_paste: bool,
    pub history_limit: i64,
    pub paused: bool,
    pub autostart: bool,
    pub hotkey: String,
    /// False when the shortcut could not be grabbed (taken by another app, or
    /// blocked by the desktop, as on Wayland).
    pub hotkey_active: bool,
}

#[tauri::command]
pub fn get_settings(app: tauri::AppHandle, state: State<Arc<AppState>>) -> Res<Settings> {
    use tauri_plugin_autostart::ManagerExt;
    let conn = state.db.lock().map_err(err)?;
    Ok(Settings {
        auto_paste: db::get_setting(&conn, "auto_paste")
            .map(|v| v != "false")
            .unwrap_or(true),
        history_limit: db::history_limit(&conn),
        paused: state.paused.load(Ordering::Relaxed),
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
        hotkey: crate::hotkey::stored(&conn),
        hotkey_active: state.hotkey.lock().map(|h| h.is_some()).unwrap_or(false),
    })
}

#[tauri::command]
pub fn set_auto_paste(state: State<Arc<AppState>>, enabled: bool) -> Res<()> {
    let conn = state.db.lock().map_err(err)?;
    db::set_setting(&conn, "auto_paste", if enabled { "true" } else { "false" }).map_err(err)
}

/// Changes the history limit and trims straight away, so the effect is visible.
#[tauri::command]
pub fn set_history_limit(state: State<Arc<AppState>>, limit: i64) -> Res<()> {
    let limit = limit.clamp(10, 5000);
    let conn = state.db.lock().map_err(err)?;
    db::set_setting(&conn, "history_limit", &limit.to_string()).map_err(err)?;
    for path in db::trim(&conn, limit).map_err(err)? {
        let _ = std::fs::remove_file(path);
    }
    Ok(())
}

#[tauri::command]
pub fn set_autostart(app: tauri::AppHandle, enabled: bool) -> Res<()> {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    if enabled {
        manager.enable().map_err(err)
    } else {
        manager.disable().map_err(err)
    }
}

/// Changes the popup shortcut. The new one is registered before the old one is
/// released, so a shortcut another app owns leaves the current one working.
#[tauri::command]
pub fn set_hotkey(app: tauri::AppHandle, state: State<Arc<AppState>>, accel: String) -> Res<()> {
    crate::hotkey::apply(&app, &accel)?;
    let conn = state.db.lock().map_err(err)?;
    db::set_setting(&conn, "hotkey", &accel).map_err(err)
}
